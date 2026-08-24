//! Tokenised entry: the tags a typist has assembled, and a field to add one.
//!
//! The set belongs to the caller. Adding and removing are reported, never
//! applied, so a host that refuses an addition simply does not add it. What
//! the control refuses on its own — a duplicate, a set already at its limit —
//! it refuses out loud: a keystroke that vanishes without a word is
//! indistinguishable from a broken field.

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TypeScale};

use crate::controls::field::{FieldState, field_shell};
use crate::controls::input::{TextInput, TextInputEvent};
use crate::display::tag::Tag;
use crate::foundation::{
    Disableable, Ident, Selectable, Sizable, StyledExt, text as foundation_text,
};
use crate::interaction::dnd::{self, DragItem, DropAxis, DropIntent, DropPosition, RowTarget};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// What a tag field reports. The owner decides what any of it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagInputEvent {
    /// A value the typist asked to add.
    Added(SharedString),
    /// A tag the typist asked to remove.
    Removed(SharedString),
    /// The value is already in the set. Reported rather than dropped, so a
    /// host can say so where the typist is looking.
    Duplicate(SharedString),
    /// The set is already as large as it was allowed to be.
    Refused(SharedString),
    /// A tag should move from `from` to `to`. The field does not move it.
    Moved { from: usize, to: usize },
    /// The typist asked to edit this tag in place.
    EditRequested(SharedString),
}

impl EventEmitter<TagInputEvent> for TagInput {}

/// A field that collects a set of short tokens.
///
/// A duplicate and a full field are refusals, shown where the typist is
/// looking and with the typed text left in place, because a token the host
/// will not take is not a token that silently vanished.
pub struct TagInput {
    ident: Ident,
    focus_handle: FocusHandle,
    field: Entity<TextInput>,
    tags: Vec<SharedString>,
    placeholder: Option<SharedString>,
    max: Option<usize>,
    size: ControlSize,
    disabled: bool,
    invalid: bool,
    /// The tag backspace has singled out, which is not a tag that is gone.
    targeted: Option<SharedString>,
    /// What the control last refused, in the words it will show.
    refusal: Option<SharedString>,
    reorderable: bool,
    collapse_at: Option<usize>,
    /// Held so the field subscription lives as long as the control does.
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for TagInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TagInput")
            .field("ident", &self.ident)
            .field("tags", &self.tags.len())
            .field("max", &self.max)
            .field("targeted", &self.targeted)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl TagInput {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let field = cx.new(|cx| TextInput::new(ident.child("field"), window, cx).bare(true));
        let subscription = cx.subscribe(&field, |tags, _field, event, cx| match event {
            TextInputEvent::Change(text) => tags.on_change(text.clone(), cx),
            TextInputEvent::Submit => tags.commit(cx),
            TextInputEvent::BackspaceAtStart => tags.backspace(cx),
            TextInputEvent::Cancel => tags.untarget(cx),
            _ => {}
        });

        Self {
            ident,
            focus_handle: cx.focus_handle(),
            field,
            tags: Vec::new(),
            placeholder: None,
            max: None,
            size: ControlSize::Md,
            disabled: false,
            invalid: false,
            targeted: None,
            refusal: None,
            reorderable: false,
            collapse_at: None,
            _subscriptions: vec![subscription],
        }
    }

    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// How many tags the set may hold. Reaching it is reported as a refusal,
    /// not enforced by swallowing what was typed.
    pub fn max(mut self, max: usize) -> Self {
        self.max = Some(max);
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Draws the control as refused after it was built, for an owner that
    /// learns the answer is wrong later — a host that rejected it, or a form
    /// that found a required answer missing. Without it the message and the
    /// control it is about disagree.
    pub fn set_invalid(&mut self, invalid: bool, cx: &mut Context<Self>) {
        if self.invalid == invalid {
            return;
        }
        self.invalid = invalid;
        cx.notify();
    }

    /// Lets a tag be picked up and put somewhere else in the set.
    pub fn reorderable(mut self, reorderable: bool) -> Self {
        self.reorderable = reorderable;
        self
    }

    /// How many tags stay visible before the rest collapse into a count.
    pub fn collapse_at(mut self, visible: usize) -> Self {
        self.collapse_at = Some(visible.max(1));
        self
    }

    /// Replaces the set from the host side, which is how an addition or a
    /// removal actually takes effect.
    pub fn set_tags(&mut self, tags: Vec<SharedString>, cx: &mut Context<Self>) {
        self.tags = tags;
        self.targeted = None;
        self.refusal = None;
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.field
            .update(cx, |field, cx| field.set_disabled(disabled, cx));
        cx.notify();
    }

    pub fn current(&self) -> &[SharedString] {
        &self.tags
    }

    pub fn field(&self) -> &Entity<TextInput> {
        &self.field
    }

    /// The tag the next backspace would remove, if there is one.
    pub fn targeted(&self) -> Option<&SharedString> {
        self.targeted.as_ref()
    }

    /// What the control last refused, in the words it shows.
    pub fn refusal(&self) -> Option<&SharedString> {
        self.refusal.as_ref()
    }

    fn is_full(&self) -> bool {
        self.max.is_some_and(|max| self.tags.len() >= max)
    }

    fn clear_field(&mut self, cx: &mut Context<Self>) {
        self.field
            .update(cx, |field, cx| field.set_text_quietly("", cx));
    }

    /// Commits what is typed, up to but not including a trailing separator.
    ///
    /// A comma is a separator rather than a character here, so typing one
    /// commits what came before it instead of ending up inside a tag.
    fn on_change(&mut self, text: SharedString, cx: &mut Context<Self>) {
        // Anything typed moves the keyboard off a targeted tag: the typist is
        // adding, not removing.
        if !text.is_empty() {
            self.targeted = None;
        }
        if text.ends_with(',') {
            let value = SharedString::from(text.trim_end_matches(',').to_string());
            self.add(value, cx);
        }
        cx.notify();
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        let typed = self.field.read(cx).value().clone();
        self.add(typed, cx);
    }

    fn add(&mut self, value: SharedString, cx: &mut Context<Self>) {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            self.clear_field(cx);
            return;
        }
        let value = SharedString::from(trimmed.to_string());

        if self.tags.iter().any(|tag| tag == &value) {
            self.refusal = Some(cx.strings().format(StringKey::TagInputDuplicate, &[&value]));
            cx.emit(TagInputEvent::Duplicate(value));
            cx.notify();
            return;
        }
        if self.is_full() {
            let max = self.max.unwrap_or_default();
            self.refusal = Some(cx.strings().format(
                StringKey::TagInputFull,
                &[cx.numbers().count(max).as_ref(), &value],
            ));
            cx.emit(TagInputEvent::Refused(value));
            cx.notify();
            return;
        }

        self.refusal = None;
        self.clear_field(cx);
        cx.emit(TagInputEvent::Added(value));
        cx.notify();
    }

    fn remove(&mut self, value: SharedString, cx: &mut Context<Self>) {
        self.targeted = None;
        self.refusal = None;
        cx.emit(TagInputEvent::Removed(value));
        cx.notify();
    }

    /// Backspace in an empty field. The first press singles out the last tag
    /// and the second removes it, because a deletion nobody saw coming is a
    /// deletion nobody agreed to.
    fn backspace(&mut self, cx: &mut Context<Self>) {
        if self.disabled || !self.field.read(cx).value().is_empty() {
            return;
        }
        match self.targeted.clone() {
            Some(value) if self.tags.iter().any(|tag| tag == &value) => self.remove(value, cx),
            _ => {
                self.targeted = self.tags.last().cloned();
                cx.notify();
            }
        }
    }

    /// Lets the singled-out tag go without removing it.
    fn untarget(&mut self, cx: &mut Context<Self>) {
        if self.targeted.take().is_some() {
            cx.notify();
        }
    }
}

impl Disableable for TagInput {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for TagInput {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for TagInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TagInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        if self.field.read(cx).placeholder_text().is_empty() {
            let placeholder = self
                .placeholder
                .clone()
                .unwrap_or_else(|| cx.strings().text(StringKey::TagInputPlaceholder));
            self.field
                .update(cx, |field, cx| field.set_placeholder(placeholder, cx));
        }
        if self.disabled != self.field.read(cx).is_disabled() {
            let disabled = self.disabled;
            self.field
                .update(cx, |field, cx| field.set_disabled(disabled, cx));
        }

        let focused = self.field.read(cx).focus_handle(cx).is_focused(window);
        let invalid = self.invalid || self.refusal.is_some();
        let full = self.is_full();

        let control = cx.entity().downgrade();
        let (visible, hidden) = match self.collapse_at {
            Some(max) if self.tags.len() > max => (&self.tags[..max], self.tags.len() - max),
            _ => (self.tags.as_slice(), 0),
        };
        let surface = self.ident.semantic_id();
        let tags = visible
            .iter()
            .enumerate()
            .map(|(index, tag_value)| {
                let ident = self.ident.child(tag_value.as_ref());
                let removing = tag_value.clone();
                let control = control.clone();
                let tag = Tag::new(ident.clone(), tag_value.clone())
                    .selected(self.targeted.as_ref() == Some(tag_value))
                    .disabled(self.disabled)
                    .on_remove({
                        let control = control.clone();
                        move |_window, cx| {
                            control
                                .update(cx, |tags, cx| tags.remove(removing.clone(), cx))
                                .ok();
                        }
                    });
                let editing = tag_value.clone();
                let mut chip = div()
                    .id(ident.child("chip").element_id())
                    .child(tag)
                    .on_click({
                        let control = control.clone();
                        move |event, _, cx| {
                            if event.click_count() == 2 {
                                control
                                    .update(cx, |_, cx| {
                                        cx.emit(TagInputEvent::EditRequested(editing.clone()));
                                    })
                                    .ok();
                                cx.stop_propagation();
                            }
                        }
                    });
                if !self.disabled && self.reorderable {
                    let item = DragItem::new(surface.clone(), tag_value.clone(), tag_value.clone());
                    chip = dnd::draggable(chip, item);
                    let accepts = {
                        let own = surface.clone();
                        Rc::new(move |item: &DragItem, _: &DropPosition| item.source == own)
                    };
                    let on_drop = {
                        let control = control.clone();
                        let tags = self.tags.clone();
                        Rc::new(
                            move |intent: &DropIntent, _window: &mut Window, cx: &mut App| {
                                let from = tags.iter().position(|tag| tag == &intent.item.id);
                                let to = match &intent.position {
                                    DropPosition::Before(id) => {
                                        tags.iter().position(|tag| tag == id)
                                    }
                                    DropPosition::After(id) => {
                                        tags.iter().position(|tag| tag == id).map(|at| at + 1)
                                    }
                                    DropPosition::Into(_) => None,
                                };
                                if let (Some(from), Some(to)) = (from, to) {
                                    control
                                        .update(cx, |_, cx| {
                                            cx.emit(TagInputEvent::Moved { from, to });
                                        })
                                        .ok();
                                }
                            },
                        )
                    };
                    chip = dnd::drop_target(
                        chip,
                        RowTarget {
                            surface: surface.clone(),
                            id: tag_value.clone(),
                            index,
                            allow_into: false,
                            axis: DropAxis::Horizontal,
                            accepts,
                            on_drop,
                        },
                    );
                }
                chip
            })
            .collect::<Vec<_>>();
        let overflow = (hidden > 0).then(|| {
            Tag::new(
                self.ident.child("overflow"),
                cx.strings().format(
                    StringKey::TagInputOverflow,
                    &[cx.numbers().count(hidden).as_ref()],
                ),
            )
            .disabled(true)
        });

        let count = self.tags.len();
        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Group)
            .disabled(self.disabled)
            .invalid(invalid)
            .focus(&self.field.read(cx).focus_handle(cx))
            .value(SharedString::from(match self.max {
                Some(max) => cx.numbers().count_of_total(count, max).to_string(),
                None => cx.numbers().count(count).to_string(),
            }));
        if let Some(refusal) = self.refusal.clone() {
            spec = spec.text(refusal);
        }

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap(px(theme.space(Space::Xs)))
            .track_focus(&self.focus_handle)
            .child(
                field_shell(
                    &theme,
                    self.size,
                    FieldState::default()
                        .focused(focused)
                        .invalid(invalid)
                        .disabled(self.disabled),
                )
                .flex_wrap()
                .py(px(theme.space(Space::Xs)))
                .gap(px(theme.space(Space::Xs)))
                .children(tags)
                .children(overflow)
                .child(div().flex_1().min_w(px(80.0)).child(self.field.clone())),
            )
            .children(self.refusal.clone().map(|refusal| {
                foundation_text(&theme, TypeScale::Caption, refusal.clone())
                    .text_color(theme.colors.danger)
                    .semantic_in(
                        cx,
                        NodeSpec::new(self.ident.child("refusal").semantic_id(), Role::Status)
                            .parent(self.ident.semantic_id())
                            .invalid(true)
                            .text(refusal),
                    )
            }))
            .when(full && self.refusal.is_none(), |element| {
                element.child(
                    foundation_text(
                        &theme,
                        TypeScale::Caption,
                        cx.strings().format(
                            StringKey::TagInputUsed,
                            &[
                                cx.numbers().count(count).as_ref(),
                                cx.numbers().count(self.max.unwrap_or_default()).as_ref(),
                            ],
                        ),
                    )
                    .text_tone(&theme, gpui_kit_theme::TextTone::Muted),
                )
            })
            .semantic_in(cx, spec)
    }
}
