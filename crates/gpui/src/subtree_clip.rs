//! Rounded subtree geometry, independent of rectangular culling and optical captures.

use crate::{Bounds, Corners, Pixels, Point, ScaledPixels};
use std::sync::Arc;

/// A rounded rectangle in window coordinates.
///
/// Radii are fitted exactly as for framework quads: each radius is nonnegative
/// and at most half the shortest side. This makes quadrant selection in the
/// rounded rectangle distance function unambiguous.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoundedClip {
    bounds: Bounds<Pixels>,
    corner_radii: Corners<Pixels>,
}

impl RoundedClip {
    /// Constructs finite geometry. Empty bounds represent an empty clip.
    ///
    /// Panics for nonfinite coordinates or radii rather than admitting an
    /// undefined CPU/GPU disagreement.
    pub fn new(bounds: Bounds<Pixels>, corner_radii: Corners<Pixels>) -> Self {
        assert!(
            [
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height
            ]
            .into_iter()
            .all(|value| f32::from(value).is_finite()),
            "rounded clip bounds must be finite"
        );
        let corner_radii = corner_radii.map(|radius| {
            assert!(
                f32::from(*radius).is_finite(),
                "rounded clip radius must be finite"
            );
            (*radius).max(Pixels::ZERO)
        });
        let corner_radii = if bounds.is_empty() {
            Corners::default()
        } else {
            corner_radii.clamp_radii_for_quad_size(bounds.size)
        };
        Self {
            bounds,
            corner_radii,
        }
    }

    /// The rectangle enclosing the rounded clip.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    /// Fitted corner radii in top-left, top-right, bottom-right, bottom-left order.
    pub fn corner_radii(&self) -> Corners<Pixels> {
        self.corner_radii
    }

    /// Tests the same rounded rectangle distance used by renderer shaders.
    pub fn contains(&self, position: Point<Pixels>) -> bool {
        !self.bounds.is_empty()
            && rounded_rect_distance(
                position.map(f32::from),
                self.bounds.map(f32::from),
                self.corner_radii.map(|radius| f32::from(*radius)),
            ) <= 0.
    }
}

/// Signed distance to a fitted rounded rectangle; negative inside.
///
/// Mirrors the renderers' `quad_sdf` including its square-corner fast path.
pub(crate) fn rounded_rect_distance(
    at: Point<f32>,
    bounds: Bounds<f32>,
    radii: Corners<f32>,
) -> f32 {
    rounded_rect_field(at, bounds, radii).0
}

/// Shared analytic rounded-rectangle distance and gradient used by clipping
/// and backdrop geometry. At medial axes choose an incident face consistently
/// with the renderer rather than a fictitious bisector.
pub(crate) fn rounded_rect_field(
    at: Point<f32>,
    bounds: Bounds<f32>,
    radii: Corners<f32>,
) -> (f32, Point<f32>) {
    let half_width = bounds.size.width / 2.;
    let half_height = bounds.size.height / 2.;
    let x = at.x - (bounds.origin.x + half_width);
    let y = at.y - (bounds.origin.y + half_height);
    let radius = if x < 0. {
        if y < 0. {
            radii.top_left
        } else {
            radii.bottom_left
        }
    } else if y < 0. {
        radii.top_right
    } else {
        radii.bottom_right
    };
    let corner_x = x.abs() - half_width + radius;
    let corner_y = y.abs() - half_height + radius;
    let outside = (corner_x.max(0.).powi(2) + corner_y.max(0.).powi(2)).sqrt();
    let mut gradient = if corner_x > corner_y {
        crate::point(1., 0.)
    } else {
        crate::point(0., 1.)
    };
    if radius != 0. && outside > 0. {
        gradient = crate::point(corner_x.max(0.) / outside, corner_y.max(0.) / outside);
    }
    gradient.x *= if x >= 0. { 1. } else { -1. };
    gradient.y *= if y >= 0. { 1. } else { -1. };
    let distance = if radius == 0. {
        corner_x.max(corner_y)
    } else {
        outside + corner_x.max(corner_y).min(0.) - radius
    };
    (distance, gradient)
}

/// Value-typed inherited clips. Cloning is cheap and equality compares geometry,
/// not allocation or scene-local indices, so this can be retained across frames.
///
/// Flat storage also makes traversal and destruction independent of nesting
/// depth; neither operation consumes a recursive call stack.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClipChain(Arc<Vec<RoundedClip>>);

impl ClipChain {
    /// Adds a clip to this value without changing clones retained elsewhere.
    pub fn push(&mut self, clip: RoundedClip) {
        Arc::make_mut(&mut self.0).push(clip);
    }

    /// Removes the innermost clip, returning it if present.
    pub fn pop(&mut self) -> Option<RoundedClip> {
        Arc::make_mut(&mut self.0).pop()
    }

    /// Ordered outermost to innermost.
    pub fn clips(&self) -> &[RoundedClip] {
        &self.0
    }

    /// True only if every ancestor admits this point. No depth limit applies.
    pub fn contains(&self, position: Point<Pixels>) -> bool {
        self.0.iter().all(|clip| clip.contains(position))
    }

    /// Conservative rectangular bounds for accessibility APIs such as AccessKit
    /// that cannot represent curved regions. Corner pixels may belong to this
    /// rectangle while being rejected by visual clipping and pointer hit tests.
    /// This is not an approximation used for pointer input or fragment coverage.
    pub fn accessible_bounds(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        self.0
            .iter()
            .fold(bounds, |bounds, clip| bounds.intersect(&clip.bounds))
    }
}

/// Scene-local linked clip index. Zero means no clip; nodes are one-based.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct ClipId {
    index: u32,
    padding: u32,
}

impl ClipId {
    /// The unclipped fast path, preserving existing framebuffer writes.
    pub const NONE: Self = Self {
        index: 0,
        padding: 0,
    };

    /// Integer used in GPU records. Nonzero indices are meaningful only in the
    /// scene that issued them.
    pub fn as_u32(self) -> u32 {
        self.index
    }
}

/// GPU-facing rounded clip record (ten 32-bit words, with no implicit padding).
/// Parent is always zero or less than this node's one-based index. Renderers can
/// walk until zero without a depth cap or cycle detection.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct ClipNode {
    /// Device-space bounds; never a replacement for a primitive's content mask.
    pub bounds: Bounds<ScaledPixels>,
    /// Fitted device-space corner radii.
    pub corner_radii: Corners<ScaledPixels>,
    /// Parent's one-based index, or zero.
    pub parent: ClipId,
}

/// Scene-owned clip storage. Only validated append operations can create nodes,
/// ensuring that GPU traversal always terminates.
#[derive(Default)]
pub struct ClipNodes(Vec<ClipNode>);

impl ClipNodes {
    /// GPU upload records, in parent-first order.
    pub fn nodes(&self) -> &[ClipNode] {
        &self.0
    }

    /// Clears frame-local indices along with the owning scene.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Encodes a CPU chain in device coordinates, returning its innermost node.
    /// Fails clearly on invalid scale or exhausting the 32-bit index space.
    pub fn insert(&mut self, chain: &ClipChain, scale: f32) -> ClipId {
        let mut parent = ClipId::NONE;
        for clip in chain.clips() {
            parent = self.push(*clip, scale, parent);
        }
        parent
    }

    pub(crate) fn push(&mut self, clip: RoundedClip, scale: f32, parent: ClipId) -> ClipId {
        assert!(
            scale.is_finite() && scale > 0.,
            "clip scale must be finite and positive"
        );
        self.append(ClipNode {
            bounds: clip.bounds.scale(scale),
            corner_radii: clip.corner_radii.scale(scale),
            parent,
        })
    }

    fn append(&mut self, node: ClipNode) -> ClipId {
        let index = u32::try_from(self.0.len())
            .ok()
            .and_then(|index| index.checked_add(1))
            .expect("scene rounded clip index space exhausted");
        assert!(
            node.parent.index < index,
            "clip parent must precede its child"
        );
        self.0.push(node);
        ClipId { index, padding: 0 }
    }

    /// Copies one old chain parent-first into this scene. It never treats an old
    /// index as a new index, even when destination nodes already occupy that slot.
    /// Callers replaying many primitives should memoize the returned old/new pair.
    pub fn replay(&mut self, old: ClipId, source: &Self) -> ClipId {
        self.replay_with_cache(old, source, &mut collections::FxHashMap::default())
    }

    pub(crate) fn replay_with_cache(
        &mut self,
        old: ClipId,
        source: &Self,
        remapped: &mut collections::FxHashMap<ClipId, ClipId>,
    ) -> ClipId {
        let mut pending = Vec::new();
        let mut cursor = old;
        let mut parent = ClipId::NONE;
        while cursor != ClipId::NONE {
            if let Some(mapped) = remapped.get(&cursor) {
                parent = *mapped;
                break;
            }
            let node = source
                .0
                .get(cursor.index as usize - 1)
                .expect("clip index does not belong to the replay source");
            pending.push((cursor, *node));
            cursor = node.parent;
        }
        for (old, mut node) in pending.into_iter().rev() {
            node.parent = parent;
            parent = self.append(node);
            remapped.insert(old, parent);
        }
        parent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{point, px, size};

    fn clip(x: f32, y: f32, width: f32, height: f32, radii: [f32; 4]) -> RoundedClip {
        RoundedClip::new(
            Bounds::new(point(px(x), px(y)), size(px(width), px(height))),
            Corners {
                top_left: px(radii[0]),
                top_right: px(radii[1]),
                bottom_right: px(radii[2]),
                bottom_left: px(radii[3]),
            },
        )
    }

    #[test]
    fn asymmetric_corners_fit_and_reject_only_their_own_corner() {
        let clip = clip(13., 29., 80., 40., [100., 0., 7., -3.]);
        assert_eq!(
            clip.corner_radii(),
            Corners {
                top_left: px(20.),
                top_right: px(0.),
                bottom_right: px(7.),
                bottom_left: px(0.),
            }
        );
        assert!(!clip.contains(point(px(14.), px(30.))));
        assert!(clip.contains(point(px(92.), px(30.))));
        assert!(!clip.contains(point(px(92.), px(68.))));
        assert!(clip.contains(point(px(14.), px(68.))));
        assert!(clip.contains(point(px(33.), px(29.))));
        assert!(clip.contains(point(px(19.), px(35.))));
        assert!(!clip.contains(point(px(18.), px(34.))));
    }

    #[test]
    fn ancestors_intersect_without_changing_accessibility_to_an_inscribed_rect() {
        let outer = clip(10., 20., 80., 60., [25.; 4]);
        let inner = clip(12., 22., 70., 50., [2.; 4]);
        let mut chain = ClipChain::default();
        chain.push(outer);
        let retained = chain.clone();
        chain.push(inner);
        assert!(!chain.contains(point(px(14.), px(24.))));
        assert!(inner.contains(point(px(14.), px(24.))));
        assert!(chain.contains(point(px(40.), px(40.))));
        assert!(!chain.contains(point(px(85.), px(50.))));
        assert!(retained.contains(point(px(85.), px(50.))));
        assert_eq!(chain.accessible_bounds(outer.bounds()), inner.bounds());
        chain.pop();
        assert_eq!(chain, retained);
        let mut independently_allocated = ClipChain::default();
        independently_allocated.push(outer);
        assert_eq!(retained, independently_allocated);
    }

    #[test]
    fn replay_remaps_every_ancestor_and_preserves_scale() {
        let mut chain = ClipChain::default();
        chain.push(clip(7., 11., 90., 50., [9., 3., 5., 1.]));
        chain.push(clip(15., 17., 40., 30., [6., 2., 4., 8.]));
        let mut source = ClipNodes::default();
        let old = source.insert(&chain, 1.5);
        let mut destination = ClipNodes::default();
        destination.insert(&chain, 2.);
        let new = destination.replay(old, &source);
        assert_eq!(new.as_u32(), 4);
        assert_eq!(destination.nodes()[2].parent, ClipId::NONE);
        assert_eq!(destination.nodes()[3].parent.as_u32(), 3);
        assert_eq!(destination.nodes()[2].bounds, source.nodes()[0].bounds);
        assert_eq!(
            destination.nodes()[3].corner_radii,
            source.nodes()[1].corner_radii
        );
        assert_eq!(destination.nodes()[2].bounds.origin.x.0, 10.5);
        assert_eq!(destination.nodes()[3].corner_radii.bottom_left.0, 12.);
        assert_eq!(destination.replay(ClipId::NONE, &source), ClipId::NONE);
    }

    #[test]
    fn deep_chains_never_truncate_or_recurse() {
        let mut chain = ClipChain::default();
        chain.push(clip(0., 0., 100., 100., [40.; 4]));
        for _ in 0..20_000 {
            chain.push(clip(0., 0., 100., 100., [0.; 4]));
        }
        assert!(!chain.contains(point(px(1.), px(1.))));
        assert!(chain.contains(point(px(50.), px(50.))));
        let mut source = ClipNodes::default();
        let old = source.insert(&chain, 1.);
        let mut destination = ClipNodes::default();
        let new = destination.replay(old, &source);
        assert_eq!(new.as_u32(), 20_001);
        assert_eq!(destination.nodes(), source.nodes());
        destination.clear();
        assert!(destination.nodes().is_empty());
        let mut remapped = collections::FxHashMap::default();
        destination.replay_with_cache(old, &source, &mut remapped);
        for node in source.nodes().iter().rev() {
            destination.replay_with_cache(node.parent, &source, &mut remapped);
        }
        assert_eq!(
            destination.nodes(),
            source.nodes(),
            "shared ancestors are remapped once, not quadratically duplicated"
        );
    }

    #[test]
    fn empty_clip_rejects_and_empty_chain_admits() {
        assert!(ClipChain::default().contains(point(px(-100.), px(700.))));
        assert!(!clip(2., 3., 0., 20., [3.; 4]).contains(point(px(2.), px(10.))));
        assert_eq!(std::mem::size_of::<ClipNode>(), 10 * 4);
        assert_eq!(std::mem::align_of::<ClipNode>(), 4);
    }

    #[test]
    #[should_panic(expected = "rounded clip radius must be finite")]
    fn nonfinite_geometry_fails_instead_of_disagreeing_with_gpu() {
        clip(0., 0., 30., 20., [f32::NAN, 0., 0., 0.]);
    }

    struct ScopedClip {
        clip: RoundedClip,
        child: crate::AnyElement,
    }

    impl crate::IntoElement for ScopedClip {
        type Element = Self;
        fn into_element(self) -> Self {
            self
        }
    }

    impl crate::Element for ScopedClip {
        type RequestLayoutState = ();
        type PrepaintState = ();
        fn id(&self) -> Option<crate::ElementId> {
            None
        }
        fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
            None
        }
        fn request_layout(
            &mut self,
            _: Option<&crate::GlobalElementId>,
            _: Option<&crate::InspectorElementId>,
            window: &mut crate::Window,
            cx: &mut crate::App,
        ) -> (crate::LayoutId, ()) {
            (self.child.request_layout(window, cx), ())
        }
        fn prepaint(
            &mut self,
            _: Option<&crate::GlobalElementId>,
            _: Option<&crate::InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut (),
            window: &mut crate::Window,
            cx: &mut crate::App,
        ) {
            window.with_rounded_content_mask(self.clip, |window| self.child.prepaint(window, cx));
        }
        fn paint(
            &mut self,
            _: Option<&crate::GlobalElementId>,
            _: Option<&crate::InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut (),
            _: &mut (),
            window: &mut crate::Window,
            cx: &mut crate::App,
        ) {
            window.with_rounded_content_mask(self.clip, |window| self.child.paint(window, cx));
        }
    }

    struct ChildView(std::rc::Rc<std::cell::Cell<usize>>);
    impl crate::Render for ChildView {
        fn render(
            &mut self,
            _: &mut crate::Window,
            _: &mut crate::Context<Self>,
        ) -> impl crate::IntoElement {
            use crate::prelude::*;
            self.0.set(self.0.get() + 1);
            crate::div()
                .id("rounded-target")
                .role(crate::Role::Group)
                .size_full()
                .bg(crate::white())
                .on_mouse_down(crate::MouseButton::Left, |_, _, _| {})
                .child(
                    crate::deferred(
                        crate::div()
                            .id("rounded-deferred")
                            .role(crate::Role::Button)
                            .size(px(25.))
                            .bg(crate::red())
                            .on_mouse_down(crate::MouseButton::Left, |_, _, _| {}),
                    )
                    .preserve_accessibility(),
                )
        }
    }

    struct RootView {
        radius: f32,
        child: crate::Entity<ChildView>,
    }
    impl crate::Render for RootView {
        fn render(
            &mut self,
            _: &mut crate::Window,
            _: &mut crate::Context<Self>,
        ) -> impl crate::IntoElement {
            use crate::prelude::*;
            ScopedClip {
                clip: clip(0., 0., 80., 60., [self.radius, 0., 5., 0.]),
                child: self
                    .child
                    .clone()
                    .cached(crate::StyleRefinement::default().size_full())
                    .into_any_element(),
            }
        }
    }

    #[gpui::test]
    fn window_cached_deferred_clips_and_accessibility_stay_in_agreement(
        cx: &mut crate::TestAppContext,
    ) {
        use crate::AppContext as _;
        let renders = std::rc::Rc::new(std::cell::Cell::new(0));
        let window = cx.open_window(size(px(100.), px(90.)), |_, cx| RootView {
            radius: 25.,
            child: cx.new(|_| ChildView(renders.clone())),
        });
        cx.run_until_parked();
        let assert_clipped = |window: &crate::Window| {
            assert!(
                window
                    .rendered_frame
                    .hit_test(point(px(1.), px(1.)))
                    .ids
                    .is_empty()
            );
            assert!(
                !window
                    .rendered_frame
                    .hit_test(point(px(20.), px(20.)))
                    .ids
                    .is_empty()
            );
            assert!(
                window
                    .rendered_frame
                    .hit_test(point(px(85.), px(40.)))
                    .ids
                    .is_empty()
            );
            let quads = &window.rendered_frame.scene.quads;
            assert!(quads.len() >= 2, "ordinary and deferred paint both survive");
            assert!(quads.iter().all(|quad| quad.clip_id != ClipId::NONE));
            for quad in quads {
                assert_eq!(
                    quad.content_mask.bounds.size,
                    size(px(100.), px(90.)).scale(window.scale_factor()),
                    "rounded clipping does not narrow rectangular culling or optical capture"
                );
                let node = window.rendered_frame.scene.clip_nodes.nodes()
                    [quad.clip_id.as_u32() as usize - 1];
                assert_eq!(
                    node.corner_radii.top_left,
                    px(25.).scale(window.scale_factor())
                );
            }
        };
        window
            .update(cx, |_, window, _| assert_clipped(window))
            .expect("initial frame is accessible");
        let initial = renders.get();
        for _ in 0..2 {
            window
                .update(cx, |_, _, cx| cx.notify())
                .expect("root redraw requested");
            cx.run_until_parked();
            window
                .update(cx, |_, window, _| assert_clipped(window))
                .expect("retained frame is accessible");
            assert_eq!(
                renders.get(),
                initial,
                "unchanged geometry reuses the retained child"
            );
        }
        window
            .update(cx, |view, _, cx| {
                view.radius = 0.;
                cx.notify();
            })
            .expect("radius updated");
        cx.run_until_parked();
        assert!(
            renders.get() > initial,
            "radius-only changes invalidate retained paint/prepaint"
        );
        window
            .update(cx, |_, window, _| {
                assert!(
                    !window
                        .rendered_frame
                        .hit_test(point(px(1.), px(1.)))
                        .ids
                        .is_empty()
                );
                assert!(window.rendered_frame.scene.quads.iter().all(|quad| {
                    window.rendered_frame.scene.clip_nodes.nodes()
                        [quad.clip_id.as_u32() as usize - 1]
                        .corner_radii
                        .top_left
                        == ScaledPixels(0.)
                }));
            })
            .expect("changed frame is accessible");
        // Accessibility intentionally refreshes every frame; check its conservative
        // rectangles after exercising retained rendering with accessibility off.
        cx.activate_accessibility(window.into());
        cx.run_until_parked();
        window
            .update(cx, |_, window, _| {
                assert!(
                    window.a11y.node_bounds.values().any(|bounds| *bounds
                        == Bounds::new(point(px(0.), px(0.)), size(px(80.), px(60.))))
                );
            })
            .expect("accessibility frame is accessible");
    }
}
