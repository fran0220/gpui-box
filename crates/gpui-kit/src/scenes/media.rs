//! Time-based and dimensional media.

use super::support::*;

/// The cube the model scenes read: a glTF document with its own buffer
/// inside it, so the reader is exercised on the path a host would take and
/// nothing is fetched to draw a scene.
pub(super) const CUBE: &[u8] = include_bytes!("../../assets/models/cube.gltf");

/// A measured envelope, in the shape a host that read the samples would hand
/// over. It is written down rather than sampled, because a scene that drew a
/// different waveform every run could not be a visual baseline.
pub(super) fn scene_peaks(count: usize) -> Vec<f32> {
    (0..count)
        .map(|index| {
            let phase = index as f32 / count as f32;
            let swell = (phase * 11.0).sin() * 0.32 + (phase * 3.0).cos() * 0.28;
            (0.42 + swell).clamp(0.05, 1.0)
        })
        .collect()
}

pub(super) fn audio_player(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(640.0))
        .child(
            AudioPlayer::new("scene.audio.ready")
                .title("Release walkthrough")
                .subtitle("Recorded 12 March")
                .transport(
                    FixtureTransport::ready(240.0)
                        .position(72.0)
                        .state(TransportState::Playing)
                        .volume(0.7)
                        .buffered([BufferedRange::new(0.0, 156.0)])
                        .shared(),
                )
                .elapsed("01:12")
                .remaining("-02:48")
                .peaks(scene_peaks(96))
                .speeds([1.0, 1.5, 2.0])
                .step_seconds(10.0)
                .on_event(|_, _, _| {}),
        )
        .child(
            Divider::new()
                .id("scene.audio.rule.backend")
                .label("Nothing here can decode it"),
        )
        .child(
            AudioPlayer::new("scene.audio.absent")
                .title("interview.opus")
                .transport(
                    FixtureTransport::new()
                        .no_backend("There is no Opus decoder on this machine.")
                        .shared(),
                )
                .on_event(|_, _, _| {}),
        )
        .child(
            Divider::new()
                .id("scene.audio.rule.none")
                .label("No player connected at all"),
        )
        .child(
            AudioPlayer::new("scene.audio.none")
                .title("Standup recording")
                .on_event(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn video_player(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(440.0))
        .child(
            VideoPlayer::new("scene.video.frame")
                .title("Screen capture")
                .transport(
                    FixtureTransport::ready(96.0)
                        .position(24.0)
                        .state(TransportState::Playing)
                        .volume(0.6)
                        .shared(),
                )
                .frame(|_, cx| Some(scene_picture("Supplied frame", cx)))
                .elapsed("00:24")
                .remaining("-01:12")
                .on_event(|_, _, _| {}),
        )
        .child(
            Divider::new()
                .id("scene.video.rule.poster")
                .label("Ready, and no frames supplied"),
        )
        .child(
            VideoPlayer::new("scene.video.poster")
                .title("Release walkthrough")
                .transport(FixtureTransport::ready(240.0).shared())
                .poster(|_, cx| Some(scene_picture("Poster", cx)))
                .elapsed("00:00")
                .remaining("-04:00")
                .on_event(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn model_viewer(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let read = || ModelState::read(CUBE, ModelBounds::default());
    stack(&theme)
        .w(px(720.0))
        .child(
            div()
                .row()
                .items_start()
                .gap(px(theme.spacing.lg))
                .child(
                    div().flex_1().min_w_0().child(
                        ModelViewer::new("scene.model.flat")
                            .title("cube.gltf")
                            .state(read())
                            .orbit(0.62, 0.42)
                            .height(240.0)
                            .on_event(|_, _, _| {}),
                    ),
                )
                .child(
                    div().flex_1().min_w_0().child(
                        ModelViewer::new("scene.model.wireframe")
                            .title("cube.gltf")
                            .state(read())
                            .shading(ModelShading::Wireframe)
                            .orbit(0.62, 0.42)
                            .height(240.0)
                            .on_event(|_, _, _| {}),
                    ),
                ),
        )
        .child(
            Divider::new()
                .id("scene.model.rule.refused")
                .label("A document past a bound"),
        )
        .child(
            ModelViewer::new("scene.model.refused")
                .title("city-block.glb")
                .rejected(ModelError::TooLarge {
                    limit: ModelLimit::Triangles,
                    found: 412_930,
                    allowed: 24_576,
                })
                .height(160.0)
                .on_event(|_, _, _| {}),
        )
        .into_any_element()
}
