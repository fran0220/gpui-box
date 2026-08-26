//! A number that counts to its new value.
//!
//! The count is decoration. What the component publishes — and therefore what
//! a test, a screen reader, or an audit reads — is the target, from the first
//! frame after it changes. A number in flight is not a fact, so
//! `the total is 1,204` can never race the animation.

use std::rc::Rc;

use gpui::{App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, TypeScale};

use crate::foundation::{Ident, StyledExt};
use crate::motion::{Easing, MotionSpec, Transition, keyed};
use crate::strings::ActiveNumbers;

type Format = Rc<dyn Fn(f64) -> String>;

/// How much larger a readout is drawn than prose at the same step.
///
/// A reading has to win over the caption naming it from across the room, and
/// the type scale is a prose ladder that stops at `Title` — the size a heading
/// inside a component takes. A number set at a heading's size beside a caption
/// is a heading, not a reading, which is why the step past the ladder lives
/// here rather than being borrowed from the top of it.
const READOUT_RATIO: f32 = 1.6;

#[derive(Default)]
struct Counter(Option<Transition<f32>>);

/// A numeric readout that animates between values.
#[derive(IntoElement)]
pub struct AnimatedNumber {
    ident: Ident,
    value: f64,
    format: Option<Format>,
    spec: Option<MotionSpec>,
    scale: TypeScale,
}

impl std::fmt::Debug for AnimatedNumber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let formatted = self
            .format
            .as_ref()
            .map_or_else(|| self.value.to_string(), |format| format(self.value));
        formatter
            .debug_struct("AnimatedNumber")
            .field("ident", &self.ident)
            .field("value", &self.value)
            .field("formatted", &formatted)
            .finish()
    }
}

impl AnimatedNumber {
    pub fn new(ident: impl Into<Ident>, value: f64) -> Self {
        Self {
            ident: ident.into(),
            value,
            format: None,
            spec: None,
            scale: TypeScale::Title,
        }
    }

    /// Decides the text for a value.
    ///
    /// The component never invents a grouping separator or a precision:
    /// how many decimals a quantity carries, and whether thousands are
    /// grouped, belong to whoever owns the quantity.
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Some(Rc::new(format));
        self
    }

    pub fn spec(mut self, spec: MotionSpec) -> Self {
        self.spec = Some(spec);
        self
    }

    pub fn type_scale(mut self, scale: TypeScale) -> Self {
        self.scale = scale;
        self
    }

    fn formatted(&self, value: f64, cx: &App) -> SharedString {
        match &self.format {
            Some(format) => SharedString::from(format(value)),
            None => cx.numbers().number(value),
        }
    }
}

impl RenderOnce for AnimatedNumber {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let spec = self.spec.unwrap_or_else(|| {
            MotionSpec::new(theme.motion.resize_ms, Easing::Standard.curve(&theme))
        });
        let target = self.value as f32;

        let counter = keyed::slot::<Counter>(
            &self.ident.semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let shown = {
            let mut counter = counter.borrow_mut();
            let mut transition = counter
                .0
                .unwrap_or_else(|| Transition::new(target, spec))
                .spec(spec);
            transition.set(target);
            let shown = transition.animate(window, cx);
            counter.0 = Some(transition);
            shown
        };

        // The published value is the target, and only the glyphs count.
        let announced = self.formatted(self.value, cx);
        let painted = self.formatted(shown as f64, cx);

        let step = theme.type_style(self.scale);
        div()
            .child(
                div()
                    .type_scale(&theme, self.scale)
                    .text_size(px(step.size * READOUT_RATIO))
                    .line_height(px(step.line_height * READOUT_RATIO))
                    .text_color(theme.colors.text)
                    .child(painted),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Status).value(announced),
            )
    }
}

/// Groups whole thousands with a separator, the format a count usually wants.
pub fn grouped(value: f64) -> String {
    let rounded = value.round() as i64;
    let negative = rounded < 0;
    let digits = rounded.abs().to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    if negative {
        format!("-{grouped}")
    } else {
        grouped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping_inserts_a_separator_every_three_digits() {
        assert_eq!(grouped(0.0), "0");
        assert_eq!(grouped(999.0), "999");
        assert_eq!(grouped(1204.0), "1,204");
        assert_eq!(grouped(1_204_000.0), "1,204,000");
        assert_eq!(grouped(-4200.0), "-4,200");
    }

    #[test]
    fn the_debug_view_reports_the_target_not_a_frame_of_it() {
        let number = AnimatedNumber::new("total", 1204.0).format(grouped);
        assert!(format!("{number:?}").contains("1,204"));
    }
}
