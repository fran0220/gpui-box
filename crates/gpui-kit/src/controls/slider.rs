//! A control for choosing a number inside a known range.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, Bounds, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space};

use crate::foundation::{Disableable, Ident};

type ChangeHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;

/// A horizontal track with one handle.
///
/// The value is caller-owned: the slider reports where the typist pointed and
/// renders whatever the caller decides, so a rejected or clamped change is
/// visible as the value not moving.
#[derive(IntoElement)]
pub struct Slider {
    ident: Ident,
    label: Option<SharedString>,
    min: f32,
    max: f32,
    value: f32,
    step: Option<f32>,
    disabled: bool,
    /// Rendered next to the label, for a unit the number alone does not carry.
    display: Option<SharedString>,
    on_change: Option<ChangeHandler>,
}

impl std::fmt::Debug for Slider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Slider")
            .field("ident", &self.ident)
            .field("range", &(self.min, self.max))
            .field("value", &self.value)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_change.is_some())
            .finish()
    }
}

impl Slider {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            min: 0.0,
            max: 1.0,
            value: 0.0,
            step: None,
            disabled: false,
            display: None,
            on_change: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The bounds the value lives in. A reversed or empty range is corrected
    /// here rather than producing a handle at an arbitrary position.
    pub fn range(mut self, min: f32, max: f32) -> Self {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        self.min = min;
        self.max = if (max - min).abs() < f32::EPSILON {
            min + 1.0
        } else {
            max
        };
        self
    }

    pub fn value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }

    /// Rounds every reported value to a multiple of `step`.
    pub fn step(mut self, step: f32) -> Self {
        self.step = (step > 0.0).then_some(step);
        self
    }

    /// What to show for the current value, such as `"70%"`.
    pub fn display(mut self, display: impl Into<SharedString>) -> Self {
        self.display = Some(display.into());
        self
    }

    pub fn on_change(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    fn clamped(&self) -> f32 {
        self.value.clamp(self.min, self.max)
    }

    fn fraction(&self) -> f32 {
        (self.clamped() - self.min) / (self.max - self.min)
    }
}

impl Disableable for Slider {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for Slider {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let actionable = !self.disabled && self.on_change.is_some();
        let fraction = self.fraction();
        let track_height = px(4.0);
        let knob = px(14.0);

        // The track is the assertion target, not the row: an automated click
        // on the centre of a slider has to land on something draggable.
        let track_id = self.ident.clone();
        // The handlers need the track's measured width to turn a pointer
        // position into a value, and only prepaint knows it.
        let measured: Rc<Cell<Bounds<Pixels>>> = Rc::new(Cell::new(Bounds::default()));
        let mut track = div()
            .id(track_id.element_id())
            .relative()
            .w_full()
            .h(knob)
            .flex()
            .items_center()
            .when(actionable, |element| element.cursor_pointer())
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .h(track_height)
                    .rounded_full()
                    .bg(theme.colors.hairline_strong),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .w(gpui::relative(fraction))
                    .h(track_height)
                    .rounded_full()
                    .bg(theme.colors.accent),
            )
            .child(
                div()
                    .absolute()
                    // The knob is centred on the value, so its own width is
                    // taken out of the offset rather than pushing the handle
                    // past the end of the track.
                    .left(gpui::relative(fraction))
                    .ml(-(knob / 2.0))
                    .size(knob)
                    .rounded_full()
                    .bg(theme.colors.text)
                    .border(px(theme.borders.hairline))
                    .border_color(theme.colors.hairline_strong),
            );

        if actionable && let Some(handler) = self.on_change.clone() {
            let (min, max, step) = (self.min, self.max, self.step);
            let down = Rc::clone(&handler);
            let down_bounds = Rc::clone(&measured);
            track = track.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                let bounds = down_bounds.get();
                let width = f32::from(bounds.size.width);
                if width <= 0.0 {
                    return;
                }
                let fraction =
                    (f32::from(event.position.x - bounds.left()) / width).clamp(0.0, 1.0);
                down(
                    quantize(min + fraction * (max - min), min, max, step),
                    window,
                    cx,
                );
            });

            let drag = Rc::clone(&handler);
            // Dragging is tracked as movement with the button held over the
            // track, which is what a mouse reports without a captured drag
            // payload. Leaving the track ends the drag.
            let move_bounds = Rc::clone(&measured);
            track = track.on_mouse_move(move |event, window, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                let bounds = move_bounds.get();
                let width = f32::from(bounds.size.width);
                if width <= 0.0 {
                    return;
                }
                let fraction =
                    (f32::from(event.position.x - bounds.left()) / width).clamp(0.0, 1.0);
                drag(
                    quantize(min + fraction * (max - min), min, max, step),
                    window,
                    cx,
                );
            });
        }

        let value = self.clamped();
        let keyboard_step = self.step.unwrap_or((self.max - self.min) / 20.0);
        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Slider)
            .disabled(self.disabled)
            .range(self.min, self.max, value);
        if let Some(label) = self.label.clone() {
            spec = spec.text(label);
        }
        if let Some(display) = self.display.clone() {
            spec = spec.value(display);
        }

        let mut frame = div()
            .id(self.ident.child("frame").element_id())
            .flex()
            .flex_col()
            .gap(px(theme.space(Space::Xs)))
            .w_full()
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .when(actionable, |element| {
                element.tab_index(0).focus(|style| {
                    style
                        .border_color(theme.colors.focus)
                        .shadow(theme.selected_ring())
                })
            })
            .when_some(self.label.clone(), |element, label| {
                element.child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .text_size(px(theme.typography.body.size))
                        .text_color(theme.colors.text_muted)
                        .child(label)
                        .when_some(self.display.clone(), |element, display| {
                            element.child(div().text_color(theme.colors.text).child(display))
                        }),
                )
            })
            .child(
                div()
                    .w_full()
                    .semantic_in(cx, spec)
                    .on_children_prepainted({
                        let measured = Rc::clone(&measured);
                        move |bounds, _, _| {
                            if let Some(first) = bounds.first() {
                                measured.set(*first);
                            }
                        }
                    })
                    .child(track),
            );

        if actionable && let Some(handler) = self.on_change.clone() {
            let (min, max) = (self.min, self.max);
            frame.interactivity().on_key_down(move |event, window, cx| {
                let next = match event.keystroke.key.as_str() {
                    "left" | "down" => value - keyboard_step,
                    "right" | "up" => value + keyboard_step,
                    "home" => min,
                    "end" => max,
                    _ => return,
                };
                handler(next.clamp(min, max), window, cx);
                cx.stop_propagation();
            });
        }

        frame
    }
}

/// Rounds a value onto the step grid, so a reported value is one the caller
/// could have produced itself.
fn quantize(value: f32, min: f32, max: f32, step: Option<f32>) -> f32 {
    let value = value.clamp(min, max);
    match step {
        Some(step) => {
            let steps = ((value - min) / step).round();
            (min + steps * step).clamp(min, max)
        }
        None => value,
    }
}
