//! A storage-neutral structured rich-text editor.
//!
//! `RichTextEditor` projects a caller-owned [`RichTextEditSession`] onto
//! GPUI's editable-text geometry. It owns focus, dragging, scroll position,
//! and the last shaped layout, but never an authoritative document. The host
//! supplies stable block ids for hard breaks and remains responsible for
//! persistence, collaboration, link policy, grammar, and document formats.

mod element;
mod projection;

use std::ops::Range;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gpui::{
    AccessibleAction, App, Bounds, ClipboardItem, Context, CursorStyle, Entity, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, UTF16Selection, Window,
    accesskit::ActionData, actions, div, point, prelude::FluentBuilder as _, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius};

use crate::content::{
    RichTextAlignment, RichTextBlockId, RichTextEditResult, RichTextEditSession, RichTextError,
    RichTextFormat, RichTextInputKind, RichTextIntent, RichTextListKind, RichTextPosition,
    RichTextRange, RichTextSelection,
};
use crate::controls::button::Button;
use crate::controls::text_edit;
use crate::controls::textarea::Frame;
use crate::foundation::{ActiveDirection, Disableable, Ident, Selectable, Sizable, StyledExt};
use crate::strings::{ActiveStrings, StringKey};
use element::{RichTextEditorElement, StoredBlockLayout};
use projection::Projection;

actions!(
    gpui_kit_rich_text_editor,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        DeleteWordRight,
        Left,
        Right,
        Up,
        Down,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectWordLeft,
        SelectWordRight,
        LineStart,
        LineEnd,
        SelectToLineStart,
        SelectToLineEnd,
        DocumentStart,
        DocumentEnd,
        SelectToDocumentStart,
        SelectToDocumentEnd,
        SelectAll,
        HardBreak,
        SoftBreak,
        Undo,
        Redo,
        Copy,
        Cut,
        Paste,
        ToggleBold,
        ToggleItalic,
        ToggleUnderline,
        ToggleStrike,
        ToggleCode,
        Indent,
        Outdent,
        ShowCharacterPalette,
    ]
);

/// The key context a rich-text editor publishes.
pub const KEY_CONTEXT: &str = "RichTextEditor";
const DEFAULT_ROWS: usize = 5;
const DEFAULT_MAX_ROWS: usize = 12;

struct RichTextEditorBindings;

impl gpui::Global for RichTextEditorBindings {}

/// Installs the editor's platform editing and formatting bindings.
pub(crate) fn install(cx: &mut App) {
    if cx.has_global::<RichTextEditorBindings>() {
        return;
    }
    cx.set_global(RichTextEditorBindings);
    let primary = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let word = if cfg!(target_os = "macos") {
        "alt"
    } else {
        "ctrl"
    };
    let mut bindings = vec![
        KeyBinding::new("backspace", Backspace, Some(KEY_CONTEXT)),
        KeyBinding::new("delete", Delete, Some(KEY_CONTEXT)),
        KeyBinding::new("left", Left, Some(KEY_CONTEXT)),
        KeyBinding::new("right", Right, Some(KEY_CONTEXT)),
        KeyBinding::new("up", Up, Some(KEY_CONTEXT)),
        KeyBinding::new("down", Down, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-left", SelectLeft, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-right", SelectRight, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-up", SelectUp, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-down", SelectDown, Some(KEY_CONTEXT)),
        KeyBinding::new("home", LineStart, Some(KEY_CONTEXT)),
        KeyBinding::new("end", LineEnd, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-home", SelectToLineStart, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-end", SelectToLineEnd, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", HardBreak, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-enter", SoftBreak, Some(KEY_CONTEXT)),
        KeyBinding::new("tab", Indent, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-tab", Outdent, Some(KEY_CONTEXT)),
        KeyBinding::new(&format!("{primary}-home"), DocumentStart, Some(KEY_CONTEXT)),
        KeyBinding::new(&format!("{primary}-end"), DocumentEnd, Some(KEY_CONTEXT)),
        KeyBinding::new(
            &format!("{primary}-shift-home"),
            SelectToDocumentStart,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new(
            &format!("{primary}-shift-end"),
            SelectToDocumentEnd,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new(&format!("{word}-left"), WordLeft, Some(KEY_CONTEXT)),
        KeyBinding::new(&format!("{word}-right"), WordRight, Some(KEY_CONTEXT)),
        KeyBinding::new(
            &format!("{word}-shift-left"),
            SelectWordLeft,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new(
            &format!("{word}-shift-right"),
            SelectWordRight,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new(
            &format!("{word}-backspace"),
            DeleteWordLeft,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new(
            &format!("{word}-delete"),
            DeleteWordRight,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new(&format!("{primary}-a"), SelectAll, Some(KEY_CONTEXT)),
        KeyBinding::new(&format!("{primary}-c"), Copy, Some(KEY_CONTEXT)),
        KeyBinding::new(&format!("{primary}-x"), Cut, Some(KEY_CONTEXT)),
        KeyBinding::new(&format!("{primary}-v"), Paste, Some(KEY_CONTEXT)),
        KeyBinding::new(&format!("{primary}-z"), Undo, Some(KEY_CONTEXT)),
        KeyBinding::new(&format!("{primary}-shift-z"), Redo, Some(KEY_CONTEXT)),
        KeyBinding::new(&format!("{primary}-b"), ToggleBold, Some(KEY_CONTEXT)),
        KeyBinding::new(&format!("{primary}-i"), ToggleItalic, Some(KEY_CONTEXT)),
        KeyBinding::new(&format!("{primary}-u"), ToggleUnderline, Some(KEY_CONTEXT)),
        KeyBinding::new(
            &format!("{primary}-shift-x"),
            ToggleStrike,
            Some(KEY_CONTEXT),
        ),
        KeyBinding::new(&format!("{primary}-e"), ToggleCode, Some(KEY_CONTEXT)),
    ];
    if cfg!(target_os = "macos") {
        bindings.extend([
            KeyBinding::new("cmd-left", LineStart, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-right", LineEnd, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-up", DocumentStart, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-down", DocumentEnd, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-shift-left", SelectToLineStart, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-shift-right", SelectToLineEnd, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-shift-up", SelectToDocumentStart, Some(KEY_CONTEXT)),
            KeyBinding::new("cmd-shift-down", SelectToDocumentEnd, Some(KEY_CONTEXT)),
            KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some(KEY_CONTEXT)),
        ]);
    }
    cx.bind_keys(bindings);
}

/// Severity rendered under one caller-owned document range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RichTextDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// A diagnostic projection. The editor draws it but never decides when or how
/// validation runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RichTextDiagnostic {
    pub range: RichTextRange,
    pub severity: RichTextDiagnosticSeverity,
}

impl RichTextDiagnostic {
    pub fn new(range: RichTextRange, severity: RichTextDiagnosticSeverity) -> Self {
        Self { range, severity }
    }
}

/// Typed output from the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RichTextEditorEvent {
    /// An intent reached the caller-owned session.
    IntentApplied {
        intent: RichTextIntent,
        result: RichTextEditResult,
    },
    /// A malformed caller range or duplicate generated block id was refused.
    IntentRefused {
        intent: RichTextIntent,
        error: RichTextError,
    },
    /// The link action needs the host's destination picker and URL policy.
    LinkRequested(RichTextSelection),
    Focus,
    Blur,
}

type BlockIdFactory = Rc<dyn Fn() -> RichTextBlockId>;

/// The visible projection over a caller-owned rich-text editing session.
pub struct RichTextEditor {
    ident: Ident,
    session: Entity<RichTextEditSession>,
    new_block_id: BlockIdFactory,
    focus_handle: FocusHandle,
    name: SharedString,
    placeholder: SharedString,
    disabled: bool,
    read_only: bool,
    invalid: bool,
    required: bool,
    frame: Frame,
    toolbar: bool,
    rows: usize,
    max_rows: usize,
    visible_rows: usize,
    scroll_offset: Pixels,
    goal_x: Option<Pixels>,
    is_selecting: bool,
    diagnostics: Vec<RichTextDiagnostic>,
    last_layouts: Vec<StoredBlockLayout>,
    last_bounds: Option<Bounds<Pixels>>,
    content_height: Pixels,
    accessibility_revision: u64,
    accessible_snapshot: Arc<Mutex<Option<gpui::PublishedAccessibleText>>>,
    accessible_geometry: Arc<Mutex<Option<text_edit::AccessibleTextGeometry>>>,
    _subscriptions: Vec<Subscription>,
}

impl RichTextEditor {
    /// Creates an editor over caller-owned state. `new_block_id` is called for
    /// each hard break and must return a fresh stable content identity.
    pub fn new(
        ident: impl Into<Ident>,
        session: Entity<RichTextEditSession>,
        new_block_id: impl Fn() -> RichTextBlockId + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let subscriptions = vec![
            cx.on_focus(&focus_handle, window, |_, _, cx| {
                cx.emit(RichTextEditorEvent::Focus);
                cx.notify();
            }),
            cx.on_blur(&focus_handle, window, |this, _, cx| {
                this.goal_x = None;
                cx.emit(RichTextEditorEvent::Blur);
                cx.notify();
            }),
            cx.observe(&session, |this, _, cx| {
                this.goal_x = None;
                this.accessibility_revision = this.accessibility_revision.wrapping_add(1);
                cx.notify();
            }),
        ];
        Self {
            ident: ident.into(),
            session,
            new_block_id: Rc::new(new_block_id),
            focus_handle,
            name: SharedString::default(),
            placeholder: cx.strings().text(StringKey::RichTextPlaceholder),
            disabled: false,
            read_only: false,
            invalid: false,
            required: false,
            frame: Frame::Own,
            toolbar: true,
            rows: DEFAULT_ROWS,
            max_rows: DEFAULT_MAX_ROWS,
            visible_rows: DEFAULT_ROWS,
            scroll_offset: px(0.0),
            goal_x: None,
            is_selecting: false,
            diagnostics: Vec::new(),
            last_layouts: Vec::new(),
            last_bounds: None,
            content_height: px(0.0),
            accessibility_revision: 0,
            accessible_snapshot: Arc::default(),
            accessible_geometry: Arc::default(),
            _subscriptions: subscriptions,
        }
    }

    pub fn session(&self) -> &Entity<RichTextEditSession> {
        &self.session
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Names the field for a reader without drawing another label.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = name.into();
        self
    }

    pub fn set_name(&mut self, name: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.name = name.into();
        cx.notify();
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    pub fn frame(mut self, frame: Frame) -> Self {
        self.frame = frame;
        self
    }

    pub fn toolbar(mut self, toolbar: bool) -> Self {
        self.toolbar = toolbar;
        self
    }

    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self.visible_rows = self.rows;
        self.max_rows = self.max_rows.max(self.rows);
        self
    }

    pub fn max_rows(mut self, max_rows: usize) -> Self {
        self.max_rows = max_rows.max(self.rows);
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Keeps selection, copy, and semantics available while refusing edits.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn diagnostics(
        mut self,
        diagnostics: impl IntoIterator<Item = RichTextDiagnostic>,
    ) -> Self {
        self.diagnostics = diagnostics.into_iter().collect();
        self
    }

    pub fn set_diagnostics(
        &mut self,
        diagnostics: impl IntoIterator<Item = RichTextDiagnostic>,
        cx: &mut Context<Self>,
    ) {
        self.diagnostics = diagnostics.into_iter().collect();
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.end_transient_input(cx);
        }
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        if read_only {
            self.end_transient_input(cx);
        }
        cx.notify();
    }

    pub fn set_invalid(&mut self, invalid: bool, cx: &mut Context<Self>) {
        self.invalid = invalid;
        cx.notify();
    }

    pub fn set_required(&mut self, required: bool, cx: &mut Context<Self>) {
        self.required = required;
        cx.notify();
    }

    /// Applies a host-created intent through the same boundary as keyboard and
    /// toolbar actions.
    pub fn apply_intent(&mut self, intent: RichTextIntent, cx: &mut Context<Self>) {
        if self.disabled || (self.read_only && !matches!(intent, RichTextIntent::Select(_))) {
            return;
        }
        self.apply_unchecked(intent, cx);
    }

    pub(super) fn visible_rows(&self) -> usize {
        self.visible_rows
    }

    pub(super) fn row_limits(&self) -> (usize, usize) {
        (self.rows, self.max_rows)
    }

    pub(super) fn placeholder_text(&self) -> &SharedString {
        &self.placeholder
    }

    pub(super) fn diagnostic_items(&self) -> &[RichTextDiagnostic] {
        &self.diagnostics
    }

    pub(super) fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub(super) fn scroll_offset(&self) -> Pixels {
        self.scroll_offset
    }

    pub(super) fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    fn set_last_layouts(
        &mut self,
        layouts: Vec<StoredBlockLayout>,
        bounds: Bounds<Pixels>,
        content_height: Pixels,
        visible_rows: usize,
        scroll_offset: Pixels,
    ) -> bool {
        let changed = self.visible_rows != visible_rows;
        self.last_layouts = layouts;
        self.last_bounds = Some(bounds);
        self.content_height = content_height;
        self.visible_rows = visible_rows;
        self.scroll_offset = scroll_offset;
        changed
    }

    fn projection(&self, cx: &App) -> Projection {
        Projection::new(self.session.read(cx).document())
    }

    fn end_transient_input(&mut self, cx: &mut Context<Self>) {
        self.is_selecting = false;
        self.goal_x = None;
        self.session.update(cx, |session, session_cx| {
            if session
                .apply(RichTextIntent::EndComposition)
                .is_ok_and(|result| result.changed())
            {
                session_cx.notify();
            }
        });
    }

    fn apply_unchecked(&mut self, intent: RichTextIntent, cx: &mut Context<Self>) {
        let emitted = intent.clone();
        let outcome = self.session.update(cx, |session, session_cx| {
            let outcome = session.apply(intent);
            if outcome.as_ref().is_ok_and(|result| result.changed()) {
                session_cx.notify();
            }
            outcome
        });
        match outcome {
            Ok(result) => {
                if result.changed() {
                    self.goal_x = None;
                    cx.notify();
                }
                cx.emit(RichTextEditorEvent::IntentApplied {
                    intent: emitted,
                    result,
                });
            }
            Err(error) => cx.emit(RichTextEditorEvent::IntentRefused {
                intent: emitted,
                error,
            }),
        }
    }

    fn apply_flat_selection(&mut self, anchor: usize, head: usize, cx: &mut Context<Self>) {
        let projection = self.projection(cx);
        let selection = RichTextSelection::new(
            projection.position_for_offset(anchor),
            projection.position_for_offset(head),
        );
        self.apply_unchecked(RichTextIntent::Select(selection), cx);
    }

    fn apply_flat_edit(
        &mut self,
        range: Range<usize>,
        text: impl Into<SharedString>,
        kind: RichTextInputKind,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.read_only {
            return;
        }
        let text = text.into();
        let emitted = RichTextIntent::Replace {
            text: text.clone(),
            kind,
        };
        let projection = self.projection(cx);
        let selection = projection.selection_for_range(range);
        let outcome = self.session.update(cx, |session, session_cx| {
            let selected = session.apply(RichTextIntent::Select(selection))?;
            let replaced = session.apply(RichTextIntent::Replace { text, kind })?;
            let result = combine_results(selected, replaced);
            if result.changed() {
                session_cx.notify();
            }
            Ok::<_, RichTextError>(result)
        });
        self.finish_compound(emitted, outcome, cx);
    }

    fn finish_compound(
        &mut self,
        intent: RichTextIntent,
        outcome: Result<RichTextEditResult, RichTextError>,
        cx: &mut Context<Self>,
    ) {
        match outcome {
            Ok(result) => {
                self.goal_x = None;
                if result.changed() {
                    cx.notify();
                }
                cx.emit(RichTextEditorEvent::IntentApplied { intent, result });
            }
            Err(error) => cx.emit(RichTextEditorEvent::IntentRefused { intent, error }),
        }
    }

    fn flat_selection(&self, cx: &App) -> (Projection, usize, usize) {
        let session = self.session.read(cx);
        let projection = Projection::new(session.document());
        let (anchor, head) = projection
            .offsets_for_selection(session.selection())
            .unwrap_or((0, 0));
        (projection, anchor, head)
    }

    fn edit_range(&self, range_utf16: Option<&Range<usize>>, cx: &App) -> Range<usize> {
        let (projection, anchor, head) = self.flat_selection(cx);
        range_utf16
            .map(|range| text_edit::range_from_utf16(projection.text(), range))
            .or_else(|| {
                self.session.read(cx).marked_range().and_then(|marked| {
                    projection.range_for_selection(&RichTextSelection::new(
                        marked.start.clone(),
                        marked.end.clone(),
                    ))
                })
            })
            .unwrap_or_else(|| anchor.min(head)..anchor.max(head))
    }

    fn selected_flat_range(&self, cx: &App) -> Range<usize> {
        let (_, anchor, head) = self.flat_selection(cx);
        anchor.min(head)..anchor.max(head)
    }

    fn move_flat(&mut self, next: usize, extend: bool, cx: &mut Context<Self>) {
        let (_, anchor, _) = self.flat_selection(cx);
        let next = next.min(self.projection(cx).len());
        self.apply_flat_selection(if extend { anchor } else { next }, next, cx);
    }

    fn move_horizontal(
        &mut self,
        right: bool,
        extend: bool,
        by_word: bool,
        cx: &mut Context<Self>,
    ) {
        let (projection, anchor, head) = self.flat_selection(cx);
        let selection = anchor.min(head)..anchor.max(head);
        let next = if !extend && !selection.is_empty() {
            if right {
                selection.end
            } else {
                selection.start
            }
        } else if right {
            if by_word {
                text_edit::next_word_boundary(projection.text(), head)
            } else {
                text_edit::next_boundary(projection.text(), head)
            }
        } else if by_word {
            text_edit::previous_word_boundary(projection.text(), head)
        } else {
            text_edit::previous_boundary(projection.text(), head)
        };
        self.move_flat(next, extend, cx);
    }

    fn line_edge(&self, end: bool, cx: &App) -> usize {
        let session = self.session.read(cx);
        let projection = Projection::new(session.document());
        let head = &session.selection().head;
        let Some(layout) = self.valid_layout_for(&head.block, session.document()) else {
            return projection.offset_for_position(head).unwrap_or(0);
        };
        let range = layout
            .layout
            .row_range(layout.layout.row_for_offset(head.offset));
        projection
            .block(&head.block)
            .map(|block| block.start + if end { range.end } else { range.start })
            .unwrap_or_else(|| projection.offset_for_position(head).unwrap_or(0))
    }

    fn move_vertical(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
        let session = self.session.read(cx);
        let document = session.document();
        let head = session.selection().head.clone();
        let Some(layout) = self.valid_layout_for(&head.block, document) else {
            return;
        };
        let Some(bounds) = self.last_bounds else {
            return;
        };
        let caret =
            layout
                .layout
                .position_for_offset_aligned(head.offset, layout.align, layout.width);
        let screen_x = bounds.left() + layout.left + caret.x;
        let goal_x = self.goal_x.unwrap_or(screen_x);
        let screen_y = bounds.top() + layout.top - self.scroll_offset + caret.y;
        let target = point(
            goal_x,
            screen_y
                + if down {
                    layout.layout.line_height()
                } else {
                    -layout.layout.line_height()
                },
        );
        let next = self.position_for_point(target, cx);
        self.goal_x = Some(goal_x);
        let (_, anchor, _) = self.flat_selection(cx);
        let projection = self.projection(cx);
        let head = projection.offset_for_position(&next).unwrap_or(0);
        self.apply_flat_selection(if extend { anchor } else { head }, head, cx);
        self.goal_x = Some(goal_x);
    }

    fn valid_layout_for(
        &self,
        id: &RichTextBlockId,
        document: &crate::content::RichTextDocument,
    ) -> Option<&StoredBlockLayout> {
        let block = document.block(id)?;
        self.last_layouts
            .iter()
            .find(|layout| layout.id == *id && layout.source == *block.text())
    }

    fn position_for_point(&self, position: Point<Pixels>, cx: &App) -> RichTextPosition {
        let session = self.session.read(cx);
        let document = session.document();
        let Some(bounds) = self.last_bounds else {
            return document.selection_at_start().head;
        };
        let content_y = position.y - bounds.top() + self.scroll_offset;
        let Some(layout) = self
            .last_layouts
            .iter()
            .filter(|layout| {
                document
                    .block(&layout.id)
                    .is_some_and(|block| layout.source == *block.text())
            })
            .min_by(|left, right| {
                distance_to_vertical_span(content_y, left.top, left.top + left.layout.height())
                    .total_cmp(&distance_to_vertical_span(
                        content_y,
                        right.top,
                        right.top + right.layout.height(),
                    ))
            })
        else {
            return document.selection_at_start().head;
        };
        let offset = layout.layout.offset_for_position_aligned(
            point(
                position.x - bounds.left() - layout.left,
                content_y - layout.top,
            ),
            layout.align,
            layout.width,
        );
        RichTextPosition::new(layout.id.clone(), offset)
    }

    fn accessible_rows(&self, cx: &App) -> Option<Vec<Range<usize>>> {
        let session = self.session.read(cx);
        let projection = Projection::new(session.document());
        if self.last_layouts.len() != session.document().blocks().len() {
            return None;
        }
        let mut rows = Vec::new();
        for (index, block) in session.document().blocks().iter().enumerate() {
            let layout = self.valid_layout_for(block.id(), session.document())?;
            let projected = projection.block(block.id())?;
            let mut local = layout.layout.visual_rows(block.text());
            if index + 1 < session.document().blocks().len()
                && let Some(last) = local.last_mut()
            {
                last.end += 1;
            }
            rows.extend(
                local
                    .into_iter()
                    .map(|range| projected.start + range.start..projected.start + range.end),
            );
        }
        Some(rows)
    }

    fn semantics(&self, cx: &App) -> NodeSpec {
        let projection = self.projection(cx);
        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::MultilineInput)
            .disabled(self.disabled)
            .read_only(self.read_only)
            .invalid(self.invalid)
            .required(self.required);
        if !self.disabled {
            spec = spec.focus(&self.focus_handle);
        }
        if !self.placeholder.is_empty() {
            spec = spec.placeholder(self.placeholder.clone());
        }
        if !self.name.is_empty() {
            spec = spec.text(self.name.clone());
        }
        if !projection.text().is_empty() {
            spec = spec.value(projection.text().clone());
        }
        spec
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        let (projection, anchor, head) = self.flat_selection(cx);
        if anchor != head {
            self.apply_flat_edit(
                anchor.min(head)..anchor.max(head),
                SharedString::default(),
                RichTextInputKind::Deleting,
                cx,
            );
            return;
        }
        let position = projection.position_for_offset(head);
        if position.offset == 0 {
            self.apply_unchecked(RichTextIntent::BackspaceAtStart, cx);
            return;
        }
        self.apply_flat_edit(
            text_edit::previous_boundary(projection.text(), head)..head,
            SharedString::default(),
            RichTextInputKind::Deleting,
            cx,
        );
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_forward(false, cx);
    }

    fn delete_word_left(&mut self, _: &DeleteWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let (projection, anchor, head) = self.flat_selection(cx);
        let range = if anchor == head {
            text_edit::previous_word_boundary(projection.text(), head)..head
        } else {
            anchor.min(head)..anchor.max(head)
        };
        self.apply_flat_edit(
            range,
            SharedString::default(),
            RichTextInputKind::Deleting,
            cx,
        );
    }

    fn delete_word_right(&mut self, _: &DeleteWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.delete_forward(true, cx);
    }

    fn delete_forward(&mut self, by_word: bool, cx: &mut Context<Self>) {
        let (projection, anchor, head) = self.flat_selection(cx);
        let range = if anchor == head {
            head..if by_word {
                text_edit::next_word_boundary(projection.text(), head)
            } else {
                text_edit::next_boundary(projection.text(), head)
            }
        } else {
            anchor.min(head)..anchor.max(head)
        };
        self.apply_flat_edit(
            range,
            SharedString::default(),
            RichTextInputKind::Deleting,
            cx,
        );
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontal(false, false, false, cx);
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontal(true, false, false, cx);
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontal(false, false, true, cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontal(true, false, true, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontal(false, true, false, cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontal(true, true, false, cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontal(false, true, true, cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_horizontal(true, true, true, cx);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(false, false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(true, false, cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(false, true, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(true, true, cx);
    }

    fn line_start(&mut self, _: &LineStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_flat(self.line_edge(false, cx), false, cx);
    }

    fn line_end(&mut self, _: &LineEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_flat(self.line_edge(true, cx), false, cx);
    }

    fn select_to_line_start(
        &mut self,
        _: &SelectToLineStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_flat(self.line_edge(false, cx), true, cx);
    }

    fn select_to_line_end(&mut self, _: &SelectToLineEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_flat(self.line_edge(true, cx), true, cx);
    }

    fn document_start(&mut self, _: &DocumentStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_flat(0, false, cx);
    }

    fn document_end(&mut self, _: &DocumentEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_flat(self.projection(cx).len(), false, cx);
    }

    fn select_to_document_start(
        &mut self,
        _: &SelectToDocumentStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_flat(0, true, cx);
    }

    fn select_to_document_end(
        &mut self,
        _: &SelectToDocumentEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_flat(self.projection(cx).len(), true, cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_flat_selection(0, self.projection(cx).len(), cx);
    }

    fn hard_break(&mut self, _: &HardBreak, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_unchecked(
            RichTextIntent::InsertHardBreak {
                new_block: (self.new_block_id)(),
            },
            cx,
        );
    }

    fn soft_break(&mut self, _: &SoftBreak, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_unchecked(RichTextIntent::InsertSoftBreak, cx);
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_unchecked(RichTextIntent::Undo, cx);
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_unchecked(RichTextIntent::Redo, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let (projection, anchor, head) = self.flat_selection(cx);
        let range = anchor.min(head)..anchor.max(head);
        if let Some(selected) = projection.text().get(range) {
            cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.read_only {
            return;
        }
        let (projection, anchor, head) = self.flat_selection(cx);
        let range = anchor.min(head)..anchor.max(head);
        if range.is_empty() {
            return;
        }
        if let Some(selected) = projection.text().get(range.clone()) {
            cx.write_to_clipboard(ClipboardItem::new_string(selected.to_owned()));
            self.apply_flat_edit(range, SharedString::default(), RichTextInputKind::Cut, cx);
        }
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.read_only {
            return;
        }
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        self.replace_plain_text(
            self.selected_flat_range(cx),
            &text,
            RichTextInputKind::Paste,
            cx,
        );
    }

    fn replace_plain_text(
        &mut self,
        range: Range<usize>,
        text: &str,
        kind: RichTextInputKind,
        cx: &mut Context<Self>,
    ) {
        let normalized = text_edit::normalize_multiline(text);
        let break_count = normalized.matches('\n').count();
        if break_count == 0 {
            self.apply_flat_edit(range, normalized, kind, cx);
            return;
        }
        let new_blocks = (0..break_count)
            .map(|_| (self.new_block_id)())
            .collect::<Vec<_>>();
        let intent = RichTextIntent::ReplaceMultiline {
            text: normalized.into(),
            new_blocks,
            kind,
        };
        let emitted = intent.clone();
        let selection = self.projection(cx).selection_for_range(range);
        let outcome = self.session.update(cx, |session, session_cx| {
            let selected = session.apply(RichTextIntent::Select(selection))?;
            let replaced = session.apply(intent)?;
            let result = combine_results(selected, replaced);
            if result.changed() {
                session_cx.notify();
            }
            Ok::<_, RichTextError>(result)
        });
        self.finish_compound(emitted, outcome, cx);
    }

    fn toggle_bold(&mut self, _: &ToggleBold, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_unchecked(RichTextIntent::ToggleFormat(RichTextFormat::Bold), cx);
    }

    fn toggle_italic(&mut self, _: &ToggleItalic, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_unchecked(RichTextIntent::ToggleFormat(RichTextFormat::Italic), cx);
    }

    fn toggle_underline(&mut self, _: &ToggleUnderline, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_unchecked(RichTextIntent::ToggleFormat(RichTextFormat::Underline), cx);
    }

    fn toggle_strike(&mut self, _: &ToggleStrike, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_unchecked(RichTextIntent::ToggleFormat(RichTextFormat::Strike), cx);
    }

    fn toggle_code(&mut self, _: &ToggleCode, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_unchecked(RichTextIntent::ToggleFormat(RichTextFormat::Code), cx);
    }

    fn indent(&mut self, _: &Indent, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_unchecked(RichTextIntent::ChangeListDepth(1), cx);
    }

    fn outdent(&mut self, _: &Outdent, _: &mut Window, cx: &mut Context<Self>) {
        self.apply_unchecked(RichTextIntent::ChangeListDepth(-1), cx);
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        window.focus(&self.focus_handle, cx);
        self.is_selecting = true;
        let position = self.position_for_point(event.position, cx);
        let projection = self.projection(cx);
        let offset = projection.offset_for_position(&position).unwrap_or(0);
        let (_, anchor, _) = self.flat_selection(cx);
        if event.modifiers.shift {
            self.apply_flat_selection(anchor, offset, cx);
        } else if event.click_count >= 3 {
            if let Some(block) = projection.block(&position.block) {
                self.apply_flat_selection(block.start, block.end, cx);
            }
        } else if event.click_count == 2 {
            let range = text_edit::word_at(projection.text(), offset);
            self.apply_flat_selection(range.start, range.end, cx);
        } else {
            self.apply_flat_selection(offset, offset, cx);
        }
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.is_selecting {
            return;
        }
        let next = self.position_for_point(event.position, cx);
        let projection = self.projection(cx);
        let next = projection.offset_for_position(&next).unwrap_or(0);
        let (_, anchor, _) = self.flat_selection(cx);
        self.apply_flat_selection(anchor, next, cx);
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn formatting_toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let strings = cx.strings();
        let session = self.session.read(cx);
        let pending = session.pending_style();
        let head = &session.selection().head;
        let paragraph = session
            .document()
            .block(&head.block)
            .map(|block| block.paragraph());
        let list = paragraph.and_then(|style| style.list());
        let can_edit = !self.disabled && !self.read_only;
        let can_undo = can_edit && session.can_undo();
        let can_redo = can_edit && session.can_redo();
        let entity = cx.entity().clone();
        let toolbar_id = self.ident.child("toolbar");

        let action = |suffix: &'static str,
                      label: SharedString,
                      selected: bool,
                      enabled: bool,
                      intent: RichTextIntent| {
            let entity = entity.clone();
            Button::new(toolbar_id.child(suffix))
                .label(label)
                .ghost()
                .control_size(ControlSize::Xs)
                .selected(selected)
                .disabled(!enabled)
                .when(enabled, move |button| {
                    button.on_click(move |window, cx| {
                        entity.update(cx, |editor, cx| {
                            editor.apply_intent(intent.clone(), cx);
                            window.focus(&editor.focus_handle, cx);
                        });
                    })
                })
        };

        let inline = vec![
            action(
                "undo",
                strings.text(StringKey::RichTextUndo),
                false,
                can_undo,
                RichTextIntent::Undo,
            ),
            action(
                "redo",
                strings.text(StringKey::RichTextRedo),
                false,
                can_redo,
                RichTextIntent::Redo,
            ),
            action(
                "bold",
                strings.text(StringKey::RichTextBold),
                pending.format(RichTextFormat::Bold),
                can_edit,
                RichTextIntent::ToggleFormat(RichTextFormat::Bold),
            ),
            action(
                "italic",
                strings.text(StringKey::RichTextItalic),
                pending.format(RichTextFormat::Italic),
                can_edit,
                RichTextIntent::ToggleFormat(RichTextFormat::Italic),
            ),
            action(
                "underline",
                strings.text(StringKey::RichTextUnderline),
                pending.format(RichTextFormat::Underline),
                can_edit,
                RichTextIntent::ToggleFormat(RichTextFormat::Underline),
            ),
            action(
                "strike",
                strings.text(StringKey::RichTextStrike),
                pending.format(RichTextFormat::Strike),
                can_edit,
                RichTextIntent::ToggleFormat(RichTextFormat::Strike),
            ),
            action(
                "code",
                strings.text(StringKey::RichTextCode),
                pending.format(RichTextFormat::Code),
                can_edit,
                RichTextIntent::ToggleFormat(RichTextFormat::Code),
            ),
            {
                let link = strings.text(StringKey::RichTextLink);
                let entity = entity.clone();
                Button::new(toolbar_id.child("link"))
                    .label(link)
                    .ghost()
                    .control_size(ControlSize::Xs)
                    .selected(pending.link().is_some())
                    .disabled(!can_edit)
                    .when(can_edit, move |button| {
                        button.on_click(move |window, cx| {
                            entity.update(cx, |editor, cx| {
                                let selection = editor.session.read(cx).selection().clone();
                                cx.emit(RichTextEditorEvent::LinkRequested(selection));
                                window.focus(&editor.focus_handle, cx);
                            });
                        })
                    })
            },
        ];
        let paragraphs = vec![
            action(
                "bullets",
                strings.text(StringKey::RichTextUnorderedList),
                list.is_some_and(|item| item.kind == RichTextListKind::Unordered),
                can_edit,
                RichTextIntent::SetList(
                    (!list.is_some_and(|item| item.kind == RichTextListKind::Unordered))
                        .then_some(RichTextListKind::Unordered),
                ),
            ),
            action(
                "numbers",
                strings.text(StringKey::RichTextOrderedList),
                list.is_some_and(|item| item.kind == RichTextListKind::Ordered),
                can_edit,
                RichTextIntent::SetList(
                    (!list.is_some_and(|item| item.kind == RichTextListKind::Ordered))
                        .then_some(RichTextListKind::Ordered),
                ),
            ),
            action(
                "indent",
                strings.text(StringKey::RichTextIndent),
                false,
                can_edit && list.is_some(),
                RichTextIntent::ChangeListDepth(1),
            ),
            action(
                "outdent",
                strings.text(StringKey::RichTextOutdent),
                false,
                can_edit && list.is_some(),
                RichTextIntent::ChangeListDepth(-1),
            ),
            action(
                "start",
                strings.text(StringKey::RichTextAlignStart),
                paragraph.is_some_and(|style| style.alignment() == RichTextAlignment::Start),
                can_edit,
                RichTextIntent::SetAlignment(RichTextAlignment::Start),
            ),
            action(
                "center",
                strings.text(StringKey::RichTextAlignCenter),
                paragraph.is_some_and(|style| style.alignment() == RichTextAlignment::Center),
                can_edit,
                RichTextIntent::SetAlignment(RichTextAlignment::Center),
            ),
            action(
                "end",
                strings.text(StringKey::RichTextAlignEnd),
                paragraph.is_some_and(|style| style.alignment() == RichTextAlignment::End),
                can_edit,
                RichTextIntent::SetAlignment(RichTextAlignment::End),
            ),
        ];

        div()
            .id(toolbar_id.element_id())
            .column()
            .gap(px(theme.spacing.xs))
            .pb(px(theme.spacing.xs))
            .child(
                div()
                    .row()
                    .flex_wrap()
                    .gap(px(theme.spacing.xs))
                    .children(inline),
            )
            .child(
                div()
                    .row()
                    .flex_wrap()
                    .gap(px(theme.spacing.xs))
                    .children(paragraphs),
            )
            .semantic_in(
                cx,
                NodeSpec::new(toolbar_id.semantic_id(), Role::Toolbar)
                    .parent(self.ident.semantic_id())
                    .text(strings.text(StringKey::RichTextToolbar)),
            )
    }
}

impl std::fmt::Debug for RichTextEditor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RichTextEditor")
            .field("ident", &self.ident)
            .field("disabled", &self.disabled)
            .field("read_only", &self.read_only)
            .field("invalid", &self.invalid)
            .finish_non_exhaustive()
    }
}

impl Disableable for RichTextEditor {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Focusable for RichTextEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<RichTextEditorEvent> for RichTextEditor {}

impl EntityInputHandler for RichTextEditor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let projection = self.projection(cx);
        let range = text_edit::range_from_utf16(projection.text(), &range_utf16);
        actual_range.replace(text_edit::range_to_utf16(projection.text(), &range));
        projection.text().get(range).map(ToOwned::to_owned)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let (projection, anchor, head) = self.flat_selection(cx);
        Some(UTF16Selection {
            range: text_edit::range_to_utf16(
                projection.text(),
                &(anchor.min(head)..anchor.max(head)),
            ),
            reversed: head < anchor,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let projection = self.projection(cx);
        let marked = self.session.read(cx).marked_range()?;
        let range = projection.range_for_selection(&RichTextSelection::new(
            marked.start.clone(),
            marked.end.clone(),
        ))?;
        Some(text_edit::range_to_utf16(projection.text(), &range))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.apply_unchecked(RichTextIntent::EndComposition, cx);
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.edit_range(range_utf16.as_ref(), cx);
        self.replace_plain_text(range, new_text, RichTextInputKind::Typing, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.read_only {
            return;
        }
        let range = self.edit_range(range_utf16.as_ref(), cx);
        let projection = self.projection(cx);
        let selection = projection.selection_for_range(range);
        let normalized = text_edit::normalize_multiline(new_text);
        let inside = new_selected_range_utf16
            .as_ref()
            .map(|range| text_edit::range_from_utf16(&normalized, range));
        let composing = self.session.read(cx).marked_range().is_some();
        let intent = RichTextIntent::Compose {
            text: normalized.into(),
            selection_in_text: inside,
        };
        let emitted = intent.clone();
        let outcome = self.session.update(cx, |session, session_cx| {
            let result = if composing {
                session.apply(intent)?
            } else {
                let selected = session.apply(RichTextIntent::Select(selection))?;
                let composed = session.apply(intent)?;
                combine_results(selected, composed)
            };
            if result.changed() {
                session_cx.notify();
            }
            Ok::<_, RichTextError>(result)
        });
        self.finish_compound(emitted, outcome, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let projection = self.projection(cx);
        let range = text_edit::range_from_utf16(projection.text(), &range_utf16);
        if range.is_empty() {
            let position = projection.position_for_offset(range.start);
            let layout = self.last_layouts.iter().find(|layout| {
                layout.id == position.block
                    && self
                        .session
                        .read(cx)
                        .document()
                        .block(&layout.id)
                        .is_some_and(|block| layout.source == *block.text())
            })?;
            return Some(layout.layout.caret_bounds_aligned(
                position.offset,
                point(
                    bounds.left() + layout.left,
                    bounds.top() + layout.top - self.scroll_offset,
                ),
                px(cx.theme().measures.caret_width),
                layout.align,
                layout.width,
            ));
        }
        let mut found: Option<Bounds<Pixels>> = None;
        for layout in &self.last_layouts {
            let projected = projection.block(&layout.id)?;
            let start = range.start.max(projected.start);
            let end = range.end.min(projected.end);
            if start >= end {
                continue;
            }
            let block_bounds = layout.layout.enclosing_bounds_for_range(
                start - projected.start..end - projected.start,
                point(
                    bounds.left() + layout.left,
                    bounds.top() + layout.top - self.scroll_offset,
                ),
                layout.align,
                layout.width,
            );
            found = Some(found.map_or(block_bounds, |current| current.union(&block_bounds)));
        }
        found
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let projection = self.projection(cx);
        let position = self.position_for_point(position, cx);
        Some(text_edit::offset_to_utf16(
            projection.text(),
            projection.offset_for_position(&position)?,
        ))
    }
}

impl Render for RichTextEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.disabled && self.focus_handle.is_focused(window) {
            window.blur();
        }
        let theme = cx.theme().clone();
        let focused = self.focus_handle.is_focused(window);
        let spec = self.semantics(cx);
        let (projection, anchor, head) = self.flat_selection(cx);
        let content = projection.text().clone();
        let accessible_rows = self.accessible_rows(cx);
        let selection_representable = text_edit::accessible_text_is_representable(&content);
        let accessible_snapshot = self.accessible_snapshot.clone();
        let accessible_geometry = self.accessible_geometry.clone();
        let accessibility_revision = self.accessibility_revision;
        let accessible_direction = if cx.layout_direction().is_rtl() {
            gpui::accesskit::TextDirection::RightToLeft
        } else {
            gpui::accesskit::TextDirection::LeftToRight
        };
        let entity = cx.entity().clone();
        let can_edit = !self.disabled && !self.read_only;
        let list_active = self
            .session
            .read(cx)
            .document()
            .block(&self.session.read(cx).selection().head.block)
            .and_then(|block| block.paragraph().list())
            .is_some();
        let field = div()
            .id(self.ident.element_id())
            .key_context(KEY_CONTEXT)
            .when(!self.disabled, |element| {
                element.track_focus(&self.focus_handle)
            })
            .when(!self.disabled, |element| {
                element
                    .on_action(cx.listener(Self::left))
                    .on_action(cx.listener(Self::right))
                    .on_action(cx.listener(Self::up))
                    .on_action(cx.listener(Self::down))
                    .on_action(cx.listener(Self::word_left))
                    .on_action(cx.listener(Self::word_right))
                    .on_action(cx.listener(Self::select_left))
                    .on_action(cx.listener(Self::select_right))
                    .on_action(cx.listener(Self::select_up))
                    .on_action(cx.listener(Self::select_down))
                    .on_action(cx.listener(Self::select_word_left))
                    .on_action(cx.listener(Self::select_word_right))
                    .on_action(cx.listener(Self::line_start))
                    .on_action(cx.listener(Self::line_end))
                    .on_action(cx.listener(Self::select_to_line_start))
                    .on_action(cx.listener(Self::select_to_line_end))
                    .on_action(cx.listener(Self::document_start))
                    .on_action(cx.listener(Self::document_end))
                    .on_action(cx.listener(Self::select_to_document_start))
                    .on_action(cx.listener(Self::select_to_document_end))
                    .on_action(cx.listener(Self::select_all))
                    .on_action(cx.listener(Self::copy))
            })
            .when(can_edit, |element| {
                element
                    .on_action(cx.listener(Self::backspace))
                    .on_action(cx.listener(Self::delete))
                    .on_action(cx.listener(Self::delete_word_left))
                    .on_action(cx.listener(Self::delete_word_right))
                    .on_action(cx.listener(Self::hard_break))
                    .on_action(cx.listener(Self::soft_break))
                    .when(self.session.read(cx).can_undo(), |element| {
                        element.on_action(cx.listener(Self::undo))
                    })
                    .when(self.session.read(cx).can_redo(), |element| {
                        element.on_action(cx.listener(Self::redo))
                    })
                    .on_action(cx.listener(Self::cut))
                    .on_action(cx.listener(Self::paste))
                    .on_action(cx.listener(Self::toggle_bold))
                    .on_action(cx.listener(Self::toggle_italic))
                    .on_action(cx.listener(Self::toggle_underline))
                    .on_action(cx.listener(Self::toggle_strike))
                    .on_action(cx.listener(Self::toggle_code))
                    .when(list_active, |element| {
                        element
                            .on_action(cx.listener(Self::indent))
                            .on_action(cx.listener(Self::outdent))
                    })
                    .on_action(cx.listener(Self::show_character_palette))
            })
            .when(!self.disabled, |element| {
                element
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .cursor(CursorStyle::IBeam)
            })
            .when_some(accessible_rows.clone(), move |element, rows| {
                element.a11y_synthetic_children(move |builder| {
                    let geometry = accessible_geometry
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .take();
                    let snapshot = text_edit::publish_accessible_text(
                        builder,
                        &content,
                        anchor,
                        head,
                        accessible_direction,
                        &rows,
                        accessibility_revision,
                        geometry.as_ref(),
                    );
                    *accessible_snapshot
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()) = snapshot;
                })
            })
            .when(
                !self.disabled && selection_representable && accessible_rows.is_some(),
                |element| {
                    let selection_entity = entity.clone();
                    let selection_snapshot = self.accessible_snapshot.clone();
                    element.on_a11y_action(
                        AccessibleAction::SetTextSelection,
                        move |data, _, cx| {
                            let Some(ActionData::SetTextSelection(selection)) = data else {
                                return;
                            };
                            let published = selection_snapshot
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner())
                                .clone();
                            selection_entity.update(cx, |editor, cx| {
                                let projection = editor.projection(cx);
                                let Some(published) = published.as_ref() else {
                                    return;
                                };
                                let Some(anchor) = text_edit::byte_offset_for_published_position(
                                    projection.text(),
                                    editor.accessibility_revision,
                                    published,
                                    selection.anchor,
                                ) else {
                                    return;
                                };
                                let Some(head) = text_edit::byte_offset_for_published_position(
                                    projection.text(),
                                    editor.accessibility_revision,
                                    published,
                                    selection.focus,
                                ) else {
                                    return;
                                };
                                editor.apply_flat_selection(anchor, head, cx);
                            });
                        },
                    )
                },
            )
            .when(can_edit, |element| {
                let entity = entity.clone();
                element.on_a11y_action(AccessibleAction::SetValue, move |data, _, cx| {
                    let Some(ActionData::Value(value)) = data else {
                        return;
                    };
                    entity.update(cx, |editor, cx| {
                        let end = editor.projection(cx).len();
                        editor.replace_plain_text(0..end, value, RichTextInputKind::Paste, cx);
                    });
                })
            })
            .w_full()
            .column()
            .when(self.frame == Frame::Own, |element| {
                element
                    .px(px(theme.spacing.sm))
                    .py(px(theme.spacing.xs))
                    .radius(&theme, Radius::Control)
                    .well(&theme)
                    .when(self.invalid, |element| {
                        element
                            .bg(theme.surface(gpui_kit_theme::Surface::Sunken).blend(
                                theme.color_wash(
                                    theme.colors.danger,
                                    gpui_kit_theme::SemanticWash::Faint,
                                ),
                            ))
                            .glow(&theme, theme.colors.danger)
                    })
                    .when(focused && !self.invalid, |element| {
                        element.shadow(
                            theme.focus_ring_on(theme.surface(gpui_kit_theme::Surface::Sunken)),
                        )
                    })
            })
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .text_color(if self.disabled {
                theme.colors.text_disabled
            } else {
                theme.colors.text
            })
            .child(RichTextEditorElement::new(cx.entity()))
            .semantic_in(cx, spec);

        div()
            .column()
            .w_full()
            .when(self.toolbar, |element| {
                element.child(self.formatting_toolbar(cx))
            })
            .child(field)
    }
}

fn combine_results(left: RichTextEditResult, right: RichTextEditResult) -> RichTextEditResult {
    RichTextEditResult {
        document_changed: left.document_changed || right.document_changed,
        selection_changed: left.selection_changed || right.selection_changed,
        pending_style_changed: left.pending_style_changed || right.pending_style_changed,
    }
}

fn distance_to_vertical_span(value: Pixels, start: Pixels, end: Pixels) -> f32 {
    if value < start {
        f32::from(start - value)
    } else if value > end {
        f32::from(value - end)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_results_preserve_every_changed_dimension() {
        let result = combine_results(
            RichTextEditResult {
                selection_changed: true,
                ..Default::default()
            },
            RichTextEditResult {
                document_changed: true,
                pending_style_changed: true,
                ..Default::default()
            },
        );
        assert!(result.document_changed);
        assert!(result.selection_changed);
        assert!(result.pending_style_changed);
    }
}
