//! A caller-owned prompt template with named slots.
//!
//! The host supplies the template text and every slot identity. This
//! component highlights the slots and reports which one was activated; it
//! never fills, expands, or sends the prompt.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, TypeScale};

use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::{Disableable, FocusRing, Ident, StyledExt};
use crate::strings::{ActiveStrings, StringKey};

type SlotHandler = Rc<dyn Fn(&PromptSlot, &mut Window, &mut App)>;

/// One named hole in a template. Identity is the host's, never the draw order.
#[derive(Debug, Clone, PartialEq)]
pub struct PromptSlot {
    pub id: SharedString,
    pub name: SharedString,
    pub value: SharedString,
}

impl PromptSlot {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            value: SharedString::default(),
        }
    }

    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = value.into();
        self
    }
}

/// How the template was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptBuilderState {
    Ready,
    Empty,
    Unavailable(SharedString),
}

/// What a slot activation reports.
#[derive(Debug, Clone, PartialEq)]
pub enum PromptBuilderEvent {
    SlotActivated(SharedString),
}

/// A template with host-owned slots.
#[derive(IntoElement)]
pub struct PromptBuilder {
    ident: Ident,
    label: SharedString,
    body: SharedString,
    slots: Vec<PromptSlot>,
    state: PromptBuilderState,
    disabled: bool,
    on_slot: Option<SlotHandler>,
}

impl PromptBuilder {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            body: SharedString::default(),
            slots: Vec::new(),
            state: PromptBuilderState::Ready,
            disabled: false,
            on_slot: None,
        }
    }

    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = body.into();
        self
    }

    pub fn slots(mut self, slots: impl IntoIterator<Item = PromptSlot>) -> Self {
        self.slots = slots.into_iter().collect();
        self
    }

    pub fn state(mut self, state: PromptBuilderState) -> Self {
        self.state = state;
        self
    }

    pub fn on_slot(
        mut self,
        handler: impl Fn(&PromptSlot, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_slot = Some(Rc::new(handler));
        self
    }
}

impl Disableable for PromptBuilder {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for PromptBuilder {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, spec): (gpui::AnyElement, NodeSpec) = match &self.state {
            PromptBuilderState::Empty => (
                EmptyState::new(
                    self.ident.child("empty"),
                    cx.strings().text(StringKey::PromptEmpty),
                )
                .kind(EmptyKind::Empty)
                .into_any_element(),
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.label.clone())
                    .value("empty"),
            ),
            PromptBuilderState::Unavailable(reason) => (
                EmptyState::new(
                    self.ident.child("unavailable"),
                    cx.strings().text(StringKey::PromptUnavailable),
                )
                .kind(EmptyKind::Unavailable)
                .detail(reason.clone())
                .into_any_element(),
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.label.clone())
                    .value("unavailable"),
            ),
            PromptBuilderState::Ready => {
                let slots = self
                    .slots
                    .into_iter()
                    .map(|slot| {
                        let handler = self.on_slot.clone().filter(|_| !self.disabled);
                        let id = slot.id.clone();
                        let mut chip = div()
                            .id(self.ident.child("slot").child(id.as_ref()).element_id())
                            .focus_ring(&theme)
                            .px(px(theme.space(Space::Xs)))
                            .py(px(2.0))
                            .radius(&theme, Radius::Small)
                            .bg(theme.colors.accent.opacity(0.16))
                            .text_color(theme.colors.accent)
                            .type_scale(&theme, TypeScale::Caption)
                            .child(if slot.value.is_empty() {
                                slot.name.clone()
                            } else {
                                slot.value.clone()
                            })
                            .semantic_in(
                                cx,
                                NodeSpec::new(
                                    self.ident.child("slot").child(id.as_ref()).semantic_id(),
                                    Role::Button,
                                )
                                .text(slot.name.clone())
                                .value(slot.value.clone()),
                            );
                        if let Some(handler) = handler {
                            chip = chip.on_click(move |_, window, cx| handler(&slot, window, cx));
                        }
                        chip
                    })
                    .collect::<Vec<_>>();
                (
                    div()
                        .column()
                        .gap_token(&theme, Space::Sm)
                        .p_token(&theme, Space::Md)
                        .radius(&theme, Radius::Card)
                        .surface(&theme, Surface::Panel)
                        .child(
                            div()
                                .type_scale(&theme, TypeScale::Caption)
                                .text_color(theme.colors.text_muted)
                                .child(self.label.clone()),
                        )
                        .child(
                            div()
                                .type_scale(&theme, TypeScale::Body)
                                .child(self.body.clone()),
                        )
                        .child(
                            div()
                                .row()
                                .flex_wrap()
                                .gap_token(&theme, Space::Xs)
                                .children(slots),
                        )
                        .into_any_element(),
                    NodeSpec::new(self.ident.semantic_id(), Role::Region)
                        .text(self.label.clone())
                        .value("ready"),
                )
            }
        };
        div().w_full().child(body).semantic_in(cx, spec)
    }
}
