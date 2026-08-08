use std::rc::Rc;

use gpui::{
    AnyElement, App, Div, FocusHandle, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlMetrics, ControlSize, Radius, Theme};

use crate::foundation::direction::{ActiveDirection, DirectionalExt, LayoutDirection};
use crate::foundation::{Disableable, FocusRing, Ident, Pressable, Selectable, Sizable, StyledExt};

/// How much weight an action carries. Primary is the one decision a local
/// area is asking for; Danger is reserved for irreversible intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
    Danger,
    Link,
}

/// Which side of the label the glyph sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconPosition {
    #[default]
    Leading,
    Trailing,
}

/// Where a button sits in a joined run of them.
///
/// A joined button gives up the radius on the side it touches its neighbour
/// and overlaps its border, so the run reads as one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonJoin {
    #[default]
    Alone,
    Leading,
    Middle,
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
    semantic_parent: Option<SharedString>,
    focus_handle: Option<FocusHandle>,
    label: Option<SharedString>,
    /// What the button is called when the label is not what it is called, or
    /// when there is no label at all.
    name: Option<SharedString>,
    description: Option<SharedString>,
    glyph: Option<Icon>,
    icon_position: IconPosition,
    variant: ButtonVariant,
    size: ControlSize,
    disabled: bool,
    selected: bool,
    checked: Option<bool>,
    loading: bool,
    full_width: bool,
    icon_only: bool,
    join: ButtonJoin,
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
            .field("selected", &self.selected)
            .field("loading", &self.loading)
            .field("has_handler", &self.on_click.is_some())
            .finish()
    }
}

impl Button {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            semantic_parent: None,
            focus_handle: None,
            label: None,
            name: None,
            description: None,
            glyph: None,
            icon_position: IconPosition::Leading,
            variant: ButtonVariant::default(),
            size: ControlSize::default(),
            disabled: false,
            selected: false,
            checked: None,
            loading: false,
            full_width: false,
            icon_only: false,
            join: ButtonJoin::Alone,
            on_click: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// What assistive technology and a test call this action.
    ///
    /// Overrides the label, which a graphic button does not have.
    pub fn accessible_name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Adds supplementary literal help to the native button node.
    pub fn accessible_description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Draws the button as a square carrying only its glyph, and names it.
    ///
    /// The name is required rather than optional because a glyph on its own
    /// is an action nobody can announce or address.
    pub fn icon_only(mut self, glyph: Icon, name: impl Into<SharedString>) -> Self {
        self.glyph = Some(glyph);
        self.label = None;
        self.icon_only = true;
        self.name = Some(name.into());
        self
    }

    /// Places the button in a joined run.
    pub fn join(mut self, join: ButtonJoin) -> Self {
        self.join = join;
        self
    }

    /// Names the surface this action belongs to in the semantic tree, so a
    /// reader can tell which notification or row an action came from.
    pub fn semantic_parent(mut self, parent: impl Into<SharedString>) -> Self {
        self.semantic_parent = Some(parent.into());
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

    /// Puts the button on a caller-owned focus handle.
    ///
    /// An overlay that keeps its own tab order needs a handle it can focus
    /// directly, and the published node then reports whether the keyboard is
    /// on this action.
    pub fn track_focus(mut self, handle: &FocusHandle) -> Self {
        self.focus_handle = Some(handle.clone());
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Publishes an explicit two-state answer, for a button that stays in.
    ///
    /// A selected button publishes `checked` only when it is selected, because
    /// "this is the current one" has no meaningful false. A toggle does: out
    /// is a state, not the absence of one, so it says so.
    pub fn checked_state(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    fn actionable(&self) -> bool {
        !self.disabled && !self.loading && self.on_click.is_some()
    }

    fn announced_name(&self) -> Option<SharedString> {
        self.name.clone().or_else(|| self.label.clone())
    }
}

impl Disableable for Button {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Selectable for Button {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
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
        let direction = cx.layout_direction();
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

        let mut button = frame(&theme, self.variant, metrics, inert, direction)
            .when(self.icon_only, |element| {
                element.w(px(metrics.height)).px(px(0.0))
            })
            .map(|element| joined(element, &theme, self.join, direction))
            .when(self.selected, |element| {
                element
                    .bg(theme.colors.selected)
                    .border_color(theme.colors.hairline_strong)
            })
            .id(self.ident.element_id())
            .when_some(self.focus_handle.clone(), |element, handle| {
                element.track_focus(&handle)
            })
            .role(gpui::Role::Button)
            .when(self.full_width, |element| element.w_full())
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .focus_ring(&theme)
                    .pressable(cx)
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
        if let Some(parent) = self.semantic_parent.clone() {
            spec = spec.parent(parent);
        }
        match self.checked {
            Some(checked) => spec = spec.checked(checked),
            None if self.selected => spec = spec.checked(true),
            None => {}
        }
        if let Some(handle) = &self.focus_handle {
            spec = spec.focus(handle);
        }
        if let Some(name) = self.announced_name() {
            spec = spec.text(name);
        }
        if let Some(description) = self.description {
            spec = spec.description(description);
        }
        button.semantic_in(cx, spec)
    }
}

/// Flattens the edges a joined button shares with its neighbour, and pulls it
/// onto that neighbour's border so the run carries one hairline, not two.
fn joined(element: Div, theme: &Theme, join: ButtonJoin, direction: LayoutDirection) -> Div {
    let flat = px(0.0);
    let overlap = px(-theme.borders.hairline);
    // Leading and trailing name places in a run, and a run is read rather
    // than measured: the first button keeps the corners on the side reading
    // starts at and gives up the ones it shares with the next.
    let start_flat = |element: Div| {
        if direction.is_rtl() {
            element.rounded_tr(flat).rounded_br(flat)
        } else {
            element.rounded_tl(flat).rounded_bl(flat)
        }
    };
    let end_flat = |element: Div| {
        if direction.is_rtl() {
            element.rounded_tl(flat).rounded_bl(flat)
        } else {
            element.rounded_tr(flat).rounded_br(flat)
        }
    };
    match join {
        ButtonJoin::Alone => element,
        ButtonJoin::Leading => end_flat(element),
        ButtonJoin::Middle => end_flat(start_flat(element.ms(direction, overlap))),
        ButtonJoin::Trailing => start_flat(element.ms(direction, overlap)),
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

fn frame(
    theme: &Theme,
    variant: ButtonVariant,
    metrics: ControlMetrics,
    inert: bool,
    direction: LayoutDirection,
) -> Div {
    // Leading and trailing are named for reading order, not for the screen,
    // so the frame runs the way the label does and the glyph stays on the
    // side of the label the caller asked for.
    let base = div()
        .row_reading(direction)
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

/// An action carried by a glyph alone.
///
/// The accessible name is a constructor argument rather than an option: an
/// icon with no name is an action neither a screen reader nor a test can
/// address, and there is no sensible default for what a picture means.
/// Everything else — tone, size, refusal, the action in flight — is
/// [`Button`]'s behaviour, reused rather than reimplemented.
#[derive(IntoElement)]
pub struct IconButton {
    button: Button,
}

impl std::fmt::Debug for IconButton {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IconButton")
            .field("button", &self.button)
            .finish()
    }
}

impl IconButton {
    pub fn new(ident: impl Into<Ident>, glyph: Icon, name: impl Into<SharedString>) -> Self {
        Self {
            button: Button::new(ident)
                .ghost()
                .icon_only(glyph, name)
                .icon_position(IconPosition::Leading),
        }
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.button = self.button.variant(variant);
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

    pub fn loading(mut self, loading: bool) -> Self {
        self.button = self.button.loading(loading);
        self
    }

    pub fn semantic_parent(mut self, parent: impl Into<SharedString>) -> Self {
        self.button = self.button.semantic_parent(parent);
        self
    }

    pub fn track_focus(mut self, handle: &FocusHandle) -> Self {
        self.button = self.button.track_focus(handle);
        self
    }

    pub fn join(mut self, join: ButtonJoin) -> Self {
        self.button = self.button.join(join);
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.button = self.button.on_click(handler);
        self
    }
}

impl Disableable for IconButton {
    fn disabled(mut self, disabled: bool) -> Self {
        self.button = self.button.disabled(disabled);
        self
    }
}

impl Selectable for IconButton {
    fn selected(mut self, selected: bool) -> Self {
        self.button = self.button.selected(selected);
        self
    }
}

impl Sizable for IconButton {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.button = self.button.control_size(size);
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.button
    }
}

/// Adjacent related actions sharing one frame.
///
/// The group reports nothing: every action still reports itself, and the
/// group only decides where the corners are. It publishes a `Group` node so
/// the actions inside it can be addressed as a set, and names each button as
/// its child.
#[derive(IntoElement)]
pub struct ButtonGroup {
    ident: Ident,
    buttons: Vec<Button>,
    size: ControlSize,
    disabled: bool,
}

impl std::fmt::Debug for ButtonGroup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ButtonGroup")
            .field("ident", &self.ident)
            .field("buttons", &self.buttons.len())
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl ButtonGroup {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            buttons: Vec::new(),
            size: ControlSize::default(),
            disabled: false,
        }
    }

    pub fn child(mut self, button: Button) -> Self {
        self.buttons.push(button);
        self
    }

    pub fn children(mut self, buttons: impl IntoIterator<Item = Button>) -> Self {
        self.buttons.extend(buttons);
        self
    }
}

impl Disableable for ButtonGroup {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for ButtonGroup {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for ButtonGroup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let last = self.buttons.len().saturating_sub(1);
        let parent = self.ident.semantic_id();
        let group_disabled = self.disabled;
        let size = self.size;
        let buttons = self
            .buttons
            .into_iter()
            .enumerate()
            .map(|(index, button)| {
                let join = match (index, last) {
                    (_, 0) => ButtonJoin::Alone,
                    (0, _) => ButtonJoin::Leading,
                    (index, last) if index == last => ButtonJoin::Trailing,
                    _ => ButtonJoin::Middle,
                };
                // One frame means one scale: a run of mismatched heights is
                // not a shared frame, it is a row of buttons.
                let button = button
                    .join(join)
                    .control_size(size)
                    .semantic_parent(parent.clone());
                if group_disabled {
                    button.disabled(true)
                } else {
                    button
                }
            })
            .collect::<Vec<_>>();

        div()
            .row_reading(cx.layout_direction())
            .flex_none()
            .children(buttons)
            .semantic_in(cx, NodeSpec::new(parent, Role::Toolbar))
    }
}
