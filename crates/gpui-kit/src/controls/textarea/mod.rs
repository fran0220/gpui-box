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
pub(crate) mod layout;

use std::ops::Range;
use std::sync::{Arc, Mutex};

use gpui::{
    AccessibleAction, App, Bounds, ClipboardItem, Context, CursorStyle, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, UTF16Selection, Window,
    accesskit::ActionData, actions, div, point, prelude::FluentBuilder as _, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius};

use crate::controls::text_edit;
use crate::foundation::{ActiveDirection, Disableable, Ident, Sizable, StyledExt};
use element::TextAreaElement;
use layout::Layout;

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

/// The visible rows a text area occupies when the caller asks for none.
const DEFAULT_ROWS: usize = 3;

/// Installs the editing key bindings.
///
/// Called by [`crate::install`]. Bindings are scoped to the text area key
/// context, so they never shadow a host's global shortcuts.
pub(crate) fn install(cx: &mut App) {
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
        // Enter belongs to the text here; a submission is the modified chord.
        KeyBinding::new("enter", Newline, Some(KEY_CONTEXT)),
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

/// What a text area reports to its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextAreaEvent {
    /// The text changed, by typing, deletion, paste, or a programmatic set.
    Change(SharedString),
    /// The submit chord was pressed while the area had focus.
    Submit,
    /// Editing was abandoned with the cancel key.
    Cancel,
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
    content: SharedString,
    placeholder: SharedString,
    /// A caret is an empty selection, so one range describes both.
    selected_range: Range<usize>,
    selection_reversed: bool,
    /// The range the input method is currently composing, which is underlined
    /// and replaced wholesale as composition continues.
    marked_range: Option<Range<usize>>,
    size: ControlSize,
    disabled: bool,
    invalid: bool,
    required: bool,
    read_only: bool,
    max_length: Option<usize>,
    rows: usize,
    max_rows: Option<usize>,
    /// The rows the frame decided to occupy, which grows with the text until
    /// `max_rows` and is measured rather than guessed.
    visible_rows: usize,
    scroll_offset: Pixels,
    /// The horizontal position vertical motion aims for, so a run of up or
    /// down keys through a short line does not drag the caret leftwards.
    goal_x: Option<Pixels>,
    is_selecting: bool,
    last_layout: Option<Layout>,
    last_layout_text: SharedString,
    last_bounds: Option<Bounds<Pixels>>,
    accessibility_revision: u64,
    accessible_snapshot: Arc<Mutex<Option<text_edit::PublishedAccessibleText>>>,
    /// Held so the focus listeners live as long as the area does.
    _subscriptions: Vec<Subscription>,
}

impl TextArea {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let subscriptions = vec![
            cx.on_focus(&focus_handle, window, |_, _, cx| {
                cx.emit(TextAreaEvent::Focus)
            }),
            cx.on_blur(&focus_handle, window, |_, _, cx| {
                cx.emit(TextAreaEvent::Blur)
            }),
        ];
        Self {
            ident: ident.into(),
            focus_handle,
            content: SharedString::default(),
            placeholder: SharedString::default(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            size: ControlSize::Md,
            disabled: false,
            invalid: false,
            required: false,
            read_only: false,
            max_length: None,
            rows: DEFAULT_ROWS,
            max_rows: None,
            visible_rows: DEFAULT_ROWS,
            scroll_offset: px(0.0),
            goal_x: None,
            is_selecting: false,
            last_layout: None,
            last_layout_text: SharedString::default(),
            last_bounds: None,
            accessibility_revision: 0,
            accessible_snapshot: Arc::default(),
            _subscriptions: subscriptions,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Seeds the initial text, with the caret at the end.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.content = text.into();
        self.selected_range = self.content.len()..self.content.len();
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
    pub fn max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    pub fn value(&self) -> &SharedString {
        &self.content
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Replaces the text from the host side, for example when a form resets.
    pub fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = value.into();
        self.accessibility_revision = self.accessibility_revision.wrapping_add(1);
        let end = self.content.len();
        self.selected_range = end..end;
        self.marked_range = None;
        self.scroll_offset = px(0.0);
        self.goal_x = None;
        cx.emit(TextAreaEvent::Change(self.content.clone()));
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.marked_range = None;
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
        self.selected_range.clone()
    }

    pub fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
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

    pub(crate) fn placeholder_text(&self) -> &SharedString {
        &self.placeholder
    }

    pub(crate) fn marked_range(&self) -> Option<Range<usize>> {
        self.marked_range.clone()
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
        layout: Layout,
        text: SharedString,
        bounds: Bounds<Pixels>,
    ) -> bool {
        let rows = layout.accessible_rows(&text);
        let changed = self.last_layout_text != text
            || self
                .last_layout
                .as_ref()
                .map(|layout| layout.accessible_rows(&text))
                != Some(rows);
        self.last_layout = Some(layout);
        self.last_layout_text = text;
        self.last_bounds = Some(bounds);
        changed
    }

    fn accessible_rows(&self) -> Option<Vec<Range<usize>>> {
        (self.last_layout_text == self.content).then(|| {
            self.last_layout
                .as_ref()
                .map(|layout| layout.accessible_rows(&self.content))
                .unwrap_or_else(|| std::iter::once(0..0).collect())
        })
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.goal_x = None;
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.goal_x = None;
        cx.notify();
    }

    fn select_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        self.selected_range = range;
        self.selection_reversed = false;
        self.goal_x = None;
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        text_edit::previous_boundary(&self.content, offset)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        text_edit::next_boundary(&self.content, offset)
    }

    fn previous_word_boundary(&self, offset: usize) -> usize {
        text_edit::previous_word_boundary(&self.content, offset)
    }

    fn next_word_boundary(&self, offset: usize) -> usize {
        text_edit::next_word_boundary(&self.content, offset)
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
        layout.offset_for_position(local).min(self.content.len())
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
            self.content.len()
        } else {
            layout
                .offset_at_row(row as usize, goal)
                .min(self.content.len())
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
            return 0..self.content.len();
        };
        let row = layout.row_for_offset(self.cursor_offset());
        let range = layout.row_range(row);
        range.start.min(self.content.len())..range.end.min(self.content.len())
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_by_row(-1, false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
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
        self.select_to(self.content.len(), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_range(0..self.content.len(), cx);
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
        self.move_to(self.content.len(), cx);
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(None, "\n", window, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_word_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_word_right(
        &mut self,
        _: &DeleteWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_word_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_to_line_start(
        &mut self,
        _: &DeleteToLineStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            self.select_to(self.caret_row_range().start, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            return;
        }
        let selected = self.content[self.selected_range.clone()].to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(selected));
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            return;
        }
        let selected = self.content[self.selected_range.clone()].to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(selected));
        self.replace_text_in_range(None, "", window, cx);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        // Line breaks survive a paste here, but only in one shape, so the
        // stored text never depends on where it was copied from.
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        self.replace_text_in_range(None, &text, window, cx);
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
            self.select_range(text_edit::paragraph_at(&self.content, offset), cx);
        } else if event.click_count == 2 {
            self.select_range(text_edit::word_at(&self.content, offset), cx);
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
        text_edit::offset_to_utf16(&self.content, offset)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        text_edit::range_to_utf16(&self.content, range)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        text_edit::range_from_utf16(&self.content, range_utf16)
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
        if !self.content.is_empty() {
            spec = spec.value(self.content.clone());
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
            .field("length", &self.content.len())
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
        Some(self.content.get(range)?.to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.read_only {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range.clone());

        let new_text = text_edit::normalize_multiline(new_text);
        let new_text =
            text_edit::fit_to_max_length(&self.content, self.max_length, &range, &new_text);
        self.content =
            (self.content[..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        self.accessibility_revision = self.accessibility_revision.wrapping_add(1);
        let caret = range.start + new_text.len();
        self.selected_range = caret..caret;
        self.selection_reversed = false;
        self.marked_range = None;
        self.goal_x = None;
        cx.emit(TextAreaEvent::Change(self.content.clone()));
        cx.notify();
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
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range.clone());

        let new_text = text_edit::normalize_multiline(new_text);
        self.content =
            (self.content[..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        self.accessibility_revision = self.accessibility_revision.wrapping_add(1);
        self.marked_range =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.start)
            .unwrap_or_else(|| {
                let caret = range.start + new_text.len();
                caret..caret
            });
        self.selection_reversed = false;
        self.goal_x = None;
        cx.emit(TextAreaEvent::Change(self.content.clone()));
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
        let start = layout.position_for_offset(range.start);
        let end = layout.position_for_offset(range.end);
        Some(Bounds::from_corners(
            point(
                bounds.left() + start.x,
                bounds.top() + start.y - self.scroll_offset,
            ),
            point(
                bounds.left() + end.x,
                bounds.top() + end.y + layout.line_height() - self.scroll_offset,
            ),
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
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let focused = self.focus_handle.is_focused(window);
        let spec = self.semantics();
        let content = self.content.clone();
        let (anchor, focus) = if self.selection_reversed {
            (self.selected_range.end, self.selected_range.start)
        } else {
            (self.selected_range.start, self.selected_range.end)
        };
        let accessible_snapshot = self.accessible_snapshot.clone();
        let selection_representable = text_edit::accessible_text_is_representable(&content);
        let accessible_rows = self.accessible_rows();
        let accessibility_revision = self.accessibility_revision;
        let entity = cx.entity().clone();
        let accessible_direction = if cx.layout_direction().is_rtl() {
            gpui::accesskit::TextDirection::RightToLeft
        } else {
            gpui::accesskit::TextDirection::LeftToRight
        };

        div()
            .id(self.ident.element_id())
            .key_context(KEY_CONTEXT)
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
                    let snapshot = text_edit::publish_accessible_text(
                        builder,
                        &content,
                        anchor,
                        focus,
                        accessible_direction,
                        &accessible_rows,
                        accessibility_revision,
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
                                    &area.content,
                                    area.accessibility_revision,
                                    published,
                                    selection.anchor,
                                ) else {
                                    return;
                                };
                                let Some(focus) = text_edit::byte_offset_for_published_position(
                                    &area.content,
                                    area.accessibility_revision,
                                    published,
                                    selection.focus,
                                ) else {
                                    return;
                                };
                                area.selected_range = anchor.min(focus)..anchor.max(focus);
                                area.selection_reversed = focus < anchor;
                                area.marked_range = None;
                                area.goal_x = None;
                                cx.notify();
                            });
                        },
                    )
                },
            )
            .when(!self.disabled && !self.read_only, |element| {
                element.on_a11y_action(AccessibleAction::SetValue, move |data, window, cx| {
                    let Some(ActionData::Value(value)) = data else {
                        return;
                    };
                    entity.update(cx, |area, cx| {
                        if area.disabled || area.read_only {
                            return;
                        }
                        let end = text_edit::offset_to_utf16(&area.content, area.content.len());
                        area.replace_text_in_range(Some(0..end), value, window, cx);
                    });
                })
            })
            .w_full()
            .column()
            .px(px(metrics.padding_x))
            .py(px(theme.spacing.xs))
            .radius(&theme, Radius::Control)
            .well(&theme)
            .when(self.invalid, |element| {
                element.border_color(theme.colors.danger)
            })
            .when(focused, |element| element.shadow(theme.focus_ring()))
            .text_size(px(metrics.font_size))
            .text_color(if self.disabled {
                theme.colors.text_faint
            } else {
                theme.colors.text
            })
            .child(TextAreaElement::new(cx.entity()))
            .semantic_in(cx, spec)
    }
}
