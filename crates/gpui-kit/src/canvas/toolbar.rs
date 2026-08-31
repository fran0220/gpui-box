//! Zoom, fit, and snap controls a canvas host already owns.
//!
//! The percentage is a finished host string. Each action reports; nothing
//! here pans, zooms, or rearranges nodes.

use std::rc::Rc;

use gpui::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Surface, TypeScale};

use crate::controls::button::Button;
use crate::foundation::{Disableable, Ident, Selectable, Sizable, StyledExt};
use crate::overlay::{Glass, GlassPreset};
use crate::strings::{ActiveStrings, StringKey};

type ActionHandler = Rc<dyn Fn(CanvasToolbarAction, &mut Window, &mut App)>;

/// A canvas chrome action the host already knows how to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasToolbarAction {
    Fit,
    Snap,
    Arrange,
}

impl CanvasToolbarAction {
    /// Every action, in the order a toolbar that offers all of them shows
    /// them.
    pub const ALL: [Self; 3] = [Self::Fit, Self::Snap, Self::Arrange];

    pub fn name(self) -> &'static str {
        match self {
            Self::Fit => "fit",
            Self::Snap => "snap",
            Self::Arrange => "arrange",
        }
    }

    fn key(self) -> StringKey {
        match self {
            Self::Fit => StringKey::GraphFit,
            Self::Snap => StringKey::GraphSnap,
            Self::Arrange => StringKey::GraphArrange,
        }
    }
}

/// What the toolbar reported.
#[derive(Debug, Clone, PartialEq)]
pub enum CanvasToolbarEvent {
    Action(CanvasToolbarAction),
}

/// Compact canvas chrome: a host-formatted zoom and the intents the host can
/// actually carry out.
#[derive(IntoElement)]
pub struct CanvasToolbar {
    ident: Ident,
    zoom: SharedString,
    actions: Vec<CanvasToolbarAction>,
    snap: bool,
    glass: Option<GlassPreset>,
    disabled: bool,
    on_action: Option<ActionHandler>,
}

impl CanvasToolbar {
    pub fn new(ident: impl Into<Ident>, zoom: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            zoom: zoom.into(),
            actions: CanvasToolbarAction::ALL.to_vec(),
            snap: false,
            glass: None,
            disabled: false,
            on_action: None,
        }
    }

    /// The actions this toolbar offers, in the order it shows them.
    ///
    /// The default is all three, which is the toolbar a graph editor wants.
    /// An inspect-only canvas cannot snap or rearrange anything, and a chip
    /// that reports an intent its host will not act on is a promise the
    /// reader is entitled to believe. Naming the actions is how a host says
    /// which promises it can keep; naming none leaves the zoom on its own,
    /// which is a legitimate readout.
    ///
    /// Repeats are dropped rather than drawn twice.
    pub fn actions(mut self, actions: impl IntoIterator<Item = CanvasToolbarAction>) -> Self {
        self.actions = Vec::new();
        for action in actions {
            if !self.actions.contains(&action) {
                self.actions.push(action);
            }
        }
        self
    }

    pub fn snap(mut self, snap: bool) -> Self {
        self.snap = snap;
        self
    }

    /// Places this floating chrome on one of the kit's glass materials.
    ///
    /// The preset resolves exclusively through the theme's `effect.glass*`
    /// tokens and the shared [`Glass`] layer, including its renderer fallback
    /// and adaptive readability tint. Without this opt-in the toolbar keeps
    /// its ordinary opaque overlay surface.
    pub fn glass(mut self, preset: GlassPreset) -> Self {
        self.glass = Some(preset);
        self
    }

    pub fn on_action(
        mut self,
        handler: impl Fn(CanvasToolbarAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

impl Disableable for CanvasToolbar {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for CanvasToolbar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let actionable = self.on_action.is_some() && !self.disabled;
        let button = |action: CanvasToolbarAction, selected: bool| {
            let label = cx.strings().text(action.key());
            let handler = self.on_action.clone().filter(|_| actionable);
            let mut chip = Button::new(self.ident.child(action.name()))
                .label(label)
                .secondary()
                .control_size(ControlSize::Xs)
                .selected(selected)
                .disabled(!actionable)
                .semantic_parent(self.ident.semantic_id());
            if let Some(handler) = handler {
                chip = chip.on_click(move |window, cx| handler(action, window, cx));
            }
            chip
        };
        let body =
            div()
                .row()
                .items_center()
                .gap_token(&theme, Space::Sm)
                .p_token(&theme, Space::Xs)
                // The zoom is a reading, not a control: it wears the mono figures
                // and a larger gap separates it from the chips beside it, so a
                // reader does not try to press it.
                .child(
                    div()
                        .px_token(&theme, Space::Xs)
                        .mono(&theme)
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_muted)
                        .child(self.zoom.clone()),
                )
                // Air separates the reading from the controls without adding
                // a decorative vertical stroke to the floating material.
                .when(!self.actions.is_empty(), |element| {
                    element.child(div().w(px(theme.space(Space::Xs))).flex_none())
                })
                .children(self.actions.iter().map(|action| {
                    button(*action, *action == CanvasToolbarAction::Snap && self.snap)
                }))
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.semantic_id(), Role::Group).value(self.zoom),
                );
        if let Some(preset) = self.glass {
            Glass::new(self.ident.child("glass"))
                .surface(Surface::Overlay)
                // Both materials draw the same detached toolbar, so they take
                // the card/popover step rather than a tiny-control radius.
                .radius(Radius::Card)
                .preset(preset)
                .adaptive(true)
                .child(body)
                .into_any_element()
        } else {
            // The opaque fallback keeps the detached toolbar's geometry when
            // glass is unavailable instead of becoming a tiny control.
            body.radius(&theme, Radius::Card)
                .surface(&theme, Surface::Overlay)
                .into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chip that reports an intent its host will not act on is a promise
    /// the reader is entitled to believe. An inspect-only canvas can snap and
    /// rearrange nothing, so it must be able to say so.
    #[test]
    fn a_toolbar_offers_only_the_intents_its_host_can_carry_out() {
        let all = CanvasToolbar::new("canvas", "100%");
        assert_eq!(all.actions, CanvasToolbarAction::ALL.to_vec());

        let inspecting = CanvasToolbar::new("canvas", "100%").actions([CanvasToolbarAction::Fit]);
        assert_eq!(inspecting.actions, vec![CanvasToolbarAction::Fit]);

        // The caller's order is the order, and a repeat is dropped rather
        // than drawn twice.
        let reordered = CanvasToolbar::new("canvas", "100%").actions([
            CanvasToolbarAction::Arrange,
            CanvasToolbarAction::Fit,
            CanvasToolbarAction::Arrange,
        ]);
        assert_eq!(
            reordered.actions,
            vec![CanvasToolbarAction::Arrange, CanvasToolbarAction::Fit]
        );

        // A zoom with nothing beside it is a readout, which is legitimate.
        assert!(
            CanvasToolbar::new("canvas", "100%")
                .actions([])
                .actions
                .is_empty()
        );

        assert_eq!(
            CanvasToolbar::new("canvas", "100%")
                .glass(GlassPreset::Frosted)
                .glass,
            Some(GlassPreset::Frosted),
            "the toolbar carries the shared glass preset rather than a local material"
        );
    }
}
