use gpui::{
    Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase, div, point, prelude::*,
    px,
};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;
use std::{cell::RefCell, rc::Rc, time::Instant};

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
