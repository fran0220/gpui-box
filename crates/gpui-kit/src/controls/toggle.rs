//! Controls that report a choice: checkbox, radio, and switch.
//!
//! All three read their state from the caller and report an intent. They never
//! hold the answer themselves, so a host that refuses a change simply does not
//! apply it, and the control keeps showing what is actually true.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, point, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Theme, TypeScale};

use crate::foundation::{
    Disableable, FocusRing, Ident, Pressable, Selectable, Sizable, StyledExt,
    text as foundation_text,
};
use crate::motion::{self, Interpolate};
use crate::reactive::Binding;

type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A box that reports one of on, off, or partly on.
///
/// The mixed state is for a parent whose children disagree; it is reported as
/// mixed rather than guessed into on or off.
#[derive(IntoElement)]
pub struct Checkbox {
    ident: Ident,
    label: Option<SharedString>,
    description: Option<SharedString>,
    checked: Option<bool>,
    disabled: bool,
    size: ControlSize,
    on_change: Option<ToggleHandler>,
}

impl std::fmt::Debug for Checkbox {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Checkbox")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("checked", &self.checked)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_change.is_some())
            .finish()
    }
}

impl Checkbox {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            description: None,
            checked: Some(false),
            disabled: false,
            size: ControlSize::Md,
            on_change: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Secondary text under the label, for a consequence the typist should
    /// know before choosing.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Neither on nor off, because the things this box stands for disagree.
    pub fn mixed(mut self) -> Self {
        self.checked = None;
        self
    }

    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Draws what the caller's [`Binding`] currently holds, and writes back
    /// through it.
    ///
    /// This is exactly [`Self::checked`] and [`Self::on_change`] against one
    /// caller-owned value, and nothing else changes: the box still draws what
    /// the caller says is true, so a caller that refuses a change still sees
    /// the checkmark stay where it was.
    pub fn bind(self, binding: &Binding<bool>, cx: &App) -> Self {
        let checked = binding.get(cx);
        let binding = binding.clone();
        self.checked(checked)
            .on_change(move |next, _window, cx| binding.set(cx, next))
    }

    fn actionable(&self) -> bool {
        !self.disabled && self.on_change.is_some()
    }
}

impl Disableable for Checkbox {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Checkbox {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let actionable = self.actionable();
        let side = px(metrics.icon_size);
        let on = self.checked.unwrap_or(false);
        let mixed = self.checked.is_none();

        // The two marks are tracked separately so mixed and checked can cross
        // over each other: the bar shrinks while the check draws, and the box
        // is never momentarily empty between the two states.
        let drawn = motion::tracked(
            &self.ident.semantic_id(),
            point(f32::from(u8::from(on)), f32::from(u8::from(mixed))),
            motion::state_change(&theme),
            window,
            cx,
        );
        let filled = drawn.x.max(drawn.y);

        let mark = div()
            .size(side)
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .relative()
            .radius(&theme, Radius::Small)
            .border(px(theme.borders.hairline))
            .border_color(
                theme
                    .colors
                    .hairline_strong
                    .lerp(theme.colors.accent, filled),
            )
            .bg(theme.colors.accent.opacity(filled))
            .when(drawn.y > 0.0, |element| {
                element.child(
                    div()
                        .absolute()
                        .w(side * 0.5 * drawn.y)
                        .h(px(theme.borders.thick))
                        .bg(theme.colors.text_on_accent),
                )
            })
            .when(drawn.x > 0.0, |element| {
                element.child(
                    div().absolute().opacity(drawn.x).child(
                        gpui_kit_assets::icon(gpui_kit_assets::Icon::Check)
                            .size(side * (0.4 + 0.4 * drawn.x))
                            .text_color(theme.colors.text_on_accent),
                    ),
                )
            });

        let next = !on;
        choice_row(
            &theme,
            cx,
            self.ident.clone(),
            mark.into_any_element(),
            self.label.clone(),
            self.description.clone(),
            metrics.font_size,
            self.disabled,
            actionable,
            self.on_change.clone().map(move |handler| {
                Rc::new(move |window: &mut Window, cx: &mut App| handler(next, window, cx))
                    as ActionHandler
            }),
        )
        .semantic_in(
            cx,
            spec(
                &self.ident,
                Role::Checkbox,
                self.label.clone(),
                self.disabled,
            )
            .tristate(self.checked),
        )
    }
}

/// One option in a set where exactly one can hold.
#[derive(IntoElement)]
pub struct Radio {
    ident: Ident,
    label: Option<SharedString>,
    description: Option<SharedString>,
    selected: bool,
    disabled: bool,
    size: ControlSize,
    on_select: Option<ActionHandler>,
}

impl std::fmt::Debug for Radio {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Radio")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_select.is_some())
            .finish()
    }
}

impl Radio {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            description: None,
            selected: false,
            disabled: false,
            size: ControlSize::Md,
            on_select: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn on_select(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Draws selected while the caller's value equals `value`, and writes
    /// `value` when it is picked.
    ///
    /// A radio is one answer out of several, so it binds a value rather than
    /// a flag: every button in the group binds the same [`Binding`] with its
    /// own value, and the exclusivity is the equality, not a rule this
    /// component enforces.
    pub fn bind_value<T>(self, binding: &Binding<T>, value: T, cx: &App) -> Self
    where
        T: Clone + PartialEq + 'static,
    {
        let selected = binding.get(cx) == value;
        let binding = binding.clone();
        self.selected(selected)
            .on_select(move |_window, cx| binding.set(cx, value.clone()))
    }
}

impl Disableable for Radio {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Selectable for Radio {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl Sizable for Radio {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Radio {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let actionable = !self.disabled && self.on_select.is_some();
        let side = px(metrics.icon_size);

        let drawn = motion::tracked(
            &self.ident.semantic_id(),
            f32::from(u8::from(self.selected)),
            motion::state_change(&theme),
            window,
            cx,
        );

        let mark = div()
            .size(side)
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .rounded_full()
            .border(px(theme.borders.hairline))
            .border_color(
                theme
                    .colors
                    .hairline_strong
                    .lerp(theme.colors.accent, drawn),
            )
            .when(drawn > 0.0, |element| {
                element.child(
                    div()
                        .size(side * 0.5 * drawn)
                        .rounded_full()
                        .bg(theme.colors.accent),
                )
            });

        choice_row(
            &theme,
            cx,
            self.ident.clone(),
            mark.into_any_element(),
            self.label.clone(),
            self.description.clone(),
            metrics.font_size,
            self.disabled,
            actionable,
            self.on_select.clone(),
        )
        .semantic_in(
            cx,
            spec(&self.ident, Role::Radio, self.label.clone(), self.disabled)
                .checked(self.selected),
        )
    }
}

/// A control that takes effect the moment it is flipped.
///
/// Use a switch for something that applies immediately, and a checkbox for
/// something that applies when a form is submitted.
#[derive(IntoElement)]
pub struct Switch {
    ident: Ident,
    label: Option<SharedString>,
    /// A name for a reader who has only the tree, when the words on screen
    /// belong to something else.
    name: Option<SharedString>,
    description: Option<SharedString>,
    on: bool,
    disabled: bool,
    invalid: bool,
    size: ControlSize,
    on_change: Option<ToggleHandler>,
}

impl std::fmt::Debug for Switch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Switch")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("on", &self.on)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_change.is_some())
            .finish()
    }
}

impl Switch {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            name: None,
            description: None,
            on: false,
            disabled: false,
            invalid: false,
            size: ControlSize::Md,
            on_change: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Names the switch without drawing the name.
    ///
    /// A switch at the right edge of a settings row is named by the row, and
    /// repeating those words beside the track would say everything twice. The
    /// name still has to reach the tree, because a control nobody can name is
    /// a control nobody can operate without looking at it.
    pub fn named(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Draws what the caller's [`Binding`] currently holds, and writes back
    /// through it. Sugar for [`Self::on`] and [`Self::on_change`].
    pub fn bind(self, binding: &Binding<bool>, cx: &App) -> Self {
        let on = binding.get(cx);
        let binding = binding.clone();
        self.on(on)
            .on_change(move |next, _window, cx| binding.set(cx, next))
    }
}

impl Disableable for Switch {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Switch {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let actionable = !self.disabled && self.on_change.is_some();
        let height = px(metrics.icon_size);
        let width = height * 1.8;
        let knob = height - px(4.0);

        // The knob is placed by margin rather than by a relative offset, so
        // the switch is the same size at every point of the slide.
        let drawn = motion::tracked(
            &self.ident.semantic_id(),
            f32::from(u8::from(self.on)),
            motion::state_change(&theme),
            window,
            cx,
        );

        let track = div()
            .w(width)
            .h(height)
            .flex_none()
            .flex()
            .items_center()
            .rounded_full()
            .p(px(theme.space(Space::Xxs)))
            .bg(theme
                .colors
                .hairline_strong
                .lerp(theme.colors.accent, drawn))
            .when(self.invalid, |track| {
                track
                    .border(px(theme.borders.hairline))
                    .border_color(theme.colors.danger)
            })
            .child(
                div()
                    .size(knob)
                    .rounded_full()
                    .bg(theme.colors.text_on_accent)
                    .ml((width - knob - px(4.0)) * drawn),
            );

        let next = !self.on;
        choice_row(
            &theme,
            cx,
            self.ident.clone(),
            track.into_any_element(),
            self.label.clone(),
            self.description.clone(),
            metrics.font_size,
            self.disabled,
            actionable,
            self.on_change.clone().map(move |handler| {
                Rc::new(move |window: &mut Window, cx: &mut App| handler(next, window, cx))
                    as ActionHandler
            }),
        )
        .semantic_in(
            cx,
            spec(
                &self.ident,
                Role::Switch,
                self.label.clone().or_else(|| self.name.clone()),
                self.disabled,
            )
            .invalid(self.invalid)
            .checked(self.on),
        )
    }
}

fn spec(ident: &Ident, role: Role, label: Option<SharedString>, disabled: bool) -> NodeSpec {
    let mut spec = NodeSpec::new(ident.semantic_id(), role).disabled(disabled);
    if let Some(label) = label {
        spec = spec.text(label);
    }
    spec
}

/// The shared frame: a mark, a label, and the whole row as the target.
///
/// The row is the hit area rather than the mark alone, because a small square
/// is a hard thing to hit and the label means the same thing.
#[allow(clippy::too_many_arguments)]
fn choice_row(
    theme: &Theme,
    cx: &App,
    ident: Ident,
    mark: AnyElement,
    label: Option<SharedString>,
    description: Option<SharedString>,
    font_size: f32,
    disabled: bool,
    actionable: bool,
    handler: Option<ActionHandler>,
) -> gpui::Stateful<gpui::Div> {
    let text_color: Hsla = if disabled {
        theme.colors.text_faint
    } else {
        theme.colors.text
    };

    let mut row = div()
        .id(ident.element_id())
        .flex()
        .flex_row()
        .items_start()
        .gap(px(theme.space(gpui_kit_theme::Space::Sm)))
        .when(disabled, |element| element.opacity(theme.opacity.disabled))
        .when(actionable, |element| {
            element
                .cursor_pointer()
                .tab_index(0)
                .focus_ring(theme)
                .pressable(cx)
        })
        .child(div().mt(px(theme.space(Space::Xxs) / 2.0)).child(mark))
        .when_some(label, |element, label| {
            element.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(theme.space(Space::Xxs)))
                    .child(
                        foundation_text(theme, TypeScale::Label, label)
                            .text_size(px(font_size))
                            .text_color(text_color),
                    )
                    .when_some(description, |element, description| {
                        element.child(
                            foundation_text(theme, TypeScale::Caption, description)
                                .text_tone(theme, gpui_kit_theme::TextTone::Muted),
                        )
                    }),
            )
        });

    if let (true, Some(handler)) = (actionable, handler) {
        let click = Rc::clone(&handler);
        row.interactivity()
            .on_click(move |_, window, cx| click(window, cx));
        row.interactivity().on_key_down(move |event, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                handler(window, cx);
                cx.stop_propagation();
            }
        });
    }

    row
}
