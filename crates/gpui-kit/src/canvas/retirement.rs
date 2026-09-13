//! Graph-owned visual retirement. A recording has no element, callbacks,
//! focus, input or semantic authority. Unsupported paint retires immediately.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::{Rc, Weak},
};

use gpui::{
    AnyElement, App, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    PaintMark, PaintRecording, ParentElement, Pixels, Point, SharedString, Styled, Window, canvas,
    div, point, px,
};
use web_time::Instant;

use super::{GraphSource, GraphViewport, source::SourceData};
use crate::motion::{MotionSpec, Transition};

#[derive(Clone)]
struct Saved {
    paint: PaintRecording,
    origin: Point<Pixels>,
    world: Point<f32>,
    zoom: f32,
}

struct Slot {
    paint: Rc<RefCell<Option<Saved>>>,
    exit: Option<Transition<f32>>,
    entrance: Option<Transition<f32>>,
    last: Instant,
    order: u64,
}

#[derive(Default)]
pub(super) struct Retirement {
    source: Option<Weak<RefCell<SourceData>>>,
    revision: Option<u64>,
    current: HashSet<SharedString>,
    added: HashSet<SharedString>,
    initialized: bool,
    slots: HashMap<SharedString, Slot>,
    order: u64,
}

impl Retirement {
    pub(super) fn sync(
        &mut self,
        ids: impl Iterator<Item = SharedString>,
        source: Option<&GraphSource>,
        enabled: bool,
        now: Instant,
        spec: MotionSpec,
    ) {
        if !enabled {
            *self = Self::default();
            return;
        }
        let identity = source.map(|source| Rc::downgrade(&source.data));
        let same = match (&identity, &self.source) {
            (Some(a), Some(b)) => Weak::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.slots.clear();
        }
        self.added.clear();
        let revision = source.map(GraphSource::revision);
        if !same || source.is_none() || revision != self.revision {
            let current: HashSet<_> = ids.collect();
            if same && self.initialized {
                self.added
                    .extend(current.difference(&self.current).cloned());
            }
            self.current = current;
        }
        self.initialized = true;
        self.source = identity;
        self.revision = revision;
        self.slots.retain(|id, slot| {
            if self.current.contains(id) {
                if let Some(entrance) = &mut slot.entrance {
                    entrance.advance(now.saturating_duration_since(slot.last));
                    if !entrance.is_animating() {
                        slot.entrance = None;
                    }
                }
                if slot.exit.take().is_some() {
                    // Reinserted identity has new live content, not the old
                    // incarnation's recording or interactive children.
                    slot.paint.borrow_mut().take();
                    slot.entrance = None;
                }
                slot.last = now;
                return true;
            }
            if slot.paint.borrow().is_none() {
                return false;
            }
            let fade = slot.exit.get_or_insert_with(|| {
                let mut fade = Transition::new(1., spec);
                fade.set(0.);
                slot.last = now;
                fade
            });
            fade.advance(now.saturating_duration_since(slot.last));
            slot.last = now;
            fade.is_animating()
        });
    }

    pub(super) fn keep_visible(&mut self, visible: &HashSet<SharedString>) {
        // Panning out is not removal, and retaining every visited card would
        // turn this into an unbounded screenshot cache.
        self.slots
            .retain(|id, slot| slot.exit.is_some() || visible.contains(id));
    }

    pub(super) fn opacity(
        &mut self,
        id: &SharedString,
        now: Instant,
        spec: MotionSpec,
        snap: bool,
        window: &mut Window,
    ) -> f32 {
        let added = self.added.remove(id);
        let slot = self.slots.entry(id.clone()).or_insert_with(|| Slot {
            paint: Rc::new(RefCell::new(None)),
            exit: None,
            entrance: None,
            last: now,
            order: 0,
        });
        if added {
            let mut entrance = Transition::new(0., spec);
            entrance.set(1.);
            slot.entrance = Some(entrance);
        }
        if snap {
            slot.entrance = None;
        }
        if let Some(entrance) = slot.entrance {
            window.request_animation_frame();
            entrance.value().clamp(0., 1.)
        } else {
            1.
        }
    }

    pub(super) fn capture(
        &mut self,
        id: SharedString,
        child: AnyElement,
        world: Point<f32>,
        zoom: f32,
        now: Instant,
    ) -> AnyElement {
        let slot = self.slots.entry(id).or_insert_with(|| Slot {
            paint: Rc::new(RefCell::new(None)),
            exit: None,
            entrance: None,
            last: now,
            order: 0,
        });
        self.order += 1;
        slot.order = self.order;
        CapturedCard {
            child,
            saved: Rc::clone(&slot.paint),
            world,
            zoom,
        }
        .into_any_element()
    }

    pub(super) fn exits(&self, viewport: GraphViewport, window: &mut Window) -> Vec<AnyElement> {
        let mut slots: Vec<_> = self
            .slots
            .values()
            .filter(|slot| slot.exit.is_some())
            .collect();
        slots.sort_unstable_by_key(|slot| slot.order);
        slots
            .into_iter()
            .filter_map(|slot| {
                let fade = slot.exit?;
                let saved = Rc::clone(&slot.paint);
                window.request_animation_frame();
                Some(
                    div()
                        .absolute()
                        .inset_0()
                        .opacity(fade.value())
                        .child(
                            canvas(
                                |_, _, _| (),
                                move |bounds, _, window, _| {
                                    let snapshot = saved.borrow().clone();
                                    let Some(snapshot) = snapshot else { return };
                                    let origin = bounds.origin
                                        + point(
                                            px(viewport.offset.x
                                                + snapshot.world.x * viewport.zoom),
                                            px(viewport.offset.y
                                                + snapshot.world.y * viewport.zoom),
                                        );
                                    let offset = origin - snapshot.origin;
                                    // Translation occurs before scaling about the destination,
                                    // so a current-local offset is transformed exactly once.
                                    let result = window.with_visual_scale(
                                        viewport.zoom / snapshot.zoom,
                                        origin,
                                        |window| {
                                            window.paint_recording_with_offset(
                                                &snapshot.paint,
                                                offset,
                                            )
                                        },
                                    );
                                    if result.is_err() {
                                        saved.borrow_mut().take();
                                    }
                                },
                            )
                            .size_full(),
                        )
                        .into_any_element(),
                )
            })
            .collect()
    }

    /// Capture an already-painted route in the graph's shared canvas layer.
    /// No element, edge callback or hit target survives in this recording.
    pub(super) fn capture_route(
        &mut self,
        id: SharedString,
        mark: PaintMark,
        canvas_origin: Point<Pixels>,
        viewport: GraphViewport,
        window: &mut Window,
        now: Instant,
    ) {
        let slot = self.slots.entry(id).or_insert_with(|| Slot {
            paint: Rc::new(RefCell::new(None)),
            exit: None,
            entrance: None,
            last: now,
            order: 0,
        });
        self.order += 1;
        slot.order = self.order;
        *slot.paint.borrow_mut() = window.record_paint_since(mark).ok().map(|paint| Saved {
            paint,
            origin: canvas_origin + point(px(viewport.offset.x), px(viewport.offset.y)),
            world: point(0., 0.),
            zoom: viewport.zoom,
        });
    }
}

struct CapturedCard {
    child: AnyElement,
    saved: Rc<RefCell<Option<Saved>>>,
    world: Point<f32>,
    zoom: f32,
}

impl IntoElement for CapturedCard {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for CapturedCard {
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
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let mark = window.paint_mark();
        self.child.paint(window, cx);
        *self.saved.borrow_mut() = window.record_paint_since(mark).ok().map(|paint| Saved {
            paint,
            origin: bounds.origin,
            world: self.world,
            zoom: self.zoom,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{foundation::Ident, motion::keyed, prelude::*};
    use gpui::{Modifiers, MouseButton, TestAppContext};
    use gpui_kit_testkit::harness::Harness;
    use std::{cell::Cell, time::Duration};

    fn history(harness: &mut Harness) -> Rc<RefCell<Retirement>> {
        harness.update(|window, cx| {
            keyed::slot::<Retirement>(
                &Ident::new("recording-graph")
                    .child("retirement")
                    .semantic_id(),
                window.window_handle().window_id(),
                cx,
            )
        })
    }

    fn card() -> Placed {
        Placed::new(
            GraphNode::new("recorded", "Caller card").width(220.),
            30.,
            20.,
        )
        .height(180.)
    }

    #[gpui::test]
    fn route_removal_drops_input_immediately_and_last_endpoint_retires_dangling_wire(
        cx: &mut TestAppContext,
    ) {
        let source = GraphSource::new(
            [
                Placed::new(GraphNode::new("a", "A").width(120.), 20., 30.).height(80.),
                Placed::new(GraphNode::new("b", "B").width(120.), 340., 30.).height(80.),
            ],
            [GraphEdge::new("a", "b").id("wire")],
        )
        .expect("source");
        let shown = source.clone();
        let disconnects = Rc::new(Cell::new(0));
        let reports = disconnects.clone();
        let mut harness = Harness::new(cx, crate::install, move |_, _| {
            let reports = reports.clone();
            div()
                .w(px(600.))
                .h(px(300.))
                .child(
                    NodeGraph::new("route-retirement-test")
                        .source(shown.clone())
                        .on_event(move |event, _, _| {
                            if matches!(event, NodeGraphEvent::DisconnectRequested { .. }) {
                                reports.set(reports.get() + 1);
                            }
                        }),
                )
                .into_any_element()
        });
        harness.update(|_, cx| cx.set_reduce_motion(false));
        harness.frame();
        let history = harness.update(|window, cx| {
            keyed::slot::<Retirement>(
                &Ident::new("route-retirement-test")
                    .child("route-retirement")
                    .semantic_id(),
                window.window_handle().window_id(),
                cx,
            )
        });
        let id = crate::canvas::composite_id("graph-edge", &["wire"]);
        let at = harness
            .bounds(&id)
            .expect("live disconnect target")
            .center();
        assert!(history.borrow().slots["wire"].paint.borrow().is_some());
        let revision = source.revision();
        harness.click(&id);
        harness.frame();
        assert_eq!(disconnects.get(), 1);
        assert_eq!(
            source.revision(),
            revision,
            "host refused the disconnect proposal"
        );
        assert!(harness.node(&id).is_some());
        assert!(history.borrow().slots["wire"].exit.is_none());
        harness
            .context()
            .simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        assert!(harness.update(|window, _| window.captured_hitbox().is_some()));
        source.replace_edges([]).expect("accepted removal");
        harness.frame();
        assert!(harness.update(|window, _| window.captured_hitbox().is_none()));
        harness
            .context()
            .simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        assert_eq!(
            disconnects.get(),
            1,
            "removed capture cannot commit on release"
        );
        assert!(harness.node(&id).is_none());
        assert!(history.borrow().slots["wire"].exit.is_some());
        assert!(history.borrow().slots["wire"].paint.borrow().is_some());
        harness
            .context()
            .simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        harness
            .context()
            .simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        harness.keystrokes("enter");
        harness.frame();
        assert_eq!(disconnects.get(), 1);
        harness.advance(Duration::from_millis(30));
        assert!(history.borrow().slots["wire"].exit.expect("exit").value() < 1.);
        source
            .replace_edges([GraphEdge::new("a", "b").id("wire").state(EdgeState::Failed)])
            .expect("new incarnation");
        harness.frame();
        assert!(history.borrow().slots["wire"].exit.is_none());
        assert!(harness.node(&id).is_some());
        source.remove("a");
        source.remove("b");
        harness.frame();
        assert!(harness.node(&id).is_none());
        assert!(history.borrow().slots["wire"].exit.is_some());
        assert!(
            history.borrow().slots["wire"].paint.borrow().is_some(),
            "last wire replays through empty graph"
        );
        harness.update(|_, cx| cx.set_reduce_motion(true));
        harness.frame();
        assert!(history.borrow().slots.is_empty());
    }

    #[gpui::test]
    fn refused_route_capture_clears_the_previous_picture(cx: &mut TestAppContext) {
        let cache = Rc::new(RefCell::new(Retirement::default()));
        let refused = Rc::new(Cell::new(false));
        let shown = cache.clone();
        let skip = refused.clone();
        let mut harness = Harness::new(cx, crate::install, move |_, _| {
            let shown = shown.clone();
            let skip = skip.clone();
            canvas(
                |_, _, _| (),
                move |bounds, _, window, cx| {
                    let mark = window.paint_mark();
                    if skip.get() {
                        window.paint_backdrop_glass(
                            bounds,
                            Default::default(),
                            gpui::GlassMaterial {
                                blur_radius: px(8.),
                                ..Default::default()
                            },
                            &[],
                        );
                    } else {
                        window.paint_quad(gpui::fill(bounds, gpui::rgb(0x7a4b91)));
                    }
                    shown.borrow_mut().capture_route(
                        "wire".into(),
                        mark,
                        bounds.origin,
                        GraphViewport::default(),
                        window,
                        cx.background_executor().now(),
                    );
                },
            )
            .w(px(100.))
            .h(px(40.))
            .into_any_element()
        });
        harness.frame();
        assert!(cache.borrow().slots["wire"].paint.borrow().is_some());
        refused.set(true);
        harness.frame();
        assert!(cache.borrow().slots["wire"].paint.borrow().is_none());
        harness.update(|window, cx| {
            let spec = MotionPolicy::resolve(crate::motion::MotionRole::Exit, cx).spec();
            cache.borrow_mut().sync(
                std::iter::empty(),
                None,
                true,
                cx.background_executor().now(),
                spec,
            );
            assert!(
                cache
                    .borrow()
                    .exits(GraphViewport::default(), window)
                    .is_empty()
            );
        });
        assert!(cache.borrow().slots.is_empty());
    }

    #[gpui::test]
    fn offscreen_route_visits_and_disabled_motion_release_recordings(cx: &mut TestAppContext) {
        let source = GraphSource::new(
            [
                Placed::new(GraphNode::new("a", "A").width(120.), 20., 30.).height(80.),
                Placed::new(GraphNode::new("b", "B").width(120.), 340., 30.).height(80.),
            ],
            [GraphEdge::new("a", "b").id("wire")],
        )
        .expect("source");
        let view = Rc::new(Cell::new(GraphViewport::default()));
        let enabled = Rc::new(Cell::new(true));
        let (nodes, viewport, animate) = (source.clone(), view.clone(), enabled.clone());
        let mut harness = Harness::new(cx, crate::install, move |_, _| {
            div()
                .w(px(600.))
                .h(px(300.))
                .child(
                    NodeGraph::new("route-cache-test")
                        .source(nodes.clone())
                        .viewport(viewport.get())
                        .animate_layout(animate.get()),
                )
                .into_any_element()
        });
        harness.update(|_, cx| cx.set_reduce_motion(false));
        harness.frame();
        let cache = harness.update(|window, cx| {
            keyed::slot::<Retirement>(
                &Ident::new("route-cache-test")
                    .child("route-retirement")
                    .semantic_id(),
                window.window_handle().window_id(),
                cx,
            )
        });
        assert_eq!(cache.borrow().slots.len(), 1);
        view.set(GraphViewport::new(point(-5000., 0.), 1.));
        harness.frame();
        harness.advance(Duration::from_secs(1));
        assert!(
            cache.borrow().slots.is_empty(),
            "panned-out route is not a removed route"
        );
        view.set(GraphViewport::default());
        harness.frame();
        harness.advance(Duration::from_secs(1));
        assert!(cache.borrow().slots["wire"].exit.is_none());
        source.replace_edges([]).expect("removal");
        harness.frame();
        assert!(cache.borrow().slots["wire"].exit.is_some());
        harness.advance(Duration::from_secs(1));
        assert!(cache.borrow().slots.is_empty());
        source
            .replace_edges([GraphEdge::new("a", "b").id("wire")])
            .expect("reinsert");
        harness.frame();
        assert_eq!(cache.borrow().slots.len(), 1);
        enabled.set(false);
        harness.frame();
        assert!(cache.borrow().slots.is_empty());
    }

    #[gpui::test]
    fn removed_source_replays_without_content_builds_or_input_and_reinsertion_discards_picture(
        cx: &mut TestAppContext,
    ) {
        let source = GraphSource::new([card()], []).expect("source");
        let builds = Rc::new(Cell::new(0));
        let clicks = Rc::new(Cell::new(0));
        let built = builds.clone();
        let clicked = clicks.clone();
        source
            .set_content("recorded", move |_, _| {
                built.set(built.get() + 1);
                let clicked = clicked.clone();
                Button::new("recorded-payload")
                    .label("Version one")
                    .on_click(move |_, _| clicked.set(clicked.get() + 1))
                    .into_any_element()
            })
            .expect("factory");
        let shown = source.clone();
        let mut harness = Harness::new(cx, crate::install, move |_, _| {
            div()
                .w(px(420.))
                .h(px(300.))
                .child(NodeGraph::new("recording-graph").source(shown.clone()))
                .into_any_element()
        });
        harness.update(|_, cx| cx.set_reduce_motion(false));
        harness.frame();
        let history = history(&mut harness);
        assert!(
            history.borrow().slots["recorded"].paint.borrow().is_some(),
            "live arbitrary child was recorded"
        );
        harness.click("recorded-payload");
        assert_eq!(clicks.get(), 1);
        let at = harness
            .bounds("recorded-payload")
            .expect("live payload bounds")
            .center();
        let built = builds.get();
        source.remove("recorded");
        harness.frame();
        assert!(harness.node("recorded").is_none());
        assert!(harness.node("recorded-payload").is_none());
        assert_eq!(builds.get(), built);
        assert!(history.borrow().slots["recorded"].exit.is_some());
        assert!(
            history.borrow().slots["recorded"].paint.borrow().is_some(),
            "replay succeeded through the empty-graph branch"
        );
        harness
            .context()
            .simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        harness
            .context()
            .simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        harness.frame();
        assert_eq!(clicks.get(), 1, "frozen button has no handler");
        harness.keystrokes("enter");
        assert_eq!(
            clicks.get(),
            1,
            "retirement does not restore keyboard activation"
        );
        harness.advance(Duration::from_millis(30));
        assert!(
            history.borrow().slots["recorded"]
                .exit
                .expect("fading")
                .value()
                < 1.
        );
        source.upsert(card()).expect("new incarnation");
        harness.frame();
        assert!(history.borrow().slots["recorded"].exit.is_none());
        assert!(
            harness.node("recorded-payload").is_none(),
            "old factory not resurrected"
        );
        source.remove("recorded");
        harness.frame();
        harness.update(|_, cx| cx.set_reduce_motion(true));
        harness.frame();
        assert!(
            history.borrow().slots.is_empty(),
            "reduced motion drops all leases"
        );
        harness.update(|_, cx| cx.set_reduce_motion(false));
        source.upsert(card()).expect("another incarnation");
        harness.frame();
        source.remove("recorded");
        harness.frame();
        assert!(!history.borrow().slots.is_empty());
        harness.advance(Duration::from_secs(1));
        assert!(
            history.borrow().slots.is_empty(),
            "completed exits release recordings"
        );
    }

    #[gpui::test]
    fn glass_capture_refusal_retires_immediately_without_a_substitute(cx: &mut TestAppContext) {
        let source = GraphSource::new([card()], []).expect("source");
        source
            .set_content("recorded", |_, _| {
                Glass::new("glass-payload")
                    .child(div().w(px(100.)).h(px(40.)).child("Glass content"))
                    .into_any_element()
            })
            .expect("glass factory");
        let shown = source.clone();
        let mut harness = Harness::new(cx, crate::install, move |_, _| {
            div()
                .w(px(420.))
                .h(px(300.))
                .child(NodeGraph::new("recording-graph").source(shown.clone()))
                .into_any_element()
        });
        harness.update(|_, cx| cx.set_reduce_motion(false));
        harness.frame();
        let history = history(&mut harness);
        assert!(
            history.borrow().slots["recorded"].paint.borrow().is_none(),
            "optical glass is not a recordable picture"
        );
        source.remove("recorded");
        harness.frame();
        assert!(
            history.borrow().slots.is_empty(),
            "refused capture has no shell-only exit"
        );
        assert!(harness.node("glass-payload").is_none());
    }

    #[gpui::test]
    fn panning_releases_offscreen_pictures_without_treating_nodes_as_removed(
        cx: &mut TestAppContext,
    ) {
        let source = GraphSource::new([card()], []).expect("source");
        let viewport = Rc::new(Cell::new(GraphViewport::default()));
        let shown = viewport.clone();
        let nodes = source.clone();
        let mut harness = Harness::new(cx, crate::install, move |_, _| {
            div()
                .w(px(420.))
                .h(px(300.))
                .child(
                    NodeGraph::new("recording-graph")
                        .source(nodes.clone())
                        .viewport(shown.get()),
                )
                .into_any_element()
        });
        harness.update(|_, cx| cx.set_reduce_motion(false));
        harness.frame();
        let history = history(&mut harness);
        assert_eq!(history.borrow().slots.len(), 1);
        assert!(history.borrow().slots.contains_key("recorded"));
        assert_eq!(history.borrow().revision, Some(source.revision()));
        source
            .upsert(
                Placed::new(GraphNode::new("far", "Far card").width(190.), 5000., 31.).height(137.),
            )
            .expect("offscreen addition");
        harness.frame();
        assert!(!history.borrow().slots.contains_key("far"));
        viewport.set(GraphViewport::new(point(-5000., 0.), 1.));
        harness.frame();
        harness.advance(Duration::from_secs(1));
        harness.frame();
        assert_eq!(source.len(), 2);
        assert_eq!(history.borrow().slots.len(), 1);
        assert!(history.borrow().slots.contains_key("far"));
        assert!(history.borrow().slots["far"].exit.is_none());
        assert!(history.borrow().slots["far"].entrance.is_none());
        assert!(!history.borrow().slots.contains_key("recorded"));
        viewport.set(GraphViewport::default());
        harness.frame();
        harness.advance(Duration::from_secs(1));
        assert!(history.borrow().slots["recorded"].entrance.is_none());
    }

    #[gpui::test]
    fn additions_enter_without_restarting_on_current_metadata_and_drag_snaps(
        cx: &mut TestAppContext,
    ) {
        let source = GraphSource::new([card()], []).expect("source");
        let nodes = source.clone();
        let mut harness = Harness::new(cx, crate::install, move |_, _| {
            div()
                .w(px(420.))
                .h(px(400.))
                .child(
                    NodeGraph::new("recording-graph")
                        .source(nodes.clone())
                        .on_event(|_, _, _| {}),
                )
                .into_any_element()
        });
        harness.update(|_, cx| cx.set_reduce_motion(false));
        harness.frame();
        let history = history(&mut harness);
        assert!(history.borrow().slots["recorded"].entrance.is_none());
        let addition = |title| {
            Placed::new(GraphNode::new("arriving", title).width(190.), 23., 190.).height(97.)
        };
        let progress = || {
            history.borrow().slots["arriving"]
                .entrance
                .expect("entering")
                .value()
        };
        source.upsert(addition("First title")).expect("addition");
        harness.frame();
        assert_eq!(progress(), 0.);
        harness.advance(Duration::from_millis(30));
        let shown = progress();
        assert!(shown > 0. && shown < 1., "intermediate entrance: {shown}");
        source.upsert(addition("Current title")).expect("update");
        harness.frame();
        assert_eq!(
            progress(),
            shown,
            "publishing metadata must not restart the entrance"
        );
        assert_eq!(
            harness.node("arriving").expect("mounted").text.as_deref(),
            Some("Current title")
        );
        source
            .set_content("arriving", |_, _| {
                Button::new("arrival-content")
                    .label("Current factory")
                    .into_any_element()
            })
            .expect("current factory");
        harness.frame();
        assert_eq!(progress(), shown);
        assert_eq!(
            harness
                .node("arrival-content")
                .expect("live content")
                .text
                .as_deref(),
            Some("Current factory")
        );
        let at =
            harness.bounds("arriving").expect("mounted bounds").origin + point(px(12.), px(12.));
        harness
            .context()
            .simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        harness.frame();
        assert!(history.borrow().slots["arriving"].entrance.is_none());
        harness
            .context()
            .simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        source.remove("arriving");
        harness.frame();
        assert!(history.borrow().slots["arriving"].exit.is_some());
        source
            .upsert(addition("New incarnation"))
            .expect("reinsert");
        harness.frame();
        assert!(history.borrow().slots["arriving"].exit.is_none());
        assert_eq!(progress(), 0.);
        harness.update(|_, cx| cx.set_reduce_motion(true));
        harness.frame();
        assert!(history.borrow().slots.is_empty());
        assert!(harness.node("arriving").is_some());
        harness.update(|_, cx| cx.set_reduce_motion(false));
        harness.frame();
        assert!(history.borrow().slots["arriving"].entrance.is_none());
    }
}
