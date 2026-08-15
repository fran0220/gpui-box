//! The contrast rules every theme must satisfy.
//!
//! Roles carry different minimums on purpose: primary and muted text clear
//! WCAG AA, while placeholder and faint text plus every active visual identity
//! clear 3:1. Disabled text is held to the same product floor even though WCAG
//! exempts inactive controls; "inactive" is not permission to disappear.

use crate::{
    Color, InteractiveColor, SemanticColor, Surface, TextTone, TokenDocument, contrast_ratio,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ContrastCheck {
    pub foreground: String,
    pub background: String,
    pub ratio: f32,
    pub minimum: f32,
}

impl ContrastCheck {
    pub fn passes(&self) -> bool {
        self.ratio >= self.minimum
    }
}

const TEXT_MINIMUM: f32 = 4.5;
const NON_TEXT_MINIMUM: f32 = 3.0;

/// Evaluates every required pair for one theme.
pub fn report(tokens: &TokenDocument) -> Vec<ContrastCheck> {
    let mut checks = Vec::new();
    let surfaces = [
        ("color.surface.canvas", Surface::Canvas),
        ("color.surface.sunken", Surface::Sunken),
        ("color.surface.panel", Surface::Panel),
        ("color.surface.raised", Surface::Raised),
        ("color.surface.overlay", Surface::Overlay),
    ];

    for (surface_name, surface) in surfaces {
        let background = tokens.surface(surface);
        for (tone_name, tone, minimum) in [
            ("color.text.primary", TextTone::Primary, TEXT_MINIMUM),
            ("color.text.muted", TextTone::Muted, TEXT_MINIMUM),
            ("color.text.faint", TextTone::Faint, NON_TEXT_MINIMUM),
            (
                "color.text.placeholder",
                TextTone::Placeholder,
                NON_TEXT_MINIMUM,
            ),
            ("color.text.disabled", TextTone::Disabled, NON_TEXT_MINIMUM),
        ] {
            checks.push(check(
                tone_name,
                tokens.text(tone),
                surface_name,
                background,
                minimum,
            ));
        }
        for (color_name, color) in [
            ("color.semantic.accent", SemanticColor::Accent),
            ("color.semantic.accentStrong", SemanticColor::AccentStrong),
            ("color.semantic.danger", SemanticColor::Danger),
            ("color.semantic.warning", SemanticColor::Warning),
            ("color.semantic.success", SemanticColor::Success),
            ("color.semantic.info", SemanticColor::Info),
        ] {
            checks.push(check(
                color_name,
                tokens.semantic(color),
                surface_name,
                background,
                NON_TEXT_MINIMUM,
            ));
        }
        for (color_name, color) in [
            ("color.interactive.hairline", InteractiveColor::Hairline),
            (
                "color.interactive.hairlineStrong",
                InteractiveColor::HairlineStrong,
            ),
            ("color.interactive.track", InteractiveColor::Track),
            ("color.interactive.divider", InteractiveColor::Divider),
        ] {
            checks.push(check(
                color_name,
                tokens.interactive(color),
                surface_name,
                background,
                NON_TEXT_MINIMUM,
            ));
        }
        checks.push(check(
            "color.interactive.focus @ effect.focusRingAlpha",
            opacity(
                tokens.interactive(InteractiveColor::Focus),
                tokens.effect.focus_ring_alpha,
            ),
            surface_name,
            background,
            NON_TEXT_MINIMUM,
        ));
        checks.push(check(
            "color.text.primary @ opacity.disabled",
            opacity(tokens.text(TextTone::Primary), tokens.opacity.disabled),
            surface_name,
            background,
            NON_TEXT_MINIMUM,
        ));
        for (index, color) in tokens.loader_gradient().into_iter().enumerate() {
            checks.push(check(
                &format!("color.loader.gradient.{index}"),
                color,
                surface_name,
                background,
                NON_TEXT_MINIMUM,
            ));
        }
    }

    // `accent` is the only accent that carries text. `accentStrong` is an
    // emphasis, border and hover color; it is held to the non-text minimum
    // against surfaces above, not to the body minimum against `onAccent`.
    checks.push(check(
        "color.text.onAccent",
        tokens.text(TextTone::OnAccent),
        "color.semantic.accent",
        tokens.semantic(SemanticColor::Accent),
        TEXT_MINIMUM,
    ));

    checks
}

fn opacity(mut color: Color, opacity: f32) -> Color {
    color.alpha *= opacity;
    color
}

pub fn failures(tokens: &TokenDocument) -> Vec<ContrastCheck> {
    report(tokens)
        .into_iter()
        .filter(|check| !check.passes())
        .collect()
}

fn check(
    foreground_name: &str,
    foreground: Color,
    background_name: &str,
    background: Color,
    minimum: f32,
) -> ContrastCheck {
    ContrastCheck {
        foreground: foreground_name.into(),
        background: background_name.into(),
        ratio: contrast_ratio(foreground, background),
        minimum,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_theme_meets_its_contrast_floor() {
        for tokens in crate::bundled() {
            let failures = failures(tokens);
            assert!(
                failures.is_empty(),
                "{} fails contrast: {:#?}",
                tokens.meta.id,
                failures
            );
        }
    }

    #[test]
    fn the_report_covers_every_surface_and_tone() {
        let checks = report(crate::studio_dark());
        assert_eq!(checks.len(), 5 * 20 + 1);
    }

    #[test]
    fn rendered_alpha_is_part_of_focus_and_disabled_checks() {
        let checks = report(crate::studio_light());
        let focus = checks
            .iter()
            .find(|check| {
                check.foreground == "color.interactive.focus @ effect.focusRingAlpha"
                    && check.background == "color.surface.sunken"
            })
            .expect("focus on the field surface");
        assert!(focus.passes());
        assert!(
            focus.ratio
                < contrast_ratio(
                    crate::studio_light().interactive(InteractiveColor::Focus),
                    crate::studio_light().surface(Surface::Sunken),
                )
        );

        let disabled = checks
            .iter()
            .find(|check| check.foreground == "color.text.primary @ opacity.disabled")
            .expect("disabled presentation check");
        assert!(disabled.passes());
    }
}
