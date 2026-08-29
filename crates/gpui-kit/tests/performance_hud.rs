use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui::{FrameTimingSummary, TestAppContext, prelude::*};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

fn summary() -> FrameTimingSummary {
    FrameTimingSummary {
        sample_count: 4,
        frames_per_second: 50.0,
        frame_budget: Duration::from_millis(16),
        mean_draw_duration: Duration::from_millis(10),
        p95_draw_duration: Duration::from_millis(20),
        over_budget_fraction: 0.25,
        mean_invalidations: 2.0,
        mean_dirty_to_draw_duration: Some(Duration::from_millis(12)),
        draw_durations: vec![
            Duration::from_millis(8),
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(2),
        ],
    }
}

#[gpui::test]
fn performance_states_remain_distinct(cx: &mut TestAppContext) {
    for (id, state, expected, busy) in [
        (
            "performance.waiting",
            PerformanceHudState::Waiting,
            "waiting",
            true,
        ),
        (
            "performance.ready",
            PerformanceHudState::Ready(summary()),
            "ready",
            false,
        ),
        (
            "performance.unavailable",
            PerformanceHudState::Unavailable("The host refused tracing.".into()),
            "unavailable",
            false,
        ),
    ] {
        let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
            PerformanceHud::new(id, state.clone()).into_any_element()
        });
        let node = harness.node(id).expect("HUD publishes its state");
        assert_eq!(node.value.as_deref(), Some(expected));
        assert_eq!(node.busy, busy);
    }
}

#[gpui::test]
fn performance_details_are_controlled_and_publish_localized_readings(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = calls.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        PerformanceHud::new("performance", PerformanceHudState::Ready(summary()))
            .expanded(false)
            .on_expanded(move |expanded, _, _| sink.borrow_mut().push(expanded))
            .into_any_element()
    });

    assert_eq!(
        harness
            .node("performance.fps")
            .expect("FPS")
            .value
            .as_deref(),
        Some("50.0 FPS")
    );
    assert!(harness.node("performance.p95-draw").is_none());
    assert_eq!(
        harness
            .node("performance.expanded")
            .expect("controlled action")
            .checked,
        Some(false)
    );

    harness.click("performance.expanded");

    assert_eq!(*calls.borrow(), vec![true]);
    assert!(
        harness.node("performance.p95-draw").is_none(),
        "reporting the intent does not mutate caller-owned expansion"
    );
}
