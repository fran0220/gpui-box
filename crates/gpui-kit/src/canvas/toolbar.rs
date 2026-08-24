//! Zoom, fit, and snap controls a canvas host already owns.
//!
//! The percentage is a finished host string. Each action reports; nothing
//! here pans, zooms, or rearranges nodes.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, TypeScale};

use crate::foundation::{Disableable, FocusRing, Ident, StyledExt};
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

/// Compact canvas chrome: a host-formatted zoom and three intents.
#[derive(IntoElement)]
pub struct CanvasToolbar {
    ident: Ident,
    zoom: SharedString,
    snap: bool,
    disabled: bool,
    on_action: Option<ActionHandler>,
}

impl CanvasToolbar {
    pub fn new(ident: impl Into<Ident>, zoom: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            zoom: zoom.into(),
            snap: false,
            disabled: false,
            on_action: None,
        }
    }

    pub fn snap(mut self, snap: bool) -> Self {
        self.snap = snap;
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
        let button = |action: CanvasToolbarAction, selected: bool| {
            let label = cx.strings().text(action.key());
            let handler = self.on_action.clone().filter(|_| !self.disabled);
            let mut chip = div()
                .id(self.ident.child(action.name()).element_id())
                .focus_ring(&theme)
                .px(px(theme.space(Space::Sm)))
                .py(px(theme.space(Space::Xs)))
                .radius(&theme, Radius::Small)
                .bg(if selected {
                    theme.colors.selected
                } else {
                    theme.colors.raised
                })
                .border(px(theme.borders.hairline))
                .border_color(if selected {
                    theme.colors.hairline_strong
                } else {
                    theme.colors.hairline
                })
                .type_scale(&theme, TypeScale::Caption)
                .child(label.clone())
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child(action.name()).semantic_id(), Role::Button)
                        .text(label)
                        .selected(selected),
                );
            if let Some(handler) = handler {
                chip = chip.on_click(move |_, window, cx| handler(action, window, cx));
            }
            chip
        };
        div()
            .row()
            .items_center()
            .gap_token(&theme, Space::Sm)
            .p_token(&theme, Space::Xs)
            .radius(&theme, Radius::Small)
            .surface(&theme, Surface::Overlay)
            // The zoom is a reading, not a control: it wears the mono figures
            // and a rule separates it from the three chips beside it, so a
            // reader does not try to press it.
            .child(
                div()
                    .px_token(&theme, Space::Xs)
                    .font_family(theme.typography.mono.clone())
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.text_muted)
                    .child(self.zoom.clone()),
            )
            .child(
                div()
                    .w(px(theme.borders.hairline))
                    .h(px(theme.typography.caption.line_height))
                    .bg(theme.colors.divider),
            )
            .child(button(CanvasToolbarAction::Fit, false))
            .child(button(CanvasToolbarAction::Snap, self.snap))
            .child(button(CanvasToolbarAction::Arrange, false))
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group).value(self.zoom),
            )
    }
}
