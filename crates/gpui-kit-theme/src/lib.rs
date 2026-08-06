//! Maps GPUI-independent tokens into the paint and typography types views use.

use gpui::{App, BoxShadow, Global, Hsla, Rgba, SharedString, point, px};
use gpui_kit_tokens::{
    BorderWeight, Color, InteractiveColor, MotionDuration, MotionEasing, OpacityRole, studio_dark,
};

pub use gpui_kit_tokens::{
    ControlSize, Radius, SemanticColor, Space, Surface, TextTone, TypeScale,
};

/// Reads the active theme from any context that dereferences to [`App`].
///
/// Components take `&mut App` during render and pull the theme themselves, so
/// callers never thread a `&Theme` through builder arguments.
pub trait ActiveTheme {
    fn theme(&self) -> &Theme;
}

impl ActiveTheme for App {
    fn theme(&self) -> &Theme {
        Theme::get(self)
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub colors: Colors,
    pub typography: Typography,
    pub spacing: Spacing,
    pub radii: Radii,
    pub control: Control,
    pub borders: Borders,
    pub opacity: Opacity,
    pub motion: Motion,
    pub effects: Effects,
}

#[derive(Debug, Clone)]
pub struct Colors {
    pub canvas: Hsla,
    pub panel: Hsla,
    pub raised: Hsla,
    pub overlay: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub text_faint: Hsla,
    pub text_on_accent: Hsla,
    pub hover: Hsla,
    pub active: Hsla,
    pub selected: Hsla,
    pub hairline: Hsla,
    pub hairline_strong: Hsla,
    pub focus: Hsla,
    pub accent: Hsla,
    pub accent_strong: Hsla,
    pub danger: Hsla,
    pub warning: Hsla,
    pub success: Hsla,
    pub info: Hsla,
    pub loader_gradient: [Hsla; 3],
}

#[derive(Debug, Clone)]
pub struct Typography {
    pub sans: SharedString,
    pub sans_fallback: SharedString,
    pub mono: SharedString,
    pub mono_fallback: SharedString,
    pub caption: TypeStyle,
    pub label: TypeStyle,
    pub body: TypeStyle,
    pub title: TypeStyle,
    pub code: TypeStyle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeStyle {
    pub size: f32,
    pub line_height: f32,
    pub weight: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radii {
    pub small: f32,
    pub control: f32,
    pub card: f32,
    pub dialog: f32,
    pub bubble: f32,
    pub pill: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Control {
    pub xs: ControlMetrics,
    pub sm: ControlMetrics,
    pub md: ControlMetrics,
    pub lg: ControlMetrics,
}

impl Control {
    pub fn get(&self, size: ControlSize) -> ControlMetrics {
        match size {
            ControlSize::Xs => self.xs,
            ControlSize::Sm => self.sm,
            ControlSize::Md => self.md,
            ControlSize::Lg => self.lg,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlMetrics {
    pub height: f32,
    pub padding_x: f32,
    pub gap: f32,
    pub font_size: f32,
    pub icon_size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Borders {
    pub hairline: f32,
    pub thick: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opacity {
    pub disabled: f32,
    pub muted: f32,
    pub scrim: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    pub instant_ms: u64,
    pub quick_ms: u64,
    pub menu_ms: u64,
    pub dialog_ms: u64,
    pub resize_ms: u64,
    pub entrance_ms: u64,
    pub pulse_ms: u64,
    pub standard: [f32; 4],
    pub exit: [f32; 4],
    pub settle: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Effects {
    pub glass_alpha: f32,
    pub backdrop_blur: f32,
    pub edge_fade_band: f32,
    pub selected_ring_alpha: f32,
}

impl Theme {
    pub fn studio_dark() -> Self {
        let tokens = studio_dark();
        let style = |step| {
            let step = tokens.type_step(step);
            TypeStyle {
                size: step.size,
                line_height: step.line_height,
                weight: step.weight,
            }
        };
        Self {
            colors: Colors {
                canvas: color(tokens.surface(Surface::Canvas)),
                panel: color(tokens.surface(Surface::Panel)),
                raised: color(tokens.surface(Surface::Raised)),
                overlay: color(tokens.surface(Surface::Overlay)),
                text: color(tokens.text(TextTone::Primary)),
                text_muted: color(tokens.text(TextTone::Muted)),
                text_faint: color(tokens.text(TextTone::Faint)),
                text_on_accent: color(tokens.text(TextTone::OnAccent)),
                hover: color(tokens.interactive(InteractiveColor::Hover)),
                active: color(tokens.interactive(InteractiveColor::Active)),
                selected: color(tokens.interactive(InteractiveColor::Selected)),
                hairline: color(tokens.interactive(InteractiveColor::Hairline)),
                hairline_strong: color(tokens.interactive(InteractiveColor::HairlineStrong)),
                focus: color(tokens.interactive(InteractiveColor::Focus)),
                accent: color(tokens.semantic(SemanticColor::Accent)),
                accent_strong: color(tokens.semantic(SemanticColor::AccentStrong)),
                danger: color(tokens.semantic(SemanticColor::Danger)),
                warning: color(tokens.semantic(SemanticColor::Warning)),
                success: color(tokens.semantic(SemanticColor::Success)),
                info: color(tokens.semantic(SemanticColor::Info)),
                loader_gradient: tokens.loader_gradient().map(color),
            },
            typography: Typography {
                sans: tokens.typography.sans.family.clone().into(),
                sans_fallback: tokens
                    .typography
                    .sans
                    .platform_fallback()
                    .to_string()
                    .into(),
                mono: tokens.typography.mono.family.clone().into(),
                mono_fallback: tokens
                    .typography
                    .mono
                    .platform_fallback()
                    .to_string()
                    .into(),
                caption: style(TypeScale::Caption),
                label: style(TypeScale::Label),
                body: style(TypeScale::Body),
                title: style(TypeScale::Title),
                code: style(TypeScale::Code),
            },
            spacing: Spacing {
                xs: tokens.spacing(Space::Xs),
                sm: tokens.spacing(Space::Sm),
                md: tokens.spacing(Space::Md),
                lg: tokens.spacing(Space::Lg),
                xl: tokens.spacing(Space::Xl),
                xxl: tokens.spacing(Space::Xxl),
            },
            radii: Radii {
                small: tokens.radius(Radius::Small),
                control: tokens.radius(Radius::Control),
                card: tokens.radius(Radius::Card),
                dialog: tokens.radius(Radius::Dialog),
                bubble: tokens.radius(Radius::Bubble),
                pill: tokens.radius(Radius::Pill),
            },
            control: {
                let metrics = |size| {
                    let step = tokens.control(size);
                    ControlMetrics {
                        height: step.height,
                        padding_x: step.padding_x,
                        gap: step.gap,
                        font_size: step.font_size,
                        icon_size: step.icon_size,
                    }
                };
                Control {
                    xs: metrics(ControlSize::Xs),
                    sm: metrics(ControlSize::Sm),
                    md: metrics(ControlSize::Md),
                    lg: metrics(ControlSize::Lg),
                }
            },
            borders: Borders {
                hairline: tokens.border_width(BorderWeight::Hairline),
                thick: tokens.border_width(BorderWeight::Thick),
            },
            opacity: Opacity {
                disabled: tokens.opacity(OpacityRole::Disabled),
                muted: tokens.opacity(OpacityRole::Muted),
                scrim: tokens.opacity(OpacityRole::Scrim),
            },
            motion: Motion {
                instant_ms: millis(tokens, MotionDuration::Instant),
                quick_ms: millis(tokens, MotionDuration::Quick),
                menu_ms: millis(tokens, MotionDuration::Menu),
                dialog_ms: millis(tokens, MotionDuration::Dialog),
                resize_ms: millis(tokens, MotionDuration::Resize),
                entrance_ms: millis(tokens, MotionDuration::Entrance),
                pulse_ms: millis(tokens, MotionDuration::Pulse),
                standard: tokens.easing(MotionEasing::Standard),
                exit: tokens.easing(MotionEasing::Exit),
                settle: tokens.easing(MotionEasing::Settle),
            },
            effects: Effects {
                glass_alpha: if cfg!(target_os = "macos") {
                    tokens.effect.glass_alpha_macos
                } else {
                    1.0
                },
                backdrop_blur: tokens.effect.backdrop_blur,
                edge_fade_band: tokens.effect.edge_fade_band,
                selected_ring_alpha: tokens.effect.selected_ring_alpha,
            },
        }
    }

    pub fn surface(&self, surface: Surface) -> Hsla {
        match surface {
            Surface::Canvas => self.colors.canvas,
            Surface::Panel => self.colors.panel,
            Surface::Raised => self.colors.raised,
            Surface::Overlay => self.colors.overlay,
        }
    }

    pub fn text_color(&self, tone: TextTone) -> Hsla {
        match tone {
            TextTone::Primary => self.colors.text,
            TextTone::Muted => self.colors.text_muted,
            TextTone::Faint => self.colors.text_faint,
            TextTone::OnAccent => self.colors.text_on_accent,
        }
    }

    pub fn semantic_color(&self, color: SemanticColor) -> Hsla {
        match color {
            SemanticColor::Accent => self.colors.accent,
            SemanticColor::AccentStrong => self.colors.accent_strong,
            SemanticColor::Danger => self.colors.danger,
            SemanticColor::Warning => self.colors.warning,
            SemanticColor::Success => self.colors.success,
            SemanticColor::Info => self.colors.info,
        }
    }

    pub fn space(&self, step: Space) -> f32 {
        match step {
            Space::Xs => self.spacing.xs,
            Space::Sm => self.spacing.sm,
            Space::Md => self.spacing.md,
            Space::Lg => self.spacing.lg,
            Space::Xl => self.spacing.xl,
            Space::Xxl => self.spacing.xxl,
        }
    }

    pub fn radius(&self, step: Radius) -> f32 {
        match step {
            Radius::Small => self.radii.small,
            Radius::Control => self.radii.control,
            Radius::Card => self.radii.card,
            Radius::Dialog => self.radii.dialog,
            Radius::Bubble => self.radii.bubble,
            Radius::Pill => self.radii.pill,
        }
    }

    pub fn type_style(&self, scale: TypeScale) -> TypeStyle {
        match scale {
            TypeScale::Caption => self.typography.caption,
            TypeScale::Label => self.typography.label,
            TypeScale::Body => self.typography.body,
            TypeScale::Title => self.typography.title,
            TypeScale::Code => self.typography.code,
        }
    }

    pub fn install(cx: &mut App) {
        cx.set_global(Self::studio_dark());
    }

    pub fn get(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn selected_ring(&self) -> Vec<BoxShadow> {
        vec![BoxShadow {
            color: self.colors.text.opacity(self.effects.selected_ring_alpha),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
            inset: true,
        }]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::studio_dark()
    }
}

impl Global for Theme {}

fn millis(tokens: &gpui_kit_tokens::TokenDocument, step: MotionDuration) -> u64 {
    tokens.motion_duration(step).as_millis() as u64
}

fn color(value: Color) -> Hsla {
    Hsla::from(Rgba {
        r: value.red,
        g: value.green,
        b: value.blue,
        a: value.alpha,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn studio_theme_preserves_surface_hierarchy() {
        let theme = Theme::studio_dark();
        assert!(theme.colors.canvas.l < theme.colors.panel.l);
        assert!(theme.colors.panel.l < theme.colors.raised.l);
        assert!(theme.colors.text_faint.l < theme.colors.text_muted.l);
        assert!(theme.colors.text_muted.l < theme.colors.text.l);
    }

    #[test]
    fn repeated_semantic_metrics_are_token_backed() {
        let theme = Theme::studio_dark();
        assert_eq!(theme.spacing.lg, 16.0);
        assert_eq!(theme.radii.card, 12.0);
        assert_eq!(theme.radii.dialog, 16.0);
        assert_eq!(theme.motion.menu_ms, 140);
    }

    #[test]
    fn control_metrics_grow_with_size() {
        let theme = Theme::studio_dark();
        let heights: Vec<f32> = ControlSize::ALL
            .iter()
            .map(|size| theme.control.get(*size).height)
            .collect();
        assert!(heights.windows(2).all(|window| window[0] < window[1]));
        assert_eq!(theme.borders.hairline, 1.0);
        assert!(theme.opacity.disabled < 1.0);
    }

    #[test]
    fn selected_ring_does_not_change_layout() {
        let theme = Theme::studio_dark();
        let ring = theme.selected_ring();
        assert_eq!(ring.len(), 1);
        assert!(ring[0].inset);
        assert_eq!(ring[0].spread_radius, px(1.0));
    }
}
