//! The contrast rules every theme must satisfy.
//!
//! Roles carry different minimums on purpose: primary and muted text clear
//! WCAG AA, while placeholder and faint text plus every active visual identity
//! clear 3:1. Disabled text is held to the same product floor even though WCAG
//! exempts inactive controls; "inactive" is not permission to disappear.
//!
//! Two backgrounds meeting each other is a separate rule with a separate
//! measure; see [`separation_report`].

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

/// One nesting of two surfaces, and how far apart they read.
///
/// `distance` is signed: positive means `near` is the lighter of the two, so
/// one check answers both questions a stacked surface raises — whether the
/// step is visible at all, and whether it goes the direction the ramp claims.
#[derive(Debug, Clone, PartialEq)]
pub struct SeparationCheck {
    pub near: String,
    pub behind: String,
    pub distance: f32,
    pub minimum: f32,
}

impl SeparationCheck {
    pub fn passes(&self) -> bool {
        self.distance >= self.minimum
    }
}

/// The perceptual lightness a surface must gain over the one behind it.
///
/// Three is about where a step stops being a rendering artifact and starts
/// being a boundary a reader can point at. It is deliberately modest: this is
/// a floor under every theme, not a house style.
pub const SEPARATION_MINIMUM: f32 = 3.0;

/// Every stack of two surfaces a component can actually build, in order.
///
/// The ramp climbs away from the page in both appearances: a well is below
/// the surface holding it, a panel is above the page, and a block inside a
/// panel is above the panel. A light theme therefore does not put white on
/// the page and leave nothing above it — the page is tinted and white is what
/// the ramp climbs to, which is also how a native window separates its
/// background from its content.
///
/// `overlay` is checked against what it can open over rather than against
/// `raised`: a popover and a code block never touch, so a theme is free to
/// give them the same value.
const NESTINGS: [(Surface, Surface); 6] = [
    (Surface::Canvas, Surface::Sunken),
    (Surface::Panel, Surface::Canvas),
    (Surface::Panel, Surface::Sunken),
    (Surface::Raised, Surface::Panel),
    (Surface::Overlay, Surface::Panel),
    (Surface::Overlay, Surface::Canvas),
];

/// Evaluates every surface nesting for one theme.
///
/// This exists because the contrast report above cannot answer it. That
/// report asks whether a foreground is legible on a background and never
/// whether two backgrounds are distinguishable, which is why `studio-light`
/// was able to give `panel`, `raised` and `overlay` the same white and still
/// pass: a card, the code block inside it, and the popover over it were one
/// undivided field of color, and nothing in the build said so.
pub fn separation_report(tokens: &TokenDocument) -> Vec<SeparationCheck> {
    NESTINGS
        .into_iter()
        .map(|(near, behind)| SeparationCheck {
            near: surface_path(near).into(),
            behind: surface_path(behind).into(),
            distance: tokens.surface(near).lightness() - tokens.surface(behind).lightness(),
            minimum: SEPARATION_MINIMUM,
        })
        .collect()
}

pub fn separation_failures(tokens: &TokenDocument) -> Vec<SeparationCheck> {
    separation_report(tokens)
        .into_iter()
        .filter(|check| !check.passes())
        .collect()
}

/// Two text tones that mean different things, and how far apart they read.
#[derive(Debug, Clone, PartialEq)]
pub struct DistinctionCheck {
    pub tone: String,
    pub against: String,
    pub distance: f32,
    pub minimum: f32,
}

impl DistinctionCheck {
    pub fn passes(&self) -> bool {
        self.distance >= self.minimum
    }
}

/// How far apart two tones that carry different facts must read.
///
/// The same floor the surfaces get, for the same reason and in the same
/// units: below it the difference is a rendering artifact rather than
/// something a reader can point at.
pub const DISTINCTION_MINIMUM: f32 = 3.0;

/// The tone ladder, each rung dimmer than the one above it.
///
/// These are four different facts and not four intensities of one. Muted is
/// secondary information the reader is meant to read; faint is supporting
/// detail; a placeholder is a description of a value that is *not there*; and
/// disabled is a value that is there and cannot be used. A theme that gives
/// two of them one colour has not styled them alike, it has stopped saying
/// which of them holds — and this library's own rule is that unavailable and
/// absent are distinct states rather than degrees of the same one.
///
/// The ladder is ordered, and each rung is measured by how far it stands from
/// the page rather than by its lightness, so one rule holds in both
/// appearances: a dimmer fact sits closer to the canvas than the fact above
/// it, whether the canvas is white or black. A signed distance therefore also
/// catches a ladder whose rungs are out of order.
const DISTINCTIONS: [(TextTone, TextTone); 3] = [
    (TextTone::Muted, TextTone::Faint),
    (TextTone::Faint, TextTone::Placeholder),
    (TextTone::Placeholder, TextTone::Disabled),
];

/// Evaluates the tone ladder for one theme.
///
/// Neither report above can answer this. The contrast report asks whether each
/// tone is legible on each surface and passes happily when three of them are
/// legible because they are the same colour, which is exactly how
/// `studio-dark` came to draw faint, placeholder and disabled in one grey
/// while the visual audit kept reporting that unavailable values were
/// unreadable. They were perfectly readable; they were just not distinguishable.
pub fn distinction_report(tokens: &TokenDocument) -> Vec<DistinctionCheck> {
    let page = tokens.surface(Surface::Canvas).lightness();
    let from_page = |tone: TextTone| (tokens.text(tone).lightness() - page).abs();
    DISTINCTIONS
        .into_iter()
        .map(|(stronger, dimmer)| DistinctionCheck {
            tone: tone_path(stronger).into(),
            against: tone_path(dimmer).into(),
            distance: from_page(stronger) - from_page(dimmer),
            minimum: DISTINCTION_MINIMUM,
        })
        .collect()
}

pub fn distinction_failures(tokens: &TokenDocument) -> Vec<DistinctionCheck> {
    distinction_report(tokens)
        .into_iter()
        .filter(|check| !check.passes())
        .collect()
}

fn tone_path(tone: TextTone) -> &'static str {
    match tone {
        TextTone::Primary => "color.text.primary",
        TextTone::Muted => "color.text.muted",
        TextTone::Faint => "color.text.faint",
        TextTone::Placeholder => "color.text.placeholder",
        TextTone::Disabled => "color.text.disabled",
        TextTone::OnAccent => "color.text.onAccent",
    }
}

fn surface_path(surface: Surface) -> &'static str {
    match surface {
        Surface::Canvas => "color.surface.canvas",
        Surface::Sunken => "color.surface.sunken",
        Surface::Panel => "color.surface.panel",
        Surface::Raised => "color.surface.raised",
        Surface::Overlay => "color.surface.overlay",
    }
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
    fn every_bundled_theme_separates_the_surfaces_that_stack() {
        for tokens in crate::bundled() {
            let failures = separation_failures(tokens);
            assert!(
                failures.is_empty(),
                "{} stacks surfaces nobody can tell apart: {:#?}",
                tokens.meta.id,
                failures
            );
        }
    }

    #[test]
    fn every_bundled_theme_keeps_its_dim_tones_apart() {
        for tokens in crate::bundled() {
            let failures = distinction_failures(tokens);
            assert!(
                failures.is_empty(),
                "{} draws two different facts in one tone: {:#?}",
                tokens.meta.id,
                failures
            );
        }
    }

    /// The failure this rule was written for. Three tones that are legible and
    /// identical pass every contrast check there is, and say nothing.
    #[test]
    fn one_grey_for_faint_placeholder_and_disabled_is_rejected() {
        let mut tokens = crate::bundled()[0].clone();
        let faint = tokens.color.text.faint.clone();
        tokens.color.text.placeholder = faint.clone();
        tokens.color.text.disabled = faint;
        assert!(
            failures(&tokens).is_empty(),
            "the contrast report is happy with it, which is the point"
        );
        assert!(!distinction_failures(&tokens).is_empty());
        assert!(matches!(
            tokens.validate(),
            Err(crate::TokenError::Distinction(_))
        ));
    }

    /// The ladder is stated in distance from the page, not in lightness, so
    /// one rule holds for a theme that dims upward and one that dims downward.
    #[test]
    fn the_ladder_holds_in_both_appearances() {
        for tokens in crate::bundled() {
            let page = tokens.surface(Surface::Canvas).lightness();
            let from_page = |tone| (tokens.text(tone).lightness() - page).abs();
            assert!(from_page(TextTone::Primary) > from_page(TextTone::Muted));
            assert!(from_page(TextTone::Muted) > from_page(TextTone::Faint));
            assert!(from_page(TextTone::Faint) > from_page(TextTone::Placeholder));
            assert!(from_page(TextTone::Placeholder) > from_page(TextTone::Disabled));
        }
    }

    /// The ramp is a claim about depth, so it has to hold in both appearances.
    #[test]
    fn the_ramp_climbs_away_from_the_page_in_every_theme() {
        for tokens in crate::bundled() {
            let lightness = |surface| tokens.surface(surface).lightness();
            let sunken = lightness(Surface::Sunken);
            let canvas = lightness(Surface::Canvas);
            let panel = lightness(Surface::Panel);
            let raised = lightness(Surface::Raised);
            assert!(
                sunken < canvas && canvas < panel && panel < raised,
                "{} does not order sunken < canvas < panel < raised: {sunken}, {canvas}, \
                 {panel}, {raised}",
                tokens.meta.id
            );
        }
    }

    /// The regression the separation rule exists to catch. A theme that paints
    /// a card, the block inside it, and the popover over it the same white
    /// clears every contrast pair in `report` and is still unreadable.
    #[test]
    fn one_white_for_panel_raised_and_overlay_is_rejected() {
        let mut tokens = crate::studio_light().clone();
        tokens.color.surface.panel = "#ffffff".into();
        tokens.color.surface.raised = "#ffffff".into();
        tokens.color.surface.overlay = "#ffffff".into();

        assert!(failures(&tokens).is_empty(), "contrast alone cannot see it");
        let failures = separation_failures(&tokens);
        assert!(
            failures
                .iter()
                .any(|check| check.near == "color.surface.raised"
                    && check.behind == "color.surface.panel"),
            "{failures:#?}"
        );
    }

    #[test]
    fn lightness_separates_what_the_wcag_ratio_flattens() {
        let near_black = crate::Color::parse("t", "#050505").expect("literal");
        let page = crate::Color::parse("t", "#0a0a0a").expect("literal");
        assert!(contrast_ratio(near_black, page) < 1.05);
        assert!((page.lightness() - near_black.lightness()) < SEPARATION_MINIMUM);

        let white = crate::Color::parse("t", "#ffffff").expect("literal");
        assert!((white.lightness() - 100.0).abs() < 0.01);
        let black = crate::Color::parse("t", "#000000").expect("literal");
        assert!(black.lightness().abs() < 0.01);
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
