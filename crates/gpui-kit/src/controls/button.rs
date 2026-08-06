use std::rc::Rc;

use gpui::{
    AnyElement, App, Div, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlMetrics, ControlSize, Radius, Theme};

use crate::foundation::{Disableable, Ident, Sizable, StyledExt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
    Danger,
    Link,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconPosition {
    #[default]
    Leading,
    Trailing,
}

type ClickHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A labeled action.
///
/// The click handler is only installed when the button is enabled and not
/// loading, so an unavailable action cannot fire through a stray event.
#[derive(IntoElement)]
pub struct Button {
    ident: Ident,
    label: Option<SharedString>,
    glyph: Option<Icon>,
    icon_position: IconPosition,
    variant: ButtonVariant,
    size: ControlSize,
    disabled: bool,
    loading: bool,
    full_width: bool,
    on_click: Option<ClickHandler>,
}

impl std::fmt::Debug for Button {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Button")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("variant", &self.variant)
            .field("size", &self.size)
            .field("disabled", &self.disabled)
            .field("loading", &self.loading)
            .field("has_handler", &self.on_click.is_some())
            .finish()
    }
}

impl Button {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            glyph: None,
            icon_position: IconPosition::Leading,
            variant: ButtonVariant::default(),
            size: ControlSize::default(),
            disabled: false,
            loading: false,
            full_width: false,
            on_click: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn icon(mut self, glyph: Icon) -> Self {
        self.glyph = Some(glyph);
        self
    }

    pub fn icon_position(mut self, position: IconPosition) -> Self {
        self.icon_position = position;
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(ButtonVariant::Primary)
    }

    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Secondary)
    }

    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    pub fn danger(self) -> Self {
        self.variant(ButtonVariant::Danger)
    }

    pub fn link(self) -> Self {
        self.variant(ButtonVariant::Link)
    }

    /// Marks the action as in flight. A loading button is not actionable.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn full_width(mut self, full_width: bool) -> Self {
        self.full_width = full_width;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    fn actionable(&self) -> bool {
        !self.disabled && !self.loading && self.on_click.is_some()
    }

    fn accessible_name(&self) -> Option<SharedString> {
        self.label.clone()
    }
}

impl Disableable for Button {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Button {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let inert = self.disabled || self.loading;
        let actionable = self.actionable();

        let mut content: Vec<AnyElement> = Vec::new();
        let glyph = self.glyph.map(|glyph| {
            // SVG paint does not inherit the frame's text color, so the icon
            // has to name the variant foreground itself.
            icon(glyph)
                .size(px(metrics.icon_size))
                .flex_none()
                .text_color(foreground(&theme, self.variant))
                .into_any_element()
        });
        if let Some(glyph) = glyph {
            match self.icon_position {
                IconPosition::Leading => content.push(glyph),
                IconPosition::Trailing => content.insert(0, glyph),
            }
        }
        if let Some(label) = self.label.clone() {
            let label = div().flex_none().child(label).into_any_element();
            match self.icon_position {
                IconPosition::Leading => content.push(label),
                IconPosition::Trailing => content.insert(0, label),
            }
        }

        let mut button = frame(&theme, self.variant, metrics, inert)
            .id(self.ident.element_id())
            .role(gpui::Role::Button)
            .when(self.full_width, |element| element.w_full())
            .when(actionable, |element| {
                element.cursor_pointer().tab_index(0).focus(|style| {
                    style
                        .border_color(theme.colors.focus)
                        .shadow(theme.selected_ring())
                })
            })
            .children(content);

        if let (true, Some(handler)) = (actionable, self.on_click.clone()) {
            let on_click = Rc::clone(&handler);
            button
                .interactivity()
                .on_click(move |_, window, cx| on_click(window, cx));
            button
                .interactivity()
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        handler(window, cx);
                        cx.stop_propagation();
                    }
                });
        }

        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Button)
            .disabled(inert)
            .busy(self.loading);
        if let Some(name) = self.accessible_name() {
            spec = spec.text(name);
        }
        button.semantic_in(cx, spec)
    }
}

fn foreground(theme: &Theme, variant: ButtonVariant) -> Hsla {
    match variant {
        ButtonVariant::Primary => theme.colors.text_on_accent,
        ButtonVariant::Secondary => theme.colors.text,
        ButtonVariant::Ghost => theme.colors.text_muted,
        ButtonVariant::Danger => gpui::white(),
        ButtonVariant::Link => theme.colors.accent,
    }
}

fn frame(theme: &Theme, variant: ButtonVariant, metrics: ControlMetrics, inert: bool) -> Div {
    let base = div()
        .row()
        .justify_center()
        .flex_none()
        .h(px(metrics.height))
        .gap(px(metrics.gap))
        .px(px(metrics.padding_x))
        .radius(theme, Radius::Control)
        .border(px(theme.borders.hairline))
        .border_color(gpui::transparent_black())
        .text_size(px(metrics.font_size))
        .font_weight(FontWeight::MEDIUM)
        .when(inert, |element| element.opacity(theme.opacity.disabled));

    match variant {
        ButtonVariant::Primary => base
            .bg(theme.colors.text)
            .text_color(theme.colors.text_on_accent)
            .when(!inert, |element| element.hover(|style| style.opacity(0.9))),
        ButtonVariant::Secondary => base
            .bg(theme.colors.raised)
            .border_color(theme.colors.hairline)
            .text_color(theme.colors.text)
            .when(!inert, |element| {
                element.hover(|style| style.bg(theme.colors.hover))
            }),
        ButtonVariant::Ghost => base
            .text_color(theme.colors.text_muted)
            .when(!inert, |element| {
                element.hover(|style| style.bg(theme.colors.hover).text_color(theme.colors.text))
            }),
        ButtonVariant::Danger => base
            .bg(theme.colors.danger.opacity(0.8))
            .text_color(gpui::white())
            .when(!inert, |element| element.hover(|style| style.opacity(0.9))),
        ButtonVariant::Link => base
            .px(px(0.0))
            .text_color(theme.colors.accent)
            .when(!inert, |element| {
                element.hover(|style| style.text_color(theme.colors.accent_strong))
            }),
    }
}
