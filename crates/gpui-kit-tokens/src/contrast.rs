//! The contrast rules every theme must satisfy.
//!
//! Roles carry different minimums on purpose: primary and muted text clear
//! WCAG AA, while placeholder and faint text plus every active visual identity
//! clear 3:1. Disabled text is held to the same product floor even though WCAG
//! exempts inactive controls; "inactive" is not permission to disappear.
//!
//! Two backgrounds meeting each other is a separate rule with a separate
//! measure; see [`separation_report`]. A decorative line drawn *between* two
//! pieces of content on one surface is a third rule again; see
//! [`line_report`].

use crate::{
    AgentColor, Color, InteractiveColor, SemanticColor, Surface, SyntaxColor, TextTone,
    TokenDocument, contrast_ratio,
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
/// The floor under the two ANSI greys, which are structure rather than text.
/// Low enough that a palette can still put "black" near the terminal's own
/// background, high enough that it cannot vanish into it.
const GREY_MINIMUM: f32 = 1.25;

/// The ANSI slot whose job is to sit at the dark end of the scale rather than
/// to be read. Slot 8, "bright black", is the same slot one octave up and is
/// the dim-text grey every terminal palette uses.
const ANSI_BLACK: usize = 0;

/// Evaluates every required pair for one theme.
pub fn report(tokens: &TokenDocument) -> Vec<ContrastCheck> {
    let mut checks = Vec::new();
    let surfaces = [
        ("color.surface.backdrop", Surface::Backdrop),
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
        let selected = crate::over(tokens.interactive(InteractiveColor::Selected), background);
        checks.push(check(
            "color.text.primary over color.interactive.selected",
            tokens.text(TextTone::Primary),
            &format!("{surface_name} + selected"),
            selected,
            TEXT_MINIMUM,
        ));
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
        for family in [
            AgentColor::Read,
            AgentColor::Network,
            AgentColor::Shell,
            AgentColor::Edit,
            AgentColor::External,
        ] {
            checks.push(check(
                family.path(),
                tokens.agent(family),
                surface_name,
                background,
                TEXT_MINIMUM,
            ));
        }
        let evidence_wash = crate::over(tokens.agent(AgentColor::EvidenceWash), background);
        checks.push(check(
            "color.text.muted",
            tokens.text(TextTone::Muted),
            &format!("{surface_name} + {}", AgentColor::EvidenceWash.path()),
            evidence_wash,
            TEXT_MINIMUM,
        ));
        // Only the lines that carry a *control's* boundary are held here:
        // the rail a slider runs in, the edge of a switch, the gutter a
        // scrollbar thumb sits in. Those are part of an interactive
        // affordance, so a reader who cannot see them cannot see the
        // control. `hairline` and `divider` are decoration between two
        // pieces of content on one surface, and are held to the separate,
        // much lower floor in `line_report` instead — holding them to 3:1
        // is what turned every card, table and menu in this library into an
        // outlined box.
        for (color_name, color) in [
            (
                "color.interactive.hairlineStrong",
                InteractiveColor::HairlineStrong,
            ),
            ("color.interactive.track", InteractiveColor::Track),
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

    // Every ANSI slot has to be legible on the terminal background *of its own
    // theme*. The regression this catches is the obvious one: a palette tuned
    // by eye against near-black, reused unchanged on near-white, where the
    // bright half is the invisible half.
    //
    // Two floors, because the table holds two kinds of thing. The twelve
    // chromatic slots carry program output and are held to the non-text
    // minimum. The black/bright-black pair are structural greys — box drawing
    // and dim hints — whose job is to sit at the quiet end of the scale rather
    // than to be read, so they only have to separate from the background at
    // all.
    let terminal_background = tokens.terminal_background();
    for (index, color) in tokens.terminal_ansi().into_iter().enumerate() {
        let minimum = if index % 8 == ANSI_BLACK {
            GREY_MINIMUM
        } else {
            NON_TEXT_MINIMUM
        };
        checks.push(check(
            &format!("color.terminal.ansi.{index}"),
            color,
            "color.terminal.background",
            terminal_background,
            minimum,
        ));
    }

    // Code is read, so its classes carry the body floor rather than the
    // 3:1 an identity gets — a keyword nobody can read is not a highlight.
    // The two surfaces are the ones code is drawn on: a fenced block sits in
    // a well, and an inline span sits on whatever prose sits on.
    //
    // `comment` is the exception, and deliberately: it is supporting detail
    // inside the code the way `text.faint` is beside prose, so it takes that
    // role's floor. Holding it to the body minimum would make every comment
    // shout as loudly as the code it annotates.
    for (surface_name, surface) in [
        ("color.surface.sunken", Surface::Sunken),
        ("color.surface.panel", Surface::Panel),
    ] {
        let background = tokens.surface(surface);
        for class in [
            SyntaxColor::Keyword,
            SyntaxColor::StringLiteral,
            SyntaxColor::Number,
            SyntaxColor::Inline,
            SyntaxColor::Comment,
        ] {
            let minimum = if class == SyntaxColor::Comment {
                NON_TEXT_MINIMUM
            } else {
                TEXT_MINIMUM
            };
            checks.push(check(
                class.path(),
                tokens.syntax(class),
                surface_name,
                background,
                minimum,
            ));
        }

        // A diff's washes are bands under whole lines, and the line's own
        // characters are ordinary body text drawn on top. So the pair is
        // checked the way it is actually stacked: primary text over the wash
        // over the surface. A wash tuned until the sign was obvious, at the
        // cost of the code on it, is the regression this catches.
        for (wash_name, wash, sign_name, sign) in [
            (
                SyntaxColor::AddedWash.path(),
                tokens.syntax(SyntaxColor::AddedWash),
                SyntaxColor::Added.path(),
                tokens.syntax(SyntaxColor::Added),
            ),
            (
                SyntaxColor::RemovedWash.path(),
                tokens.syntax(SyntaxColor::RemovedWash),
                SyntaxColor::Removed.path(),
                tokens.syntax(SyntaxColor::Removed),
            ),
        ] {
            let washed = crate::over(wash, background);
            checks.push(check(
                "color.text.primary",
                tokens.text(TextTone::Primary),
                &format!("{surface_name} + {wash_name}"),
                washed,
                TEXT_MINIMUM,
            ));
            // The sign colour marks the side the line is on, in the gutter and
            // the accent bar. It is an identity rather than prose.
            checks.push(check(
                sign_name,
                sign,
                surface_name,
                background,
                NON_TEXT_MINIMUM,
            ));
        }

        // The wash under an inline span carries code that has to stay
        // readable on it, in the middle of a sentence drawn on the surface.
        let inline_washed = crate::over(tokens.syntax(SyntaxColor::InlineWash), background);
        checks.push(check(
            SyntaxColor::Inline.path(),
            tokens.syntax(SyntaxColor::Inline),
            &format!("{surface_name} + {}", SyntaxColor::InlineWash.path()),
            inline_washed,
            TEXT_MINIMUM,
        ));
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
/// Two is about where a step stops being a rendering artifact and starts
/// being a boundary a reader can point at, and it is what shipped light
/// chrome actually uses: measured against a native window and against the
/// editors people compare this library to, the step from the brightest plane
/// to the one under it lands near 2.4 L\*, not 3. A higher floor is not a
/// stricter reading of the same rule, it is a house style — with five rungs
/// under one white ceiling, every extra tenth of a floor is subtracted from
/// the page, and a light theme built to clear it comes out visibly darker
/// than the platform it sits on.
///
/// It is deliberately modest: a floor under every theme, not a ladder. What
/// it rules out is what it was written for — two planes a reader cannot tell
/// apart at all.
pub const SEPARATION_MINIMUM: f32 = 2.0;

/// Every stack of two surfaces a component can actually build, in order.
///
/// The ramp climbs away from the page in both appearances: a well is below
/// the surface holding it, a panel is above the page, and a block inside a
/// panel is above the panel. A light theme therefore does not put white on
/// the page and leave nothing above it — the page is tinted and white is what
/// the ramp climbs to, which is also how a native window separates its
/// background from its content.
///
/// `backdrop` is the substrate behind the page. It is checked against
/// `canvas` and `panel` because those are the surfaces that can sit on it.
/// It is not checked against `sunken`: a well never sits on the substrate,
/// it sits in a panel or on the page, and requiring three L* between those
/// two would collapse the dark ramp.
///
/// `overlay` is checked against what it can open over rather than against
/// `raised`: a popover and a code block never touch, so a theme is free to
/// give them the same value.
const NESTINGS: [(Surface, Surface); 8] = [
    (Surface::Canvas, Surface::Backdrop),
    (Surface::Panel, Surface::Backdrop),
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

/// One decorative line on one surface, and how far it stands from it.
#[derive(Debug, Clone, PartialEq)]
pub struct LineCheck {
    pub line: String,
    pub surface: String,
    pub distance: f32,
    pub minimum: f32,
}

impl LineCheck {
    pub fn passes(&self) -> bool {
        self.distance >= self.minimum
    }
}

/// The perceptual lightness a decorative line must gain over its surface.
///
/// Deliberately far below [`SEPARATION_MINIMUM`]. A hairline separating two
/// rows of one table is not a plane a reader has to identify, it is a hint
/// that two rows are two rows, and the row heights and the hover wash have
/// already said so. What this floor rules out is a line that was *typed* and
/// never *drawn*: an alpha low enough to round away against its own surface
/// is a line the author believes is there and no display renders.
///
/// The upper bound is the point of the rule. A line held to the 3:1 that
/// [`report`] requires of control boundaries is not a hairline, it is an
/// outline, and a library that draws one around every card, row, menu and
/// tab has no borderless language left to speak.
pub const LINE_MINIMUM: f32 = 1.5;

/// Every decorative line, against every surface it can be drawn on.
///
/// The line colours are translucent by design, so each is composited onto the
/// surface before it is measured: what the rule asks is what a reader
/// actually sees, not what the channel says in isolation.
pub fn line_report(tokens: &TokenDocument) -> Vec<LineCheck> {
    let surfaces = [
        ("color.surface.backdrop", Surface::Backdrop),
        ("color.surface.canvas", Surface::Canvas),
        ("color.surface.sunken", Surface::Sunken),
        ("color.surface.panel", Surface::Panel),
        ("color.surface.raised", Surface::Raised),
        ("color.surface.overlay", Surface::Overlay),
    ];
    let lines = [
        ("color.interactive.hairline", InteractiveColor::Hairline),
        ("color.interactive.divider", InteractiveColor::Divider),
    ];
    // The washes belong to this rule and not to the contrast one for the same
    // reason a hairline does: they are translucent marks whose whole failure
    // mode is being typed and never drawn. What they must carry *on top* of
    // themselves is asked in `report`; what is asked here is whether the band
    // exists at all.
    let washes = [
        SyntaxColor::AddedWash,
        SyntaxColor::RemovedWash,
        SyntaxColor::InlineWash,
    ];

    let mut checks = Vec::new();
    for (surface_name, surface) in surfaces {
        let background = tokens.surface(surface);
        for (line_name, line) in lines {
            let drawn = crate::over(tokens.interactive(line), background);
            checks.push(LineCheck {
                line: line_name.into(),
                surface: surface_name.into(),
                distance: (drawn.lightness() - background.lightness()).abs(),
                minimum: LINE_MINIMUM,
            });
        }
        for wash in washes {
            let drawn = crate::over(tokens.syntax(wash), background);
            checks.push(LineCheck {
                line: wash.path().into(),
                surface: surface_name.into(),
                distance: (drawn.lightness() - background.lightness()).abs(),
                minimum: LINE_MINIMUM,
            });
        }
        let evidence_wash = AgentColor::EvidenceWash;
        let drawn = crate::over(tokens.agent(evidence_wash), background);
        checks.push(LineCheck {
            line: evidence_wash.path().into(),
            surface: surface_name.into(),
            distance: (drawn.lightness() - background.lightness()).abs(),
            minimum: LINE_MINIMUM,
        });
    }
    checks
}

pub fn line_failures(tokens: &TokenDocument) -> Vec<LineCheck> {
    line_report(tokens)
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
        Surface::Backdrop => "color.surface.backdrop",
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
        // Twenty-five tones against each of six surfaces, ten code checks
        // against each of the two surfaces code is drawn on, the sixteen ANSI
        // slots against the terminal background, and `onAccent` against
        // `accent`.
        assert_eq!(checks.len(), 6 * 25 + 2 * 10 + 16 + 1);
    }

    #[test]
    fn agent_family_tints_are_readable_but_quieter_than_primary_text() {
        for tokens in crate::bundled() {
            let canvas = tokens.surface(Surface::Canvas);
            let primary_distance =
                (tokens.text(TextTone::Primary).lightness() - canvas.lightness()).abs();
            for family in [
                AgentColor::Read,
                AgentColor::Network,
                AgentColor::Shell,
                AgentColor::Edit,
                AgentColor::External,
            ] {
                let color = tokens.agent(family);
                assert!(
                    contrast_ratio(color, canvas) >= TEXT_MINIMUM,
                    "{} makes {} unreadable",
                    tokens.meta.id,
                    family.path()
                );
                assert!(
                    (color.lightness() - canvas.lightness()).abs() < primary_distance,
                    "{} makes {} compete with primary prose",
                    tokens.meta.id,
                    family.path()
                );
            }
        }
    }

    #[test]
    fn code_is_held_to_the_floor_it_is_read_at() {
        // The four classes a reader scans for are body text and carry the body
        // floor; a comment is supporting detail and carries the quieter one.
        // A theme that made every class as loud as the code would pass a
        // single shared floor and say nothing, which is what this pins.
        let checks = report(crate::studio_dark());
        let floor = |path: &str| {
            checks
                .iter()
                .filter(|check| check.foreground == path)
                .map(|check| check.minimum)
                .fold(f32::INFINITY, f32::min)
        };
        for class in [
            SyntaxColor::Keyword,
            SyntaxColor::StringLiteral,
            SyntaxColor::Number,
        ] {
            assert_eq!(floor(class.path()), TEXT_MINIMUM, "{}", class.path());
        }
        assert_eq!(floor(SyntaxColor::Comment.path()), NON_TEXT_MINIMUM);
    }

    #[test]
    fn a_diff_wash_that_swallows_its_own_text_is_rejected() {
        // The failure this catches is a wash tuned until the sign was obvious,
        // at the cost of the code drawn on it.
        let mut tokens = crate::studio_dark().clone();
        tokens.color.syntax.added_wash = "{green.500}/f0".into();
        let failures = failures(&tokens);
        assert!(
            failures
                .iter()
                .any(|check| check.background.contains("addedWash")),
            "an opaque wash under body text must fail: {failures:#?}"
        );
    }

    #[test]
    fn every_bundled_theme_draws_lines_somebody_can_see() {
        for tokens in crate::bundled() {
            let failures = line_failures(tokens);
            assert!(
                failures.is_empty(),
                "{} types a line it never draws: {:#?}",
                tokens.meta.id,
                failures
            );
        }
    }

    /// The line rule is a floor and not the 3:1 the control boundaries carry.
    /// A theme whose hairlines clear 3:1 has drawn an outline around every
    /// card and row in the library, which is the failure this whole gate
    /// split exists to end.
    #[test]
    fn a_decorative_line_is_far_below_a_control_boundary() {
        const { assert!(LINE_MINIMUM < SEPARATION_MINIMUM) };
        for tokens in crate::bundled() {
            let canvas = tokens.surface(Surface::Canvas);
            for line in [InteractiveColor::Hairline, InteractiveColor::Divider] {
                let ratio = contrast_ratio(tokens.interactive(line), canvas);
                assert!(
                    ratio < NON_TEXT_MINIMUM,
                    "{} draws {line:?} at {ratio:.2}:1, which is an outline",
                    tokens.meta.id
                );
            }
        }
    }

    /// The regression the line rule exists to catch: an alpha so low the
    /// line composites back into its own surface.
    #[test]
    fn a_line_that_rounds_away_against_its_surface_is_rejected() {
        let mut tokens = crate::studio_dark().clone();
        tokens.color.interactive.hairline = "{neutral.900}/01".into();
        assert!(
            failures(&tokens).is_empty(),
            "the contrast report no longer looks at decorative lines"
        );
        let failures = line_failures(&tokens);
        assert!(
            failures
                .iter()
                .any(|check| check.line == "color.interactive.hairline"),
            "{failures:#?}"
        );
        assert!(matches!(tokens.validate(), Err(crate::TokenError::Line(_))));
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
            let backdrop = lightness(Surface::Backdrop);
            let sunken = lightness(Surface::Sunken);
            let canvas = lightness(Surface::Canvas);
            let panel = lightness(Surface::Panel);
            let raised = lightness(Surface::Raised);
            assert!(
                backdrop < sunken && sunken < canvas && canvas < panel && panel < raised,
                "{} does not order backdrop < sunken < canvas < panel < raised: {backdrop}, \
                 {sunken}, {canvas}, {panel}, {raised}",
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
