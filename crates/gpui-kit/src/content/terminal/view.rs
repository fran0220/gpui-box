//! The grid, drawn, and the gestures over it reported.
//!
//! Ported from `crabtalk/bezel` (MIT); see `PROVENANCE.md`.
//!
//! This component holds no emulator. The host owns one, because the host is
//! what has the bytes, and it hands a snapshot back through
//! [`Terminal::grid`] once a frame. The callback shape is not indirection for
//! its own sake: how many columns fit is a *measurement*, taken from the
//! resolved monospace font during prepaint, and there is no earlier moment at
//! which the answer exists. So the element measures, reports the grid it
//! measured, and receives what to paint.
//!
//! What a pointer does is reported as a [`TerminalEvent`] and nothing else.
//! Selection lives in the emulator, so a press reports the cell it hit and the
//! host anchors it.
//!
//! Keystrokes are not handled here, and that is a boundary rather than a gap.
//! A terminal takes every key on the keyboard, so a component that installed a
//! key handler would decide what the application's own bindings are; and the
//! focus handle the handler would need is the host's, which is why
//! [`Terminal::focused`] is told about focus rather than holding it. The host
//! encodes with [`super::keystroke_bytes`], which needs the emulator's cursor
//! mode — state the host owns — and buffers with [`super::InputCoalescer`].
//!
//! The only state this keeps is which button is down and where it went down:
//! hover, focus and drag, the transient visual state a component is allowed.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, GlobalElementId, Hsla, InteractiveElement, IntoElement, LayoutId,
    MouseButton, PaintQuad, ParentElement, Pixels, Point, RenderOnce, ShapedLine, SharedString,
    Style, Styled, TextRun, Window, div, fill, font, outline, point, px, relative, size,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Surface, Theme};

use super::emulator::{CellColor, CellSnapshot, CursorSnapshot, SelectionKind};
use super::input::{CellHit, SELECTION_DRAG_THRESHOLD, cell_at};
use super::palette::resolve;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::loading::PulseLoader;
use crate::foundation::{Ident, StyledExt};
use crate::motion::keyed;
use crate::strings::{ActiveStrings, StringKey};

/// The inset between the panel edge and the first glyph.
///
/// It occurs once and is geometry rather than a semantic space step: it is
/// subtracted from the measured box before the column count is derived, so
/// changing it changes how many columns fit.
const GRID_PADDING: f32 = 12.0;

/// The widest and tallest grid this will report.
///
/// A window mid-animation can measure absurd, and a program told it has 40,000
/// columns allocates for 40,000 columns. The clamp is on the number handed
/// out, not on the box.
const MAX_COLS: i64 = 500;
const MAX_ROWS: i64 = 500;

/// What SGR 2 does to a colour.
///
/// A protocol constant rather than a theme one: "dim" is defined by the
/// escape sequence, it occurs in exactly one place, and a theme that made it
/// configurable would be answering a question nobody asked about a value the
/// program already chose.
const DIM_ALPHA: f32 = 0.6;

/// What is known about the session behind the grid.
///
/// A terminal whose process never started and one whose process ended are not
/// the same thing as one with nothing on screen yet, and an empty grid is a
/// perfectly ordinary Ready terminal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TerminalState {
    /// A session is being opened. Nothing has been drawn yet.
    Loading,
    /// No session could be opened, and the host says why.
    Unavailable(SharedString),
    /// The session ended or failed, and the host says why. Whatever was last
    /// on the grid is not erased by this: a reader still needs to see it.
    Error(SharedString),
    #[default]
    Ready,
}

/// What the grid reports. Every variant is the host's to act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    /// A gesture began on a cell. The row is a *viewport* row, so the host
    /// turns it into a grid point with `Emulator::grid_point`, which is the
    /// only thing that knows how far the view is scrolled back.
    SelectionStarted {
        hit: CellHit,
        kind: SelectionKind,
    },
    SelectionUpdated {
        hit: CellHit,
    },
    SelectionCleared,
    /// The wheel asked to move the view, in lines; positive goes into history.
    Scrolled {
        lines: i32,
    },
}

/// The grid the element measured, in the coordinates a pointer arrives in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridGeometry {
    /// The top-left of the first glyph, in window space.
    pub origin: Point<Pixels>,
    pub cell_width: f32,
    pub line_height: f32,
    pub cols: u16,
    pub rows: u16,
}

/// What the host hands back to be painted.
pub struct GridSnapshot {
    pub lines: Vec<Vec<CellSnapshot>>,
    pub cursor: Option<CursorSnapshot>,
}

type GridHook = Rc<dyn Fn(GridGeometry, &mut App) -> Option<GridSnapshot>>;
type EventHandler = Rc<dyn Fn(TerminalEvent, &mut Window, &mut App)>;

/// A terminal grid.
#[derive(IntoElement)]
pub struct Terminal {
    ident: Ident,
    state: TerminalState,
    grid: Option<GridHook>,
    on_event: Option<EventHandler>,
    focused: bool,
    scrollback: bool,
}

impl Terminal {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            state: TerminalState::Ready,
            grid: None,
            on_event: None,
            focused: false,
            scrollback: true,
        }
    }

    /// What is known about the session.
    pub fn state(mut self, state: TerminalState) -> Self {
        self.state = state;
        self
    }

    /// The host's once-a-frame hook: it is told the grid that fits and returns
    /// what to draw in it. Without one the panel draws its chrome and nothing
    /// else, which is what a terminal with no session looks like.
    pub fn grid(
        mut self,
        grid: impl Fn(GridGeometry, &mut App) -> Option<GridSnapshot> + 'static,
    ) -> Self {
        self.grid = Some(Rc::new(grid));
        self
    }

    /// Whether the grid holds keyboard focus. A focused cursor is a filled
    /// block and an unfocused one an outline, which is the difference between
    /// "typing goes here" and "this is where typing left off".
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Whether the wheel scrolls the view. A full-screen program on the
    /// alternate screen has no scrollback, and scrolling one is how a reader
    /// ends up dragging a view that cannot move.
    pub fn scrollback(mut self, scrollback: bool) -> Self {
        self.scrollback = scrollback;
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(TerminalEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

/// What a press left behind, so the move that follows knows what it is.
#[derive(Default)]
struct Gesture {
    /// Where the button went down, in window space, and `None` once the button
    /// is up. The position is kept rather than a flag because the drag
    /// threshold is a distance from it.
    pressed_at: Option<Point<Pixels>>,
    /// Whether the press has already travelled far enough to be a selection.
    dragging: bool,
}

impl RenderOnce for Terminal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();
        let state = self.state.clone();

        let geometry =
            keyed::slot::<Option<GridGeometry>>(&ident.child("geometry").semantic_id(), cx);
        let gesture = keyed::slot::<Gesture>(&ident.child("gesture").semantic_id(), cx);

        let mut panel = div()
            .id(ident.element_id())
            .relative()
            .size_full()
            .overflow_hidden()
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Raised, Elevation::Raised)
            .bg(theme.colors.terminal_background);

        if matches!(state, TerminalState::Ready | TerminalState::Error(_))
            && let Some(grid) = self.grid.clone()
        {
            let report = Rc::clone(&geometry);
            panel = panel.child(TerminalElement {
                grid,
                focused: self.focused,
                theme: theme.clone(),
                report: Box::new(move |measured| *report.borrow_mut() = Some(measured)),
            });
        }

        if let Some(handler) = self.on_event.clone() {
            panel = wire_pointer(panel, &geometry, &gesture, &handler, self.scrollback);
        }

        let overlay = state_overlay(&ident, &state, cx);

        panel.children(overlay).semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Region)
                .text(cx.strings().text(StringKey::TerminalGrid))
                .busy(matches!(state, TerminalState::Loading))
                .invalid(matches!(state, TerminalState::Error(_)))
                // The measured grid, which is the one fact about a
                // terminal a test can assert without reading the output
                // somebody else's program wrote.
                .value(match *geometry.borrow() {
                    Some(measured) => format!("{}x{}", measured.cols, measured.rows),
                    None => String::new(),
                }),
        )
    }
}

/// The reason overlay, or nothing when the grid speaks for itself.
fn state_overlay(ident: &Ident, state: &TerminalState, cx: &App) -> Option<AnyElement> {
    let strings = cx.strings();
    match state {
        TerminalState::Ready => None,
        TerminalState::Loading => Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    PulseLoader::new(ident.child("loading"))
                        .label(strings.text(StringKey::TerminalStarting)),
                )
                .into_any_element(),
        ),
        TerminalState::Unavailable(reason) => Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    EmptyState::new(
                        ident.child("unavailable"),
                        strings.text(StringKey::TerminalUnavailable),
                    )
                    .kind(EmptyKind::Unavailable)
                    .detail(reason.clone()),
                )
                .into_any_element(),
        ),
        // An ended session keeps its output on screen and says so underneath,
        // because the last thing the program printed is usually the reason.
        TerminalState::Error(reason) => Some(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .child(
                    EmptyState::new(ident.child("error"), strings.text(StringKey::TerminalError))
                        .kind(EmptyKind::Failed)
                        .detail(reason.clone()),
                )
                .into_any_element(),
        ),
    }
}

/// Attach the pointer gestures to the panel.
///
/// Split out because the wiring is four handlers that share one piece of
/// state, and threading them through `render` buried what the component does.
fn wire_pointer(
    panel: gpui::Stateful<gpui::Div>,
    geometry: &Rc<std::cell::RefCell<Option<GridGeometry>>>,
    gesture: &Rc<std::cell::RefCell<Gesture>>,
    handler: &EventHandler,
    scrollback: bool,
) -> gpui::Stateful<gpui::Div> {
    /// A window position, as a hit on the grid last measured.
    fn hit(measured: GridGeometry, position: Point<Pixels>) -> CellHit {
        cell_at(
            f32::from(position.x - measured.origin.x),
            f32::from(position.y - measured.origin.y),
            measured.cell_width,
            measured.line_height,
            measured.cols as usize,
            measured.rows as usize,
        )
    }

    let down_geometry = Rc::clone(geometry);
    let down_gesture = Rc::clone(gesture);
    let down_handler = Rc::clone(handler);
    let mut panel = panel.on_mouse_down(MouseButton::Left, move |event, window, cx| {
        let Some(measured) = *down_geometry.borrow() else {
            return;
        };
        {
            let mut gesture = down_gesture.borrow_mut();
            gesture.pressed_at = Some(event.position);
            gesture.dragging = false;
        }
        let hit = hit(measured, event.position);
        // A multiple-click is a selection outright: there is no drag to wait
        // for, and waiting would make a double-click do nothing.
        match event.click_count {
            2 => down_handler(
                TerminalEvent::SelectionStarted {
                    hit,
                    kind: SelectionKind::Word,
                },
                window,
                cx,
            ),
            count if count >= 3 => down_handler(
                TerminalEvent::SelectionStarted {
                    hit,
                    kind: SelectionKind::Line,
                },
                window,
                cx,
            ),
            _ => down_handler(TerminalEvent::SelectionCleared, window, cx),
        }
    });

    let move_geometry = Rc::clone(geometry);
    let move_gesture = Rc::clone(gesture);
    let move_handler = Rc::clone(handler);
    panel = panel.on_mouse_move(move |event, window, cx| {
        let Some(measured) = *move_geometry.borrow() else {
            return;
        };
        let (pressed_at, dragging) = {
            let gesture = move_gesture.borrow();
            (gesture.pressed_at, gesture.dragging)
        };
        let Some(pressed_at) = pressed_at else {
            return;
        };
        // A button released outside the panel never delivers an up here, so
        // the press is ended by the first move that reports it gone.
        if event.pressed_button != Some(MouseButton::Left) {
            let mut gesture = move_gesture.borrow_mut();
            gesture.pressed_at = None;
            gesture.dragging = false;
            return;
        }
        if !dragging {
            let travelled = f32::from(event.position.x - pressed_at.x).abs()
                + f32::from(event.position.y - pressed_at.y).abs();
            if travelled < SELECTION_DRAG_THRESHOLD {
                return;
            }
            move_gesture.borrow_mut().dragging = true;
            move_handler(
                TerminalEvent::SelectionStarted {
                    hit: hit(measured, pressed_at),
                    kind: SelectionKind::Drag,
                },
                window,
                cx,
            );
        }
        move_handler(
            TerminalEvent::SelectionUpdated {
                hit: hit(measured, event.position),
            },
            window,
            cx,
        );
    });

    let up_gesture = Rc::clone(gesture);
    panel = panel.on_mouse_up(MouseButton::Left, move |_, _, _| {
        let mut gesture = up_gesture.borrow_mut();
        gesture.pressed_at = None;
        gesture.dragging = false;
    });

    if scrollback {
        let scroll_handler = Rc::clone(handler);
        let scroll_geometry = Rc::clone(geometry);
        panel = panel.on_scroll_wheel(move |event, window, cx| {
            let Some(measured) = *scroll_geometry.borrow() else {
                return;
            };
            let lines = match event.delta {
                gpui::ScrollDelta::Lines(delta) => delta.y,
                gpui::ScrollDelta::Pixels(delta) => f32::from(delta.y) / measured.line_height,
            };
            let lines = lines.round() as i32;
            if lines != 0 {
                scroll_handler(TerminalEvent::Scrolled { lines }, window, cx);
            }
        });
    }

    panel
}

/// Measures the grid and paints it.
///
/// A custom element rather than a tree of divs because a terminal is one
/// measurement and thousands of cells: the cell size comes from the font
/// system at prepaint, and a div per cell would be a layout pass per frame
/// over an entire screen of text.
struct TerminalElement {
    grid: GridHook,
    focused: bool,
    theme: Theme,
    report: Box<dyn Fn(GridGeometry)>,
}

struct TerminalPrepaint {
    background: Vec<PaintQuad>,
    /// The selection wash, painted over the cell backgrounds and under the
    /// glyphs: it has to tint a cell's own colour rather than replace it, and
    /// it must not bury the text it is highlighting.
    selection: Vec<PaintQuad>,
    /// Per row, each shaped segment and the grid *column* it starts at. Not
    /// one line per row; see [`shape_row`].
    rows: Vec<Vec<(usize, ShapedLine)>>,
    cell_width: Pixels,
    line_height: Pixels,
    origin: Point<Pixels>,
    cursor: Option<PaintQuad>,
}

impl gpui::IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = TerminalPrepaint;

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let theme = &self.theme;
        let font_size = px(theme.typography.code.size);
        let line_height = px(theme.typography.code.line_height);

        // Ligatures off. A terminal is a fixed grid, so the shaper has to emit
        // one cell of advance per character; a contextual substitution turns
        // `-->` into one glyph, the row paints short, and the cursor — a quad
        // at `cell_width * col` — stays on the true column while the text
        // beside it does not.
        let mut mono = font(theme.typography.mono.clone());
        mono.features = gpui::FontFeatures(std::sync::Arc::new(vec![
            ("liga".into(), 0),
            ("calt".into(), 0),
            ("dlig".into(), 0),
        ]));

        // The font probe: the cell is whatever this font's em advance actually
        // is, measured, rather than an aspect ratio guessed from the size.
        let font_id = window.text_system().resolve_font(&mono);
        let cell_width = window
            .text_system()
            .em_advance(font_id, font_size)
            .unwrap_or(font_size * 0.6);

        let inner_width = f32::from(bounds.size.width) - 2.0 * GRID_PADDING;
        let inner_height = f32::from(bounds.size.height) - 2.0 * GRID_PADDING;
        let cols = ((inner_width / f32::from(cell_width)).floor() as i64).clamp(2, MAX_COLS) as u16;
        let rows =
            ((inner_height / f32::from(line_height)).floor() as i64).clamp(1, MAX_ROWS) as u16;
        let origin = point(
            bounds.left() + px(GRID_PADDING),
            bounds.top() + px(GRID_PADDING),
        );

        let measured = GridGeometry {
            origin,
            cell_width: f32::from(cell_width),
            line_height: f32::from(line_height),
            cols,
            rows,
        };
        (self.report)(measured);

        let empty = TerminalPrepaint {
            background: Vec::new(),
            selection: Vec::new(),
            rows: Vec::new(),
            cell_width,
            line_height,
            origin,
            cursor: None,
        };
        let Some(snapshot) = (self.grid)(measured, cx) else {
            return empty;
        };

        let mut background = Vec::new();
        let mut selection = Vec::new();
        let mut shaped = Vec::with_capacity(snapshot.lines.len());

        for (index, row) in snapshot.lines.iter().enumerate() {
            let y = origin.y + line_height * index as f32;
            let quad = |start: usize, end: usize, color: Hsla| {
                fill(
                    Bounds::new(
                        point(origin.x + cell_width * start as f32, y),
                        size(cell_width * (end - start) as f32, line_height),
                    ),
                    color,
                )
            };

            // One quad per contiguous run rather than one per cell: a screen
            // of coloured output is otherwise tens of thousands of quads.
            let mut run: Option<usize> = None;
            for col in 0..=row.len() {
                match (run, row.get(col).is_some_and(|cell| cell.selected)) {
                    (None, true) => run = Some(col),
                    (Some(start), false) => {
                        selection.push(quad(start, col, theme.colors.terminal_selection));
                        run = None;
                    }
                    _ => {}
                }
            }

            let mut run: Option<(usize, Hsla)> = None;
            for (col, color) in row
                .iter()
                .map(|cell| cell.display_colors().1)
                // A sentinel default past the end, so the last run closes
                // without repeating the flush after the loop.
                .chain(std::iter::once(CellColor::Background))
                .enumerate()
            {
                let paint = match color {
                    CellColor::Background => None,
                    other => Some(resolve(other, theme)),
                };
                match (run, paint) {
                    (None, Some(color)) => run = Some((col, color)),
                    (Some((start, current)), next) if next != Some(current) => {
                        background.push(quad(start, col, current));
                        run = next.map(|color| (col, color));
                    }
                    _ => {}
                }
            }

            shaped.push(shape_row(row, theme, &mono, font_size, window));
        }

        let cursor = snapshot.cursor.map(|cursor| {
            let cell = Bounds::new(
                point(
                    origin.x + cell_width * cursor.col as f32,
                    origin.y + line_height * cursor.row as f32,
                ),
                size(cell_width, line_height),
            );
            if self.focused {
                fill(cell, theme.colors.accent)
            } else {
                outline(cell, theme.colors.accent, gpui::BorderStyle::Solid)
            }
        });

        TerminalPrepaint {
            background,
            selection,
            rows: shaped,
            cursor,
            ..empty
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (origin, cell_width, line_height) =
            (prepaint.origin, prepaint.cell_width, prepaint.line_height);
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            for quad in prepaint.background.drain(..) {
                window.paint_quad(quad);
            }
            for quad in prepaint.selection.drain(..) {
                window.paint_quad(quad);
            }
            for (index, segments) in prepaint.rows.iter().enumerate() {
                let y = origin.y + line_height * index as f32;
                for (col, line) in segments {
                    let _ = line.paint(
                        point(origin.x + cell_width * *col as f32, y),
                        line_height,
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                }
            }
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        });
    }
}

/// Shape one row into segments pinned to the columns they start at.
///
/// A shaped line places glyphs by their font advances, and those agree with
/// the grid only while every glyph is the monospace font's own width. The
/// moment a character resolves through *font fallback* — box drawing, arrows,
/// emoji, CJK — its advance is whatever that other font uses, and the rest of
/// the row slides out of the grid: a box border lands a few pixels off, and a
/// double-width glyph whose fallback advances one cell swallows the column
/// after it. The quads never drift, because they are placed at
/// `cell_width * col`, which is exactly what makes the drift visible.
///
/// So a run of ASCII shapes together, and anything else is its own segment
/// pinned at its own column. A wide spacer is skipped: the glyph before it
/// covers both columns, and the next segment re-pins regardless.
fn shape_row(
    row: &[CellSnapshot],
    theme: &Theme,
    mono: &gpui::Font,
    font_size: Pixels,
    window: &Window,
) -> Vec<(usize, ShapedLine)> {
    fn flush(
        segments: &mut Vec<(usize, ShapedLine)>,
        text: &mut String,
        runs: &mut Vec<TextRun>,
        column: usize,
        font_size: Pixels,
        window: &Window,
    ) {
        if text.is_empty() {
            return;
        }
        let shaped = window.text_system().shape_line(
            SharedString::from(std::mem::take(text)),
            font_size,
            runs,
            None,
        );
        segments.push((column, shaped));
        runs.clear();
    }

    let mut segments: Vec<(usize, ShapedLine)> = Vec::new();
    let mut text = String::with_capacity(row.len());
    let mut runs: Vec<TextRun> = Vec::new();
    let mut column = 0usize;

    for (col, cell) in row.iter().enumerate() {
        if cell.wide_spacer {
            continue;
        }
        let ch = if cell.hidden { ' ' } else { cell.ch };
        let pinned = !ch.is_ascii() || cell.wide;
        if pinned {
            flush(
                &mut segments,
                &mut text,
                &mut runs,
                column,
                font_size,
                window,
            );
        }
        if text.is_empty() {
            column = col;
        }

        let (foreground, _) = cell.display_colors();
        let mut color = resolve(foreground, theme);
        if cell.dim {
            color.a *= DIM_ALPHA;
        }
        let mut cell_font = mono.clone();
        cell_font.weight = if cell.bold {
            gpui::FontWeight::BOLD
        } else {
            gpui::FontWeight::NORMAL
        };
        cell_font.style = if cell.italic {
            gpui::FontStyle::Italic
        } else {
            gpui::FontStyle::Normal
        };
        let underline = cell.underline.then_some(gpui::UnderlineStyle {
            color: Some(color),
            thickness: px(theme.borders.hairline),
            wavy: false,
        });

        let len = ch.len_utf8();
        text.push(ch);
        match runs.last_mut() {
            Some(last)
                if last.color == color && last.font == cell_font && last.underline == underline =>
            {
                last.len += len;
            }
            _ => runs.push(TextRun {
                len,
                font: cell_font,
                color,
                background_color: None,
                background_radius: None,
                underline,
                strikethrough: None,
            }),
        }
        if pinned {
            flush(
                &mut segments,
                &mut text,
                &mut runs,
                column,
                font_size,
                window,
            );
        }
    }
    flush(
        &mut segments,
        &mut text,
        &mut runs,
        column,
        font_size,
        window,
    );
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_terminal_is_ready_until_it_is_told_otherwise() {
        let terminal = Terminal::new("session.shell");
        assert_eq!(terminal.state, TerminalState::Ready);
        assert!(terminal.scrollback);
        assert!(!terminal.focused);
    }

    #[test]
    fn an_ended_session_is_not_the_same_as_one_that_never_started() {
        let ended = Terminal::new("session.shell").state(TerminalState::Error("exit 1".into()));
        let never =
            Terminal::new("session.shell").state(TerminalState::Unavailable("no shell".into()));
        assert_ne!(ended.state, never.state);
    }

    #[test]
    fn a_terminal_without_a_grid_hook_installs_no_element() {
        // A component with no session draws its chrome and nothing else,
        // rather than an empty grid that claims a session exists.
        assert!(Terminal::new("session.shell").grid.is_none());
    }

    #[test]
    fn a_terminal_without_a_handler_reports_nothing() {
        assert!(Terminal::new("session.shell").on_event.is_none());
    }
}
