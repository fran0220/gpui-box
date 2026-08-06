//! Typed design tokens with no dependency on GPUI or a windowing system.
//!
//! The bundled token document is embedded in the crate, so consumers never
//! depend on a monorepo-relative path. Applications select semantic roles
//! instead of copying colors and metrics into view code.

use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

const STUDIO_DARK_JSON: &str = include_str!("../../../tokens/studio-dark.json");

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("token JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("token `{path}` is invalid: {message}")]
    Invalid { path: String, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl Color {
    pub fn parse(path: &str, value: &str) -> Result<Self, TokenError> {
        let digits = value.strip_prefix('#').ok_or_else(|| TokenError::Invalid {
            path: path.into(),
            message: "expected #RRGGBB or #RRGGBBAA".into(),
        })?;
        if digits.len() != 6 && digits.len() != 8 {
            return Err(TokenError::Invalid {
                path: path.into(),
                message: "expected six or eight hexadecimal digits".into(),
            });
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
}

fn invalid_color(path: &str) -> TokenError {
    TokenError::Invalid {
        path: path.into(),
        message: "contains a non-hexadecimal color channel".into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Light,
    Dark,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenDocument {
    pub meta: Metadata,
    pub color: ColorTokens,
    pub space: SpacingTokens,
    pub radius: RadiusTokens,
    pub control: ControlTokens,
    pub border: BorderTokens,
    pub opacity: OpacityTokens,
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
        if self.meta.id.trim().is_empty() {
            return invalid("meta.id", "must not be empty");
        }
        if self.meta.name.trim().is_empty() {
            return invalid("meta.name", "must not be empty");
        }

        for (path, value) in self.color.entries() {
            Color::parse(path, value)?;
        }

        let spacing = [
            self.space.xs,
            self.space.sm,
            self.space.md,
            self.space.lg,
            self.space.xl,
            self.space.xxl,
        ];
        if spacing.windows(2).any(|window| window[0] >= window[1]) {
            return invalid("space", "steps must be strictly increasing");
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
            if step.height < step.font_size {
                return invalid(path, "height must not be smaller than fontSize");
            }
        }

        if self.border.hairline <= 0.0 || self.border.thick <= self.border.hairline {
            return invalid("border", "thick must exceed a positive hairline");
        }

        for (path, value) in [
            ("effect.glassAlphaMacos", self.effect.glass_alpha_macos),
            ("effect.selectedRingAlpha", self.effect.selected_ring_alpha),
            ("opacity.disabled", self.opacity.disabled),
            ("opacity.muted", self.opacity.muted),
            ("opacity.scrim", self.opacity.scrim),
        ] {
            if !(0.0..=1.0).contains(&value) {
                return invalid(path, "must be between 0 and 1");
            }
        }
        Ok(())
    }

    pub fn surface(&self, role: Surface) -> Color {
        let (path, value) = match role {
            Surface::Canvas => ("color.surface.canvas", self.color.surface.canvas.as_str()),
            Surface::Panel => ("color.surface.panel", self.color.surface.panel.as_str()),
            Surface::Raised => ("color.surface.raised", self.color.surface.raised.as_str()),
            Surface::Overlay => ("color.surface.overlay", self.color.surface.overlay.as_str()),
        };
        embedded_color(path, value)
    }

    pub fn text(&self, role: TextTone) -> Color {
        let (path, value) = match role {
            TextTone::Primary => ("color.text.primary", self.color.text.primary.as_str()),
            TextTone::Muted => ("color.text.muted", self.color.text.muted.as_str()),
            TextTone::Faint => ("color.text.faint", self.color.text.faint.as_str()),
            TextTone::OnAccent => ("color.text.onAccent", self.color.text.on_accent.as_str()),
        };
        embedded_color(path, value)
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
        embedded_color(path, value)
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
        embedded_color(path, value)
    }

    pub fn loader_gradient(&self) -> [Color; 3] {
        [
            embedded_color("color.loader.gradient.0", &self.color.loader.gradient[0]),
            embedded_color("color.loader.gradient.1", &self.color.loader.gradient[1]),
            embedded_color("color.loader.gradient.2", &self.color.loader.gradient[2]),
        ]
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
            MotionDuration::Pulse => self.motion.duration_ms.pulse,
        })
    }

    pub fn easing(&self, step: MotionEasing) -> [f32; 4] {
        match step {
            MotionEasing::Standard => self.motion.easing.standard,
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

fn embedded_color(path: &str, value: &str) -> Color {
    Color::parse(path, value).expect("the embedded token document is validated before release")
}

pub fn studio_dark() -> &'static TokenDocument {
    static TOKENS: OnceLock<TokenDocument> = OnceLock::new();
    TOKENS.get_or_init(|| {
        TokenDocument::parse(STUDIO_DARK_JSON)
            .expect("tokens/studio-dark.json must pass TokenDocument::validate")
    })
}

pub fn studio_dark_json() -> &'static str {
    STUDIO_DARK_JSON
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Canvas,
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
    Pulse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionEasing {
    Standard,
    Exit,
    Settle,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Metadata {
    pub id: String,
    pub name: String,
    pub appearance: Appearance,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColorTokens {
    pub surface: SurfaceColors,
    pub text: TextColors,
    pub interactive: InteractiveColors,
    pub semantic: SemanticColors,
    pub loader: LoaderColors,
}

impl ColorTokens {
    fn entries(&self) -> [(&'static str, &str); 23] {
        [
            ("color.surface.canvas", &self.surface.canvas),
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
pub struct SurfaceColors {
    pub canvas: String,
    pub panel: String,
    pub raised: String,
    pub overlay: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextColors {
    pub primary: String,
    pub muted: String,
    pub faint: String,
    pub on_accent: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveColors {
    pub hover: String,
    pub active: String,
    pub selected: String,
    pub hairline: String,
    pub hairline_strong: String,
    pub focus: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticColors {
    pub accent: String,
    pub accent_strong: String,
    pub danger: String,
    pub warning: String,
    pub success: String,
    pub info: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoaderColors {
    pub gradient: [String; 3],
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpacingTokens {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RadiusTokens {
    pub small: f32,
    pub control: f32,
    pub card: f32,
    pub dialog: f32,
    pub bubble: f32,
    pub pill: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
#[serde(rename_all = "camelCase")]
pub struct ControlStep {
    pub height: f32,
    pub padding_x: f32,
    pub gap: f32,
    pub font_size: f32,
    pub icon_size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct BorderTokens {
    pub hairline: f32,
    pub thick: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct OpacityTokens {
    pub disabled: f32,
    pub muted: f32,
    pub scrim: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TypographyTokens {
    pub sans: FontTokens,
    pub mono: FontTokens,
    pub scale: TypeScaleTokens,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
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
pub struct TypeScaleTokens {
    pub caption: TypeStep,
    pub label: TypeStep,
    pub body: TypeStep,
    pub title: TypeStep,
    pub code: TypeStep,
}

impl TypeScaleTokens {
    fn entries(&self) -> [(&'static str, &TypeStep); 5] {
        [
            ("typography.scale.caption", &self.caption),
            ("typography.scale.label", &self.label),
            ("typography.scale.body", &self.body),
            ("typography.scale.title", &self.title),
            ("typography.scale.code", &self.code),
        ]
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeStep {
    pub size: f32,
    pub line_height: f32,
    pub weight: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionTokens {
    pub duration_ms: DurationTokens,
    pub easing: EasingTokens,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DurationTokens {
    pub instant: u64,
    pub quick: u64,
    pub menu: u64,
    pub dialog: u64,
    pub resize: u64,
    pub entrance: u64,
    pub pulse: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EasingTokens {
    pub standard: [f32; 4],
    pub exit: [f32; 4],
    pub settle: [f32; 4],
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectTokens {
    pub glass_alpha_macos: f32,
    pub backdrop_blur: f32,
    pub edge_fade_band: f32,
    pub selected_ring_alpha: f32,
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
