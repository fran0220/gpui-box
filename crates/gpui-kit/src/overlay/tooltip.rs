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
use crate::motion::{Animated, Entrance};

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

    /// Records the control this explains in deterministic semantic snapshots.
    ///
    /// GPUI does not currently expose a native cross-tree described-by
    /// relation. Callers should also publish this help as a literal accessible
    /// description on the role-bearing trigger when that association matters.
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
            spec = spec.describes(control);
        }

        // The surface arrives rather than appearing. It publishes its node
        // from the settled box and only the pixels travel, so a reader that
        // asks where the tooltip is gets the answer it will still be giving
        // once the arrival has finished.
        let surface = div()
            .max_w(px(260.0))
            .px_token(&theme, Space::Sm)
            .py_token(&theme, Space::Xs)
            .radius(&theme, Radius::Small)
            .bg(theme.colors.overlay)
            .elevation(&theme, Elevation::Overlay)
            .text_size(px(theme.typography.label.size))
            .line_height(px(theme.typography.label.line_height))
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .text_color(theme.colors.text)
            .child(self.text.clone());

        div()
            .child(surface.animate_in(self.ident.child("in").element_id(), cx, Entrance::Menu))
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
    /// control identified by `control` in deterministic semantic snapshots.
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
