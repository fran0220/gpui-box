//! Top-level destinations; selection and routing stay with the caller.
use crate::{
    controls::button::Button,
    foundation::{Disableable, Ident, Selectable, Sizable, StyledExt},
};
use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, Surface};
use std::rc::Rc;

/// A destination identified by caller identity, never list position.
#[derive(Debug, Clone)]
pub struct NavigationItem {
    pub id: SharedString,
    pub label: SharedString,
    pub icon: Option<Icon>,
    pub disabled: bool,
}
impl NavigationItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            disabled: false,
        }
    }
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

type Select = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// Equal-width destination actions for compact pages. No safe-area padding is
/// added here: PageLayout consumes system insets once for all its slots.
/// Without a handler destinations are disabled rather than falsely operable.
#[derive(IntoElement)]
pub struct BottomNavigation {
    ident: Ident,
    items: Vec<NavigationItem>,
    selected: Option<SharedString>,
    on_select: Option<Select>,
}
impl BottomNavigation {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            items: Vec::new(),
            selected: None,
            on_select: None,
        }
    }
    pub fn items(mut self, items: impl IntoIterator<Item = NavigationItem>) -> Self {
        self.items = items.into_iter().collect();
        self
    }
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }
    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}
impl RenderOnce for BottomNavigation {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let parent = self.ident.semantic_id();
        div()
            .id(self.ident.element_id())
            .flex()
            .w_full()
            .flex_none()
            .p(px(theme.space(Space::Xs)))
            .gap(px(theme.space(Space::Xs)))
            .surface(&theme, Surface::Panel)
            .children(self.items.into_iter().map(|item| {
                let handler = self.on_select.clone();
                let selected = self.selected.as_ref() == Some(&item.id);
                let disabled = item.disabled || handler.is_none();
                div().flex_1().min_w_0().child(
                    Button::new(self.ident.child(item.id.as_ref()))
                        .semantic_parent(parent.clone())
                        .label(item.label)
                        .ghost()
                        .control_size(ControlSize::Touch)
                        .full_width(true)
                        .selected(selected)
                        .disabled(disabled)
                        .when_some(item.icon, |button, icon| button.icon(icon))
                        .when(!disabled, |button| {
                            button.on_click(move |window, cx| {
                                if let Some(handler) = &handler {
                                    handler(item.id.clone(), window, cx);
                                }
                            })
                        }),
                )
            }))
            .semantic_in(cx, NodeSpec::new(parent, Role::Group))
    }
}
