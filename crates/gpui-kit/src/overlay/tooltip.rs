//! Optional hover and keyboard-focus help for a control that is already usable without it.
//!
//! A tooltip is never actionable and never carries the only copy of something
//! the user needs in order to act. [`Tooltipped::tip`] is hover-only;
//! [`Tooltipped::help_tip`] explicitly adds immediate focus help with Escape
//! dismissal while preserving the same semantic description.
//!
//! Delay, placement, and dismissal come from GPUI's tooltip machinery; this
//! module supplies the themed surface it renders and the semantic node it publishes.

use gpui::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement, Render, RenderOnce,
    SharedString, Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space};

use crate::foundation::{Ident, StyledExt};
use crate::motion::{Animated, Entrance};
use crate::overlay::layer::{OverlaySurface, surface};
use crate::overlay::tail::{TailSide, tail};

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

    /// Associates this help with its role-bearing control in deterministic
    /// snapshots and GPUI's native accessibility tree.
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
        //
        // The tail is what separates help from a control: a tooltip and a
        // secondary button are the same rounded rectangle of overlay colour,
        // and only the point says this one came out of something else.
        let surface = div()
            .column()
            .items_start()
            .child(
                div()
                    .ml(px(crate::overlay::tail::inset(&theme)))
                    .child(tail(&theme, TailSide::Up, theme.colors.overlay)),
            )
            .child(
                // The one floating recipe, same as a menu or a toast: a
                // tooltip that kept its own radius was a second vocabulary
                // for the same detached plane.
                surface(self.ident.clone(), &theme, OverlaySurface::FLOATING)
                    .max_w(px(260.0))
                    .px_token(&theme, Space::Sm)
                    .py_token(&theme, Space::Xs)
                    .text_size(px(theme.typography.label.size))
                    .line_height(px(theme.typography.label.line_height))
                    .font_fallbacks(gpui_kit_assets::text_fallbacks())
                    .child(self.text.clone()),
            );

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

    /// Shows one help tooltip while the control is hovered or focused.
    ///
    /// The control must already have a stable element id and must be focusable
    /// for keyboard help, for example through `tab_index` or `track_focus`.
    /// Hover keeps GPUI's normal delay, focus opens immediately, and Escape
    /// dismisses help until focus leaves. The tooltip describes `control` in
    /// the semantic tree.
    fn help_tip(self, control: impl Into<Ident>, text: impl Into<SharedString>) -> Self {
        let control = control.into();
        let text = text.into();
        self.focusable_tooltip(move |_window, cx| {
            Tooltip::new(control.child("tooltip"), text.clone())
                .describes(control.semantic_id())
                .view(cx)
        })
    }
}

impl<E: gpui::StatefulInteractiveElement + Sized> Tooltipped for E {}
