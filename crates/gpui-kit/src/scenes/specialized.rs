//! Raw-value specialized visualization exhibits, using original synthetic data.

use super::support::*;
use crate::display::specialized::{
    HierarchyNode, SpecializedChart, SpecializedData, WeightedValue,
};

#[derive(Default)]
struct Exploration {
    revision: usize,
    focus: Option<SharedString>,
    selected: Option<SharedString>,
    refuse: bool,
}

pub(super) fn specialized_exploration(window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let state = crate::motion::keyed::slot::<Exploration>(
        &"scene.specialized.explore-state".into(),
        window.window_handle().window_id(),
        cx,
    );
    let revision = state.borrow().revision;
    let selected = state.borrow().selected.clone();
    let focus = state
        .borrow()
        .focus
        .clone()
        .unwrap_or_else(|| "total".into());
    let root = HierarchyNode::branch(
        "total",
        "All samples",
        vec![
            HierarchyNode::branch(
                "group",
                "Processing",
                vec![
                    HierarchyNode::leaf(WeightedValue::new(
                        "parse",
                        "Parsing",
                        if revision.is_multiple_of(2) {
                            12.0
                        } else {
                            47.0
                        },
                    )),
                    HierarchyNode::leaf(WeightedValue::new(
                        "execute",
                        "Execution",
                        if revision.is_multiple_of(2) {
                            68.0
                        } else {
                            13.0
                        },
                    )),
                ],
            ),
            HierarchyNode::leaf(WeightedValue::new("deliver", "Delivery", 20.0)),
        ],
    );
    let next = state.clone();
    let refusal = state.clone();
    let charts = [
        (
            "tree",
            "Area · drill/back",
            SpecializedData::treemap_at(&root, &focus),
        ),
        (
            "sun",
            "Angle · drill/back",
            SpecializedData::sunburst_at(&root, &focus),
        ),
        (
            "funnel",
            "Width · enter/update/exit",
            SpecializedData::funnel(match revision % 3 {
                0 => vec![
                    WeightedValue::new("start", "Started", 120.0),
                    WeightedValue::new("finish", "Finished", 24.0),
                ],
                1 => vec![
                    WeightedValue::new("start", "Started", 75.0),
                    WeightedValue::new("new", "New sample", 38.0),
                ],
                _ => vec![],
            }),
        ),
    ];
    stack(&theme).w(px(860.0))
        .child(caption(&theme,"Synthetic live fixtures · advance during motion to interrupt; empty exit shapes are not selectable"))
        .child(div().row().gap_token(&theme,Space::Sm)
            .child(Button::new("scene.specialized.advance").label("Advance samples").on_click(move |window,_| { next.borrow_mut().revision+=1; window.refresh(); }))
            .child(Button::new("scene.specialized.refuse").label(if state.borrow().refuse { "Accept proposals" } else { "Refuse proposals" }).on_click(move |window,_| { let mut s=refusal.borrow_mut(); s.refuse = !s.refuse; window.refresh(); })))
        .child(div().row().gap_token(&theme,Space::Lg).children(charts.into_iter().map(|(id,label,data)| {
            let select = state.clone();
            let navigate = state.clone();
            div().w(px(260.0)).child(SpecializedChart::new(Ident::new("scene.specialized.live").child(id),label,PlotState::Ready(data.expect("valid exploration fixture")))
                .motion(true).labels(true).selected(selected.clone())
                .on_current(move |id,window,_| { let mut s=select.borrow_mut(); if !s.refuse { s.selected=Some(id); } window.refresh(); })
                .on_navigate(move |id,window,_| { let mut s=navigate.borrow_mut(); if !s.refuse { s.focus=Some(id); } window.refresh(); }))
        }))).into_any_element()
}

pub(super) fn specialized(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let leaf = |id, label, value| HierarchyNode::leaf(WeightedValue::new(id, label, value));
    let root = HierarchyNode::branch(
        "total",
        "All work",
        vec![
            leaf("build", "Build", 60.0),
            HierarchyNode::branch(
                "verify",
                "Verification",
                vec![leaf("tests", "Tests", 30.0), leaf("lint", "Lint", 10.0)],
            ),
        ],
    );
    let fixtures = [
        (
            "treemap",
            "Treemap · area = work",
            SpecializedData::treemap(&root),
        ),
        (
            "sunburst",
            "Sunburst · angle = work",
            SpecializedData::sunburst(&root),
        ),
        (
            "funnel",
            "Funnel · width = count",
            SpecializedData::funnel(vec![
                WeightedValue::new("seen", "Seen", 120.0),
                WeightedValue::new("started", "Started", 75.0),
                WeightedValue::new("done", "Completed", 24.0),
            ]),
        ),
    ];
    stack(&theme)
        .w(px(940.0))
        .child(caption(
            &theme,
            "Synthetic hierarchy fixtures · select marks with arrow keys or value keys",
        ))
        .child(
            div()
                .row()
                .gap_token(&theme, Space::Lg)
                .children(fixtures.into_iter().map(|(id, label, data)| {
                    div().w(px(280.0)).child(
                        SpecializedChart::new(
                            Ident::new("scene.specialized").child(id),
                            label,
                            PlotState::Ready(data.expect("valid hierarchy")),
                        )
                        .on_current(|_, _, _| {}),
                    )
                })),
        )
        .child(
            div().w(px(220.0)).child(SpecializedChart::new(
                "scene.specialized.zero",
                "Zero is measured, not missing",
                PlotState::Stale {
                    data: SpecializedData::funnel(vec![WeightedValue::new(
                        "zero",
                        "Measured zero",
                        0.0,
                    )])
                    .expect("zero"),
                    reason: "Refresh refused; verified zero retained".into(),
                },
            )),
        )
        .into_any_element()
}

pub(super) fn specialized_distribution(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let fixtures = [
        (
            "histogram",
            "Histogram · count per bin",
            SpecializedData::histogram(&[-2.0, -1.0, 0.0, 0.0, 1.0, 3.0], &[-2.0, 0.0, 1.0, 3.0]),
        ),
        (
            "box",
            "Box · R-7 / Tukey · domain 0–110",
            SpecializedData::box_plot(
                "latency",
                "Latency",
                &[1.0, 2.0, 4.0, 7.0, 8.0, 100.0],
                [0.0, 110.0],
            ),
        ),
        (
            "range",
            "Range · domain −10–30",
            SpecializedData::range("interval", "Expected interval", -4.0, 18.0, [-10.0, 30.0]),
        ),
        (
            "waterfall",
            "Waterfall · signed changes",
            SpecializedData::waterfall(vec![
                WeightedValue::new("open", "Opening", 60.0),
                WeightedValue::new("cost", "Cost", -90.0),
                WeightedValue::new("credit", "Credit", 10.0),
            ]),
        ),
    ];
    stack(&theme)
        .w(px(1000.0))
        .child(caption(
            &theme,
            "Synthetic distribution fixtures · explicit bin edges and R-7 / Tukey statistics",
        ))
        .child(
            div()
                .row()
                .flex_wrap()
                .gap_token(&theme, Space::Lg)
                .children(fixtures.into_iter().map(|(id, label, data)| {
                    div().w(px(440.0)).child(
                        SpecializedChart::new(
                            Ident::new("scene.specialized").child(id),
                            label,
                            PlotState::Ready(data.expect("valid synthetic fixture")),
                        )
                        .on_current(|_, _, _| {}),
                    )
                })),
        )
        .into_any_element()
}
