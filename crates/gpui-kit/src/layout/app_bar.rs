//! Page chrome, independent of native window controls and route policy.
use crate::foundation::{Ident, StyledExt};
use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, Surface, TypeScale};

/// A page title with caller-owned leading and trailing actions. Slots should
/// contain accessible controls. Safe areas belong to the enclosing PageLayout.
#[derive(IntoElement)]
pub struct AppBar {
    ident: Ident,
    title: SharedString,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
}

impl AppBar {
    pub fn new(ident: impl Into<Ident>, title: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            title: title.into(),
            leading: None,
            trailing: None,
        }
    }
    pub fn leading(mut self, content: impl IntoElement) -> Self {
        self.leading = Some(content.into_any_element());
        self
    }
    pub fn trailing(mut self, content: impl IntoElement) -> Self {
        self.trailing = Some(content.into_any_element());
        self
    }
}

impl RenderOnce for AppBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .id(self.ident.element_id())
            .flex()
            .items_center()
            .w_full()
            .flex_none()
            .gap(px(theme.space(Space::Sm)))
            .p(px(theme.space(Space::Sm)))
            .surface(&theme, Surface::Panel)
            .children(self.leading)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .type_scale(&theme, TypeScale::Title)
                    .child(self.title.clone())
                    .semantic_in(
                        cx,
                        NodeSpec::new(self.ident.child("title").semantic_id(), Role::Heading)
                            .parent(self.ident.semantic_id())
                            .text(self.title.clone()),
                    ),
            )
            .children(self.trailing)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Toolbar).text(self.title),
            )
    }
}
