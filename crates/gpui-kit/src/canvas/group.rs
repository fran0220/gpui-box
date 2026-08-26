//! A labelled boundary around a host-owned set of nodes.
//!
//! Positions stay with the host. This is only the wash and the name, so a
//! group can be seen without inventing a layout.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
    relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TextTone, TypeScale};

use crate::foundation::{Ident, StyledExt};

/// A labelled group drawn over a canvas region the host already decided.
#[derive(IntoElement)]
pub struct NodeGroup {
    ident: Ident,
    label: SharedString,
    selected: bool,
    children: Vec<AnyElement>,
}

impl NodeGroup {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            selected: false,
            children: Vec::new(),
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl ParentElement for NodeGroup {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for NodeGroup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        // The sunken wash and label establish the region. A transparent border
        // reserves geometry for the accent selection report, so selecting the
        // group neither reflows it nor leaves a decorative resting outline.
        div()
            .relative()
            .column()
            .w(relative(1.0))
            .min_h(px(120.0))
            .radius(&theme, Radius::Card)
            .bg(theme.colors.sunken)
            .border(px(theme.borders.hairline))
            .border_color(if self.selected {
                theme.colors.accent
            } else {
                gpui::transparent_black()
            })
            .p_token(&theme, Space::Sm)
            .gap_token(&theme, Space::Sm)
            .child(
                div().row().flex_none().child(
                    div()
                        .px_token(&theme, Space::Xs)
                        .radius(&theme, Radius::Small)
                        .bg(if self.selected {
                            theme.colors.selected
                        } else {
                            theme.colors.raised
                        })
                        .type_scale(&theme, TypeScale::Caption)
                        .text_tone(
                            &theme,
                            if self.selected {
                                TextTone::Primary
                            } else {
                                TextTone::Muted
                            },
                        )
                        .child(self.label.clone()),
                ),
            )
            .children(self.children)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .text(self.label)
                    .selected(self.selected),
            )
    }
}
