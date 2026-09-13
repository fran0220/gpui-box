//! Explicit continuous color encodings, kept separate from contribution levels.
use super::support::*;
use crate::display::heatmap::{ContinuousHeatCell, ContinuousHeatmap, HeatColorScale};

#[derive(Default)]
struct MatrixRevisions {
    revision: usize,
    direct: bool,
    selected: Option<SharedString>,
}

pub(super) fn heatmap_reordering(window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let state = crate::motion::keyed::slot::<MatrixRevisions>(
        &"scene.heat.reorder-state".into(),
        window.window_handle().window_id(),
        cx,
    );
    let revision = state.borrow().revision % 3;
    let rows = match revision {
        0 => vec!["east", "west"],
        1 => vec!["west", "north", "east"],
        _ => vec!["east"],
    };
    let columns = match revision {
        0 => vec!["a", "b", "c"],
        1 => vec!["c", "a", "d", "b"],
        _ => vec!["a", "b"],
    };
    let cells = rows
        .iter()
        .flat_map(|row| {
            columns.iter().map(move |column| {
                let reading = match (*row, *column) {
                    ("east", "a") => Some(3.0),
                    ("east", "b") => Some(12.0),
                    ("west", "a") => Some(42.0),
                    ("west", "c") => Some(81.0),
                    ("north", "b") => Some(65.0),
                    _ => None,
                };
                ContinuousHeatCell::new(
                    format!("{row}-{column}"),
                    *row,
                    *column,
                    format!("{row} / {column}"),
                    reading,
                )
            })
        })
        .collect();
    let next = state.clone();
    let direct = state.clone();
    let selection = state.clone();
    stack(&theme)
        .w(px(560.0))
        .child(caption(
            &theme,
            "Synthetic keyed axes · reorder, enter, exit and reinsert · raw readings never tween",
        ))
        .child(
            div()
                .row()
                .gap_token(&theme, Space::Sm)
                .child(
                    Button::new("scene.heat.reorder.advance")
                        .label("Advance axes")
                        .on_click(move |window, _| {
                            next.borrow_mut().revision += 1;
                            window.refresh();
                        }),
                )
                .child(
                    Button::new("scene.heat.reorder.direct")
                        .label(if state.borrow().direct {
                            "Enable animation"
                        } else {
                            "Direct updates"
                        })
                        .on_click(move |window, _| {
                            let mut s = direct.borrow_mut();
                            s.direct = !s.direct;
                            window.refresh();
                        }),
                ),
        )
        .child(
            ContinuousHeatmap::new(
                "scene.heat.reorder",
                "Stable caller coordinates",
                HeatColorScale::sequential([0.0, 100.0], theme.colors.canvas, theme.colors.accent)
                    .expect("domain"),
                PlotState::Ready(cells),
            )
            .rows(rows)
            .columns(columns)
            .animate(!state.borrow().direct)
            .on_current(move |id, window, _| {
                selection.borrow_mut().selected = Some(id);
                window.refresh();
            })
            .when_some(state.borrow().selected.clone(), |chart, id| {
                chart.current(id)
            }),
        )
        .into_any_element()
}

pub(super) fn continuous_heatmap_transition(window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let revision = crate::motion::keyed::slot::<usize>(
        &"scene.heat.transition-state".into(),
        window.window_handle().window_id(),
        cx,
    );
    let value = match *revision.borrow() % 3 {
        0 => Some(-8.0),
        1 => Some(32.0),
        _ => None,
    };
    let next = revision.clone();
    stack(&theme)
        .w(px(440.0))
        .child(caption(
            &theme,
            "Synthetic observations · exact latest text with animated color; missing is never zero",
        ))
        .child(
            Button::new("scene.heat.advance")
                .label("Advance reading")
                .on_click(move |window, _| {
                    *next.borrow_mut() += 1;
                    window.refresh();
                }),
        )
        .child(
            ContinuousHeatmap::new(
                "scene.heat.live",
                "Controlled measurement",
                HeatColorScale::diverging(
                    [-8.0, 2.0, 32.0],
                    theme.colors.accent,
                    theme.colors.canvas,
                    theme.colors.danger,
                )
                .expect("domain"),
                PlotState::Ready(vec![
                    ContinuousHeatCell::new("changing", "row", "a", "Changing reading", value),
                    ContinuousHeatCell::new("zero", "row", "b", "Verified zero", Some(0.0)),
                ]),
            )
            .rows(["row"])
            .columns(["a", "b"])
            .motion(true),
        )
        .into_any_element()
}

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
