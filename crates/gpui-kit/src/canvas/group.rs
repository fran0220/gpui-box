//! A labelled boundary around a host-owned set of nodes.
//!
//! Positions stay with the host. This is only the wash and the name, so a
//! group can be seen without inventing a layout.

use gpui::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};

use crate::foundation::{Ident, StyledExt};

/// A group outline drawn over a canvas region the host already decided.
#[derive(IntoElement)]
pub struct NodeGroup {
    ident: Ident,
    label: SharedString,
    selected: bool,
}

impl NodeGroup {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            selected: false,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl RenderOnce for NodeGroup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let wash = if self.selected {
            theme.colors.accent.opacity(0.16)
        } else {
            theme.colors.accent.opacity(0.08)
        };
        div()
            .relative()
            .w(relative(1.0))
            .h(px(120.0))
            .radius(&theme, Radius::Card)
            .bg(wash)
            .p_token(&theme, Space::Sm)
            .child(
                div()
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.accent)
                    .child(self.label.clone()),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .text(self.label)
                    .selected(self.selected),
            )
    }
}
