//! A surface that shows what is behind it, out of focus and bent.
//!
//! [`Glass`] is the material a popover, a dialog or a rail is placed on when
//! the window itself is translucent. Regular Liquid refracts the scattered
//! (blurred) source at its rim: colour bands and luminance bend there, not
//! recognizable background details. Clear and Lens stay sharp because their
//! default blur is zero; Frosted scatters without bending the backdrop.
//!
//! # One layer, in one order
//!
//! The whole subtree paints inside a single scene layer, which is the reason
//! `BackdropLayer` is an element and not a styled `div`. Paint order is
//! per-primitive otherwise, so a repaint elsewhere in the frame can reorder
//! the surface's own quads underneath the blur — a divider or a border is then
//! snapshotted and blurred away, intermittently, in a way no test reproduces.
//! Inside one layer the relationship is structural: surface first, fill and
//! content after.
//!
//! Regular Liquid owns its blur, saturation and achromatic wash in the
//! material, not in a source-over fill. Clear is reserved for media, with a
//! dimming layer behind it. Adaptive appearance is a small-control policy;
//! large reading surfaces retain the window appearance.
//!
//! `docs/coverage.md` records which renderer does which of these today.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AnyElement, App, BackdropStatistics, Bounds, Corners, Element, GlassEdge, GlassLobe,
    GlassMaterial, GlobalElementId, Hsla, InspectorElementId, InteractiveElement as _, IntoElement,
    LayoutId, LuminanceProbeLease, MAX_GLASS_LOBES, MouseButton, ParentElement, Pixels, RenderOnce,
    Rgba, RoundedClip, StatefulInteractiveElement as _, Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, Appearance, Elevation, Radius, Space, Surface, Theme, ThemeRegistry,
};

use crate::foundation::{Ident, ThemeOverlay};
use crate::layout::measure;
use crate::motion::{self, MotionPolicy, MotionRole, keyed};

/// Which appearance a glass surface is currently painting.
///
/// `Inherited` is the window theme. `Light` and `Dark` are the counterpart
/// the surface resolved from its backdrop luminance when
/// [`Glass::adaptive_appearance`] is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlassAppearance {
    /// The window theme is in force.
    #[default]
    Inherited,
    /// The surface installed the light counterpart.
    Light,
    /// The surface installed the dark counterpart.
    Dark,
}

/// How a glass surface responds to light.
///
/// The presets are named for what they are made of rather than for where they
/// are used, because the same material carries a popover on one screen and a
/// rail on another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlassPreset {
    /// Blurred and tinted, and nothing else. This is what [`super::Frost`]
    /// paints, and what every renderer that can blur at all can produce.
    Frosted,
    /// Regular Liquid Glass: blurred, saturation-adjusted backdrop with an
    /// achromatic wash, edge lensing and a highlighted rim.
    #[default]
    Liquid,
    /// Apple Clear variant, only above media, always with light content.
    /// Pair with `dimmed(true)`. Reduced transparency uses a dark Frosted body.
    Clear,
    /// The bend without the colour split or the highlight, for a surface that
    /// sits over text that the dispersion would otherwise fringe.
    Lens,
}

impl GlassPreset {
    /// Clear carries its on-media reading policy independently of the host
    /// appearance. The same scoped roles reach text and icons in its subtree.
    fn content_theme(self, theme: &Theme) -> Theme {
        if self != Self::Clear {
            return theme.clone();
        }
        theme.clone().modify(|theme| {
            theme.appearance = Appearance::Dark;
            let colors = &mut theme.colors;
            colors.text = colors.on_media_foreground;
            colors.text_muted = colors.on_media_foreground;
            colors.text_faint = colors.on_media_foreground;
            colors.text_placeholder = colors.on_media_foreground;
            colors.hairline = colors.on_media_hairline;
            colors.hairline_strong = colors.on_media_hairline;
            colors.backdrop = colors.on_media_background;
            colors.canvas = colors.on_media_background;
            colors.sunken = colors.on_media_background;
            colors.panel = colors.on_media_background;
            colors.raised = colors.on_media_background;
            colors.overlay = colors.on_media_background;
        })
    }

    fn resolved(self, theme: &Theme) -> Self {
        if theme.reduce_transparency {
            Self::Frosted
        } else {
            self
        }
    }

    /// How much ordinary source-over fill the surface paints, at this theme.
    ///
    /// A frosted surface uses `effect.glassAlpha` to separate its scattered
    /// backdrop from surrounding content. Liquid and Lens use shader-owned
    /// optics instead; their wash belongs to the material, not this fill.
    pub fn tint_alpha(self, theme: &Theme) -> f32 {
        match self.resolved(theme) {
            GlassPreset::Frosted => theme.effects.glass_alpha,
            GlassPreset::Liquid | GlassPreset::Lens | GlassPreset::Clear => 0.0,
        }
    }

    /// The material this preset asks the renderer for, at this theme.
    ///
    /// Every value comes from a token: a preset names a combination, it does
    /// not carry numbers of its own.
    pub fn material(self, theme: &Theme) -> GlassMaterial<Pixels> {
        let effects = &theme.effects;
        let wash_channel = if theme.appearance == Appearance::Dark {
            0.0
        } else {
            1.0
        };
        match self.resolved(theme) {
            GlassPreset::Frosted => GlassMaterial::frosted(px(effects.glass_frost_blur)),
            GlassPreset::Liquid => GlassMaterial {
                blur_radius: px(effects.glass_frost_blur),
                saturation: effects.glass_saturation,
                wash: Rgba {
                    r: wash_channel,
                    g: wash_channel,
                    b: wash_channel,
                    a: effects.glass_wash,
                },
                refraction: effects.glass_refraction,
                thickness: px(effects.glass_thickness),
                refractive_index: effects.glass_refractive_index,
                backdrop_depth: px(effects.glass_backdrop_depth),
                dispersion: effects.glass_dispersion,
                specular: effects.glass_specular,
                transmission_gain: effects.glass_transmission_gain,
                optical_lift: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: effects.glass_optical_lift,
                },
                hairline: px(effects.glass_hairline),
                light_angle: effects.glass_light_angle,
                specular_sharpness: effects.glass_specular_sharpness,
                ..GlassMaterial::clear()
            },
            GlassPreset::Lens => GlassMaterial {
                refraction: effects.glass_refraction,
                thickness: px(effects.glass_thickness),
                refractive_index: effects.glass_refractive_index,
                backdrop_depth: px(effects.glass_backdrop_depth),
                ..GlassMaterial::clear()
            },
            GlassPreset::Clear => GlassMaterial {
                refraction: effects.glass_refraction,
                thickness: px(effects.glass_thickness),
                refractive_index: effects.glass_refractive_index,
                backdrop_depth: px(effects.glass_backdrop_depth),
                dispersion: effects.glass_dispersion,
                specular: effects.glass_specular,
                transmission_gain: effects.glass_transmission_gain,
                optical_lift: Rgba {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: effects.glass_optical_lift,
                },
                hairline: px(effects.glass_hairline),
                light_angle: effects.glass_light_angle,
                specular_sharpness: effects.glass_specular_sharpness,
                ..GlassMaterial::clear()
            },
        }
    }

    /// The responsive optical profile this preset resolves once its bounds are
    /// known. Frosted is flat; optical presets scale with their own
    /// control rather than borrowing one fixed pixel bevel.
    fn bevel(self, theme: &Theme) -> Option<ResponsiveBevel> {
        (self.resolved(theme) != GlassPreset::Frosted).then_some(ResponsiveBevel {
            ratio: theme.effects.glass_bevel_ratio,
            min: px(theme.effects.glass_bevel_min),
            max: px(theme.effects.glass_bevel_max),
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct ResponsiveBevel {
    ratio: f32,
    min: Pixels,
    max: Pixels,
}

impl ResponsiveBevel {
    /// Resolve from the short edge of one control. A fused body uses the
    /// smallest constituent short edge, never the potentially very wide union.
    fn resolve(self, bounds: Bounds<Pixels>, lobes: &[GlassLobe<Pixels>]) -> Pixels {
        let short_edge = if lobes.is_empty() {
            f32::from(bounds.size.width).min(f32::from(bounds.size.height))
        } else {
            lobes.iter().fold(f32::INFINITY, |shortest, lobe| {
                shortest
                    .min(f32::from(lobe.bounds.size.width).min(f32::from(lobe.bounds.size.height)))
            })
        };
        if !short_edge.is_finite() || short_edge <= 0.0 {
            return px(0.0);
        }

        let upper = f32::from(self.max).max(0.0).min(short_edge * 0.5);
        let lower = f32::from(self.min).max(0.0).min(upper);
        px((short_edge * self.ratio.max(0.0)).clamp(lower, upper))
    }
}

/// The transient visual state an interactive glass surface keeps across
/// frames: whether it is pressed, which side of the contrast band it last
/// settled on, and its probe slot.
#[derive(Default)]
struct GlassState {
    pressed: bool,
    appearance_flipped: bool,
    lease: LuminanceProbeLease,
    statistics: Option<BackdropStatistics>,
}

/// A glass surface: optionally scattered and bent backdrop, optional fill,
/// and caller-owned content.
#[derive(IntoElement)]
pub struct Glass {
    ident: Ident,
    surface: Surface,
    elevation: Elevation,
    radius: Radius,
    radius_px: Option<f32>,
    blur: Option<f32>,
    preset: GlassPreset,
    thickness: Option<f32>,
    refractive_index: Option<f32>,
    backdrop_depth: Option<f32>,
    refraction: Option<f32>,
    dispersion: Option<f32>,
    specular: Option<f32>,
    light_angle: Option<f32>,
    track_pointer: bool,
    pressable: bool,
    adaptive: bool,
    adaptive_appearance: bool,
    dimmed: bool,
    focused: bool,
    protect_text: bool,
    tint: Option<Hsla>,
    edge_mask: Option<(GlassEdge, f32)>,
    children: Vec<AnyElement>,
    frame: Option<gpui::Stateful<gpui::Div>>,
}

impl std::fmt::Debug for Glass {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Glass")
            .field("ident", &self.ident)
            .field("surface", &self.surface)
            .field("radius", &self.radius)
            .field("radius_px", &self.radius_px)
            .field("blur", &self.blur)
            .field("preset", &self.preset)
            .field("thickness", &self.thickness)
            .field("refractive_index", &self.refractive_index)
            .field("backdrop_depth", &self.backdrop_depth)
            .field("refraction", &self.refraction)
            .field("dispersion", &self.dispersion)
            .field("specular", &self.specular)
            .field("light_angle", &self.light_angle)
            .field("track_pointer", &self.track_pointer)
            .field("pressable", &self.pressable)
            .field("adaptive", &self.adaptive)
            .field("adaptive_appearance", &self.adaptive_appearance)
            .field("dimmed", &self.dimmed)
            .field("focused", &self.focused)
            .field("protect_text", &self.protect_text)
            .field("tint", &self.tint)
            .field("edge_mask", &self.edge_mask)
            .field("has_child", &!self.children.is_empty())
            .finish()
    }
}

impl Glass {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            surface: Surface::Overlay,
            elevation: Elevation::Raised,
            radius: Radius::Card,
            radius_px: None,
            blur: None,
            preset: GlassPreset::default(),
            thickness: None,
            refractive_index: None,
            backdrop_depth: None,
            refraction: None,
            dispersion: None,
            specular: None,
            light_angle: None,
            track_pointer: false,
            pressable: false,
            adaptive: false,
            adaptive_appearance: false,
            dimmed: false,
            focused: false,
            protect_text: true,
            tint: None,
            edge_mask: None,
            children: Vec::new(),
            frame: None,
        }
    }

    /// Which surface colour Frosted lays over the backdrop.
    pub fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    /// Report caller-owned focus with `interactive.focus` inside this surface's
    /// fitted rounded edge, at `effect.focusRingWidth`. Replaces the optical
    /// hairline; does not install a focus handler or add an external halo.
    /// Use this instead of applying `Theme::focus_ring_on` to glass.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Token shadow appropriate to this surface's elevation. The shadow is
    /// clipped outside the glass, never captured as a dark interior fill.
    pub fn elevation(mut self, elevation: Elevation) -> Self {
        self.elevation = elevation;
        self
    }

    /// The rounding of the glass. It clips the blur as well as the fill, so a
    /// caller rounding the card inside must say the same thing here or the
    /// blur will show past the corners.
    pub fn radius(mut self, radius: Radius) -> Self {
        self.radius = radius;
        self
    }

    /// The rounding of the glass in pixels, overriding the role.
    ///
    /// A canvas draws its cards at a zoom the theme knows nothing about, so
    /// the card inside is rounded at a scaled radius while the role resolves
    /// to its unscaled one. Since the glass clips the blur as well as the
    /// fill, resolving the role again here would show the backdrop past the
    /// card's corners at every zoom but one. A caller that already scaled the
    /// radius hands over the number it used rather than the role it came from.
    pub fn radius_px(mut self, radius: f32) -> Self {
        self.radius_px = Some(radius);
        self
    }

    /// How far the backdrop is blurred, in pixels, overriding the preset.
    /// Clear and Lens default to zero; Regular Liquid includes scattering.
    pub fn blur(mut self, blur: f32) -> Self {
        self.blur = Some(blur.max(0.0));
        self
    }

    /// Which combination of optics the surface asks for.
    pub fn preset(mut self, preset: GlassPreset) -> Self {
        self.preset = preset;
        self
    }

    /// Profile height in pixels before the [`Self::refraction`] multiplier,
    /// overriding `effect.glassThickness`. Zero follows the bounded bevel.
    /// Use `refraction(1.0)` to make this the effective maximum thickness.
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = Some(thickness.max(0.0));
        self
    }

    /// Refractive index, overriding `effect.glassRefractiveIndex`, clamped to
    /// 1..=2.5. One does not bend light.
    pub fn refractive_index(mut self, refractive_index: f32) -> Self {
        self.refractive_index = Some(refractive_index.clamp(1.0, 2.5));
        self
    }

    /// Effective distance in pixels through the refracted medium to the 2D
    /// source plane, overriding `effect.glassBackdropDepth`. Clamped
    /// nonnegative; this is not a physical air gap behind the glass.
    pub fn backdrop_depth(mut self, backdrop_depth: f32) -> Self {
        self.backdrop_depth = Some(backdrop_depth.max(0.0));
        self
    }

    /// Signed thickness multiplier controlling optical height and normal,
    /// overriding `effect.glassRefraction`.
    pub fn refraction(mut self, refraction: f32) -> Self {
        self.refraction = Some(refraction);
        self
    }

    /// How far the edge splits the backdrop into colour, overriding
    /// `effect.glassDispersion`.
    pub fn dispersion(mut self, dispersion: f32) -> Self {
        self.dispersion = Some(dispersion);
        self
    }

    /// How bright the rim highlight is, overriding `effect.glassSpecular`.
    pub fn specular(mut self, specular: f32) -> Self {
        self.specular = Some(specular);
        self
    }

    /// Where the light is, in radians clockwise from straight up, overriding
    /// `effect.glassLightAngle`.
    pub fn light_angle(mut self, radians: f32) -> Self {
        self.light_angle = Some(radians);
        self
    }

    /// Move the rim highlight to the pointer's side of the surface while the
    /// pointer is over it, as if the pointer carried the light. The bounds the
    /// angle is computed against are the ones measured last frame, which is
    /// the same one-frame settling every measured control accepts.
    pub fn track_pointer(mut self, track_pointer: bool) -> Self {
        self.track_pointer = track_pointer;
        self
    }

    /// Deepen the refraction while the surface is pressed, by
    /// `effect.glassPressDepth`, and scale foreground by `effect.glassPressScale`
    /// without changing layout or the surface outline, springing back on release.
    /// Reduced motion suppresses foreground scaling. This is a purely
    /// visual response: the surface publishes no action and installs no
    /// handler beyond the press tracking itself.
    pub fn pressable(mut self, pressable: bool) -> Self {
        self.pressable = pressable;
        self
    }

    /// Let small controls flip their material and content appearance from
    /// backdrop probes. Large surfaces never flip. Before a probe resolves,
    /// the window theme supplies the direction; no extra fill is painted.
    pub fn adaptive(mut self, adaptive: bool) -> Self {
        self.adaptive = adaptive;
        self
    }

    /// Flip this surface, and the subtree it holds, to the counterpart
    /// appearance when the backdrop luminance opposes the current theme.
    ///
    /// Uses the same probe and hysteresis as [`Self::adaptive`]. A product
    /// that registered only one appearance never flips.
    pub fn adaptive_appearance(mut self, adaptive: bool) -> Self {
        self.adaptive_appearance = adaptive;
        self
    }

    /// Dim the media behind Clear glass by `effect.glassDimming`.
    /// Attenuates transmission before optical lift and highlights, independent
    /// of foreground primitive batching.
    /// Other presets ignore this setting.
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    /// Preserve the default worst-neutral-backdrop 4.5:1 body-text protection
    /// for untinted Regular glass. Disable only when the caller owns foreground
    /// legibility: the material then retains its token wash rather than
    /// darkening or whitening the entire surface for the theme's body text.
    /// Other presets and reduced transparency retain their own policies.
    pub fn protect_text_contrast(mut self, protect: bool) -> Self {
        self.protect_text = protect;
        self
    }

    /// Colour the optical material, before its rim and highlights. Alpha
    /// controls tint strength; zero preserves the preset, one replaces its
    /// transmitted colour. Frosted and opaque fallbacks use this as their fill.
    /// The caller owns foreground contrast when supplying a tint.
    pub fn tint(mut self, tint: impl Into<Hsla>) -> Self {
        self.tint = Some(tint.into());
        self
    }

    /// Fade the optics from `edge` over `band` pixels, for a scroll-edge
    /// ramp. The inner side of the band is the content itself.
    pub fn edge_mask(mut self, edge: GlassEdge, band: f32) -> Self {
        self.edge_mask = Some((edge, band.max(0.0)));
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children = vec![child.into_any_element()];
        self
    }

    /// Keep the overlay's existing percentage-sizing and focus boundary.
    pub(crate) fn frame(
        mut self,
        frame: gpui::Stateful<gpui::Div>,
        children: Vec<AnyElement>,
    ) -> Self {
        self.frame = Some(frame);
        self.children = children;
        self
    }

    /// The material this surface asks the renderer for: the preset's
    /// combination with the caller's overrides laid over it.
    fn material(&self, theme: &Theme) -> GlassMaterial<Pixels> {
        if theme.reduce_transparency {
            return GlassPreset::Frosted.material(theme);
        }
        let mut material = self.preset.material(theme);
        if let Some(blur) = self.blur {
            material.blur_radius = px(blur);
        }
        if let Some(thickness) = self.thickness {
            material.thickness = px(thickness);
        }
        if let Some(refractive_index) = self.refractive_index {
            material.refractive_index = refractive_index;
        }
        if let Some(backdrop_depth) = self.backdrop_depth {
            material.backdrop_depth = px(backdrop_depth);
        }
        if let Some(refraction) = self.refraction {
            material.refraction = refraction;
        }
        if let Some(dispersion) = self.dispersion {
            material.dispersion = dispersion;
        }
        if let Some(specular) = self.specular {
            material.specular = specular;
        }
        if let Some(light_angle) = self.light_angle {
            material.light_angle = light_angle;
        }
        if let Some((edge, band)) = self.edge_mask {
            material.edge_mask_edge = edge.as_f32();
            material.edge_mask_band = px(band);
        }
        if self.focused {
            material.hairline = Pixels::ZERO;
        }
        material
    }
}

impl RenderOnce for Glass {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut theme = self.preset.content_theme(cx.theme());
        let radius = self.radius_px.unwrap_or_else(|| theme.radius(self.radius));
        let alpha = self.preset.tint_alpha(&theme).clamp(0.0, 1.0);
        let mut material = self.material(&theme);
        let bevel = self.preset.bevel(&theme);

        let id = self.ident.semantic_id();
        let interactive = self.track_pointer
            || self.pressable
            || self.adaptive
            || self.adaptive_appearance
            || self.preset == GlassPreset::Liquid;
        let state = interactive
            .then(|| keyed::slot::<GlassState>(&id, window.window_handle().window_id(), cx));
        let measured = measure::cell(&id, window, cx);
        let bounds = measured.get();
        let adaptive = (self.adaptive || self.adaptive_appearance)
            && self.preset != GlassPreset::Clear
            && !theme.reduce_transparency
            && can_flip(bounds, theme.effects.glass_flip_max_extent);

        // The pointer carries the light: the angle from the surface's centre
        // to the pointer, clockwise from straight up, which is the convention
        // the material states its light in. Off the surface, or dead centre,
        // the theme's light stays where it was.
        if self.track_pointer
            && let Some(angle) = pointer_light_angle(bounds, window.mouse_position())
        {
            material.light_angle = angle;
        }

        // One spring drives material depth and foreground scale. Layout and
        // the outer surface stay fixed; descendant input/IME/semantics follow
        // the framework's displayed transform, not a second local mapping.
        let mut press_depth = 1.0;
        let mut press_scale = 1.0;
        if self.pressable {
            let pressed = state.as_ref().is_some_and(|state| state.borrow().pressed);
            (press_depth, press_scale) = glass_press(&id, pressed, &theme, window, cx);
            material.refraction *= press_depth;
        }

        let mut statistics = None;
        if (adaptive || self.preset == GlassPreset::Liquid)
            && let Some(state) = &state
        {
            let mut state = state.borrow_mut();
            if let Some(slot) = state.lease.id() {
                material.probe = slot;
                state.statistics = window.backdrop_statistics(slot).or(state.statistics);
                statistics = state.statistics;
                if adaptive && let Some(statistics) = statistics {
                    state.appearance_flipped = deepen_tint(
                        state.appearance_flipped,
                        statistics.mean_luminance,
                        theme.appearance == Appearance::Dark,
                        theme.effects.glass_contrast_flip_low,
                        theme.effects.glass_contrast_flip_high,
                    );
                }
            }
        }

        let mut overlay_theme = (self.preset == GlassPreset::Clear).then(|| theme.clone());
        if adaptive
            && state
                .as_ref()
                .is_some_and(|state| state.borrow().appearance_flipped)
            && let Some(counterpart) = cx
                .try_global::<ThemeRegistry>()
                .and_then(|registry| registry.counterpart_for(&theme))
        {
            theme = counterpart;
            let probe = material.probe;
            material = self.material(&theme);
            material.probe = probe;
            material.refraction *= press_depth;
            if self.track_pointer
                && let Some(angle) = pointer_light_angle(bounds, window.mouse_position())
            {
                material.light_angle = angle;
            }
            overlay_theme = Some(theme.clone());
        }

        if self.protect_text && self.preset.resolved(&theme) == GlassPreset::Liquid {
            protect_text_contrast(&mut material, &theme);
        }
        tint_material(&mut material, self.preset.resolved(&theme), self.tint);
        let tone = self.tint.unwrap_or_else(|| theme.surface(self.surface));
        let fill = tone.opacity(alpha);
        let fallback = Some(tone.opacity(1.0));
        let translucent = alpha < 1.0;
        let focus_report = self
            .focused
            .then_some((theme.colors.focus, px(theme.effects.focus_ring_width)));

        let mut surface = self
            .frame
            .unwrap_or_else(|| {
                div().semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::Region))
            })
            .rounded(px(radius))
            .text_color(theme.colors.text)
            .shadow(glass_shadows(&theme, self.elevation, statistics))
            .children(self.children.into_iter().map(|child| GlassContents {
                radius: px(radius),
                scale: press_scale,
                bounds: Rc::clone(&measured),
                child,
            }));
        if alpha > 0.0 {
            surface = surface.bg(fill);
        }

        if interactive {
            // `semantic_in` already made the surface stateful under its
            // semantic id, which is the identity the listeners hang off.
            let mut stateful = surface;
            if self.track_pointer {
                // The highlight follows the pointer, so every move over the
                // surface is a frame the surface has to paint.
                stateful = stateful.on_mouse_move(|_, window, _| window.refresh());
            }
            if self.pressable
                && let Some(state) = &state
            {
                let press = Rc::clone(state);
                stateful = stateful.on_mouse_down(MouseButton::Left, move |_, window, _| {
                    press.borrow_mut().pressed = true;
                    window.refresh();
                });
                let release = Rc::clone(state);
                stateful = stateful.on_mouse_up(MouseButton::Left, move |_, window, _| {
                    release.borrow_mut().pressed = false;
                    window.refresh();
                });
            }
            // One hover listener carries both concerns: the highlight resets
            // and a press that left the surface lets go.
            if self.track_pointer || self.pressable {
                let leave = state.clone();
                stateful = stateful.on_hover(move |hovered, window, _| {
                    if !*hovered && let Some(state) = &leave {
                        state.borrow_mut().pressed = false;
                    }
                    window.refresh();
                });
            }
            return finish_glass(
                overlay_theme,
                BackdropLayer {
                    radius: px(radius),
                    dimming: (self.dimmed && self.preset == GlassPreset::Clear)
                        .then_some(theme.effects.glass_dimming),
                    material,
                    bevel,
                    lobes: LobeSource::Surface,
                    translucent,
                    fallback,
                    focus_report,
                    measured: Some(measured),
                    child: stateful.into_any_element(),
                },
            );
        }

        finish_glass(
            overlay_theme,
            BackdropLayer {
                radius: px(radius),
                dimming: (self.dimmed && self.preset == GlassPreset::Clear)
                    .then_some(theme.effects.glass_dimming),
                material,
                bevel,
                lobes: LobeSource::Surface,
                translucent,
                fallback,
                focus_report,
                measured: Some(measured),
                child: surface.into_any_element(),
            },
        )
    }
}

fn finish_glass(overlay_theme: Option<Theme>, layer: BackdropLayer) -> AnyElement {
    match overlay_theme {
        Some(theme) => ThemeOverlay::theme(theme, layer).into_any_element(),
        None => layer.into_any_element(),
    }
}

/// Several glass panes fused into one body.
///
/// Each pane is a rounded rect lobe of a single glass surface; where two
/// panes come within `effect.glassMergeDistance` of each other, the shape's
/// smooth minimum joins them into one outline, the way two drops of water
/// meet. The optics — bevel, refraction, dispersion, the highlight — follow
/// the fused outline rather than each pane's own.
///
/// At most [`MAX_GLASS_LOBES`] panes fuse. A larger group paints opaque
/// overlay panes instead, preserving every pane's content without holes.
#[derive(IntoElement)]
pub struct GlassGroup {
    ident: Ident,
    surface: Surface,
    radius: Radius,
    blur: Option<f32>,
    preset: GlassPreset,
    thickness: Option<f32>,
    refractive_index: Option<f32>,
    backdrop_depth: Option<f32>,
    merge: Option<f32>,
    gap: Option<f32>,
    pressable: bool,
    adaptive: bool,
    adaptive_appearance: bool,
    dimmed: bool,
    protect_text: bool,
    tint: Option<Hsla>,
    panes: Vec<(Ident, AnyElement)>,
}

impl std::fmt::Debug for GlassGroup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GlassGroup")
            .field("ident", &self.ident)
            .field("surface", &self.surface)
            .field("radius", &self.radius)
            .field("blur", &self.blur)
            .field("preset", &self.preset)
            .field("thickness", &self.thickness)
            .field("refractive_index", &self.refractive_index)
            .field("backdrop_depth", &self.backdrop_depth)
            .field("merge", &self.merge)
            .field("gap", &self.gap)
            .field("pressable", &self.pressable)
            .field("adaptive", &self.adaptive)
            .field("adaptive_appearance", &self.adaptive_appearance)
            .field("dimmed", &self.dimmed)
            .field("protect_text", &self.protect_text)
            .field("tint", &self.tint)
            .field("panes", &self.panes.len())
            .finish()
    }
}

impl GlassGroup {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            surface: Surface::Overlay,
            radius: Radius::Card,
            blur: None,
            preset: GlassPreset::default(),
            thickness: None,
            refractive_index: None,
            backdrop_depth: None,
            merge: None,
            gap: None,
            pressable: false,
            adaptive: false,
            adaptive_appearance: false,
            dimmed: false,
            protect_text: true,
            tint: None,
            panes: Vec::new(),
        }
    }

    /// Which surface colour each Frosted pane lays over the backdrop.
    pub fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    /// The rounding of each pane's lobe and fill.
    pub fn radius(mut self, radius: Radius) -> Self {
        self.radius = radius;
        self
    }

    /// How far the backdrop is blurred, overriding the preset.
    pub fn blur(mut self, blur: f32) -> Self {
        self.blur = Some(blur.max(0.0));
        self
    }

    /// Which combination of optics the fused surface asks for.
    pub fn preset(mut self, preset: GlassPreset) -> Self {
        self.preset = preset;
        self
    }

    /// Profile height in pixels before the theme's `effect.glassRefraction`
    /// multiplier, overriding `effect.glassThickness`. Zero follows the bevel.
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = Some(thickness.max(0.0));
        self
    }

    /// Refractive index, overriding `effect.glassRefractiveIndex`, clamped to
    /// 1..=2.5. One does not bend light.
    pub fn refractive_index(mut self, refractive_index: f32) -> Self {
        self.refractive_index = Some(refractive_index.clamp(1.0, 2.5));
        self
    }

    /// Effective distance in pixels through the refracted medium to the 2D
    /// source plane, overriding `effect.glassBackdropDepth`. Clamped
    /// nonnegative; this is not a physical air gap behind the glass.
    pub fn backdrop_depth(mut self, backdrop_depth: f32) -> Self {
        self.backdrop_depth = Some(backdrop_depth.max(0.0));
        self
    }

    /// How far apart two panes may sit and still join, in pixels, overriding
    /// `effect.glassMergeDistance`.
    pub fn merge(mut self, merge: f32) -> Self {
        self.merge = Some(merge.max(0.0));
        self
    }

    /// The space between panes, in pixels. The default is the theme's small
    /// step, which sits inside the default merge distance so adjacent panes
    /// join out of the box.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap.max(0.0));
        self
    }

    /// Deepen the fused refraction while any pane is pressed, by
    /// `effect.glassPressDepth`. The response is optical only: the group
    /// publishes no action of its own.
    pub fn pressable(mut self, pressable: bool) -> Self {
        self.pressable = pressable;
        self
    }

    /// Let a small fused body flip appearance using [`Glass::adaptive`]'s
    /// probe, area limit, and hysteresis. Never adds a source-over wash.
    pub fn adaptive(mut self, adaptive: bool) -> Self {
        self.adaptive = adaptive;
        self
    }

    /// Flip this fused body, and the subtree it holds, to the counterpart
    /// appearance when the backdrop luminance opposes the current theme.
    pub fn adaptive_appearance(mut self, adaptive: bool) -> Self {
        self.adaptive_appearance = adaptive;
        self
    }

    /// Dim the media behind Clear glass by `effect.glassDimming`.
    /// Other presets ignore this setting; Clear retains it under reduced transparency.
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    /// Apply [`Glass::protect_text_contrast`]'s body-text policy to the joined
    /// material. Enabled by default; disabling makes legibility caller-owned.
    pub fn protect_text_contrast(mut self, protect: bool) -> Self {
        self.protect_text = protect;
        self
    }

    /// Colour the joined optical material once, including its bridge, using
    /// [`Glass::tint`]'s alpha and foreground-contrast contract.
    pub fn tint(mut self, tint: impl Into<Hsla>) -> Self {
        self.tint = Some(tint.into());
        self
    }

    /// One pane of the body: a lobe of the shape, a fill, and the caller's
    /// content.
    pub fn pane(mut self, ident: impl Into<Ident>, child: impl IntoElement) -> Self {
        self.panes.push((ident.into(), child.into_any_element()));
        self
    }

    /// Resolve the same caller overrides initially and after an appearance flip.
    fn material(&self, theme: &Theme) -> GlassMaterial<Pixels> {
        let mut material = self.preset.material(theme);
        if !theme.reduce_transparency {
            if let Some(blur) = self.blur {
                material.blur_radius = px(blur);
            }
            if let Some(thickness) = self.thickness {
                material.thickness = px(thickness);
            }
            if let Some(refractive_index) = self.refractive_index {
                material.refractive_index = refractive_index;
            }
            if let Some(backdrop_depth) = self.backdrop_depth {
                material.backdrop_depth = px(backdrop_depth);
            }
        }
        material.smoothing = px(self.merge.unwrap_or(theme.effects.glass_merge_distance));
        material
    }
}

impl RenderOnce for GlassGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let over_budget = self.panes.len() > MAX_GLASS_LOBES;
        let mut theme = self.preset.content_theme(cx.theme());
        let radius = theme.radius(self.radius);
        let alpha = if over_budget {
            1.0
        } else {
            self.preset.tint_alpha(&theme).clamp(0.0, 1.0)
        };
        let mut material = self.material(&theme);
        let bevel = self.preset.bevel(&theme);

        let id = self.ident.semantic_id();
        let interactive = self.pressable || self.adaptive || self.adaptive_appearance;
        let state = interactive
            .then(|| keyed::slot::<GlassState>(&id, window.window_handle().window_id(), cx));
        let measured = measure::cell(&id, window, cx);
        let adaptive = (self.adaptive || self.adaptive_appearance)
            && self.preset != GlassPreset::Clear
            && !theme.reduce_transparency
            && can_flip(measured.get(), theme.effects.glass_flip_max_extent);

        // A press deepens the fused outline, not each pane: the group is one
        // body, so the finger answers against the joined refraction.
        let mut press_depth = 1.0;
        let mut press_scale = 1.0;
        if self.pressable {
            let pressed = state.as_ref().is_some_and(|state| state.borrow().pressed);
            (press_depth, press_scale) = glass_press(&id, pressed, &theme, window, cx);
            material.refraction *= press_depth;
        }

        if adaptive && let Some(state) = &state {
            let mut state = state.borrow_mut();
            if let Some(slot) = state.lease.id() {
                material.probe = slot;
                state.statistics = window.backdrop_statistics(slot).or(state.statistics);
                if let Some(statistics) = state.statistics {
                    state.appearance_flipped = deepen_tint(
                        state.appearance_flipped,
                        statistics.mean_luminance,
                        theme.appearance == Appearance::Dark,
                        theme.effects.glass_contrast_flip_low,
                        theme.effects.glass_contrast_flip_high,
                    );
                }
            }
        }

        let mut overlay_theme = (self.preset == GlassPreset::Clear).then(|| theme.clone());
        if adaptive
            && state
                .as_ref()
                .is_some_and(|state| state.borrow().appearance_flipped)
            && let Some(counterpart) = cx
                .try_global::<ThemeRegistry>()
                .and_then(|registry| registry.counterpart_for(&theme))
        {
            theme = counterpart;
            let counterpart_material = self.material(&theme);
            material = GlassMaterial {
                probe: material.probe,
                smoothing: material.smoothing,
                ..counterpart_material
            };
            material.refraction *= press_depth;
            overlay_theme = Some(theme.clone());
        }

        if self.protect_text && self.preset.resolved(&theme) == GlassPreset::Liquid {
            protect_text_contrast(&mut material, &theme);
        }
        tint_material(&mut material, self.preset.resolved(&theme), self.tint);
        let tone = self.tint.unwrap_or_else(|| theme.surface(self.surface));
        let fill = tone.opacity(alpha);
        let fallback = Some(tone.opacity(1.0));
        let translucent = alpha < 1.0;
        let collected: Rc<RefCell<Vec<GlassLobe<Pixels>>>> = Rc::default();

        let row = div()
            .flex()
            .flex_row()
            .text_color(theme.colors.text)
            .gap(px(self.gap.unwrap_or(theme.space(Space::Sm))))
            .children(self.panes.into_iter().map(|(ident, child)| {
                let bounds = Rc::default();
                let pane = div()
                    .rounded(px(radius))
                    .bg(fill)
                    .child(GlassContents {
                        radius: px(radius),
                        scale: press_scale,
                        bounds: Rc::clone(&bounds),
                        child,
                    })
                    .semantic_in(cx, NodeSpec::new(ident.semantic_id(), Role::Region));
                Lobe {
                    radius: px(radius),
                    bounds,
                    collected: Rc::clone(&collected),
                    child: pane.into_any_element(),
                }
            }))
            .semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::Group));

        let child = if interactive {
            let mut stateful = row;
            if self.pressable
                && let Some(state) = &state
            {
                let press = Rc::clone(state);
                stateful = stateful.on_mouse_down(MouseButton::Left, move |_, window, _| {
                    press.borrow_mut().pressed = true;
                    window.refresh();
                });
                let release = Rc::clone(state);
                stateful = stateful.on_mouse_up(MouseButton::Left, move |_, window, _| {
                    release.borrow_mut().pressed = false;
                    window.refresh();
                });
            }
            if self.pressable {
                let leave = state.clone();
                stateful = stateful.on_hover(move |hovered, window, _| {
                    if !*hovered && let Some(state) = &leave {
                        state.borrow_mut().pressed = false;
                    }
                    window.refresh();
                });
            }
            stateful.into_any_element()
        } else {
            row.into_any_element()
        };

        finish_glass(
            overlay_theme,
            BackdropLayer {
                radius: px(radius),
                material,
                bevel,
                lobes: LobeSource::Collected(collected),
                dimming: (self.dimmed && self.preset == GlassPreset::Clear)
                    .then_some(theme.effects.glass_dimming),
                translucent,
                fallback,
                focus_report: None,
                measured: Some(measured),
                child,
            },
        )
    }
}

/// Records where one pane landed, as a lobe of the group's shape.
///
/// The wrapper takes its child's layout, so the bounds it sees are the
/// pane's own; prepaint runs for every pane before the group paints, which
/// is what lets the group's single backdrop know all its lobes.
struct Lobe {
    radius: Pixels,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    collected: Rc<RefCell<Vec<GlassLobe<Pixels>>>>,
    child: AnyElement,
}

impl Element for Lobe {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.bounds.set(bounds);
        self.collected.borrow_mut().push(GlassLobe {
            bounds,
            corner_radii: Corners::all(self.radius).clamp_radii_for_quad_size(bounds.size),
        });
        window.with_rounded_content_mask(
            RoundedClip::new(bounds, Corners::all(self.radius)),
            |window| self.child.prepaint(window, cx),
        );
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

impl IntoElement for Lobe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Clips caller content to its surface, without applying coverage a second
/// time to the surface's already-rounded fill, border, or shadow. The parent
/// installs the same clip during prepaint and records its full bounds here;
/// a padded child's own bounds must not shrink the material's clipping region.
struct GlassContents {
    radius: Pixels,
    scale: f32,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    child: AnyElement,
}

impl IntoElement for GlassContents {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for GlassContents {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_visual_scale(self.scale, self.bounds.get().center(), |window| {
            self.child.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_rounded_content_mask(
            RoundedClip::new(self.bounds.get(), Corners::all(self.radius)),
            |window| {
                window.with_visual_scale(self.scale, self.bounds.get().center(), |window| {
                    self.child.paint(window, cx);
                });
            },
        );
    }
}

/// A shared normalized spring keeps optical and foreground responses in phase,
/// including reversal mid-press. Foreground scaling is a Kit policy inspired
/// by native label observations, not a fitted universal native transfer.
fn glass_press(
    id: &gpui::SharedString,
    pressed: bool,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> (f32, f32) {
    let progress = motion::tracked(
        id,
        if pressed { 1.0_f32 } else { 0.0 },
        MotionPolicy::spec(MotionRole::Tracking, theme),
        window,
        cx,
    );
    (
        1.0 + (theme.effects.glass_press_depth - 1.0) * progress,
        if cx.reduce_motion() {
            1.0
        } else {
            1.0 + (theme.effects.glass_press_scale - 1.0) * progress
        },
    )
}

/// Where the light is when the pointer carries it: the angle from the
/// surface's centre toward the pointer, in radians clockwise from straight
/// up, or `None` when the pointer is off the surface or dead centre — the
/// two positions that name no direction.
fn pointer_light_angle(bounds: Bounds<Pixels>, mouse: gpui::Point<Pixels>) -> Option<f32> {
    if !bounds.contains(&mouse) {
        return None;
    }
    let dx = f32::from(mouse.x) - f32::from(bounds.center().x);
    let dy = f32::from(mouse.y) - f32::from(bounds.center().y);
    (dx.abs() + dy.abs() > f32::EPSILON).then(|| dx.atan2(-dy))
}

/// Only measured controls within the token area budget may flip appearance.
fn can_flip(bounds: Bounds<Pixels>, max_extent: f32) -> bool {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    width > 0.0 && height > 0.0 && width * height <= max_extent * max_extent
}

/// Compose the caller's tint with the existing wash in straight-alpha form.
/// A foreground quad would cover the highlight and miss a group's bridge.
/// Frosted already owns its source-over fill, so it must not be tinted twice.
fn tint_material(material: &mut GlassMaterial<Pixels>, preset: GlassPreset, tint: Option<Hsla>) {
    if preset == GlassPreset::Frosted {
        return;
    }
    let Some(tint) = tint.filter(|tint| tint.a > 0.0) else {
        return;
    };
    let tint: Rgba = tint.into();
    let residual = material.wash.a * (1.0 - tint.a);
    let alpha = tint.a + residual;
    material.wash = Rgba {
        r: (tint.r * tint.a + material.wash.r * residual) / alpha,
        g: (tint.g * tint.a + material.wash.g * residual) / alpha,
        b: (tint.b * tint.a + material.wash.b * residual) / alpha,
        a: alpha,
    };
}

/// Keep untinted body text readable even before the probe arrives, or where its mean
/// conceals a locally opposed patch. Adjust the shader's achromatic wash, not
/// a second source-over face. The worst neutral backdrop includes the shader's
/// transmission gain and optical lift; 4.5 is the WCAG normal-text ratio.
fn protect_text_contrast(material: &mut GlassMaterial<Pixels>, theme: &Theme) {
    let text: Rgba = theme.colors.text.into();
    let foreground = gpui_kit_tokens::Color {
        red: text.r,
        green: text.g,
        blue: text.b,
        alpha: text.a,
    };
    let backdrop = if theme.appearance == Appearance::Dark {
        1.0
    } else {
        0.0
    };
    let contrast = |alpha: f32| {
        let channel = (backdrop * material.transmission_gain * (1.0 - alpha)
            + material.wash.r * alpha
            + material.optical_lift.r * material.optical_lift.a)
            .clamp(0.0, 1.0);
        gpui_kit_tokens::contrast_ratio(
            foreground,
            gpui_kit_tokens::Color {
                red: channel,
                green: channel,
                blue: channel,
                alpha: 1.0,
            },
        )
    };
    if contrast(material.wash.a) < 4.5 {
        let (mut low, mut high) = (material.wash.a, 1.0);
        for _ in 0..16 {
            let mid = (low + high) / 2.0;
            if contrast(mid) < 4.5 {
                low = mid;
            } else {
                high = mid;
            }
        }
        material.wash.a = high;
    }
}

fn glass_shadows(
    theme: &Theme,
    elevation: Elevation,
    statistics: Option<BackdropStatistics>,
) -> Vec<gpui::BoxShadow> {
    let strength = statistics.map_or(1.0, |statistics| {
        // Encoded samples in [0,1] have standard deviation at most 0.5.
        // Busy sampled backdrops need separation even when their mean is light.
        // This is a five-point cue, not an exhaustive image statistic.
        let separation = (1.0 - statistics.mean_luminance)
            .max(2.0 * statistics.luminance_variance.sqrt())
            .clamp(0.0, 1.0);
        theme.effects.glass_shadow_min
            + (theme.effects.glass_shadow_max - theme.effects.glass_shadow_min) * separation
    });
    theme
        .shadow(elevation)
        .iter()
        .cloned()
        .map(|mut shadow| {
            shadow.color.a = (shadow.color.a * strength).clamp(0.0, 1.0);
            shadow.style = gpui::ShadowStyle::Ring;
            shadow
        })
        .collect()
}

fn deepen_tint(deepened: bool, luminance: f32, dark_fill: bool, low: f32, high: f32) -> bool {
    let (deepen, release) = if dark_fill {
        (luminance > high, luminance < low)
    } else {
        (luminance < low, luminance > high)
    };
    if deepen {
        true
    } else if release {
        false
    } else {
        deepened
    }
}

/// Whether there is anything for a backdrop to show. An opaque fill hides all
/// optics; otherwise the material decides, and clear refraction remains real
/// even when its blur radius is zero.
#[cfg(test)]
fn shows_a_backdrop(alpha: f32, material: &GlassMaterial<Pixels>) -> bool {
    alpha < 1.0 && material.needs_backdrop()
}

/// Where a backdrop surface's shape comes from.
pub(crate) enum LobeSource {
    /// The single rounded rect the surface's own bounds describe.
    Surface,
    /// The lobes a group's panes recorded during prepaint, one rounded rect
    /// per pane, fused by the material's smoothing. Shared rather than owned
    /// because the panes are laid out by the same frame that paints the
    /// backdrop: prepaint fills it, paint reads it.
    Collected(Rc<RefCell<Vec<GlassLobe<Pixels>>>>),
}

/// The single scene layer, with the glass surface painted first inside it.
///
/// This is the piece [`Glass`], [`GlassGroup`] and [`super::Frost`] share. It
/// holds no policy of its own: what shape, what material and whether to paint
/// a backdrop at all are decided by the caller and passed in already resolved.
pub(crate) struct BackdropLayer {
    pub(crate) radius: Pixels,
    pub(crate) dimming: Option<f32>,
    pub(crate) material: GlassMaterial<Pixels>,
    /// Drawn after content, including opaque and budget-refused fallbacks.
    focus_report: Option<(Hsla, Pixels)>,
    bevel: Option<ResponsiveBevel>,
    pub(crate) lobes: LobeSource,
    pub(crate) translucent: bool,
    /// Ordinary surface fill used only when the framework's per-frame glass
    /// admission budget is exhausted. A caller already painting a tint needs
    /// no second fallback; material-only Liquid, Clear and Lens do.
    pub(crate) fallback: Option<gpui::Hsla>,
    /// Where to record the bounds this layer was painted at, for a caller
    /// whose pointer math needs them next frame.
    pub(crate) measured: Option<Rc<Cell<Bounds<Pixels>>>>,
    pub(crate) child: AnyElement,
}

impl Element for BackdropLayer {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(cell) = &self.measured {
            measure::record(cell, bounds, window);
        }
        if matches!(&self.lobes, LobeSource::Surface) {
            window.with_rounded_content_mask(
                RoundedClip::new(bounds, Corners::all(self.radius)),
                |window| self.child.prepaint(window, cx),
            );
        } else {
            // Each pane clips its own descendants; the fused optical bridge
            // is not the intersection of its constituent rounded rectangles.
            self.child.prepaint(window, cx);
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Use the same fitted shape for optics and the inward report border.
        let corner_radii = Corners::all(self.radius).clamp_radii_for_quad_size(bounds.size);
        let mut paint_content = |window: &mut Window, cx: &mut App| {
            self.child.paint(window, cx);
            if let Some((color, width)) = self.focus_report {
                window.paint_quad(
                    gpui::outline(bounds, color, gpui::BorderStyle::default())
                        .corner_radii(corner_radii)
                        .border_widths(width),
                );
            }
        };
        if !self.translucent {
            paint_content(window, cx);
            return;
        }
        let collected;
        let lobes: &[GlassLobe<Pixels>] = match &self.lobes {
            LobeSource::Surface => &[],
            LobeSource::Collected(lobes) => {
                collected = lobes.borrow();
                &collected[..collected.len().min(MAX_GLASS_LOBES)]
            }
        };
        let mut material = self.material;
        if let Some(bevel) = self.bevel {
            material.bevel = bevel.resolve(bounds, lobes);
        }
        if let Some(alpha) = self.dimming {
            // Black source-over on the transmitted source is multiplication.
            // Keep it inside the material, before lift/rim, rather than relying
            // on a same-layer quad to enter the renderer's backdrop snapshot.
            material.transmission_gain *= 1.0 - alpha.clamp(0.0, 1.0);
        }
        if !material.needs_backdrop() {
            paint_content(window, cx);
            return;
        }
        // Match the styled child's fitted radii. Theme Pill is deliberately
        // oversized; raw paint APIs preserve such radii rather than fitting
        // them, so passing it through would discard both dimming and optics.
        window.paint_layer(bounds, |window| {
            if let Some(fallback) = self.fallback {
                window.paint_backdrop_glass_with_fallback(
                    bounds,
                    corner_radii,
                    material,
                    lobes,
                    fallback,
                );
            } else {
                window.paint_backdrop_glass(bounds, corner_radii, material, lobes);
            }
            paint_content(window, cx);
        });
    }
}

impl IntoElement for BackdropLayer {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
impl Glass {
    /// Resolve wrapper material without a window. Keep test-only declarations
    /// after production APIs so the generated catalog includes GlassGroup.
    pub(crate) fn material_for_test(&self, theme: &Theme) -> GlassMaterial<Pixels> {
        self.material(theme)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn rounded_glass_descendants_reject_corner_input(cx: &mut gpui::TestAppContext) {
        let clicks = Rc::new(Cell::new(0));
        let received = clicks.clone();
        let mut harness = gpui_kit_testkit::harness::Harness::new(
            cx,
            |cx| {
                crate::install(cx);
                cx.set_reduce_motion(true);
            },
            move |_, cx| {
                let child = |id: &'static str, width| {
                    let received = received.clone();
                    div()
                        .id(id)
                        .w(px(width))
                        .h(px(48.0))
                        .on_mouse_down(MouseButton::Left, move |_, _, _| {
                            received.set(received.get() + 1)
                        })
                };
                div()
                    .p(px(40.0))
                    .flex()
                    .gap(px(32.0))
                    .child(
                        Glass::new("clip.input.single")
                            .radius_px(24.0)
                            .child(child("clip.input.single.child", 80.0)),
                    )
                    .child(
                        GlassGroup::new("clip.input.group")
                            .radius(Radius::Pill)
                            .pane("clip.input.left", child("clip.input.left.child", 48.0))
                            .pane("clip.input.right", child("clip.input.right.child", 48.0)),
                    )
                    .child(
                        crate::overlay::surface(
                            "clip.input.frame",
                            cx.theme(),
                            crate::overlay::OverlaySurface::MODAL,
                        )
                        .semantic_in(cx, NodeSpec::new("clip.input.frame", Role::Region))
                        .child(child("clip.input.frame.child", 80.0)),
                    )
                    .into_any_element()
            },
        );
        for id in [
            "clip.input.single",
            "clip.input.left",
            "clip.input.right",
            "clip.input.frame",
        ] {
            let bounds = harness.bounds(id).expect("glass region");
            let previous = clicks.get();
            harness.context().simulate_click(
                bounds.origin + gpui::point(px(1.0), px(1.0)),
                gpui::Modifiers::none(),
            );
            assert_eq!(clicks.get(), previous, "rounded corner of {id}");
            harness
                .context()
                .simulate_click(bounds.center(), gpui::Modifiers::none());
            assert_eq!(clicks.get(), previous + 1, "interior of {id}");
        }
    }

    #[test]
    fn optical_overrides_survive_theme_resolution_and_respect_accessibility() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            let theme = theme.modify(|theme| {
                theme.effects.glass_thickness = 5.0;
                theme.effects.glass_refractive_index = 1.7;
                theme.effects.glass_backdrop_depth = 19.0;
            });
            for preset in [GlassPreset::Liquid, GlassPreset::Clear, GlassPreset::Lens] {
                let base = preset.material(&theme);
                assert_eq!(base.thickness, px(5.0));
                assert_eq!(base.refractive_index, 1.7);
                assert_eq!(base.backdrop_depth, px(19.0));
                let glass = Glass::new("optic").preset(preset).thickness(11.0);
                let group = GlassGroup::new("optics")
                    .preset(preset)
                    .refractive_index(2.1)
                    .backdrop_depth(37.0);
                let single = glass.material(&theme);
                assert_eq!(single.thickness, px(11.0));
                assert_eq!(single.refractive_index, 1.7);
                assert_eq!(single.backdrop_depth, px(19.0));
                let fused = group.material(&theme);
                assert_eq!(fused.thickness, px(5.0));
                assert_eq!(fused.refractive_index, 2.1);
                assert_eq!(fused.backdrop_depth, px(37.0));
                let reduced = theme.clone().with_reduce_transparency(true);
                let frosted = GlassPreset::Frosted.material(&reduced);
                assert_eq!(glass.material(&reduced), frosted);
                let mut expected = frosted;
                expected.smoothing = px(reduced.effects.glass_merge_distance);
                assert_eq!(group.material(&reduced), expected);
            }
        }
    }

    #[test]
    fn optical_builders_clamp_distances_and_index_independently() {
        let theme = Theme::studio_dark();
        let single = Glass::new("optic")
            .thickness(-3.0)
            .refractive_index(9.0)
            .backdrop_depth(-7.0)
            .refraction(-0.4)
            .material(&theme);
        assert_eq!(single.thickness, px(0.0));
        assert_eq!(single.refractive_index, 2.5);
        assert_eq!(single.backdrop_depth, px(0.0));
        assert_eq!(single.refraction, -0.4);
        let group = GlassGroup::new("optics")
            .thickness(13.0)
            .refractive_index(0.5)
            .backdrop_depth(29.0)
            .material(&theme);
        assert_eq!(group.thickness, px(13.0));
        assert_eq!(group.refractive_index, 1.0);
        assert_eq!(group.backdrop_depth, px(29.0));
    }

    #[test]
    fn clear_optics_show_a_backdrop_without_blur() {
        let theme = Theme::studio_dark();
        let mut lens = GlassMaterial::clear();
        lens.bevel = px(12.0);
        lens.refraction = 0.34;

        assert_eq!(GlassPreset::Liquid.tint_alpha(&theme), 0.0);
        assert_eq!(GlassPreset::Lens.tint_alpha(&theme), 0.0);
        assert!(shows_a_backdrop(0.72, &lens));
        assert!(
            !shows_a_backdrop(1.0, &lens),
            "an opaque fill hides every optical result"
        );
        assert!(
            !shows_a_backdrop(0.72, &GlassMaterial::clear()),
            "a plain clear copy changes no pixel"
        );
    }

    #[test]
    fn the_frosted_preset_asks_for_no_optics() {
        let theme = Theme::studio_dark();
        assert_eq!(
            GlassPreset::Frosted.material(&theme),
            GlassMaterial::frosted(px(theme.effects.glass_frost_blur))
        );
        assert!(GlassPreset::Frosted.material(&theme).is_flat());
    }

    #[test]
    fn focused_replaces_only_the_hairline_in_every_material() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            for reduced in [false, true] {
                let theme = theme.clone().with_reduce_transparency(reduced);
                for preset in [
                    GlassPreset::Clear,
                    GlassPreset::Liquid,
                    GlassPreset::Frosted,
                    GlassPreset::Lens,
                ] {
                    let ordinary = Glass::new("test.focus").preset(preset).material(&theme);
                    let mut expected = ordinary;
                    expected.hairline = Pixels::ZERO;
                    assert_eq!(
                        Glass::new("test.focus")
                            .preset(preset)
                            .focused(true)
                            .dimmed(true)
                            .material(&theme),
                        expected
                    );
                    assert_eq!(
                        Glass::new("test.focus")
                            .preset(preset)
                            .focused(true)
                            .focused(false)
                            .material(&theme),
                        ordinary
                    );
                }
            }
        }
    }

    #[test]
    fn the_lens_preset_bends_without_colouring_or_lighting() {
        let theme = Theme::studio_dark();
        let mut material = GlassPreset::Lens.material(&theme);
        material.bevel = GlassPreset::Lens
            .bevel(&theme)
            .expect("a lens has a responsive bevel")
            .resolve(surface_bounds(), &[]);

        assert!(material.bends_light());
        assert_eq!(material.blur_radius, px(0.0), "a lens is clear by default");
        assert_eq!(material.dispersion, 0.0, "a lens does not fringe text");
        assert_eq!(material.specular, 0.0, "a lens carries no highlight");
    }

    #[test]
    fn the_liquid_preset_takes_every_optic_from_tokens() {
        let theme = Theme::studio_dark();
        let mut material = GlassPreset::Liquid.material(&theme);
        material.bevel = GlassPreset::Liquid
            .bevel(&theme)
            .expect("liquid has a responsive bevel")
            .resolve(surface_bounds(), &[]);

        assert_eq!(
            material.bevel,
            px(100.0 * theme.effects.glass_bevel_ratio),
            "the profile follows the control's short edge"
        );
        assert_eq!(material.blur_radius, px(theme.effects.glass_frost_blur));
        assert_eq!(material.saturation, theme.effects.glass_saturation);
        assert_eq!(
            material.wash,
            Rgba {
                r: 0.,
                g: 0.,
                b: 0.,
                a: theme.effects.glass_wash
            }
        );
        let light = Theme::studio_light();
        assert_eq!(
            GlassPreset::Liquid.material(&light).wash,
            Rgba {
                r: 1.,
                g: 1.,
                b: 1.,
                a: light.effects.glass_wash
            }
        );
        assert_eq!(material.refraction, theme.effects.glass_refraction);
        assert_eq!(material.dispersion, theme.effects.glass_dispersion);
        assert_eq!(material.specular, theme.effects.glass_specular);
        assert_eq!(
            material.transmission_gain,
            theme.effects.glass_transmission_gain
        );
        assert_eq!(material.optical_lift.a, theme.effects.glass_optical_lift);
        assert_eq!(material.hairline, px(theme.effects.glass_hairline));
        assert_eq!(material.light_angle, theme.effects.glass_light_angle);
        assert!(!material.is_flat());
    }

    #[test]
    fn optical_tint_composes_with_wash_without_covering_the_rim() {
        let base = GlassMaterial {
            wash: Rgba {
                r: 0.2,
                g: 0.4,
                b: 0.8,
                a: 0.5,
            },
            specular: 0.37,
            ..GlassMaterial::clear()
        };
        for preset in [GlassPreset::Liquid, GlassPreset::Clear, GlassPreset::Lens] {
            let mut material = base;
            tint_material(&mut material, preset, Some(gpui::hsla(0.0, 1.0, 0.5, 0.25)));
            // red at 1/4 over the prior wash at 1/2: alpha 5/8;
            // premultiplied colour = (0.325, 0.15, 0.3).
            for (actual, expected) in [
                (material.wash.r, 0.52),
                (material.wash.g, 0.24),
                (material.wash.b, 0.48),
                (material.wash.a, 0.625),
            ] {
                assert!((actual - expected).abs() < 1e-6);
            }
            assert_eq!(material.specular, base.specular);
            let mut unchanged = base;
            tint_material(&mut unchanged, preset, Some(gpui::hsla(0.3, 0.8, 0.7, 0.0)));
            assert_eq!(unchanged, base);
            tint_material(&mut material, preset, Some(gpui::hsla(0.0, 1.0, 0.5, 1.0)));
            assert_eq!(material.wash, gpui::red().into());
        }
        let mut frosted = base;
        tint_material(&mut frosted, GlassPreset::Frosted, Some(gpui::red()));
        assert_eq!(frosted, base);
    }

    #[test]
    fn a_tint_and_edge_mask_are_caller_owned() {
        let theme = Theme::studio_dark();
        let tint = gpui::hsla(0.6, 0.4, 0.5, 1.0);
        let glass = Glass::new("surface")
            .preset(GlassPreset::Frosted)
            .tint(tint)
            .edge_mask(GlassEdge::Top, 28.0);
        let material = glass.material(&theme);

        assert_eq!(material.edge_mask_edge, GlassEdge::Top.as_f32());
        assert_eq!(material.edge_mask_band, px(28.0));
        assert_eq!(
            GlassPreset::Frosted.tint_alpha(&theme),
            theme.effects.glass_alpha
        );
        let _ = tint;
    }

    #[test]
    fn a_caller_override_wins_over_the_preset() {
        let theme = Theme::studio_dark();
        let glass = Glass::new("surface")
            .preset(GlassPreset::Liquid)
            .refraction(0.1)
            .dispersion(0.0)
            .specular(0.9)
            .light_angle(1.5);
        let material = glass.material(&theme);

        assert_eq!(material.refraction, 0.1);
        assert_eq!(material.dispersion, 0.0);
        assert_eq!(material.specular, 0.9);
        assert_eq!(material.light_angle, 1.5);
        assert_eq!(
            material.transmission_gain, theme.effects.glass_transmission_gain,
            "what the caller did not override stays with the preset"
        );
    }

    fn surface_bounds() -> Bounds<Pixels> {
        Bounds {
            origin: gpui::point(px(100.), px(100.)),
            size: gpui::size(px(200.), px(100.)),
        }
    }

    #[test]
    fn responsive_bevel_scales_and_clamps_by_short_edge() {
        let policy = ResponsiveBevel {
            ratio: 0.225,
            min: px(8.0),
            max: px(36.0),
        };
        let bounds = |width, height| Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: gpui::size(px(width), px(height)),
        };

        assert_eq!(policy.resolve(bounds(200.0, 20.0), &[]), px(8.0));
        assert_eq!(policy.resolve(bounds(200.0, 120.0), &[]), px(27.0));
        assert_eq!(policy.resolve(bounds(400.0, 240.0), &[]), px(36.0));
    }

    #[test]
    fn a_group_bevel_uses_constituent_controls_not_the_union() {
        let policy = ResponsiveBevel {
            ratio: 0.225,
            min: px(0.0),
            max: px(100.0),
        };
        let lobe = |x, width, height| GlassLobe {
            bounds: Bounds {
                origin: gpui::point(px(x), px(0.0)),
                size: gpui::size(px(width), px(height)),
            },
            corner_radii: Corners::default(),
        };
        let lobes = [lobe(0.0, 100.0, 40.0), lobe(112.0, 160.0, 60.0)];
        let union = Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: gpui::size(px(272.0), px(60.0)),
        };

        assert_eq!(policy.resolve(union, &lobes), px(9.0));
    }

    #[test]
    fn the_pointer_names_the_light_by_where_it_stands() {
        let bounds = surface_bounds();

        let above = pointer_light_angle(bounds, gpui::point(px(200.), px(110.)))
            .expect("a pointer on the surface lights it");
        assert!(above.abs() < 1e-6, "straight above the centre is angle 0");

        let right = pointer_light_angle(bounds, gpui::point(px(290.), px(150.)))
            .expect("a pointer on the surface lights it");
        assert!(
            (right - std::f32::consts::FRAC_PI_2).abs() < 1e-6,
            "due right is a quarter turn clockwise, got {right}"
        );

        assert_eq!(
            pointer_light_angle(bounds, gpui::point(px(10.), px(10.))),
            None,
            "a pointer off the surface leaves the theme's light alone"
        );
        assert_eq!(
            pointer_light_angle(bounds, gpui::point(px(200.), px(150.))),
            None,
            "dead centre names no direction"
        );
    }

    #[test]
    fn the_tint_deepens_against_the_backdrop_and_holds_inside_the_band() {
        let (low, high) = (0.42, 0.58);

        // A dark fill dissolves over a bright backdrop.
        assert!(deepen_tint(false, 0.9, true, low, high));
        assert!(!deepen_tint(true, 0.1, true, low, high));
        // A light fill dissolves over a dark backdrop.
        assert!(deepen_tint(false, 0.1, false, low, high));
        assert!(!deepen_tint(true, 0.9, false, low, high));
        // Inside the band the previous answer stands, whichever it was.
        assert!(deepen_tint(true, 0.5, true, low, high));
        assert!(!deepen_tint(false, 0.5, true, low, high));
    }

    #[test]
    fn adaptive_glass_starts_with_the_window_appearance() {
        assert!(!GlassState::default().appearance_flipped);
    }

    #[test]
    fn clear_keeps_light_content_and_a_dark_reduced_body_in_both_appearances() {
        for host in [Theme::studio_dark(), Theme::studio_light()] {
            for reduce in [false, true] {
                let theme = GlassPreset::Clear
                    .content_theme(&host.clone().with_reduce_transparency(reduce));
                assert_eq!(theme.appearance, Appearance::Dark);
                assert_eq!(theme.colors.text, host.colors.on_media_foreground);
                assert_eq!(theme.colors.hairline, host.colors.on_media_hairline);
                assert_eq!(
                    theme.surface(Surface::Raised),
                    host.colors.on_media_background
                );
                assert_eq!(theme.reduce_transparency, reduce);
                assert_eq!(
                    GlassPreset::Clear.resolved(&theme),
                    if reduce {
                        GlassPreset::Frosted
                    } else {
                        GlassPreset::Clear
                    }
                );
            }
            assert_eq!(
                GlassPreset::Liquid.content_theme(&host).appearance,
                host.appearance
            );
        }
    }

    #[test]
    fn regular_body_text_survives_opposed_backdrops_without_flipping() {
        for theme in [Theme::studio_dark(), Theme::studio_light()] {
            let mut material = GlassPreset::Liquid.material(&theme);
            let direction = material.wash.r;
            protect_text_contrast(&mut material, &theme);
            assert_eq!(material.wash.r, direction);
            assert!(material.wash.a < 1.0);
            let text: Rgba = theme.colors.text.into();
            let foreground = gpui_kit_tokens::Color {
                red: text.r,
                green: text.g,
                blue: text.b,
                alpha: text.a,
            };
            for backdrop in [0.0, 1.0] {
                let channel = (backdrop * material.transmission_gain * (1.0 - material.wash.a)
                    + direction * material.wash.a
                    + material.optical_lift.r * material.optical_lift.a)
                    .clamp(0.0, 1.0);
                let background = gpui_kit_tokens::Color {
                    red: channel,
                    green: channel,
                    blue: channel,
                    alpha: 1.0,
                };
                assert!(gpui_kit_tokens::contrast_ratio(foreground, background) >= 4.5);
            }
        }
    }

    #[test]
    fn glass_shadows_are_outside_only_and_heavier_on_dark_backdrops() {
        let theme = Theme::studio_light();
        let samples = |values: [u8; 5]| {
            let rgba = values.map(|v| [v, v, v, 255]).concat();
            BackdropStatistics::from_encoded_texels(&rgba, 4, false)
        };
        let bright = glass_shadows(&theme, Elevation::Overlay, Some(samples([255; 5])));
        let dark = glass_shadows(&theme, Elevation::Overlay, Some(samples([0; 5])));
        let flat = glass_shadows(&theme, Elevation::Overlay, Some(samples([153; 5])));
        let busy = glass_shadows(
            &theme,
            Elevation::Overlay,
            Some(samples([0, 0, 255, 255, 255])),
        );
        assert!(!bright.is_empty());
        for (bright, dark) in bright.iter().zip(&dark) {
            assert_eq!(bright.style, gpui::ShadowStyle::Ring);
            assert!(dark.color.a > bright.color.a);
            assert_eq!(dark.blur_radius, bright.blur_radius);
        }
        for (flat, busy) in flat.iter().zip(&busy) {
            assert!(
                busy.color.a > flat.color.a,
                "equal means, different variance"
            );
            assert_eq!(flat.blur_radius, busy.blur_radius);
        }
    }

    #[gpui::test]
    fn glass_press_scales_foreground_without_reflow_and_releases(cx: &mut gpui::TestAppContext) {
        use std::time::Duration;
        for grouped in [false, true] {
            let captured = Rc::new(Cell::new((
                Bounds::default(),
                gpui::VisualTransform::default(),
            )));
            let observed = captured.clone();
            let mut harness = gpui_kit_testkit::harness::Harness::new(
                cx,
                |cx| {
                    crate::install(cx);
                    cx.set_reduce_motion(false);
                },
                move |_, _| {
                    let observed = observed.clone();
                    let child = div().w(px(120.)).h(px(60.)).pl(px(7.)).pt(px(11.)).child(
                        gpui::canvas(
                            move |bounds, w, _| {
                                observed.set((bounds, w.visual_transform()));
                                w.visual_transform()
                            },
                            |_, transform, w, _| assert_eq!(transform, w.visual_transform()),
                        )
                        .w(px(40.))
                        .h(px(20.)),
                    );
                    let glass = if grouped {
                        GlassGroup::new("press")
                            .pressable(true)
                            .pane("press.pane", child)
                            .into_any_element()
                    } else {
                        Glass::new("press")
                            .pressable(true)
                            .child(child)
                            .into_any_element()
                    };
                    div()
                        .flex()
                        .flex_col()
                        .items_start()
                        .pl(px(33.))
                        .pt(px(21.))
                        .child(glass)
                        .into_any_element()
                },
            );
            harness.frame();
            let original = captured.get();
            let outer = harness.bounds("press").expect("surface");
            harness.context().simulate_mouse_down(
                outer.center(),
                MouseButton::Left,
                gpui::Modifiers::none(),
            );
            harness.frame();
            harness.advance(Duration::from_secs(2));
            let (logical, transform) = captured.get();
            assert_eq!(logical, original.0, "foreground layout is fixed");
            assert_eq!(
                harness.bounds("press").expect("pressed surface"),
                outer,
                "outer surface is fixed"
            );
            let scale = harness.update(|_, cx| cx.theme().effects.glass_press_scale);
            assert!((transform.scale() - scale).abs() < 0.00001);
            let expected = outer.center() + (logical.origin - outer.center()) * scale;
            let actual = transform.map_point(logical.origin);
            assert!((actual.x - expected.x).abs() < px(0.0001));
            assert!((actual.y - expected.y).abs() < px(0.0001));
            harness.context().simulate_mouse_up(
                outer.center(),
                MouseButton::Left,
                gpui::Modifiers::none(),
            );
            harness.frame();
            harness.advance(Duration::from_secs(2));
            assert_eq!(captured.get(), original, "release restores exact identity");
            harness.update(|_, cx| cx.set_reduce_motion(true));
            harness.context().simulate_mouse_down(
                outer.center(),
                MouseButton::Left,
                gpui::Modifiers::none(),
            );
            harness.frame();
            assert_eq!(captured.get(), original, "reduced motion suppresses scale");
        }
    }

    #[test]
    fn reader_preference_resolves_every_preset_to_frosted() {
        let theme = Theme::studio_dark().with_reduce_transparency(true);
        let frosted = GlassMaterial::frosted(px(theme.effects.glass_frost_blur));
        for preset in [
            GlassPreset::Liquid,
            GlassPreset::Clear,
            GlassPreset::Lens,
            GlassPreset::Frosted,
        ] {
            assert_eq!(preset.material(&theme), frosted);
            assert_eq!(preset.tint_alpha(&theme), theme.effects.glass_alpha);
            assert!(preset.bevel(&theme).is_none());
            assert_eq!(
                Glass::new("reduced")
                    .preset(preset)
                    .refraction(2.0)
                    .blur(0.0)
                    .material(&theme),
                frosted
            );
        }
    }

    #[test]
    fn only_small_measured_surfaces_may_flip() {
        assert!(!can_flip(Bounds::default(), 72.0));
        assert!(can_flip(
            Bounds::new(gpui::point(px(0.), px(0.)), gpui::size(px(120.), px(32.))),
            72.0
        ));
        assert!(!can_flip(surface_bounds(), 72.0));
    }

    #[test]
    fn a_blur_override_composes_frost_with_liquid() {
        let theme = Theme::studio_dark();
        let material = Glass::new("surface")
            .preset(GlassPreset::Liquid)
            .blur(12.0)
            .material(&theme);

        assert_eq!(material.blur_radius, px(12.0));
        assert_eq!(material.refraction, theme.effects.glass_refraction);
        assert_eq!(material.optical_lift.a, theme.effects.glass_optical_lift);
    }
}
