//! Layout-independent, positive uniform visual transforms.
use crate::{Bounds, Pixels, Point, ScaledPixels, TransformationMatrix, point};

/// A positive uniform scale and translation from layout/window coordinates to
/// displayed window coordinates. Events remain displayed window coordinates.
/// Capture this value during prepaint for later event handlers and semantic
/// measurement; do not read the current scope from an asynchronous handler.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisualTransform {
    scale: f32,
    offset: Point<Pixels>,
}

impl Eq for VisualTransform {}

impl Default for VisualTransform {
    fn default() -> Self {
        Self {
            scale: 1.,
            offset: Point::default(),
        }
    }
}

impl VisualTransform {
    /// Scale about an origin in the enclosing scope's untransformed window
    /// coordinates. Zero, negative and nonfinite scales/origins are refused.
    pub fn scale_about(scale: f32, origin: Point<Pixels>) -> Self {
        Self::checked(scale, origin * (1. - scale))
    }

    fn checked(scale: f32, offset: Point<Pixels>) -> Self {
        assert!(
            scale.is_finite() && scale > 0. && offset.x.0.is_finite() && offset.y.0.is_finite(),
            "visual scale must be positive and finite with finite translation"
        );
        Self { scale, offset }
    }

    /// Apply `inner` first, then this transform. Nested origins are therefore
    /// expressed in the same unscaled layout coordinates as descendant bounds.
    pub fn compose(self, inner: Self) -> Self {
        Self::checked(self.scale * inner.scale, self.map_point(inner.offset))
    }

    /// Effective uniform raster/length multiplier, excluding display DPI.
    pub fn scale(self) -> f32 {
        self.scale
    }

    /// Map a layout point to displayed window coordinates.
    pub fn map_point(self, point: Point<Pixels>) -> Point<Pixels> {
        point * self.scale + self.offset
    }

    /// Map a window-global event position back to this scope's layout coordinates.
    pub fn unmap_point(self, point: Point<Pixels>) -> Point<Pixels> {
        (point - self.offset) / self.scale
    }

    /// Displayed bounds for semantic measurement and accessibility. Layout
    /// callbacks still receive original bounds; this never requests new layout.
    pub fn map_bounds(self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        Bounds::new(
            self.map_point(bounds.origin),
            bounds.size.map(|length| length * self.scale),
        )
    }

    /// Convert an enclosing displayed rectangle into this scope's coordinates.
    pub fn unmap_bounds(self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        Bounds::new(self.unmap_point(bounds.origin), bounds.size / self.scale)
    }

    pub(crate) fn matrix(self, device_scale: f32) -> TransformationMatrix {
        TransformationMatrix {
            rotation_scale: [[self.scale, 0.], [0., self.scale]],
            translation: [
                self.offset.x.0 * device_scale,
                self.offset.y.0 * device_scale,
            ],
        }
    }
}

/// Uniform transforms are baked into primitives, not layered offscreen. This
/// keeps backdrop capture in the original ordered target and needs no new GPU ABI.
pub(crate) fn transform_primitive(primitive: &mut crate::Primitive, t: TransformationMatrix) {
    use crate::Primitive;
    if t == TransformationMatrix::unit() {
        return;
    }
    let s = t.rotation_scale[0][0];
    let bounds = |b| t.transform_bounds(b);
    let pos = |p: Point<ScaledPixels>| {
        point(
            ScaledPixels(p.x.0 * s + t.translation[0]),
            ScaledPixels(p.y.0 * s + t.translation[1]),
        )
    };
    match primitive {
        Primitive::Quad(p) => {
            p.bounds = bounds(p.bounds);
            p.content_mask.bounds = bounds(p.content_mask.bounds);
            p.corner_radii = p.corner_radii.map(|r| *r * s);
            p.border_widths = p.border_widths.map(|r| *r * s);
        }
        Primitive::Shadow(p) => {
            p.bounds = bounds(p.bounds);
            p.element_bounds = bounds(p.element_bounds);
            p.content_mask.bounds = bounds(p.content_mask.bounds);
            p.blur_radius *= s;
            p.corner_radii = p.corner_radii.map(|r| *r * s);
            p.element_corner_radii = p.element_corner_radii.map(|r| *r * s);
        }
        Primitive::Underline(p) => {
            p.bounds = bounds(p.bounds);
            p.content_mask.bounds = bounds(p.content_mask.bounds);
            p.thickness *= s;
        }
        Primitive::Path(p) => {
            p.bounds = bounds(p.bounds);
            p.content_mask.bounds = bounds(p.content_mask.bounds);
            for vertex in &mut p.vertices {
                vertex.xy_position = pos(vertex.xy_position);
            }
        }
        Primitive::MonochromeSprite(p) => {
            p.transformation = t.compose(p.transformation);
            p.content_mask.bounds = bounds(p.content_mask.bounds);
        }
        Primitive::SubpixelSprite(p) => {
            p.transformation = t.compose(p.transformation);
            p.content_mask.bounds = bounds(p.content_mask.bounds);
        }
        Primitive::PolychromeSprite(p) => {
            p.transformation = t.compose(p.transformation);
            p.content_mask.bounds = bounds(p.content_mask.bounds);
        }
        Primitive::Surface(p) => {
            p.bounds = bounds(p.bounds);
            p.content_mask.bounds = bounds(p.content_mask.bounds);
        }
    }
}

pub(crate) fn transform_glass(p: &mut crate::BackdropGlass, t: TransformationMatrix) {
    if t == TransformationMatrix::unit() {
        return;
    }
    let s = t.rotation_scale[0][0];
    p.bounds = t.transform_bounds(p.bounds);
    p.content_mask.bounds = t.transform_bounds(p.content_mask.bounds);
    p.corner_radii = p.corner_radii.map(|r| *r * s);
    for lobe in p.lobes.iter_mut().take(p.lobe_count as usize) {
        lobe.bounds = t.transform_bounds(lobe.bounds);
        lobe.corner_radii = lobe.corner_radii.map(|r| *r * s);
    }
    p.material.blur_radius *= s;
    p.material.bevel *= s;
    p.material.thickness *= s;
    p.material.backdrop_depth *= s;
    p.material.hairline *= s;
    p.material.smoothing *= s;
    p.material.edge_mask_band *= s;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContentMask, Corners, RoundedClip, prelude::*, px, size};

    #[test]
    fn nested_origins_inverse_and_lengths_are_asymmetric() {
        let outer = VisualTransform::scale_about(2., point(px(10.), px(20.)));
        let inner = VisualTransform::scale_about(0.75, point(px(30.), px(10.)));
        let t = outer.compose(inner);
        let bounds = Bounds::new(point(px(10.), px(20.)), size(px(40.), px(30.)));
        assert_eq!(
            t.map_bounds(bounds),
            Bounds::new(point(px(20.), px(15.)), size(px(60.), px(45.)))
        );
        assert_eq!(t.unmap_bounds(t.map_bounds(bounds)), bounds);
        assert_eq!(
            t.unmap_point(point(px(35.), px(30.))),
            point(px(20.), px(30.))
        );
        assert_ne!(t, inner.compose(outer));
        assert_eq!(t.matrix(2.).translation, [10., -30.]);
        for invalid in [0., -1., f32::NAN, f32::INFINITY] {
            assert!(
                std::panic::catch_unwind(|| VisualTransform::scale_about(
                    invalid,
                    Point::default()
                ))
                .is_err()
            );
        }
    }

    struct Scaled {
        factor: f32,
        child: crate::AnyElement,
    }
    impl IntoElement for Scaled {
        type Element = Self;
        fn into_element(self) -> Self {
            self
        }
    }
    impl crate::Element for Scaled {
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
            w: &mut crate::Window,
            cx: &mut crate::App,
        ) -> (crate::LayoutId, ()) {
            (self.child.request_layout(w, cx), ())
        }
        fn prepaint(
            &mut self,
            _: Option<&crate::GlobalElementId>,
            _: Option<&crate::InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut (),
            w: &mut crate::Window,
            cx: &mut crate::App,
        ) {
            w.with_content_mask(
                Some(ContentMask {
                    bounds: Bounds::new(point(px(0.), px(0.)), size(px(90.), px(70.))),
                }),
                |w| {
                    w.with_visual_scale(self.factor, point(px(-10.), px(-5.)), |w| {
                        w.with_rounded_content_mask(
                            RoundedClip::new(
                                Bounds::new(Point::default(), size(px(60.), px(40.))),
                                Corners {
                                    top_left: px(20.),
                                    ..Corners::default()
                                },
                            ),
                            |w| self.child.prepaint(w, cx),
                        );
                    });
                },
            );
        }
        fn paint(
            &mut self,
            _: Option<&crate::GlobalElementId>,
            _: Option<&crate::InspectorElementId>,
            _: Bounds<Pixels>,
            _: &mut (),
            _: &mut (),
            w: &mut crate::Window,
            cx: &mut crate::App,
        ) {
            w.with_content_mask(
                Some(ContentMask {
                    bounds: Bounds::new(point(px(0.), px(0.)), size(px(90.), px(70.))),
                }),
                |w| {
                    w.with_visual_scale(self.factor, point(px(-10.), px(-5.)), |w| {
                        w.with_rounded_content_mask(
                            RoundedClip::new(
                                Bounds::new(Point::default(), size(px(60.), px(40.))),
                                Corners {
                                    top_left: px(20.),
                                    ..Corners::default()
                                },
                            ),
                            |w| self.child.paint(w, cx),
                        );
                    });
                },
            );
        }
    }

    struct Child(std::rc::Rc<std::cell::Cell<usize>>);
    impl crate::Render for Child {
        fn render(
            &mut self,
            _: &mut crate::Window,
            _: &mut crate::Context<Self>,
        ) -> impl IntoElement {
            self.0.set(self.0.get() + 1);
            crate::div()
                .id("scaled-child")
                .role(crate::Role::Group)
                .size_full()
                .bg(crate::white())
                .on_mouse_down(crate::MouseButton::Left, |_, _, _| {})
                .child(
                    crate::deferred(
                        crate::div()
                            .id("scaled-deferred")
                            .size(px(20.))
                            .bg(crate::red())
                            .on_mouse_down(crate::MouseButton::Left, |_, _, _| {}),
                    )
                    .preserve_accessibility(),
                )
        }
    }
    struct Root {
        factor: f32,
        child: crate::Entity<Child>,
    }
    impl crate::Render for Root {
        fn render(
            &mut self,
            _: &mut crate::Window,
            _: &mut crate::Context<Self>,
        ) -> impl IntoElement {
            Scaled {
                factor: self.factor,
                child: self
                    .child
                    .clone()
                    .cached(crate::StyleRefinement::default().size_full())
                    .into_any_element(),
            }
        }
    }

    #[gpui::test]
    fn scale_preserves_layout_deferred_cache_clips_and_accessibility(
        cx: &mut crate::TestAppContext,
    ) {
        use crate::AppContext;
        let renders = std::rc::Rc::new(std::cell::Cell::new(0));
        let handle = cx.open_window(size(px(100.), px(80.)), |_, cx| Root {
            factor: 1.5,
            child: cx.new(|_| Child(renders.clone())),
        });
        cx.run_until_parked();
        let verify = |w: &crate::Window| {
            let scene = &w.rendered_frame.scene;
            assert_eq!(scene.quads.len(), 2);
            let dpi = w.scale_factor();
            assert_eq!(
                scene.quads[0].bounds,
                Bounds::new(point(px(5.), px(2.5)), size(px(150.), px(120.))).scale(dpi)
            );
            assert_eq!(
                scene.quads[1].bounds.size,
                size(px(30.), px(30.)).scale(dpi)
            );
            assert_eq!(
                scene.quads[0].content_mask.bounds,
                Bounds::new(Point::default(), size(px(90.), px(70.))).scale(dpi)
            );
            assert!(
                w.rendered_frame
                    .hit_test(point(px(6.), px(3.)))
                    .ids
                    .is_empty(),
                "scaled rounded corner rejects"
            );
            assert!(
                !w.rendered_frame
                    .hit_test(point(px(30.), px(25.)))
                    .ids
                    .is_empty()
            );
            assert!(
                w.rendered_frame
                    .hit_test(point(px(92.), px(35.)))
                    .ids
                    .is_empty(),
                "outer rectangular clip stays fixed"
            );
            let hitbox = &w.rendered_frame.hitboxes[0];
            assert_eq!(
                hitbox.bounds.size,
                size(px(100.), px(80.)),
                "layout unchanged"
            );
            assert_eq!(
                hitbox
                    .visual_transform
                    .unmap_point(point(px(35.), px(32.5))),
                point(px(20.), px(20.))
            );
        };
        handle
            .update(cx, |_, w, _| verify(w))
            .expect("initial scaled frame");
        let count = renders.get();
        handle
            .update(cx, |_, _, cx| cx.notify())
            .expect("redraw root");
        cx.run_until_parked();
        handle
            .update(cx, |_, w, _| verify(w))
            .expect("replayed scaled frame");
        assert_eq!(
            renders.get(),
            count,
            "retained child reused without double scale"
        );
        handle
            .update(cx, |v, _, cx| {
                v.factor = 1.;
                cx.notify();
            })
            .expect("release scale");
        cx.run_until_parked();
        assert!(renders.get() > count, "scale-only change invalidates cache");
        handle
            .update(cx, |_, w, _| {
                assert_eq!(
                    w.rendered_frame.scene.quads[0].bounds.size,
                    size(px(100.), px(80.)).scale(w.scale_factor())
                )
            })
            .expect("released geometry");
        handle
            .update(cx, |v, _, cx| {
                v.factor = 1.5;
                cx.notify();
            })
            .expect("restore scale");
        cx.activate_accessibility(handle.into());
        cx.run_until_parked();
        handle
            .update(cx, |_, w, _| {
                assert!(
                    w.a11y
                        .node_bounds
                        .values()
                        .any(|b| *b == Bounds::new(point(px(5.), px(2.5)), size(px(85.), px(60.))))
                );
            })
            .expect("displayed accessibility bounds");
    }
}
