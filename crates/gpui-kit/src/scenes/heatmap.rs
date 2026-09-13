//! Explicit continuous color encodings, kept separate from contribution levels.
use super::support::*;
use crate::display::heatmap::{ContinuousHeatCell, ContinuousHeatmap, HeatColorScale};

pub(super) fn continuous_heatmap(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let diverging = HeatColorScale::diverging(
        [-8.0, 2.0, 32.0],
        theme.colors.accent,
        theme.colors.canvas,
        theme.colors.danger,
    )
    .expect("explicit domain");
    let sequential =
        HeatColorScale::sequential([0.0, 100.0], theme.colors.canvas, theme.colors.success)
            .expect("explicit domain");
    let cells = [
        ("east", "a", Some(-8.0)),
        ("east", "b", Some(0.0)),
        ("east", "c", Some(2.0)),
        ("west", "a", Some(17.0)),
        ("west", "b", Some(32.0)),
        ("west", "c", None),
    ]
    .into_iter()
    .map(|(row, column, reading)| {
        ContinuousHeatCell::new(
            format!("{row}-{column}"),
            row,
            column,
            format!("Synthetic observation {row} / {column}"),
            reading,
        )
    })
    .collect::<Vec<_>>();
    stack(&theme)
        .w(px(800.0))
        .child(caption(
            &theme,
            "Synthetic measurements · explicit neutral = 2 · no observation is not zero",
        ))
        .child(
            div().w(px(720.0)).child(
                ContinuousHeatmap::new(
                    "scene.continuous.wide",
                    "Diverging measurements",
                    diverging,
                    PlotState::Ready(cells.clone()),
                )
                .rows(["east", "west"])
                .columns(["a", "b", "c"])
                .current("east-c")
                .on_current(|_, _, _| {}),
            ),
        )
        .child(
            div().w(px(240.0)).child(
                ContinuousHeatmap::new(
                    "scene.continuous.narrow",
                    "Narrow · last verified values",
                    diverging,
                    PlotState::Stale {
                        data: cells,
                        reason: "Refresh refused".into(),
                    },
                )
                .rows(["east", "west"])
                .columns(["a", "b", "c"]),
            ),
        )
        .child(
            div().w(px(400.0)).child(
                ContinuousHeatmap::new(
                    "scene.continuous.sequential",
                    "Sequential · domain 0–100",
                    sequential,
                    PlotState::Ready(vec![
                        ContinuousHeatCell::new("zero", "r", "a", "Measured zero", Some(0.0)),
                        ContinuousHeatCell::new("middle", "r", "b", "Middle", Some(50.0)),
                        ContinuousHeatCell::new("high", "r", "c", "High", Some(100.0)),
                    ]),
                )
                .rows(["r"])
                .columns(["a", "b", "c"]),
            ),
        )
        .child(
            ContinuousHeatmap::new(
                "scene.continuous.invalid",
                "Invalid readings are errors",
                sequential,
                PlotState::Ready(vec![ContinuousHeatCell::new(
                    "outside",
                    "r",
                    "c",
                    "Outside domain",
                    Some(101.0),
                )]),
            )
            .rows(["r"])
            .columns(["c"]),
        )
        .into_any_element()
}
