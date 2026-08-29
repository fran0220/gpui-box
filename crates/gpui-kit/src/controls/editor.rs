//! A source-oriented facade over [`TextArea`](super::textarea::TextArea).
//!
//! `Editor` deliberately does not own another document, caret, history, IME
//! session, or text layout. It fixes the shared area to no-wrap source
//! geometry, adds a measured line-number gutter, and projects caller-owned
//! revision-tagged highlights and indentation decisions onto that one editing
//! surface. Parsing, language servers, persistence, collaboration, folding,
//! and minimaps remain downstream concerns.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Bounds, Context, Entity, EventEmitter, Focusable, HighlightStyle,
    InteractiveElement, IntoElement, ParentElement, Pixels, Render, SharedString, Styled,
    Subscription, Window, div, point, prelude::FluentBuilder as _, px, size,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space};

use crate::foundation::{Disableable, Ident, StyledExt};
use crate::strings::ActiveNumbers;

use super::textarea::{
    Frame, Pasted, TextArea, TextAreaEdit, TextAreaEvent, TextAreaSnapshot, TextAreaWrap,
};

const DEFAULT_ROWS: usize = 12;

type Indenter = Rc<dyn Fn(EditorIndentRequest) -> Option<EditorIndentation>>;

/// One caller-owned style range in an editor revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorHighlight {
    range: Range<usize>,
    style: HighlightStyle,
}

impl EditorHighlight {
    pub fn new(range: Range<usize>, style: HighlightStyle) -> Self {
        Self { range, style }
    }

    pub fn range(&self) -> &Range<usize> {
        &self.range
    }

    pub fn style(&self) -> HighlightStyle {
        self.style
    }
}

/// Syntax and diagnostic styles tagged with the text revision they describe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorHighlights {
    revision: u64,
    spans: Vec<EditorHighlight>,
}

impl EditorHighlights {
    pub fn new(revision: u64, spans: impl IntoIterator<Item = EditorHighlight>) -> Self {
        Self {
            revision,
            spans: spans.into_iter().collect(),
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn spans(&self) -> &[EditorHighlight] {
        &self.spans
    }
}

/// Which indentation chord the source editor received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorIndentDirection {
    Indent,
    Outdent,
}

/// The immutable input to a caller-owned indentation policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorIndentRequest {
    pub snapshot: TextAreaSnapshot,
    pub selection: Range<usize>,
    pub direction: EditorIndentDirection,
}

/// One synchronous replacement returned by an indentation policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorIndentation {
    pub range: Range<usize>,
    pub text: SharedString,
    /// The selection to apply after replacement, in the resulting revision.
    pub selection: Option<Range<usize>>,
}

impl EditorIndentation {
    pub fn new(range: Range<usize>, text: impl Into<SharedString>) -> Self {
        Self {
            range,
            text: text.into(),
            selection: None,
        }
    }

    pub fn selection(mut self, selection: Range<usize>) -> Self {
        self.selection = Some(selection);
        self
    }
}

/// Current-frame geometry for one hard source line.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorLineGeometry {
    /// One-based line number.
    pub line: usize,
    /// UTF-8 source range, including its trailing line break when present.
    pub range: Range<usize>,
    /// Painted row bounds in window coordinates.
    pub bounds: Bounds<Pixels>,
}

/// The measured source viewport, tagged with the revision it describes.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorGeometry {
    /// The text revision whose hard lines were measured.
    pub revision: u64,
    /// The painted text viewport in window coordinates.
    pub viewport: Bounds<Pixels>,
    /// The no-wrap source row's offset from its logical left edge.
    pub horizontal_scroll: Pixels,
    /// The hard-line stack's offset from its logical top edge.
    pub vertical_scroll: Pixels,
    /// Every hard source line, in source order.
    pub lines: Vec<EditorLineGeometry>,
}

/// What a source editor reports to its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorEvent {
    /// One replacement entered the shared undo history.
    Edited(TextAreaEdit),
    /// The complete plain-text value changed.
    Changed(SharedString),
    /// The byte selection changed on grapheme boundaries.
    SelectionChanged(Range<usize>),
    /// The platform clipboard supplied non-text input.
    Pasted(Pasted),
    /// The configured submit chord was pressed.
    Submitted,
    /// Escape or the configured cancellation chord was pressed.
    Cancelled,
    /// The shared editing surface gained focus.
    Focused,
    /// The shared editing surface lost focus.
    Blurred,
}

impl EventEmitter<EditorEvent> for Editor {}

/// Plain source editing with caller-owned language policy.
pub struct Editor {
    ident: Ident,
    label: SharedString,
    area: Entity<TextArea>,
    rows: usize,
    line_numbers: bool,
    disabled: bool,
    read_only: bool,
    highlights: Option<EditorHighlights>,
    indenter: Option<Indenter>,
    _subscription: Subscription,
}

impl std::fmt::Debug for Editor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Editor")
            .field("ident", &self.ident)
            .field("rows", &self.rows)
            .field("line_numbers", &self.line_numbers)
            .field("disabled", &self.disabled)
            .field("read_only", &self.read_only)
            .finish()
    }
}

impl Editor {
    pub fn new(
        ident: impl Into<Ident>,
        label: impl Into<SharedString>,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ident = ident.into();
        let area_ident = ident.child("input");
        let text = text.into();
        let area = cx.new(|cx| {
            TextArea::new(area_ident, window, cx)
                .frame(Frame::Host)
                .wrap(TextAreaWrap::None)
                .rows(DEFAULT_ROWS)
                .max_rows(DEFAULT_ROWS)
                .text(text)
        });
        let subscription = cx.subscribe(&area, |editor, area, event, cx| {
            editor.on_area_event(&area, event, cx)
        });
        Self {
            ident,
            label: label.into(),
            area,
            rows: DEFAULT_ROWS,
            line_numbers: true,
            disabled: false,
            read_only: false,
            highlights: None,
            indenter: None,
            _subscription: subscription,
        }
    }

    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    pub fn line_numbers(mut self, visible: bool) -> Self {
        self.line_numbers = visible;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Installs a synchronous, caller-owned indentation policy.
    ///
    /// Without one, tab is not claimed by the editor. The policy receives an
    /// immutable revision and returns at most one replacement; it never gets
    /// access to the editor's internal buffer or history.
    pub fn indent_with(
        mut self,
        indenter: impl Fn(EditorIndentRequest) -> Option<EditorIndentation> + 'static,
    ) -> Self {
        self.indenter = Some(Rc::new(indenter));
        self
    }

    pub fn highlights(mut self, highlights: EditorHighlights) -> Self {
        self.highlights = Some(highlights);
        self
    }

    pub fn text_area(&self) -> &Entity<TextArea> {
        &self.area
    }

    pub fn snapshot(&self, cx: &App) -> TextAreaSnapshot {
        self.area.read(cx).snapshot()
    }

    pub fn selected_range(&self, cx: &App) -> Range<usize> {
        self.area.read(cx).selected_range()
    }

    pub fn geometry(&self, cx: &App) -> Option<EditorGeometry> {
        let geometry = self.area.read(cx).source_geometry()?;
        let origin = point(
            geometry.viewport.left(),
            geometry.viewport.top() - geometry.vertical_scroll,
        );
        let lines = geometry
            .rows
            .into_iter()
            .enumerate()
            .map(|(index, range)| EditorLineGeometry {
                line: index + 1,
                range,
                bounds: Bounds::new(
                    point(origin.x, origin.y + geometry.line_height * index as f32),
                    size(geometry.viewport.size.width, geometry.line_height),
                ),
            })
            .collect();
        Some(EditorGeometry {
            revision: geometry.revision,
            viewport: geometry.viewport,
            horizontal_scroll: geometry.horizontal_scroll,
            vertical_scroll: geometry.vertical_scroll,
            lines,
        })
    }

    pub fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        let area = self.area.clone();
        let value = value.into();
        area.update(cx, |area, cx| area.set_value(value, cx));
    }

    pub fn set_highlights(&mut self, highlights: EditorHighlights, cx: &mut Context<Self>) -> bool {
        let valid = valid_highlights(&self.snapshot(cx), &highlights);
        self.highlights = Some(highlights);
        cx.notify();
        valid
    }

    fn on_area_event(
        &mut self,
        area: &Entity<TextArea>,
        event: &TextAreaEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            TextAreaEvent::Edited(edit) => cx.emit(EditorEvent::Edited(edit.clone())),
            TextAreaEvent::Change(value) => cx.emit(EditorEvent::Changed(value.clone())),
            TextAreaEvent::SelectionChanged(selection) => {
                cx.emit(EditorEvent::SelectionChanged(selection.clone()));
                cx.notify();
            }
            TextAreaEvent::Pasted(pasted) => cx.emit(EditorEvent::Pasted(pasted.clone())),
            TextAreaEvent::Submit => cx.emit(EditorEvent::Submitted),
            TextAreaEvent::Cancel => cx.emit(EditorEvent::Cancelled),
            TextAreaEvent::Focus => cx.emit(EditorEvent::Focused),
            TextAreaEvent::Blur => cx.emit(EditorEvent::Blurred),
            TextAreaEvent::IndentRequested => {
                self.apply_indentation(area, EditorIndentDirection::Indent, cx)
            }
            TextAreaEvent::OutdentRequested => {
                self.apply_indentation(area, EditorIndentDirection::Outdent, cx)
            }
            TextAreaEvent::GeometryChanged => cx.notify(),
            TextAreaEvent::MoveUp
            | TextAreaEvent::MoveDown
            | TextAreaEvent::AcceptCompletion
            | TextAreaEvent::DismissCompletion => {}
        }
    }

    fn apply_indentation(
        &mut self,
        area: &Entity<TextArea>,
        direction: EditorIndentDirection,
        cx: &mut Context<Self>,
    ) {
        let Some(indenter) = self.indenter.clone() else {
            return;
        };
        let request = {
            let area = area.read(cx);
            EditorIndentRequest {
                snapshot: area.snapshot(),
                selection: area.selected_range(),
                direction,
            }
        };
        let Some(indentation) = indenter(request) else {
            return;
        };
        area.update(cx, |area, cx| {
            if area
                .replace_range(indentation.range, &indentation.text, cx)
                .is_some()
                && let Some(selection) = indentation.selection
            {
                area.set_selected_range(selection, cx);
            }
        });
    }
}

impl Disableable for Editor {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let spans = self
            .highlights
            .as_ref()
            .map(|highlights| {
                (
                    highlights.revision,
                    highlights
                        .spans
                        .iter()
                        .map(|span| (span.range.clone(), span.style))
                        .collect::<Vec<_>>(),
                )
            })
            .unwrap_or_default();
        self.area.update(cx, |area, cx| {
            area.set_row_limits(self.rows, self.rows);
            area.set_indentation_claimed(self.indenter.is_some());
            area.set_highlights(spans.0, spans.1);
            if area.is_disabled() != self.disabled {
                area.set_disabled(self.disabled, cx);
            }
            if area.is_read_only() != self.read_only {
                area.set_read_only(self.read_only, cx);
            }
        });

        let theme = cx.theme().clone();
        let metrics = theme.control.get(ControlSize::Md);
        let focus = self.area.read(cx).focus_handle(cx);
        let focused = focus.is_focused(window);
        let snapshot = self.area.read(cx).snapshot();
        let selection = self.area.read(cx).selected_range();
        let active_line = hard_line_at(&snapshot.text, selection.end);
        let line_count = snapshot.text.matches('\n').count() + 1;
        let digits = line_count.to_string().len().max(2);
        let gutter_width = px(metrics.font_size * digits as f32 * 0.65
            + theme.spacing.sm
            + theme.spacing.xs * 2.0);
        let line_height = px(theme
            .type_style(gpui_kit_theme::TypeScale::Code)
            .line_height);
        let scroll = self
            .area
            .read(cx)
            .source_geometry()
            .map(|geometry| geometry.vertical_scroll)
            .unwrap_or(px(0.0));
        let numbers = self.line_numbers.then(|| {
            div()
                .relative()
                .flex_none()
                .w(gutter_width)
                .h(line_height * self.rows as f32)
                .overflow_hidden()
                .border_r_1()
                .border_color(theme.colors.divider)
                .child(
                    div()
                        .absolute()
                        .top(-scroll)
                        .left(px(0.0))
                        .w_full()
                        .children((0..line_count).map(|index| {
                            div()
                                .h(line_height)
                                .pr(px(theme.spacing.xs))
                                .flex()
                                .items_center()
                                .justify_end()
                                .text_color(if index == active_line {
                                    theme.colors.accent
                                } else {
                                    theme.colors.text_faint
                                })
                                .child(cx.numbers().count(index + 1))
                        })),
                )
        });

        div()
            .id(self.ident.element_id())
            .w_full()
            .min_w_0()
            .flex()
            .items_start()
            .py_token(&theme, Space::Xs)
            .radius(&theme, Radius::Control)
            .well(&theme)
            .border_color(if self.disabled {
                gpui::transparent_black()
            } else {
                theme.colors.hairline
            })
            .when(focused, |element| element.shadow(theme.focus_ring()))
            .track_focus(&focus)
            .font_family(theme.typography.mono.clone())
            .type_scale(&theme, gpui_kit_theme::TypeScale::Code)
            .children(numbers)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_token(&theme, Space::Sm)
                    .child(self.area.clone()),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group).text(self.label.clone()),
            )
    }
}

fn hard_line_at(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
}

fn valid_highlights(snapshot: &TextAreaSnapshot, highlights: &EditorHighlights) -> bool {
    if snapshot.revision != highlights.revision {
        return false;
    }
    let mut spans = highlights.spans.iter().collect::<Vec<_>>();
    spans.sort_by_key(|span| (span.range.start, span.range.end));
    let mut end = 0;
    spans.into_iter().all(|span| {
        let valid = span.range.start >= end
            && span.range.start <= span.range.end
            && snapshot.text.get(span.range.clone()).is_some();
        end = span.range.end;
        valid
    })
}
