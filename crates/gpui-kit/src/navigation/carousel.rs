//! A paged, focusable track over caller-owned content.
//!
//! `Carousel` keeps page selection and data at the call site. It provides
//! stable item identity, previous/next and direct-page intents, viewport
//! clipping, reduced-motion-aware crossfading, and explicit content phases.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, TypeScale};

use crate::controls::button::{Button, IconButton};
use crate::display::state_view::StateView;
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{Disableable, FocusRing, Ident, Selectable, Sizable, StyledExt, text};
use crate::motion;
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// One page in a [`Carousel`].
pub struct CarouselItem {
    pub id: SharedString,
    pub label: SharedString,
    pub content: gpui::AnyElement,
}

impl std::fmt::Debug for CarouselItem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CarouselItem")
            .field("id", &self.id)
            .field("label", &self.label)
            .finish()
    }
}

impl CarouselItem {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            content: content.into_any_element(),
        }
    }
}

/// What a carousel reports. The caller changes the active item or phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CarouselEvent {
    Previous,
    Next,
    Selected(SharedString),
}

#[derive(Debug, Clone)]
struct CarouselStatus {
    phase: Phase,
    reason: Option<SharedString>,
    stale: bool,
}

impl HasPhase for CarouselStatus {
    fn phase(&self) -> Phase {
        self.phase
    }

    fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    fn is_stale(&self) -> bool {
        self.stale
    }
}

type EventHandler = Rc<dyn Fn(CarouselEvent, &mut Window, &mut App)>;

/// A token-backed paged viewport.
#[derive(IntoElement)]
pub struct Carousel {
    ident: Ident,
    items: Vec<CarouselItem>,
    active: Option<SharedString>,
    status: CarouselStatus,
    looped: bool,
    size: gpui_kit_theme::ControlSize,
    on_event: Option<EventHandler>,
}

impl std::fmt::Debug for Carousel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Carousel")
            .field("ident", &self.ident)
            .field("items", &self.items.len())
            .field("active", &self.active)
            .field("phase", &self.status.phase)
            .finish()
    }
}

impl Carousel {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            items: Vec::new(),
            active: None,
            status: CarouselStatus {
                phase: Phase::Ready,
                reason: None,
                stale: false,
            },
            looped: false,
            size: gpui_kit_theme::ControlSize::Md,
            on_event: None,
        }
    }

    pub fn items(mut self, items: impl IntoIterator<Item = CarouselItem>) -> Self {
        self.items = items.into_iter().collect();
        self
    }

    pub fn active(mut self, id: impl Into<SharedString>) -> Self {
        self.active = Some(id.into());
        self
    }

    pub fn phase(mut self, phase: Phase) -> Self {
        self.status.phase = phase;
        self
    }

    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.status.reason = Some(reason.into());
        self
    }

    pub fn stale(mut self, stale: bool) -> Self {
        self.status.stale = stale;
        self
    }

    pub fn looped(mut self, looped: bool) -> Self {
        self.looped = looped;
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(CarouselEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

impl Sizable for Carousel {
    fn control_size(mut self, size: gpui_kit_theme::ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Carousel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let item_count = self.items.len();
        let active_index = self
            .active
            .as_ref()
            .and_then(|active| self.items.iter().position(|item| &item.id == active))
            .unwrap_or(0)
            .min(item_count.saturating_sub(1));
        let item_ids: Vec<SharedString> = self.items.iter().map(|item| item.id.clone()).collect();
        let page_label = self
            .items
            .get(active_index)
            .map(|item| item.label.clone())
            .unwrap_or_else(|| cx.strings().text(StringKey::CarouselEmpty));

        let has_items = item_count > 0;
        let can_previous = has_items && (self.looped || active_index > 0);
        let can_next = has_items && (self.looped || active_index + 1 < item_count);
        let handler = self.on_event.clone();

        let previous = {
            let mut button = IconButton::new(
                self.ident.child("previous"),
                Icon::AltArrowLeft,
                cx.strings().text(StringKey::CarouselPrevious),
            )
            .secondary()
            .disabled(!can_previous || handler.is_none());
            if can_previous && let Some(handler) = handler.clone() {
                button =
                    button.on_click(move |window, cx| handler(CarouselEvent::Previous, window, cx));
            }
            button
        };
        let next = {
            let mut button = IconButton::new(
                self.ident.child("next"),
                Icon::AltArrowRight,
                cx.strings().text(StringKey::CarouselNext),
            )
            .secondary()
            .disabled(!can_next || handler.is_none());
            if can_next && let Some(handler) = handler.clone() {
                button =
                    button.on_click(move |window, cx| handler(CarouselEvent::Next, window, cx));
            }
            button
        };

        let indicators = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let mut button = Button::new(self.ident.child("page").child(item.id.as_ref()))
                    .label(cx.numbers().count(index + 1))
                    .accessible_name(cx.strings().format(
                        StringKey::CarouselPage,
                        &[cx.numbers().count(index + 1).as_ref()],
                    ))
                    .ghost()
                    .selected(index == active_index)
                    .disabled(handler.is_none());
                if let Some(handler) = handler.clone() {
                    let id = item.id.clone();
                    button = button.on_click(move |window, cx| {
                        handler(CarouselEvent::Selected(id.clone()), window, cx)
                    });
                }
                button
            })
            .collect::<Vec<_>>();

        let effective_phase = if !has_items && self.status.phase == Phase::Ready {
            Phase::Empty
        } else {
            self.status.phase
        };
        let track = if let Some(item) = self.items.into_iter().nth(active_index) {
            let drawn_index = motion::tracked(
                &self.ident.semantic_id(),
                active_index as f32,
                motion::state_change(&theme),
                window,
                cx,
            );
            let opacity = (1.0 - (drawn_index - active_index as f32).abs()).clamp(0.0, 1.0);
            let item_ident = self.ident.child("item").child(item.id.as_ref());
            let content = div()
                .id(item_ident.element_id())
                .w_full()
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .opacity(opacity)
                .child(item.content)
                .semantic_in(
                    cx,
                    NodeSpec::new(item_ident.semantic_id(), Role::Group)
                        .parent(self.ident.child("state").semantic_id())
                        .text(item.label),
                );
            StateView::new(
                self.ident.child("state"),
                CarouselStatus {
                    phase: effective_phase,
                    reason: self.status.reason.clone(),
                    stale: self.status.stale,
                },
            )
            .content(content)
            .into_any_element()
        } else {
            StateView::new(
                self.ident.child("state"),
                CarouselStatus {
                    phase: effective_phase,
                    reason: self.status.reason.clone(),
                    stale: self.status.stale,
                },
            )
            .into_any_element()
        };

        let mut viewport = div()
            .id(self.ident.child("track").element_id())
            .w_full()
            .overflow_hidden()
            .child(track)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.child("track").semantic_id(), Role::Region)
                    .parent(self.ident.semantic_id())
                    .text(page_label.clone())
                    .value(cx.numbers().count(active_index + usize::from(has_items))),
            );
        if handler.is_some() && has_items {
            let current = active_index;
            let total = item_count;
            let looped = self.looped;
            let handler = handler.clone();
            viewport = viewport.tab_index(0).focus_ring(&theme).on_key_down(
                move |event: &KeyDownEvent, window, cx| {
                    let step = direction.arrow_step(event.keystroke.key.as_str());
                    let selected = match step {
                        Some(1) if current + 1 < total => Some(current + 1),
                        Some(-1) if current > 0 => Some(current - 1),
                        Some(1) if looped => Some(0),
                        Some(-1) if looped => Some(total - 1),
                        _ => match event.keystroke.key.as_str() {
                            "home" => Some(0),
                            "end" => Some(total - 1),
                            _ => None,
                        },
                    };
                    let Some(selected) = selected else {
                        return;
                    };
                    if let Some(handler) = &handler {
                        if selected == current + 1 || (current + 1 == total && looped) {
                            handler(CarouselEvent::Next, window, cx);
                        } else if selected + 1 == current || (current == 0 && looped) {
                            handler(CarouselEvent::Previous, window, cx);
                        } else if let Some(id) = item_ids.get(selected).cloned() {
                            handler(CarouselEvent::Selected(id), window, cx);
                        }
                    }
                    cx.stop_propagation();
                },
            );
        }

        div()
            .id(self.ident.element_id())
            .column()
            .gap(px(theme.space(Space::Sm)))
            .child(
                div()
                    .row_reading(direction)
                    .justify_between()
                    .child(previous)
                    .child(text(&theme, TypeScale::Label, page_label))
                    .child(next),
            )
            .child(viewport)
            .child(
                div()
                    .row_reading(direction)
                    .justify_center()
                    .gap(px(theme.space(Space::Xxs)))
                    .children(indicators),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .value(cx.numbers().count(item_count)),
            )
    }
}
