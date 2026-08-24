//! A colour the caller already owns, reported back when the typist changes it.
//!
//! The picker does not apply a theme, write a token, or remember a palette.
//! Presets and recents are host lists. The value is [`gpui::Hsla`]; hex is
//! derived from that value as syntax, not as a translated word.

use std::rc::Rc;

use gpui::{
    App, CursorStyle, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, hsla,
    linear_color_stop, linear_gradient_stops, prelude::FluentBuilder as _, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};

use crate::foundation::{Disableable, FocusRing, Ident, StyledExt};
use crate::layout::measure;
use crate::strings::{ActiveStrings, StringKey};

type ChangeHandler = Rc<dyn Fn(Hsla, &mut Window, &mut App)>;

/// A clickable colour chip. Reports the colour it was given; applies nothing.
#[derive(IntoElement)]
pub struct ColorSwatch {
    ident: Ident,
    color: Hsla,
    selected: bool,
    disabled: bool,
    on_click: Option<ChangeHandler>,
}

impl std::fmt::Debug for ColorSwatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ColorSwatch")
            .field("ident", &self.ident)
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl ColorSwatch {
    pub fn new(ident: impl Into<Ident>, color: Hsla) -> Self {
        Self {
            ident: ident.into(),
            color,
            selected: false,
            disabled: false,
            on_click: None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(Hsla, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Disableable for ColorSwatch {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for ColorSwatch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let hex = hex_of(self.color);
        let mut chip = div()
            .id(self.ident.element_id())
            .size(px(20.0))
            .flex_none()
            .radius(&theme, Radius::Small)
            .bg(self.color)
            .border(px(theme.borders.hairline))
            .border_color(if self.selected {
                theme.colors.accent
            } else {
                theme.colors.hairline_strong
            });
        if let (false, Some(handler)) = (self.disabled, self.on_click.clone()) {
            let color = self.color;
            chip = chip
                .cursor_pointer()
                .tab_index(0)
                .focus_ring(&theme)
                .on_click(move |_, window, cx| handler(color, window, cx));
        }
        chip.semantic_in(
            cx,
            NodeSpec::new(self.ident.semantic_id(), Role::Button)
                .text(hex)
                .selected(self.selected)
                .disabled(self.disabled),
        )
    }
}

/// Hue, saturation-brightness, optional opacity, and host-owned swatches.
#[derive(IntoElement)]
pub struct ColorPicker {
    ident: Ident,
    value: Hsla,
    alpha: bool,
    presets: Vec<Hsla>,
    recent: Vec<Hsla>,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl std::fmt::Debug for ColorPicker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ColorPicker")
            .field("ident", &self.ident)
            .field("alpha", &self.alpha)
            .field("presets", &self.presets.len())
            .field("recent", &self.recent.len())
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl ColorPicker {
    pub fn new(ident: impl Into<Ident>, value: Hsla) -> Self {
        Self {
            ident: ident.into(),
            value,
            alpha: false,
            presets: Vec::new(),
            recent: Vec::new(),
            disabled: false,
            on_change: None,
        }
    }

    pub fn alpha(mut self, alpha: bool) -> Self {
        self.alpha = alpha;
        self
    }

    pub fn presets(mut self, colors: impl IntoIterator<Item = Hsla>) -> Self {
        self.presets = colors.into_iter().collect();
        self
    }

    pub fn recent(mut self, colors: impl IntoIterator<Item = Hsla>) -> Self {
        self.recent = colors.into_iter().collect();
        self
    }

    pub fn on_change(mut self, handler: impl Fn(Hsla, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl Disableable for ColorPicker {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for ColorPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let hex = hex_of(self.value);
        let (hue, sat, val) = hsl_to_hsv(self.value);
        let report = self.on_change.clone().filter(|_| !self.disabled);

        let board = saturation_board(
            &self.ident,
            self.value,
            hue,
            sat,
            val,
            report.clone(),
            window,
            cx,
        );
        let hue_track = channel_track(
            &self.ident.child("hue"),
            cx.strings().text(StringKey::ColorHue),
            hue,
            hue_fill(),
            false,
            {
                let value = self.value;
                let report = report.clone();
                report.map(|handler| {
                    Rc::new(move |hue: f32, window: &mut Window, cx: &mut App| {
                        let color = hsv_to_hsla(hue, sat, val, value.a);
                        handler(color, window, cx);
                    }) as ChannelHandler
                })
            },
            &theme,
            window,
            cx,
        );
        let alpha_track = self.alpha.then(|| {
            channel_track(
                &self.ident.child("alpha"),
                cx.strings().text(StringKey::ColorAlpha),
                self.value.a,
                linear_gradient_stops(
                    90.0,
                    [
                        linear_color_stop(self.value.opacity(0.0), 0.0),
                        linear_color_stop(self.value.opacity(1.0), 1.0),
                    ],
                ),
                true,
                {
                    let value = self.value;
                    report.clone().map(|handler| {
                        Rc::new(move |alpha: f32, window: &mut Window, cx: &mut App| {
                            handler(value.opacity(alpha), window, cx);
                        }) as ChannelHandler
                    })
                },
                &theme,
                window,
                cx,
            )
        });

        let presets = swatch_row(
            &self.ident.child("presets"),
            cx.strings().text(StringKey::ColorPresets),
            &self.presets,
            self.value,
            self.disabled,
            report.clone(),
            window,
            cx,
        );
        let recent = swatch_row(
            &self.ident.child("recent"),
            cx.strings().text(StringKey::ColorRecent),
            &self.recent,
            self.value,
            self.disabled,
            report,
            window,
            cx,
        );

        div()
            .id(self.ident.element_id())
            .column()
            .w(px(220.0))
            .gap_token(&theme, Space::Sm)
            .child(
                div()
                    .row()
                    .items_center()
                    .gap_token(&theme, Space::Sm)
                    .child(
                        div()
                            .size(px(28.0))
                            .flex_none()
                            .radius(&theme, Radius::Small)
                            .bg(self.value)
                            .border(px(theme.borders.hairline))
                            .border_color(theme.colors.hairline_strong),
                    )
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Caption)
                            .text_color(theme.colors.text_muted)
                            .child(hex.clone()),
                    ),
            )
            .child(board)
            .child(hue_track)
            .children(alpha_track)
            .children(presets)
            .children(recent)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .text(cx.strings().text(StringKey::ColorHex))
                    .value(hex)
                    .disabled(self.disabled),
            )
    }
}

type ChannelHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;

#[allow(clippy::too_many_arguments)]
fn saturation_board(
    ident: &Ident,
    value: Hsla,
    hue: f32,
    sat: f32,
    val: f32,
    report: Option<ChangeHandler>,
    window: &Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let theme = cx.theme().clone();
    let board = ident.child("board");
    let measured = measure::cell(&board.semantic_id(), window, cx);
    let label = cx.strings().text(StringKey::ColorSaturation);
    let mut surface = div()
        .on_children_prepainted({
            let measured = Rc::clone(&measured);
            move |bounds, window, _| {
                if let Some(first) = bounds.first() {
                    measure::record(&measured, *first, window);
                }
            }
        })
        .id(board.element_id())
        .relative()
        .w_full()
        .h(px(120.0))
        .radius(&theme, Radius::Small)
        .overflow_hidden()
        .cursor(CursorStyle::Crosshair)
        .bg(hsv_to_hsla(hue, 1.0, 1.0, 1.0))
        .child(div().absolute().inset_0().bg(linear_gradient_stops(
            90.0,
            [
                linear_color_stop(hsla(0.0, 0.0, 1.0, 1.0), 0.0),
                linear_color_stop(hsla(0.0, 0.0, 1.0, 0.0), 1.0),
            ],
        )))
        .child(div().absolute().inset_0().bg(linear_gradient_stops(
            180.0,
            [
                linear_color_stop(hsla(0.0, 0.0, 0.0, 0.0), 0.0),
                linear_color_stop(hsla(0.0, 0.0, 0.0, 1.0), 1.0),
            ],
        )))
        .child(
            div()
                .absolute()
                .left(relative(sat.clamp(0.0, 1.0)))
                .top(relative((1.0 - val).clamp(0.0, 1.0)))
                .ml(px(-5.0))
                .mt(px(-5.0))
                .size(px(10.0))
                .rounded_full()
                .border(px(theme.borders.thick))
                .border_color(theme.colors.text_on_accent)
                .bg(value),
        );

    if let Some(handler) = report {
        let pick = {
            let measured = Rc::clone(&measured);
            let handler = Rc::clone(&handler);
            move |position: gpui::Point<gpui::Pixels>, window: &mut Window, cx: &mut App| {
                let bounds = measured.get();
                let width = f32::from(bounds.size.width).max(1.0);
                let height = f32::from(bounds.size.height).max(1.0);
                let sat = ((f32::from(position.x - bounds.origin.x)) / width).clamp(0.0, 1.0);
                let val =
                    1.0 - ((f32::from(position.y - bounds.origin.y)) / height).clamp(0.0, 1.0);
                handler(hsv_to_hsla(hue, sat, val, value.a), window, cx);
            }
        };
        let down = pick.clone();
        surface = surface
            .on_mouse_down_with_pointer_capture(MouseButton::Left, move |event, window, cx| {
                down(event.position, window, cx);
                cx.stop_propagation();
            })
            .on_mouse_move(move |event, window, cx| {
                if event.pressed_button == Some(MouseButton::Left) {
                    pick(event.position, window, cx);
                    cx.stop_propagation();
                }
            });
    }

    surface
        .semantic_in(
            cx,
            NodeSpec::new(board.semantic_id(), Role::Slider)
                .parent(ident.semantic_id())
                .text(label)
                .range(0.0, 1.0, sat),
        )
        .into_any_element()
}

/// How tall either channel track is. Both take it from here, so an alpha run
/// and a hue run are the same object at different jobs.
const TRACK_HEIGHT: f32 = 12.0;
const CHECKER: f32 = TRACK_HEIGHT / 2.0;
/// How many squares the checkerboard lays down before it runs out. The track
/// clips, so the count only has to exceed the widest picker.
const CHECKER_COLUMNS: usize = 64;

/// The surface that says "nothing is painted here".
///
/// A colour at zero alpha over a solid panel is indistinguishable from the
/// panel, so the alpha run needs something behind it that visibly does not
/// belong to the colour.
fn checkerboard(theme: &gpui_kit_theme::Theme) -> Vec<gpui::Div> {
    (0..2)
        .map(|row| {
            div()
                .absolute()
                .left_0()
                .top(px(row as f32 * CHECKER))
                .h(px(CHECKER))
                .flex()
                .flex_row()
                .children((0..CHECKER_COLUMNS).map(|column| {
                    div()
                        .w(px(CHECKER))
                        .h(px(CHECKER))
                        .flex_none()
                        .bg(if (row + column) % 2 == 0 {
                            theme.colors.raised
                        } else {
                            theme.colors.track
                        })
                }))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn channel_track(
    ident: &Ident,
    label: SharedString,
    value: f32,
    fill: gpui::Background,
    transparency: bool,
    report: Option<ChannelHandler>,
    theme: &gpui_kit_theme::Theme,
    window: &Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let measured = measure::cell(&ident.semantic_id(), window, cx);
    let mut track = div()
        .on_children_prepainted({
            let measured = Rc::clone(&measured);
            move |bounds, window, _| {
                if let Some(first) = bounds.first() {
                    measure::record(&measured, *first, window);
                }
            }
        })
        .id(ident.element_id())
        .relative()
        .w_full()
        .h(px(TRACK_HEIGHT))
        // The clipped part is the gradient, not the handle. Rounding the
        // whole thing cut the handle to a half-circle whenever the value sat
        // at either end of the run.
        .child(
            div()
                .absolute()
                .inset_0()
                .radius(theme, Radius::Small)
                .overflow_hidden()
                .when(transparency, |element| {
                    element.children(checkerboard(theme))
                })
                .child(div().absolute().inset_0().bg(fill)),
        )
        .child(
            div()
                .absolute()
                .top(px(-2.0))
                .left(relative(value.clamp(0.0, 1.0)))
                .ml(px(-5.0))
                .size(px(10.0))
                .rounded_full()
                .bg(theme.colors.raised)
                .border(px(theme.borders.hairline))
                .border_color(theme.colors.hairline_strong),
        );

    if let Some(handler) = report {
        let pick = {
            let measured = Rc::clone(&measured);
            let handler = Rc::clone(&handler);
            move |position: gpui::Point<gpui::Pixels>, window: &mut Window, cx: &mut App| {
                let bounds = measured.get();
                let width = f32::from(bounds.size.width).max(1.0);
                let next = ((f32::from(position.x - bounds.origin.x)) / width).clamp(0.0, 1.0);
                handler(next, window, cx);
            }
        };
        let down = pick.clone();
        track = track
            .cursor_pointer()
            .on_mouse_down_with_pointer_capture(MouseButton::Left, move |event, window, cx| {
                down(event.position, window, cx);
                cx.stop_propagation();
            })
            .on_mouse_move(move |event, window, cx| {
                if event.pressed_button == Some(MouseButton::Left) {
                    pick(event.position, window, cx);
                    cx.stop_propagation();
                }
            });
    }

    track
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Slider)
                .text(label)
                .range(0.0, 1.0, value),
        )
        .into_any_element()
}

fn hue_fill() -> gpui::Background {
    linear_gradient_stops(
        90.0,
        [
            linear_color_stop(hsla(0.00, 1.0, 0.5, 1.0), 0.00),
            linear_color_stop(hsla(0.16, 1.0, 0.5, 1.0), 0.16),
            linear_color_stop(hsla(0.33, 1.0, 0.5, 1.0), 0.33),
            linear_color_stop(hsla(0.50, 1.0, 0.5, 1.0), 0.50),
            linear_color_stop(hsla(0.66, 1.0, 0.5, 1.0), 0.66),
            linear_color_stop(hsla(0.83, 1.0, 0.5, 1.0), 0.83),
            linear_color_stop(hsla(1.00, 1.0, 0.5, 1.0), 1.00),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn swatch_row(
    ident: &Ident,
    label: SharedString,
    colors: &[Hsla],
    current: Hsla,
    disabled: bool,
    report: Option<ChangeHandler>,
    window: &mut Window,
    cx: &mut App,
) -> Option<gpui::AnyElement> {
    if colors.is_empty() {
        return None;
    }
    let theme = cx.theme().clone();
    Some(
        div()
            .column()
            .gap_token(&theme, Space::Xs)
            .child(
                div()
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.text_faint)
                    .child(label.clone()),
            )
            .child(
                div()
                    .row()
                    .flex_wrap()
                    .gap_token(&theme, Space::Xs)
                    .children(colors.iter().map(|color| {
                        let selected = colors_close(*color, current);
                        let mut swatch =
                            ColorSwatch::new(ident.child(hex_of(*color).as_ref()), *color)
                                .selected(selected)
                                .disabled(disabled);
                        if let Some(handler) = report.clone() {
                            swatch = swatch.on_click(move |color, window, cx| {
                                handler(color, window, cx);
                            });
                        }
                        swatch.render(window, cx).into_any_element()
                    })),
            )
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Group).text(label),
            )
            .into_any_element(),
    )
}

fn hex_of(color: Hsla) -> SharedString {
    let rgb = color.to_rgb();
    let digits = format!(
        "{:02X}{:02X}{:02X}",
        (rgb.r * 255.0).round() as u8,
        (rgb.g * 255.0).round() as u8,
        (rgb.b * 255.0).round() as u8
    );
    let mut hex = String::with_capacity(7);
    hex.push('#');
    hex.push_str(&digits);
    SharedString::from(hex)
}

fn hsl_to_hsv(color: Hsla) -> (f32, f32, f32) {
    let value = color.l + color.s * color.l.min(1.0 - color.l);
    let sat = if value <= f32::EPSILON {
        0.0
    } else {
        2.0 * (1.0 - color.l / value)
    };
    (
        color.h.rem_euclid(1.0),
        sat.clamp(0.0, 1.0),
        value.clamp(0.0, 1.0),
    )
}

fn hsv_to_hsla(hue: f32, sat: f32, val: f32, alpha: f32) -> Hsla {
    let lightness = val * (1.0 - sat / 2.0);
    let saturation = if lightness <= f32::EPSILON || lightness >= 1.0 - f32::EPSILON {
        0.0
    } else {
        (val - lightness) / lightness.min(1.0 - lightness)
    };
    hsla(
        hue.rem_euclid(1.0),
        saturation.clamp(0.0, 1.0),
        lightness.clamp(0.0, 1.0),
        alpha.clamp(0.0, 1.0),
    )
}

fn colors_close(left: Hsla, right: Hsla) -> bool {
    (left.h - right.h).abs() < 0.02
        && (left.s - right.s).abs() < 0.04
        && (left.l - right.l).abs() < 0.04
        && (left.a - right.a).abs() < 0.04
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_derived_from_the_hsla_value() {
        let red = hsla(0.0, 1.0, 0.5, 1.0);
        assert_eq!(hex_of(red).as_ref(), "#FF0000");
    }

    #[test]
    fn hsv_round_trips_a_saturated_colour() {
        let color = hsv_to_hsla(0.6, 0.8, 0.9, 1.0);
        let (hue, sat, val) = hsl_to_hsv(color);
        assert!((hue - 0.6).abs() < 0.02);
        assert!((sat - 0.8).abs() < 0.05);
        assert!((val - 0.9).abs() < 0.05);
    }
}
