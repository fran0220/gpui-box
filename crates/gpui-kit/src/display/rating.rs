//! A scalar, keyboard-accessible rating.
//!
//! Unlike `FeedbackRating`, which records a helpful vote, `Rating` represents
//! a bounded numeric value. The value remains caller-owned; every pointer or
//! keyboard gesture reports an intent and leaves the displayed value alone.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TypeScale};

use crate::controls::button::IconButton;
use crate::foundation::{Disableable, Ident, Sizable, StyledExt, text};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type ChangeHandler = Rc<dyn Fn(Option<f32>, &mut Window, &mut App)>;

/// The smallest step a rating can report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RatingPrecision {
    #[default]
    Whole,
    Half,
}

impl RatingPrecision {
    fn step(self) -> f32 {
        match self {
            Self::Whole => 1.0,
            Self::Half => 0.5,
        }
    }
}

/// A controlled scalar rating rendered as token-sized stars.
#[derive(IntoElement)]
pub struct Rating {
    ident: Ident,
    label: Option<SharedString>,
    value: Option<f32>,
    maximum: usize,
    precision: RatingPrecision,
    size: ControlSize,
    disabled: bool,
    clearable: bool,
    on_change: Option<ChangeHandler>,
}

impl std::fmt::Debug for Rating {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Rating")
            .field("ident", &self.ident)
            .field("value", &self.value)
            .field("maximum", &self.maximum)
            .field("precision", &self.precision)
            .finish()
    }
}

impl Rating {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            value: None,
            maximum: 5,
            precision: RatingPrecision::Whole,
            size: ControlSize::Md,
            disabled: false,
            clearable: false,
            on_change: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the current value. `None` is an unrated value.
    pub fn value(mut self, value: Option<f32>) -> Self {
        self.value = value.map(|value| value.clamp(0.0, self.maximum as f32));
        self
    }

    pub fn maximum(mut self, maximum: usize) -> Self {
        self.maximum = maximum.max(1);
        self.value = self
            .value
            .map(|value| value.clamp(0.0, self.maximum as f32));
        self
    }

    pub fn precision(mut self, precision: RatingPrecision) -> Self {
        self.precision = precision;
        self
    }

    pub fn clearable(mut self, clearable: bool) -> Self {
        self.clearable = clearable;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(Option<f32>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    fn actionable(&self) -> bool {
        !self.disabled && self.on_change.is_some()
    }
}

impl Disableable for Rating {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Rating {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Rating {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let value = self.value.unwrap_or(0.0);
        let step = self.precision.step();
        let maximum = self.maximum as f32;
        let actionable = self.actionable();
        let star_size = px(theme.type_style(TypeScale::Title).line_height);
        let mut stars = div()
            .row()
            .items_center()
            .gap(px(theme.space(Space::Xxs)))
            .when(actionable, |element| element.cursor_pointer());

        for index in 1..=self.maximum {
            let star_value = index as f32;
            let filled = value >= star_value;
            let half =
                !filled && self.precision == RatingPrecision::Half && value >= star_value - 0.5;
            let star_id = self.ident.child(format!("value-{index}"));
            let mut star = div()
                .id(star_id.element_id())
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .w(star_size)
                .h(star_size)
                .child(
                    icon(Icon::Star)
                        .size(star_size)
                        .text_color(theme.colors.hairline_strong),
                );
            if filled {
                star = star.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            icon(Icon::StarFilled)
                                .size(star_size)
                                .text_color(theme.colors.warning),
                        ),
                );
            } else if half {
                star = star.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(gpui::relative(0.5))
                        .overflow_hidden()
                        .flex()
                        .items_center()
                        .justify_start()
                        .child(
                            icon(Icon::StarFilled)
                                .size(star_size)
                                .text_color(theme.colors.warning),
                        ),
                );
            }
            if actionable {
                let handler = self.on_change.clone();
                if self.precision == RatingPrecision::Half {
                    let left_handler = handler.clone();
                    star = star.child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .bottom_0()
                            .w(gpui::relative(0.5))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                if let Some(handler) = &left_handler {
                                    handler(Some((star_value - 0.5).max(0.5)), window, cx);
                                }
                            }),
                    );
                    let right_handler = handler;
                    star = star.child(
                        div()
                            .absolute()
                            .right_0()
                            .top_0()
                            .bottom_0()
                            .w(gpui::relative(0.5))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                if let Some(handler) = &right_handler {
                                    handler(Some(star_value), window, cx);
                                }
                            }),
                    );
                } else {
                    let handler = handler;
                    star = star.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        if let Some(handler) = &handler {
                            handler(Some(star_value.min(maximum)), window, cx);
                        }
                    });
                }
            }
            stars = stars.child(
                star.semantic_in(
                    cx,
                    NodeSpec::new(star_id.semantic_id(), Role::Button)
                        .parent(self.ident.semantic_id())
                        .text(cx.strings().format(
                            StringKey::RatingStar,
                            &[cx.numbers().decimal(f64::from(star_value), 1).as_ref()],
                        ))
                        .selected(filled || half)
                        .disabled(self.disabled),
                ),
            );
        }

        let ident = self.ident.clone();
        let current = self.value;
        let clear = self.clearable && current.is_some() && actionable;
        let mut frame = div()
            .id(ident.element_id())
            .row()
            .items_center()
            .gap(px(theme.space(Space::Sm)))
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .child(stars)
            .when_some(self.label.clone(), |element, label| {
                element.child(text(&theme, TypeScale::Label, label))
            })
            .when(clear, |element| {
                let handler = self.on_change.clone();
                element.child(
                    IconButton::new(
                        ident.child("clear"),
                        gpui_kit_assets::Icon::Close,
                        cx.strings().text(StringKey::RatingClear),
                    )
                    .ghost()
                    .on_click(move |window, cx| {
                        if let Some(handler) = &handler {
                            handler(None, window, cx);
                        }
                    }),
                )
            })
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Slider)
                    .text(self.label.clone().unwrap_or_default())
                    .range(0.0, maximum, value)
                    .value(if self.value.is_some() {
                        cx.strings().format(
                            StringKey::RatingValue,
                            &[
                                cx.numbers()
                                    .decimal(
                                        f64::from(value),
                                        if self.precision == RatingPrecision::Half {
                                            1
                                        } else {
                                            0
                                        },
                                    )
                                    .as_ref(),
                                cx.numbers().decimal(f64::from(maximum), 0).as_ref(),
                            ],
                        )
                    } else {
                        cx.strings().text(StringKey::RatingUnrated)
                    })
                    .disabled(self.disabled),
            );

        if actionable {
            let handler = self.on_change.clone();
            let clearable = self.clearable;
            frame = frame.on_key_down(move |event: &KeyDownEvent, window, cx| {
                let current = current.unwrap_or(0.0);
                let next = match event.keystroke.key.as_str() {
                    "left" | "down" => Some((current - step).max(0.0)),
                    "right" | "up" => Some((current + step).min(maximum)),
                    "home" => Some(0.0),
                    "end" => Some(maximum),
                    "space" | "backspace" if clearable => None,
                    _ => return,
                };
                if let Some(handler) = &handler {
                    handler(next, window, cx);
                }
                cx.stop_propagation();
            });
        }
        frame
    }
}
