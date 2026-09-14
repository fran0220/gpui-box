//! Metric cards retain caller text inside the width their host assigned.

use gpui::{TestAppContext, div, prelude::*, px};
use gpui_kit::display::{
    badge::Tone,
    metric::{MetricCard, MetricReading, MetricState},
};
use gpui_kit_testkit::harness::Harness;

fn measured_card(
    cx: &mut TestAppContext,
    width: f32,
    state: MetricState,
    label: &str,
) -> (f32, f32) {
    let label = label.to_string();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(width))
            .child(MetricCard::new("metric", label.clone(), state.clone()))
            .into_any_element()
    });
    let node = harness.node("metric").expect("metric card is published");
    (node.bounds.width, node.bounds.height)
}

#[gpui::test]
fn long_metric_text_wraps_inside_a_240_pixel_host(cx: &mut TestAppContext) {
    let cases = [
        (
            "stale reason",
            "Tokens",
            MetricState::Stale {
                reading: MetricReading::new("12.4k"),
                reason: "Gateway refresh failed while offline; showing the last verified reading until a connection is restored".into(),
            },
        ),
        (
            "unbroken unavailable reason",
            "Tokens",
            MetricState::Unavailable(
                "gatewayadminofflinewithoutabreakortruncationmarker".into(),
            ),
        ),
        (
            "multilingual error reason",
            "Tokens",
            MetricState::Error(
                "计量服务返回了无效读数，最后确认的数值会继续保留；接続が回復するまで最後に確認された値は保持されます。计量服务返回了无效读数，最后确认的数值会继续保留。".into(),
            ),
        ),
        (
            "long label",
            "Verified session throughput across all connected regions",
            MetricState::Ready(MetricReading::new("12.4k")),
        ),
        (
            "long value beside a fixed delta",
            "Tokens",
            MetricState::Ready(
                MetricReading::new(
                    "sessionthroughputwithoutabreaksessionthroughputwithoutabreaksessionthroughputwithoutabreak12.4k",
                )
                .delta("+8%", Tone::Success),
            ),
        ),
    ];

    for (name, label, state) in cases {
        let (narrow_width, narrow_height) = measured_card(cx, 240.0, state.clone(), label);
        let (_, wide_height) = measured_card(cx, 720.0, state, label);
        assert!(
            narrow_width <= 240.0,
            "{name} widened the card to {narrow_width}px"
        );
        assert!(
            narrow_height > wide_height,
            "{name} did not wrap: narrow={narrow_height}px wide={wide_height}px"
        );
    }
}
