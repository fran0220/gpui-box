//! Typed design tokens with no dependency on GPUI or a windowing system.
//!
//! The bundled token document is embedded in the crate, so consumers never
//! depend on a monorepo-relative path. Applications select semantic roles
//! instead of copying colors and metrics into view code.

use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod color;
pub mod contrast;

pub use color::{Color, Palette, contrast_ratio, over};

const STUDIO_DARK_JSON: &str = include_str!("../tokens/studio-dark.json");
const STUDIO_LIGHT_JSON: &str = include_str!("../tokens/studio-light.json");
const CATPPUCCIN_MOCHA_JSON: &str = include_str!("../tokens/catppuccin-mocha.json");
const CATPPUCCIN_LATTE_JSON: &str = include_str!("../tokens/catppuccin-latte.json");
const NORD_JSON: &str = include_str!("../tokens/nord.json");
const TOKYO_NIGHT_JSON: &str = include_str!("../tokens/tokyo-night.json");
const GRUVBOX_DARK_JSON: &str = include_str!("../tokens/gruvbox-dark.json");
const DRACULA_JSON: &str = include_str!("../tokens/dracula.json");
const SOLARIZED_DARK_JSON: &str = include_str!("../tokens/solarized-dark.json");
const SOLARIZED_LIGHT_JSON: &str = include_str!("../tokens/solarized-light.json");
#[cfg(test)]
const TOKEN_SCHEMA_JSON: &str = include_str!("../tokens/schema.json");

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("token JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("token `{path}` is invalid: {message}")]
    Invalid { path: String, message: String },
    #[error("token contrast is invalid:\n{0}")]
    Contrast(String),
    #[error("token surface separation is invalid:\n{0}")]
    Separation(String),
    #[error("token decorative lines are not visible:\n{0}")]
    Line(String),
    #[error("token loading placeholders are outside their loudness band:\n{0}")]
    Placeholder(String),
    #[error("token tones are not distinguishable:\n{0}")]
    Distinction(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Light,
    Dark,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenDocument {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub meta: Metadata,
    pub color: ColorTokens,
    pub space: SpacingTokens,
    pub radius: RadiusTokens,
    pub control: ControlTokens,
    pub border: BorderTokens,
    pub opacity: OpacityTokens,
    pub elevation: ElevationTokens,
    pub z_index: ZIndexTokens,
    pub density: DensityTokens,
    pub typography: TypographyTokens,
    pub motion: MotionTokens,
    pub effect: EffectTokens,
}

impl TokenDocument {
    pub fn parse(json: &str) -> Result<Self, TokenError> {
        let document: Self = serde_json::from_str(json)?;
        document.validate()?;
        Ok(document)
    }

    pub fn validate(&self) -> Result<(), TokenError> {
        if self.schema.trim().is_empty() {
            return invalid("$schema", "must not be empty");
        }
        if self.meta.id.trim().is_empty() {
            return invalid("meta.id", "must not be empty");
        }
        if self.meta.name.trim().is_empty() {
            return invalid("meta.name", "must not be empty");
        }

        for (group, steps) in &self.color.palette {
            for (step, value) in steps {
                Color::parse(&format!("color.palette.{group}.{step}"), value)?;
            }
        }
        for (path, value) in self.color.entries() {
            Color::resolve(&path, value, &self.color.palette)?;
        }
        if self.color.sequence.categorical.len() != SEQUENCE_LENGTH {
            return invalid(
                "color.sequence.categorical",
                &format!(
                    "must carry exactly {SEQUENCE_LENGTH} colors in order, and carries {}",
                    self.color.sequence.categorical.len()
                ),
            );
        }
        for (path, layers) in self.elevation.levels() {
            for (index, layer) in layers.iter().enumerate() {
                Color::resolve(
                    &format!("{path}.{index}.color"),
                    &layer.color,
                    &self.color.palette,
                )?;
                if layer.blur < 0.0 {
                    return invalid(&format!("{path}.{index}"), "blur must not be negative");
                }
            }
        }

        let reaches = [
            step_reach(&self.elevation.flat),
            step_reach(&self.elevation.raised),
            step_reach(&self.elevation.overlay),
            step_reach(&self.elevation.modal),
        ];
        if reaches.windows(2).any(|window| window[0] >= window[1]) {
            return invalid(
                "elevation",
                "steps must strictly increase in reach (y + blur of the farthest layer)",
            );
        }

        let layers = self.z_index.ordered();
        if layers.windows(2).any(|window| window[0].1 >= window[1].1) {
            return invalid("zIndex", "layers must be strictly increasing");
        }

        for (path, scale) in self.density.entries() {
            for (field, value) in [
                ("space", scale.space),
                ("control", scale.control),
                ("font", scale.font),
            ] {
                if !(0.5..=1.5).contains(&value) {
                    return invalid(&format!("{path}.{field}"), "must be between 0.5 and 1.5");
                }
            }
        }
        if self.density.comfortable.space != 1.0
            || self.density.comfortable.control != 1.0
            || self.density.comfortable.font != 1.0
        {
            return invalid(
                "density.comfortable",
                "is the reference density and must scale by exactly 1",
            );
        }

        let spacing = [
            self.space.xs,
            self.space.sm,
            self.space.md,
            self.space.lg,
            self.space.xl,
            self.space.xxl,
        ];
        if spacing.iter().any(|step| *step < 0.0) {
            return invalid("space", "steps must not be negative");
        }
        if spacing.windows(2).any(|window| window[0] >= window[1]) {
            return invalid("space", "steps must be strictly increasing");
        }

        for (path, radius) in [
            ("radius.small", self.radius.small),
            ("radius.control", self.radius.control),
            ("radius.card", self.radius.card),
            ("radius.dialog", self.radius.dialog),
            ("radius.bubble", self.radius.bubble),
            ("radius.pill", self.radius.pill),
        ] {
            if radius < 0.0 {
                return invalid(path, "must not be negative");
            }
        }

        for (path, step) in self.typography.scale.entries() {
            if step.size <= 0.0 || step.line_height < step.size {
                return invalid(path, "requires size > 0 and lineHeight >= size");
            }
            if !(100.0..=900.0).contains(&step.weight) {
                return invalid(path, "weight must be between 100 and 900");
            }
        }

        let heights = [
            self.control.xs.height,
            self.control.sm.height,
            self.control.md.height,
            self.control.lg.height,
        ];
        if heights.windows(2).any(|window| window[0] >= window[1]) {
            return invalid("control", "heights must be strictly increasing");
        }
        for (path, step) in self.control.entries() {
            if step.height <= 0.0 || step.font_size <= 0.0 || step.icon_size <= 0.0 {
                return invalid(path, "height, fontSize and iconSize must be positive");
            }
            if step.padding_x < 0.0 || step.gap < 0.0 {
                return invalid(path, "paddingX and gap must not be negative");
            }
            if step.height < step.font_size {
                return invalid(path, "height must not be smaller than fontSize");
            }
        }

        if self.border.hairline <= 0.0 || self.border.thick <= self.border.hairline {
            return invalid("border", "thick must exceed a positive hairline");
        }

        if self.effect.focus_ring_width <= 0.0 {
            return invalid("effect.focusRingWidth", "must be positive");
        }

        if self.effect.selection_rail_width <= 0.0 {
            return invalid("effect.selectionRailWidth", "must be positive");
        }

        if self.effect.rail_width <= 0.0 {
            return invalid("effect.railWidth", "must be positive");
        }

        for (path, value) in [
            ("effect.edgeFadeBand", self.effect.edge_fade_band),
            ("effect.glowBlur", self.effect.glow_blur),
            ("effect.glassBlur", self.effect.glass_blur),
            ("effect.glassBevel", self.effect.glass_bevel),
            ("effect.glassRefraction", self.effect.glass_refraction),
            (
                "effect.glassMergeDistance",
                self.effect.glass_merge_distance,
            ),
        ] {
            if value < 0.0 {
                return invalid(path, "must not be negative");
            }
        }

        // A bloom pulled in further than it is blurred never reaches the
        // surface's edge, which is a glow the theme pays for and nobody sees.
        if self.effect.glow_spread > 0.0 || self.effect.glow_spread.abs() >= self.effect.glow_blur {
            return invalid(
                "effect.glowSpread",
                "must not be positive and must be pulled in by less than effect.glowBlur",
            );
        }

        if self.effect.glass_specular_sharpness < 1.0 {
            return invalid("effect.glassSpecularSharpness", "must be at least 1");
        }

        // A press that thins the glass would read as the surface receding
        // from the finger that pushed it.
        if self.effect.glass_press_depth < 1.0 {
            return invalid("effect.glassPressDepth", "must be at least 1");
        }

        for (path, value) in [
            ("effect.selectedRingAlpha", self.effect.selected_ring_alpha),
            ("effect.focusRingAlpha", self.effect.focus_ring_alpha),
            ("effect.glowAlpha", self.effect.glow_alpha),
            ("effect.sheenAlpha", self.effect.sheen_alpha),
            ("effect.areaWashAlpha", self.effect.area_wash_alpha),
            ("effect.headerTintAlpha", self.effect.header_tint_alpha),
            ("effect.glassAlpha", self.effect.glass_alpha),
            ("effect.glassLiquidAlpha", self.effect.glass_liquid_alpha),
            ("effect.glassDispersion", self.effect.glass_dispersion),
            ("effect.glassSpecular", self.effect.glass_specular),
            (
                "effect.glassContrastFlipLow",
                self.effect.glass_contrast_flip_low,
            ),
            (
                "effect.glassContrastFlipHigh",
                self.effect.glass_contrast_flip_high,
            ),
            ("opacity.disabled", self.opacity.disabled),
            ("opacity.muted", self.opacity.muted),
            ("opacity.scrim", self.opacity.scrim),
        ] {
            if !(0.0..=1.0).contains(&value) {
                return invalid(path, "must be between 0 and 1");
            }
        }

        // A flip band that crosses over is a theme asking for the oscillation
        // the band exists to prevent.
        if self.effect.glass_contrast_flip_low > self.effect.glass_contrast_flip_high {
            return invalid(
                "effect.glassContrastFlipLow",
                "must not be above effect.glassContrastFlipHigh",
            );
        }

        for preset in SpringPreset::ALL {
            let spring = self.spring(preset);
            if spring.stiffness <= 0.0 || spring.mass <= 0.0 || spring.damping < 0.0 {
                return invalid(
                    &format!("motion.spring.{}", preset.name()),
                    "requires positive stiffness and mass and non-negative damping",
                );
            }
        }

        // A press or hover response that travels further than a hairline stops
        // reading as a response to the pointer and starts reading as a layout
        // change the user did not ask for.
        for (path, value) in [
            ("motion.pressOffsetPx", self.motion.press_offset_px),
            ("motion.hoverLiftPx", self.motion.hover_lift_px),
        ] {
            if !(0.0..=4.0).contains(&value) {
                return invalid(path, "must be between 0 and 4 pixels");
            }
        }

        if self.motion.flick_velocity_px_per_sec <= 0.0 {
            return invalid("motion.flickVelocityPxPerSec", "must be positive");
        }
        // A tension of one passes the whole pull through and is no band at
        // all; a tension of zero refuses to move and reads as a stuck view.
        if !(0.0..=1.0).contains(&self.motion.rubber_band_tension)
            || self.motion.rubber_band_tension == 0.0
        {
            return invalid("motion.rubberBandTension", "must be above 0 and at most 1");
        }

        let failures = contrast::failures(self);
        if !failures.is_empty() {
            return Err(TokenError::Contrast(
                failures
                    .iter()
                    .map(|failure| {
                        format!(
                            "  {} on {} is {:.2}:1; requires {:.1}:1",
                            failure.foreground, failure.background, failure.ratio, failure.minimum
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            ));
        }

        let failures = contrast::separation_failures(self);
        if !failures.is_empty() {
            return Err(TokenError::Separation(
                failures
                    .iter()
                    .map(|failure| {
                        format!(
                            "  {} over {} gains {:.1} L*; requires {:.1}",
                            failure.near, failure.behind, failure.distance, failure.minimum
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            ));
        }

        let failures = contrast::line_failures(self);
        if !failures.is_empty() {
            return Err(TokenError::Line(
                failures
                    .iter()
                    .map(|failure| {
                        format!(
                            "  {} on {} gains {:.2} L*; requires {:.2}",
                            failure.line, failure.surface, failure.distance, failure.minimum
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            ));
        }

        let failures = contrast::placeholder_failures(self);
        if !failures.is_empty() {
            return Err(TokenError::Placeholder(
                failures
                    .iter()
                    .map(|failure| {
                        format!(
                            "  {} over {} reads {:.1} L*; requires {:.1} to {:.1}",
                            failure.role,
                            failure.surface,
                            failure.distance,
                            failure.minimum,
                            failure.maximum
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            ));
        }

        let failures = contrast::distinction_failures(self);
        if !failures.is_empty() {
            return Err(TokenError::Distinction(
                failures
                    .iter()
                    .map(|failure| {
                        format!(
                            "  {} reads {:.1} L* from {}; requires {:.1}",
                            failure.tone, failure.distance, failure.against, failure.minimum
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            ));
        }
        Ok(())
    }

    pub fn surface(&self, role: Surface) -> Color {
        let (path, value) = match role {
            Surface::Backdrop => (
                "color.surface.backdrop",
                self.color.surface.backdrop.as_str(),
            ),
            Surface::Canvas => ("color.surface.canvas", self.color.surface.canvas.as_str()),
            Surface::Sunken => ("color.surface.sunken", self.color.surface.sunken.as_str()),
            Surface::Panel => ("color.surface.panel", self.color.surface.panel.as_str()),
            Surface::Raised => ("color.surface.raised", self.color.surface.raised.as_str()),
            Surface::Overlay => ("color.surface.overlay", self.color.surface.overlay.as_str()),
        };
        self.resolved(path, value)
    }

    /// The modal veil. Separate from [`TokenDocument::surface`] because the
    /// scrim never carries text and never joins the surface ordering.
    pub fn scrim(&self) -> Color {
        self.resolved("color.surface.scrim", self.color.surface.scrim.as_str())
    }

    pub fn text(&self, role: TextTone) -> Color {
        let (path, value) = match role {
            TextTone::Primary => ("color.text.primary", self.color.text.primary.as_str()),
            TextTone::Muted => ("color.text.muted", self.color.text.muted.as_str()),
            TextTone::Faint => ("color.text.faint", self.color.text.faint.as_str()),
            TextTone::Placeholder => (
                "color.text.placeholder",
                self.color.text.placeholder.as_str(),
            ),
            TextTone::Disabled => ("color.text.disabled", self.color.text.disabled.as_str()),
            TextTone::OnAccent => ("color.text.onAccent", self.color.text.on_accent.as_str()),
        };
        self.resolved(path, value)
    }

    pub fn interactive(&self, role: InteractiveColor) -> Color {
        let (path, value) = match role {
            InteractiveColor::Hover => (
                "color.interactive.hover",
                self.color.interactive.hover.as_str(),
            ),
            InteractiveColor::Active => (
                "color.interactive.active",
                self.color.interactive.active.as_str(),
            ),
            InteractiveColor::Selected => (
                "color.interactive.selected",
                self.color.interactive.selected.as_str(),
            ),
            InteractiveColor::Hairline => (
                "color.interactive.hairline",
                self.color.interactive.hairline.as_str(),
            ),
            InteractiveColor::HairlineStrong => (
                "color.interactive.hairlineStrong",
                self.color.interactive.hairline_strong.as_str(),
            ),
            InteractiveColor::Track => (
                "color.interactive.track",
                self.color.interactive.track.as_str(),
            ),
            InteractiveColor::Divider => (
                "color.interactive.divider",
                self.color.interactive.divider.as_str(),
            ),
            InteractiveColor::Focus => (
                "color.interactive.focus",
                self.color.interactive.focus.as_str(),
            ),
        };
        self.resolved(path, value)
    }

    pub fn semantic(&self, role: SemanticColor) -> Color {
        let (path, value) = match role {
            SemanticColor::Accent => ("color.semantic.accent", self.color.semantic.accent.as_str()),
            SemanticColor::AccentStrong => (
                "color.semantic.accentStrong",
                self.color.semantic.accent_strong.as_str(),
            ),
            SemanticColor::Danger => ("color.semantic.danger", self.color.semantic.danger.as_str()),
            SemanticColor::Warning => (
                "color.semantic.warning",
                self.color.semantic.warning.as_str(),
            ),
            SemanticColor::Success => (
                "color.semantic.success",
                self.color.semantic.success.as_str(),
            ),
            SemanticColor::Info => ("color.semantic.info", self.color.semantic.info.as_str()),
        };
        self.resolved(path, value)
    }

    /// The categorical series scale, in the order a chart consumes it.
    ///
    /// Ordered rather than named, because a series is chosen by index: the
    /// third slice of a donut is the third entry, and a caller with more
    /// series than entries cycles. See [`Self::sequence_color`].
    pub fn sequence(&self) -> Vec<Color> {
        (0..self.color.sequence.categorical.len())
            .map(|index| self.sequence_color(index))
            .collect()
    }

    /// The `index`th series colour, cycling past the end of the scale.
    ///
    /// Cycling rather than clamping: a tenth series that repeats the second
    /// colour is a chart the reader can still take apart by position, and one
    /// that repeats the last colour forever is not.
    pub fn sequence_color(&self, index: usize) -> Color {
        let entries = &self.color.sequence.categorical;
        let index = index % entries.len();
        self.resolved(
            &format!("color.sequence.categorical.{index}"),
            &entries[index],
        )
    }

    /// One paint role of the node canvas.
    pub fn node(&self, role: NodeColor) -> Color {
        let value = match role {
            NodeColor::HeaderWash => self.color.node.header_wash.as_str(),
            NodeColor::PortIdle => self.color.node.port_idle.as_str(),
            NodeColor::PortConnected => self.color.node.port_connected.as_str(),
            NodeColor::Edge => self.color.node.edge.as_str(),
            NodeColor::EdgeActive => self.color.node.edge_active.as_str(),
            NodeColor::LabelWash => self.color.node.label_wash.as_str(),
            NodeColor::Grid => self.color.node.grid.as_str(),
            NodeColor::GridStrong => self.color.node.grid_strong.as_str(),
        };
        self.resolved(role.path(), value)
    }

    pub fn agent(&self, role: AgentColor) -> Color {
        let value = match role {
            AgentColor::Read => self.color.agent.read.as_str(),
            AgentColor::Network => self.color.agent.network.as_str(),
            AgentColor::Shell => self.color.agent.shell.as_str(),
            AgentColor::Edit => self.color.agent.edit.as_str(),
            AgentColor::External => self.color.agent.external.as_str(),
            AgentColor::EvidenceWash => self.color.agent.evidence_wash.as_str(),
        };
        self.resolved(role.path(), value)
    }

    pub fn loader(&self, role: LoaderColor) -> Color {
        let value = match role {
            LoaderColor::Mark => self.color.loader.mark.as_str(),
            LoaderColor::Track => self.color.loader.track.as_str(),
            LoaderColor::Placeholder => self.color.loader.placeholder.as_str(),
            LoaderColor::Sheen => self.color.loader.sheen.as_str(),
        };
        self.resolved(role.path(), value)
    }

    /// One paint class of code, from this theme.
    pub fn syntax(&self, class: SyntaxColor) -> Color {
        let value = match class {
            SyntaxColor::Keyword => self.color.syntax.keyword.as_str(),
            SyntaxColor::StringLiteral => self.color.syntax.string.as_str(),
            SyntaxColor::Comment => self.color.syntax.comment.as_str(),
            SyntaxColor::Number => self.color.syntax.number.as_str(),
            SyntaxColor::Inline => self.color.syntax.inline.as_str(),
            SyntaxColor::InlineWash => self.color.syntax.inline_wash.as_str(),
            SyntaxColor::Added => self.color.syntax.added.as_str(),
            SyntaxColor::AddedWash => self.color.syntax.added_wash.as_str(),
            SyntaxColor::Removed => self.color.syntax.removed.as_str(),
            SyntaxColor::RemovedWash => self.color.syntax.removed_wash.as_str(),
        };
        self.resolved(class.path(), value)
    }

    /// The plane a terminal grid is painted on.
    ///
    /// Its own value rather than a surface role, because the two appearances
    /// want opposite ends of the ramp: a terminal is near-black on dark and
    /// near-white on light, and no single surface step is both.
    pub fn terminal_background(&self) -> Color {
        self.resolved(
            "color.terminal.background",
            self.color.terminal.background.as_str(),
        )
    }

    /// The wash over selected cells.
    ///
    /// Achromatic by contract, which is where it differs from
    /// `color.interactive.selected`: a tinted veil drags all sixteen ANSI
    /// hues toward itself, so red under a blue selection reads purple. Only
    /// lightness may change, so the program's colours survive being selected.
    pub fn terminal_selection(&self) -> Color {
        self.resolved(
            "color.terminal.selection",
            self.color.terminal.selection.as_str(),
        )
    }

    /// The sixteen ANSI slots: 0-7 normal, 8-15 bright.
    ///
    /// These are *named* slots ("red", "bright blue"), not literal values, so
    /// every theme retints them — which is why they are tokens and the 6x6x6
    /// colour cube above them is arithmetic. See
    /// `crate::contrast::report` for the floors each slot has to clear on its
    /// own terminal background.
    pub fn terminal_ansi(&self) -> [Color; 16] {
        std::array::from_fn(|index| {
            self.resolved(
                &format!("color.terminal.ansi.{index}"),
                &self.color.terminal.ansi[index],
            )
        })
    }

    fn resolved(&self, path: &str, value: &str) -> Color {
        Color::resolve(path, value, &self.color.palette)
            .expect("the embedded token document is validated before release")
    }

    pub fn spacing(&self, step: Space) -> f32 {
        match step {
            Space::Xs => self.space.xs,
            Space::Sm => self.space.sm,
            Space::Md => self.space.md,
            Space::Lg => self.space.lg,
            Space::Xl => self.space.xl,
            Space::Xxl => self.space.xxl,
        }
    }

    pub fn radius(&self, step: Radius) -> f32 {
        match step {
            Radius::Small => self.radius.small,
            Radius::Control => self.radius.control,
            Radius::Card => self.radius.card,
            Radius::Dialog => self.radius.dialog,
            Radius::Bubble => self.radius.bubble,
            Radius::Pill => self.radius.pill,
        }
    }

    pub fn elevation(&self, level: Elevation) -> ResolvedElevation {
        let (path, layers) = match level {
            Elevation::Flat => ("elevation.flat", &self.elevation.flat),
            Elevation::Raised => ("elevation.raised", &self.elevation.raised),
            Elevation::Overlay => ("elevation.overlay", &self.elevation.overlay),
            Elevation::Modal => ("elevation.modal", &self.elevation.modal),
        };
        ResolvedElevation {
            layers: layers
                .iter()
                .enumerate()
                .map(|(index, layer)| ResolvedElevationLayer {
                    y: layer.y,
                    blur: layer.blur,
                    spread: layer.spread,
                    color: self.resolved(&format!("{path}.{index}.color"), &layer.color),
                })
                .collect(),
        }
    }

    pub fn z_index(&self, layer: Layer) -> i32 {
        match layer {
            Layer::Content => self.z_index.content,
            Layer::Sticky => self.z_index.sticky,
            Layer::Dock => self.z_index.dock,
            Layer::Popover => self.z_index.popover,
            Layer::Tooltip => self.z_index.tooltip,
            Layer::Modal => self.z_index.modal,
            Layer::Toast => self.z_index.toast,
        }
    }

    pub fn density(&self, density: Density) -> DensityScale {
        match density {
            Density::Compact => self.density.compact,
            Density::Comfortable => self.density.comfortable,
        }
    }

    pub fn spring(&self, spring: SpringPreset) -> SpringTokens {
        match spring {
            SpringPreset::Snappy => self.motion.spring.snappy,
            SpringPreset::Smooth => self.motion.spring.smooth,
            SpringPreset::Bouncy => self.motion.spring.bouncy,
            SpringPreset::Grab => self.motion.spring.grab,
        }
    }

    pub fn control(&self, size: ControlSize) -> &ControlStep {
        match size {
            ControlSize::Xs => &self.control.xs,
            ControlSize::Sm => &self.control.sm,
            ControlSize::Md => &self.control.md,
            ControlSize::Lg => &self.control.lg,
        }
    }

    pub fn border_width(&self, weight: BorderWeight) -> f32 {
        match weight {
            BorderWeight::Hairline => self.border.hairline,
            BorderWeight::Thick => self.border.thick,
        }
    }

    pub fn opacity(&self, role: OpacityRole) -> f32 {
        match role {
            OpacityRole::Disabled => self.opacity.disabled,
            OpacityRole::Muted => self.opacity.muted,
            OpacityRole::Scrim => self.opacity.scrim,
        }
    }

    pub fn type_step(&self, step: TypeScale) -> &TypeStep {
        match step {
            TypeScale::Caption => &self.typography.scale.caption,
            TypeScale::Label => &self.typography.scale.label,
            TypeScale::Body => &self.typography.scale.body,
            TypeScale::Strong => &self.typography.scale.strong,
            TypeScale::Subtitle => &self.typography.scale.subtitle,
            TypeScale::Title => &self.typography.scale.title,
            TypeScale::Code => &self.typography.scale.code,
        }
    }

    pub fn motion_duration(&self, step: MotionDuration) -> Duration {
        Duration::from_millis(match step {
            MotionDuration::Instant => self.motion.duration_ms.instant,
            MotionDuration::Quick => self.motion.duration_ms.quick,
            MotionDuration::Menu => self.motion.duration_ms.menu,
            MotionDuration::Dialog => self.motion.duration_ms.dialog,
            MotionDuration::Resize => self.motion.duration_ms.resize,
            MotionDuration::Entrance => self.motion.duration_ms.entrance,
            MotionDuration::Spin => self.motion.duration_ms.spin,
            MotionDuration::Slow => self.motion.duration_ms.slow,
            MotionDuration::StaggerStep => self.motion.duration_ms.stagger_step,
            MotionDuration::Pulse => self.motion.duration_ms.pulse,
            MotionDuration::Shimmer => self.motion.duration_ms.shimmer,
            MotionDuration::Toast => self.motion.duration_ms.toast,
        })
    }

    /// How far a pressed control sinks, in pixels.
    pub fn press_offset(&self) -> f32 {
        self.motion.press_offset_px
    }

    /// How far a hovered control rises, in pixels.
    pub fn hover_lift(&self) -> f32 {
        self.motion.hover_lift_px
    }

    /// The speed past which a released gesture counts as a flick, in pixels a
    /// second.
    pub fn flick_velocity(&self) -> f32 {
        self.motion.flick_velocity_px_per_sec
    }

    /// How much of an overscroll is shown at the boundary.
    pub fn rubber_band_tension(&self) -> f32 {
        self.motion.rubber_band_tension
    }

    pub fn easing(&self, step: MotionEasing) -> [f32; 4] {
        match step {
            MotionEasing::Linear => self.motion.easing.linear,
            MotionEasing::Standard => self.motion.easing.standard,
            MotionEasing::EaseIn => self.motion.easing.ease_in,
            MotionEasing::EaseOut => self.motion.easing.ease_out,
            MotionEasing::EaseInOut => self.motion.easing.ease_in_out,
            MotionEasing::Emphasized => self.motion.easing.emphasized,
            MotionEasing::Overshoot => self.motion.easing.overshoot,
            MotionEasing::Exit => self.motion.easing.exit,
            MotionEasing::Settle => self.motion.easing.settle,
        }
    }
}

fn invalid<T>(path: &str, message: &str) -> Result<T, TokenError> {
    Err(TokenError::Invalid {
        path: path.into(),
        message: message.into(),
    })
}

pub fn studio_dark() -> &'static TokenDocument {
    static TOKENS: OnceLock<TokenDocument> = OnceLock::new();
    TOKENS.get_or_init(|| {
        TokenDocument::parse(STUDIO_DARK_JSON)
            .expect("tokens/studio-dark.json must pass TokenDocument::validate")
    })
}

pub fn studio_light() -> &'static TokenDocument {
    static TOKENS: OnceLock<TokenDocument> = OnceLock::new();
    TOKENS.get_or_init(|| {
        TokenDocument::parse(STUDIO_LIGHT_JSON)
            .expect("tokens/studio-light.json must pass TokenDocument::validate")
    })
}

/// The two themes this library designs against, in registration order.
///
/// The presets in [`presets`] are deliberately not here: this set is what the
/// visual baselines are captured in, and every scene is rendered once per
/// member of it. See [`all`] for the whole shipped catalog.
pub fn bundled() -> [&'static TokenDocument; 2] {
    [studio_dark(), studio_light()]
}

macro_rules! preset {
    ($name:ident, $json:ident, $file:literal) => {
        #[doc = concat!("The `", $file, "` preset, transcribed from its upstream scheme.")]
        pub fn $name() -> &'static TokenDocument {
            static TOKENS: OnceLock<TokenDocument> = OnceLock::new();
            TOKENS.get_or_init(|| {
                TokenDocument::parse($json).expect(concat!(
                    "tokens/",
                    $file,
                    " must pass TokenDocument::validate"
                ))
            })
        }
    };
}

preset!(
    catppuccin_mocha,
    CATPPUCCIN_MOCHA_JSON,
    "catppuccin-mocha.json"
);
preset!(
    catppuccin_latte,
    CATPPUCCIN_LATTE_JSON,
    "catppuccin-latte.json"
);
preset!(nord, NORD_JSON, "nord.json");
preset!(tokyo_night, TOKYO_NIGHT_JSON, "tokyo-night.json");
preset!(gruvbox_dark, GRUVBOX_DARK_JSON, "gruvbox-dark.json");
preset!(dracula, DRACULA_JSON, "dracula.json");
preset!(solarized_dark, SOLARIZED_DARK_JSON, "solarized-dark.json");
preset!(
    solarized_light,
    SOLARIZED_LIGHT_JSON,
    "solarized-light.json"
);

/// The community schemes shipped alongside the studio pair, in registration
/// order.
pub fn presets() -> [&'static TokenDocument; 8] {
    [
        catppuccin_mocha(),
        catppuccin_latte(),
        nord(),
        tokyo_night(),
        gruvbox_dark(),
        dracula(),
        solarized_dark(),
        solarized_light(),
    ]
}

/// Every theme shipped with the library: the studio pair, then the presets.
pub fn all() -> Vec<&'static TokenDocument> {
    bundled().into_iter().chain(presets()).collect()
}

pub fn studio_dark_json() -> &'static str {
    STUDIO_DARK_JSON
}

pub fn studio_light_json() -> &'static str {
    STUDIO_LIGHT_JSON
}

pub fn bundled_json() -> [&'static str; 2] {
    [STUDIO_DARK_JSON, STUDIO_LIGHT_JSON]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// The substrate behind the page. A card can sit on it; a well cannot.
    Backdrop,
    Canvas,
    /// Recessed below whatever carries it: the well an editable value sits in.
    Sunken,
    Panel,
    Raised,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTone {
    Primary,
    Muted,
    Faint,
    Placeholder,
    Disabled,
    OnAccent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveColor {
    Hover,
    Active,
    Selected,
    Hairline,
    HairlineStrong,
    Track,
    Divider,
    Focus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticColor {
    Accent,
    AccentStrong,
    Danger,
    Warning,
    Success,
    Info,
}

/// The four paint roles of work in progress. See [`LoaderColors`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderColor {
    /// The moving part: a bar's fill, a spinner's arc, a breathing dot.
    Mark,
    /// The groove the mark travels, quieter than the mark by construction.
    Track,
    /// The shape of content that is not there yet.
    Placeholder,
    /// The highlight that crosses a placeholder.
    Sheen,
}

impl LoaderColor {
    pub const ALL: [Self; 4] = [Self::Mark, Self::Track, Self::Placeholder, Self::Sheen];

    pub fn path(self) -> &'static str {
        match self {
            Self::Mark => "color.loader.mark",
            Self::Track => "color.loader.track",
            Self::Placeholder => "color.loader.placeholder",
            Self::Sheen => "color.loader.sheen",
        }
    }
}

/// Paint roles used by quiet evidence inside an agent transcript.
///
/// The five families classify a tool without colouring the tool's arguments
/// or result. `EvidenceWash` is the optional, deliberately faint plane under
/// expanded caller-owned text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentColor {
    Read,
    Network,
    Shell,
    Edit,
    External,
    EvidenceWash,
}

impl AgentColor {
    pub const ALL: [Self; 6] = [
        Self::Read,
        Self::Network,
        Self::Shell,
        Self::Edit,
        Self::External,
        Self::EvidenceWash,
    ];

    pub fn path(self) -> &'static str {
        match self {
            Self::Read => "color.agent.read",
            Self::Network => "color.agent.network",
            Self::Shell => "color.agent.shell",
            Self::Edit => "color.agent.edit",
            Self::External => "color.agent.external",
            Self::EvidenceWash => "color.agent.evidenceWash",
        }
    }
}

/// One paint role of the node canvas. See [`NodeColors`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeColor {
    HeaderWash,
    PortIdle,
    PortConnected,
    Edge,
    EdgeActive,
    LabelWash,
    Grid,
    GridStrong,
}

impl NodeColor {
    pub const ALL: [Self; 8] = [
        Self::HeaderWash,
        Self::PortIdle,
        Self::PortConnected,
        Self::Edge,
        Self::EdgeActive,
        Self::LabelWash,
        Self::Grid,
        Self::GridStrong,
    ];

    pub fn path(self) -> &'static str {
        match self {
            Self::HeaderWash => "color.node.headerWash",
            Self::PortIdle => "color.node.portIdle",
            Self::PortConnected => "color.node.portConnected",
            Self::Edge => "color.node.edge",
            Self::EdgeActive => "color.node.edgeActive",
            Self::LabelWash => "color.node.labelWash",
            Self::Grid => "color.node.grid",
            Self::GridStrong => "color.node.gridStrong",
        }
    }
}

/// One paint class of code. See [`SyntaxColors`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxColor {
    Keyword,
    StringLiteral,
    Comment,
    Number,
    Inline,
    InlineWash,
    Added,
    AddedWash,
    Removed,
    RemovedWash,
}

impl SyntaxColor {
    /// Every class, so a caller building a palette map covers the set by
    /// construction rather than by remembering it.
    pub const ALL: [Self; 10] = [
        Self::Keyword,
        Self::StringLiteral,
        Self::Comment,
        Self::Number,
        Self::Inline,
        Self::InlineWash,
        Self::Added,
        Self::AddedWash,
        Self::Removed,
        Self::RemovedWash,
    ];

    pub fn path(self) -> &'static str {
        match self {
            Self::Keyword => "color.syntax.keyword",
            Self::StringLiteral => "color.syntax.string",
            Self::Comment => "color.syntax.comment",
            Self::Number => "color.syntax.number",
            Self::Inline => "color.syntax.inline",
            Self::InlineWash => "color.syntax.inlineWash",
            Self::Added => "color.syntax.added",
            Self::AddedWash => "color.syntax.addedWash",
            Self::Removed => "color.syntax.removed",
            Self::RemovedWash => "color.syntax.removedWash",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Space {
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
    Xxl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Radius {
    Small,
    Control,
    Card,
    Dialog,
    Bubble,
    Pill,
}

/// The four control heights every interactive component resolves against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub enum ControlSize {
    Xs,
    Sm,
    #[default]
    Md,
    Lg,
}

impl ControlSize {
    pub const ALL: [Self; 4] = [Self::Xs, Self::Sm, Self::Md, Self::Lg];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Elevation {
    #[default]
    Flat,
    Raised,
    Overlay,
    Modal,
}

/// Painting order for floating surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Content,
    Sticky,
    Dock,
    Popover,
    Tooltip,
    Modal,
    Toast,
}

impl Layer {
    pub const ALL: [Self; 7] = [
        Self::Content,
        Self::Sticky,
        Self::Dock,
        Self::Popover,
        Self::Tooltip,
        Self::Modal,
        Self::Toast,
    ];
}

/// The information-density axis applications expose as a preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Density {
    Compact,
    #[default]
    Comfortable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpringPreset {
    Snappy,
    Smooth,
    Bouncy,
    /// Direct manipulation: tight enough that the element reads as attached to
    /// the pointer rather than trailing it.
    Grab,
}

impl SpringPreset {
    pub const ALL: [Self; 4] = [Self::Snappy, Self::Smooth, Self::Bouncy, Self::Grab];

    pub fn name(self) -> &'static str {
        match self {
            Self::Snappy => "snappy",
            Self::Smooth => "smooth",
            Self::Bouncy => "bouncy",
            Self::Grab => "grab",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedElevationLayer {
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
}

/// The resolved shadows one elevation step casts.
///
/// An empty set is a flat surface: it allocates no shadow work. `reach` is
/// the farthest any layer extends (`y + blur`), and is how the four steps
/// are ordered.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedElevation {
    pub layers: Vec<ResolvedElevationLayer>,
}

impl ResolvedElevation {
    pub fn reach(&self) -> f32 {
        self.layers
            .iter()
            .map(|layer| layer.y + layer.blur)
            .fold(0.0, f32::max)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderWeight {
    Hairline,
    Thick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpacityRole {
    Disabled,
    Muted,
    Scrim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeScale {
    Caption,
    Label,
    Body,
    /// Body at emphasis weight. Same size and line height, so a run can be
    /// emphasised without changing the line box it sits in.
    Strong,
    /// A heading inside a component, between body and the component's own name.
    Subtitle,
    Title,
    Code,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionDuration {
    Instant,
    Quick,
    Menu,
    Dialog,
    Resize,
    Entrance,
    /// One turn of a mark that reports work nobody can size.
    Spin,
    Slow,
    /// The gap between one member of a staggered group and the next.
    StaggerStep,
    Pulse,
    /// How long a loading placeholder's highlight takes to cross it.
    Shimmer,
    /// How long a transient notification stays before it leaves on its own.
    Toast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionEasing {
    Linear,
    Standard,
    EaseIn,
    EaseOut,
    EaseInOut,
    Emphasized,
    Overshoot,
    Exit,
    Settle,
}

impl MotionEasing {
    pub const ALL: [Self; 9] = [
        Self::Linear,
        Self::Standard,
        Self::EaseIn,
        Self::EaseOut,
        Self::EaseInOut,
        Self::Emphasized,
        Self::Overshoot,
        Self::Exit,
        Self::Settle,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Standard => "standard",
            Self::EaseIn => "easeIn",
            Self::EaseOut => "easeOut",
            Self::EaseInOut => "easeInOut",
            Self::Emphasized => "emphasized",
            Self::Overshoot => "overshoot",
            Self::Exit => "exit",
            Self::Settle => "settle",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub id: String,
    pub name: String,
    pub appearance: Appearance,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ElevationTokens {
    pub flat: Vec<ElevationLayer>,
    pub raised: Vec<ElevationLayer>,
    pub overlay: Vec<ElevationLayer>,
    pub modal: Vec<ElevationLayer>,
}

impl ElevationTokens {
    fn levels(&self) -> [(&'static str, &[ElevationLayer]); 4] {
        [
            ("elevation.flat", &self.flat),
            ("elevation.raised", &self.raised),
            ("elevation.overlay", &self.overlay),
            ("elevation.modal", &self.modal),
        ]
    }
}

/// One shadow in an elevation step.
///
/// There is no horizontal offset: a layer is a downward cast, and a close
/// contact shadow is `y` plus `blur`, not a sideways smear.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ElevationLayer {
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: String,
}

fn step_reach(layers: &[ElevationLayer]) -> f32 {
    layers
        .iter()
        .map(|layer| layer.y + layer.blur)
        .fold(0.0, f32::max)
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ZIndexTokens {
    pub content: i32,
    pub sticky: i32,
    pub dock: i32,
    pub popover: i32,
    pub tooltip: i32,
    pub modal: i32,
    pub toast: i32,
}

impl ZIndexTokens {
    fn ordered(&self) -> [(&'static str, i32); 7] {
        [
            ("content", self.content),
            ("sticky", self.sticky),
            ("dock", self.dock),
            ("popover", self.popover),
            ("tooltip", self.tooltip),
            ("modal", self.modal),
            ("toast", self.toast),
        ]
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DensityTokens {
    pub compact: DensityScale,
    pub comfortable: DensityScale,
}

impl DensityTokens {
    fn entries(&self) -> [(&'static str, DensityScale); 2] {
        [
            ("density.compact", self.compact),
            ("density.comfortable", self.comfortable),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DensityScale {
    pub space: f32,
    pub control: f32,
    pub font: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpringTokens {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpringPresetTokens {
    pub snappy: SpringTokens,
    pub smooth: SpringTokens,
    pub bouncy: SpringTokens,
    pub grab: SpringTokens,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ColorTokens {
    pub palette: color::Palette,
    pub surface: SurfaceColors,
    pub text: TextColors,
    pub interactive: InteractiveColors,
    pub semantic: SemanticColors,
    pub sequence: SequenceColors,
    pub node: NodeColors,
    pub agent: AgentColors,
    pub loader: LoaderColors,
    pub syntax: SyntaxColors,
    pub terminal: TerminalColors,
}

impl ColorTokens {
    /// Every role, paired with the source string the document declares it as.
    ///
    /// A `Vec` of owned paths rather than a fixed array, because the series
    /// scale is addressed by index and has no name to be `'static` about.
    fn entries(&self) -> Vec<(String, &str)> {
        let fixed: [(&'static str, &str); 55] = [
            ("color.agent.read", &self.agent.read),
            ("color.agent.network", &self.agent.network),
            ("color.agent.shell", &self.agent.shell),
            ("color.agent.edit", &self.agent.edit),
            ("color.agent.external", &self.agent.external),
            ("color.agent.evidenceWash", &self.agent.evidence_wash),
            ("color.syntax.keyword", &self.syntax.keyword),
            ("color.syntax.string", &self.syntax.string),
            ("color.syntax.comment", &self.syntax.comment),
            ("color.syntax.number", &self.syntax.number),
            ("color.syntax.inline", &self.syntax.inline),
            ("color.syntax.inlineWash", &self.syntax.inline_wash),
            ("color.syntax.added", &self.syntax.added),
            ("color.syntax.addedWash", &self.syntax.added_wash),
            ("color.syntax.removed", &self.syntax.removed),
            ("color.syntax.removedWash", &self.syntax.removed_wash),
            ("color.surface.backdrop", &self.surface.backdrop),
            ("color.surface.canvas", &self.surface.canvas),
            ("color.surface.sunken", &self.surface.sunken),
            ("color.surface.panel", &self.surface.panel),
            ("color.surface.raised", &self.surface.raised),
            ("color.surface.overlay", &self.surface.overlay),
            ("color.surface.scrim", &self.surface.scrim),
            ("color.text.primary", &self.text.primary),
            ("color.text.muted", &self.text.muted),
            ("color.text.faint", &self.text.faint),
            ("color.text.placeholder", &self.text.placeholder),
            ("color.text.disabled", &self.text.disabled),
            ("color.text.onAccent", &self.text.on_accent),
            ("color.interactive.hover", &self.interactive.hover),
            ("color.interactive.active", &self.interactive.active),
            ("color.interactive.selected", &self.interactive.selected),
            ("color.interactive.hairline", &self.interactive.hairline),
            (
                "color.interactive.hairlineStrong",
                &self.interactive.hairline_strong,
            ),
            ("color.interactive.track", &self.interactive.track),
            ("color.interactive.divider", &self.interactive.divider),
            ("color.interactive.focus", &self.interactive.focus),
            ("color.semantic.accent", &self.semantic.accent),
            ("color.semantic.accentStrong", &self.semantic.accent_strong),
            ("color.semantic.danger", &self.semantic.danger),
            ("color.semantic.warning", &self.semantic.warning),
            ("color.semantic.success", &self.semantic.success),
            ("color.semantic.info", &self.semantic.info),
            ("color.loader.mark", &self.loader.mark),
            ("color.loader.track", &self.loader.track),
            ("color.loader.placeholder", &self.loader.placeholder),
            ("color.loader.sheen", &self.loader.sheen),
            ("color.node.headerWash", &self.node.header_wash),
            ("color.node.portIdle", &self.node.port_idle),
            ("color.node.portConnected", &self.node.port_connected),
            ("color.node.edge", &self.node.edge),
            ("color.node.edgeActive", &self.node.edge_active),
            ("color.node.labelWash", &self.node.label_wash),
            ("color.node.grid", &self.node.grid),
            ("color.node.gridStrong", &self.node.grid_strong),
        ];
        fixed
            .into_iter()
            .map(|(path, value)| (path.to_string(), value))
            .chain(
                self.sequence
                    .categorical
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        (
                            format!("color.sequence.categorical.{index}"),
                            value.as_str(),
                        )
                    }),
            )
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceColors {
    /// The substrate behind the page. A card can sit on it; a well cannot,
    /// which is why it is not compared to `sunken`.
    pub backdrop: String,
    pub canvas: String,
    /// The well an editable value sits in, recessed below the surface that
    /// carries it. It is what a text field is made of once the field has no
    /// border to be made of instead.
    pub sunken: String,
    pub panel: String,
    pub raised: String,
    pub overlay: String,
    /// The veil painted over the page behind a modal surface, at
    /// `opacity.scrim`. Not a text-bearing surface, so it takes no part in
    /// the surface ordering: on a near-black backdrop a pure-black veil
    /// disappears, which is why dark themes declare a cast the page does not
    /// have instead of more black.
    pub scrim: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextColors {
    pub primary: String,
    pub muted: String,
    pub faint: String,
    pub placeholder: String,
    pub disabled: String,
    pub on_accent: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InteractiveColors {
    pub hover: String,
    pub active: String,
    pub selected: String,
    pub hairline: String,
    pub hairline_strong: String,
    pub track: String,
    pub divider: String,
    pub focus: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticColors {
    pub accent: String,
    pub accent_strong: String,
    pub danger: String,
    pub warning: String,
    pub success: String,
    pub info: String,
}

/// How many colours a categorical series scale carries.
///
/// Eight is a series a reader can hold at once and a legend can name. It is
/// fixed rather than open so a chart that cycles the scale repeats a colour
/// at a position both the theme and the caller can predict.
pub const SEQUENCE_LENGTH: usize = 8;

/// The categorical series scale: distinct hues in a fixed order.
///
/// The hues differ rather than the lightnesses, which is the whole point. A
/// chart drawn in four steps of one hue tells a reader that its four series
/// are four degrees of one thing, and the four series of a donut are not
/// ordered at all. The order here is the order a series takes them in, so two
/// charts of the same data agree on which slice is which.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SequenceColors {
    pub categorical: Vec<String>,
}

/// The vocabulary a node canvas paints in.
///
/// A graph is read by following its connections, so the parts that carry the
/// connection are roles rather than one grey borrowed from the control
/// vocabulary: an idle port and a connected one are different facts, and a
/// resting edge and a live one are different facts, and neither pair can be
/// told apart when both are `interactive.hairline`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeColors {
    /// The tint behind a node's header band when the node declares no
    /// category colour of its own. A band on the node's own surface, not a
    /// second surface: it is held to the line floor rather than the surface
    /// separation one.
    pub header_wash: String,
    /// An unconnected port.
    pub port_idle: String,
    /// A port an edge lands on. Further from the canvas than `portIdle` by
    /// contract, because "attached" is the state a reader scans a graph for.
    pub port_connected: String,
    /// A resting connection. Quiet by design: a canvas is mostly edges, and
    /// an edge drawn at a control boundary's loudness turns the graph into a
    /// mesh.
    pub edge: String,
    /// A connection carrying traffic, or under the pointer.
    pub edge_active: String,
    /// The chip behind an edge label. Nearly opaque, because the label sits
    /// on the canvas *and* on whatever edge passes under it.
    pub label_wash: String,
    /// The canvas dot grid, and its major interval. Barely visible on
    /// purpose, which is why both are held to the line floor: a grid typed at
    /// an alpha that rounds away is a canvas the author believes is ruled.
    pub grid: String,
    pub grid_strong: String,
}

/// Quiet classification paint for evidence in an agent transcript.
///
/// These are not state severities. A failed shell call still uses danger for
/// its failure mark; `shell` only identifies what kind of work the row did.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentColors {
    pub read: String,
    pub network: String,
    pub shell: String,
    pub edit: String,
    pub external: String,
    pub evidence_wash: String,
}

/// The vocabulary of work in progress.
///
/// `mark` is the moving part — a bar's fill, a spinner's arc, a breathing
/// dot. Every theme points it at its accent so the moving part is legible at
/// any stroke, and it is the only member held to a legibility floor; a
/// caller who means something beyond "working" still says so with its own
/// tone. The quiet roles stay grey: waiting has one voice, not four.
/// `track` is the groove that mark travels, quieter
/// than the mark by construction. `placeholder` is the shape of absent
/// content, held *inside* a loudness band because a skeleton that outshouts
/// real content is a defect, not an emphasis. `sheen` is the highlight that
/// crosses a placeholder, measured against the placeholder it crosses.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LoaderColors {
    pub mark: String,
    pub track: String,
    pub placeholder: String,
    pub sheen: String,
}

/// The vocabulary code is painted in, wherever this library draws code.
///
/// These are token roles rather than a component's constants for the same
/// reason the ANSI table is: every one of them changes with the theme, and a
/// renderer that branched on the appearance itself would be a second theme
/// system. They are held to their own contrast floors against the surfaces
/// code actually sits on; see [`contrast::report`].
///
/// The set is deliberately four classes and not a grammar. A tokenizer that
/// distinguished forty kinds of thing would need forty tokens in every theme,
/// and a reader scanning a block for a string literal is served by four.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyntaxColors {
    pub keyword: String,
    pub string: String,
    /// Quiet by contract: a comment is supporting detail inside code the way
    /// `text.faint` is beside prose, and carries that role's floor.
    pub comment: String,
    pub number: String,
    /// Inline code inside prose, and the wash it sits on. A span of code in a
    /// sentence is not a code block: it keeps the paragraph's rhythm and only
    /// changes face and tone, so it has its own pair rather than borrowing the
    /// block's surface.
    pub inline: String,
    pub inline_wash: String,
    /// A diff's two sides. The text tone is what the line's characters are
    /// drawn in; the wash is the band under the whole line, which is why they
    /// are separate values and not one colour at two alphas: the wash has to
    /// stay quiet enough for the text on it to clear its own floor.
    pub added: String,
    pub added_wash: String,
    pub removed: String,
    pub removed_wash: String,
}

/// The vocabulary a terminal grid paints in.
///
/// It lives in the token document rather than beside the component because
/// every one of these values changes with the theme, and a component that
/// branched on the appearance itself would be a second theme system with one
/// customer.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalColors {
    pub background: String,
    pub selection: String,
    /// Slots 0-7 normal, 8-15 bright.
    pub ansi: [String; 16],
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpacingTokens {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RadiusTokens {
    pub small: f32,
    pub control: f32,
    pub card: f32,
    pub dialog: f32,
    pub bubble: f32,
    pub pill: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ControlTokens {
    pub xs: ControlStep,
    pub sm: ControlStep,
    pub md: ControlStep,
    pub lg: ControlStep,
}

impl ControlTokens {
    fn entries(&self) -> [(&'static str, &ControlStep); 4] {
        [
            ("control.xs", &self.xs),
            ("control.sm", &self.sm),
            ("control.md", &self.md),
            ("control.lg", &self.lg),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlStep {
    pub height: f32,
    pub padding_x: f32,
    pub gap: f32,
    pub font_size: f32,
    pub icon_size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BorderTokens {
    pub hairline: f32,
    pub thick: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OpacityTokens {
    pub disabled: f32,
    pub muted: f32,
    pub scrim: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypographyTokens {
    pub sans: FontTokens,
    pub mono: FontTokens,
    pub scale: TypeScaleTokens,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontTokens {
    pub family: String,
    pub fallback_macos: String,
    pub fallback_windows: String,
    pub fallback_linux: String,
}

impl FontTokens {
    pub fn platform_fallback(&self) -> &str {
        if cfg!(target_os = "macos") {
            &self.fallback_macos
        } else if cfg!(target_os = "windows") {
            &self.fallback_windows
        } else {
            &self.fallback_linux
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypeScaleTokens {
    pub caption: TypeStep,
    pub label: TypeStep,
    pub body: TypeStep,
    pub strong: TypeStep,
    pub subtitle: TypeStep,
    pub title: TypeStep,
    pub code: TypeStep,
}

impl TypeScaleTokens {
    fn entries(&self) -> [(&'static str, &TypeStep); 7] {
        [
            ("typography.scale.caption", &self.caption),
            ("typography.scale.label", &self.label),
            ("typography.scale.body", &self.body),
            ("typography.scale.strong", &self.strong),
            ("typography.scale.subtitle", &self.subtitle),
            ("typography.scale.title", &self.title),
            ("typography.scale.code", &self.code),
        ]
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypeStep {
    pub size: f32,
    pub line_height: f32,
    pub weight: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotionTokens {
    pub duration_ms: DurationTokens,
    pub easing: EasingTokens,
    pub spring: SpringPresetTokens,
    /// How far a control sinks while the pointer is held on it.
    pub press_offset_px: f32,
    /// How far a control rises under the pointer.
    pub hover_lift_px: f32,
    /// The speed, in pixels a second, past which a released gesture is a
    /// flick rather than a drag that happened to end.
    pub flick_velocity_px_per_sec: f32,
    /// The fraction of an overscroll that is shown at the boundary, before
    /// the band starts tightening.
    pub rubber_band_tension: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DurationTokens {
    pub instant: u64,
    pub quick: u64,
    pub menu: u64,
    pub dialog: u64,
    pub resize: u64,
    pub entrance: u64,
    /// How long one full turn of a spinning mark takes.
    pub spin: u64,
    /// The longest step a user waits through deliberately: a panel taking a
    /// region over, or a graph laying itself out.
    pub slow: u64,
    /// The gap between one member of a staggered group and the next.
    pub stagger_step: u64,
    pub pulse: u64,
    /// How long a loading placeholder's highlight takes to cross it.
    pub shimmer: u64,
    pub toast: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EasingTokens {
    pub linear: [f32; 4],
    pub standard: [f32; 4],
    pub ease_in: [f32; 4],
    pub ease_out: [f32; 4],
    pub ease_in_out: [f32; 4],
    pub emphasized: [f32; 4],
    pub overshoot: [f32; 4],
    pub exit: [f32; 4],
    pub settle: [f32; 4],
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectTokens {
    pub edge_fade_band: f32,
    pub selected_ring_alpha: f32,
    /// How wide the bar marking the selected row in a collection is, in
    /// pixels.
    ///
    /// A row's selection is a wash plus this rail at the reading edge. The
    /// wash alone is a neutral tint that a light theme cannot make strong
    /// enough to read as *chosen* without also reading as *disabled*; the
    /// rail is what says which row the collection is on, at a colour and an
    /// area small enough that a hundred rows do not become a field of accent.
    pub selection_rail_width: f32,
    /// How wide the ring around the focused control is drawn, in pixels.
    pub focus_ring_width: f32,
    pub focus_ring_alpha: f32,
    /// How strongly a surface that carries a state colour bleeds it into the
    /// pixels around its edge.
    pub glow_alpha: f32,
    /// How far that bleed reaches, in pixels.
    pub glow_blur: f32,
    /// The bloom budget: how far the glow is pulled in before it is blurred,
    /// in pixels. Negative by design.
    ///
    /// A blurred shadow at spread zero puts its full alpha at the surface's
    /// own edge and reaches `glowBlur` past it in every direction, which is
    /// how a failed card came to tint the card beside it and a running node
    /// came to be cut flat at the edge of its canvas. Pulling the shape in
    /// first keeps the state readable at the edge it belongs to and keeps it
    /// out of its neighbours' pixels. The validator holds `|glowSpread|`
    /// below `glowBlur`, because a bloom pulled in further than it is blurred
    /// is a glow nobody sees.
    pub glow_spread: f32,
    /// How opaque a frosted surface's own fill is over what it blurs. A theme
    /// that sets this to 1 declares itself opaque, and a frosted surface then
    /// paints no blur at all rather than blurring pixels nobody can see.
    pub glass_alpha: f32,
    /// How far a frosted surface blurs what is behind it, in pixels.
    pub glass_blur: f32,
    /// How much of a glass surface is tint when the optics are what makes it
    /// legible rather than the fill.
    ///
    /// This is well below `glassAlpha` on purpose. A frosted surface has only
    /// its fill to separate it from the backdrop, so the fill must be strong;
    /// a surface that bends, splits and lights its edge is read by those, and
    /// a fill strong enough to stand alone would cover them.
    pub glass_liquid_alpha: f32,
    /// How far in from its edge a glass surface's bevel reaches, in pixels.
    /// This is the band the optics act in: refraction, dispersion and the
    /// specular rim all fall to nothing once the surface is this deep.
    pub glass_bevel: f32,
    /// How far the bevel displaces what is behind it, as a fraction of the
    /// bevel. Read as the thickness of the glass body.
    pub glass_refraction: f32,
    /// How far apart the red and blue samples land at the bevel, as a fraction
    /// of the refraction offset. Zero is a colourless bend.
    pub glass_dispersion: f32,
    /// Peak brightness of the rim highlight.
    pub glass_specular: f32,
    /// How tight that highlight is. Larger is smaller and harder.
    pub glass_specular_sharpness: f32,
    /// Where the light that makes the highlight is, in radians clockwise from
    /// straight up. A surface that tracks the pointer starts here.
    pub glass_light_angle: f32,
    /// How close two glass surfaces in a group must be to join into one body,
    /// in pixels.
    pub glass_merge_distance: f32,
    /// Backdrop luminance below which a glass surface carries light content,
    /// and above which it carries dark. The two are apart rather than equal on
    /// purpose: a single threshold makes a surface over a backdrop sitting on
    /// it flip back and forth, and the gap is what stops that.
    pub glass_contrast_flip_low: f32,
    pub glass_contrast_flip_high: f32,
    /// How much thicker a pressable glass surface reads while pressed, as a
    /// factor on its refraction. 1 is a surface that does not deform.
    pub glass_press_depth: f32,
    /// How strongly a raised surface catches light along its top edge.
    ///
    /// The scalar of a gradient rather than a gradient: this library composes
    /// one from a colour and an alpha ladder the way [`Self::glow_alpha`] is
    /// composed, so a theme sets how much light there is and the component
    /// decides which edge it falls on. Light themes carry less of it, because
    /// a white highlight on a near-white surface has nowhere to go and reads
    /// as a smudge rather than as a lit edge.
    pub sheen_alpha: f32,
    /// The alpha an area fill starts at under a chart line, fading to nothing
    /// at the baseline.
    pub area_wash_alpha: f32,
    /// How strongly a node's header band takes the node's category colour.
    pub header_tint_alpha: f32,
    /// How wide an identity rail is, in pixels: a node's category stripe, a
    /// callout's edge.
    ///
    /// Distinct from [`Self::selection_rail_width`], which marks *which row
    /// the collection is on* and is therefore transient. This one says what a
    /// thing is, is drawn whether or not anybody is looking at it, and is
    /// wider because it has to read as part of the surface rather than as a
    /// state on top of it.
    pub rail_width: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_theme_parses_and_validates() {
        let ids: Vec<&str> = all().iter().map(|tokens| tokens.meta.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "studio-dark",
                "studio-light",
                "catppuccin-mocha",
                "catppuccin-latte",
                "nord",
                "tokyo-night",
                "gruvbox-dark",
                "dracula",
                "solarized-dark",
                "solarized-light",
            ]
        );
        for tokens in all() {
            tokens
                .validate()
                .unwrap_or_else(|error| panic!("{} is invalid: {error}", tokens.meta.id));
        }
    }

    /// The visual baselines are captured once per bundled theme, so a preset
    /// that leaked into this set would multiply the snapshot catalog.
    #[test]
    fn the_presets_stay_out_of_the_bundled_pair() {
        assert_eq!(bundled().len(), 2);
        assert_eq!(presets().len(), 8);
    }

    #[test]
    fn bundled_document_is_valid_and_typed() {
        let tokens = studio_dark();
        assert_eq!(tokens.meta.id, "studio-dark");
        assert_eq!(tokens.meta.appearance, Appearance::Dark);
        assert_eq!(tokens.spacing(Space::Lg), 16.0);
        assert_eq!(tokens.radius(Radius::Dialog), 16.0);
        assert_eq!(
            tokens.motion_duration(MotionDuration::Menu),
            Duration::from_millis(140)
        );
    }

    #[test]
    fn palette_references_preserve_the_literal_values_they_replaced() {
        let tokens = studio_dark();
        assert_eq!(
            tokens.surface(Surface::Backdrop),
            Color::parse("literal", "#050505").expect("literal")
        );
        assert_eq!(
            tokens.surface(Surface::Canvas),
            Color::parse("literal", "#131313").expect("literal")
        );
        assert_eq!(
            tokens.interactive(InteractiveColor::Hover),
            Color::parse("literal", "#ebebeb14").expect("literal")
        );
        assert_eq!(
            tokens.semantic(SemanticColor::Accent),
            Color::parse("literal", "#7c86ff").expect("literal")
        );
    }

    #[test]
    fn every_bundled_theme_parses_and_declares_its_appearance() {
        let ids: Vec<&str> = bundled().iter().map(|doc| doc.meta.id.as_str()).collect();
        assert_eq!(ids, vec!["studio-dark", "studio-light"]);
        assert_eq!(studio_light().meta.appearance, Appearance::Light);
    }

    #[test]
    fn portable_schema_accepts_every_bundled_theme() {
        let schema: serde_json::Value =
            serde_json::from_str(TOKEN_SCHEMA_JSON).expect("valid JSON schema");
        let validator = jsonschema::validator_for(&schema).expect("valid token schema");
        for json in bundled_json() {
            let document: serde_json::Value = serde_json::from_str(json).expect("theme JSON");
            if let Err(error) = validator.validate(&document) {
                panic!("{} does not match schema: {error}", document["meta"]["id"]);
            }
        }
    }

    #[test]
    fn themes_agree_on_every_metric_that_is_not_a_color() {
        let dark = studio_dark();
        let light = studio_light();
        for size in ControlSize::ALL {
            assert_eq!(dark.control(size).height, light.control(size).height);
            assert_eq!(dark.control(size).font_size, light.control(size).font_size);
        }
        for step in [Space::Xs, Space::Md, Space::Xxl] {
            assert_eq!(dark.spacing(step), light.spacing(step));
        }
        for layer in Layer::ALL {
            assert_eq!(dark.z_index(layer), light.z_index(layer));
        }
        for easing in MotionEasing::ALL {
            assert_eq!(dark.easing(easing), light.easing(easing));
        }
    }

    #[test]
    fn layers_paint_in_a_fixed_order() {
        let tokens = studio_dark();
        assert!(tokens.z_index(Layer::Popover) < tokens.z_index(Layer::Modal));
        assert!(tokens.z_index(Layer::Modal) < tokens.z_index(Layer::Toast));
    }

    #[test]
    fn compact_density_shrinks_and_comfortable_is_the_reference() {
        let tokens = studio_dark();
        let compact = tokens.density(Density::Compact);
        let comfortable = tokens.density(Density::Comfortable);
        assert_eq!(comfortable.space, 1.0);
        assert!(compact.space < comfortable.space);
        assert!(compact.font < comfortable.font);
    }

    #[test]
    fn elevation_grows_with_the_layer_it_serves() {
        let tokens = studio_dark();
        assert!(tokens.elevation(Elevation::Flat).layers.is_empty());
        assert_eq!(tokens.elevation(Elevation::Flat).reach(), 0.0);
        assert!(
            tokens.elevation(Elevation::Modal).reach()
                > tokens.elevation(Elevation::Raised).reach()
        );
    }

    #[test]
    fn a_theme_without_backdrop_is_rejected() {
        let mut value: serde_json::Value =
            serde_json::from_str(studio_dark_json()).expect("bundled JSON");
        value["color"]["surface"]
            .as_object_mut()
            .expect("surface object")
            .remove("backdrop");
        let error = TokenDocument::parse(&value.to_string()).expect_err("missing backdrop");
        assert!(error.to_string().contains("backdrop"));
    }

    #[test]
    fn elevation_reach_that_does_not_increase_is_rejected() {
        let mut value: serde_json::Value =
            serde_json::from_str(studio_dark_json()).expect("bundled JSON");
        value["elevation"]["overlay"] = serde_json::json!([
            { "y": 1, "blur": 2, "spread": 0, "color": "{neutral.0}/66" }
        ]);
        let error = TokenDocument::parse(&value.to_string()).expect_err("unordered reach");
        assert!(error.to_string().contains("elevation"));
        assert!(error.to_string().contains("reach"));
    }

    #[test]
    fn control_steps_are_ordered_and_complete() {
        let tokens = studio_dark();
        let heights: Vec<f32> = ControlSize::ALL
            .iter()
            .map(|size| tokens.control(*size).height)
            .collect();
        assert!(heights.windows(2).all(|window| window[0] < window[1]));
        assert_eq!(tokens.control(ControlSize::Md).padding_x, 12.0);
        assert_eq!(tokens.border_width(BorderWeight::Hairline), 1.0);
        assert!(tokens.opacity(OpacityRole::Disabled) < 1.0);
    }

    #[test]
    fn out_of_order_control_heights_fail_validation() {
        let mut value: serde_json::Value =
            serde_json::from_str(studio_dark_json()).expect("bundled JSON");
        value["control"]["lg"]["height"] = serde_json::json!(10);
        let error = TokenDocument::parse(&value.to_string()).expect_err("unordered heights");
        assert!(error.to_string().contains("control"));
    }

    #[test]
    fn colors_accept_rgb_and_rgba_hex() {
        assert_eq!(Color::parse("opaque", "#ffffff").expect("color").alpha, 1.0);
        let translucent = Color::parse("wash", "#ffffff14").expect("color");
        assert!((translucent.alpha - 20.0 / 255.0).abs() < f32::EPSILON);
    }

    #[test]
    fn invalid_external_documents_fail_loudly() {
        let mut value: serde_json::Value =
            serde_json::from_str(studio_dark_json()).expect("bundled JSON");
        value["color"]["surface"]["canvas"] = serde_json::json!("black");
        let error = TokenDocument::parse(&value.to_string()).expect_err("invalid color");
        assert!(error.to_string().contains("color.surface.canvas"));
    }

    #[test]
    fn external_documents_report_every_contrast_failure() {
        let mut value: serde_json::Value =
            serde_json::from_str(studio_dark_json()).expect("bundled JSON");
        value["color"]["text"]["primary"] = value["color"]["surface"]["canvas"].clone();
        let error = TokenDocument::parse(&value.to_string()).expect_err("invisible primary text");
        let message = error.to_string();
        assert!(message.contains("token contrast is invalid"));
        assert!(
            message
                .contains("color.text.primary on color.surface.canvas is 1.00:1; requires 4.5:1")
        );
        assert!(message.contains("color.text.primary on color.surface.overlay"));
    }

    #[test]
    fn unknown_and_legacy_fields_are_rejected() {
        let mut value: serde_json::Value =
            serde_json::from_str(studio_dark_json()).expect("bundled JSON");
        value["version"] = serde_json::json!(1);
        let error = TokenDocument::parse(&value.to_string()).expect_err("unknown root field");
        assert!(error.to_string().contains("unknown field `version`"));

        let mut value: serde_json::Value =
            serde_json::from_str(studio_dark_json()).expect("bundled JSON");
        value["color"]["surface"]["legacyCanvas"] = serde_json::json!("#000000");
        let error = TokenDocument::parse(&value.to_string()).expect_err("unknown nested field");
        assert!(error.to_string().contains("unknown field `legacyCanvas`"));
    }

    #[test]
    fn a_series_scale_of_the_wrong_length_is_rejected() {
        for length in [SEQUENCE_LENGTH - 1, SEQUENCE_LENGTH + 1] {
            let mut value: serde_json::Value =
                serde_json::from_str(studio_dark_json()).expect("bundled JSON");
            let entries = value["color"]["sequence"]["categorical"]
                .as_array()
                .expect("the scale is a list")
                .clone();
            value["color"]["sequence"]["categorical"] = serde_json::Value::Array(
                entries
                    .iter()
                    .cycle()
                    .take(length)
                    .cloned()
                    .collect::<Vec<_>>(),
            );
            let error =
                TokenDocument::parse(&value.to_string()).expect_err("a mis-sized series scale");
            let message = error.to_string();
            assert!(message.contains("color.sequence.categorical"), "{message}");
            assert!(message.contains(&SEQUENCE_LENGTH.to_string()), "{message}");
            assert!(message.contains(&length.to_string()), "{message}");
        }
    }

    /// A series is consumed by index and a caller with more series than the
    /// scale has colours keeps drawing, so the scale wraps rather than
    /// running out or repeating its last entry forever.
    #[test]
    fn the_series_scale_cycles_rather_than_ending() {
        let tokens = studio_dark();
        let series = tokens.sequence();
        assert_eq!(series.len(), SEQUENCE_LENGTH);
        assert_eq!(tokens.sequence_color(0), series[0]);
        assert_eq!(tokens.sequence_color(SEQUENCE_LENGTH), series[0]);
        assert_eq!(tokens.sequence_color(SEQUENCE_LENGTH + 3), series[3]);
    }

    #[test]
    fn every_shipped_theme_carries_the_whole_canvas_vocabulary() {
        for tokens in all() {
            for role in NodeColor::ALL {
                // Resolving is the assertion: an undeclared reference panics
                // in `resolved`, and a theme that declared the role as
                // nothing at all would be transparent.
                assert!(
                    tokens.node(role).alpha > 0.0,
                    "{} draws nothing for {}",
                    tokens.meta.id,
                    role.path()
                );
            }
        }
    }

    /// Depth is a contact shadow plus a soft key, so each step above flat
    /// casts both. A single layer is the cast that made every raised surface
    /// in the library read as a sticker.
    #[test]
    fn every_step_above_flat_casts_a_contact_shadow_under_its_key() {
        for tokens in all() {
            assert!(tokens.elevation(Elevation::Flat).layers.is_empty());
            for level in [Elevation::Raised, Elevation::Overlay, Elevation::Modal] {
                let step = tokens.elevation(level);
                assert_eq!(step.layers.len(), 2, "{} {level:?}", tokens.meta.id);
                let (ambient, key) = (&step.layers[0], &step.layers[1]);
                assert!(
                    ambient.y < key.y && ambient.blur < key.blur,
                    "{} {level:?} draws its contact shadow no tighter than its key",
                    tokens.meta.id
                );
                assert!(
                    ambient.color.alpha < key.color.alpha,
                    "{} {level:?} draws its contact shadow no quieter than its key",
                    tokens.meta.id
                );
            }
        }
    }

    #[test]
    fn semantic_colors_are_not_layout_surfaces() {
        let tokens = studio_dark();
        assert_ne!(
            tokens.semantic(SemanticColor::Accent),
            tokens.surface(Surface::Canvas)
        );
        assert_ne!(
            tokens.semantic(SemanticColor::Danger),
            tokens.surface(Surface::Raised)
        );
    }
}
