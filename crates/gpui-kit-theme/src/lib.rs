//! Maps GPUI-independent tokens into the paint and typography types views use.

use std::sync::Arc;

use gpui::{App, BorrowAppContext, BoxShadow, Global, Hsla, Rgba, SharedString, point, px};
use gpui_kit_tokens::{
    BorderWeight, Color, DensityScale, InteractiveColor, OpacityRole, TokenDocument, TokenError,
    bundled,
};

pub use gpui_kit_tokens::{
    Appearance, ControlSize, Density, Elevation, Layer, MotionDuration, MotionEasing, Radius,
    SemanticColor, Space, SpringPreset, SpringTokens, Surface, TextTone, TypeScale,
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
    pub id: SharedString,
    pub name: SharedString,
    pub appearance: Appearance,
    pub density: Density,
    pub colors: Colors,
    pub typography: Typography,
    pub spacing: Spacing,
    pub radii: Radii,
    pub control: Control,
    pub borders: Borders,
    pub opacity: Opacity,
    pub motion: Motion,
    pub elevation: Elevations,
    pub z_index: ZIndices,
    pub effects: Effects,
}

#[derive(Debug, Clone)]
pub struct Colors {
    pub canvas: Hsla,
    pub sunken: Hsla,
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
    pub strong: TypeStyle,
    pub subtitle: TypeStyle,
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
    pub spin_ms: u64,
    pub slow_ms: u64,
    /// The gap between one member of a staggered group and the next.
    pub stagger_step_ms: u64,
    pub pulse_ms: u64,
    pub shimmer_ms: u64,
    pub toast_ms: u64,
    pub linear: [f32; 4],
    pub standard: [f32; 4],
    pub ease_in: [f32; 4],
    pub ease_out: [f32; 4],
    pub ease_in_out: [f32; 4],
    pub emphasized: [f32; 4],
    pub overshoot: [f32; 4],
    pub exit: [f32; 4],
    pub settle: [f32; 4],
    pub snappy: SpringTokens,
    pub smooth: SpringTokens,
    pub bouncy: SpringTokens,
    pub grab: SpringTokens,
    /// How far a pressed control sinks, in pixels.
    pub press_offset: f32,
    /// How far a hovered control rises, in pixels.
    pub hover_lift: f32,
    /// The speed past which a released gesture is a flick, in pixels a second.
    pub flick_velocity: f32,
    /// How much of an overscroll is shown at the boundary.
    pub rubber_band_tension: f32,
}

/// Shadows for each elevation step. Flat is intentionally empty rather than a
/// transparent shadow, so a flat surface allocates no shadow work at all.
#[derive(Debug, Clone, PartialEq)]
pub struct Elevations {
    pub flat: Vec<BoxShadow>,
    pub raised: Vec<BoxShadow>,
    pub overlay: Vec<BoxShadow>,
    pub modal: Vec<BoxShadow>,
}

impl Elevations {
    pub fn get(&self, level: Elevation) -> &[BoxShadow] {
        match level {
            Elevation::Flat => &self.flat,
            Elevation::Raised => &self.raised,
            Elevation::Overlay => &self.overlay,
            Elevation::Modal => &self.modal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZIndices {
    pub content: i32,
    pub sticky: i32,
    pub dock: i32,
    pub popover: i32,
    pub tooltip: i32,
    pub modal: i32,
    pub toast: i32,
}

impl ZIndices {
    pub fn get(&self, layer: Layer) -> i32 {
        match layer {
            Layer::Content => self.content,
            Layer::Sticky => self.sticky,
            Layer::Dock => self.dock,
            Layer::Popover => self.popover,
            Layer::Tooltip => self.tooltip,
            Layer::Modal => self.modal,
            Layer::Toast => self.toast,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Effects {
    pub edge_fade_band: f32,
    pub selected_ring_alpha: f32,
    pub focus_ring_width: f32,
    pub focus_ring_alpha: f32,
    pub glow_alpha: f32,
    pub glow_blur: f32,
    pub glass_alpha: f32,
    pub glass_blur: f32,
    pub glass_liquid_alpha: f32,
    pub glass_bevel: f32,
    pub glass_refraction: f32,
    pub glass_dispersion: f32,
    pub glass_specular: f32,
    pub glass_specular_sharpness: f32,
    pub glass_light_angle: f32,
    pub glass_merge_distance: f32,
    pub glass_contrast_flip_low: f32,
    pub glass_contrast_flip_high: f32,
    pub glass_press_depth: f32,
}

impl Theme {
    pub fn studio_dark() -> Self {
        Self::from_tokens(gpui_kit_tokens::studio_dark(), Density::default())
    }

    pub fn studio_light() -> Self {
        Self::from_tokens(gpui_kit_tokens::studio_light(), Density::default())
    }

    /// Builds a theme from any validated token document at one density.
    ///
    /// Density scales spacing, control geometry and type independently, and
    /// rounds to whole pixels so compact layouts stay on the pixel grid.
    pub fn from_tokens(tokens: &TokenDocument, density: Density) -> Self {
        let scale = tokens.density(density);
        let style = |step| {
            let step = tokens.type_step(step);
            TypeStyle {
                size: scale_font(step.size, scale),
                line_height: scale_font(step.line_height, scale),
                weight: step.weight,
            }
        };
        Self {
            id: tokens.meta.id.clone().into(),
            name: tokens.meta.name.clone().into(),
            appearance: tokens.meta.appearance,
            density,
            colors: Colors {
                canvas: color(tokens.surface(Surface::Canvas)),
                sunken: color(tokens.surface(Surface::Sunken)),
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
                strong: style(TypeScale::Strong),
                subtitle: style(TypeScale::Subtitle),
                title: style(TypeScale::Title),
                code: style(TypeScale::Code),
            },
            spacing: Spacing {
                xs: scale_space(tokens.spacing(Space::Xs), scale),
                sm: scale_space(tokens.spacing(Space::Sm), scale),
                md: scale_space(tokens.spacing(Space::Md), scale),
                lg: scale_space(tokens.spacing(Space::Lg), scale),
                xl: scale_space(tokens.spacing(Space::Xl), scale),
                xxl: scale_space(tokens.spacing(Space::Xxl), scale),
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
                        height: scale_control(step.height, scale),
                        padding_x: scale_control(step.padding_x, scale),
                        gap: scale_control(step.gap, scale),
                        font_size: scale_font(step.font_size, scale),
                        icon_size: scale_control(step.icon_size, scale),
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
                spin_ms: millis(tokens, MotionDuration::Spin),
                slow_ms: millis(tokens, MotionDuration::Slow),
                stagger_step_ms: millis(tokens, MotionDuration::StaggerStep),
                pulse_ms: millis(tokens, MotionDuration::Pulse),
                shimmer_ms: millis(tokens, MotionDuration::Shimmer),
                toast_ms: millis(tokens, MotionDuration::Toast),
                linear: tokens.easing(MotionEasing::Linear),
                standard: tokens.easing(MotionEasing::Standard),
                ease_in: tokens.easing(MotionEasing::EaseIn),
                ease_out: tokens.easing(MotionEasing::EaseOut),
                ease_in_out: tokens.easing(MotionEasing::EaseInOut),
                emphasized: tokens.easing(MotionEasing::Emphasized),
                overshoot: tokens.easing(MotionEasing::Overshoot),
                exit: tokens.easing(MotionEasing::Exit),
                settle: tokens.easing(MotionEasing::Settle),
                snappy: tokens.spring(SpringPreset::Snappy),
                smooth: tokens.spring(SpringPreset::Smooth),
                bouncy: tokens.spring(SpringPreset::Bouncy),
                grab: tokens.spring(SpringPreset::Grab),
                press_offset: tokens.press_offset(),
                hover_lift: tokens.hover_lift(),
                flick_velocity: tokens.flick_velocity(),
                rubber_band_tension: tokens.rubber_band_tension(),
            },
            elevation: Elevations {
                flat: shadow(tokens, Elevation::Flat),
                raised: shadow(tokens, Elevation::Raised),
                overlay: shadow(tokens, Elevation::Overlay),
                modal: shadow(tokens, Elevation::Modal),
            },
            z_index: ZIndices {
                content: tokens.z_index(Layer::Content),
                sticky: tokens.z_index(Layer::Sticky),
                dock: tokens.z_index(Layer::Dock),
                popover: tokens.z_index(Layer::Popover),
                tooltip: tokens.z_index(Layer::Tooltip),
                modal: tokens.z_index(Layer::Modal),
                toast: tokens.z_index(Layer::Toast),
            },
            effects: Effects {
                edge_fade_band: tokens.effect.edge_fade_band,
                selected_ring_alpha: tokens.effect.selected_ring_alpha,
                focus_ring_width: tokens.effect.focus_ring_width,
                focus_ring_alpha: tokens.effect.focus_ring_alpha,
                glow_alpha: tokens.effect.glow_alpha,
                glow_blur: tokens.effect.glow_blur,
                glass_alpha: tokens.effect.glass_alpha,
                glass_blur: tokens.effect.glass_blur,
                glass_liquid_alpha: tokens.effect.glass_liquid_alpha,
                glass_bevel: tokens.effect.glass_bevel,
                glass_refraction: tokens.effect.glass_refraction,
                glass_dispersion: tokens.effect.glass_dispersion,
                glass_specular: tokens.effect.glass_specular,
                glass_specular_sharpness: tokens.effect.glass_specular_sharpness,
                glass_light_angle: tokens.effect.glass_light_angle,
                glass_merge_distance: tokens.effect.glass_merge_distance,
                glass_contrast_flip_low: tokens.effect.glass_contrast_flip_low,
                glass_contrast_flip_high: tokens.effect.glass_contrast_flip_high,
                glass_press_depth: tokens.effect.glass_press_depth,
            },
        }
    }

    pub fn surface(&self, surface: Surface) -> Hsla {
        match surface {
            Surface::Canvas => self.colors.canvas,
            Surface::Sunken => self.colors.sunken,
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
            TypeScale::Strong => self.typography.strong,
            TypeScale::Subtitle => self.typography.subtitle,
            TypeScale::Title => self.typography.title,
            TypeScale::Code => self.typography.code,
        }
    }

    pub fn easing(&self, easing: MotionEasing) -> [f32; 4] {
        match easing {
            MotionEasing::Linear => self.motion.linear,
            MotionEasing::Standard => self.motion.standard,
            MotionEasing::EaseIn => self.motion.ease_in,
            MotionEasing::EaseOut => self.motion.ease_out,
            MotionEasing::EaseInOut => self.motion.ease_in_out,
            MotionEasing::Emphasized => self.motion.emphasized,
            MotionEasing::Overshoot => self.motion.overshoot,
            MotionEasing::Exit => self.motion.exit,
            MotionEasing::Settle => self.motion.settle,
        }
    }

    pub fn spring(&self, preset: SpringPreset) -> SpringTokens {
        match preset {
            SpringPreset::Snappy => self.motion.snappy,
            SpringPreset::Smooth => self.motion.smooth,
            SpringPreset::Bouncy => self.motion.bouncy,
            SpringPreset::Grab => self.motion.grab,
        }
    }

    pub fn shadow(&self, level: Elevation) -> &[BoxShadow] {
        self.elevation.get(level)
    }

    pub fn layer(&self, layer: Layer) -> i32 {
        self.z_index.get(layer)
    }

    /// Installs the bundled theme registry. Idempotent.
    pub fn install(cx: &mut App) {
        if !cx.has_global::<ThemeRegistry>() {
            cx.set_global(ThemeRegistry::new());
        }
    }

    pub fn get(cx: &App) -> &Self {
        cx.global::<ThemeRegistry>().active()
    }

    /// The ring drawn around whichever control currently has the keyboard.
    ///
    /// It spreads outward in the focus colour, so it reads differently from
    /// [`Self::selected_ring`]: focus says where the next keystroke goes,
    /// selection says which answer is current, and a reader that cannot tell
    /// the two apart cannot tell what pressing a key would do.
    pub fn focus_ring(&self) -> Vec<BoxShadow> {
        vec![BoxShadow {
            color: self.colors.focus.opacity(self.effects.focus_ring_alpha),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(self.effects.focus_ring_width),
            inset: false,
        }]
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

    /// The colour a surface in a named state bleeds into the pixels around it.
    ///
    /// It is the state itself made visible at the edge, which is what lets a
    /// surface report "running" or "failed" without a border drawn round it.
    /// Blurred and unoffset, so nothing about it reads as a line.
    pub fn glow(&self, color: Hsla) -> Vec<BoxShadow> {
        vec![BoxShadow {
            color: color.opacity(self.effects.glow_alpha),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(self.effects.glow_blur),
            spread_radius: px(0.0),
            inset: false,
        }]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::studio_dark()
    }
}

/// The set of themes an application can switch between at runtime.
#[derive(Debug)]
pub struct ThemeRegistry {
    tokens: Vec<Arc<TokenDocument>>,
    active: usize,
    density: Density,
    theme: Theme,
}

impl Global for ThemeRegistry {}

impl ThemeRegistry {
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn new() -> Self {
        let tokens: Vec<Arc<TokenDocument>> = bundled()
            .into_iter()
            .map(|document| Arc::new(document.clone()))
            .collect();
        let theme = Theme::from_tokens(&tokens[0], Density::default());
        Self {
            tokens,
            active: 0,
            density: Density::default(),
            theme,
        }
    }

    /// Adds or replaces a theme. A registered id replaces the earlier document
    /// so an application can override a bundled theme without shadowing it.
    pub fn register(&mut self, tokens: TokenDocument) {
        let id = tokens.meta.id.clone();
        match self.tokens.iter().position(|other| other.meta.id == id) {
            Some(index) => self.tokens[index] = Arc::new(tokens),
            None => self.tokens.push(Arc::new(tokens)),
        }
        self.rebuild();
    }

    pub fn register_json(&mut self, json: &str) -> Result<(), TokenError> {
        self.register(TokenDocument::parse(json)?);
        Ok(())
    }

    pub fn ids(&self) -> Vec<SharedString> {
        self.tokens
            .iter()
            .map(|tokens| SharedString::from(tokens.meta.id.clone()))
            .collect()
    }

    pub fn active(&self) -> &Theme {
        &self.theme
    }

    pub fn density(&self) -> Density {
        self.density
    }

    /// Returns false when the id is not registered, leaving the active theme
    /// untouched rather than falling back to a default the caller did not ask
    /// for.
    pub fn activate(&mut self, id: &str) -> bool {
        let Some(index) = self.tokens.iter().position(|tokens| tokens.meta.id == id) else {
            return false;
        };
        self.active = index;
        self.rebuild();
        true
    }

    pub fn set_density(&mut self, density: Density) {
        self.density = density;
        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.theme = Theme::from_tokens(&self.tokens[self.active], self.density);
    }
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Switches the active theme and repaints every window.
pub fn activate_theme(id: &str, cx: &mut App) -> bool {
    let switched = cx.update_global::<ThemeRegistry, bool>(|registry, _| registry.activate(id));
    if switched {
        cx.refresh_windows();
    }
    switched
}

/// Changes the density axis and repaints every window.
pub fn set_density(density: Density, cx: &mut App) {
    cx.update_global::<ThemeRegistry, ()>(|registry, _| registry.set_density(density));
    cx.refresh_windows();
}

fn scale_space(value: f32, scale: DensityScale) -> f32 {
    (value * scale.space).round().max(1.0)
}

fn scale_control(value: f32, scale: DensityScale) -> f32 {
    (value * scale.control).round().max(1.0)
}

fn scale_font(value: f32, scale: DensityScale) -> f32 {
    ((value * scale.font) * 2.0).round() / 2.0
}

fn shadow(tokens: &TokenDocument, level: Elevation) -> Vec<BoxShadow> {
    let step = tokens.elevation(level);
    if step.color.alpha == 0.0 {
        return Vec::new();
    }
    vec![BoxShadow {
        color: color(step.color),
        offset: point(px(0.0), px(step.y)),
        blur_radius: px(step.blur),
        spread_radius: px(step.spread),
        inset: false,
    }]
}

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

    /// The library groups content with colour rather than with a line drawn
    /// round it, so a step between two surfaces has to be visible on its own.
    /// Below this the step stops reading and a border is doing the work
    /// instead, which is the arrangement this threshold exists to prevent
    /// anyone drifting back into.
    const MIN_SURFACE_STEP: f32 = 0.02;

    #[test]
    fn a_surface_step_is_visible_without_a_border_to_help_it() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            let steps = [
                ("sunken to panel", theme.colors.sunken, theme.colors.panel),
                ("canvas to panel", theme.colors.canvas, theme.colors.panel),
            ];
            for (name, lower, upper) in steps {
                assert!(
                    (upper.l - lower.l).abs() >= MIN_SURFACE_STEP,
                    "{} in {}: {} is too close to {} to group anything",
                    name,
                    theme.id,
                    lower.l,
                    upper.l
                );
            }
        }
    }

    #[test]
    fn every_theme_separates_surfaces_and_text_emphasis() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            assert!(theme.colors.sunken.l < theme.colors.panel.l);
            assert!(theme.colors.canvas.l < theme.colors.panel.l);
            assert!(theme.colors.panel.l <= theme.colors.raised.l);

            // Emphasis is distance from the canvas, which inverts with
            // appearance, so compare magnitudes rather than raw lightness.
            let emphasis = |tone: Hsla| (tone.l - theme.colors.canvas.l).abs();
            assert!(emphasis(theme.colors.text) > emphasis(theme.colors.text_muted));
            assert!(emphasis(theme.colors.text_muted) > emphasis(theme.colors.text_faint));
        }
    }

    #[test]
    fn compact_density_shrinks_geometry_without_touching_color() {
        let comfortable = Theme::from_tokens(gpui_kit_tokens::studio_dark(), Density::Comfortable);
        let compact = Theme::from_tokens(gpui_kit_tokens::studio_dark(), Density::Compact);
        assert!(compact.spacing.lg < comfortable.spacing.lg);
        assert!(
            compact.control.get(ControlSize::Md).height
                < comfortable.control.get(ControlSize::Md).height
        );
        assert!(compact.typography.body.size < comfortable.typography.body.size);
        assert_eq!(compact.colors.accent, comfortable.colors.accent);
        assert_eq!(compact.radii.card, comfortable.radii.card);
    }

    #[test]
    fn density_keeps_geometry_on_the_pixel_grid() {
        let compact = Theme::from_tokens(gpui_kit_tokens::studio_dark(), Density::Compact);
        for size in ControlSize::ALL {
            let metrics = compact.control.get(size);
            assert_eq!(metrics.height.fract(), 0.0);
            assert_eq!(metrics.padding_x.fract(), 0.0);
        }
        assert_eq!(compact.spacing.md.fract(), 0.0);
    }

    #[test]
    fn the_registry_switches_themes_and_refuses_unknown_ids() {
        let mut registry = ThemeRegistry::new();
        assert_eq!(registry.active().id, "studio-dark");
        assert!(registry.activate("studio-light"));
        assert_eq!(registry.active().appearance, Appearance::Light);
        assert!(!registry.activate("studio-solarized"));
        assert_eq!(registry.active().id, "studio-light");
    }

    #[test]
    fn a_registered_theme_replaces_the_bundled_one_with_the_same_id() {
        let mut registry = ThemeRegistry::new();
        let before = registry.ids().len();
        registry
            .register_json(gpui_kit_tokens::studio_dark_json())
            .expect("bundled json is valid");
        assert_eq!(registry.ids().len(), before);
    }

    #[test]
    fn invalid_contrast_is_reported_before_the_registry_changes() {
        let mut registry = ThemeRegistry::new();
        let before = registry.ids();
        let mut value: serde_json::Value =
            serde_json::from_str(gpui_kit_tokens::studio_dark_json()).expect("bundled JSON");
        value["meta"]["id"] = serde_json::json!("low-contrast");
        value["color"]["text"]["primary"] = value["color"]["surface"]["canvas"].clone();

        let error = registry
            .register_json(&value.to_string())
            .expect_err("invisible text must not register");
        assert!(error.to_string().contains("text.primary on surface.canvas"));
        assert_eq!(registry.ids(), before);
    }

    #[test]
    fn density_survives_a_theme_switch() {
        let mut registry = ThemeRegistry::new();
        registry.set_density(Density::Compact);
        registry.activate("studio-light");
        assert_eq!(registry.active().density, Density::Compact);
    }

    #[test]
    fn flat_elevation_costs_nothing_and_deeper_layers_cast_more() {
        let theme = Theme::studio_dark();
        assert!(theme.shadow(Elevation::Flat).is_empty());
        assert!(
            theme.shadow(Elevation::Modal)[0].blur_radius
                > theme.shadow(Elevation::Raised)[0].blur_radius
        );
        assert!(theme.layer(Layer::Toast) > theme.layer(Layer::Popover));
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
    fn focus_and_selection_do_not_look_alike() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            let focus = theme.focus_ring();
            let selected = theme.selected_ring();
            assert_eq!(focus.len(), 1);
            assert_ne!(focus[0].color, selected[0].color);
            assert!(!focus[0].inset && selected[0].inset);
            // A ring that reserved space would move the layout the moment the
            // keyboard arrived on a control.
            assert_eq!(focus[0].offset, point(px(0.0), px(0.0)));
            assert!(focus[0].spread_radius > px(0.0));
        }
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
