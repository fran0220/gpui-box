//! Nothing here fetches an image and nothing here plays anything.
//!
//! The viewer draws what the host handed it, names what it did not, and
//! refuses to report a size nobody stated. The transport reports every
//! control and moves no head, keeps a duration nobody knows as a state rather
//! than a zero, and never calls waiting "paused".

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    IntoElement, Modifiers, ParentElement, Styled, TestAppContext, TouchPhase, div, point, px,
};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Sink<T> = Rc<RefCell<Vec<T>>>;

fn sink<T: 'static>() -> (Sink<T>, Sink<T>) {
    let calls: Sink<T> = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

fn gallery() -> Vec<ImageFrame> {
    vec![
        ImageFrame::new("graph", "The run graph")
            .source("runs/graph.png")
            .natural(1600, 900),
        ImageFrame::new("trace", "The failing trace")
            .source("runs/trace.png")
            .natural(1200, 1200),
    ]
}

/// A viewer whose host supplies every image it is asked for.
fn viewer(
    cx: &mut TestAppContext,
    frames: Vec<ImageFrame>,
    showing: &'static str,
    fit: FitMode,
    supply: bool,
) -> (Harness, Sink<ImageViewerEvent>) {
    let (calls, into) = sink::<ImageViewerEvent>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let into = into.clone();
        let frames = frames.clone();
        div()
            .w(px(400.0))
            .child(
                ImageViewer::new("viewer", frames)
                    .showing(showing)
                    .fit(fit)
                    .height(300.0)
                    .image(move |_, _, _| supply.then(|| div().size_full().into_any_element()))
                    .on_event(move |event, _, _| into.borrow_mut().push(event.clone())),
            )
            .into_any_element()
    });
    // The frame is measured during prepaint, so the scale it implies exists
    // from the frame after the first one.
    harness.frame();
    (harness, calls)
}

#[gpui::test]
fn a_supplied_image_is_ready_and_states_the_size_the_host_gave(cx: &mut TestAppContext) {
    let (mut harness, _events) = viewer(cx, gallery(), "graph", FitMode::Contain, true);

    let frame = harness.node("viewer.frame").expect("published");
    assert_eq!(frame.role, Role::Image);
    assert_eq!(frame.text.as_deref(), Some("The run graph"));
    assert_eq!(frame.value.as_deref(), Some("ready"));

    let measurement = harness.node("viewer.measurement").expect("published");
    assert!(
        measurement
            .text
            .as_deref()
            .is_some_and(|text| text.starts_with("1600 × 900")),
        "the host's own dimensions, not the box it was drawn in: {:?}",
        measurement.text
    );
}

#[gpui::test]
fn a_host_that_could_not_supply_an_image_is_shown_as_a_refusal(cx: &mut TestAppContext) {
    let refused = vec![
        ImageFrame::new("scan", "Page 4 of the scan")
            .source("scans/page-4.tiff")
            .natural(2480, 3508)
            .unavailable("The workspace is frozen for the release."),
    ];
    let (mut harness, _events) = viewer(cx, refused, "scan", FitMode::Contain, true);

    let frame = harness.node("viewer.frame").expect("published");
    assert_eq!(
        frame.value.as_deref(),
        Some("unavailable"),
        "a refusal must not be rendered as an empty frame"
    );
    assert!(!frame.invalid, "a refusal is not a decode failure");
    assert!(frame.visible && frame.bounds.height > 0.0);
}

#[gpui::test]
fn an_image_that_failed_to_decode_is_published_as_invalid(cx: &mut TestAppContext) {
    let broken = vec![
        ImageFrame::new("scan", "Page 4 of the scan")
            .source("scans/page-4.tiff")
            .natural(2480, 3508)
            .failed("The file ends in the middle of a scanline."),
    ];
    let (mut harness, _events) = viewer(cx, broken, "scan", FitMode::Contain, true);

    let frame = harness.node("viewer.frame").expect("published");
    assert_eq!(frame.value.as_deref(), Some("failed"));
    assert!(frame.invalid);
}

#[gpui::test]
fn a_loading_image_is_neither_missing_nor_refused(cx: &mut TestAppContext) {
    let waiting = vec![
        ImageFrame::new("graph", "The run graph")
            .source("runs/graph.png")
            .loading(),
    ];
    let (mut harness, events) = viewer(cx, waiting, "graph", FitMode::Contain, true);

    let frame = harness.node("viewer.frame").expect("published");
    assert_eq!(frame.value.as_deref(), Some("loading"));
    assert!(frame.busy);
    assert!(
        events.borrow().is_empty(),
        "an image the host is still fetching has not been asked for again"
    );
}

#[gpui::test]
fn an_unsupplied_image_is_named_and_requested_once(cx: &mut TestAppContext) {
    let (mut harness, events) = viewer(cx, gallery(), "graph", FitMode::Contain, false);
    harness.frame();
    harness.frame();

    assert_eq!(
        harness
            .node("viewer.frame")
            .expect("published")
            .value
            .as_deref(),
        Some("ready"),
    );
    let requests: Vec<ImageViewerEvent> = events
        .borrow()
        .iter()
        .filter(|event| matches!(event, ImageViewerEvent::ImageRequested(_)))
        .cloned()
        .collect();
    assert_eq!(
        requests.len(),
        1,
        "a host that answers a request must not be asked again every frame"
    );
    let ImageViewerEvent::ImageRequested(request) = &requests[0] else {
        panic!("expected a request");
    };
    assert_eq!(request.id.as_ref(), "graph");
    assert_eq!(request.source.as_ref(), "runs/graph.png");
}

#[gpui::test]
fn a_source_nobody_measured_says_the_size_is_unknown(cx: &mut TestAppContext) {
    let unmeasured = vec![ImageFrame::new("sketch", "A pasted sketch").source("clipboard")];
    let (mut harness, _events) = viewer(cx, unmeasured, "sketch", FitMode::Contain, true);

    assert_eq!(
        harness
            .node("viewer.measurement")
            .expect("published")
            .text
            .as_deref(),
        Some("Size unknown"),
        "the rendered size is not the source's size"
    );
    assert!(
        harness
            .node("viewer.fit.contain")
            .expect("published")
            .disabled,
        "a scale against a size nobody stated cannot be offered"
    );
}

#[gpui::test]
fn a_fit_control_reports_the_mode_and_changes_nothing_itself(cx: &mut TestAppContext) {
    let (mut harness, events) = viewer(cx, gallery(), "graph", FitMode::Contain, true);
    let before = harness
        .node("viewer.measurement")
        .expect("published")
        .text
        .clone();

    harness.click("viewer.fit.cover");

    assert_eq!(
        *events.borrow(),
        vec![ImageViewerEvent::FitChanged(FitMode::Cover)]
    );
    assert_eq!(
        harness
            .node("viewer.measurement")
            .expect("published")
            .text
            .clone(),
        before,
        "the viewer draws the fit the caller says holds, not the one asked for"
    );
}

#[gpui::test]
fn the_wheel_zooms_and_reports_a_scale_rather_than_applying_one(cx: &mut TestAppContext) {
    let (mut harness, events) = viewer(cx, gallery(), "graph", FitMode::Actual, true);
    let over = harness.point_across("viewer.frame", 0.25);

    harness.context().simulate_event(gpui::ScrollWheelEvent {
        position: over,
        delta: gpui::ScrollDelta::Lines(point(0.0, 1.0)),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });

    let zoomed: Vec<f32> = events
        .borrow()
        .iter()
        .filter_map(|event| match event {
            ImageViewerEvent::FitChanged(FitMode::Zoom(scale)) => Some(*scale),
            _ => None,
        })
        .collect();
    assert_eq!(zoomed.len(), 1, "one notch, one report");
    assert!(zoomed[0] > 1.0, "a notch forward zooms in: {:?}", zoomed);
    assert!(
        harness
            .node("viewer.measurement")
            .expect("published")
            .text
            .as_deref()
            .is_some_and(|text| text.ends_with("100%")),
        "the caller has not applied the zoom, so the viewer still draws 1:1"
    );
}

#[gpui::test]
fn a_zoom_never_leaves_the_range_the_caller_set(cx: &mut TestAppContext) {
    let (calls, into) = sink::<ImageViewerEvent>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let into = into.clone();
        div()
            .w(px(400.0))
            .child(
                ImageViewer::new("viewer", gallery())
                    .showing("graph")
                    .fit(FitMode::Actual)
                    .zoom_range(0.5, 1.1)
                    .height(300.0)
                    .image(|_, _, _| Some(div().size_full().into_any_element()))
                    .on_event(move |event, _, _| into.borrow_mut().push(event.clone())),
            )
            .into_any_element()
    });
    harness.frame();
    let over = harness.point_across("viewer.frame", 0.5);

    for _ in 0..4 {
        harness.context().simulate_event(gpui::ScrollWheelEvent {
            position: over,
            delta: gpui::ScrollDelta::Lines(point(0.0, 1.0)),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
    }

    for event in calls.borrow().iter() {
        if let ImageViewerEvent::FitChanged(FitMode::Zoom(scale)) = event {
            assert!(
                (0.5..=1.1).contains(scale),
                "reported a scale outside the caller's range: {scale}"
            );
        }
    }
}

#[gpui::test]
fn stepping_reports_the_image_by_its_own_id(cx: &mut TestAppContext) {
    let (mut harness, events) = viewer(cx, gallery(), "graph", FitMode::Contain, true);

    assert_eq!(
        harness
            .node("viewer.position")
            .expect("published")
            .text
            .as_deref(),
        Some("1 of 2")
    );
    harness.click("viewer.next");

    assert_eq!(
        *events.borrow(),
        vec![ImageViewerEvent::Stepped { id: "trace".into() }]
    );
}

#[gpui::test]
fn the_end_of_the_gallery_is_refused_rather_than_wrapped(cx: &mut TestAppContext) {
    let (mut harness, events) = viewer(cx, gallery(), "trace", FitMode::Contain, true);

    let next = harness.node("viewer.next").expect("published");
    assert!(next.disabled, "there is nothing after the last image");
    harness.click("viewer.next");
    assert!(
        events.borrow().is_empty(),
        "a control with nowhere to go installs no handler at all"
    );
    assert_eq!(
        harness
            .node("viewer.position")
            .expect("published")
            .text
            .as_deref(),
        Some("2 of 2"),
        "where the reader is, is published rather than left to be counted"
    );
}

// -- the transport ----------------------------------------------------------

fn clip(
    cx: &mut TestAppContext,
    build: impl Fn(TransportBar) -> TransportBar + 'static,
) -> (Harness, Sink<TransportEvent>) {
    let (calls, into) = sink::<TransportEvent>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let into = into.clone();
        div()
            .w(px(560.0))
            .child(
                build(TransportBar::new("bar").label("Release walkthrough"))
                    .on_event(move |event, _, _| into.borrow_mut().push(event.clone())),
            )
            .into_any_element()
    });
    (harness, calls)
}

fn playing(bar: TransportBar) -> TransportBar {
    bar.state(TransportState::Playing)
        .position(72.0)
        .duration(240.0)
        .elapsed("01:12")
        .remaining("-02:48")
        .step_seconds(10.0)
        .volume(0.7)
}

#[gpui::test]
fn the_scrubber_publishes_the_position_the_caller_says_holds(cx: &mut TestAppContext) {
    let (mut harness, _events) = clip(cx, playing);

    let scrubber = harness.node("bar.scrubber").expect("published");
    assert_eq!(scrubber.role, Role::Slider);
    assert_eq!(scrubber.value_min, Some(0.0));
    assert_eq!(scrubber.value_max, Some(240.0));
    assert_eq!(scrubber.value_now, Some(72.0));
    assert_eq!(
        scrubber.value.as_deref(),
        Some("01:12"),
        "the readout is the host's own string"
    );
}

#[gpui::test]
fn asking_to_pause_pauses_nothing(cx: &mut TestAppContext) {
    let (mut harness, events) = clip(cx, playing);

    harness.click("bar.pause");

    assert_eq!(*events.borrow(), vec![TransportEvent::PauseRequested]);
    assert_eq!(
        harness.node("bar").expect("published").value.as_deref(),
        Some("playing"),
        "the transport draws what the caller says is true"
    );
}

#[gpui::test]
fn a_stalled_transport_says_it_is_waiting_rather_than_paused(cx: &mut TestAppContext) {
    let (mut harness, events) = clip(cx, |bar| playing(bar).state(TransportState::Buffering));

    assert_eq!(
        harness.node("bar").expect("published").value.as_deref(),
        Some("buffering")
    );
    assert!(harness.node("bar").expect("published").busy);
    assert_eq!(
        harness
            .node("bar.status")
            .expect("published")
            .text
            .as_deref(),
        Some("Waiting for data")
    );
    assert!(
        harness.node("bar.play").is_none(),
        "nothing has stopped, so nothing offers to resume"
    );

    harness.click("bar.pause");
    assert_eq!(
        *events.borrow(),
        vec![TransportEvent::PauseRequested],
        "a stalled transport is still playing, so the control that stops it is the one offered"
    );
}

#[gpui::test]
fn a_duration_nobody_knows_draws_no_track_fraction(cx: &mut TestAppContext) {
    let (mut harness, events) = clip(cx, |bar| {
        bar.state(TransportState::Playing)
            .position(1543.0)
            .unknown_duration()
            .elapsed("25:43")
    });

    let scrubber = harness.node("bar.scrubber").expect("published");
    assert_eq!(
        scrubber.role,
        Role::Status,
        "there is no range to slide along"
    );
    assert_eq!(scrubber.text.as_deref(), Some("Duration unknown"));
    assert_eq!(scrubber.value.as_deref(), Some("25:43"));
    assert_eq!(scrubber.value_now, None);

    harness.drag_start("bar.scrubber");
    harness.drop_here();
    assert!(
        events
            .borrow()
            .iter()
            .all(|event| !matches!(event, TransportEvent::SeekRequested(_))),
        "a stream with no total has no position to scrub to"
    );
}

#[gpui::test]
fn a_scrub_previews_continuously_and_commits_once(cx: &mut TestAppContext) {
    let (mut harness, events) = clip(cx, playing);

    let quarter = harness.point_across("bar.scrubber", 0.25);
    let half = harness.point_across("bar.scrubber", 0.5);
    harness.drag_start("bar.scrubber");
    harness.drag_to(quarter);
    harness.drag_to(half);
    harness.drop_here();

    let previews: Vec<f32> = events
        .borrow()
        .iter()
        .filter_map(|event| match event {
            TransportEvent::SeekPreview(seconds) => Some(*seconds),
            _ => None,
        })
        .collect();
    let seeks: Vec<f32> = events
        .borrow()
        .iter()
        .filter_map(|event| match event {
            TransportEvent::SeekRequested(seconds) => Some(*seconds),
            _ => None,
        })
        .collect();

    assert!(
        previews.len() > 1,
        "a preview per move is what lets a host show a frame: {previews:?}"
    );
    assert_eq!(seeks.len(), 1, "one seek per gesture: {seeks:?}");
    assert!((seeks[0] - 120.0).abs() < 1.0, "{seeks:?}");
    assert_eq!(
        harness
            .node("bar.scrubber")
            .expect("published")
            .value_now
            .map(|seconds| seconds.round()),
        Some(72.0),
        "the head is where the caller says it is, not where the pointer went"
    );
}

#[gpui::test]
fn buffered_ranges_are_published_apart_from_the_position(cx: &mut TestAppContext) {
    let (mut harness, _events) = clip(cx, |bar| {
        playing(bar).buffered([BufferedRange::new(0.0, 156.0)])
    });

    let buffered = harness.node("bar.buffered").expect("published");
    assert_eq!(buffered.value.as_deref(), Some("1 ranges"));
    assert_eq!(
        buffered.value_now.map(|share| (share * 100.0).round()),
        Some(65.0)
    );
    assert_ne!(
        buffered.value_now,
        harness.node("bar.scrubber").expect("published").value_now,
        "how much is held and how far it has played are two facts"
    );
}

#[gpui::test]
fn a_host_that_supplied_no_buffer_shows_none(cx: &mut TestAppContext) {
    let (mut harness, _events) = clip(cx, playing);

    assert!(
        harness.node("bar.buffered").is_none(),
        "no ranges means no buffer, never a full one"
    );
}

#[gpui::test]
fn a_transport_formats_no_times_of_its_own(cx: &mut TestAppContext) {
    let (mut harness, _events) = clip(cx, |bar| {
        bar.state(TransportState::Paused)
            .position(72.0)
            .duration(240.0)
    });

    assert_eq!(
        harness
            .node("bar.scrubber")
            .expect("published")
            .value
            .as_deref(),
        Some("Time unknown"),
        "a time the host did not write is not written here"
    );
}

#[gpui::test]
fn volume_mute_and_speed_all_report_and_apply_nothing(cx: &mut TestAppContext) {
    let (mut harness, events) = clip(cx, |bar| playing(bar).speeds([1.0, 1.5, 2.0], 1.0));

    harness.click("bar.mute");
    harness.click("bar.speed.speed-1-5");
    harness.click("bar.volume");

    let reported = events.borrow().clone();
    assert!(reported.contains(&TransportEvent::MuteToggled));
    assert!(reported.contains(&TransportEvent::SpeedRequested(1.5)));
    assert!(
        reported
            .iter()
            .any(|event| matches!(event, TransportEvent::VolumeRequested(_))),
    );
    assert_eq!(
        harness
            .node("bar.volume")
            .expect("published")
            .value
            .as_deref(),
        Some("70%"),
        "the volume drawn is the caller's, not the one asked for"
    );
    assert!(
        !harness.node("bar.mute").expect("published").selected,
        "muting is the host's to apply"
    );
}

#[gpui::test]
fn a_track_control_with_nowhere_to_go_installs_no_handler(cx: &mut TestAppContext) {
    let (mut harness, events) = clip(cx, |bar| playing(bar).has_next(true));

    assert!(harness.node("bar.previous").expect("published").disabled);
    harness.click("bar.previous");
    assert!(events.borrow().is_empty());

    harness.click("bar.next");
    assert_eq!(
        *events.borrow(),
        vec![TransportEvent::Stepped(TrackStep::Next)]
    );
}

#[gpui::test]
fn a_disabled_transport_reports_nothing_at_all(cx: &mut TestAppContext) {
    let (mut harness, events) = clip(cx, |bar| playing(bar).disabled(true));

    harness.click("bar.pause");
    harness.click("bar.mute");

    assert!(events.borrow().is_empty());
    assert!(harness.node("bar").expect("published").disabled);
}

#[gpui::test]
fn the_keyboard_zooms_and_resets_through_the_same_report(cx: &mut TestAppContext) {
    let (mut harness, events) = viewer(cx, gallery(), "graph", FitMode::Actual, true);

    // Clicking inside the viewer puts the keyboard on it. The step control is
    // the nearest focusable thing and it does not answer to `+` itself.
    harness.click("viewer.next");
    events.borrow_mut().clear();
    harness.keystrokes("+");
    harness.keystrokes("0");

    assert_eq!(
        *events.borrow(),
        vec![
            ImageViewerEvent::FitChanged(FitMode::Zoom(1.25)),
            ImageViewerEvent::FitChanged(FitMode::Contain),
        ]
    );
}

#[gpui::test]
fn space_toggles_and_an_arrow_steps_by_the_callers_own_step(cx: &mut TestAppContext) {
    let (mut harness, events) = clip(cx, playing);

    // The status label is inside the bar and is not a control, so the
    // keyboard lands on the bar itself rather than on a button that would
    // answer to the space bar before the transport saw it.
    harness.click("bar.status");
    events.borrow_mut().clear();
    harness.keystrokes("space");
    harness.keystrokes("right");
    harness.keystrokes("left");

    assert_eq!(
        *events.borrow(),
        vec![
            TransportEvent::PauseRequested,
            TransportEvent::SeekRequested(82.0),
            TransportEvent::SeekRequested(62.0),
        ]
    );
}
