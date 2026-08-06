//! Hover-delayed help for a control that is already usable without it.
//!
//! A tooltip is never actionable and never carries the only copy of something
//! the user needs in order to act, because it cannot be reached by keyboard,
//! by touch, or by anyone who does not hover.
//!
//! The delay, the placement, and the dismissal come from GPUI's own hover
//! machinery ([`gpui::StatefulInteractiveElement::tooltip`]); this module supplies the
//! themed surface it renders and the semantic node it publishes.

use gpui::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement, Render, RenderOnce,
    SharedString, Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space};

use crate::foundation::{Ident, StyledExt};

/// A themed help surface.
#[derive(Debug, Clone, IntoElement)]
pub struct Tooltip {
    ident: Ident,
    text: SharedString,
    describes: Option<SharedString>,
}

impl Tooltip {
    pub fn new(ident: impl Into<Ident>, text: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            text: text.into(),
            describes: None,
        }
    }

    /// Names the control this explains, which is how a reader knows what the
    /// floating text belongs to.
    pub fn describes(mut self, control: impl Into<SharedString>) -> Self {
        self.describes = Some(control.into());
        self
    }

    /// Wraps the surface in the view GPUI's hover machinery renders.
    pub fn view(self, cx: &mut App) -> AnyView {
        cx.new(|_| TooltipView(self)).into()
    }
}

impl RenderOnce for Tooltip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut spec =
            NodeSpec::new(self.ident.semantic_id(), Role::Tooltip).text(self.text.clone());
        if let Some(control) = self.describes.clone() {
            spec = spec.parent(control);
        }

        div()
            .max_w(px(260.0))
            .px_token(&theme, Space::Sm)
            .py_token(&theme, Space::Xs)
            .radius(&theme, Radius::Small)
            .bg(theme.colors.overlay)
            .hairline_strong(&theme)
            .elevation(&theme, Elevation::Overlay)
            .text_size(px(theme.typography.label.size))
            .line_height(px(theme.typography.label.line_height))
            .text_color(theme.colors.text)
            .child(self.text.clone())
            .semantic_in(cx, spec)
    }
}

struct TooltipView(Tooltip);

impl Render for TooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.0.clone()
    }
}

/// Attaches hover help to a control.
///
/// Hover tracking needs an element identity, so the element must already carry
/// an id.
pub trait Tooltipped: gpui::StatefulInteractiveElement + Sized {
    /// Shows `text` after GPUI's hover delay, published as help for the
    /// control identified by `control`.
    fn tip(self, control: impl Into<Ident>, text: impl Into<SharedString>) -> Self {
        let control = control.into();
        let text = text.into();
        self.tooltip(move |_window, cx| {
            Tooltip::new(control.child("tooltip"), text.clone())
                .describes(control.semantic_id())
                .view(cx)
        })
    }
}

impl<E: gpui::StatefulInteractiveElement + Sized> Tooltipped for E {}
