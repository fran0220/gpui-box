//! The escape-sequence state machine: bytes in, grid snapshots out.
//!
//! Ported from `crabtalk/bezel` (MIT); see `PROVENANCE.md`. The state machine
//! itself is `alacritty_terminal`'s `Term` driven by vte's ANSI `Processor`,
//! and this module is the wrapper that makes it a pure fold with no I/O in it:
//! [`Emulator::feed`] advances the machine over bytes somebody else read, and
//! [`Emulator::lines`] hands back what is on the grid. Nothing here opens a
//! pty, spawns a process, or schedules anything — that is the host's, and it
//! is why the whole escape-sequence surface is testable with byte strings.
//!
//! Selection lives here rather than in the caller because `Term` is what knows
//! how to keep an anchor on its text while output scrolls the grid underneath
//! it. The caller supplies pointer positions; this translates them.
//!
//! No `alacritty_terminal` type appears in this module's public API. A grid
//! point, a selection granularity and a cell edge are all named here, and the
//! conversions are private, so a consumer of this library never has to depend
//! on the emulator crate to speak to the emulator.

use std::{cell::RefCell, rc::Rc};

use alacritty_terminal::{
    event::{Event, EventListener},
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point, Side as AnsiSide},
    selection::{Selection, SelectionRange, SelectionType},
    term::{Config, Term, TermMode, cell::Flags},
    vte::ansi::{Color as AnsiColor, CursorShape, NamedColor, Processor, Rgb as AnsiRgb},
};

/// Scrollback kept on the client, in lines.
///
/// It bounds what stays scrollable in the view and nothing else: whatever
/// window of output the host keeps for itself is a separate decision, made
/// where the output comes from.
pub const SCROLLBACK_LINES: usize = 10_000;

/// The viewport, in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSize {
    pub cols: u16,
    pub rows: u16,
}

impl GridSize {
    /// Clamps to a grid that can hold something. A zero-column terminal is not
    /// a smaller terminal, it is a division by zero waiting to happen, and the
    /// caller measuring one has a window mid-collapse rather than an opinion.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols: cols.max(2),
            rows: rows.max(1),
        }
    }
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }

    fn screen_lines(&self) -> usize {
        self.rows as usize
    }

    fn columns(&self) -> usize {
        self.cols as usize
    }
}

/// A place on the grid, in the coordinates a *selection* anchors to.
///
/// `line` is signed and counts down from the live viewport, so a negative line
/// is scrollback. That is the difference between this and a viewport row: a
/// viewport row is what a pointer hits and moves out from under the text as
/// output arrives, while a grid point stays on the text it named. Anchoring in
/// grid space is the whole reason a selection survives the screen scrolling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridPoint {
    pub line: i32,
    pub column: usize,
}

impl GridPoint {
    pub fn new(line: i32, column: usize) -> Self {
        Self { line, column }
    }

    fn into_ansi(self) -> Point {
        Point::new(Line(self.line), Column(self.column))
    }
}

/// Which edge of a cell an anchor sits on.
///
/// A selection anchors to an edge, not to a cell: pressing on the left half of
/// a glyph includes it, the right half excludes it. Without the distinction a
/// drag cannot express "up to but not including this character".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellSide {
    Left,
    Right,
}

impl CellSide {
    fn into_ansi(self) -> AnsiSide {
        match self {
            Self::Left => AnsiSide::Left,
            Self::Right => AnsiSide::Right,
        }
    }
}

/// How much a selection takes per gesture: a drag, a double-click word, a
/// triple-click row.
///
/// The granularity is the emulator's business rather than the caller's because
/// deciding where a word ends means reading the grid, and a caller that did it
/// from the text it can see would disagree with the copy it gets back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    Drag,
    Word,
    Line,
}

impl SelectionKind {
    fn into_ansi(self) -> SelectionType {
        match self {
            Self::Drag => SelectionType::Simple,
            Self::Word => SelectionType::Semantic,
            Self::Line => SelectionType::Lines,
        }
    }
}

/// What a cell says its colour is, before a theme decides what that means.
///
/// Kept symbolic on purpose. `Foreground` and `Background` are the terminal's
/// defaults and follow the theme; an index is a named slot or a point in the
/// colour cube; and only `Rgb` is a colour the program actually chose. A
/// snapshot that resolved these to pixels would have to be rebuilt on every
/// theme change, and would lose the distinction between "red" and `#ff0000`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellColor {
    Foreground,
    Background,
    /// 0-15 the ANSI slots, 16-231 the colour cube, 232-255 the grey ramp.
    Indexed(u8),
    Rgb(u8, u8, u8),
}

fn map_color(color: AnsiColor) -> CellColor {
    match color {
        AnsiColor::Spec(AnsiRgb { r, g, b }) => CellColor::Rgb(r, g, b),
        AnsiColor::Indexed(index) => CellColor::Indexed(index),
        AnsiColor::Named(named) => {
            let index = named as usize;
            if index < 16 {
                return CellColor::Indexed(index as u8);
            }
            match named {
                NamedColor::Background => CellColor::Background,
                // A dim named colour folds onto its base slot; the DIM flag
                // still travels on the cell, so the dimming happens once, at
                // paint time, rather than being baked into a second palette.
                NamedColor::DimBlack
                | NamedColor::DimRed
                | NamedColor::DimGreen
                | NamedColor::DimYellow
                | NamedColor::DimBlue
                | NamedColor::DimMagenta
                | NamedColor::DimCyan
                | NamedColor::DimWhite => {
                    CellColor::Indexed((index - NamedColor::DimBlack as usize) as u8)
                }
                _ => CellColor::Foreground,
            }
        }
    }
}

/// One cell as it stands: the character, its colours, and the attributes that
/// change how it is painted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellSnapshot {
    pub ch: char,
    pub fg: CellColor,
    pub bg: CellColor,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    pub hidden: bool,
    /// A double-width character, covering this cell and the spacer after it.
    pub wide: bool,
    /// The spacer half of a wide character. Never shaped; only its background
    /// is painted, because the glyph beside it already covers this column.
    pub wide_spacer: bool,
    pub selected: bool,
}

impl CellSnapshot {
    /// The colours to paint with, after INVERSE and HIDDEN are resolved.
    ///
    /// Both are attributes about painting rather than about the text, so they
    /// are resolved here and not stored twice: a cell keeps what the program
    /// set, and this answers what a painter should use.
    pub fn display_colors(&self) -> (CellColor, CellColor) {
        let (fg, bg) = if self.inverse {
            (self.bg, self.fg)
        } else {
            (self.fg, self.bg)
        };
        if self.hidden { (bg, bg) } else { (fg, bg) }
    }
}

/// Where the cursor is, in viewport rows (row 0 is the top of what is visible).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorSnapshot {
    pub row: usize,
    pub col: usize,
}

/// Catches `Term`'s callbacks.
///
/// Interior mutability because `EventListener::send_event` takes `&self`.
/// Single-threaded, because an emulator belongs to whatever holds it and a
/// terminal that could be fed from two threads would interleave escape
/// sequences.
#[derive(Default, Clone)]
struct EventCapture {
    events: Rc<RefCell<Vec<Event>>>,
}

impl EventListener for EventCapture {
    fn send_event(&self, event: Event) {
        self.events.borrow_mut().push(event);
    }
}

/// A terminal's state: a pure fold of output bytes into a grid.
pub struct Emulator {
    term: Term<EventCapture>,
    parser: Processor,
    capture: EventCapture,
    title: Option<String>,
    bell: bool,
}

impl Emulator {
    pub fn new(cols: u16, rows: u16) -> Self {
        let capture = EventCapture::default();
        let config = Config {
            scrolling_history: SCROLLBACK_LINES,
            ..Config::default()
        };
        let term = Term::new(config, &GridSize::new(cols, rows), capture.clone());
        Self {
            term,
            parser: Processor::new(),
            capture,
            title: None,
            bell: false,
        }
    }

    /// Advance over output bytes, returning whatever the terminal wants
    /// written back.
    ///
    /// The return value is not decoration: a cursor position report or a device
    /// attributes query is a question the program is waiting on, and a host
    /// that drops the answer hangs it. Handing the bytes back rather than
    /// writing them keeps this module free of the pty.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.parser.advance(&mut self.term, bytes);
        let mut responses = Vec::new();
        for event in self.capture.events.borrow_mut().drain(..) {
            match event {
                Event::PtyWrite(text) => responses.extend_from_slice(text.as_bytes()),
                Event::Title(title) => self.title = Some(title),
                Event::ResetTitle => self.title = None,
                Event::Bell => self.bell = true,
                _ => {}
            }
        }
        responses
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.term.resize(GridSize::new(cols, rows));
    }

    pub fn cols(&self) -> usize {
        self.term.columns()
    }

    pub fn rows(&self) -> usize {
        self.term.screen_lines()
    }

    /// The title the running program set, if it set one. Text somebody else
    /// wrote, so a caller showing it owes it the same treatment as any other
    /// content it did not write.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Whether a BEL has arrived since this was last asked. Reading clears it,
    /// because a bell is an event and a caller that missed one has missed it.
    pub fn take_bell(&mut self) -> bool {
        std::mem::take(&mut self.bell)
    }

    /// Whether arrows should be sent as SS3 (`ESC O A`) rather than CSI.
    pub fn app_cursor_mode(&self) -> bool {
        self.term.mode().contains(TermMode::APP_CURSOR)
    }

    /// Whether a paste should be wrapped in `ESC [200~` / `ESC [201~`.
    pub fn bracketed_paste_mode(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// How far the view is scrolled back; 0 is pinned to the live bottom.
    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    /// How many lines are available above the viewport.
    pub fn history_lines(&self) -> usize {
        self.term.grid().history_size()
    }

    /// Scroll the view: positive goes up into history, negative toward live.
    pub fn scroll(&mut self, delta: i32) {
        self.term.scroll_display(Scroll::Delta(delta));
    }

    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }

    /// The grid point under a viewport cell.
    ///
    /// The two coordinate systems differ by the scrollback offset, and this is
    /// the only place they meet. The column clamps into the grid so a pointer
    /// past the right edge cannot anchor a selection outside it.
    pub fn grid_point(&self, viewport_row: usize, col: usize) -> GridPoint {
        GridPoint::new(
            viewport_row as i32 - self.display_offset() as i32,
            col.min(self.cols().saturating_sub(1)),
        )
    }

    /// Begin a selection at a point, with the granularity the gesture implies.
    pub fn start_selection(&mut self, kind: SelectionKind, point: GridPoint, side: CellSide) {
        self.term.selection = Some(Selection::new(
            kind.into_ansi(),
            point.into_ansi(),
            side.into_ansi(),
        ));
    }

    /// Extend the selection in progress. Does nothing without one, so a drag
    /// that never started cannot invent a selection out of a pointer move.
    pub fn update_selection(&mut self, point: GridPoint, side: CellSide) {
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point.into_ansi(), side.into_ansi());
        }
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    /// The selected text, or `None` when nothing is selected.
    ///
    /// A click with no drag leaves an empty selection behind, and reporting
    /// that as a selection is what makes a bare click clobber the clipboard.
    pub fn selection_text(&self) -> Option<String> {
        self.term
            .selection_to_string()
            .filter(|text| !text.is_empty())
    }

    /// Whether anything is actually selected.
    pub fn has_selection(&self) -> bool {
        self.selection_range().is_some()
    }

    fn selection_range(&self) -> Option<SelectionRange> {
        self.term
            .selection
            .as_ref()
            .and_then(|selection| selection.to_range(&self.term))
    }

    /// One viewport row, top row first.
    pub fn line(&self, viewport_row: usize) -> Vec<CellSnapshot> {
        self.line_inner(viewport_row, self.selection_range())
    }

    /// The body of [`Self::line`], taking the range as an argument so
    /// [`Self::lines`] resolves it once per frame rather than once per row:
    /// a word or line selection re-walks the grid to answer.
    fn line_inner(
        &self,
        viewport_row: usize,
        selection: Option<SelectionRange>,
    ) -> Vec<CellSnapshot> {
        let line = Line(viewport_row as i32 - self.display_offset() as i32);
        let row = &self.term.grid()[line];
        (0..self.cols())
            .map(|col| {
                let cell = &row[Column(col)];
                CellSnapshot {
                    ch: cell.c,
                    fg: map_color(cell.fg),
                    bg: map_color(cell.bg),
                    bold: cell.flags.intersects(Flags::BOLD),
                    dim: cell.flags.intersects(Flags::DIM),
                    italic: cell.flags.intersects(Flags::ITALIC),
                    underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
                    inverse: cell.flags.intersects(Flags::INVERSE),
                    hidden: cell.flags.intersects(Flags::HIDDEN),
                    wide: cell.flags.intersects(Flags::WIDE_CHAR),
                    wide_spacer: cell
                        .flags
                        .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER),
                    selected: selection
                        .is_some_and(|range| range.contains(Point::new(line, Column(col)))),
                }
            })
            .collect()
    }

    /// Every viewport row, top to bottom.
    pub fn lines(&self) -> Vec<Vec<CellSnapshot>> {
        let selection = self.selection_range();
        (0..self.rows())
            .map(|row| self.line_inner(row, selection))
            .collect()
    }

    /// The cursor in viewport coordinates, or `None` when it is hidden or has
    /// scrolled out of view.
    pub fn cursor(&self) -> Option<CursorSnapshot> {
        let content = self.term.renderable_content();
        if content.cursor.shape == CursorShape::Hidden {
            return None;
        }
        let Point { line, column } = content.cursor.point;
        let row = line.0 + self.display_offset() as i32;
        if row < 0 || row >= self.rows() as i32 {
            return None;
        }
        Some(CursorSnapshot {
            row: row as usize,
            col: column.0,
        })
    }

    /// One viewport row as trimmed text, with wide-character spacers skipped.
    ///
    /// A reading rather than a rendering: it is what a test asserts against and
    /// what a copy of a single row would contain.
    pub fn row_text(&self, viewport_row: usize) -> String {
        let mut text: String = self
            .line(viewport_row)
            .iter()
            .filter(|cell| !cell.wide_spacer)
            .map(|cell| cell.ch)
            .collect();
        while text.ends_with(' ') {
            text.pop();
        }
        text
    }
}

impl std::fmt::Debug for Emulator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Emulator")
            .field("cols", &self.cols())
            .field("rows", &self.rows())
            .field("display_offset", &self.display_offset())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emulator(cols: u16, rows: u16) -> Emulator {
        Emulator::new(cols, rows)
    }

    #[test]
    fn plain_text_lands_on_row_zero() {
        let mut term = emulator(20, 5);
        term.feed(b"hello");
        assert_eq!(term.row_text(0), "hello");
        assert_eq!(term.cursor(), Some(CursorSnapshot { row: 0, col: 5 }));
    }

    #[test]
    fn crlf_moves_lines_and_cr_returns_to_column_zero() {
        let mut term = emulator(20, 5);
        term.feed(b"one\r\ntwo\r\nthree");
        assert_eq!(term.row_text(0), "one");
        assert_eq!(term.row_text(1), "two");
        assert_eq!(term.row_text(2), "three");
        term.feed(b"\rXX");
        assert_eq!(term.row_text(2), "XXree");
    }

    #[test]
    fn a_long_line_wraps_at_the_grid_width() {
        let mut term = emulator(10, 4);
        term.feed(b"abcdefghijKLM");
        assert_eq!(term.row_text(0), "abcdefghij");
        assert_eq!(term.row_text(1), "KLM");
    }

    #[test]
    fn sgr_sets_colors_and_attributes() {
        let mut term = emulator(40, 4);
        term.feed(b"\x1b[31mred\x1b[0m plain \x1b[1;44mboldbg\x1b[0m");
        let line = term.line(0);
        assert_eq!(line[0].fg, CellColor::Indexed(1));
        assert_eq!(line[0].bg, CellColor::Background);
        assert_eq!(line[4].fg, CellColor::Foreground, "reset restores defaults");
        let bold = line[10];
        assert!(bold.bold);
        assert_eq!(bold.bg, CellColor::Indexed(4));
    }

    #[test]
    fn bright_indexed_and_direct_colors_stay_distinct() {
        let mut term = emulator(40, 2);
        term.feed(b"\x1b[95mA\x1b[38;5;196mB\x1b[38;2;10;20;30mC");
        let line = term.line(0);
        assert_eq!(line[0].fg, CellColor::Indexed(13));
        assert_eq!(line[1].fg, CellColor::Indexed(196));
        assert_eq!(line[2].fg, CellColor::Rgb(10, 20, 30));
    }

    #[test]
    fn inverse_and_hidden_resolve_in_display_colors() {
        let mut term = emulator(10, 2);
        term.feed(b"\x1b[7mI\x1b[0m\x1b[8mH");
        let inverse = term.line(0)[0];
        assert!(inverse.inverse);
        assert_eq!(
            inverse.display_colors(),
            (CellColor::Background, CellColor::Foreground)
        );
        let hidden = term.line(0)[1];
        assert!(hidden.hidden);
        let (fg, bg) = hidden.display_colors();
        assert_eq!(fg, bg, "hidden text paints as its own background");
    }

    #[test]
    fn cursor_addressing_and_relative_moves() {
        let mut term = emulator(20, 6);
        term.feed(b"\x1b[3;5Hx");
        assert_eq!(term.line(2)[4].ch, 'x');
        assert_eq!(term.cursor(), Some(CursorSnapshot { row: 2, col: 5 }));
        term.feed(b"\x1b[2D");
        assert_eq!(term.cursor(), Some(CursorSnapshot { row: 2, col: 3 }));
        term.feed(b"\x1b[A");
        assert_eq!(term.cursor(), Some(CursorSnapshot { row: 1, col: 3 }));
    }

    #[test]
    fn clear_screen_and_home() {
        let mut term = emulator(20, 4);
        term.feed(b"aaa\r\nbbb\r\nccc");
        term.feed(b"\x1b[2J\x1b[H");
        for row in 0..4 {
            assert_eq!(term.row_text(row), "");
        }
        assert_eq!(term.cursor(), Some(CursorSnapshot { row: 0, col: 0 }));
        term.feed(b"fresh");
        assert_eq!(term.row_text(0), "fresh");
    }

    #[test]
    fn erase_to_the_end_of_the_line() {
        let mut term = emulator(20, 2);
        term.feed(b"abcdef\x1b[3D\x1b[K");
        assert_eq!(term.row_text(0), "abc");
    }

    #[test]
    fn scrollback_holds_what_left_the_viewport() {
        let mut term = emulator(10, 3);
        for index in 1..=8 {
            term.feed(format!("line{index}\r\n").as_bytes());
        }
        assert_eq!(term.row_text(0), "line7");
        assert_eq!(term.history_lines(), 6);
        assert_eq!(term.display_offset(), 0);

        term.scroll(2);
        assert_eq!(term.display_offset(), 2);
        assert_eq!(term.row_text(0), "line5");
        assert_eq!(term.cursor(), None, "the cursor is below the viewport");

        term.scroll(100);
        assert_eq!(term.display_offset(), 6, "over-scroll clamps to the top");
        assert_eq!(term.row_text(0), "line1");

        term.scroll_to_bottom();
        assert_eq!(term.display_offset(), 0);
        assert_eq!(term.row_text(0), "line7");
    }

    #[test]
    fn the_alternate_screen_gives_the_primary_one_back() {
        let mut term = emulator(20, 4);
        term.feed(b"primary");
        term.feed(b"\x1b[?1049h\x1b[H");
        term.feed(b"alt-content");
        assert_eq!(term.row_text(0), "alt-content");
        term.feed(b"\x1b[?1049l");
        assert_eq!(term.row_text(0), "primary");
    }

    #[test]
    fn a_cursor_report_comes_back_as_bytes_for_the_host_to_write() {
        let mut term = emulator(20, 4);
        term.feed(b"\x1b[2;3H");
        let responses = term.feed(b"\x1b[6n");
        assert_eq!(String::from_utf8_lossy(&responses), "\x1b[2;3R");
    }

    #[test]
    fn the_title_and_the_bell_are_reported_once() {
        let mut term = emulator(20, 2);
        assert_eq!(term.title(), None);
        term.feed(b"\x1b]0;my title\x07");
        assert_eq!(term.title(), Some("my title"));
        assert!(!term.take_bell());
        term.feed(b"\x07");
        assert!(term.take_bell());
        assert!(!term.take_bell(), "reading a bell clears it");
    }

    #[test]
    fn the_modes_a_caller_has_to_encode_against_are_readable() {
        let mut term = emulator(10, 2);
        assert!(!term.app_cursor_mode());
        term.feed(b"\x1b[?1h");
        assert!(term.app_cursor_mode());
        term.feed(b"\x1b[?1l");
        assert!(!term.app_cursor_mode());
        term.feed(b"\x1b[?2004h");
        assert!(term.bracketed_paste_mode());
    }

    #[test]
    fn a_hidden_cursor_is_reported_as_no_cursor() {
        let mut term = emulator(10, 2);
        term.feed(b"\x1b[?25l");
        assert_eq!(term.cursor(), None);
        term.feed(b"\x1b[?25h");
        assert!(term.cursor().is_some());
    }

    #[test]
    fn resizing_keeps_the_content() {
        let mut term = emulator(20, 5);
        term.feed(b"keepme\r\nsecond");
        term.resize(30, 3);
        assert_eq!(term.cols(), 30);
        assert_eq!(term.rows(), 3);
        assert_eq!(term.row_text(0), "keepme");
        assert_eq!(term.row_text(1), "second");
    }

    #[test]
    fn a_collapsed_grid_still_has_cells_in_it() {
        let size = GridSize::new(0, 0);
        assert_eq!(size.cols, 2);
        assert_eq!(size.rows, 1);
    }

    #[test]
    fn a_wide_character_takes_two_cells_and_a_spacer() {
        let mut term = emulator(10, 2);
        term.feed("宽w".as_bytes());
        let line = term.line(0);
        assert!(line[0].wide);
        assert_eq!(line[0].ch, '宽');
        assert!(line[1].wide_spacer);
        assert_eq!(line[2].ch, 'w');
        assert_eq!(term.row_text(0), "宽w");
        assert_eq!(term.cursor(), Some(CursorSnapshot { row: 0, col: 3 }));
    }

    #[test]
    fn a_grid_point_offsets_by_the_scrollback_position() {
        let mut term = emulator(10, 3);
        for index in 1..=8 {
            term.feed(format!("line{index}\r\n").as_bytes());
        }
        assert_eq!(term.grid_point(0, 2), GridPoint::new(0, 2));
        term.scroll(4);
        assert_eq!(term.grid_point(0, 2), GridPoint::new(-4, 2));
        assert_eq!(
            term.grid_point(0, 99).column,
            9,
            "a pointer past the edge anchors inside the grid"
        );
    }

    #[test]
    fn a_drag_selects_its_text_and_marks_its_cells() {
        let mut term = emulator(20, 3);
        term.feed(b"hello world");
        assert!(!term.has_selection());
        assert_eq!(term.selection_text(), None);

        term.start_selection(SelectionKind::Drag, term.grid_point(0, 0), CellSide::Left);
        term.update_selection(term.grid_point(0, 4), CellSide::Right);
        assert!(term.has_selection());
        assert_eq!(term.selection_text().as_deref(), Some("hello"));

        let line = term.line(0);
        assert!(line[..5].iter().all(|cell| cell.selected));
        assert!(!line[5].selected, "the space past the drag is not selected");

        term.clear_selection();
        assert!(!term.has_selection());
        assert!(term.line(0).iter().all(|cell| !cell.selected));
    }

    #[test]
    fn a_word_selection_expands_without_the_caller_finding_the_boundary() {
        let mut term = emulator(30, 2);
        term.feed(b"alpha beta gamma");
        term.start_selection(SelectionKind::Word, term.grid_point(0, 7), CellSide::Left);
        assert_eq!(term.selection_text().as_deref(), Some("beta"));
    }

    #[test]
    fn a_line_selection_takes_the_break_with_it() {
        let mut term = emulator(30, 3);
        term.feed(b"first row\r\nsecond row");
        term.start_selection(SelectionKind::Line, term.grid_point(1, 3), CellSide::Left);
        assert_eq!(term.selection_text().as_deref(), Some("second row\n"));
    }

    #[test]
    fn a_selection_across_rows_keeps_the_newline() {
        let mut term = emulator(10, 3);
        term.feed(b"ab\r\ncd");
        term.start_selection(SelectionKind::Drag, term.grid_point(0, 0), CellSide::Left);
        term.update_selection(term.grid_point(1, 1), CellSide::Right);
        assert_eq!(term.selection_text().as_deref(), Some("ab\ncd"));
    }

    #[test]
    fn a_selection_follows_its_text_when_output_scrolls() {
        let mut term = emulator(10, 3);
        term.feed(b"target\r\n");
        term.start_selection(SelectionKind::Drag, term.grid_point(0, 0), CellSide::Left);
        term.update_selection(term.grid_point(0, 5), CellSide::Right);
        assert_eq!(term.selection_text().as_deref(), Some("target"));
        term.feed(b"a\r\nb\r\nc\r\n");
        assert_eq!(
            term.selection_text().as_deref(),
            Some("target"),
            "the anchors are in grid space, so the text did not move"
        );
    }

    #[test]
    fn a_click_without_a_drag_selects_nothing() {
        let mut term = emulator(20, 2);
        term.feed(b"hello");
        term.start_selection(SelectionKind::Drag, term.grid_point(0, 2), CellSide::Left);
        assert_eq!(term.selection_text(), None);
        assert!(!term.has_selection());
    }

    #[test]
    fn a_character_split_across_two_feeds_reassembles() {
        let mut term = emulator(10, 2);
        let bytes = "é".as_bytes();
        term.feed(&bytes[..1]);
        term.feed(&bytes[1..]);
        assert_eq!(term.row_text(0), "é");
    }
}
