//! A surface that shows what is behind it, out of focus and bent.
//!
//! [`Glass`] is the material a popover, a dialog or a rail is placed on when
//! the window itself is translucent. The pixels underneath are blurred, the
//! surface colour is laid over the blur at `effect.glassAlpha`, and — where
//! the renderer can — the edge behaves like the rim of a body of glass: it
//! bends what is behind it, splits the bend into colour, and catches a
//! highlight.
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
//! # Where the optics do not exist
//!
//! A backdrop blur is a renderer capability, not a paintable colour, and the
//! optics on top of it are another. Where the renderer has neither, the fill
//! is all that remains, which is a legible surface rather than a broken one —
//! this is why the fill is painted whether or not anything was blurred. A
//! theme that declares itself opaque by setting `effect.glassAlpha` to 1 takes
//! the same path deliberately: there is nothing to see through, so nothing is
//! blurred and nothing is bent.
//!
//! `docs/coverage.md` records which renderer does which of these today.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use gpui::{
    AnyElement, App, Bounds, Corners, Element, GlassLobe, GlassMaterial, GlobalElementId,
    InspectorElementId, InteractiveElement as _, IntoElement, LayoutId, MAX_GLASS_LOBES,
    MAX_LUMINANCE_PROBES, MouseButton, ParentElement, Pixels, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, Theme};

use crate::foundation::Ident;
use crate::layout::measure;
use crate::motion::{self, keyed};

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
    /// The theme's full optics: the edge bends the backdrop, splits it into
    /// colour, and carries a highlight.
    #[default]
    Liquid,
    /// The bend without the colour split or the highlight, for a surface that
    /// sits over text that the dispersion would otherwise fringe.
    Lens,
}

impl GlassPreset {
    /// How much of the surface is tint, at this theme.
    ///
    /// A frosted surface has nothing but its fill to separate it from the
    /// backdrop, so it takes `effect.glassAlpha`. The other two are read by
    /// their optics, and a fill that strong would cover them, so they take
    /// the thinner `effect.glassLiquidAlpha`.
    pub fn tint_alpha(self, theme: &Theme) -> f32 {
        match self {
            GlassPreset::Frosted => theme.effects.glass_alpha,
            GlassPreset::Liquid | GlassPreset::Lens => theme.effects.glass_liquid_alpha,
        }
    }

    /// The material this preset asks the renderer for, at this theme.
    ///
    /// Every value comes from a token: a preset names a combination, it does
    /// not carry numbers of its own.
    pub fn material(self, theme: &Theme) -> GlassMaterial<Pixels> {
        let effects = &theme.effects;
        match self {
            GlassPreset::Frosted => GlassMaterial::frosted(),
            GlassPreset::Liquid => GlassMaterial {
                bevel: px(effects.glass_bevel),
                refraction: effects.glass_refraction,
                dispersion: effects.glass_dispersion,
                specular: effects.glass_specular,
                light_angle: effects.glass_light_angle,
                specular_sharpness: effects.glass_specular_sharpness,
                ..GlassMaterial::frosted()
            },
            GlassPreset::Lens => GlassMaterial {
                bevel: px(effects.glass_bevel),
                refraction: effects.glass_refraction,
                ..GlassMaterial::frosted()
            },
        }
    }
}

/// Which luminance probe slots are claimed, one bit per slot, across the
/// process. Two windows never collide by sharing a slot number — each window
/// reads its own renderer — so a process-wide ledger is merely conservative,
/// never wrong.
static PROBE_SLOTS: AtomicU32 = AtomicU32::new(0);

/// One surface's claim on a luminance probe slot, freed when the surface
/// stops rendering and its keyed state is dropped.
#[derive(Default)]
struct ProbeLease(Option<u32>);

impl ProbeLease {
    /// The slot this lease holds, claiming the lowest free one on first use.
    /// `None` once every slot is claimed, which a caller treats exactly like
    /// a renderer that takes no probes: the surface keeps its unadapted fill.
    fn slot(&mut self) -> Option<u32> {
        if self.0.is_none() {
            let mut claimed = PROBE_SLOTS.load(Ordering::Relaxed);
            loop {
                let free = (!claimed).trailing_zeros();
                if free as usize >= MAX_LUMINANCE_PROBES {
                    return None;
                }
                match PROBE_SLOTS.compare_exchange_weak(
                    claimed,
                    claimed | (1 << free),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        self.0 = Some(free);
                        break;
                    }
                    Err(now) => claimed = now,
                }
            }
        }
        self.0
    }
}

impl Drop for ProbeLease {
    fn drop(&mut self) {
        if let Some(slot) = self.0 {
            PROBE_SLOTS.fetch_and(!(1 << slot), Ordering::Relaxed);
        }
    }
}

/// The transient visual state an interactive glass surface keeps across
/// frames: whether it is pressed, which side of the contrast band it last
/// settled on, and its probe slot.
#[derive(Default)]
struct GlassState {
    pressed: bool,
    deepened: bool,
    lease: ProbeLease,
}

/// A glass surface: blurred and bent backdrop, tinted fill, caller's content.
#[derive(IntoElement)]
pub struct Glass {
    ident: Ident,
    surface: Surface,
    radius: Radius,
    blur: Option<f32>,
    preset: GlassPreset,
    refraction: Option<f32>,
    dispersion: Option<f32>,
    specular: Option<f32>,
    light_angle: Option<f32>,
    track_pointer: bool,
    pressable: bool,
    adaptive: bool,
    child: Option<AnyElement>,
}

impl std::fmt::Debug for Glass {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Glass")
            .field("ident", &self.ident)
            .field("surface", &self.surface)
            .field("radius", &self.radius)
            .field("blur", &self.blur)
            .field("preset", &self.preset)
            .field("refraction", &self.refraction)
            .field("dispersion", &self.dispersion)
            .field("specular", &self.specular)
            .field("light_angle", &self.light_angle)
            .field("track_pointer", &self.track_pointer)
            .field("pressable", &self.pressable)
            .field("adaptive", &self.adaptive)
            .field("has_child", &self.child.is_some())
            .finish()
    }
}

impl Glass {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            surface: Surface::Overlay,
            radius: Radius::Card,
            blur: None,
            preset: GlassPreset::default(),
            refraction: None,
            dispersion: None,
            specular: None,
            light_angle: None,
            track_pointer: false,
            pressable: false,
            adaptive: false,
            child: None,
        }
    }

    /// Which surface colour is laid over the blur. The overlay surface is the
    /// default because that is what a floating thing is made of.
    pub fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    /// The rounding of the glass. It clips the blur as well as the fill, so a
    /// caller rounding the card inside must say the same thing here or the
    /// blur will show past the corners.
    pub fn radius(mut self, radius: Radius) -> Self {
        self.radius = radius;
        self
    }

    /// How far the backdrop is blurred, in pixels, when `effect.glassBlur` is
    /// not what this particular surface wants.
    pub fn blur(mut self, blur: f32) -> Self {
        self.blur = Some(blur.max(0.0));
        self
    }

    /// Which combination of optics the surface asks for.
    pub fn preset(mut self, preset: GlassPreset) -> Self {
        self.preset = preset;
        self
    }

    /// How thick the glass reads, overriding `effect.glassRefraction`.
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
    /// `effect.glassPressDepth`, springing back on release. This is a purely
    /// visual response: the surface publishes no action and installs no
    /// handler beyond the press tracking itself.
    pub fn pressable(mut self, pressable: bool) -> Self {
        self.pressable = pressable;
        self
    }

    /// Deepen the tint from `effect.glassLiquidAlpha` to `effect.glassAlpha`
    /// while the blurred backdrop opposes the surface's own fill — a dark
    /// panel over a bright backdrop, a light one over a dark backdrop — where
    /// a thin tint would let the backdrop wash out the content sitting on it.
    ///
    /// The reading comes from [`Window::backdrop_luminance`] one frame after
    /// the backdrop moved, so the flip lands on the next frame the window
    /// draws. On a renderer that takes no probes the reading never arrives
    /// and the tint honestly stays thin.
    pub fn adaptive(mut self, adaptive: bool) -> Self {
        self.adaptive = adaptive;
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }

    /// The material this surface asks the renderer for, for tests that need to
    /// assert what a wrapper resolved to without rendering a window.
    #[cfg(test)]
    pub(crate) fn material_for_test(&self, theme: &Theme) -> GlassMaterial<Pixels> {
        self.material(theme)
    }

    /// The material this surface asks the renderer for: the preset's
    /// combination with the caller's overrides laid over it.
    fn material(&self, theme: &Theme) -> GlassMaterial<Pixels> {
        let mut material = self.preset.material(theme);
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
        material
    }
}

impl RenderOnce for Glass {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let radius = theme.radius(self.radius);
        let mut alpha = self.preset.tint_alpha(&theme).clamp(0.0, 1.0);
        let blur = self.blur.unwrap_or(theme.effects.glass_blur);
        let mut material = self.material(&theme);

        let id = self.ident.semantic_id();
        let interactive = self.track_pointer || self.pressable || self.adaptive;
        let state = interactive.then(|| keyed::slot::<GlassState>(&id, cx));
        let measured = measure::cell(&id, cx);
        let bounds = measured.get();

        // The pointer carries the light: the angle from the surface's centre
        // to the pointer, clockwise from straight up, which is the convention
        // the material states its light in. Off the surface, or dead centre,
        // the theme's light stays where it was.
        if self.track_pointer
            && let Some(angle) = pointer_light_angle(bounds, window.mouse_position())
        {
            material.light_angle = angle;
        }

        // A press reads as pushing the glass down into the backdrop: the
        // refraction deepens toward `effect.glassPressDepth` on a spring and
        // returns on release. The layout, the hit target and the semantics
        // never move; only the optics answer the finger.
        if self.pressable {
            let pressed = state.as_ref().is_some_and(|state| state.borrow().pressed);
            let target = if pressed {
                theme.effects.glass_press_depth
            } else {
                1.0
            };
            let depth = motion::tracked(&id, target, motion::state_change(&theme), window, cx);
            material.refraction *= depth;
        }

        if self.adaptive
            && let Some(state) = &state
        {
            let mut state = state.borrow_mut();
            if let Some(slot) = state.lease.slot() {
                material.probe = slot;
                if let Some(luminance) = window.backdrop_luminance(slot) {
                    state.deepened = deepen_tint(
                        state.deepened,
                        luminance,
                        theme.surface(self.surface).l < 0.5,
                        theme.effects.glass_contrast_flip_low,
                        theme.effects.glass_contrast_flip_high,
                    );
                }
            }
            if state.deepened {
                alpha = alpha.max(theme.effects.glass_alpha).clamp(0.0, 1.0);
            }
        }

        let fill = theme.surface(self.surface).opacity(alpha);
        let translucent = shows_a_backdrop(alpha, blur);

        let surface = div()
            .rounded(px(radius))
            .bg(fill)
            .children(self.child)
            .semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::Region));

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
            let leave = state.clone();
            stateful = stateful.on_hover(move |hovered, window, _| {
                if !*hovered && let Some(state) = &leave {
                    state.borrow_mut().pressed = false;
                }
                window.refresh();
            });
            return BackdropLayer {
                radius: px(radius),
                blur: px(blur),
                material,
                lobes: LobeSource::Surface,
                translucent,
                measured: Some(measured),
                child: stateful.into_any_element(),
            };
        }

        BackdropLayer {
            radius: px(radius),
            blur: px(blur),
            material,
            lobes: LobeSource::Surface,
            translucent,
            measured: Some(measured),
            child: surface.into_any_element(),
        }
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
/// The fill does not fuse: each pane lays its own tint, and the neck between
/// two panes shows the bare bent backdrop. A group holds at most
/// [`MAX_GLASS_LOBES`] panes; panes past that keep their fill and their
/// content but fall outside the fused shape, so the bound is asserted in
/// debug rather than silently absorbed.
#[derive(IntoElement)]
pub struct GlassGroup {
    ident: Ident,
    surface: Surface,
    radius: Radius,
    blur: Option<f32>,
    preset: GlassPreset,
    merge: Option<f32>,
    gap: Option<f32>,
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
            .field("merge", &self.merge)
            .field("gap", &self.gap)
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
            merge: None,
            gap: None,
            panes: Vec::new(),
        }
    }

    /// Which surface colour each pane lays over the blur.
    pub fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    /// The rounding of each pane's lobe and fill.
    pub fn radius(mut self, radius: Radius) -> Self {
        self.radius = radius;
        self
    }

    /// How far the backdrop is blurred, overriding `effect.glassBlur`.
    pub fn blur(mut self, blur: f32) -> Self {
        self.blur = Some(blur.max(0.0));
        self
    }

    /// Which combination of optics the fused surface asks for.
    pub fn preset(mut self, preset: GlassPreset) -> Self {
        self.preset = preset;
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

    /// One pane of the body: a lobe of the shape, a fill, and the caller's
    /// content.
    pub fn pane(mut self, ident: impl Into<Ident>, child: impl IntoElement) -> Self {
        self.panes.push((ident.into(), child.into_any_element()));
        self
    }
}

impl RenderOnce for GlassGroup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        debug_assert!(
            self.panes.len() <= MAX_GLASS_LOBES,
            "a glass group holds at most {MAX_GLASS_LOBES} panes"
        );
        let theme = cx.theme().clone();
        let radius = theme.radius(self.radius);
        let alpha = self.preset.tint_alpha(&theme).clamp(0.0, 1.0);
        let fill = theme.surface(self.surface).opacity(alpha);
        let blur = self.blur.unwrap_or(theme.effects.glass_blur);
        let mut material = self.preset.material(&theme);
        material.smoothing = px(self.merge.unwrap_or(theme.effects.glass_merge_distance));
        let translucent = shows_a_backdrop(alpha, blur);
        let collected: Rc<RefCell<Vec<GlassLobe<Pixels>>>> = Rc::default();

        let row = div()
            .flex()
            .flex_row()
            .gap(px(self.gap.unwrap_or(theme.space(Space::Sm))))
            .children(self.panes.into_iter().map(|(ident, child)| {
                let pane = div()
                    .rounded(px(radius))
                    .bg(fill)
                    .child(child)
                    .semantic_in(cx, NodeSpec::new(ident.semantic_id(), Role::Region));
                Lobe {
                    radius: px(radius),
                    collected: Rc::clone(&collected),
                    child: pane.into_any_element(),
                }
            }))
            .semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::Group));

        BackdropLayer {
            radius: px(radius),
            blur: px(blur),
            material,
            lobes: LobeSource::Collected(collected),
            translucent,
            measured: None,
            child: row.into_any_element(),
        }
    }
}

/// Records where one pane landed, as a lobe of the group's shape.
///
/// The wrapper takes its child's layout, so the bounds it sees are the
/// pane's own; prepaint runs for every pane before the group paints, which
/// is what lets the group's single backdrop know all its lobes.
struct Lobe {
    radius: Pixels,
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
        self.collected.borrow_mut().push(GlassLobe {
            bounds,
            corner_radii: Corners::all(self.radius),
        });
        self.child.prepaint(window, cx);
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

/// Whether an adaptive surface's tint is deepened, given where the backdrop's
/// luminance sits against the flip band.
///
/// A dark fill dissolves over a bright backdrop and a light fill over a dark
/// one, so the deepening side depends on the fill. Inside the band the
/// previous answer stands: the gap is the hysteresis that stops a backdrop
/// sitting on one threshold from flipping the surface every frame.
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

/// Whether there is anything for a backdrop to show. Blurring what a fully
/// opaque fill is about to cover costs a render pass and changes no pixel, and
/// a radius of zero is a caller saying not to blur at all.
///
/// The optics ride on the same decision: they act on the blurred backdrop, so
/// where there is no backdrop there is nothing for them to act on either.
fn shows_a_backdrop(alpha: f32, blur: f32) -> bool {
    alpha < 1.0 && blur > 0.0
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
    pub(crate) blur: Pixels,
    pub(crate) material: GlassMaterial<Pixels>,
    pub(crate) lobes: LobeSource,
    pub(crate) translucent: bool,
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
        self.child.prepaint(window, cx);
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
        if !self.translucent {
            self.child.paint(window, cx);
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
        window.paint_layer(bounds, |window| {
            window.paint_backdrop_glass(
                bounds,
                Corners::all(self.radius),
                self.blur,
                self.material,
                lobes,
            );
            self.child.paint(window, cx);
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
mod tests {
    use super::*;

    #[test]
    fn an_opaque_theme_shows_no_backdrop() {
        assert!(shows_a_backdrop(0.72, 24.0));
        assert!(
            !shows_a_backdrop(1.0, 24.0),
            "an opaque fill hides what it blurred"
        );
        assert!(!shows_a_backdrop(0.72, 0.0), "no radius is no blur");
    }

    #[test]
    fn the_frosted_preset_asks_for_no_optics() {
        let theme = Theme::studio_dark();
        assert_eq!(
            GlassPreset::Frosted.material(&theme),
            GlassMaterial::frosted()
        );
        assert!(GlassPreset::Frosted.material(&theme).is_flat());
    }

    #[test]
    fn the_lens_preset_bends_without_colouring_or_lighting() {
        let theme = Theme::studio_dark();
        let material = GlassPreset::Lens.material(&theme);

        assert!(material.bends_light());
        assert_eq!(material.dispersion, 0.0, "a lens does not fringe text");
        assert_eq!(material.specular, 0.0, "a lens carries no highlight");
    }

    #[test]
    fn the_liquid_preset_takes_every_optic_from_tokens() {
        let theme = Theme::studio_dark();
        let material = GlassPreset::Liquid.material(&theme);

        assert_eq!(material.bevel, px(theme.effects.glass_bevel));
        assert_eq!(material.refraction, theme.effects.glass_refraction);
        assert_eq!(material.dispersion, theme.effects.glass_dispersion);
        assert_eq!(material.specular, theme.effects.glass_specular);
        assert_eq!(material.light_angle, theme.effects.glass_light_angle);
        assert!(!material.is_flat());
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
            material.bevel,
            px(theme.effects.glass_bevel),
            "what the caller did not override stays with the theme"
        );
    }

    fn surface_bounds() -> Bounds<Pixels> {
        Bounds {
            origin: gpui::point(px(100.), px(100.)),
            size: gpui::size(px(200.), px(100.)),
        }
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
    fn probe_slots_are_claimed_once_and_freed_on_drop() {
        let mut first = ProbeLease::default();
        let slot = first.slot().expect("a slot is free");
        assert_eq!(first.slot(), Some(slot), "a lease keeps its slot");

        let mut second = ProbeLease::default();
        let other = second.slot().expect("a second slot is free");
        assert_ne!(slot, other, "two leases never share a slot");

        drop(first);
        // Another test thread may have claimed slots in between, so the
        // reclaim assertion is that a slot is claimable and it is not the one
        // still leased, not that it is numerically the freed one.
        let mut third = ProbeLease::default();
        let reclaimed = third.slot().expect("a freed slot is claimable again");
        assert_ne!(reclaimed, other, "a live lease's slot stays claimed");
    }

    #[test]
    fn an_override_can_take_the_optics_off_a_lit_preset() {
        let theme = Theme::studio_dark();
        let material = Glass::new("surface")
            .preset(GlassPreset::Liquid)
            .refraction(0.0)
            .specular(0.0)
            .material(&theme);

        assert!(material.is_flat(), "a caller may ask for a plain pane");
    }
}
