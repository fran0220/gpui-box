//! Tokenised entry: the tags a typist has assembled, and a field to add one.
//!
//! The set belongs to the caller. Adding and removing are reported, never
//! applied, so a host that refuses an addition simply does not add it. What
//! the control refuses on its own — a duplicate, a set already at its limit —
//! it refuses out loud: a keystroke that vanishes without a word is
//! indistinguishable from a broken field.

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled, Subscription,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TypeScale};

use crate::controls::field::{FieldState, field_shell};
use crate::controls::input::{TextInput, TextInputEvent};
use crate::display::tag::Tag;
use crate::foundation::{Disableable, Ident, Selectable, Sizable, StyledExt};

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
    placeholder: SharedString,
    max: Option<usize>,
    size: ControlSize,
    disabled: bool,
    invalid: bool,
    /// The tag backspace has singled out, which is not a tag that is gone.
    targeted: Option<SharedString>,
    /// What the control last refused, in the words it will show.
    refusal: Option<SharedString>,
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
            placeholder: SharedString::from("Add"),
            max: None,
            size: ControlSize::Md,
            disabled: false,
            invalid: false,
            targeted: None,
            refusal: None,
            _subscriptions: vec![subscription],
        }
    }

    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
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
            self.refusal = Some(SharedString::from(format!("“{value}” is already here")));
            cx.emit(TagInputEvent::Duplicate(value));
            cx.notify();
            return;
        }
        if self.is_full() {
            let max = self.max.unwrap_or_default();
            self.refusal = Some(SharedString::from(format!(
                "This field holds at most {max}; “{value}” was not added"
            )));
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
            let placeholder = self.placeholder.clone();
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
        let tags = self
            .tags
            .iter()
            .map(|value| {
                let ident = self.ident.child(value.as_ref());
                let removing = value.clone();
                let control = control.clone();
                Tag::new(ident, value.clone())
                    .selected(self.targeted.as_ref() == Some(value))
                    .disabled(self.disabled)
                    .on_remove(move |_window, cx| {
                        control
                            .update(cx, |tags, cx| tags.remove(removing.clone(), cx))
                            .ok();
                    })
            })
            .collect::<Vec<_>>();

        let count = self.tags.len();
        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Group)
            .disabled(self.disabled)
            .invalid(invalid)
            .focus(&self.field.read(cx).focus_handle(cx))
            .value(SharedString::from(match self.max {
                Some(max) => format!("{count} of {max}"),
                None => count.to_string(),
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
                .child(div().flex_1().min_w(px(80.0)).child(self.field.clone())),
            )
            .children(self.refusal.clone().map(|refusal| {
                div()
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.danger)
                    .child(refusal.clone())
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
                    div()
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_muted)
                        .child(SharedString::from(format!(
                            "{count} of {} used",
                            self.max.unwrap_or_default()
                        ))),
                )
            })
            .semantic_in(cx, spec)
    }
}
