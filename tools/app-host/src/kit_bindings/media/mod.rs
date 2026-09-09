//! Pure media presentation. No transport, decoder, path, URL or process authority.
use super::{Emit, KitSlots, Node, number, text};
use gpui::{AnyElement, App, IntoElement, Window};
use gpui_kit::media::*;
use serde_json::{Value, json};

#[cfg(all(test, feature = "capture"))]
mod tests;

pub(super) const COMPONENTS: &[&str] =
    &["AudioWaveform", "AudioPlayer", "VideoPlayer", "ModelViewer"];

#[derive(Default)]
pub(super) struct State;
impl State {
    pub(super) fn reconcile(&self, _node: &Node, _cx: &mut App) {}
    pub(super) fn render(
        &self,
        node: &Node,
        slots: KitSlots,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        render(node, slots, window, cx, emit)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn invoke(
        &self,
        _node: &Node,
        _method: &str,
        _args: &Value,
        _query: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) -> anyhow::Result<Value> {
        anyhow::bail!(
            "media builders have no native commands or queries; resource authority unavailable"
        )
    }
}

pub(super) fn render(
    node: &Node,
    slots: KitSlots,
    _window: &mut Window,
    _cx: &mut App,
    emit: Emit,
) -> AnyElement {
    let peaks = || {
        node.props
            .get("peaks")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_f64)
            .map(|v| v as f32)
    };
    match node.component.as_deref().unwrap_or_default() {
        "AudioWaveform" => {
            let reason = text(node, "reason").into();
            let mut control = AudioWaveform::new(node.id.clone())
                .peaks(peaks())
                .state(match text(node, "state").as_str() {
                    "loading" => AudioWaveformState::Loading,
                    "empty" => AudioWaveformState::Empty,
                    "unavailable" => AudioWaveformState::Unavailable(reason),
                    "error" => AudioWaveformState::Error(reason),
                    _ => AudioWaveformState::Ready,
                });
            if node.props.contains_key("playhead") {
                control = control.playhead(number(node, "playhead", 0.));
            }
            super::content::slotted(control, slots).into_any_element()
        }
        "AudioPlayer" => {
            // Native AudioPlayer's no-transport state is genuinely unavailable.
            let mut control = AudioPlayer::new(node.id.clone())
                .title(text(node, "title"))
                .subtitle(text(node, "subtitle"))
                .peaks(peaks());
            if node.props.contains_key("elapsed") {
                control = control.elapsed(text(node, "elapsed"));
            }
            if node.props.contains_key("remaining") {
                control = control.remaining(text(node, "remaining"));
            }
            if node.props.contains_key("stepSeconds") {
                control = control.step_seconds(number(node, "stepSeconds", 5.));
            }
            if let Some(speeds) = node.props.get("speeds").and_then(Value::as_array) {
                control = control.speeds(speeds.iter().filter_map(Value::as_f64).map(|v| v as f32));
            }
            control.into_any_element()
        }
        "VideoPlayer" => {
            let mut control = VideoPlayer::new(node.id.clone()).title(text(node, "title"));
            if node.props.contains_key("ratio") {
                control = control.ratio(number(node, "ratio", 16. / 9.));
            }
            if node.props.contains_key("elapsed") {
                control = control.elapsed(text(node, "elapsed"));
            }
            if node.props.contains_key("remaining") {
                control = control.remaining(text(node, "remaining"));
            }
            if node.props.contains_key("stepSeconds") {
                control = control.step_seconds(number(node, "stepSeconds", 5.));
            }
            if let Some(speeds) = node.props.get("speeds").and_then(Value::as_array) {
                control = control.speeds(speeds.iter().filter_map(Value::as_f64).map(|v| v as f32));
            }
            if let Some(build) = slots.get("poster").cloned() {
                control = control.poster(move |window, cx| Some(build(window, cx)));
            }
            // A supplied element is not a decoded video frame or a playing transport.
            control.into_any_element()
        }
        "ModelViewer" => {
            let mut control = ModelViewer::new(node.id.clone())
                .title(text(node, "title"))
                .shading(if text(node, "shading") == "wireframe" {
                    ModelShading::Wireframe
                } else {
                    ModelShading::Flat
                })
                .orbit(number(node, "yaw", 0.), number(node, "pitch", 0.));
            if node.props.contains_key("height") {
                control = control.height(number(node, "height", 240.));
            }
            if let Some(document) = node.props.get("document").and_then(Value::as_str) {
                // Bounded pure parser rejects external references; never opens files.
                control = control.state(ModelState::read(
                    document.as_bytes(),
                    ModelBounds::default(),
                ));
            } else if text(node, "state") == "loading" {
                control = control.loading();
            }
            if let Some(action) = node.events.get("event").cloned() {
                control = control.on_event(move |event, _, _| {
                    emit(
                        &action,
                        match event {
                            ModelViewerEvent::OrbitChanged { yaw, pitch } => {
                                json!({"kind":"orbitChanged","yaw":yaw,"pitch":pitch})
                            }
                            ModelViewerEvent::ShadingChanged(shading) => {
                                json!({"kind":"shadingChanged","shading":shading.name()})
                            }
                        },
                    )
                });
            }
            control.into_any_element()
        }
        _ => unreachable!("validated media registration"),
    }
}
