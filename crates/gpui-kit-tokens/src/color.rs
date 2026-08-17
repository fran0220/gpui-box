//! Color parsing, palette references, and contrast math.

use std::collections::BTreeMap;

use crate::TokenError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

/// A palette is free-form groups of steps, so a theme can name its own scales.
pub type Palette = BTreeMap<String, BTreeMap<String, String>>;

impl Color {
    /// Parses `#RRGGBB`, `#RRGGBBAA`, `{group.step}`, or `{group.step}/AA`.
    ///
    /// The alpha suffix stays hexadecimal so a reference reproduces the exact
    /// channel a literal would have produced.
    pub fn resolve(path: &str, value: &str, palette: &Palette) -> Result<Self, TokenError> {
        if let Some(rest) = value.strip_prefix('{') {
            let (reference, alpha) = match rest.split_once('}') {
                Some((reference, suffix)) => (reference, suffix.strip_prefix('/')),
                None => {
                    return Err(invalid(
                        path,
                        "palette reference is missing its closing brace",
                    ));
                }
            };
            let (group, step) = reference
                .split_once('.')
                .ok_or_else(|| invalid(path, "palette reference must be {group.step}"))?;
            let literal = palette
                .get(group)
                .and_then(|steps| steps.get(step))
                .ok_or_else(|| {
                    invalid(
                        path,
                        &format!("palette reference `{reference}` is not defined"),
                    )
                })?;
            let mut color = Self::parse(path, literal)?;
            if let Some(alpha) = alpha {
                if alpha.len() != 2 {
                    return Err(invalid(path, "alpha suffix must be two hexadecimal digits"));
                }
                color.alpha = u8::from_str_radix(alpha, 16)
                    .map_err(|_| invalid(path, "alpha suffix is not hexadecimal"))?
                    as f32
                    / 255.0;
            }
            return Ok(color);
        }
        Self::parse(path, value)
    }

    pub fn parse(path: &str, value: &str) -> Result<Self, TokenError> {
        let digits = value
            .strip_prefix('#')
            .ok_or_else(|| invalid(path, "expected #RRGGBB, #RRGGBBAA, or {palette.reference}"))?;
        if digits.len() != 6 && digits.len() != 8 {
            return Err(invalid(path, "expected six or eight hexadecimal digits"));
        }
        let channel = |range: std::ops::Range<usize>| {
            u8::from_str_radix(&digits[range], 16).map(|value| f32::from(value) / 255.0)
        };
        Ok(Self {
            red: channel(0..2).map_err(|_| invalid_color(path))?,
            green: channel(2..4).map_err(|_| invalid_color(path))?,
            blue: channel(4..6).map_err(|_| invalid_color(path))?,
            alpha: if digits.len() == 8 {
                channel(6..8).map_err(|_| invalid_color(path))?
            } else {
                1.0
            },
        })
    }

    /// WCAG relative luminance of an opaque color.
    pub fn luminance(self) -> f32 {
        fn linear(channel: f32) -> f32 {
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linear(self.red) + 0.7152 * linear(self.green) + 0.0722 * linear(self.blue)
    }

    /// CIE L\*, perceptual lightness from 0 to 100.
    ///
    /// This is the measure two backgrounds are compared by, and the WCAG
    /// ratio is not. That ratio adds 0.05 to both sides so that black text
    /// stays measurable, which compresses everything near black into almost
    /// no range at all: `#050505` against `#0a0a0a` reads 1.03:1, the same
    /// answer it would give for two colors nobody could tell apart, and so
    /// does a step that is plainly visible. L\* is uniform across the range,
    /// so one threshold means the same thing on a dark theme and a light one.
    pub fn lightness(self) -> f32 {
        const EPSILON: f32 = 216.0 / 24389.0;
        const KAPPA: f32 = 24389.0 / 27.0;
        let luminance = self.luminance();
        if luminance > EPSILON {
            116.0 * luminance.cbrt() - 16.0
        } else {
            KAPPA * luminance
        }
    }
}

/// Composites `foreground` over an opaque `background`.
pub fn over(foreground: Color, background: Color) -> Color {
    let alpha = foreground.alpha.clamp(0.0, 1.0);
    Color {
        red: foreground.red * alpha + background.red * (1.0 - alpha),
        green: foreground.green * alpha + background.green * (1.0 - alpha),
        blue: foreground.blue * alpha + background.blue * (1.0 - alpha),
        alpha: 1.0,
    }
}

/// WCAG contrast ratio, compositing any translucency onto the background.
pub fn contrast_ratio(foreground: Color, background: Color) -> f32 {
    let foreground = over(foreground, background).luminance();
    let background = background.luminance();
    let (lighter, darker) = if foreground >= background {
        (foreground, background)
    } else {
        (background, foreground)
    };
    (lighter + 0.05) / (darker + 0.05)
}

fn invalid(path: &str, message: &str) -> TokenError {
    TokenError::Invalid {
        path: path.into(),
        message: message.into(),
    }
}

fn invalid_color(path: &str) -> TokenError {
    invalid(path, "contains a non-hexadecimal color channel")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Palette {
        Palette::from([(
            "neutral".to_string(),
            BTreeMap::from([("900".to_string(), "#ffffff".to_string())]),
        )])
    }

    #[test]
    fn references_reproduce_literal_channels_exactly() {
        let referenced = Color::resolve("c", "{neutral.900}/24", &palette()).expect("reference");
        let literal = Color::parse("c", "#ffffff24").expect("literal");
        assert_eq!(referenced, literal);
    }

    #[test]
    fn unknown_references_fail_loudly() {
        let error = Color::resolve("c", "{neutral.404}", &palette()).expect_err("missing step");
        assert!(error.to_string().contains("neutral.404"));
    }

    #[test]
    fn contrast_matches_the_wcag_reference_pairs() {
        let white = Color::parse("w", "#ffffff").expect("white");
        let black = Color::parse("b", "#000000").expect("black");
        assert!((contrast_ratio(white, black) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(white, white) - 1.0).abs() < 0.01);
    }

    #[test]
    fn translucent_foregrounds_composite_before_comparison() {
        let background = Color::parse("bg", "#000000").expect("black");
        let ghost = Color::parse("fg", "#ffffff10").expect("faint white");
        assert!(contrast_ratio(ghost, background) < 2.0);
    }
}
