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

/// A colour in OKLab, the perceptual space this crate reasons about hue and
/// chroma in.
///
/// CIE L\* answers how light two colours are relative to each other, which is
/// what stacking surfaces and reading text need. It says nothing about whether
/// two colours of the same lightness are far enough apart to be told apart,
/// and that is the question a categorical scale asks. OKLab answers it: equal
/// distances in it are approximately equal perceived differences, across the
/// whole gamut and in both appearances, which is what lets one threshold mean
/// the same thing on a dark theme and a light one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklab {
    /// Perceptual lightness, 0 at black and 1 at white.
    pub lightness: f32,
    /// The green-red axis.
    pub a: f32,
    /// The blue-yellow axis.
    pub b: f32,
}

/// The same colour in cylindrical form, which is how a scale is authored:
/// pick a lightness and a chroma, then walk the hue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    /// Perceptual lightness, 0 at black and 1 at white.
    pub lightness: f32,
    /// Distance from grey. Zero is neutral; roughly 0.37 is the most any
    /// sRGB colour reaches.
    pub chroma: f32,
    /// Hue angle in degrees, `0..360`.
    pub hue: f32,
}

impl Oklch {
    pub const fn new(lightness: f32, chroma: f32, hue: f32) -> Self {
        Self {
            lightness,
            chroma,
            hue,
        }
    }

    /// The shortest angular distance to another hue, in degrees, `0..=180`.
    pub fn hue_distance(self, other: Self) -> f32 {
        let delta = (self.hue - other.hue).rem_euclid(360.0);
        delta.min(360.0 - delta)
    }
}

impl From<Oklab> for Oklch {
    fn from(lab: Oklab) -> Self {
        let chroma = lab.a.hypot(lab.b);
        // A neutral has no hue to report, and `atan2(0, 0)` would invent one.
        let hue = if chroma < 1.0e-6 {
            0.0
        } else {
            lab.b.atan2(lab.a).to_degrees().rem_euclid(360.0)
        };
        Self {
            lightness: lab.lightness,
            chroma,
            hue,
        }
    }
}

impl From<Oklch> for Oklab {
    fn from(lch: Oklch) -> Self {
        let hue = lch.hue.to_radians();
        Self {
            lightness: lch.lightness,
            a: lch.chroma * hue.cos(),
            b: lch.chroma * hue.sin(),
        }
    }
}

/// Removes the sRGB transfer function, giving light-linear channel values.
fn linear(channel: f32) -> f32 {
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// Reapplies the sRGB transfer function.
fn encode(channel: f32) -> f32 {
    if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

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
        0.2126 * linear(self.red) + 0.7152 * linear(self.green) + 0.0722 * linear(self.blue)
    }

    /// This colour in OKLab, ignoring its alpha.
    ///
    /// Composite first with [`over`] when the colour is translucent: a
    /// perceptual distance between two paints nobody sees at full strength is
    /// a measurement of something that is not on screen.
    pub fn oklab(self) -> Oklab {
        let (red, green, blue) = (linear(self.red), linear(self.green), linear(self.blue));
        let long = 0.412_221_47 * red + 0.536_332_54 * green + 0.051_445_995 * blue;
        let medium = 0.211_903_5 * red + 0.680_699_5 * green + 0.107_396_96 * blue;
        let short = 0.088_302_46 * red + 0.281_718_84 * green + 0.629_978_7 * blue;
        let (long, medium, short) = (long.cbrt(), medium.cbrt(), short.cbrt());
        Oklab {
            lightness: 0.210_454_26 * long + 0.793_617_8 * medium - 0.004_072_047 * short,
            a: 1.977_998_5 * long - 2.428_592_2 * medium + 0.450_593_7 * short,
            b: 0.025_904_037 * long + 0.782_771_77 * medium - 0.808_675_77 * short,
        }
    }

    /// This colour in cylindrical OKLCH, ignoring its alpha.
    pub fn oklch(self) -> Oklch {
        self.oklab().into()
    }

    /// The opaque sRGB colour nearest to `lab`, with channels clamped into
    /// gamut rather than reported as out of range.
    ///
    /// Clamping is the honest failure here: a token author asking for a
    /// chroma sRGB cannot reach gets the closest thing a display will show,
    /// and the perceptual gates then measure what was actually produced
    /// rather than what was requested.
    pub fn from_oklab(lab: Oklab) -> Self {
        let long = lab.lightness + 0.396_337_78 * lab.a + 0.215_803_76 * lab.b;
        let medium = lab.lightness - 0.105_561_346 * lab.a - 0.063_854_17 * lab.b;
        let short = lab.lightness - 0.089_484_18 * lab.a - 1.291_485_5 * lab.b;
        let (long, medium, short) = (
            long * long * long,
            medium * medium * medium,
            short * short * short,
        );
        let red = 4.076_741_7 * long - 3.307_711_6 * medium + 0.230_969_94 * short;
        let green = -1.268_438 * long + 2.609_757_4 * medium - 0.341_319_38 * short;
        let blue = -0.004_196_086_3 * long - 0.703_418_6 * medium + 1.707_614_7 * short;
        Self {
            red: encode(red).clamp(0.0, 1.0),
            green: encode(green).clamp(0.0, 1.0),
            blue: encode(blue).clamp(0.0, 1.0),
            alpha: 1.0,
        }
    }

    /// The opaque sRGB colour nearest to `lch`.
    pub fn from_oklch(lch: Oklch) -> Self {
        Self::from_oklab(lch.into())
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

/// The perceptual distance between two opaque colours, in OKLab units.
///
/// This is the measure that answers "can a reader tell these two apart", which
/// neither the WCAG ratio nor an L\* difference does. Two colours of the same
/// lightness in different hues are 1:1 by contrast ratio and zero apart in
/// L\*, and a categorical scale is made entirely of such pairs.
///
/// As a rule of thumb for the thresholds in this crate: below about `0.02` is
/// a difference nobody reliably sees, `0.10` is a comfortable separation
/// between adjacent series, and `1.0` is roughly black against white.
pub fn perceptual_distance(left: Color, right: Color) -> f32 {
    let (left, right) = (left.oklab(), right.oklab());
    ((left.lightness - right.lightness).powi(2)
        + (left.a - right.a).powi(2)
        + (left.b - right.b).powi(2))
    .sqrt()
}

/// Blends two opaque colours perceptually, `0` returning `left` and `1`
/// returning `right`.
///
/// Blending in sRGB channels dips through a muddy, darker midpoint because
/// those channels are gamma-encoded rather than proportional to light. In
/// OKLab the midpoint is the colour a reader would name as halfway, which is
/// what a wash, a tint, or a state crossfade is claiming to show.
pub fn mix(left: Color, right: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    let (a, b) = (left.oklab(), right.oklab());
    let blend = |from: f32, to: f32| from + (to - from) * amount;
    let mut color = Color::from_oklab(Oklab {
        lightness: blend(a.lightness, b.lightness),
        a: blend(a.a, b.a),
        b: blend(a.b, b.b),
    });
    color.alpha = blend(left.alpha, right.alpha);
    color
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

    #[test]
    fn oklab_pins_the_poles_and_leaves_a_neutral_without_a_hue() {
        let white = Color::parse("w", "#ffffff").expect("white").oklab();
        assert!((white.lightness - 1.0).abs() < 0.001);
        assert!(white.a.abs() < 0.001 && white.b.abs() < 0.001);

        let black = Color::parse("b", "#000000").expect("black").oklab();
        assert!(black.lightness.abs() < 0.001);

        // A grey has no hue to report, so the cylindrical form must not
        // invent one out of the noise in two near-zero axes.
        let grey = Color::parse("g", "#808080").expect("grey").oklch();
        assert!(grey.chroma < 0.001);
        assert_eq!(grey.hue, 0.0);
    }

    #[test]
    fn the_round_trip_through_oklch_returns_the_colour_it_started_from() {
        for literal in ["#3b6ef5", "#f5a03b", "#1e9e6a", "#c03b8f", "#101418"] {
            let original = Color::parse("c", literal).expect("literal");
            let restored = Color::from_oklch(original.oklch());
            assert!(
                perceptual_distance(original, restored) < 0.002,
                "{literal} did not survive the round trip"
            );
        }
    }

    #[test]
    fn perceptual_distance_sees_what_the_contrast_ratio_flattens() {
        // Two hues chosen at one lightness: identical to the WCAG ratio and
        // to CIE L*, obviously different to a reader.
        let one = Color::from_oklch(Oklch::new(0.62, 0.15, 25.0));
        let other = Color::from_oklch(Oklch::new(0.62, 0.15, 205.0));
        // Nowhere near the 3:1 a control boundary is held to, and only a
        // couple of L* apart, yet plainly two different colours.
        assert!(contrast_ratio(one, other) < 1.5);
        assert!((one.lightness() - other.lightness()).abs() < 6.0);
        assert!(perceptual_distance(one, other) > 0.2);

        let white = Color::parse("w", "#ffffff").expect("white");
        let black = Color::parse("b", "#000000").expect("black");
        assert!(perceptual_distance(white, black) > 0.95);
        assert_eq!(perceptual_distance(white, white), 0.0);
    }

    #[test]
    fn a_perceptual_blend_keeps_the_midpoint_off_the_mud_srgb_would_take() {
        let blue = Color::parse("a", "#0000ff").expect("blue");
        let yellow = Color::parse("b", "#ffff00").expect("yellow");
        let midpoint = mix(blue, yellow, 0.5);

        // The naive channel average of these two is mid grey. A perceptual
        // blend keeps the light the two ends actually carry.
        let naive = Color {
            red: 0.5,
            green: 0.5,
            blue: 0.5,
            alpha: 1.0,
        };
        assert!(midpoint.lightness() > naive.lightness());
        assert!(perceptual_distance(mix(blue, yellow, 0.0), blue) < 0.002);
        assert!(perceptual_distance(mix(blue, yellow, 1.0), yellow) < 0.002);
    }

    #[test]
    fn hue_distance_takes_the_short_way_round() {
        let near_zero = Oklch::new(0.6, 0.1, 350.0);
        let past_zero = Oklch::new(0.6, 0.1, 10.0);
        assert!((near_zero.hue_distance(past_zero) - 20.0).abs() < 0.001);
        assert!((past_zero.hue_distance(near_zero) - 20.0).abs() < 0.001);
        assert!(
            (Oklch::new(0.6, 0.1, 0.0).hue_distance(Oklch::new(0.6, 0.1, 180.0)) - 180.0).abs()
                < 0.001
        );
    }
}
