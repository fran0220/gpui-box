//! Frozen GPUI paint, deliberately without element or interaction ownership.
use crate::{Scene, WindowId};
use std::{rc::Rc, sync::Arc};

/// A paint-phase position, usable only in the same window and scene epoch.
/// Take this immediately before painting the live subtree.
pub struct PaintMark {
    pub(crate) epoch: Arc<()>,
    pub(crate) start: usize,
    pub(crate) window: WindowId,
    pub(crate) scale: f32,
    pub(crate) platform_views: usize,
    pub(crate) resource_revision: u64,
    pub(crate) transform: crate::VisualTransform,
    pub(crate) opacity: f32,
    pub(crate) layers: Vec<crate::Bounds<crate::ScaledPixels>>,
}

/// Immutable last-live paint operations, not an element and not a screenshot.
///
/// Supports quads/gradients, paths, shadows, underlines and leased text/image
/// sprites. Replay owns no input, focus, IME, accessibility or semantic targets.
/// Capture-time visible clipping is part of the picture: moving it never reveals
/// pixels clipped or culled during capture. Replay maps that picture from the
/// captured ambient transform/opacity into the current scope, then intersects
/// current ancestor clipping. Subtree-local transforms/opacity remain frozen.
/// Glass, surfaces, hosted views and deferred overlays refuse capture; frozen
/// composited snapshots remain a separate framework gap.
///
/// Clones share storage. Callers must bound the number/lifetime of recordings
/// and drop them on retirement completion, reinsertion and reduced motion. The
/// submitted frame retains its resources until that frame is cleared.
#[derive(Clone)]
pub struct PaintRecording {
    pub(crate) scene: Rc<Scene>,
    pub(crate) window: WindowId,
    pub(crate) scale: f32,
    pub(crate) transform: crate::VisualTransform,
    pub(crate) opacity: f32,
}

/// Capture/replay refusal. No partial recording or partially replayed subtree
/// is returned. Callers should immediately retire unsupported content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PaintRecordingError {
    /// The mark is from another paint frame or does not name a valid range.
    #[error("paint mark is stale")]
    StaleMark,
    /// Paint cannot cross windows.
    #[error("paint recording belongs to another window")]
    WindowMismatch,
    /// Raster resources cannot cross display scale factors.
    #[error("paint recording display scale changed")]
    ScaleMismatch,
    /// Device loss or atlas reset invalidated owned resources.
    #[error("paint recording resources were reset")]
    ResourceReset,
    /// The atlas cannot retain these allocations, or they have been evicted.
    #[error("atlas cannot retain the recorded tiles")]
    AtlasUnsupported,
    /// Glass requires a frozen composited backdrop, not live-backdrop replay.
    #[error("backdrop glass requires composited snapshot capture")]
    BackdropGlass,
    /// Platform pixel buffers may be externally mutable.
    #[error("platform surfaces cannot be frozen as paint operations")]
    Surface,
    /// Hosted platform views are not GPUI paint operations.
    #[error("hosted platform views cannot be recorded")]
    PlatformView,
    /// Deferred elements paint outside the contiguous subtree paint range.
    #[error("frames containing deferred draws cannot be recorded")]
    DeferredDraw,
    /// A range cannot close inherited layers or leave newly opened layers open.
    #[error("paint recording crosses a layer boundary")]
    UnbalancedLayers,
    /// Edge fade is a position-dependent policy, not a uniform replay opacity.
    #[error("edge-fade replay requires an explicit composited snapshot")]
    EdgeFade,
    /// Zero/nonfinite ambient opacity cannot be normalized for future scopes.
    #[error("capture needs finite positive ambient opacity")]
    InvisibleCapture,
    /// Replay translations must be finite.
    #[error("paint recording offset must be finite")]
    InvalidOffset,
    /// The normalized transform or opacity exceeded finite renderer values.
    #[error("paint recording replay scope is not finite")]
    InvalidReplayScope,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        App, AtlasLeaseRegistry, AvailableSpace, BackdropGlass, Bounds, ClipChain, ContentMask,
        Corners, FocusHandle, InputHandler, MouseButton, MouseDownEvent, MouseUpEvent, Pixels,
        PlatformAtlas, PlatformInput, PlatformWindow, Point, Quad, Role, RoundedClip, ScaledPixels,
        TestAppContext, TransformationMatrix, UTF16Selection, Window, canvas, div, hsla, point,
        prelude::*, px, rgb, size,
    };
    use std::cell::{Cell, RefCell};

    fn quad() -> Quad {
        let bounds = Bounds::new(
            point(ScaledPixels(7.), ScaledPixels(11.)),
            size(ScaledPixels(41.), ScaledPixels(23.)),
        );
        Quad {
            bounds,
            content_mask: ContentMask { bounds },
            background: hsla(0.3, 0.7, 0.6, 0.8).into(),
            ..Default::default()
        }
    }

    #[test]
    fn frozen_paint_owns_geometry_and_composes_clip_transform_opacity() {
        let mut live = Scene::default();
        let clip = RoundedClip::new(
            Bounds::new(point(px(3.), px(5.)), size(px(80.), px(60.))),
            Corners::all(px(7.)),
        );
        let mut chain = ClipChain::default();
        chain.push(clip);
        live.with_clip_chain(&chain, 1., |scene| scene.insert_primitive(quad()));
        let frozen = live
            .freeze_paint(0..live.len(), &[], Arc::new(crate::TestAtlas::new()))
            .expect("quad capture");
        live.clear();
        let mut out = Scene::default();
        let parent = RoundedClip::new(
            Bounds::new(point(px(20.), px(1.)), size(px(200.), px(100.))),
            Corners::all(px(3.)),
        );
        let mut chain = ClipChain::default();
        chain.push(parent);
        out.with_clip_chain(&chain, 1., |scene| {
            scene.replace_visual_transform(TransformationMatrix {
                rotation_scale: [[2., 0.], [0., 2.]],
                translation: [13., -4.],
            });
            let mask = ContentMask {
                bounds: Bounds::new(
                    point(ScaledPixels(31.), ScaledPixels(0.)),
                    size(ScaledPixels(51.), ScaledPixels(80.)),
                ),
            };
            scene
                .replay_frozen(&frozen, mask, 0.25)
                .expect("clipped replay");
        });
        assert_eq!(
            out.quads[0].bounds,
            Bounds::new(
                point(ScaledPixels(27.), ScaledPixels(18.)),
                size(ScaledPixels(82.), ScaledPixels(46.))
            )
        );
        assert_eq!(
            out.quads[0].content_mask.bounds,
            Bounds::new(
                point(ScaledPixels(31.), ScaledPixels(18.)),
                size(ScaledPixels(51.), ScaledPixels(46.))
            )
        );
        assert_eq!(out.quads[0].background.solid.a, 0.2);
        let nodes = out.clip_nodes.nodes();
        assert_eq!(nodes.len(), 2);
        assert_eq!(
            nodes[1].bounds.origin,
            point(ScaledPixels(19.), ScaledPixels(6.))
        );
        assert_eq!(nodes[1].corner_radii.top_left, ScaledPixels(14.));
        assert_eq!(nodes[1].parent.as_u32(), 1);
    }

    #[test]
    fn frozen_paint_preserves_nested_ordering_and_gradient_path_alpha_increase() {
        let mut live = Scene::default();
        live.push_layer(quad().bounds);
        let start = live.len();
        let layers = live.recording_layers();
        let gradient = crate::linear_gradient_stops(
            37.,
            [
                crate::linear_color_stop(hsla(0.1, 0.7, 0.6, 0.12), 0.),
                crate::linear_color_stop(hsla(0.6, 0.4, 0.3, 0.32), 1.),
            ],
        );
        let mut card = quad();
        card.background = gradient;
        live.insert_primitive(card);
        live.push_layer(quad().bounds);
        let mut path = crate::Path::new(point(px(9.), px(12.)));
        path.line_to(point(px(37.), px(16.)));
        path.line_to(point(px(13.), px(29.)));
        path.color = gradient;
        path.content_mask = ContentMask {
            bounds: Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.))),
        };
        live.insert_primitive(path.scale(1.));
        live.pop_layer();
        let frozen = live
            .freeze_paint(
                start..live.len(),
                &layers,
                Arc::new(crate::TestAtlas::new()),
            )
            .expect("nested balanced range");
        live.pop_layer();
        let mut out = Scene::default();
        out.replace_visual_transform(TransformationMatrix {
            rotation_scale: [[2., 0.], [0., 2.]],
            translation: [3., -5.],
        });
        out.replay_frozen(
            &frozen,
            ContentMask {
                bounds: Bounds::new(
                    point(ScaledPixels(0.), ScaledPixels(0.)),
                    size(ScaledPixels(200.), ScaledPixels(200.)),
                ),
            },
            1.5,
        )
        .expect("increasing ambient opacity");
        assert!(out.paths[0].order > out.quads[0].order);
        for background in [out.quads[0].background, out.paths[0].color] {
            assert!((background.colors[0].color.a - 0.18).abs() < 1e-6);
            assert!((background.colors[1].color.a - 0.48).abs() < 1e-6);
            assert_eq!(background.gradient_angle_or_pattern_height, 37.);
        }
        assert_eq!(
            out.paths[0].vertices[1].xy_position,
            point(ScaledPixels(77.), ScaledPixels(27.))
        );
    }

    #[test]
    fn frozen_paint_refuses_glass_layers_and_reset_without_partial_output() {
        let atlas: Arc<dyn PlatformAtlas> = Arc::new(crate::TestAtlas::new());
        let mut scene = Scene::default();
        scene.insert_primitive(quad());
        scene.push_layer(quad().bounds);
        assert!(matches!(
            scene.freeze_paint(0..scene.len(), &[], atlas.clone()),
            Err(PaintRecordingError::UnbalancedLayers)
        ));
        scene.pop_layer();
        assert!(matches!(
            scene.freeze_paint(2..scene.len(), &[], atlas.clone()),
            Err(PaintRecordingError::UnbalancedLayers)
        ));
        scene
            .paint_operations
            .push(crate::scene::PaintOperation::BackdropGlass {
                glass: BackdropGlass {
                    order: 0,
                    bounds: quad().bounds,
                    content_mask: quad().content_mask,
                    corner_radii: Default::default(),
                    material: Default::default(),
                    lobes: Default::default(),
                    lobe_count: 0,
                    clip_id: crate::ClipId::NONE,
                },
                fallback: None,
            });
        assert!(matches!(
            scene.freeze_paint(0..scene.len(), &[], atlas),
            Err(PaintRecordingError::BackdropGlass)
        ));

        let mut registry = AtlasLeaseRegistry::default();
        let lease = registry.pin(Vec::new(), |_, _| {});
        let mut frozen = Scene::default();
        frozen.insert_primitive(quad());
        frozen.paint_leases.push((0..1, lease));
        let mut out = Scene::default();
        out.replay_frozen(&frozen, quad().content_mask, 1.)
            .expect("valid lease replay");
        registry.invalidate();
        assert!(
            !out.paint_resources_valid(),
            "submission must reject a reset after replay"
        );
        out.clear();
        assert_eq!(
            out.replay_frozen(&frozen, quad().content_mask, 1.),
            Err(PaintRecordingError::ResourceReset)
        );
        assert!(out.is_empty());
    }

    #[test]
    fn frozen_paint_frame_leases_do_not_spread_into_unrelated_reuse() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let released = Arc::new([AtomicUsize::new(0), AtomicUsize::new(0)]);
        let mut registry = AtlasLeaseRegistry::default();
        let mut source = Scene::default();
        for index in 0..2 {
            source.insert_primitive(quad());
            let released = released.clone();
            source.paint_leases.push((
                index..index + 1,
                registry.pin(Vec::new(), move |_, _| {
                    released[index].fetch_add(1, Ordering::SeqCst);
                }),
            ));
        }
        let mut retained = Scene::default();
        retained.replay(0..2, &source);
        let mut unrelated = Scene::default();
        unrelated.replay(1..2, &retained);
        source.clear();
        assert_eq!(
            released[0].load(Ordering::SeqCst),
            0,
            "submitted frame owns its leases"
        );
        retained.clear();
        assert_eq!(
            released[0].load(Ordering::SeqCst),
            1,
            "other cached paint must not pin the retired range"
        );
        assert_eq!(released[1].load(Ordering::SeqCst), 0);
        unrelated.clear();
        assert_eq!(released[1].load(Ordering::SeqCst), 1);
    }

    struct Input;
    impl InputHandler for Input {
        fn selected_text_range(
            &mut self,
            _: bool,
            _: &mut Window,
            _: &mut App,
        ) -> Option<UTF16Selection> {
            None
        }
        fn marked_text_range(
            &mut self,
            _: &mut Window,
            _: &mut App,
        ) -> Option<std::ops::Range<usize>> {
            None
        }
        fn text_for_range(
            &mut self,
            _: std::ops::Range<usize>,
            _: &mut Option<std::ops::Range<usize>>,
            _: &mut Window,
            _: &mut App,
        ) -> Option<String> {
            None
        }
        fn replace_text_in_range(
            &mut self,
            _: Option<std::ops::Range<usize>>,
            _: &str,
            _: &mut Window,
            _: &mut App,
        ) {
        }
        fn replace_and_mark_text_in_range(
            &mut self,
            _: Option<std::ops::Range<usize>>,
            _: &str,
            _: Option<std::ops::Range<usize>>,
            _: &mut Window,
            _: &mut App,
        ) {
        }
        fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}
        fn bounds_for_range(
            &mut self,
            _: std::ops::Range<usize>,
            _: &mut Window,
            _: &mut App,
        ) -> Option<Bounds<Pixels>> {
            None
        }
        fn character_index_for_point(
            &mut self,
            _: Point<Pixels>,
            _: &mut Window,
            _: &mut App,
        ) -> Option<usize> {
            None
        }
    }

    struct Host {
        live: bool,
        recording: Rc<RefCell<Option<PaintRecording>>>,
        focus: FocusHandle,
        paints: Rc<Cell<usize>>,
        clicks: Rc<Cell<usize>>,
    }
    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let slot = self.recording.clone();
            let focus = self.focus.clone();
            let paints = self.paints.clone();
            let clicks = self.clicks.clone();
            let live = self.live;
            canvas(
                move |bounds, window, cx| {
                    if !live {
                        return None;
                    }
                    let child = div()
                        .id("retiring-input")
                        .size(px(70.))
                        .bg(rgb(0x229966))
                        .track_focus(&focus)
                        .role(Role::TextInput)
                        .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
                        .child(
                            canvas(
                                |_, _, _| (),
                                move |_, _, window, cx| {
                                    paints.set(paints.get() + 1);
                                    window.handle_input(&focus, Input, cx);
                                },
                            )
                            .size_full(),
                        );
                    let mut child = child.into_any_element();
                    child.prepaint_as_root(
                        bounds.origin,
                        bounds.size.map(AvailableSpace::Definite),
                        window,
                        cx,
                    );
                    Some(child)
                },
                move |_, child, window, cx| {
                    if let Some(mut child) = child {
                        let mark = window.paint_mark();
                        child.paint(window, cx);
                        *slot.borrow_mut() =
                            Some(window.record_paint_since(mark).expect("live child capture"));
                    } else if let Some(recording) = slot.borrow().as_ref() {
                        window
                            .paint_recording(recording)
                            .expect("retired child replay");
                    }
                },
            )
            .size_full()
        }
    }

    #[gpui::test]
    fn frozen_paint_mounted_removal_drops_live_registrations(cx: &mut TestAppContext) {
        let paints = Rc::new(Cell::new(0));
        let clicks = Rc::new(Cell::new(0));
        let slot = Rc::new(RefCell::new(None));
        let window = cx.add_window(|window, cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            Host {
                live: true,
                recording: slot.clone(),
                focus,
                paints: paints.clone(),
                clicks: clicks.clone(),
            }
        });
        cx.activate_accessibility(window.into());
        let draw = |cx: &mut TestAppContext| {
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
                .expect("draw mounted host");
        };
        draw(cx);
        let mut platform = cx.test_window(window.into());
        let handler = platform
            .take_input_handler()
            .expect("live IME registration");
        platform.set_input_handler(handler);
        let before = paints.get();
        cx.update_window(window.into(), |_, window, _| {
            assert!(
                window
                    .debug_a11y_tree_json()
                    .expect("activated accessibility tree")
                    .contains("retiring-input")
            );
        })
        .expect("inspect live tree");
        window
            .update(cx, |host, window, cx| {
                assert!(
                    window
                        .rendered_frame
                        .dispatch_tree
                        .focusable_node_id(host.focus.id)
                        .is_some()
                );
                host.live = false;
                cx.notify();
            })
            .expect("remove child");
        draw(cx);
        window
            .update(cx, |host, window, _| {
                assert!(
                    window
                        .rendered_frame
                        .dispatch_tree
                        .focusable_node_id(host.focus.id)
                        .is_none(),
                    "retired paint must not restore the focus dispatch target"
                );
            })
            .expect("inspect retired focus target");
        assert_eq!(
            paints.get(),
            before,
            "removal must not replay arbitrary paint callbacks"
        );
        assert!(cx.test_window(window.into()).take_input_handler().is_none());
        cx.update_window(window.into(), |_, window, cx| {
            assert!(
                !window
                    .debug_a11y_tree_json()
                    .expect("activated accessibility tree after removal")
                    .contains("retiring-input")
            );
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: point(px(25.), px(25.)),
                    ..Default::default()
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    button: MouseButton::Left,
                    position: point(px(25.), px(25.)),
                    ..Default::default()
                }),
                cx,
            );
        })
        .expect("dispatch against removed child");
        assert_eq!(clicks.get(), 0);
        window
            .update(cx, |host, _, cx| {
                host.live = true;
                host.recording.borrow_mut().take();
                cx.notify();
            })
            .expect("reinsert child");
        draw(cx);
        assert!(paints.get() > before);
        assert!(cx.test_window(window.into()).take_input_handler().is_some());
    }

    #[derive(Clone)]
    struct ContextProbe {
        recording: Rc<RefCell<Option<PaintRecording>>>,
        mark: Rc<RefCell<Option<PaintMark>>>,
        errors: Rc<RefCell<Vec<PaintRecordingError>>>,
    }
    impl Render for ContextProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let probe = self.clone();
            canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    if let Some(mark) = probe.mark.borrow_mut().take() {
                        probe.errors.borrow_mut().push(
                            window
                                .record_paint_since(mark)
                                .err()
                                .expect("old mark must fail"),
                        );
                    }
                    if let Some(recording) = probe.recording.borrow().as_ref() {
                        if let Err(error) = window.paint_recording(recording) {
                            probe.errors.borrow_mut().push(error);
                        } else {
                            let mut wrong_scale = recording.clone();
                            wrong_scale.scale *= 1.5;
                            probe.errors.borrow_mut().push(
                                window
                                    .paint_recording(&wrong_scale)
                                    .expect_err("wrong display scale must fail"),
                            );
                        }
                    }
                    let mark = window.paint_mark();
                    *probe.recording.borrow_mut() = Some(
                        window
                            .record_paint_since(mark)
                            .expect("context probe capture"),
                    );
                    *probe.mark.borrow_mut() = Some(window.paint_mark());
                },
            )
            .size_full()
        }
    }

    #[gpui::test]
    fn frozen_paint_marks_and_recordings_reject_other_frames_windows_and_scales(
        cx: &mut TestAppContext,
    ) {
        let probe = ContextProbe {
            recording: Default::default(),
            mark: Default::default(),
            errors: Default::default(),
        };
        let first = cx.add_window(|_, _| probe.clone());
        for _ in 0..2 {
            cx.update_window(first.into(), |_, window, cx| window.draw(cx).clear(cx))
                .expect("draw first context");
        }
        let second = cx.add_window(|_, _| probe.clone());
        cx.update_window(second.into(), |_, window, cx| window.draw(cx).clear(cx))
            .expect("draw other window");
        let errors = probe.errors.borrow();
        assert!(errors.contains(&PaintRecordingError::StaleMark));
        assert!(errors.contains(&PaintRecordingError::ScaleMismatch));
        assert!(errors.contains(&PaintRecordingError::WindowMismatch));
    }

    struct NormalizationProbe;
    impl Render for NormalizationProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            canvas(
                |_, _, _| (),
                |_, _, window, _| {
                    let recording =
                        window.with_visual_scale(1.5, point(px(10.), px(10.)), |window| {
                            window.with_element_opacity(Some(0.4), |window| {
                                let mark = window.paint_mark();
                                window.paint_quad(crate::fill(
                                    Bounds::new(point(px(20.), px(30.)), size(px(40.), px(24.))),
                                    hsla(0.3, 0.7, 0.6, 0.8),
                                ));
                                window.record_paint_since(mark).expect("ambient capture")
                            })
                        });
                    let dpi = window.scale_factor();
                    window.with_visual_scale(2., point(px(5.), px(7.)), |window| {
                        window.with_element_opacity(Some(0.6), |window| {
                            window.with_content_mask(
                                Some(ContentMask {
                                    bounds: Bounds::new(
                                        point(px(27.), px(25.)),
                                        size(px(8.), px(12.)),
                                    ),
                                }),
                                |window| {
                                    let mark = window.paint_mark();
                                    window
                                        .paint_recording_with_offset(
                                            &recording,
                                            point(px(9.), px(-3.)),
                                        )
                                        .expect("offset replay");
                                    let output = window
                                        .record_paint_since(mark)
                                        .expect("inspect normalized replay");
                                    let quad = &output.scene.quads[0];
                                    for (bounds, expected) in [
                                        (quad.bounds, [53., 47., 80., 48.]),
                                        (quad.content_mask.bounds, [49., 43., 16., 24.]),
                                        (
                                            quad.bounds.intersect(&quad.content_mask.bounds),
                                            [53., 47., 12., 20.],
                                        ),
                                    ] {
                                        let actual = [
                                            bounds.origin.x.0 / dpi,
                                            bounds.origin.y.0 / dpi,
                                            bounds.size.width.0 / dpi,
                                            bounds.size.height.0 / dpi,
                                        ];
                                        for (actual, expected) in actual.into_iter().zip(expected) {
                                            assert!(
                                                (actual - expected).abs() < 1e-4,
                                                "{actual} != {expected}"
                                            );
                                        }
                                    }
                                    assert!(
                                        (quad.background.solid.a - 0.48).abs() < 1e-6,
                                        "ambient opacity replaces, not multiplies twice"
                                    );
                                },
                            );
                        });
                    });
                    window.with_element_opacity(Some(0.), |window| {
                        let mark = window.paint_mark();
                        assert!(matches!(
                            window.record_paint_since(mark),
                            Err(PaintRecordingError::InvisibleCapture)
                        ));
                    });
                    window.paint_layer(
                        Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.))),
                        |window| {
                            let mark = window.paint_mark();
                            window.paint_quad(crate::fill(
                                Bounds::new(point(px(2.), px(3.)), size(px(30.), px(20.))),
                                crate::red(),
                            ));
                            let recording = window
                                .record_paint_since(mark)
                                .expect("balanced range within inherited layer");
                            assert_eq!(recording.scene.quads.len(), 1);
                            assert_eq!(recording.scene.len(), 3);
                        },
                    );
                },
            )
            .size_full()
        }
    }

    #[gpui::test]
    fn frozen_paint_normalizes_ambient_scope_and_applies_local_offset_once(
        cx: &mut TestAppContext,
    ) {
        let window = cx.add_window(|_, _| NormalizationProbe);
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .expect("draw normalization probe");
    }
}
