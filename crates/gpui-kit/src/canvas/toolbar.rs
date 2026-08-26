//! Zoom, fit, and snap controls a canvas host already owns.
//!
//! The percentage is a finished host string. Each action reports; nothing
//! here pans, zooms, or rearranges nodes.

use std::rc::Rc;

use gpui::{App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Surface, TypeScale};

use crate::controls::button::Button;
use crate::foundation::{Disableable, Ident, Selectable, Sizable, StyledExt};
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
