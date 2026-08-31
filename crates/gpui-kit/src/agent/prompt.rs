//! A caller-owned prompt template with named slots.
//!
//! The host supplies the template text and every slot identity. This
//! component highlights the slots and reports which one was activated; it
//! never fills, expands, or sends the prompt.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, TypeScale};

use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::loading::PulseLoader;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Disableable, FocusRing, Ident, Pressable, StyledExt};
use crate::state::{HasPhase, Phase};
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
    Loading,
    Ready,
    Empty,
    Unavailable(SharedString),
    Error(SharedString),
}

impl PromptBuilderState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Empty => "empty",
            Self::Unavailable(_) => "unavailable",
            Self::Error(_) => "error",
        }
    }
}

impl HasPhase for PromptBuilderState {
    fn phase(&self) -> Phase {
        match self {
            Self::Loading => Phase::Loading,
            Self::Ready => Phase::Ready,
            Self::Empty => Phase::Empty,
            Self::Unavailable(_) => Phase::Unavailable,
            Self::Error(_) => Phase::Error,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(reason) | Self::Error(reason) => Some(reason.as_ref()),
            _ => None,
        }
    }
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
    replacements: Slots,
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
            replacements: Slots::default(),
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

impl Slotted for PromptBuilder {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.replacements
    }
}

impl Disableable for PromptBuilder {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for PromptBuilder {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (body, spec): (gpui::AnyElement, NodeSpec) = match &self.state {
            PromptBuilderState::Loading => (
                PulseLoader::new(self.ident.child("loading"))
                    .label(cx.strings().text(StringKey::Loading))
                    .into_any_element(),
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.label.clone())
                    .value(self.state.name())
                    .busy(true),
            ),
            PromptBuilderState::Empty => (
                self.replacements.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        self.ident.child("empty"),
                        cx.strings().text(StringKey::PromptEmpty),
                    )
                    .kind(EmptyKind::Empty)
                    .icon(Icon::Document)
                    .into_any_element()
                }),
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.label.clone())
                    .value(self.state.name()),
            ),
            PromptBuilderState::Unavailable(reason) => (
                self.replacements.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        self.ident.child("unavailable"),
                        cx.strings().text(StringKey::PromptUnavailable),
                    )
                    .kind(EmptyKind::Unavailable)
                    .detail(reason.clone())
                    .into_any_element()
                }),
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.label.clone())
                    .value(self.state.name()),
            ),
            PromptBuilderState::Error(reason) => (
                self.replacements.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        self.ident.child("error"),
                        cx.strings().text(StringKey::PromptFailed),
                    )
                    .kind(EmptyKind::Failed)
                    .detail(reason.clone())
                    .into_any_element()
                }),
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .text(self.label.clone())
                    .value(self.state.name()),
            ),
            PromptBuilderState::Ready => {
                let slots = self
                    .slots
                    .into_iter()
                    .map(|slot| {
                        let handler = self.on_slot.clone().filter(|_| !self.disabled);
                        let actionable = handler.is_some();
                        let id = slot.id.clone();
                        let filled = !slot.value.is_empty();
                        // A slot that carries a value and a slot still waiting
                        // for one are two different facts, and the accent wash
                        // said neither: it coloured both alike and sat below
                        // the contrast the caption size needs. A filled slot
                        // is a settled value on a raised chip; an empty one is
                        // a sunken tonal seat with the slot's name in it.
                        let mut chip = div()
                            .id(self.ident.child("slot").child(id.as_ref()).element_id())
                            .focus_ring(&theme)
                            .px(px(theme.space(Space::Xs)))
                            .py_token(&theme, Space::Xs)
                            .radius(&theme, Radius::Small)
                            .type_scale(&theme, TypeScale::Caption)
                            .map(|element| {
                                if filled {
                                    element
                                        .surface(&theme, Surface::Raised)
                                        .text_color(theme.colors.text)
                                } else {
                                    element
                                        .surface(&theme, Surface::Sunken)
                                        .text_color(theme.colors.text_muted)
                                }
                            })
                            .child(if filled {
                                slot.value.clone()
                            } else {
                                slot.name.clone()
                            })
                            .semantic_in(
                                cx,
                                NodeSpec::new(
                                    self.ident.child("slot").child(id.as_ref()).semantic_id(),
                                    Role::Button,
                                )
                                .text(slot.name.clone())
                                .value(slot.value.clone())
                                .disabled(!actionable),
                            );
                        if let Some(handler) = handler {
                            let click_handler = Rc::clone(&handler);
                            let click_slot = slot.clone();
                            chip = chip
                                .cursor_pointer()
                                .tab_index(0)
                                .pressable(cx)
                                .on_click(move |_, window, cx| {
                                    click_handler(&click_slot, window, cx)
                                })
                                .on_key_down(move |event, window, cx| {
                                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                        handler(&slot, window, cx);
                                        cx.stop_propagation();
                                    }
                                });
                        }
                        chip
                    })
                    .collect::<Vec<_>>();
                (
                    div()
                        .column()
                        .gap_token(&theme, Space::Sm)
                        .child(
                            div()
                                .type_scale(&theme, TypeScale::Body)
                                .text_color(theme.colors.text)
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
                        .value(self.state.name()),
                )
            }
        };
        // Every state is the same template card. Previously only the ready
        // one had a card at all, so an empty or refused template arrived as a
        // loose fragment with no label on it and no edge to sit against.
        let ready = matches!(self.state, PromptBuilderState::Ready);
        div()
            .w_full()
            .column()
            .gap_token(&theme, Space::Sm)
            .p_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .child(
                div()
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.text_muted)
                    .child(self.label.clone()),
            )
            .child(
                div()
                    .w_full()
                    .column()
                    .when(!ready, |element| {
                        element
                            .items_center()
                            .justify_center()
                            .py_token(&theme, Space::Sm)
                    })
                    .child(body),
            )
            .semantic_in(cx, spec)
    }
}

#[cfg(test)]
mod prompt_phase_tests {
    use super::*;

    #[test]
    fn unavailable_is_not_empty() {
        let state = PromptBuilderState::Unavailable("denied".into());
        assert_eq!(state.phase(), Phase::Unavailable);
        assert_eq!(state.name(), "unavailable");
    }
}
