//! A source-oriented facade over [`TextArea`].
//!
//! `Editor` deliberately does not own another document, caret, history, IME
//! session, or text layout. It fixes the shared area to no-wrap source
//! geometry, adds a measured line-number gutter, and projects caller-owned
//! revision-tagged highlights and indentation decisions onto that one editing
//! surface. Optional Tree-sitter parsing runs in-process; language servers,
//! persistence, collaboration and workspace policy remain downstream concerns.

#[cfg(feature = "syntax")]
mod syntax;
#[cfg(feature = "syntax")]
pub use syntax::{EditorParseWork, EditorSyntax, EditorSyntaxCapture, EditorSyntaxError};

mod services;
pub use services::*;
mod folding;
pub use folding::EditorFold;

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Bounds, Context, Entity, EventEmitter, Focusable, HighlightStyle,
    InteractiveElement, IntoElement, ParentElement, Pixels, Point, Render, SharedString, Styled,
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

gpui::actions!(
    editor,
    [
        RequestCompletion,
        RequestHover,
        RequestDefinition,
        RequestCodeActions,
        ToggleFold
    ]
);

pub(crate) fn install(cx: &mut App) {
    cx.bind_keys([
        gpui::KeyBinding::new("ctrl-space", RequestCompletion, Some("Editor")),
        gpui::KeyBinding::new("f12", RequestDefinition, Some("Editor")),
        gpui::KeyBinding::new("ctrl-k ctrl-i", RequestHover, Some("Editor")),
        gpui::KeyBinding::new("ctrl-.", RequestCodeActions, Some("Editor")),
        gpui::KeyBinding::new("ctrl-alt-f", ToggleFold, Some("Editor")),
    ]);
}

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
    /// The host should answer this revision-tagged language request.
    ServiceRequested(EditorServiceRequest),
    /// A stable service item was accepted successfully.
    ServiceAccepted(SharedString),
    /// Navigation belongs to the caller's document/workspace host.
    DefinitionRequested {
        target: SharedString,
        range: Range<usize>,
    },
    /// A caller-defined code action requires host execution.
    CodeActionRequested(SharedString),
    /// One replacement entered the shared undo history.
    Edited(TextAreaEdit),
    /// The value changed. The persistent snapshot does not materialize a
    /// contiguous string unless the recipient explicitly calls `text()`.
    Changed(gpui::EditSnapshot),
    /// The byte selection changed on grapheme boundaries.
    SelectionChanged(Range<usize>),
    /// A caller-identified fold changed its transient collapsed state.
    FoldChanged { id: SharedString, collapsed: bool },
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
    /// A revision was parsed in-process, including any grammar errors.
    #[cfg(feature = "syntax")]
    Parsed {
        revision: u64,
        errors: Vec<EditorSyntaxError>,
    },
    /// The configured highlight query refused to publish a partial result.
    #[cfg(feature = "syntax")]
    SyntaxUnavailable(SharedString),
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
    services_enabled: bool,
    next_service_request: u64,
    service_popup: Option<services::ServicePopup>,
    hover_position: Option<(u64, usize)>,
    diagnostics: Option<(u64, Vec<EditorDiagnostic>)>,
    semantic_tokens: Option<(u64, Vec<EditorSemanticToken>)>,
    folds: Vec<EditorFold>,
    collapsed_folds: std::collections::HashSet<SharedString>,
    #[cfg(feature = "syntax")]
    syntax: Option<EditorSyntax>,
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
            services_enabled: false,
            next_service_request: 0,
            service_popup: None,
            hover_position: None,
            diagnostics: None,
            semantic_tokens: None,
            folds: Vec::new(),
            collapsed_folds: std::collections::HashSet::new(),
            #[cfg(feature = "syntax")]
            syntax: None,
            _subscription: subscription,
        }
    }

    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    pub fn set_label(&mut self, label: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.label = label.into();
        cx.notify();
    }

    pub fn set_rows(&mut self, rows: usize, cx: &mut Context<Self>) {
        self.rows = rows.max(1);
        self.area.update(cx, |area, cx| {
            area.set_rows(self.rows, cx);
            area.set_max_rows(Some(self.rows), cx);
        });
        cx.notify();
    }

    pub fn set_line_numbers(&mut self, visible: bool, cx: &mut Context<Self>) {
        self.line_numbers = visible;
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        self.area
            .update(cx, |area, cx| area.set_read_only(read_only, cx));
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.service_popup = None;
            self.hover_position = None;
        }
        self.area
            .update(cx, |area, cx| area.set_disabled(disabled, cx));
        cx.notify();
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Replaces the caller indentation policy in place. `None` restores literal
    /// Tab input. The closure is never called merely to update this option.
    pub fn set_indent_with(
        &mut self,
        indenter: Option<Rc<dyn Fn(EditorIndentRequest) -> Option<EditorIndentation>>>,
        cx: &mut Context<Self>,
    ) {
        self.indenter = indenter;
        self.area.update(cx, |area, _| {
            area.set_indentation_claimed(self.indenter.is_some())
        });
        cx.notify();
    }

    /// Installs/replaces a parser or removes it without replacing document state.
    #[cfg(feature = "syntax")]
    pub fn set_syntax(&mut self, syntax: Option<EditorSyntax>, cx: &mut Context<Self>) {
        self.syntax = syntax;
        cx.notify();
    }

    /// Changes service bindings in place. Disabling cancels the visible request;
    /// late replies cannot restore its popup. Caller diagnostics remain intact.
    pub fn set_language_services(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.services_enabled = enabled;
        if !enabled {
            self.service_popup = None;
            self.hover_position = None;
            self.area.update(cx, |area, _| {
                area.set_completion_claimed(false);
                area.set_arrows_claimed(false);
            });
        }
        cx.notify();
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

    /// Installs actual incremental in-process syntax parsing. Explicit
    /// caller highlights for the current revision take precedence over it.
    #[cfg(feature = "syntax")]
    pub fn syntax(mut self, syntax: EditorSyntax) -> Self {
        self.syntax = Some(syntax);
        self
    }

    /// The parser's current revision, captures, grammar errors and work counts.
    #[cfg(feature = "syntax")]
    pub fn syntax_state(&self) -> Option<&EditorSyntax> {
        self.syntax.as_ref()
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

    /// Primary-first selections owned by the shared input/history surface.
    pub fn selections(&self, cx: &App) -> Vec<(Range<usize>, bool)> {
        self.area.read(cx).selections()
    }

    pub fn set_selections(
        &mut self,
        selections: impl IntoIterator<Item = (Range<usize>, bool)>,
        cx: &mut Context<Self>,
    ) -> bool {
        self.area
            .update(cx, |area, cx| area.set_selections(selections, cx))
    }

    pub fn select_rectangle(
        &mut self,
        anchor: Point<Pixels>,
        focus: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        self.area
            .update(cx, |area, cx| area.select_rectangle(anchor, focus, cx))
    }

    /// Applies caller-supplied changes only to the revision they describe.
    /// Overlapping or invalid edits are refused before any text/history change.
    pub fn apply_edits(
        &mut self,
        revision: u64,
        edits: impl IntoIterator<Item = (Range<usize>, SharedString)>,
        cx: &mut Context<Self>,
    ) -> bool {
        if revision != self.area.read(cx).revision() {
            return false;
        }
        self.area
            .update(cx, |area, cx| area.replace_ranges(edits, cx))
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
                line: self.area.read(cx).document().line_at(range.start) + 1,
                range,
                bounds: geometry.visual_transform.map_bounds(Bounds::new(
                    point(origin.x, origin.y + geometry.line_height * index as f32),
                    size(geometry.viewport.size.width, geometry.line_height),
                )),
            })
            .collect();
        Some(EditorGeometry {
            revision: geometry.revision,
            viewport: geometry.visual_transform.map_bounds(geometry.viewport),
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
            TextAreaEvent::Edited(edit) => {
                self.folds.clear();
                self.collapsed_folds.clear();
                let refresh = self
                    .service_popup
                    .as_ref()
                    .is_some_and(|popup| popup.request.kind == EditorServiceKind::Completion);
                self.service_popup = None;
                self.hover_position = None;
                cx.emit(EditorEvent::Edited(edit.clone()));
                #[cfg(feature = "syntax")]
                self.update_syntax(Some(edit), cx);
                if refresh {
                    self.request_service(
                        EditorServiceKind::Completion,
                        area.read(cx).cursor_offset(),
                        cx,
                    );
                }
            }
            TextAreaEvent::Change(value) => cx.emit(EditorEvent::Changed(value.clone())),
            TextAreaEvent::SelectionChanged(selection) => {
                self.expand_caret_fold(cx);
                if self.service_popup.as_ref().is_some_and(|popup| {
                    popup.request.position != area.read(cx).cursor_offset()
                        || popup.request.selection != *selection
                        || popup.request.revision != area.read(cx).revision()
                }) {
                    self.service_popup = None;
                }
                cx.emit(EditorEvent::SelectionChanged(selection.clone()));
                cx.notify();
            }
            TextAreaEvent::Pasted(pasted) => cx.emit(EditorEvent::Pasted(pasted.clone())),
            TextAreaEvent::Submit => cx.emit(EditorEvent::Submitted),
            TextAreaEvent::Cancel => cx.emit(EditorEvent::Cancelled),
            TextAreaEvent::Focus => cx.emit(EditorEvent::Focused),
            TextAreaEvent::Blur => {
                self.service_popup = None;
                cx.emit(EditorEvent::Blurred);
                cx.notify();
            }
            TextAreaEvent::IndentRequested => {
                self.apply_indentation(area, EditorIndentDirection::Indent, cx)
            }
            TextAreaEvent::OutdentRequested => {
                self.apply_indentation(area, EditorIndentDirection::Outdent, cx)
            }
            TextAreaEvent::GeometryChanged => cx.notify(),
            TextAreaEvent::MoveUp => self.move_service_selection(-1, cx),
            TextAreaEvent::MoveDown => self.move_service_selection(1, cx),
            TextAreaEvent::AcceptCompletion => self.accept_selected_service(cx),
            TextAreaEvent::DismissCompletion => self.dismiss_service(cx),
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

    #[cfg(feature = "syntax")]
    fn update_syntax(&mut self, edit: Option<&TextAreaEdit>, cx: &mut Context<Self>) {
        let Some(syntax) = self.syntax.as_mut() else {
            return;
        };
        let area = self.area.read(cx);
        let revision = area.revision();
        if syntax.update(revision, area.document(), edit) {
            cx.emit(EditorEvent::Parsed {
                revision,
                errors: syntax.errors(),
            });
            cx.notify();
        }
    }

    #[cfg(feature = "syntax")]
    fn syntax_spans(
        &mut self,
        fallback: (u64, Vec<(Range<usize>, HighlightStyle)>),
        cx: &mut Context<Self>,
    ) -> (u64, Vec<(Range<usize>, HighlightStyle)>) {
        self.update_syntax(None, cx);
        let Some(syntax) = self.syntax.as_ref() else {
            return fallback;
        };
        let area = self.area.read(cx);
        let revision = area.revision();
        if self
            .highlights
            .as_ref()
            .is_some_and(|highlights| highlights.revision == revision)
        {
            return fallback;
        }
        let theme = cx.theme();
        let line_height = px(theme
            .type_style(gpui_kit_theme::TypeScale::Code)
            .line_height);
        let ranges = area.visible_source_ranges(line_height, line_height * self.rows as f32);
        let spans = ranges.into_iter().try_fold(Vec::new(), |mut spans, range| {
            spans.extend(syntax.highlights(range, theme)?);
            Ok::<_, SharedString>(spans)
        });
        match spans {
            Ok(spans) => (
                revision,
                spans
                    .into_iter()
                    .map(|span| (span.range, span.style))
                    .collect(),
            ),
            Err(reason) => {
                cx.emit(EditorEvent::SyntaxUnavailable(reason));
                fallback
            }
        }
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
        #[cfg(feature = "syntax")]
        let spans = self.syntax_spans(spans, cx);
        let spans = self.diagnostic_spans(spans, cx);
        let service_open = self
            .service_popup
            .as_ref()
            .is_some_and(|popup| popup.request.kind != EditorServiceKind::Hover);
        self.area.update(cx, |area, cx| {
            area.set_completion_claimed(service_open);
            area.set_arrows_claimed(service_open);
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
        let document = self.area.read(cx).document();
        let active_line = document.line_at(self.area.read(cx).cursor_offset());
        let line_count = document.line_count();
        let digits = line_count.to_string().len().max(2);
        let gutter_width = px(metrics.font_size * digits as f32 * 0.65
            + theme.spacing.sm
            + theme.spacing.xs * 2.0);
        let line_height = px(theme
            .type_style(gpui_kit_theme::TypeScale::Code)
            .line_height);
        let scroll = self.area.read(cx).scroll_offset();
        let projection = self.area.read(cx).source_projection();
        let first_line = ((scroll / line_height).floor() as usize).min(projection.rows() - 1);
        let last_line = (first_line + self.rows + 1).min(projection.rows());
        let numbers = self.line_numbers.then(|| {
            div()
                .relative()
                .flex_none()
                .w(gutter_width)
                .h(line_height * self.rows as f32)
                .overflow_hidden()
                .bg(theme.colors.sunken.opacity(theme.effects.area_wash_alpha))
                .child(
                    div()
                        .absolute()
                        .top(line_height * first_line as f32 - scroll)
                        .left(px(0.0))
                        .w_full()
                        .children((first_line..last_line).map(|row| {
                            let index = projection.source_line(row);
                            let toggle = self.fold_toggle(index, cx);
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
                                .children(toggle)
                                .child(cx.numbers().count(index + 1))
                        })),
                )
        });

        let popup = self.service_surface(cx);
        div()
            .id(self.ident.element_id())
            .key_context("Editor")
            .w_full()
            .min_w_0()
            .flex()
            .items_start()
            .py_token(&theme, Space::Xs)
            .radius(&theme, Radius::Control)
            .well(&theme)
            .when(focused, |element| element.shadow(theme.focus_ring()))
            .track_focus(&focus)
            .when(!self.disabled, |element| {
                element.on_action(cx.listener(|editor, _: &ToggleFold, _, cx| {
                    let area = editor.area.read(cx);
                    let line = area.document().line_at(area.cursor_offset());
                    let id = editor
                        .folds
                        .iter()
                        .rev()
                        .find(|fold| fold.lines.contains(&line))
                        .map(|fold| fold.id.clone());
                    if let Some(id) = id {
                        editor.set_fold_collapsed(&id, !editor.is_fold_collapsed(&id), cx);
                    }
                }))
            })
            .when(self.services_enabled && !self.disabled, |element| {
                element
                    .on_action(cx.listener(|editor, _: &RequestCompletion, _, cx| {
                        editor.request_service(
                            EditorServiceKind::Completion,
                            editor.area.read(cx).cursor_offset(),
                            cx,
                        );
                    }))
                    .on_action(cx.listener(|editor, _: &RequestHover, _, cx| {
                        editor.request_service(
                            EditorServiceKind::Hover,
                            editor.area.read(cx).cursor_offset(),
                            cx,
                        );
                    }))
                    .on_action(cx.listener(|editor, _: &RequestDefinition, _, cx| {
                        editor.request_service(
                            EditorServiceKind::Definition,
                            editor.area.read(cx).cursor_offset(),
                            cx,
                        );
                    }))
                    .on_action(cx.listener(|editor, _: &RequestCodeActions, _, cx| {
                        editor.request_service(
                            EditorServiceKind::CodeActions,
                            editor.area.read(cx).cursor_offset(),
                            cx,
                        );
                    }))
                    .on_mouse_move(cx.listener(|editor, event: &gpui::MouseMoveEvent, _, cx| {
                        if event.pressed_button.is_some()
                            || editor
                                .service_popup
                                .as_ref()
                                .is_some_and(|popup| popup.request.kind != EditorServiceKind::Hover)
                        {
                            return;
                        }
                        let area = editor.area.read(cx);
                        if !area
                            .viewport_bounds()
                            .is_some_and(|bounds| bounds.contains(&event.position))
                        {
                            return;
                        }
                        let position = area.index_for_position(event.position);
                        let key = (area.revision(), position);
                        if editor.hover_position != Some(key) {
                            editor.hover_position = Some(key);
                            editor.request_service(EditorServiceKind::Hover, position, cx);
                        }
                    }))
            })
            .mono(&theme)
            .type_scale(&theme, gpui_kit_theme::TypeScale::Code)
            .children(numbers)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_token(&theme, Space::Sm)
                    .child(self.area.clone()),
            )
            .children(popup)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group).text(self.label.clone()),
            )
    }
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

#[cfg(test)]
mod visual_transform_tests {
    use super::*;
    use crate::controls::input::TestVisualScale;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    fn displayed(bounds: Bounds<Pixels>, enabled: bool) -> Bounds<Pixels> {
        if !enabled {
            return bounds;
        }
        Bounds::new(
            point(bounds.left() * 1.5 + px(7.5), bounds.top() * 1.5 - px(20.5)),
            bounds.size.map(|value| value * 1.5),
        )
    }

    #[gpui::test]
    fn editor_visual_transform_keeps_public_geometry_hover_and_popup_coherent(
        cx: &mut gpui::TestAppContext,
    ) {
        let scaled = Rc::new(Cell::new(false));
        let scale = scaled.clone();
        let slot = Rc::new(RefCell::new(None));
        let build = slot.clone();
        let mut harness = gpui_kit_testkit::harness::Harness::new(
            cx,
            |cx| {
                crate::install(cx);
                // Test affine geometry, not the popup's independent entrance
                // animation, which otherwise moves between bounds snapshots.
                cx.set_reduce_motion(true);
            },
            move |window, cx| {
                let editor = build
                    .borrow_mut()
                    .get_or_insert_with(|| {
                        cx.new(|cx| {
                            Editor::new(
                                "scaled.editor",
                                "Source",
                                "Wi mQz\nabc def\nqrst uv",
                                window,
                                cx,
                            )
                        })
                    })
                    .clone();
                TestVisualScale {
                    enabled: scale.get(),
                    child: div()
                        .p(px(40.0))
                        .w(px(360.0))
                        .child(editor)
                        .into_any_element(),
                }
                .into_any_element()
            },
        );
        harness.frame();
        let editor = slot.borrow().clone().expect("editor mounted");
        let logical = harness.update(|_, cx| editor.read(cx).geometry(cx).expect("geometry"));
        harness.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                editor.set_language_services(true, cx);
                editor
                    .request_service(EditorServiceKind::Completion, 4, cx)
                    .expect("request");
            })
        });
        harness.frame();
        let popup = harness
            .bounds("scaled.editor.service.status")
            .expect("loading popup");
        for enabled in [true, false] {
            scaled.set(enabled);
            harness.frame();
            harness.update(|_, cx| {
                let current = editor.read(cx).geometry(cx).expect("transformed geometry");
                assert_eq!(current.revision, logical.revision);
                assert_eq!(current.viewport, displayed(logical.viewport, enabled));
                assert_eq!(current.horizontal_scroll, logical.horizontal_scroll);
                assert_eq!(current.vertical_scroll, logical.vertical_scroll);
                assert_eq!(current.lines.len(), logical.lines.len());
                for (actual, original) in current.lines.iter().zip(&logical.lines) {
                    assert_eq!(actual.range, original.range);
                    assert_eq!(actual.bounds, displayed(original.bounds, enabled));
                }
            });
            assert_eq!(
                harness.bounds("scaled.editor.service.status"),
                Some(displayed(popup, enabled)),
                "deferred popup anchor must not be transformed twice"
            );
        }
        scaled.set(true);
        harness.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                editor.service_popup = None;
                cx.notify();
            })
        });
        harness.frame();
        let point = harness.update(|_, cx| {
            editor
                .read(cx)
                .area
                .read(cx)
                .bounds_for_position(11)
                .expect("hover caret")
                .center()
        });
        harness
            .context()
            .simulate_mouse_move(point, None, gpui::Modifiers::none());
        harness.update(|_, cx| {
            let editor = editor.read(cx);
            let request = &editor
                .service_popup
                .as_ref()
                .expect("hover request")
                .request;
            assert_eq!(request.kind, EditorServiceKind::Hover);
            assert_eq!(request.position, 11);
        });
    }
}
