//! A control for choosing a number inside a known range.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, MouseButton, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TypeScale};

use crate::foundation::direction::{ActiveDirection, DirectionalExt, LayoutDirection};
use crate::foundation::{
    Disableable, FocusRing, Ident, Sizable, StyledExt, text as foundation_text,
};
use crate::layout::measure;
use crate::motion::{self, keyed};

/// Set by the slider's own pointer handlers, and cleared by the render that
/// reads it.
///
/// A value the pointer is holding must be exactly under the pointer, so the
/// spring is skipped for a change this slider caused itself and used for one
/// that arrived from anywhere else.
#[derive(Default)]
struct PointerDriven(bool);

type ChangeHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;
type RangeHandler = Rc<dyn Fn(f32, f32, &mut Window, &mut App)>;

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
    size: ControlSize,
    disabled: bool,
    /// Rendered next to the label, for a unit the number alone does not carry.
    display: Option<SharedString>,
    /// The high end of a range slider. `None` is a single handle.
    high: Option<f32>,
    /// Tick marks on the track, as values on the same range as the handle.
    marks: Vec<f32>,
    on_change: Option<ChangeHandler>,
    on_range_change: Option<RangeHandler>,
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
            .field("high", &self.high)
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
            size: ControlSize::Md,
            disabled: false,
            display: None,
            high: None,
            marks: Vec::new(),
            on_change: None,
            on_range_change: None,
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

    /// A second handle. The fill then sits between the two values rather than
    /// from the start of the track, and [`Slider::on_range_change`] reports
    /// both ends.
    pub fn high(mut self, high: f32) -> Self {
        self.high = Some(high);
        self
    }

    pub fn values(self, low: f32, high: f32) -> Self {
        self.value(low).high(high)
    }

    /// Tick marks on the track. Values outside the range are skipped rather
    /// than drawn off the ends.
    pub fn marks(mut self, marks: impl IntoIterator<Item = f32>) -> Self {
        self.marks = marks.into_iter().collect();
        self
    }

    /// Reports both ends of a range slider. A single-handle slider ignores it.
    pub fn on_range_change(
        mut self,
        handler: impl Fn(f32, f32, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_range_change = Some(Rc::new(handler));
        self
    }

    fn clamped(&self) -> f32 {
        self.value.clamp(self.min, self.max)
    }

    fn fraction(&self) -> f32 {
        (self.clamped() - self.min) / (self.max - self.min)
    }

    fn clamped_high(&self) -> Option<f32> {
        self.high.map(|high| high.clamp(self.clamped(), self.max))
    }

    fn high_fraction(&self) -> Option<f32> {
        self.clamped_high()
            .map(|high| (high - self.min) / (self.max - self.min))
    }
}

impl Disableable for Slider {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Slider {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let metrics = theme.control.get(self.size);
        let actionable =
            !self.disabled && (self.on_change.is_some() || self.on_range_change.is_some());
        let dragging = keyed::slot::<PointerDriven>(
            &self.ident.semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let snap = std::mem::take(&mut dragging.borrow_mut().0);
        let fraction = motion::tracked_or_snap(
            &self.ident.semantic_id(),
            self.fraction(),
            motion::tracking(&theme),
            snap,
            window,
            cx,
        );
        let high_fraction = self.high_fraction().map(|high| {
            motion::tracked_or_snap(
                &self.ident.child("high").semantic_id(),
                high,
                motion::tracking(&theme),
                snap,
                window,
                cx,
            )
        });
        let physical_fraction = directed_fraction(fraction, direction);
        let physical_high = high_fraction.map(|high| directed_fraction(high, direction));
        let track_height = px(4.0);
        // The handle is the control's only tappable part, so it is sized from
        // the same scale step the other controls take their glyphs from.
        let knob = px(metrics.icon_size);

        // The track is the assertion target, not the row: an automated click
        // on the centre of a slider has to land on something draggable.
        let track_id = self.ident.clone();
        // The handlers need the track's measured width to turn a pointer
        // position into a value, and only prepaint knows it.
        let measured = measure::cell(&track_id.semantic_id(), window, cx);
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
                    .bg(theme.colors.track),
            )
            .children(self.marks.iter().filter_map(|mark| {
                if *mark < self.min || *mark > self.max {
                    return None;
                }
                let at = directed_fraction((*mark - self.min) / (self.max - self.min), direction);
                Some(
                    div()
                        .absolute()
                        .left(gpui::relative(at))
                        .ml(px(-1.0))
                        .w(px(2.0))
                        .h(px(8.0))
                        .rounded_full()
                        .bg(theme.colors.hairline_strong),
                )
            }))
            .child(if let Some(high) = physical_high {
                let start = physical_fraction.min(high);
                let span = (physical_fraction - high).abs();
                div()
                    .absolute()
                    .left(gpui::relative(start))
                    .w(gpui::relative(span))
                    .h(track_height)
                    .rounded_full()
                    .bg(theme.colors.accent)
            } else {
                div()
                    .absolute()
                    .left_0()
                    .w(gpui::relative(physical_fraction))
                    .h(track_height)
                    .rounded_full()
                    .bg(theme.colors.accent)
            })
            .child(knob_at(physical_fraction, knob, &theme))
            .children(physical_high.map(|high| knob_at(high, knob, &theme)));

        if actionable {
            let (min, max, step) = (self.min, self.max, self.step);
            let low = self.clamped();
            let high = self.clamped_high();
            let on_change = self.on_change.clone();
            let on_range = self.on_range_change.clone();
            let report: ChangeHandler = Rc::new(move |value, window, cx| {
                if let (Some(high), Some(range)) = (high, on_range.clone()) {
                    let toward_high = (value - high).abs() < (value - low).abs();
                    let (next_low, next_high) = if toward_high {
                        (low, value.max(low))
                    } else {
                        (value.min(high), high)
                    };
                    range(next_low, next_high, window, cx);
                } else if let Some(handler) = on_change.clone() {
                    handler(value, window, cx);
                }
            });
            let down = Rc::clone(&report);
            let down_bounds = Rc::clone(&measured);
            let down_dragging = Rc::clone(&dragging);
            track = track.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                let bounds = down_bounds.get();
                let width = f32::from(bounds.size.width);
                if width <= 0.0 {
                    return;
                }
                down_dragging.borrow_mut().0 = true;
                let fraction = directed_fraction(
                    (f32::from(event.position.x - bounds.left()) / width).clamp(0.0, 1.0),
                    direction,
                );
                down(
                    quantize(min + fraction * (max - min), min, max, step),
                    window,
                    cx,
                );
            });

            let drag = Rc::clone(&report);
            let move_bounds = Rc::clone(&measured);
            let move_dragging = Rc::clone(&dragging);
            track = track.on_mouse_move(move |event, window, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                let bounds = move_bounds.get();
                let width = f32::from(bounds.size.width);
                if width <= 0.0 {
                    return;
                }
                move_dragging.borrow_mut().0 = true;
                let fraction = directed_fraction(
                    (f32::from(event.position.x - bounds.left()) / width).clamp(0.0, 1.0),
                    direction,
                );
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
                element.tab_index(0).focus_ring(&theme)
            })
            .when_some(self.label.clone(), |element, label| {
                element.child(
                    div()
                        .row_reading(direction)
                        .justify_between()
                        .child(
                            foundation_text(&theme, TypeScale::Label, label)
                                .text_size(px(metrics.font_size))
                                .text_tone(&theme, gpui_kit_theme::TextTone::Muted),
                        )
                        .when_some(self.display.clone(), |element, display| {
                            element.child(
                                foundation_text(&theme, TypeScale::Label, display)
                                    .text_size(px(metrics.font_size)),
                            )
                        }),
                )
            })
            .child(
                div()
                    .w_full()
                    .on_children_prepainted({
                        let measured = Rc::clone(&measured);
                        move |bounds, window, _| {
                            if let Some(first) = bounds.first() {
                                measure::record(&measured, *first, window);
                            }
                        }
                    })
                    .child(track)
                    .semantic_in(cx, spec),
            );

        if actionable && let Some(handler) = self.on_change.clone() {
            let (min, max) = (self.min, self.max);
            frame.interactivity().on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.as_str();
                let next = match direction.arrow_step(key) {
                    Some(1) => value + keyboard_step,
                    Some(_) => value - keyboard_step,
                    None => match key {
                        "down" => value - keyboard_step,
                        "up" => value + keyboard_step,
                        "home" => min,
                        "end" => max,
                        _ => return,
                    },
                };
                handler(next.clamp(min, max), window, cx);
                cx.stop_propagation();
            });
        }

        frame
    }
}

fn knob_at(fraction: f32, knob: gpui::Pixels, theme: &gpui_kit_theme::Theme) -> gpui::Div {
    div()
        .absolute()
        .left(gpui::relative(fraction))
        .ml(-(knob / 2.0))
        .size(knob)
        .rounded_full()
        .bg(theme.colors.text)
        .border(px(theme.borders.hairline))
        .border_color(theme.colors.hairline_strong)
}

/// Converts between a logical fraction and a physical left-origin fraction.
/// The operation is its own inverse, so pointer input and paint use one rule.
fn directed_fraction(fraction: f32, direction: LayoutDirection) -> f32 {
    if direction.is_rtl() {
        1.0 - fraction
    } else {
        fraction
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn right_to_left_values_start_at_the_right_edge() {
        assert_eq!(directed_fraction(0.0, LayoutDirection::RightToLeft), 1.0);
        assert_eq!(directed_fraction(1.0, LayoutDirection::RightToLeft), 0.0);
        assert_eq!(directed_fraction(0.25, LayoutDirection::LeftToRight), 0.25);
    }
}
