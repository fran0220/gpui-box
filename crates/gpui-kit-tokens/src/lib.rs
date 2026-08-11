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
            Color::resolve(path, value, &self.color.palette)?;
        }
        for (path, level) in self.elevation.entries() {
            Color::resolve(&format!("{path}.color"), &level.color, &self.color.palette)?;
            if level.blur < 0.0 {
                return invalid(path, "blur must not be negative");
            }
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

        for (path, value) in [
            ("effect.edgeFadeBand", self.effect.edge_fade_band),
            ("effect.glowBlur", self.effect.glow_blur),
            ("effect.glassBlur", self.effect.glass_blur),
        ] {
            if value < 0.0 {
                return invalid(path, "must not be negative");
            }
        }

        for (path, value) in [
            ("effect.selectedRingAlpha", self.effect.selected_ring_alpha),
            ("effect.focusRingAlpha", self.effect.focus_ring_alpha),
            ("effect.glowAlpha", self.effect.glow_alpha),
            ("effect.glassAlpha", self.effect.glass_alpha),
            ("opacity.disabled", self.opacity.disabled),
            ("opacity.muted", self.opacity.muted),
            ("opacity.scrim", self.opacity.scrim),
        ] {
            if !(0.0..=1.0).contains(&value) {
                return invalid(path, "must be between 0 and 1");
            }
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
        Ok(())
    }

    pub fn surface(&self, role: Surface) -> Color {
        let (path, value) = match role {
            Surface::Canvas => ("color.surface.canvas", self.color.surface.canvas.as_str()),
            Surface::Sunken => ("color.surface.sunken", self.color.surface.sunken.as_str()),
            Surface::Panel => ("color.surface.panel", self.color.surface.panel.as_str()),
            Surface::Raised => ("color.surface.raised", self.color.surface.raised.as_str()),
            Surface::Overlay => ("color.surface.overlay", self.color.surface.overlay.as_str()),
        };
        self.resolved(path, value)
    }

    pub fn text(&self, role: TextTone) -> Color {
        let (path, value) = match role {
            TextTone::Primary => ("color.text.primary", self.color.text.primary.as_str()),
            TextTone::Muted => ("color.text.muted", self.color.text.muted.as_str()),
            TextTone::Faint => ("color.text.faint", self.color.text.faint.as_str()),
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

    pub fn loader_gradient(&self) -> [Color; 3] {
        [
            self.resolved("color.loader.gradient.0", &self.color.loader.gradient[0]),
            self.resolved("color.loader.gradient.1", &self.color.loader.gradient[1]),
            self.resolved("color.loader.gradient.2", &self.color.loader.gradient[2]),
        ]
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
        let (path, step) = match level {
            Elevation::Flat => ("elevation.flat", &self.elevation.flat),
            Elevation::Raised => ("elevation.raised", &self.elevation.raised),
            Elevation::Overlay => ("elevation.overlay", &self.elevation.overlay),
            Elevation::Modal => ("elevation.modal", &self.elevation.modal),
        };
        ResolvedElevation {
            y: step.y,
            blur: step.blur,
            spread: step.spread,
            color: self.resolved(&format!("{path}.color"), &step.color),
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

/// Every theme shipped with the library, in registration order.
pub fn bundled() -> [&'static TokenDocument; 2] {
    [studio_dark(), studio_light()]
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
    OnAccent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveColor {
    Hover,
    Active,
    Selected,
    Hairline,
    HairlineStrong,
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
pub struct ResolvedElevation {
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
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
    pub flat: ElevationStep,
    pub raised: ElevationStep,
    pub overlay: ElevationStep,
    pub modal: ElevationStep,
}

impl ElevationTokens {
    fn entries(&self) -> [(&'static str, &ElevationStep); 4] {
        [
            ("elevation.flat", &self.flat),
            ("elevation.raised", &self.raised),
            ("elevation.overlay", &self.overlay),
            ("elevation.modal", &self.modal),
        ]
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ElevationStep {
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: String,
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
    pub loader: LoaderColors,
}

impl ColorTokens {
    fn entries(&self) -> [(&'static str, &str); 24] {
        [
            ("color.surface.canvas", &self.surface.canvas),
            ("color.surface.sunken", &self.surface.sunken),
            ("color.surface.panel", &self.surface.panel),
            ("color.surface.raised", &self.surface.raised),
            ("color.surface.overlay", &self.surface.overlay),
            ("color.text.primary", &self.text.primary),
            ("color.text.muted", &self.text.muted),
            ("color.text.faint", &self.text.faint),
            ("color.text.onAccent", &self.text.on_accent),
            ("color.interactive.hover", &self.interactive.hover),
            ("color.interactive.active", &self.interactive.active),
            ("color.interactive.selected", &self.interactive.selected),
            ("color.interactive.hairline", &self.interactive.hairline),
            (
                "color.interactive.hairlineStrong",
                &self.interactive.hairline_strong,
            ),
            ("color.interactive.focus", &self.interactive.focus),
            ("color.semantic.accent", &self.semantic.accent),
            ("color.semantic.accentStrong", &self.semantic.accent_strong),
            ("color.semantic.danger", &self.semantic.danger),
            ("color.semantic.warning", &self.semantic.warning),
            ("color.semantic.success", &self.semantic.success),
            ("color.semantic.info", &self.semantic.info),
            ("color.loader.gradient.0", &self.loader.gradient[0]),
            ("color.loader.gradient.1", &self.loader.gradient[1]),
            ("color.loader.gradient.2", &self.loader.gradient[2]),
        ]
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceColors {
    pub canvas: String,
    /// The well an editable value sits in, recessed below the surface that
    /// carries it. It is what a text field is made of once the field has no
    /// border to be made of instead.
    pub sunken: String,
    pub panel: String,
    pub raised: String,
    pub overlay: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextColors {
    pub primary: String,
    pub muted: String,
    pub faint: String,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LoaderColors {
    pub gradient: [String; 3],
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
    /// How wide the ring around the focused control is drawn, in pixels.
    pub focus_ring_width: f32,
    pub focus_ring_alpha: f32,
    /// How strongly a surface that carries a state colour bleeds it into the
    /// pixels around its edge.
    pub glow_alpha: f32,
    /// How far that bleed reaches, in pixels.
    pub glow_blur: f32,
    /// How opaque a frosted surface's own fill is over what it blurs. A theme
    /// that sets this to 1 declares itself opaque, and a frosted surface then
    /// paints no blur at all rather than blurring pixels nobody can see.
    pub glass_alpha: f32,
    /// How far a frosted surface blurs what is behind it, in pixels.
    pub glass_blur: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

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
            tokens.surface(Surface::Canvas),
            Color::parse("literal", "#0a0a0a").expect("literal")
        );
        assert_eq!(
            tokens.interactive(InteractiveColor::Hover),
            Color::parse("literal", "#ebebeb24").expect("literal")
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
        assert_eq!(tokens.elevation(Elevation::Flat).blur, 0.0);
        assert_eq!(tokens.elevation(Elevation::Flat).color.alpha, 0.0);
        assert!(tokens.elevation(Elevation::Modal).blur > tokens.elevation(Elevation::Raised).blur);
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
        assert!(message.contains("text.primary on surface.canvas is 1.00:1; requires 4.5:1"));
        assert!(message.contains("text.primary on surface.overlay"));
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
