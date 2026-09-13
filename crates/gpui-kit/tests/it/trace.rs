use gpui::{
    Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase, div, point, prelude::*,
    px,
};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;
use std::{cell::RefCell, rc::Rc, time::Instant};

#[gpui::test]
fn hierarchy_reflow_tracks_displayed_rows_without_accepting_refused_expansion(
    cx: &mut TestAppContext,
) {
    use gpui_kit::motion::{CubicBezier, MotionSpec};
    use std::{cell::Cell, time::Duration};
    // Exercise both shared row-mount paths, with a sibling after an asymmetric
    // three-descendant branch.
    for visible_rows in [8, 64] {
        let collapsed = Rc::new(Cell::new(false));
        let enabled = Rc::new(Cell::new(true));
        let proposals = Rc::new(RefCell::new(Vec::new()));
        let (state, animate, log) = (collapsed.clone(), enabled.clone(), proposals.clone());
        let spans = Rc::new(
            [
                TraceSpan::new("root", "Root", 0., 1.),
                TraceSpan::new("branch", "Branch", 0., 0.8).depth(1),
                TraceSpan::new("leaf", "Leaf", 0.2, 0.3).depth(3),
                TraceSpan::new("peer", "Peer", 0.4, 0.6).depth(1),
                TraceSpan::new("outside", "Outside", 0., 1.).time(125.25, 875.75),
            ]
            .into_iter()
            .chain((0..20).map(|i| TraceSpan::new(format!("extra.{i}"), "Extra", 0., 1.)))
            .collect::<Vec<_>>(),
        );
        let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
            let (state, log) = (state.clone(), log.clone());
            div()
                .w(px(640.))
                .child(
                    TraceView::new("trace", "Reflow fixture")
                        .shared_spans(spans.clone())
                        .visible_rows(visible_rows)
                        .current("outside")
                        .time_viewport([0., 1000.])
                        .expect("domain")
                        .animate_viewport(false)
                        .animate_layout(animate.get())
                        .layout_animation(MotionSpec::new(400, CubicBezier::new(0., 0., 1., 1.)))
                        .collapsed(state.get().then(|| "root".into()))
                        .on_toggle(move |id, expanded, _, cx| {
                            log.borrow_mut().push((id, expanded));
                            if !expanded {
                                state.set(true);
                            }
                            cx.refresh_windows();
                        }),
                )
                .into_any_element()
        });
        harness.update(|_, cx| cx.set_reduce_motion(false));
        let before = harness.bounds("trace.outside").expect("mounted sibling");
        let description = harness
            .node("trace.outside")
            .expect("mounted sibling")
            .description;
        harness.click("trace.root.toggle");
        assert_eq!(
            harness.bounds("trace.outside").expect("mounted sibling"),
            before
        );
        assert!(harness.node("trace.branch").is_none());
        assert!(harness.node("trace.branch.toggle").is_none());
        assert!(harness.node("trace.leaf").is_none());
        harness.advance(Duration::from_millis(100));
        let middle = harness.bounds("trace.outside").expect("moving sibling");
        assert!(middle.top() < before.top());
        assert_eq!(
            harness
                .node("trace.outside")
                .expect("moving sibling")
                .description,
            description
        );
        assert!(
            harness
                .node("trace.outside")
                .expect("moving sibling")
                .selected
        );
        // Expansion is refused by the host, even while survivors are moving.
        harness.click("trace.root.toggle");
        assert!(collapsed.get());
        assert!(harness.node("trace.branch").is_none());
        assert_eq!(
            &*proposals.borrow(),
            &[("root".into(), false), ("root".into(), true)]
        );
        // A subsequent caller acceptance retargets from the displayed position.
        let displayed = harness.bounds("trace.outside").expect("moving sibling");
        collapsed.set(false);
        harness.frame();
        assert_eq!(
            harness.bounds("trace.outside").expect("retargeted sibling"),
            displayed
        );
        // Newly exposed rows must not cover the still-moving sibling or
        // install invisible interaction targets in their final slots.
        assert!(harness.node("trace.leaf").is_none());
        assert!(harness.node("trace.branch.toggle").is_none());
        harness.advance(Duration::from_millis(100));
        assert!(
            harness
                .bounds("trace.outside")
                .expect("moving sibling")
                .top()
                > displayed.top()
        );
        assert!(harness.node("trace.leaf").is_none());
        harness.advance(Duration::from_millis(300));
        assert!(harness.node("trace.leaf").is_some());
        assert_eq!(
            harness.bounds("trace.outside").expect("settled sibling"),
            before
        );
        collapsed.set(true);
        harness.frame();
        harness.advance(Duration::from_millis(100));
        collapsed.set(false);
        harness.frame();
        assert!(harness.node("trace.leaf").is_none());
        enabled.set(false);
        harness.frame();
        assert!(harness.node("trace.leaf").is_some());
        assert_eq!(
            harness
                .bounds("trace.outside")
                .expect("disabled motion sibling"),
            before
        );
        enabled.set(true);
        harness.frame();
        assert_eq!(
            harness
                .bounds("trace.outside")
                .expect("reenabled motion sibling"),
            before
        );
        collapsed.set(true);
        harness.frame();
        harness.advance(Duration::from_millis(100));
        harness.update(|_, cx| cx.set_reduce_motion(true));
        harness.frame();
        let settled = harness
            .bounds("trace.outside")
            .expect("reduced motion sibling");
        assert!(settled.top() < middle.top());
        harness.advance(Duration::from_millis(500));
        assert_eq!(
            harness.bounds("trace.outside").expect("settled sibling"),
            settled
        );
        harness.update(|window, _| window.remove_window());
    }
}

#[gpui::test]
fn timeline_scroll_is_not_reorder_and_new_virtual_rows_mount_in_place(cx: &mut TestAppContext) {
    use gpui_kit::motion::{CubicBezier, MotionSpec};
    use std::time::Duration;
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(640.))
            .child(
                SpanTimeline::new("timeline", "Scroll fixture")
                    .spans((0..100).map(|i| TraceSpan::new(format!("span.{i}"), "Span", 0., 1.)))
                    .visible_rows(4)
                    .animate_layout(true)
                    .layout_animation(MotionSpec::new(400, CubicBezier::new(0., 0., 1., 1.))),
            )
            .into_any_element()
    });
    harness.update(|_, cx| cx.set_reduce_motion(false));
    let before = harness
        .bounds("timeline.span.2")
        .expect("initial visible row");
    harness.context().simulate_event(ScrollWheelEvent {
        position: before.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-20.))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    harness.frame();
    let scrolled = harness.bounds("timeline.span.2").expect("scrolled row");
    assert_eq!(scrolled.top(), before.top() - px(20.));
    harness.advance(Duration::from_millis(100));
    assert_eq!(
        harness.bounds("timeline.span.2").expect("stable row"),
        scrolled
    );
    harness.context().simulate_event(ScrollWheelEvent {
        position: scrolled.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-600.))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    harness.frame();
    assert!(harness.node("timeline.span.2").is_none());
    let mounted = harness
        .current_snapshot()
        .descendants_of("timeline")
        .into_iter()
        .filter(|node| node.role == Role::TreeItem)
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    assert!(!mounted.is_empty());
    let bounds = mounted
        .iter()
        .map(|id| harness.bounds(id).expect("newly mounted row"))
        .collect::<Vec<_>>();
    harness.advance(Duration::from_millis(100));
    for (id, bounds) in mounted.iter().zip(bounds) {
        assert_eq!(
            harness.bounds(id).expect("stable mounted row"),
            bounds,
            "new identity mounts at its own rank"
        );
    }
}

#[gpui::test]
fn trace_input_proposals_leave_refused_time_and_hierarchy_authoritative(cx: &mut TestAppContext) {
    let viewports = Rc::new(RefCell::new(vec![]));
    let toggles = Rc::new(RefCell::new(vec![]));
    let (v, t) = (viewports.clone(), toggles.clone());
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let (v, t) = (v.clone(), t.clone());
        div()
            .w(px(640.))
            .child(
                TraceView::new("trace", "Fixture")
                    .spans(
                        [
                            TraceSpan::new("parent", "Parent", 0., 1.).time(100., 900.),
                            TraceSpan::new("child", "Child", 0., 1.)
                                .time(250., 500.)
                                .depth(1),
                        ]
                        .into_iter()
                        .chain((0..20).map(|i| {
                            TraceSpan::new(format!("extra.{i}"), "Extra", 0., 1.).time(100., 800.)
                        })),
                    )
                    .visible_rows(4)
                    .time_viewport([100., 900.])
                    .expect("valid fixture time domain")
                    .on_viewport(move |scale, _, _| v.borrow_mut().push(scale))
                    .on_toggle(move |id, expanded, _, _| t.borrow_mut().push((id, expanded))),
            )
            .into_any_element()
    });
    let before = harness.bounds("trace.child").expect("visible child");
    let bounds = harness.bounds("trace").expect("visible trace");
    let pointer = point(bounds.left() + px(420.), bounds.top() + px(45.));
    for _ in 0..2 {
        harness.context().simulate_event(ScrollWheelEvent {
            position: pointer,
            delta: ScrollDelta::Lines(point(0., -1.)),
            modifiers: Modifiers {
                control: true,
                ..Modifiers::none()
            },
            touch_phase: TouchPhase::Moved,
        });
        harness.frame();
    }
    assert_eq!(viewports.borrow().len(), 2);
    assert_eq!(
        viewports.borrow()[0],
        viewports.borrow()[1],
        "refused proposal must not accumulate"
    );
    assert!(viewports.borrow()[0].domain()[1] - viewports.borrow()[0].domain()[0] > 800.);
    harness.click("trace.parent.toggle");
    assert_eq!(&*toggles.borrow(), &[("parent".into(), false)]);
    assert!(
        harness.node("trace.child").is_some(),
        "refused collapse retains descendants"
    );
    assert_eq!(
        harness.bounds("trace.child").expect("retained child"),
        before
    );
}

#[gpui::test]
fn trace_mount_work_is_bounded_at_one_hundred_thousand_spans(cx: &mut TestAppContext) {
    let mut expected_paint = None;
    for count in [1_000, 10_000, 100_000] {
        let begin = Instant::now();
        let spans = Rc::new(
            (0..count)
                .map(|i| TraceSpan::new(format!("span.{i}"), "Fixture span", 0.1, 0.7))
                .collect::<Vec<_>>(),
        );
        let input = begin.elapsed();
        let begin = Instant::now();
        let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
            div()
                .w(px(640.))
                .child(
                    TraceView::new("trace", "Fixture")
                        .shared_spans(spans.clone())
                        .visible_rows(8),
                )
                .into_any_element()
        });
        let mount = begin.elapsed();
        let begin = Instant::now();
        harness.frame();
        let redraw = begin.elapsed();
        let stats = harness.frame_stats();
        if let Some(expected) = expected_paint {
            assert_eq!(stats.paint_calls, expected);
        }
        expected_paint = Some(stats.paint_calls);
        let snapshot = harness.current_snapshot();
        let rows = snapshot
            .descendants_of("trace")
            .into_iter()
            .filter(|node| node.role == Role::TreeItem)
            .count();
        assert!(
            (8..=16).contains(&rows),
            "{count} input spans mounted {rows} semantic rows"
        );
        eprintln!(
            "trace spans={count} input={input:?} test_platform_mount={mount:?} static_redraw={redraw:?} published_rows={rows} paint_calls={}; GPU submission not measured",
            stats.paint_calls
        );
        // Later Harness::frame calls refresh every remaining window.
        harness.update(|window, _| window.remove_window());
    }
}

#[gpui::test]
fn keyboard_navigation_reaches_unmounted_trace_rows(cx: &mut TestAppContext) {
    let selected = Rc::new(RefCell::new(gpui::SharedString::from("span.0")));
    let state = selected.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = state.clone();
        div()
            .w(px(640.))
            .child(
                TraceView::new("trace", "Fixture")
                    .spans((0..100).map(|i| TraceSpan::new(format!("span.{i}"), "Stage", 0., 1.)))
                    .visible_rows(4)
                    .current(state.borrow().clone())
                    .on_select(move |id, _, cx| {
                        *sink.borrow_mut() = id;
                        cx.refresh_windows();
                    }),
            )
            .into_any_element()
    });
    harness.click("trace.span.0");
    harness.keystrokes("end");
    assert_eq!(selected.borrow().as_ref(), "span.99");
    assert!(harness.node("trace.span.99").is_some());
    harness.keystrokes("up up");
    assert_eq!(selected.borrow().as_ref(), "span.97");
}

#[gpui::test]
fn accepted_hierarchy_navigation_keeps_nested_collapse_and_uses_parent_identity(
    cx: &mut TestAppContext,
) {
    let selected = Rc::new(RefCell::new(gpui::SharedString::from("root")));
    let collapsed = Rc::new(RefCell::new(
        std::collections::HashSet::<gpui::SharedString>::new(),
    ));
    let spans = Rc::new(
        [
            TraceSpan::new("root", "Root", 0., 1.),
            TraceSpan::new("branch", "Branch", 0., 0.8).depth(2),
            TraceSpan::new("leaf", "Leaf", 0.1, 0.3).depth(5),
            TraceSpan::new("peer", "Peer", 0.5, 0.9).depth(2),
            TraceSpan::new("outside", "Outside", 0., 1.),
        ]
        .into_iter()
        .collect::<Vec<_>>(),
    );
    let (selection, collapse) = (selected.clone(), collapsed.clone());
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let (select, toggle) = (selection.clone(), collapse.clone());
        div()
            .w(px(640.))
            .child(
                TraceView::new("trace", "Fixture")
                    .shared_spans(spans.clone())
                    .current(selection.borrow().clone())
                    .collapsed(collapse.borrow().iter().cloned())
                    .on_select(move |id, _, cx| {
                        *select.borrow_mut() = id;
                        cx.refresh_windows();
                    })
                    .on_toggle(move |id, expanded, _, cx| {
                        if expanded {
                            toggle.borrow_mut().remove(&id);
                        } else {
                            toggle.borrow_mut().insert(id);
                        }
                        cx.refresh_windows();
                    }),
            )
            .into_any_element()
    });
    harness.click("trace.root");
    harness.keystrokes("right right");
    assert_eq!(selected.borrow().as_ref(), "leaf");
    harness.keystrokes("left");
    assert_eq!(
        selected.borrow().as_ref(),
        "branch",
        "depth gaps still navigate by ancestry"
    );
    harness.keystrokes("left");
    assert!(collapsed.borrow().contains("branch"));
    assert!(harness.node("trace.leaf").is_none());
    harness.keystrokes("left left");
    assert_eq!(selected.borrow().as_ref(), "root");
    assert!(harness.node("trace.branch").is_none());
    assert!(harness.node("trace.outside").is_some());
    harness.keystrokes("right right");
    assert_eq!(selected.borrow().as_ref(), "branch");
    assert!(
        harness.node("trace.leaf").is_none(),
        "nested collapse survives ancestor expansion"
    );
    harness.keystrokes("right right");
    assert_eq!(selected.borrow().as_ref(), "leaf");
    assert!(harness.node("trace.leaf").is_some());
}

#[gpui::test]
fn time_viewport_samples_retarget_and_reduced_motion_with_current_readout(cx: &mut TestAppContext) {
    let epoch = 1_700_000_000_000.;
    let domain = Rc::new(RefCell::new([epoch, epoch + 1000.]));
    let state = domain.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(640.))
            .child(
                TraceView::new("trace", "Fixture")
                    .spans([TraceSpan::new("work", "Work", 0., 1.)
                        .time(epoch + 400.125, epoch + 600.875)
                        .duration("200.75 ms")])
                    .time_viewport(*state.borrow())
                    .expect("valid fixture viewport"),
            )
            .into_any_element()
    });
    harness.update(|_, cx| cx.set_reduce_motion(false));
    let before = harness
        .bounds("trace.work.interval")
        .expect("initial interval");
    *domain.borrow_mut() = [epoch + 200., epoch + 800.];
    harness.frame();
    assert_eq!(
        harness
            .bounds("trace.work.interval")
            .expect("starting interval"),
        before
    );
    harness.advance(std::time::Duration::from_millis(80));
    let middle = harness
        .bounds("trace.work.interval")
        .expect("intermediate interval");
    assert!(middle.size.width > before.size.width);
    *domain.borrow_mut() = [epoch - 200., epoch + 1200.];
    harness.frame();
    assert_eq!(
        harness
            .bounds("trace.work.interval")
            .expect("retarget interval"),
        middle,
        "retarget stays at displayed geometry"
    );
    harness.advance(std::time::Duration::from_millis(80));
    assert!(
        harness
            .bounds("trace.work.interval")
            .expect("resumed interval")
            .size
            .width
            < middle.size.width
    );
    harness.update(|_, cx| cx.set_reduce_motion(true));
    harness.frame();
    let settled = harness
        .bounds("trace.work.interval")
        .expect("settled interval");
    assert!(
        (f32::from(settled.size.width) / f32::from(before.size.width) - 1000. / 1400.).abs() < 0.01
    );
    let description = harness
        .node("trace.work")
        .expect("span row")
        .description
        .expect("exact description");
    assert!(description.contains("1700000000400.125"));
    assert!(description.contains("1700000000600.875"));
    assert!(description.contains("200.75 ms"));
}

#[gpui::test]
fn persistent_time_range_capture_keeps_acceptance_and_refusal_distinct(cx: &mut TestAppContext) {
    use gpui::MouseButton;
    use gpui_kit::interaction::range::{RangeEvent, RangeIntent, RangeTarget};
    let epoch = 1_700_000_000_000.;
    for accepts in [false, true] {
        let selected = Rc::new(RefCell::new(Some([epoch + 200., epoch + 450.])));
        let events = Rc::new(RefCell::new(Vec::new()));
        let current = selected.clone();
        let log = events.clone();
        let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
            let current = current.clone();
            let log = log.clone();
            let selected = *current.borrow();
            div()
                .w(px(600.))
                .child(
                    TraceView::new("trace", "Range fixture")
                        .spans([TraceSpan::new("span", "Span", 0., 1.)])
                        .time_viewport([epoch, epoch + 1000.])
                        .expect("domain")
                        .selected_time(selected)
                        .expect("selection")
                        .on_time_selection(move |event, window, _| {
                            log.borrow_mut().push(event);
                            if accepts
                                && let RangeEvent::Update { value, .. }
                                | RangeEvent::Commit { value, .. } = event
                            {
                                *current.borrow_mut() = Some(value);
                            }
                            window.refresh();
                        }),
                )
                .into_any_element()
        });
        let end = harness
            .bounds("trace.time-selection.end")
            .expect("end handle")
            .center();
        let track = harness
            .bounds("trace.time-selection.track")
            .expect("range track");
        harness
            .context()
            .simulate_mouse_down(end, MouseButton::Left, Modifiers::none());
        harness.drag_to(point(track.right() + px(900.), end.y));
        harness.drop_here();
        assert_eq!(
            events.borrow().last(),
            Some(&RangeEvent::Commit {
                intent: RangeIntent::Resize(RangeTarget::End),
                value: [epoch + 200., epoch + 1000.],
            })
        );
        assert_eq!(
            *selected.borrow(),
            Some([epoch + 200., epoch + if accepts { 1000. } else { 450. }])
        );
        assert!(harness.update(|window, _| window.captured_hitbox().is_none()));
        assert_eq!(
            harness
                .node("trace.time-selection")
                .expect("selection")
                .value
                .as_deref(),
            Some("controlled")
        );
        events.borrow_mut().clear();
        harness.drag_start("trace.time-selection.start");
        harness.context().simulate_event(gpui::MouseCancelEvent);
        harness.frame();
        assert!(matches!(
            events.borrow().last(),
            Some(RangeEvent::Cancel {
                intent: RangeIntent::Resize(RangeTarget::Start)
            })
        ));
        assert!(
            !events
                .borrow()
                .iter()
                .any(|event| matches!(event, RangeEvent::Commit { .. }))
        );
        harness.update(|window, _| window.remove_window());
    }
}

#[gpui::test]
fn reversed_time_range_keyboard_and_disabled_capture_preserve_raw_identity(
    cx: &mut TestAppContext,
) {
    use gpui::MouseButton;
    use gpui_kit::interaction::range::{RangeEvent, RangeIntent, RangeTarget};
    let epoch = 1_700_000_000_000.;
    let enabled = Rc::new(std::cell::Cell::new(true));
    let shown = Rc::new(std::cell::Cell::new(true));
    let events = Rc::new(RefCell::new(Vec::new()));
    let (active, present, log) = (enabled.clone(), shown.clone(), events.clone());
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let mut trace = TraceView::new("trace", "Reversed range")
            .spans([TraceSpan::new("span", "Span", 0., 1.)])
            .time_viewport([epoch + 1000., epoch])
            .expect("reversed domain")
            .selected_time(Some([epoch + 200., epoch + 450.]))
            .expect("ascending selection");
        if active.get() {
            let log = log.clone();
            trace = trace.on_time_selection(move |event, _, _| log.borrow_mut().push(event));
        }
        div()
            .w(px(600.))
            .children(present.get().then_some(trace))
            .into_any_element()
    });
    let start = harness
        .bounds("trace.time-selection.start")
        .expect("raw start")
        .center();
    let end = harness
        .bounds("trace.time-selection.end")
        .expect("raw end")
        .center();
    assert!(start.x > end.x, "raw Start remains the right-hand endpoint");
    harness.click("trace.time-selection.start");
    events.borrow_mut().clear();
    harness.keystrokes("right");
    assert_eq!(
        events.borrow().last(),
        Some(&RangeEvent::Commit {
            intent: RangeIntent::Resize(RangeTarget::Start),
            value: [epoch + 190., epoch + 450.],
        })
    );
    events.borrow_mut().clear();
    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    enabled.set(false);
    harness.frame();
    assert!(harness.update(|window, _| window.captured_hitbox().is_none()));
    assert!(matches!(
        events.borrow().last(),
        Some(RangeEvent::Cancel { .. })
    ));
    assert!(
        harness
            .node("trace.time-selection.start")
            .expect("disabled endpoint")
            .disabled
    );
    events.borrow_mut().clear();
    harness.click("trace.time-selection.end");
    assert!(
        events.borrow().is_empty(),
        "disabled control installs no handler"
    );
    enabled.set(true);
    harness.frame();
    harness
        .context()
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    shown.set(false);
    harness.frame();
    assert!(harness.update(|window, _| window.captured_hitbox().is_none()));
    harness.context().simulate_mouse_up(
        point(px(-200.), px(-200.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    shown.set(true);
    harness.frame();
    assert_eq!(
        harness
            .node("trace.time-selection")
            .expect("remounted")
            .value
            .as_deref(),
        Some("controlled")
    );
    assert!(
        !events
            .borrow()
            .iter()
            .any(|event| matches!(event, RangeEvent::Commit { .. }))
    );
}

#[gpui::test]
fn time_range_holds_presented_motion_and_cancels_caller_viewport_replacement(
    cx: &mut TestAppContext,
) {
    let domain = Rc::new(RefCell::new([0., 1000.]));
    let current = domain.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let log = events.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let log = log.clone();
        div()
            .w(px(700.))
            .child(
                TraceView::new("trace", "Motion and range")
                    .spans([TraceSpan::new("span", "Span", 0., 1.).time(420., 655.)])
                    .time_viewport(*current.borrow())
                    .expect("domain")
                    .selected_time(Some([420., 655.]))
                    .expect("selection")
                    .on_time_selection(move |event, _, _| log.borrow_mut().push(event)),
            )
            .into_any_element()
    });
    harness.update(|_, cx| cx.set_reduce_motion(false));
    *domain.borrow_mut() = [200., 800.];
    harness.frame();
    harness.advance(std::time::Duration::from_millis(80));
    let moving = harness.bounds("trace.span.interval").expect("moving bar");
    harness.drag_start("trace.time-selection.start");
    harness.advance(std::time::Duration::from_millis(80));
    assert_eq!(
        harness.bounds("trace.span.interval").expect("held bar"),
        moving
    );
    harness.context().simulate_event(gpui::MouseCancelEvent);
    harness.frame();
    harness.advance(std::time::Duration::from_millis(80));
    assert!(
        harness
            .bounds("trace.span.interval")
            .expect("resumed bar")
            .size
            .width
            > moving.size.width
    );
    harness.drag_start("trace.time-selection.start");
    events.borrow_mut().clear();
    *domain.borrow_mut() = [-100., 1100.];
    harness.frame();
    assert!(harness.update(|window, _| window.captured_hitbox().is_none()));
    assert!(matches!(
        events.borrow().as_slice(),
        [gpui_kit::interaction::range::RangeEvent::Cancel { .. }]
    ));
}

#[gpui::test]
fn localized_time_range_uses_caller_format_without_changing_raw_values(cx: &mut TestAppContext) {
    use gpui_kit::interaction::range::{RangeEvent, RangeIntent, RangeTarget};
    use gpui_kit::strings::TranslationPack;
    let epoch = 1_700_000_000_000.;
    let value = Rc::new(RefCell::new(Some([epoch + 200.125, epoch + 450.875])));
    let selected = value.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let log = events.clone();
    let mut harness = Harness::new(
        cx,
        |cx| {
            gpui_kit::install(cx);
            cx.set_global(TranslationPack::SimplifiedChinese.strings());
        },
        move |_, _| {
            let log = log.clone();
            div()
                .w(px(720.))
                .child(
                    TraceView::new("trace", "中文时间")
                        .spans([TraceSpan::new("span", "操作", 0., 1.)
                            .time(epoch + 25.125, epoch + 680.875)])
                        .time_viewport([epoch, epoch + 1000.])
                        .expect("finite time")
                        .selected_time(*selected.borrow())
                        .expect("ascending range")
                        .format_time(move |value| format!("本地+08:{:.3}", value - epoch).into())
                        .on_time_selection(move |event, _, _| log.borrow_mut().push(event)),
                )
                .into_any_element()
        },
    );
    assert_eq!(
        harness
            .node("trace.time-selection.readout")
            .expect("readout")
            .text
            .as_deref(),
        Some("所选时间：本地+08:200.125 至 本地+08:450.875")
    );
    assert_eq!(
        harness
            .node("trace.time-selection.start")
            .expect("start")
            .text
            .as_deref(),
        Some("选区起点")
    );
    assert_eq!(
        harness
            .node("trace.time-selection.start")
            .expect("start")
            .value
            .as_deref(),
        Some("1700000000200.125")
    );
    assert_eq!(
        harness
            .node("trace.time-selection.track")
            .expect("track")
            .value
            .as_deref(),
        Some("1700000000200.125,1700000000450.875")
    );
    assert_eq!(
        harness
            .node("trace.span")
            .expect("span")
            .description
            .as_deref(),
        Some("操作 · 等待中\n开始：本地+08:25.125；结束：本地+08:680.875")
    );
    harness.drag_start("trace.time-selection.end");
    let outside = point(px(1600.), px(80.));
    harness.drag_to(outside);
    harness.frame();
    assert_eq!(
        harness
            .node("trace.time-selection.readout")
            .expect("preview")
            .text
            .as_deref(),
        Some("时间选择预览：本地+08:200.125 至 本地+08:1000.000")
    );
    harness
        .context()
        .simulate_mouse_up(outside, gpui::MouseButton::Left, Modifiers::none());
    harness.frame();
    assert_eq!(
        events.borrow().last(),
        Some(&RangeEvent::Commit {
            intent: RangeIntent::Resize(RangeTarget::End),
            value: [epoch + 200.125, epoch + 1000.],
        })
    );
    assert_eq!(
        harness
            .node("trace.time-selection.readout")
            .expect("refused")
            .text
            .as_deref(),
        Some("所选时间：本地+08:200.125 至 本地+08:450.875")
    );
    *value.borrow_mut() = None;
    harness.frame();
    assert_eq!(
        harness
            .node("trace.time-selection.readout")
            .expect("empty")
            .text
            .as_deref(),
        Some("未选择时间")
    );
}
