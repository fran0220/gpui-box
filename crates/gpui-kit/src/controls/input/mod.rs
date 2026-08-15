//! A single-line editable text control.
//!
//! `TextInput` is a view rather than a `RenderOnce` builder, because editing
//! carries state that outlives a frame: the caret, the selection, the
//! in-progress input method composition, and the horizontal scroll position.
//! Callers own the entity and read [`TextInput::value`] from it.
//!
//! ```no_run
//! # use gpui::{App, AppContext as _, Context, Window};
//! # use gpui_kit::controls::input::{TextInput, TextInputEvent};
//! # struct Host;
//! # fn example(window: &mut Window, cx: &mut Context<Host>) {
//! let input = cx.new(|cx| {
//!     TextInput::new("settings.token", window, cx)
//!         .placeholder("sk-...")
//!         .secret(true)
//! });
//! cx.subscribe(&input, |_host, input, event, cx| {
//!     if let TextInputEvent::Submit = event {
//!         let _typed = input.read(cx).value().to_string();
//!     }
//! })
//! .detach();
//! # }
//! ```

mod element;

use std::ops::Range;
use std::sync::{Arc, Mutex};

use gpui::{
    AccessibleAction, App, Bounds, ClipboardItem, Context, CursorStyle, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ShapedLine,
    SharedString, StatefulInteractiveElement, Styled, Subscription, UTF16Selection, Window,
    accesskit::ActionData, actions, div, prelude::FluentBuilder as _, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize};
use unicode_segmentation::UnicodeSegmentation;

use crate::controls::field::{FieldState, field_shell};
use crate::controls::text_edit;
use crate::foundation::{ActiveDirection, Disableable, Ident, Sizable};
use element::TextElement;

actions!(
    gpui_kit_input,
    [
        Backspace,
        Delete,
        DeleteToLineStart,
        DeleteWordLeft,
        DeleteWordRight,
        Left,
        Right,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectWordLeft,
        SelectWordRight,
        SelectToLineStart,
        SelectToLineEnd,
        SelectAll,
        LineStart,
        LineEnd,
        Copy,
        Cut,
        Paste,
        Submit,
        Cancel,
        ShowCharacterPalette,
    ]
);

/// The key context every input publishes, so a host can layer its own
/// bindings on top without re-declaring these.
pub const KEY_CONTEXT: &str = "TextInput";

/// Installs the editing key bindings.
///
/// Called by [`crate::install`]. Bindings are scoped to the input key context,
/// so they never shadow a host's global shortcuts.
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
    let line = if cfg!(target_os = "macos") { "cmd" } else { "" };

    let mut bindings = vec![
        KeyBinding::new("backspace", Backspace, Some(KEY_CONTEXT)),
        KeyBinding::new("delete", Delete, Some(KEY_CONTEXT)),
        KeyBinding::new("left", Left, Some(KEY_CONTEXT)),
        KeyBinding::new("right", Right, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-left", SelectLeft, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-right", SelectRight, Some(KEY_CONTEXT)),
        KeyBinding::new("home", LineStart, Some(KEY_CONTEXT)),
        KeyBinding::new("end", LineEnd, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-home", SelectToLineStart, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-end", SelectToLineEnd, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", Submit, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", Cancel, Some(KEY_CONTEXT)),
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

    if !line.is_empty() {
        bindings.extend([
            KeyBinding::new(&format!("{line}-left"), LineStart, Some(KEY_CONTEXT)),
            KeyBinding::new(&format!("{line}-right"), LineEnd, Some(KEY_CONTEXT)),
            KeyBinding::new(
                &format!("{line}-shift-left"),
                SelectToLineStart,
                Some(KEY_CONTEXT),
            ),
            KeyBinding::new(
                &format!("{line}-shift-right"),
                SelectToLineEnd,
                Some(KEY_CONTEXT),
            ),
            KeyBinding::new(
                &format!("{line}-backspace"),
                DeleteToLineStart,
                Some(KEY_CONTEXT),
            ),
            KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some(KEY_CONTEXT)),
        ]);
    }

    cx.bind_keys(bindings);
}

/// What an input reports to its owner.
#[derive(Clone, PartialEq, Eq)]
pub enum TextInputEvent {
    /// The text changed, by typing, deletion, paste, or a programmatic set.
    Change(SharedString),
    /// The primary key was pressed while the input had focus.
    Submit,
    /// Editing was abandoned with the cancel key.
    Cancel,
    /// Backspace was pressed with nothing before the caret to delete.
    ///
    /// A bound key never reaches an ancestor listener, so a control that
    /// composes an input — a tag field, where backspace reaches past the
    /// start of the text — is told here instead.
    BackspaceAtStart,
    Focus,
    Blur,
}

impl std::fmt::Debug for TextInputEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // TextInput is also the editor under sensitive controls. Its
            // payload remains available to the subscriber, but formatting an
            // event must never turn the payload into an action log.
            Self::Change(_) => formatter
                .debug_tuple("Change")
                .field(&"[REDACTED]")
                .finish(),
            Self::Submit => formatter.write_str("Submit"),
            Self::Cancel => formatter.write_str("Cancel"),
            Self::BackspaceAtStart => formatter.write_str("BackspaceAtStart"),
            Self::Focus => formatter.write_str("Focus"),
            Self::Blur => formatter.write_str("Blur"),
        }
    }
}

impl EventEmitter<TextInputEvent> for TextInput {}

/// One line of editable text.
///
/// The field owns the caret, the selection, and any composition in flight; the
/// committed text belongs to the caller, which is why a host that refuses a
/// change simply does not apply it and the field keeps showing what is true.
/// A secret field publishes its shape and never its content.
pub struct TextInput {
    ident: Ident,
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    /// What to call the field where nothing on screen already does. A control
    /// that wraps a bare field owns the visible label, so the field it types
    /// into has to be told its own name or it reaches a reader unnamed.
    name: SharedString,
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
    /// Sensitivity controls every export boundary and never changes while a
    /// credential is visually revealed.
    secret: bool,
    /// Visual masking is deliberately separate from sensitivity. A password
    /// reveal changes this bit and leaves semantic, accessibility, clipboard,
    /// and Debug redaction untouched.
    visually_masked: bool,
    /// Set when a composing control supplies the frame itself.
    bare: bool,
    max_length: Option<usize>,
    /// Used by segmented sensitive inputs, where one slot means one Unicode
    /// grapheme rather than one UTF-8 byte.
    max_graphemes: Option<usize>,
    /// A custom visual may segment the one editor into this many slots. The
    /// editor still owns hit testing and IME geometry for the full surface.
    visual_slots: Option<usize>,
    scroll_offset: Pixels,
    is_selecting: bool,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    accessibility_revision: u64,
    accessible_snapshot: Arc<Mutex<Option<text_edit::PublishedAccessibleText>>>,
    /// Held so the focus listeners live as long as the input does.
    _subscriptions: Vec<Subscription>,
}

impl TextInput {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let subscriptions = vec![
            cx.on_focus(&focus_handle, window, |_, _, cx| {
                cx.emit(TextInputEvent::Focus)
            }),
            cx.on_blur(&focus_handle, window, |_, _, cx| {
                cx.emit(TextInputEvent::Blur)
            }),
        ];
        Self {
            ident: ident.into(),
            focus_handle,
            content: SharedString::default(),
            placeholder: SharedString::default(),
            name: SharedString::default(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            size: ControlSize::Md,
            disabled: false,
            invalid: false,
            required: false,
            read_only: false,
            secret: false,
            visually_masked: false,
            bare: false,
            max_length: None,
            max_graphemes: None,
            visual_slots: None,
            scroll_offset: px(0.0),
            is_selecting: false,
            last_layout: None,
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

    /// Names the field for a reader without drawing anything. Use it when a
    /// surrounding control carries the visible label.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = name.into();
        self
    }

    /// Names the field after it has been built.
    pub fn set_name(&mut self, name: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.name = name.into();
        cx.notify();
    }

    /// Seeds the initial text, with the caret at the end.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        let text = text.into();
        self.content = text_edit::normalize_single_line(&text).into();
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

    /// Keeps the value focusable and selectable while refusing editing, IME,
    /// and accessibility value changes.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Renders as dots and keeps the text out of every snapshot and export.
    pub fn secret(mut self, secret: bool) -> Self {
        self.secret = secret;
        self.visually_masked = secret;
        self
    }

    /// Changes only what is painted for a sensitive field.
    ///
    /// This is crate-private because a caller should choose a public
    /// sensitive control rather than assemble an export policy from toggles.
    pub(crate) fn set_visually_masked(&mut self, masked: bool, cx: &mut Context<Self>) {
        self.visually_masked = self.secret && masked;
        cx.notify();
    }

    /// Gives a sensitive editor a segmented visual contract while preserving
    /// one input handler, focus handle, selection, and composition.
    pub(crate) fn set_sensitive_slots(&mut self, slots: usize, cx: &mut Context<Self>) {
        let slots = slots.max(1);
        self.secret = true;
        self.visually_masked = true;
        self.max_graphemes = Some(slots);
        self.visual_slots = Some(slots);
        cx.notify();
    }

    /// Drops the input's own border and background.
    ///
    /// For a control that composes the input with something else — a step
    /// button, a token list — and draws one [`crate::controls::field::field_shell`]
    /// around the lot, so a composed field is not two nested frames.
    pub fn bare(mut self, bare: bool) -> Self {
        self.bare = bare;
        self
    }

    /// Refuses input past a length in bytes of UTF-8.
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
        let value = value.into();
        self.content = text_edit::normalize_single_line(&value).into();
        self.accessibility_revision = self.accessibility_revision.wrapping_add(1);
        let end = self.content.len();
        self.selected_range = end..end;
        self.marked_range = None;
        self.scroll_offset = px(0.0);
        cx.emit(TextInputEvent::Change(self.content.clone()));
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

    /// Replaces the text without reporting a change.
    ///
    /// For a composing control that is putting its owner's value on screen:
    /// nobody asked for that text, so reporting it as an edit would send the
    /// host a change it made itself.
    pub fn set_text_quietly(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        let value = value.into();
        self.content = text_edit::normalize_single_line(&value).into();
        self.accessibility_revision = self.accessibility_revision.wrapping_add(1);
        let end = self.content.len();
        self.selected_range = end..end;
        self.marked_range = None;
        self.scroll_offset = px(0.0);
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.marked_range = None;
            self.is_selecting = false;
        }
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        cx.notify();
    }

    pub(crate) fn set_required(&mut self, required: bool, cx: &mut Context<Self>) {
        self.required = required;
        cx.notify();
    }

    pub(crate) fn set_control_size(&mut self, size: ControlSize, cx: &mut Context<Self>) {
        self.size = size;
        cx.notify();
    }

    pub fn set_invalid(&mut self, invalid: bool, cx: &mut Context<Self>) {
        self.invalid = invalid;
        cx.notify();
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn is_secret(&self) -> bool {
        self.secret
    }

    pub(crate) fn visual_slots(&self) -> Option<usize> {
        self.visual_slots
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

    pub(crate) fn placeholder_text(&self) -> &SharedString {
        &self.placeholder
    }

    pub(crate) fn accessible_name(&self) -> &SharedString {
        &self.name
    }

    pub(crate) fn marked_range(&self) -> Option<Range<usize>> {
        self.marked_range.clone()
    }

    pub(crate) fn scroll_offset(&self) -> Pixels {
        self.scroll_offset
    }

    pub(crate) fn set_scroll_offset(&mut self, offset: Pixels) {
        self.scroll_offset = offset;
    }

    pub(crate) fn set_last_layout(&mut self, line: ShapedLine, bounds: Bounds<Pixels>) {
        self.last_layout = Some(line);
        self.last_bounds = Some(bounds);
    }

    /// What the element shapes, which is dots for a secret.
    ///
    /// The mask is one dot per grapheme so the caret can still be placed
    /// between characters the typist entered.
    pub(crate) fn display_text(&self) -> SharedString {
        if !self.visually_masked || self.content.is_empty() {
            return self.content.clone();
        }
        SharedString::from("•".repeat(self.content.graphemes(true).count()))
    }

    /// Maps a content offset onto the masked text, which has its own byte
    /// widths, so the caret lands between dots rather than inside one.
    pub(crate) fn display_offset(&self, offset: usize) -> usize {
        if !self.visually_masked {
            return offset;
        }
        let graphemes = self.content[..offset.min(self.content.len())]
            .graphemes(true)
            .count();
        graphemes * "•".len()
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
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

    pub(crate) fn index_for_position(&self, position: Point<Pixels>, rtl: bool) -> usize {
        let Some(bounds) = self.last_bounds.as_ref() else {
            return 0;
        };
        if let Some(slots) = self.visual_slots {
            let x = (position.x - bounds.left()).clamp(px(0.0), bounds.size.width);
            let width = bounds.size.width.max(px(1.0));
            let slot_width = width / slots as f32;
            let physical_slot = ((x / slot_width).floor() as usize).min(slots.saturating_sub(1));
            let logical_slot = if rtl {
                slots - physical_slot - 1
            } else {
                physical_slot
            };
            let after_midpoint = if rtl {
                x - slot_width * (physical_slot as f32) < slot_width / 2.0
            } else {
                x - slot_width * physical_slot as f32 >= slot_width / 2.0
            };
            let boundary = (logical_slot + usize::from(after_midpoint))
                .min(self.content.graphemes(true).count());
            return self.content_offset_for_grapheme(boundary);
        }
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        let Some(line) = self.last_layout.as_ref() else {
            return 0;
        };
        let x = position.x - bounds.left() + self.scroll_offset;
        let display_index = if x <= px(0.0) {
            0
        } else if x >= line.width {
            self.display_text().len()
        } else {
            line.index_for_x(x).unwrap_or(self.display_text().len())
        };
        self.content_offset_for_display(display_index)
    }

    /// The inverse of [`Self::display_offset`], for a hit test on masked text.
    fn content_offset_for_display(&self, display_index: usize) -> usize {
        if !self.visually_masked {
            return display_index;
        }
        let dots = display_index / "•".len();
        self.content_offset_for_grapheme(dots)
    }

    fn content_offset_for_grapheme(&self, grapheme: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .nth(grapheme)
            .map(|(index, _)| index)
            .unwrap_or(self.content.len())
    }

    fn grapheme_offset(&self, offset: usize) -> usize {
        self.content[..offset.min(self.content.len())]
            .graphemes(true)
            .count()
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let backwards = self.visual_slots.is_none() || !cx.layout_direction().is_rtl();
        if self.selected_range.is_empty() {
            let offset = if backwards {
                self.previous_boundary(self.cursor_offset())
            } else {
                self.next_boundary(self.cursor_offset())
            };
            self.move_to(offset, cx);
        } else {
            let offset = if backwards {
                self.selected_range.start
            } else {
                self.selected_range.end
            };
            self.move_to(offset, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let forwards = self.visual_slots.is_none() || !cx.layout_direction().is_rtl();
        if self.selected_range.is_empty() {
            let offset = if forwards {
                self.next_boundary(self.cursor_offset())
            } else {
                self.previous_boundary(self.cursor_offset())
            };
            self.move_to(offset, cx);
        } else {
            let offset = if forwards {
                self.selected_range.end
            } else {
                self.selected_range.start
            };
            self.move_to(offset, cx);
        }
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.visual_slots.is_some() && cx.layout_direction().is_rtl() {
            self.next_word_boundary(self.cursor_offset())
        } else {
            self.previous_word_boundary(self.cursor_offset())
        };
        self.move_to(offset, cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.visual_slots.is_some() && cx.layout_direction().is_rtl() {
            self.previous_word_boundary(self.cursor_offset())
        } else {
            self.next_word_boundary(self.cursor_offset())
        };
        self.move_to(offset, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.visual_slots.is_some() && cx.layout_direction().is_rtl() {
            self.next_boundary(self.cursor_offset())
        } else {
            self.previous_boundary(self.cursor_offset())
        };
        self.select_to(offset, cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.visual_slots.is_some() && cx.layout_direction().is_rtl() {
            self.previous_boundary(self.cursor_offset())
        } else {
            self.next_boundary(self.cursor_offset())
        };
        self.select_to(offset, cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.visual_slots.is_some() && cx.layout_direction().is_rtl() {
            self.next_word_boundary(self.cursor_offset())
        } else {
            self.previous_word_boundary(self.cursor_offset())
        };
        self.select_to(offset, cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.visual_slots.is_some() && cx.layout_direction().is_rtl() {
            self.previous_word_boundary(self.cursor_offset())
        } else {
            self.next_word_boundary(self.cursor_offset())
        };
        self.select_to(offset, cx);
    }

    fn select_to_line_start(
        &mut self,
        _: &SelectToLineStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(0, cx);
    }

    fn select_to_line_end(&mut self, _: &SelectToLineEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.content.len(), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx);
    }

    fn line_start(&mut self, _: &LineStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn line_end(&mut self, _: &LineEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            if self.cursor_offset() == 0 {
                cx.emit(TextInputEvent::BackspaceAtStart);
                return;
            }
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
            self.select_to(0, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        // A secret is never handed to the clipboard, where nothing in this
        // library controls where it goes next.
        if self.selected_range.is_empty() || self.secret {
            return;
        }
        let selected = self.content[self.selected_range.clone()].to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(selected));
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() || self.secret {
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
        // A single-line control accepts pasted lines as spaces rather than
        // silently dropping everything after the first newline.
        let text = text.replace(['\n', '\r'], " ");
        self.replace_text_in_range(None, &text, window, cx);
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Submit);
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Cancel);
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
        let offset = self.index_for_position(event.position, cx.layout_direction().is_rtl());
        if event.modifiers.shift {
            self.select_to(offset, cx);
        } else if event.click_count > 1 {
            self.move_to(0, cx);
            self.select_to(self.content.len(), cx);
        } else {
            self.move_to(offset, cx);
        }
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(
                self.index_for_position(event.position, cx.layout_direction().is_rtl()),
                cx,
            );
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

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        text_edit::range_from_utf16(&self.content, range)
    }

    fn semantics(&self, window: &Window) -> NodeSpec {
        let role = if self.secret {
            Role::PasswordInput
        } else {
            Role::Input
        };
        let mut spec = NodeSpec::new(self.ident.semantic_id(), role)
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
        // A secret publishes its shape, never its text, so a snapshot can
        // assert that something was typed without carrying the credential.
        if self.secret {
            if !self.content.is_empty() {
                spec = spec.value("[REDACTED]");
            }
            if let Some(slots) = self.visual_slots {
                spec = spec.description(SharedString::from(format!(
                    "{}/{}",
                    self.content.graphemes(true).count(),
                    slots
                )));
            }
        } else if !self.content.is_empty() {
            spec = spec.value(self.content.clone());
        }
        let _ = window;
        spec
    }
}

impl std::fmt::Debug for TextInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The content is deliberately absent: an input may hold a credential,
        // and a debug log is not a place for one.
        formatter
            .debug_struct("TextInput")
            .field("id", &self.ident)
            .field("size", &self.size)
            .field("disabled", &self.disabled)
            .field("invalid", &self.invalid)
            .field("secret", &self.secret)
            .field("length", &self.content.graphemes(true).count())
            .finish()
    }
}

impl Disableable for TextInput {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for TextInput {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for TextInput {
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

        let new_text = text_edit::normalize_single_line(new_text);
        let new_text =
            text_edit::fit_to_max_length(&self.content, self.max_length, &range, &new_text);
        let new_text =
            text_edit::fit_to_max_graphemes(&self.content, self.max_graphemes, &range, &new_text);
        let next_content =
            self.content[..range.start].to_owned() + &new_text + &self.content[range.end..];
        let changed = self.content.as_ref() != next_content.as_str();
        if changed {
            self.content = next_content.into();
            self.accessibility_revision = self.accessibility_revision.wrapping_add(1);
        }
        let caret = range.start + new_text.len();
        self.selected_range = caret..caret;
        self.selection_reversed = false;
        self.marked_range = None;
        if changed {
            cx.emit(TextInputEvent::Change(self.content.clone()));
        }
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

        let new_text = text_edit::normalize_single_line(new_text);
        let new_text =
            text_edit::fit_to_max_length(&self.content, self.max_length, &range, &new_text);
        let new_text =
            text_edit::fit_to_max_graphemes(&self.content, self.max_graphemes, &range, &new_text);
        let next_content =
            self.content[..range.start].to_owned() + &new_text + &self.content[range.end..];
        let changed = self.content.as_ref() != next_content.as_str();
        if changed {
            self.content = next_content.into();
            self.accessibility_revision = self.accessibility_revision.wrapping_add(1);
        }
        self.marked_range =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        self.selected_range = new_selected_range_utf16
            .as_ref()
            // GPUI reports this range relative to the composing replacement,
            // not the full document. Convert against exactly that replacement
            // before offsetting it into the document; converting against the
            // already-mutated document can land inside an astral UTF-8 scalar.
            .map(|range_utf16| text_edit::range_from_utf16(&new_text, range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.start)
            .unwrap_or_else(|| {
                let caret = range.start + new_text.len();
                caret..caret
            });
        self.selection_reversed = false;
        if changed {
            cx.emit(TextInputEvent::Change(self.content.clone()));
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        if let Some(slots) = self.visual_slots {
            let boundary_x = |offset| {
                let boundary = self.grapheme_offset(offset) as f32 / slots as f32;
                if cx.layout_direction().is_rtl() {
                    bounds.right() - bounds.size.width * boundary
                } else {
                    bounds.left() + bounds.size.width * boundary
                }
            };
            let start = boundary_x(range.start);
            let end = boundary_x(range.end);
            return Some(Bounds::from_corners(
                gpui::point(start.min(end), bounds.top()),
                gpui::point(start.max(end), bounds.bottom()),
            ));
        }
        let line = self.last_layout.as_ref()?;
        Some(Bounds::from_corners(
            gpui::point(
                bounds.left() + line.x_for_index(self.display_offset(range.start))
                    - self.scroll_offset,
                bounds.top(),
            ),
            gpui::point(
                bounds.left() + line.x_for_index(self.display_offset(range.end))
                    - self.scroll_offset,
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let offset = self.index_for_position(point, cx.layout_direction().is_rtl());
        Some(self.offset_to_utf16(offset))
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.disabled && self.focus_handle.is_focused(window) {
            window.blur();
        }
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let focused = self.focus_handle.is_focused(window);
        let spec = self.semantics(window);
        let shell = if self.bare {
            div().w_full().flex().flex_row().items_center()
        } else {
            field_shell(
                &theme,
                self.size,
                FieldState::default()
                    .focused(focused)
                    .invalid(self.invalid)
                    .disabled(self.disabled),
            )
        };
        let shell = shell.font_fallbacks(gpui_kit_assets::text_fallbacks());

        let content = self.content.clone();
        let (anchor, focus) = if self.selection_reversed {
            (self.selected_range.end, self.selected_range.start)
        } else {
            (self.selected_range.start, self.selected_range.end)
        };
        let accessible_snapshot = self.accessible_snapshot.clone();
        let selection_representable = text_edit::accessible_text_is_representable(&content);
        let accessible_rows = std::iter::once(0..content.len()).collect::<Vec<_>>();
        let accessibility_revision = self.accessibility_revision;
        let entity = cx.entity().clone();
        let accessible_direction = if cx.layout_direction().is_rtl() {
            gpui::accesskit::TextDirection::RightToLeft
        } else {
            gpui::accesskit::TextDirection::LeftToRight
        };

        shell
            .id(self.ident.element_id())
            .key_context(KEY_CONTEXT)
            .when(!self.disabled, |element| {
                element.track_focus(&self.focus_handle)
            })
            .when(!self.disabled, |element| {
                element
                    .on_action(cx.listener(Self::left))
                    .on_action(cx.listener(Self::right))
                    .on_action(cx.listener(Self::word_left))
                    .on_action(cx.listener(Self::word_right))
                    .on_action(cx.listener(Self::select_left))
                    .on_action(cx.listener(Self::select_right))
                    .on_action(cx.listener(Self::select_word_left))
                    .on_action(cx.listener(Self::select_word_right))
                    .on_action(cx.listener(Self::select_to_line_start))
                    .on_action(cx.listener(Self::select_to_line_end))
                    .on_action(cx.listener(Self::select_all))
                    .on_action(cx.listener(Self::line_start))
                    .on_action(cx.listener(Self::line_end))
                    .on_action(cx.listener(Self::copy))
                    .on_action(cx.listener(Self::submit))
                    .on_action(cx.listener(Self::cancel))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .cursor(CursorStyle::IBeam)
            })
            .when(!self.disabled && !self.read_only, |element| {
                element
                    .on_action(cx.listener(Self::backspace))
                    .on_action(cx.listener(Self::delete))
                    .on_action(cx.listener(Self::delete_word_left))
                    .on_action(cx.listener(Self::delete_word_right))
                    .on_action(cx.listener(Self::delete_to_line_start))
                    .on_action(cx.listener(Self::cut))
                    .on_action(cx.listener(Self::paste))
                    .on_action(cx.listener(Self::show_character_palette))
            })
            .when(!self.secret, |element| {
                element
                    .a11y_synthetic_children(move |builder| {
                        let ids = text_edit::publish_accessible_text(
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
                            .unwrap_or_else(|poisoned| poisoned.into_inner()) = ids;
                    })
                    .when(!self.disabled && selection_representable, |element| {
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
                                selection_entity.update(cx, |input, cx| {
                                    if input.disabled {
                                        return;
                                    }
                                    let Some(published) = published.as_ref() else {
                                        return;
                                    };
                                    let Some(anchor) =
                                        text_edit::byte_offset_for_published_position(
                                            &input.content,
                                            input.accessibility_revision,
                                            published,
                                            selection.anchor,
                                        )
                                    else {
                                        return;
                                    };
                                    let Some(focus) = text_edit::byte_offset_for_published_position(
                                        &input.content,
                                        input.accessibility_revision,
                                        published,
                                        selection.focus,
                                    ) else {
                                        return;
                                    };
                                    input.selected_range = anchor.min(focus)..anchor.max(focus);
                                    input.selection_reversed = focus < anchor;
                                    input.marked_range = None;
                                    cx.notify();
                                });
                            },
                        )
                    })
            })
            .when(!self.disabled && !self.read_only, |element| {
                element.on_a11y_action(AccessibleAction::SetValue, move |data, window, cx| {
                    let Some(ActionData::Value(value)) = data else {
                        return;
                    };
                    entity.update(cx, |input, cx| {
                        if input.disabled || input.read_only {
                            return;
                        }
                        let end = text_edit::offset_to_utf16(&input.content, input.content.len());
                        input.replace_text_in_range(Some(0..end), value, window, cx);
                    });
                })
            })
            .h(px(metrics.height))
            .child(TextElement::new(cx.entity()))
            .semantic_in(cx, spec)
    }
}
