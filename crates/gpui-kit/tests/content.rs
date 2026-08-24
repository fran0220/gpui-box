//! Progress, tags, and empty surfaces report what is true, including when
//! what is true is that nothing is known.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    IntoElement, Modifiers, MouseMoveEvent, ParentElement, Styled, TestAppContext, div, point, px,
};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

#[gpui::test]
fn a_known_extent_is_published_as_a_position(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ProgressBar::new("index.progress")
            .label("Indexing")
            .count(3, 12)
            .into_any_element()
    });

    let node = harness.node("index.progress").expect("published");
    assert_eq!(node.value_min, Some(0.0));
    assert_eq!(node.value_max, Some(1.0));
    assert_eq!(node.value_now, Some(0.25));
    assert_eq!(node.value.as_deref(), Some("3 of 12"));
    assert!(node.busy);
}

#[gpui::test]
fn an_unknown_extent_reports_no_position(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ProgressBar::new("index.progress")
            .label("Contacting host")
            .into_any_element()
    });

    let node = harness.node("index.progress").expect("published");
    assert_eq!(node.value_now, None, "an unknown extent must not claim one");
    assert!(node.busy);
}

#[gpui::test]
fn a_total_of_zero_stays_indeterminate(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ProgressBar::new("index.progress")
            .label("Indexing")
            .count(0, 0)
            .into_any_element()
    });

    assert_eq!(
        harness.node("index.progress").expect("published").value_now,
        None
    );
}

#[gpui::test]
fn a_tag_reports_its_own_removal(cx: &mut TestAppContext) {
    let removals = Rc::new(RefCell::new(0usize));
    let sink = removals.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Tag::new("filter.rust", "rust")
            .on_remove(move |_, _| *sink.borrow_mut() += 1)
            .into_any_element()
    });

    harness.click("filter.rust.remove");
    assert_eq!(*removals.borrow(), 1);
    assert_eq!(
        harness
            .node("filter.rust.remove")
            .expect("published")
            .text
            .as_deref(),
        Some("Remove rust")
    );
}

#[gpui::test]
fn a_tag_that_cannot_be_removed_offers_no_way_to(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        Tag::new("filter.pinned", "pinned")
            .disabled(true)
            .on_remove(|_, _| {})
            .into_any_element()
    });

    assert!(
        harness.node("filter.pinned.remove").is_none(),
        "a refused removal must not publish a control"
    );
}

#[gpui::test]
fn a_tinted_tag_still_publishes_its_tone(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, cx| {
        let tint = cx
            .theme()
            .palette_color("agent.external")
            .expect("bundled identity scale");
        Tag::new("filter.ada", "Ada")
            .tone(Tone::Neutral)
            .tint(tint)
            .into_any_element()
    });

    assert_eq!(
        harness
            .node("filter.ada")
            .expect("published")
            .value
            .as_deref(),
        Some("neutral"),
        "a tint must not replace the severity the tag reports"
    );
}

#[gpui::test]
fn an_empty_surface_says_which_fact_it_is_reporting(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .flex()
            .flex_col()
            .child(
                EmptyState::new("runs.empty", "No runs yet")
                    .kind(EmptyKind::Unstarted)
                    .detail("A run appears here once one has been started."),
            )
            .child(
                EmptyState::new("runs.refused", "The host refused the request")
                    .kind(EmptyKind::Unavailable),
            )
            .into_any_element()
    });

    assert_eq!(
        harness
            .node("runs.empty")
            .expect("published")
            .value
            .as_deref(),
        Some("unstarted")
    );
    assert_eq!(
        harness
            .node("runs.refused")
            .expect("published")
            .value
            .as_deref(),
        Some("unavailable"),
        "a refusal must not be reported as an absence of data"
    );
}

#[gpui::test]
fn an_avatar_and_a_divider_appear_only_when_named(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .flex()
            .flex_col()
            .child(Avatar::new("Ada Lovelace").id("member.ada"))
            .child(Avatar::new("Unnamed"))
            .child(Divider::new().id("section.rule").label("Filters"))
            .child(Divider::new())
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    assert_eq!(
        snapshot
            .find("member.ada")
            .and_then(|node| node.text.as_deref()),
        Some("Ada Lovelace")
    );
    assert_eq!(
        snapshot
            .find("section.rule")
            .and_then(|node| node.text.as_deref()),
        Some("Filters")
    );
    assert_eq!(
        snapshot.nodes.len(),
        2,
        "decorative marks must not add noise to the tree"
    );
}

#[gpui::test]
fn browser_states_publish_one_truthful_current_contract(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .w(px(260.0))
                    .h(px(150.0))
                    .child(BrowserPanel::new("browser.loading").state(ViewportState::Loading)),
            )
            .child(
                div()
                    .w(px(260.0))
                    .h(px(150.0))
                    .child(BrowserPanel::new("browser.empty").state(ViewportState::Empty)),
            )
            .child(div().w(px(260.0)).h(px(150.0)).child(
                BrowserPanel::new("browser.unavailable").state(ViewportState::Unavailable(
                    "The host refused this address.".into(),
                )),
            ))
            .child(
                div().w(px(260.0)).h(px(150.0)).child(
                    BrowserPanel::new("browser.error")
                        .state(ViewportState::Error("Navigation failed.".into())),
                ),
            )
            .child(
                div().w(px(260.0)).h(px(150.0)).child(
                    BrowserPanel::new("browser.ready")
                        .state(ViewportState::Ready)
                        .viewport(div().size_full().child("page")),
                ),
            )
            .into_any_element()
    });

    for (id, expected) in [
        ("browser.loading", "loading"),
        ("browser.empty", "empty"),
        ("browser.unavailable", "unavailable"),
        ("browser.error", "error"),
        ("browser.ready", "ready"),
    ] {
        assert_eq!(
            harness.node(id).expect("browser state").value.as_deref(),
            Some(expected)
        );
        assert_eq!(
            harness
                .node(&format!("{id}.viewport"))
                .expect("stable viewport")
                .value
                .as_deref(),
            Some(expected)
        );
    }
    assert!(harness.node("browser.loading").expect("loading").busy);
    assert!(
        !harness
            .node("browser.unavailable")
            .expect("unavailable")
            .busy
    );
}

#[gpui::test]
fn ready_without_a_host_viewport_reports_an_error(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w(px(280.0))
            .h(px(180.0))
            .child(BrowserPanel::new("browser").state(ViewportState::Ready))
            .into_any_element()
    });

    assert_eq!(
        harness.node("browser").expect("browser").value.as_deref(),
        Some("error")
    );
    assert_eq!(
        harness
            .node("browser.viewport.status")
            .expect("host contract error")
            .value
            .as_deref(),
        Some("failed")
    );
}

#[gpui::test]
fn browser_actions_follow_tab_order_and_keyboard_activation(cx: &mut TestAppContext) {
    let back = Rc::new(Cell::new(0));
    let forward = Rc::new(Cell::new(0));
    let reload = Rc::new(Cell::new(0));
    let back_sink = Rc::clone(&back);
    let forward_sink = Rc::clone(&forward);
    let reload_sink = Rc::clone(&reload);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let back = Rc::clone(&back_sink);
        let forward = Rc::clone(&forward_sink);
        let reload = Rc::clone(&reload_sink);
        div()
            .w(px(320.0))
            .h(px(180.0))
            .child(
                BrowserPanel::new("browser")
                    .on_back(move |_, _| back.set(back.get() + 1))
                    .on_forward(move |_, _| forward.set(forward.get() + 1))
                    .on_reload(move |_, _| reload.set(reload.get() + 1)),
            )
            .into_any_element()
    });

    // GPUI's host-level Tab action advances the window tab order; the panel's
    // enabled controls participate in document order and activate by key.
    for (id, name, key) in [
        ("browser.back", "Back", "enter"),
        ("browser.forward", "Forward", "space"),
        ("browser.reload", "Reload", "enter"),
    ] {
        assert!(harness.node(id).is_some(), "stable semantic id {id}");
        harness.update(|window, cx| window.focus_next(cx));
        let tree = harness.accessibility_tree();
        let focused = tree["gpui_focus"].as_str().expect("focused node");
        assert_eq!(tree["nodes"][focused]["aria"]["label"], name);
        harness.keystrokes(key);
    }

    assert_eq!((back.get(), forward.get(), reload.get()), (1, 1, 1));
}

#[gpui::test]
fn browser_long_content_stays_inside_a_narrow_contract(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w(px(184.0))
            .h(px(160.0))
            .child(
                BrowserPanel::new("browser")
                    .url("https://example.com/a/very/long/path/that/must/not/widen/the/panel")
                    .state(ViewportState::Ready)
                    .viewport(div().size_full().child(
                        "A long host-owned page line remains clipped to this narrow viewport.",
                    )),
            )
            .into_any_element()
    });

    let panel = harness.bounds("browser").expect("panel bounds");
    let address = harness.bounds("browser.address").expect("address bounds");
    let viewport = harness.bounds("browser.viewport").expect("viewport bounds");
    assert!(f32::from(panel.size.width) <= 184.0);
    assert!(address.right() <= panel.right());
    assert!(viewport.right() <= panel.right());
}

fn chart_series() -> Vec<ChartSeries> {
    vec![ChartSeries::new("cpu", "CPU").points([
        ChartPoint::new("early", 0.15, 0.2, "00:15", "20%"),
        ChartPoint::new("late", 0.85, 0.8, "00:45", "80%"),
    ])]
}

#[gpui::test]
fn chart_pointer_and_keyboard_report_business_identity(cx: &mut TestAppContext) {
    let reports = Rc::new(RefCell::new(Vec::<ChartSelection>::new()));
    let sink = Rc::clone(&reports);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        div()
            .w(px(420.0))
            .child(
                LineChart::new("chart", "CPU", ChartState::Ready(chart_series()))
                    .crosshair()
                    .current("cpu", "early")
                    .on_current(move |selection, _, _| sink.borrow_mut().push(selection)),
            )
            .into_any_element()
    });

    let plot = harness.bounds("chart.plot").expect("plot bounds");
    let pointer = point(
        plot.left() + plot.size.width * 0.85,
        plot.top() + plot.size.height * 0.2,
    );
    harness.context().simulate_event(MouseMoveEvent {
        position: pointer,
        pressed_button: None,
        modifiers: Modifiers::none(),
    });
    harness.context().run_until_parked();

    assert_eq!(
        reports.borrow().last(),
        Some(&ChartSelection::new("cpu", "late"))
    );
    assert_eq!(
        harness
            .node("chart.series.cpu.point.late")
            .expect("current point")
            .value
            .as_deref(),
        Some("80%")
    );

    harness.update(|window, cx| window.focus_next(cx));
    harness.keystrokes("left");
    assert_eq!(
        reports.borrow().last(),
        Some(&ChartSelection::new("cpu", "early"))
    );
}

#[gpui::test]
fn chart_semantics_take_new_text_before_pixels_settle(cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ChartState::Ready(vec![
        ChartSeries::new("cpu", "CPU").points([ChartPoint::new("now", 0.25, 0.2, "Now", "20%")]),
    ])));
    let scene = Rc::clone(&state);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        LineChart::new("chart", "CPU", scene.borrow().clone())
            .area()
            .current("cpu", "now")
            .into_any_element()
    });

    state.replace(ChartState::Stale {
        series: vec![
            ChartSeries::new("cpu", "CPU")
                .points([ChartPoint::new("now", 0.75, 0.9, "Now", "90%")]),
        ],
        reason: "refresh failed".into(),
    });
    harness.update(|_, cx| cx.refresh_windows());

    assert_eq!(
        harness
            .node("chart.series.cpu.point.now")
            .expect("current point")
            .value
            .as_deref(),
        Some("90%"),
        "only the geometry travels; current text is the newest host fact"
    );
    let chart = harness.node("chart").expect("chart");
    assert_eq!(chart.value.as_deref(), Some("stale"));
    assert_eq!(chart.description.as_deref(), Some("refresh failed"));
    assert_eq!(
        harness
            .node("chart.stale")
            .expect("visible refresh warning")
            .text
            .as_deref(),
        Some("refresh failed")
    );
}

#[gpui::test]
fn a_state_view_does_not_render_a_refusal_as_empty(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        StateView::new(
            "panel",
            Loadable::<(), String>::Unavailable("the host refused".into()),
        )
        .into_any_element()
    });

    let node = harness.node("panel").expect("published");
    assert_eq!(node.value.as_deref(), Some("unavailable"));
    assert_eq!(
        harness
            .node("panel.empty")
            .expect("refusal surface")
            .value
            .as_deref(),
        Some("unavailable")
    );
}

#[gpui::test]
fn a_refreshing_state_view_keeps_the_verified_value(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        let mut value = AsyncValue::<_, String>::ready("12 runs");
        value.refresh();
        StateView::from_async("panel", &value, |text| {
            div().child(*text).into_any_element()
        })
        .into_any_element()
    });

    let node = harness.node("panel").expect("published");
    assert_eq!(node.value.as_deref(), Some("refreshing"));
    assert!(node.busy);
    assert!(
        harness.node("panel.veil").is_some(),
        "a refresh must veil the last verified value, not erase it"
    );
    assert!(
        harness.node("panel.stale").is_none(),
        "a refresh in flight is not a failed refresh"
    );
}

#[gpui::test]
fn an_unauthorized_empty_state_is_not_unavailable(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        EmptyState::new("workspace", "This workspace is locked")
            .kind(EmptyKind::Unauthorized)
            .into_any_element()
    });

    assert_eq!(
        harness
            .node("workspace")
            .expect("published")
            .value
            .as_deref(),
        Some("unauthorized")
    );
}

#[gpui::test]
fn a_paused_bar_is_not_busy(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ProgressBar::new("upload")
            .label("Uploading")
            .count(6, 12)
            .paused(true)
            .on_cancel(|_, _| {})
            .into_any_element()
    });

    let node = harness.node("upload").expect("published");
    assert!(!node.busy, "paused work must not claim it is still moving");
    assert!(harness.node("upload.cancel").is_some());
}

#[gpui::test]
fn a_banner_reports_its_dismissal(cx: &mut TestAppContext) {
    let dismissed = Rc::new(Cell::new(false));
    let sink = dismissed.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Banner::new("notice", "The last refresh failed.", Tone::Warning)
            .title("Stale")
            .on_dismiss(move |_, _| sink.set(true))
            .into_any_element()
    });

    harness.click("notice.dismiss");
    assert!(dismissed.get());
}

#[gpui::test]
fn an_outcome_panel_names_a_partial_success(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        OutcomePanel::new("import", OutcomeKind::Partial)
            .count("47 succeeded, 3 failed")
            .into_any_element()
    });

    assert_eq!(
        harness.node("import").expect("published").value.as_deref(),
        Some("partial")
    );
}

#[gpui::test]
fn a_stage_progress_publishes_each_stage(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        StageProgress::new("install")
            .stages([
                ProgressStage::new("download", "Download", StageStatus::Done),
                ProgressStage::new("verify", "Verify", StageStatus::Active),
            ])
            .into_any_element()
    });

    assert_eq!(
        harness.node("install").expect("published").value.as_deref(),
        Some("active")
    );
    assert!(harness.node("install").expect("published").busy);
    assert_eq!(
        harness
            .node("install.verify")
            .expect("published")
            .value
            .as_deref(),
        Some("active")
    );
}

#[gpui::test]
fn a_spinner_publishes_a_busy_progress_node(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        Spinner::new("wait")
            .label("Contacting host")
            .into_any_element()
    });

    let node = harness.node("wait").expect("published");
    assert!(node.busy);
}
