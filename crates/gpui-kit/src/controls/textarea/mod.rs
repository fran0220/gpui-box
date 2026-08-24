//! A multi-line editable text control.
//!
//! `TextArea` is a view rather than a `RenderOnce` builder, for the same
//! reason [`crate::controls::input::TextInput`] is: the caret, the selection,
//! the in-progress input method composition, and the vertical scroll position
//! all outlive a frame. Text wraps at the width the control was given, so
//! there is no horizontal scrolling, and the caret moves by visual line.
//!
//! ```no_run
//! # use gpui::{App, AppContext as _, Context, Window};
//! # use gpui_kit::controls::textarea::{TextArea, TextAreaEvent};
//! # struct Host;
//! # fn example(window: &mut Window, cx: &mut Context<Host>) {
//! let notes = cx.new(|cx| {
//!     TextArea::new("review.notes", window, cx)
//!         .placeholder("What changed, and why")
//!         .rows(4)
//!         .max_rows(12)
//! });
//! cx.subscribe(&notes, |_host, notes, event, cx| {
//!     if let TextAreaEvent::Submit = event {
//!         let _typed = notes.read(cx).value().to_string();
//!     }
//! })
//! .detach();
//! # }
//! ```

mod element;

use std::ops::Range;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gpui::{
    AccessibleAction, App, Bounds, ClipboardItem, Context, CursorStyle, EditableTextLayout,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Point, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, UTF16Selection,
    Window, accesskit::ActionData, actions, div, point, prelude::FluentBuilder as _, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, TypeScale};

use crate::controls::text_edit;
use crate::foundation::{
    ActiveDirection, DirectionalExt, Disableable, Ident, Sizable, StyledExt, text,
};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};
use element::TextAreaElement;

actions!(
    gpui_kit_textarea,
    [
        Backspace,
        Delete,
        DeleteToLineStart,
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
        SelectToLineStart,
        SelectToLineEnd,
        SelectToDocumentStart,
        SelectToDocumentEnd,
        SelectAll,
        LineStart,
        LineEnd,
        DocumentStart,
        DocumentEnd,
        Newline,
        Undo,
        Redo,
        Copy,
        Cut,
        Paste,
        Submit,
        Cancel,
        ShowCharacterPalette,
    ]
);

/// The key context every text area publishes, so a host can layer its own
/// bindings on top without re-declaring these.
pub const KEY_CONTEXT: &str = "TextArea";

/// The second identifier an area publishes when enter submits it.
///
/// What enter means is per-area policy and a keymap is global, so the policy
/// is carried in the context the area declares and the two enter bindings are
/// written against it. That is the only way round it: GPUI dispatches a bound
/// key before any raw handler sees it, so an area cannot decide in its own
/// handler which of the two it just got.
const SUBMIT_CONTEXT: &str = "TextAreaSubmits";

/// The whole context such an area declares: the shared one, so every other
/// binding still reaches it, plus the marker.
const SUBMIT_KEY_CONTEXT: &str = "TextArea TextAreaSubmits";

/// The visible rows a text area occupies when the caller asks for none.
const DEFAULT_ROWS: usize = 3;

/// Installs the editing key bindings.
///
/// Called by [`crate::install`]. Bindings are scoped to the text area key
/// context, so they never shadow a host's global shortcuts.
struct TextAreaBindings;

impl gpui::Global for TextAreaBindings {}

pub(crate) fn install(cx: &mut App) {
    if cx.has_global::<TextAreaBindings>() {
        return;
    }
    cx.set_global(TextAreaBindings);
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
        // What enter does is the area's policy. In the default one it belongs
        // to the text and a submission is the modified chord; in the other the
        // two swap, and shift-enter opens a line.
        KeyBinding::new(
            "enter",
            Newline,
            Some(&format!("{KEY_CONTEXT} && !{SUBMIT_CONTEXT}")),
        ),
        KeyBinding::new(
            "enter",
            Submit,
            Some(&format!("{KEY_CONTEXT} && {SUBMIT_CONTEXT}")),
        ),
        // Bound in both, because a modified enter means the same thing in
        // both: the one that is not the common act.
        KeyBinding::new("shift-enter", Newline, Some(KEY_CONTEXT)),
        KeyBinding::new(&format!("{primary}-enter"), Submit, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", Cancel, Some(KEY_CONTEXT)),
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
            KeyBinding::new("cmd-backspace", DeleteToLineStart, Some(KEY_CONTEXT)),
            KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some(KEY_CONTEXT)),
        ]);
    }

    cx.bind_keys(bindings);
}

/// What the enter key does.
///
/// Both are ordinary and neither is a preference: it depends on what the text
/// is. A field in a form holds a value that is edited and then committed, and
/// enter is part of editing it. A composer holds a message, where sending is
/// the common act and a second line is the exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Enter {
    /// Enter opens a line; the platform modifier plus enter submits.
    #[default]
    Opens,
    /// Enter submits; shift plus enter opens a line.
    Submits,
}

/// What a paste carried, when it was not text.
///
/// An area cannot put an image or a file into a string, so it says what
/// arrived and stops there. Whether this text is a message that takes
/// attachments, or a field that has no use for one, is the host's to know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pasted {
    /// Image data, such as a screenshot or a copied picture.
    Images(Vec<gpui::Image>),
    /// Paths, from a file manager's copy.
    Paths(Vec<PathBuf>),
}

/// Who draws the frame the text sits in.
///
/// A field standing in a form is a control, and the area draws the whole of
/// it. An area inside a composer's pill, or inside a row a settings page
/// already framed, is not: drawing a second well inside the first is two
/// surfaces where the reader sees one control, and the frame the host drew is
/// the one they will aim at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Frame {
    /// The area draws its own well, radius, padding and focus ring.
    #[default]
    Own,
    /// The host drew the frame. The area contributes text, caret, selection
    /// and every editing behaviour, and inherits the type it is placed in, so
    /// the frame around it can be any shape the host wants — including one
    /// that changes shape from what [`Measured`] reports.
    Host,
}

/// What a text area measured the last time it was laid out.
///
/// An area grows itself between `rows` and `max_rows`, which is the whole
/// answer for a field that stands in a column. It is not the answer for a
/// frame that changes shape around the text — a one-line pill that becomes a
/// panel when the message outgrows it — because that host has to decide
/// before it lays the area out, and at a width the area is not currently in.
/// So the area publishes what it knows rather than the decision: how wide the
/// text wants to be, how tall it came out, and the frame it was measured in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measured {
    /// The widest line's width with nothing wrapping it, which is what a
    /// narrower frame would have to hold to keep the text on one row.
    pub text: Pixels,
    /// The height of the wrapped text, before the frame clamps it.
    pub height: Pixels,
    /// The width the text was wrapped against, which is the area's own frame
    /// less the padding the control keeps around it.
    pub wrapped: Pixels,
    /// Which layout pass this came from. A host that changes the frame should
    /// wait for a pass later than the one it acted on, or it will read the old
    /// shape and change its mind twice.
    pub pass: u64,
}

/// What a text area reports to its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextAreaEvent {
    /// The text changed, by typing, deletion, paste, or a programmatic set.
    Change(SharedString),
    /// The submit chord was pressed while the area had focus.
    Submit,
    /// Editing was abandoned with the cancel key.
    Cancel,
    /// A paste carried something the text cannot hold.
    Pasted(Pasted),
    /// Up, while the arrows belong to something else. The caret did not move.
    MoveUp,
    /// Down, on the same terms.
    MoveDown,
    Focus,
    Blur,
}

impl EventEmitter<TextAreaEvent> for TextArea {}

/// Wrapped, multi-line editable text.
///
/// Enter inserts a line and the platform modifier plus enter submits. Motion
/// follows visual rows with a preserved goal column, and the frame grows from
/// `rows` to `max_rows` before it scrolls rather than pushing the page around.
pub struct TextArea {
    ident: Ident,
    focus_handle: FocusHandle,
    placeholder: SharedString,
    /// The value, the caret, the composition in flight, and the transactions
    /// that got here. Every mutation goes through it, so undo describes the
    /// value the area is actually showing.
    edit: text_edit::EditBuffer,
    size: ControlSize,
    disabled: bool,
    invalid: bool,
    required: bool,
    read_only: bool,
    max_length: Option<usize>,
    rows: usize,
    max_rows: Option<usize>,
    enter: Enter,
    frame: Frame,
    /// Whether the vertical arrows belong to something other than the caret.
    /// Set from the host's render while a surface over the area is listing
    /// options, because that is the only thing that knows there is one.
    arrows_claimed: bool,
    /// The rows the frame decided to occupy, which grows with the text until
    /// `max_rows` and is measured rather than guessed.
    visible_rows: usize,
    scroll_offset: Pixels,
    /// The horizontal position vertical motion aims for, so a run of up or
    /// down keys through a short line does not drag the caret leftwards.
    goal_x: Option<Pixels>,
    is_selecting: bool,
    last_layout: Option<EditableTextLayout>,
    last_layout_text: SharedString,
    last_bounds: Option<Bounds<Pixels>>,
    /// Bumped once per layout pass, so a host that resizes the frame around
    /// this area can tell a measurement taken after its last change from one
    /// taken before it.
    layout_pass: u64,
    accessibility_revision: u64,
    accessible_snapshot: Arc<Mutex<Option<text_edit::PublishedAccessibleText>>>,
    accessible_geometry: Arc<Mutex<Option<text_edit::AccessibleTextGeometry>>>,
    /// Held so the focus listeners live as long as the area does.
    _subscriptions: Vec<Subscription>,
}

impl TextArea {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut area = Self::detached(ident, cx);
        area.watch_focus(window, cx);
        area
    }

    /// Builds an area where there is no window to hand.
    ///
    /// Focus belongs to a window, so an area normally takes one and starts
    /// watching immediately. A host does not always have one: a view built
    /// inside a subscription, a background task, or a test that never opened
    /// a window has a `Context` and nothing else. Such an area starts
    /// watching at its first render, which is the first moment a window
    /// certainly exists, and reports focus from then on.
    pub fn detached(ident: impl Into<Ident>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let subscriptions = Vec::new();
        Self {
            ident: ident.into(),
            focus_handle,
            placeholder: SharedString::default(),
            edit: text_edit::EditBuffer::default(),
            size: ControlSize::Md,
            disabled: false,
            invalid: false,
            required: false,
            read_only: false,
            max_length: None,
            rows: DEFAULT_ROWS,
            max_rows: None,
            enter: Enter::Opens,
            frame: Frame::Own,
            arrows_claimed: false,
            visible_rows: DEFAULT_ROWS,
            scroll_offset: px(0.0),
            goal_x: None,
            is_selecting: false,
            last_layout: None,
            last_layout_text: SharedString::default(),
            last_bounds: None,
            layout_pass: 0,
            accessibility_revision: 0,
            accessible_snapshot: Arc::default(),
            accessible_geometry: Arc::default(),
            _subscriptions: subscriptions,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Starts reporting focus and blur. Idempotent: an area that is already
    /// watching does not subscribe twice, so a render may call it freely.
    fn watch_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self._subscriptions.is_empty() {
            return;
        }
        let focus_handle = self.focus_handle.clone();
        self._subscriptions = vec![
            cx.on_focus(&focus_handle, window, |_, _, cx| {
                cx.emit(TextAreaEvent::Focus)
            }),
            cx.on_blur(&focus_handle, window, |_, _, cx| {
                cx.emit(TextAreaEvent::Blur)
            }),
        ];
    }

    /// Changes what the empty area suggests, after it was built.
    ///
    /// What an empty area is waiting for can change without the area being
    /// rebuilt — the language it is read in, or a question the host is part
    /// way through asking — and rebuilding it to say so would throw away
    /// whatever had been typed.
    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    /// Who draws the frame around the text. See [`Frame`].
    pub fn frame(mut self, frame: Frame) -> Self {
        self.frame = frame;
        self
    }

    /// Seeds the initial text, with the caret at the end.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.edit.set_text(&text.into());
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

    /// Keeps the value focusable and exposed while refusing keyboard,
    /// pointer, IME, and accessibility value changes.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// The rows the area occupies before it has anything longer to show.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self.visible_rows = self.rows;
        self
    }

    /// Grows with the text up to this many rows, then scrolls instead.
    pub fn max_rows(mut self, max_rows: usize) -> Self {
        self.max_rows = Some(max_rows.max(1));
        self
    }

    /// Truncates input past a length in bytes of UTF-8.
    /// What the enter key does. See [`Enter`].
    pub fn enter(mut self, enter: Enter) -> Self {
        self.enter = enter;
        self
    }

    /// Hands the vertical arrows to the host, or takes them back.
    ///
    /// While they are claimed, up and down report [`TextAreaEvent::MoveUp`]
    /// and [`TextAreaEvent::MoveDown`] and the caret does not move. A menu
    /// drawn over the area cannot take them for itself: GPUI dispatches a
    /// bound key before any raw listener, so the area has to hand them over,
    /// and it only does so while the host says there is something up there to
    /// move through.
    ///
    /// There is no notify, because this is set from the host's own render and
    /// asking for a frame from inside one would spin.
    pub fn set_arrows_claimed(&mut self, claimed: bool) {
        self.arrows_claimed = claimed;
    }

    pub fn arrows_claimed(&self) -> bool {
        self.arrows_claimed
    }

    pub fn max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self.edit.rules_mut().max_length = Some(max_length);
        self
    }

    pub fn value(&self) -> &SharedString {
        self.edit.text()
    }

    pub fn is_empty(&self) -> bool {
        self.edit.is_empty()
    }

    /// Replaces the text from the host side, for example when a form resets.
    pub fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        // A value the host set is not a step the reader can walk back
        // through, so it ends the history rather than joining it.
        self.edit.set_text(&value.into());
        self.accessibility_revision = self.accessibility_revision.wrapping_add(1);
        self.scroll_offset = px(0.0);
        self.goal_x = None;
        cx.emit(TextAreaEvent::Change(self.edit.text().clone()));
        cx.notify();
    }

    /// Inserts text at the caret, replacing the selection, exactly as a paste
    /// would.
    ///
    /// This is what a drop onto the area is: text that arrived from outside
    /// with no keystroke behind it. It goes through the same edit as
    /// everything else, so it is one undo step and it reports one change.
    pub fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        self.apply_edit(None, text, text_edit::Cause::Paste, cx);
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.edit.set_marked(None);
            self.is_selecting = false;
            self.goal_x = None;
        }
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        cx.notify();
    }

    pub fn set_invalid(&mut self, invalid: bool, cx: &mut Context<Self>) {
        self.invalid = invalid;
        cx.notify();
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn selected_range(&self) -> Range<usize> {
        self.edit.selection()
    }

    pub fn cursor_offset(&self) -> usize {
        if self.edit.is_reversed() {
            self.edit.selection().start
        } else {
            self.edit.selection().end
        }
    }

    /// The visual row the caret sits on, counting wrapped rows.
    ///
    /// Zero until the area has been laid out once, because a wrapped row only
    /// exists once a width is known.
    pub fn cursor_row(&self) -> usize {
        self.last_layout
            .as_ref()
            .map(|layout| layout.row_for_offset(self.cursor_offset()))
            .unwrap_or(0)
    }

    /// What the last layout pass measured, or nothing before the first one.
    pub fn measured(&self) -> Option<Measured> {
        let layout = self.last_layout.as_ref()?;
        let bounds = self.last_bounds?;
        Some(Measured {
            text: layout.text_width(),
            height: layout.height(),
            wrapped: bounds.size.width,
            pass: self.layout_pass,
        })
    }

    pub(crate) fn placeholder_text(&self) -> &SharedString {
        &self.placeholder
    }

    pub(crate) fn marked_range(&self) -> Option<Range<usize>> {
        self.edit.marked()
    }

    pub(crate) fn scroll_offset(&self) -> Pixels {
        self.scroll_offset
    }

    pub(crate) fn row_limits(&self) -> (usize, usize) {
        (self.rows, self.max_rows.unwrap_or(self.rows).max(self.rows))
    }

    pub(crate) fn visible_rows(&self) -> usize {
        self.visible_rows
    }

    pub(crate) fn set_visible_rows(&mut self, rows: usize) {
        self.visible_rows = rows;
    }

    pub(crate) fn set_scroll_offset(&mut self, offset: Pixels) {
        self.scroll_offset = offset;
    }

    pub(crate) fn set_last_layout(
        &mut self,
        layout: EditableTextLayout,
        text: SharedString,
        bounds: Bounds<Pixels>,
    ) -> bool {
        let rows = layout.visual_rows(&text);
        let changed = self.last_layout_text != text
            || self
                .last_layout
                .as_ref()
                .map(|layout| layout.visual_rows(&text))
                != Some(rows);
        self.last_layout = Some(layout);
        self.last_layout_text = text;
        self.last_bounds = Some(bounds);
        self.layout_pass = self.layout_pass.wrapping_add(1);
        changed
    }

    fn accessible_rows(&self) -> Option<Vec<Range<usize>>> {
        (self.last_layout_text == *self.edit.text()).then(|| {
            self.last_layout
                .as_ref()
                .map(|layout| layout.visual_rows(self.edit.text()))
                .unwrap_or_else(|| std::iter::once(0..0).collect())
        })
    }

    /// The range an edit covers when the caller did not name one: whatever an
    /// input method is composing, or the selection.
    fn edit_range(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or_else(|| self.edit.marked())
            .unwrap_or_else(|| self.edit.selection())
    }

    /// The one place this area's text changes.
    ///
    /// `cause` is what the reader did, which decides whether the edit joins
    /// the step before it and whether it is remembered at all.
    fn apply_edit(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        cause: text_edit::Cause,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.read_only {
            return;
        }
        let range = self.edit_range(range_utf16);
        // A key that arrives while an input method is composing ends the
        // composition, so the run is one step rather than merging with what
        // follows it.
        self.edit.end_composition();
        let outcome = self.edit.replace(range, new_text, cause);
        self.goal_x = None;
        if outcome.changed {
            self.accessibility_revision = self.accessibility_revision.wrapping_add(1);
            cx.emit(TextAreaEvent::Change(self.edit.text().clone()));
        }
        cx.notify();
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.read_only || !self.edit.undo() {
            return;
        }
        self.accessibility_revision = self.accessibility_revision.wrapping_add(1);
        self.goal_x = None;
        cx.emit(TextAreaEvent::Change(self.edit.text().clone()));
        cx.notify();
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.read_only || !self.edit.redo() {
            return;
        }
        self.accessibility_revision = self.accessibility_revision.wrapping_add(1);
        self.goal_x = None;
        cx.emit(TextAreaEvent::Change(self.edit.text().clone()));
        cx.notify();
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.edit.set_caret(offset);
        self.goal_x = None;
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.edit.extend_selection(offset);
        self.goal_x = None;
        cx.notify();
    }

    fn select_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        self.edit.set_selection(range, false);
        self.goal_x = None;
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        text_edit::previous_boundary(self.edit.text(), offset)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        text_edit::next_boundary(self.edit.text(), offset)
    }

    fn previous_word_boundary(&self, offset: usize) -> usize {
        text_edit::previous_word_boundary(self.edit.text(), offset)
    }

    fn next_word_boundary(&self, offset: usize) -> usize {
        text_edit::next_word_boundary(self.edit.text(), offset)
    }

    pub(crate) fn index_for_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(layout)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        let local = point(
            position.x - bounds.left(),
            position.y - bounds.top() + self.scroll_offset,
        );
        layout
            .offset_for_position(local)
            .min(self.edit.text().len())
    }

    /// Moves the caret by whole visual rows, keeping the column it aimed for.
    fn move_by_row(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let Some(layout) = self.last_layout.as_ref() else {
            return;
        };
        let caret = self.cursor_offset();
        let position = layout.position_for_offset(caret);
        let goal = self.goal_x.unwrap_or(position.x);
        let row = layout.row_for_offset(caret) as isize + delta;
        let offset = if row < 0 {
            0
        } else if row as usize >= layout.total_rows() {
            self.edit.text().len()
        } else {
            layout
                .offset_at_row(row as usize, goal)
                .min(self.edit.text().len())
        };
        if extend {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
        self.goal_x = Some(goal);
    }

    /// The bounds of the visual row the caret sits on, in content offsets.
    fn caret_row_range(&self) -> Range<usize> {
        let Some(layout) = self.last_layout.as_ref() else {
            return 0..self.edit.text().len();
        };
        let row = layout.row_for_offset(self.cursor_offset());
        let range = layout.row_range(row);
        range.start.min(self.edit.text().len())..range.end.min(self.edit.text().len())
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.edit.selection().is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.edit.selection().start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.edit.selection().is_empty() {
            self.move_to(self.next_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.edit.selection().end, cx);
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if self.arrows_claimed {
            cx.emit(TextAreaEvent::MoveUp);
            return;
        }
        self.move_by_row(-1, false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if self.arrows_claimed {
            cx.emit(TextAreaEvent::MoveDown);
            return;
        }
        self.move_by_row(1, false, cx);
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.previous_word_boundary(self.cursor_offset()), cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.next_word_boundary(self.cursor_offset()), cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_by_row(-1, true, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_by_row(1, true, cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_word_boundary(self.cursor_offset()), cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_word_boundary(self.cursor_offset()), cx);
    }

    fn select_to_line_start(
        &mut self,
        _: &SelectToLineStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.caret_row_range().start, cx);
    }

    fn select_to_line_end(&mut self, _: &SelectToLineEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.caret_row_range().end, cx);
    }

    fn select_to_document_start(
        &mut self,
        _: &SelectToDocumentStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(0, cx);
    }

    fn select_to_document_end(
        &mut self,
        _: &SelectToDocumentEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.edit.text().len(), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_range(0..self.edit.text().len(), cx);
    }

    fn line_start(&mut self, _: &LineStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.caret_row_range().start, cx);
    }

    fn line_end(&mut self, _: &LineEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.caret_row_range().end, cx);
    }

    fn document_start(&mut self, _: &DocumentStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn document_end(&mut self, _: &DocumentEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.edit.text().len(), cx);
    }

    fn newline(&mut self, _: &Newline, _window: &mut Window, cx: &mut Context<Self>) {
        self.apply_edit(None, "\n", text_edit::Cause::Typing, cx);
    }

    fn backspace(&mut self, _: &Backspace, _window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.selection().is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx);
        }
        self.apply_edit(None, "", text_edit::Cause::Deleting, cx);
    }

    fn delete(&mut self, _: &Delete, _window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.selection().is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx);
        }
        self.apply_edit(None, "", text_edit::Cause::Deleting, cx);
    }

    fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edit.selection().is_empty() {
            self.select_to(self.previous_word_boundary(self.cursor_offset()), cx);
        }
        self.apply_edit(None, "", text_edit::Cause::Deleting, cx);
    }

    fn delete_word_right(
        &mut self,
        _: &DeleteWordRight,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edit.selection().is_empty() {
            self.select_to(self.next_word_boundary(self.cursor_offset()), cx);
        }
        self.apply_edit(None, "", text_edit::Cause::Deleting, cx);
    }

    fn delete_to_line_start(
        &mut self,
        _: &DeleteToLineStart,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edit.selection().is_empty() {
            self.select_to(self.caret_row_range().start, cx);
        }
        self.apply_edit(None, "", text_edit::Cause::Deleting, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if self.edit.selection().is_empty() {
            return;
        }
        let selected = self.edit.text()[self.edit.selection()].to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(selected));
    }

    fn cut(&mut self, _: &Cut, _window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.selection().is_empty() {
            return;
        }
        let selected = self.edit.text()[self.edit.selection()].to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(selected));
        self.apply_edit(None, "", text_edit::Cause::Cut, cx);
    }

    fn paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        if let Some(text) = item.text() {
            // Line breaks survive a paste here, but only in one shape, so the
            // stored text never depends on where it was copied from.
            let text = text.replace("\r\n", "\n").replace('\r', "\n");
            self.apply_edit(None, &text, text_edit::Cause::Paste, cx);
            return;
        }
        // Not text. The area reports what arrived rather than dropping it
        // silently or writing a path into the message as if somebody had
        // typed one.
        if let Some(pasted) = non_text(&item) {
            cx.emit(TextAreaEvent::Pasted(pasted));
        }
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextAreaEvent::Submit);
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextAreaEvent::Cancel);
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
        let offset = self.index_for_position(event.position);
        if event.modifiers.shift {
            self.select_to(offset, cx);
        } else if event.click_count >= 3 {
            self.select_range(text_edit::paragraph_at(self.edit.text(), offset), cx);
        } else if event.click_count == 2 {
            self.select_range(text_edit::word_at(self.edit.text(), offset), cx);
        } else {
            self.move_to(offset, cx);
        }
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_position(event.position), cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        text_edit::offset_to_utf16(self.edit.text(), offset)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        text_edit::range_to_utf16(self.edit.text(), range)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        text_edit::range_from_utf16(self.edit.text(), range_utf16)
    }

    fn semantics(&self) -> NodeSpec {
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
        if !self.edit.is_empty() {
            spec = spec.value(self.edit.text().clone());
        }
        spec
    }
}

impl std::fmt::Debug for TextArea {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The content is deliberately absent: an area holds whatever a person
        // wrote, and a debug log is not a place for it.
        formatter
            .debug_struct("TextArea")
            .field("id", &self.ident)
            .field("size", &self.size)
            .field("disabled", &self.disabled)
            .field("invalid", &self.invalid)
            .field("rows", &self.rows)
            .field("length", &self.edit.text().len())
            .finish()
    }
}

impl Disableable for TextArea {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for TextArea {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for TextArea {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for TextArea {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.edit.text().get(range)?.to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.edit.selection()),
            reversed: self.edit.is_reversed(),
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.edit.marked().map(|range| self.range_to_utf16(&range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.edit.end_composition();
        self.edit.set_marked(None);
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Text arriving through the input handler is text the reader entered,
        // whether from a key or from an input method that just committed.
        self.apply_edit(range_utf16, new_text, text_edit::Cause::Typing, cx);
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
        let range = self.edit_range(range_utf16);
        // The composing selection is reported relative to the replacement,
        // not to the whole value, so it is converted against exactly that
        // replacement. Converting against the already-mutated value can land
        // inside an astral scalar.
        let normalised = text_edit::normalize_multiline(new_text);
        let inside = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| text_edit::range_from_utf16(&normalised, range_utf16));
        let outcome = self.edit.replace_and_mark(range, new_text, inside);
        self.goal_x = None;
        if outcome.changed {
            self.accessibility_revision = self.accessibility_revision.wrapping_add(1);
            cx.emit(TextAreaEvent::Change(self.edit.text().clone()));
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(layout.enclosing_bounds_for_range(
            range,
            point(bounds.left(), bounds.top() - self.scroll_offset),
            gpui::TextAlign::Left,
            bounds.size.width,
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let offset = self.index_for_position(point);
        Some(self.offset_to_utf16(offset))
    }
}

impl Render for TextArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.disabled && self.focus_handle.is_focused(window) {
            window.blur();
        }
        self.watch_focus(window, cx);
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let focused = self.focus_handle.is_focused(window);
        let spec = self.semantics();
        let content = self.edit.text().clone();
        let (anchor, focus) = if self.edit.is_reversed() {
            (self.edit.selection().end, self.edit.selection().start)
        } else {
            (self.edit.selection().start, self.edit.selection().end)
        };
        let accessible_snapshot = self.accessible_snapshot.clone();
        let accessible_geometry = self.accessible_geometry.clone();
        let selection_representable = text_edit::accessible_text_is_representable(&content);
        let accessible_rows = self.accessible_rows();
        let accessibility_revision = self.accessibility_revision;
        let entity = cx.entity().clone();
        let accessible_direction = if cx.layout_direction().is_rtl() {
            gpui::accesskit::TextDirection::RightToLeft
        } else {
            gpui::accesskit::TextDirection::LeftToRight
        };

        let field = div()
            .id(self.ident.element_id())
            // The second identifier is what the two enter bindings are written
            // against, so what enter means travels with the area rather than
            // with the keymap.
            .key_context(match self.enter {
                Enter::Opens => KEY_CONTEXT,
                Enter::Submits => SUBMIT_KEY_CONTEXT,
            })
            .when(!self.disabled, |element| {
                element.track_focus(&self.focus_handle)
            })
            .when(!self.disabled && !self.read_only, |element| {
                element
                    .on_action(cx.listener(Self::backspace))
                    .on_action(cx.listener(Self::delete))
                    .on_action(cx.listener(Self::delete_word_left))
                    .on_action(cx.listener(Self::delete_word_right))
                    .on_action(cx.listener(Self::delete_to_line_start))
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
                    .on_action(cx.listener(Self::select_to_line_start))
                    .on_action(cx.listener(Self::select_to_line_end))
                    .on_action(cx.listener(Self::select_to_document_start))
                    .on_action(cx.listener(Self::select_to_document_end))
                    .on_action(cx.listener(Self::select_all))
                    .on_action(cx.listener(Self::line_start))
                    .on_action(cx.listener(Self::line_end))
                    .on_action(cx.listener(Self::document_start))
                    .on_action(cx.listener(Self::document_end))
                    .on_action(cx.listener(Self::newline))
                    .on_action(cx.listener(Self::copy))
                    .on_action(cx.listener(Self::cut))
                    .on_action(cx.listener(Self::paste))
                    // An action with nothing to act on installs no handler,
                    // so a host binding on the same key is not shadowed by a
                    // listener that would do nothing.
                    .when(self.edit.can_undo(), |element| {
                        element.on_action(cx.listener(Self::undo))
                    })
                    .when(self.edit.can_redo(), |element| {
                        element.on_action(cx.listener(Self::redo))
                    })
                    .on_action(cx.listener(Self::submit))
                    .on_action(cx.listener(Self::cancel))
                    .on_action(cx.listener(Self::show_character_palette))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .cursor(CursorStyle::IBeam)
            })
            .when_some(accessible_rows.clone(), move |element, accessible_rows| {
                element.a11y_synthetic_children(move |builder| {
                    let geometry = accessible_geometry
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .take();
                    let snapshot = text_edit::publish_accessible_text(
                        builder,
                        &content,
                        anchor,
                        focus,
                        accessible_direction,
                        &accessible_rows,
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
                            selection_entity.update(cx, |area, cx| {
                                if area.disabled {
                                    return;
                                }
                                let Some(published) = published.as_ref() else {
                                    return;
                                };
                                let Some(anchor) = text_edit::byte_offset_for_published_position(
                                    area.edit.text(),
                                    area.accessibility_revision,
                                    published,
                                    selection.anchor,
                                ) else {
                                    return;
                                };
                                let Some(focus) = text_edit::byte_offset_for_published_position(
                                    area.edit.text(),
                                    area.accessibility_revision,
                                    published,
                                    selection.focus,
                                ) else {
                                    return;
                                };
                                area.edit.set_selection(
                                    anchor.min(focus)..anchor.max(focus),
                                    focus < anchor,
                                );
                                area.edit.set_marked(None);
                                area.goal_x = None;
                                cx.notify();
                            });
                        },
                    )
                },
            )
            .when(!self.disabled && !self.read_only, |element| {
                element.on_a11y_action(AccessibleAction::SetValue, move |data, _window, cx| {
                    let Some(ActionData::Value(value)) = data else {
                        return;
                    };
                    entity.update(cx, |area, cx| {
                        if area.disabled || area.read_only {
                            return;
                        }
                        let end =
                            text_edit::offset_to_utf16(area.edit.text(), area.edit.text().len());
                        // A value set through assistive technology replaces
                        // the area wholesale; it is one step, not a run of
                        // typing that the next keystroke could join.
                        area.apply_edit(Some(0..end), value, text_edit::Cause::Programmatic, cx);
                    });
                })
            })
            .w_full()
            .column()
            // In a host's frame the area contributes only the text: a well
            // inside the host's well is two surfaces for one control, and the
            // type belongs to whatever the host put the area in.
            .when(self.frame == Frame::Own, |element| {
                element
                    .px(px(metrics.padding_x))
                    .py(px(theme.spacing.xs))
                    .radius(&theme, Radius::Control)
                    .well(&theme)
                    // The same drawn boundary every field in the library
                    // carries; invalidity recolours it rather than adding one.
                    .border_color(if self.invalid {
                        theme.colors.danger
                    } else {
                        theme.colors.hairline
                    })
                    .when(focused, |element| element.shadow(theme.focus_ring()))
                    .text_size(px(metrics.font_size))
            })
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .text_color(if self.disabled {
                theme.colors.text_disabled
            } else {
                theme.colors.text
            })
            .child(TextAreaElement::new(cx.entity()))
            .semantic_in(cx, spec);
        match self.max_length {
            Some(max) => {
                let used = self.edit.text().chars().count();
                let count = cx.strings().format(
                    StringKey::TextAreaCount,
                    &[
                        cx.numbers().count(used).as_ref(),
                        cx.numbers().count(max).as_ref(),
                    ],
                );
                div()
                    .column()
                    .w_full()
                    .child(field)
                    .child(
                        text(&theme, TypeScale::Caption, count.clone())
                            .mt(px(theme.spacing.xs))
                            // Under the trailing edge of the field it counts,
                            // which is the only edge it has anything to do
                            // with.
                            .w_full()
                            .text_end(cx.layout_direction())
                            .text_color(if used >= max {
                                theme.colors.danger
                            } else {
                                theme.colors.text_faint
                            })
                            .semantic_in(
                                cx,
                                NodeSpec::new(
                                    self.ident.child("count").semantic_id(),
                                    Role::Status,
                                )
                                .parent(self.ident.semantic_id())
                                .text(count.clone())
                                .value(count),
                            ),
                    )
                    .into_any_element()
            }
            None => field.into_any_element(),
        }
    }
}

/// The non-text half of a clipboard item, when there is one.
///
/// Every image entry, or every path entry, whichever the item leads with. A
/// clipboard carrying both is one thing described two ways, and reporting it
/// twice would stage it twice.
fn non_text(item: &ClipboardItem) -> Option<Pasted> {
    let images: Vec<gpui::Image> = item
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            gpui::ClipboardEntry::Image(image) => Some(image.clone()),
            _ => None,
        })
        .collect();
    if !images.is_empty() {
        return Some(Pasted::Images(images));
    }
    let paths: Vec<PathBuf> = item
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            gpui::ClipboardEntry::ExternalPaths(paths) => Some(paths.paths().to_vec()),
            _ => None,
        })
        .flatten()
        .collect();
    (!paths.is_empty()).then_some(Pasted::Paths(paths))
}
