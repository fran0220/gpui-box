//! Nothing here plays anything, and every surface says so.
//!
//! The players ask a transport and report what it answered: a refused command
//! leaves the state that still holds, a machine with no backend is not a
//! machine that is loading, and a fixture is published as a fixture. The model
//! viewer reads a document inside a fence, publishes what it counted, and
//! names what it refused.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    IntoElement, Modifiers, MouseButton, ParentElement, Styled, TestAppContext, div, point, px,
};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

/// The cube the model tests read, with its buffer inside the document.
const CUBE: &[u8] = include_bytes!("../../../fixtures/models/cube.gltf");

type Sink<T> = Rc<RefCell<Vec<T>>>;

fn sink<T: 'static>() -> (Sink<T>, Sink<T>) {
    let calls: Sink<T> = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

/// An audio player over whatever transport the case supplies.
fn audio(
    cx: &mut TestAppContext,
    transport: Option<Rc<dyn MediaTransport>>,
    peaks: Vec<f32>,
) -> (Harness, Sink<MediaEvent>) {
    let (calls, into) = sink::<MediaEvent>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let into = into.clone();
        let mut player = AudioPlayer::new("player")
            .title("Release walkthrough")
            .elapsed("01:12")
            .peaks(peaks.clone())
            .on_event(move |event, _, _| into.borrow_mut().push(event.clone()));
        if let Some(transport) = transport.clone() {
            player = player.transport(transport);
        }
        div().w(px(520.0)).child(player).into_any_element()
    });
    harness.frame();
    (harness, calls)
}

#[gpui::test]
fn a_player_with_no_transport_offers_no_controls(cx: &mut TestAppContext) {
    let (mut harness, _events) = audio(cx, None, Vec::new());

    let player = harness.node("player").expect("published");
    assert_eq!(player.role, Role::Group);
    assert_eq!(
        player.value.as_deref(),
        Some("no-transport"),
        "no player connected is not a player that has not started"
    );
    assert!(
        harness.node("player.transport").is_none(),
        "a transport bar here would say playback exists and has not begun"
    );
    assert!(
        harness.node("player.origin").is_none(),
        "there is no transport, so there is no origin to state"
    );
}

#[gpui::test]
fn a_fixture_transport_is_published_as_a_fixture(cx: &mut TestAppContext) {
    let (mut harness, _events) = audio(
        cx,
        Some(FixtureTransport::ready(240.0).shared()),
        Vec::new(),
    );

    let origin = harness.node("player.origin").expect("published");
    assert_eq!(origin.value.as_deref(), Some("fixture"));
    assert_eq!(
        harness.node("player").expect("published").value.as_deref(),
        Some("ready")
    );
}

#[gpui::test]
fn a_machine_that_cannot_play_it_is_not_a_machine_that_is_loading(cx: &mut TestAppContext) {
    let refused = FixtureTransport::new()
        .no_backend("There is no Opus decoder on this machine.")
        .shared();
    let (mut harness, _events) = audio(cx, Some(refused), Vec::new());

    let player = harness.node("player").expect("published");
    assert_eq!(player.value.as_deref(), Some("no-backend"));
    assert!(!player.busy, "nothing is being waited for");
    assert!(!player.invalid, "a missing backend is not a broken file");
    assert!(
        harness.node("player.transport").is_none(),
        "there is nothing to move, so there is no scrubber"
    );
}

#[gpui::test]
fn media_that_could_not_be_read_is_published_as_invalid(cx: &mut TestAppContext) {
    let broken = FixtureTransport::new()
        .failed("The file ends in the middle of a packet.")
        .shared();
    let (mut harness, _events) = audio(cx, Some(broken), Vec::new());

    let player = harness.node("player").expect("published");
    assert_eq!(player.value.as_deref(), Some("failed"));
    assert!(player.invalid);
}

#[gpui::test]
fn a_control_asks_the_transport_and_reports_what_it_answered(cx: &mut TestAppContext) {
    let fixture = Rc::new(FixtureTransport::ready(240.0).position(72.0));
    let (mut harness, events) = audio(cx, Some(fixture.clone()), Vec::new());

    harness.click("player.transport.play");
    harness.frame();

    assert_eq!(
        *events.borrow(),
        vec![MediaEvent::Applied(MediaCommand::Play)]
    );
    assert_eq!(fixture.commands(), vec![MediaCommand::Play]);
    assert_eq!(
        harness
            .node("player.transport")
            .expect("published")
            .value
            .as_deref(),
        Some("playing"),
        "the bar draws the transport's snapshot, which the command changed"
    );
    assert_eq!(
        fixture.snapshot().position,
        72.0,
        "playing a fixture decodes nothing, so the head does not move"
    );
}

#[gpui::test]
fn a_refused_command_is_reported_as_a_refusal_and_moves_nothing(cx: &mut TestAppContext) {
    let fixture = Rc::new(
        FixtureTransport::ready(240.0)
            .position(72.0)
            .refusing("The output device is in use."),
    );
    let (mut harness, events) = audio(cx, Some(fixture.clone()), Vec::new());

    harness.click("player.transport.play");
    harness.frame();

    assert_eq!(
        *events.borrow(),
        vec![MediaEvent::Refused(
            MediaCommand::Play,
            "The output device is in use.".into()
        )]
    );
    assert_eq!(
        harness
            .node("player.transport")
            .expect("published")
            .value
            .as_deref(),
        Some("paused"),
        "a refusal must leave the state that still holds"
    );
}

#[gpui::test]
fn a_player_draws_no_waveform_it_did_not_measure(cx: &mut TestAppContext) {
    let (mut harness, _events) = audio(
        cx,
        Some(FixtureTransport::ready(240.0).shared()),
        Vec::new(),
    );
    assert!(
        harness.node("player.peaks").is_none(),
        "an invented envelope would be a picture of nothing"
    );

    let (mut measured, _events) = audio(
        cx,
        Some(FixtureTransport::ready(240.0).shared()),
        vec![0.2, 0.9, 0.4, 0.7],
    );
    let peaks = measured.node("player.peaks").expect("published");
    assert_eq!(peaks.role, Role::Image);
    assert_eq!(peaks.value.as_deref(), Some("4"));
}

/// A video player over whatever transport and pictures the case supplies.
fn video(
    cx: &mut TestAppContext,
    transport: Option<Rc<dyn MediaTransport>>,
    frames: bool,
    poster: bool,
) -> (Harness, Sink<MediaEvent>) {
    let (calls, into) = sink::<MediaEvent>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let into = into.clone();
        let mut player = VideoPlayer::new("clip")
            .title("Screen capture")
            .on_event(move |event, _, _| into.borrow_mut().push(event.clone()));
        if let Some(transport) = transport.clone() {
            player = player.transport(transport);
        }
        if frames {
            player = player.frame(|_, _| Some(div().size_full().into_any_element()));
        }
        if poster {
            player = player.poster(|_, _| Some(div().size_full().into_any_element()));
        }
        div().w(px(520.0)).child(player).into_any_element()
    });
    harness.frame();
    (harness, calls)
}

#[gpui::test]
fn a_supplied_frame_is_published_as_a_frame(cx: &mut TestAppContext) {
    let (mut harness, _events) =
        video(cx, Some(FixtureTransport::ready(96.0).shared()), true, true);

    let surface = harness.node("clip.surface").expect("published");
    assert_eq!(surface.role, Role::Image);
    assert_eq!(surface.value.as_deref(), Some("frame"));
}

#[gpui::test]
fn a_poster_is_never_published_as_a_moving_picture(cx: &mut TestAppContext) {
    let (mut harness, _events) = video(
        cx,
        Some(FixtureTransport::ready(96.0).shared()),
        false,
        true,
    );

    assert_eq!(
        harness
            .node("clip.surface")
            .expect("published")
            .value
            .as_deref(),
        Some("poster"),
        "a still standing in for playback is the one lie this must not tell"
    );
}

#[gpui::test]
fn a_video_nothing_can_decode_shows_the_refusal_over_its_poster(cx: &mut TestAppContext) {
    let absent = FixtureTransport::new()
        .no_backend("There is no AV1 decoder on this machine.")
        .shared();
    let (mut harness, _events) = video(cx, Some(absent), true, true);

    let player = harness.node("clip").expect("published");
    assert_eq!(player.value.as_deref(), Some("no-backend"));
    assert_eq!(
        harness
            .node("clip.surface")
            .expect("published")
            .value
            .as_deref(),
        Some("poster"),
        "frames are not asked for from a transport that cannot open the media"
    );
    let bar = harness.node("clip.transport").expect("published");
    assert!(bar.disabled, "there is nothing to move");
}

#[gpui::test]
fn a_video_with_no_transport_offers_no_controls(cx: &mut TestAppContext) {
    let (mut harness, _events) = video(cx, None, false, false);

    assert_eq!(
        harness.node("clip").expect("published").value.as_deref(),
        Some("no-transport")
    );
    assert!(harness.node("clip.transport").is_none());
    assert_eq!(
        harness
            .node("clip.surface")
            .expect("published")
            .value
            .as_deref(),
        Some("none")
    );
}

/// A model viewer over whatever state the case supplies.
fn model(cx: &mut TestAppContext, state: ModelState) -> (Harness, Sink<ModelViewerEvent>) {
    let (calls, into) = sink::<ModelViewerEvent>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let into = into.clone();
        div()
            .w(px(520.0))
            .child(
                ModelViewer::new("model")
                    .title("cube.gltf")
                    .state(state.clone())
                    .orbit(0.0, 0.0)
                    .height(240.0)
                    .on_event(move |event, _, _| into.borrow_mut().push(*event)),
            )
            .into_any_element()
    });
    harness.frame();
    (harness, calls)
}

#[gpui::test]
fn a_model_the_reader_accepted_publishes_what_it_counted(cx: &mut TestAppContext) {
    let (mut harness, _events) = model(cx, ModelState::read(CUBE, ModelBounds::default()));

    let viewer = harness.node("model").expect("published");
    assert_eq!(viewer.value.as_deref(), Some("ready"));
    assert!(!viewer.invalid);
    assert_eq!(
        harness
            .node("model.meshes")
            .expect("published")
            .value
            .as_deref(),
        Some("1")
    );
    assert_eq!(
        harness
            .node("model.vertices")
            .expect("published")
            .value
            .as_deref(),
        Some("8")
    );
    assert_eq!(
        harness
            .node("model.triangles")
            .expect("published")
            .value
            .as_deref(),
        Some("12")
    );
}

#[gpui::test]
fn a_document_past_a_bound_is_refused_and_publishes_no_counts(cx: &mut TestAppContext) {
    let bounds = ModelBounds::default().max_vertices(4);
    let (mut harness, _events) = model(cx, ModelState::read(CUBE, bounds));

    let viewer = harness.node("model").expect("published");
    assert_eq!(viewer.value.as_deref(), Some("too-large"));
    assert!(viewer.invalid);
    assert!(
        harness.node("model.triangles").is_none(),
        "a refusal counts nothing, and three zeroes would be three claims"
    );
}

#[gpui::test]
fn a_document_outside_the_subset_is_refused_rather_than_approximated(cx: &mut TestAppContext) {
    let (mut harness, _events) = model(
        cx,
        ModelState::read(b"<not a model>", ModelBounds::default()),
    );

    assert_eq!(
        harness.node("model").expect("published").value.as_deref(),
        Some("rejected")
    );
}

#[gpui::test]
fn an_empty_viewer_is_neither_a_refusal_nor_a_model(cx: &mut TestAppContext) {
    let (mut harness, _events) = model(cx, ModelState::Empty);

    let viewer = harness.node("model").expect("published");
    assert_eq!(viewer.value.as_deref(), Some("empty"));
    assert!(!viewer.invalid);
    assert!(harness.node("model.meshes").is_none());
    assert!(
        harness.node("model.shading").expect("published").disabled,
        "there is nothing to shade"
    );
}

#[gpui::test]
fn a_drag_reports_the_angles_it_asks_for_and_turns_nothing(cx: &mut TestAppContext) {
    let (mut harness, events) = model(cx, ModelState::read(CUBE, ModelBounds::default()));
    let before = harness
        .node("model.camera")
        .expect("published")
        .value
        .clone();
    let start = harness.point_in("model.frame");

    harness.context().simulate_event(gpui::MouseDownEvent {
        position: start,
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    harness.context().simulate_event(gpui::MouseMoveEvent {
        position: start + point(px(60.0), px(0.0)),
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    });
    harness.context().run_until_parked();

    let reported: Vec<f32> = events
        .borrow()
        .iter()
        .filter_map(|event| match event {
            ModelViewerEvent::OrbitChanged { yaw, .. } => Some(*yaw),
            _ => None,
        })
        .collect();
    assert_eq!(reported.len(), 1, "one move, one report");
    assert!(
        reported[0] > 0.0,
        "a drag to the right turns it: {reported:?}"
    );
    assert_eq!(
        harness.node("model.camera").expect("published").value,
        before,
        "the caller has not applied the angle, so the model has not turned"
    );
}

#[gpui::test]
fn the_shading_control_reports_and_does_not_switch_itself(cx: &mut TestAppContext) {
    let (mut harness, events) = model(cx, ModelState::read(CUBE, ModelBounds::default()));

    harness.click("model.shading.wireframe");

    assert_eq!(
        *events.borrow(),
        vec![ModelViewerEvent::ShadingChanged(ModelShading::Wireframe)]
    );
    assert_eq!(
        harness
            .node("model.shading.flat")
            .expect("published")
            .checked,
        Some(true),
        "the viewer draws the shading the caller says holds"
    );
}
