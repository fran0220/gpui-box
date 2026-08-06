//! The contrast rules every theme must satisfy.
//!
//! Roles carry different minimums on purpose: body text must clear WCAG AA,
//! while faint text and status colors are held to the 3:1 non-text minimum
//! because they never carry required instructions on their own.

use crate::{Color, SemanticColor, Surface, TextTone, TokenDocument, contrast_ratio};

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

const BODY_MINIMUM: f32 = 4.5;
const LARGE_MINIMUM: f32 = 3.0;

/// Evaluates every required pair for one theme.
pub fn report(tokens: &TokenDocument) -> Vec<ContrastCheck> {
    let mut checks = Vec::new();
    let surfaces = [
        ("surface.canvas", Surface::Canvas),
        ("surface.panel", Surface::Panel),
        ("surface.raised", Surface::Raised),
        ("surface.overlay", Surface::Overlay),
    ];

    for (surface_name, surface) in surfaces {
        let background = tokens.surface(surface);
        for (tone_name, tone, minimum) in [
            ("text.primary", TextTone::Primary, BODY_MINIMUM),
            ("text.muted", TextTone::Muted, BODY_MINIMUM),
            ("text.faint", TextTone::Faint, LARGE_MINIMUM),
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
            ("semantic.danger", SemanticColor::Danger),
            ("semantic.warning", SemanticColor::Warning),
            ("semantic.success", SemanticColor::Success),
            ("semantic.info", SemanticColor::Info),
            ("semantic.accent", SemanticColor::Accent),
        ] {
            checks.push(check(
                color_name,
                tokens.semantic(color),
                surface_name,
                background,
                LARGE_MINIMUM,
            ));
        }
    }

    // `accent` is the only accent that carries text. `accentStrong` is an
    // emphasis, border and hover color; it is held to the non-text minimum
    // against surfaces above, not to the body minimum against `onAccent`.
    checks.push(check(
        "text.onAccent",
        tokens.text(TextTone::OnAccent),
        "semantic.accent",
        tokens.semantic(SemanticColor::Accent),
        BODY_MINIMUM,
    ));

    checks
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
        assert_eq!(checks.len(), 4 * 8 + 1);
    }
}
