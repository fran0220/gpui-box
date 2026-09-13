use gpui::{IntoElement, ParentElement, SharedString, Styled, TestAppContext, div, px};
use gpui_kit::display::{
    plot::PlotState,
    specialized::{SpecializedChart, SpecializedData, WeightedValue},
};
use gpui_kit_testkit::harness::Harness;
use std::{cell::RefCell, rc::Rc};

#[gpui::test]
fn measured_labels_stay_disjoint_during_interrupted_layout(cx: &mut TestAppContext) {
    let scene = gpui_kit::scenes::find("specialized-exploration").expect("dynamic exhibit");
    let mut h = Harness::new(cx, gpui_kit::install, scene.build);
    for _ in 0..4 {
        h.click("scene.specialized.advance");
        h.advance(std::time::Duration::from_millis(60));
        let snapshot = h.snapshot();
        for family in ["tree", "sun", "funnel"] {
            let labels = snapshot.under(&format!("scene.specialized.live.{family}.plot.label."));
            if family != "funnel" {
                assert!(labels.len() >= 2, "hierarchy must expose measured labels");
            }
            for (index, a) in labels.iter().enumerate() {
                for b in &labels[index + 1..] {
                    let (a, b) = (a.bounds, b.bounds);
                    assert!(
                        a.x + a.width <= b.x
                            || b.x + b.width <= a.x
                            || a.y + a.height <= b.y
                            || b.y + b.height <= a.y
                    );
                }
            }
        }
    }
}

#[gpui::test]
fn exploration_exhibit_refuses_then_accepts_drill_back_and_removes_live_targets(
    cx: &mut TestAppContext,
) {
    let scene = gpui_kit::scenes::find("specialized-exploration").expect("dynamic exhibit");
    let mut h = Harness::new(cx, gpui_kit::install, scene.build);
    h.advance(std::time::Duration::from_secs(2));
    h.click("scene.specialized.refuse");
    h.click("scene.specialized.live.tree.navigation.group");
    assert!(
        h.node("scene.specialized.live.tree.plot.mark.deliver")
            .is_some()
    );
    h.click("scene.specialized.refuse");
    h.click("scene.specialized.live.tree.navigation.group");
    assert!(
        h.node("scene.specialized.live.tree.plot.mark.deliver")
            .is_none()
    );
    assert!(
        h.node("scene.specialized.live.tree.navigation.group")
            .expect("focus")
            .disabled
    );
    h.click("scene.specialized.live.tree.navigation.total");
    assert!(
        h.node("scene.specialized.live.tree.plot.mark.deliver")
            .is_some()
    );
    let initial = h
        .node("scene.specialized.live.tree.plot.mark.parse")
        .expect("initial shape")
        .bounds;
    h.click("scene.specialized.advance");
    assert_eq!(
        h.node("scene.specialized.live.tree.plot.mark.parse")
            .expect("latest value")
            .value
            .as_deref(),
        Some("47")
    );
    assert!(
        h.node("scene.specialized.live.funnel.plot.mark.finish")
            .is_none()
    );
    assert!(h.node("scene.specialized.live.funnel.key.finish").is_none());
    h.advance(std::time::Duration::from_millis(60));
    let intermediate = h
        .node("scene.specialized.live.tree.plot.mark.parse")
        .expect("moving shape")
        .bounds;
    assert_ne!(initial, intermediate);
    h.click("scene.specialized.advance");
    assert_eq!(
        h.node("scene.specialized.live.funnel")
            .expect("empty state")
            .value
            .as_deref(),
        Some("empty")
    );
    assert!(
        h.node("scene.specialized.live.funnel.plot.mark.start")
            .is_none()
    );
    h.update(|_, cx| cx.set_reduce_motion(true));
    h.advance(std::time::Duration::ZERO);
    h.click("scene.specialized.advance");
    assert_eq!(
        h.node("scene.specialized.live.funnel.plot.mark.finish")
            .expect("reinserted shape")
            .value
            .as_deref(),
        Some("24")
    );
}

#[gpui::test]
fn heatmap_motion_publishes_exact_latest_readings_and_missing_without_waiting(
    cx: &mut TestAppContext,
) {
    let scene = gpui_kit::scenes::find("continuous-heatmap-transition").expect("dynamic exhibit");
    let mut h = Harness::new(cx, gpui_kit::install, scene.build);
    h.click("scene.heat.advance");
    assert_eq!(
        h.node("scene.heat.live.cell.changing")
            .expect("changed reading")
            .value
            .as_deref(),
        Some("32")
    );
    h.advance(std::time::Duration::from_millis(40));
    h.click("scene.heat.advance");
    assert_eq!(
        h.node("scene.heat.live.cell.changing")
            .expect("missing reading")
            .value
            .as_deref(),
        Some("Not observed")
    );
    assert_eq!(
        h.node("scene.heat.live.cell.zero")
            .expect("verified zero")
            .value
            .as_deref(),
        Some("0")
    );
    h.update(|_, cx| cx.set_reduce_motion(true));
    h.click("scene.heat.advance");
    assert_eq!(
        h.node("scene.heat.live.cell.changing")
            .expect("settled reading")
            .value
            .as_deref(),
        Some("-8")
    );
}

#[gpui::test]
fn geometric_pointer_proposals_do_not_accept_refused_selection(cx: &mut TestAppContext) {
    use gpui_kit::display::specialized::HierarchyNode;
    let reports = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let sink = reports.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        SpecializedChart::new(
            "sun",
            "Hierarchy",
            PlotState::Ready(
                SpecializedData::sunburst(&HierarchyNode::branch(
                    "root",
                    "Root",
                    vec![
                        HierarchyNode::leaf(WeightedValue::new("small", "Small", 1.0)),
                        HierarchyNode::leaf(WeightedValue::new("large", "Large", 3.0)),
                    ],
                ))
                .expect("hierarchy"),
            ),
        )
        .selected(None)
        .on_current(move |id, _, _| sink.borrow_mut().push(id))
        .into_any_element()
    });
    let frame = harness.node("sun.plot").expect("measured plot").bounds;
    for (x, y, expected) in [
        (0.5, 0.5, Some("root")),
        (0.72, 0.22, Some("small")),
        (0.99, 0.99, None),
    ] {
        reports.borrow_mut().clear();
        let p = gpui::point(
            px(frame.x + x * frame.width),
            px(frame.y + y * frame.height),
        );
        harness
            .context()
            .simulate_mouse_down(p, gpui::MouseButton::Left, gpui::Modifiers::none());
        harness
            .context()
            .simulate_mouse_up(p, gpui::MouseButton::Left, gpui::Modifiers::none());
        harness.context().run_until_parked();
        assert_eq!(reports.borrow().last().map(SharedString::as_ref), expected);
        for id in ["root", "small", "large"] {
            assert!(
                !harness
                    .node(&format!("sun.plot.mark.{id}"))
                    .expect("mark")
                    .selected
            );
        }
    }
    harness.click("sun.key.small");
    assert_eq!(
        reports.borrow().last().map(SharedString::as_ref),
        Some("small")
    );
    assert!(!harness.node("sun.key.small").expect("key").selected);
}

#[gpui::test]
fn specialized_selection_and_stale_values_agree(cx: &mut TestAppContext) {
    let reports = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let sink = reports.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        div()
            .w(px(220.0))
            .child(
                SpecializedChart::new(
                    "funnel",
                    "Stages",
                    PlotState::Stale {
                        data: SpecializedData::funnel(vec![
                            WeightedValue::new("seen", "Seen", 120.0),
                            WeightedValue::new("done", "Done", 24.0),
                        ])
                        .expect("valid funnel"),
                        reason: "refresh refused".into(),
                    },
                )
                .on_current(move |id, _, _| sink.borrow_mut().push(id)),
            )
            .into_any_element()
    });
    assert_eq!(
        harness.node("funnel").expect("funnel").value.as_deref(),
        Some("stale")
    );
    assert_eq!(
        harness
            .node("funnel")
            .expect("funnel")
            .description
            .as_deref(),
        Some("refresh refused")
    );
    let seen = harness.node("funnel.plot.mark.seen").expect("seen mark");
    let done = harness.node("funnel.plot.mark.done").expect("done mark");
    assert!((done.bounds.width / seen.bounds.width - 0.2).abs() < 1e-5);
    assert_eq!(done.value.as_deref(), Some("24"));
    assert_eq!(
        harness
            .node("funnel.key.done")
            .expect("done key")
            .value
            .as_deref(),
        Some("24")
    );
    harness.update(|window, cx| window.focus_next(cx));
    harness.keystrokes("right");
    assert_eq!(
        reports.borrow().last().map(SharedString::as_ref),
        Some("done")
    );
    assert!(
        harness
            .node("funnel.plot.mark.done")
            .expect("done mark")
            .selected
    );
    assert!(harness.node("funnel.key.done").expect("done key").selected);
    harness.click("funnel.key.seen");
    assert_eq!(
        reports.borrow().last().map(SharedString::as_ref),
        Some("seen")
    );
    assert!(
        harness
            .node("funnel.plot.mark.seen")
            .expect("seen mark")
            .selected
    );
}

#[gpui::test]
fn continuous_heatmap_keeps_zero_missing_domain_and_controlled_selection_distinct(
    cx: &mut TestAppContext,
) {
    use gpui_kit::display::heatmap::{ContinuousHeatCell, ContinuousHeatmap, HeatColorScale};
    let reports = Rc::new(RefCell::new(Vec::<SharedString>::new()));
    let sink = reports.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        let scale = HeatColorScale::diverging(
            [-8.0, 2.0, 32.0],
            gpui::rgb(0x0000ff).into(),
            gpui::rgb(0xffffff).into(),
            gpui::rgb(0xff0000).into(),
        )
        .expect("valid diverging domain");
        div()
            .w(px(240.0))
            .child(
                ContinuousHeatmap::new(
                    "heat",
                    "Measured",
                    scale,
                    PlotState::Stale {
                        data: vec![
                            ContinuousHeatCell::new("zero", "r", "a", "Zero", Some(0.0)),
                            ContinuousHeatCell::new("missing", "r", "b", "Missing", None),
                        ],
                        reason: "refresh refused".into(),
                    },
                )
                .rows(["r"])
                .columns(["a", "b"])
                .current("missing")
                .on_current(move |id, _, _| sink.borrow_mut().push(id)),
            )
            .into_any_element()
    });
    assert_eq!(
        harness
            .node("heat.cell.zero")
            .expect("zero cell")
            .value
            .as_deref(),
        Some("0")
    );
    assert_eq!(
        harness
            .node("heat.cell.missing")
            .expect("missing cell")
            .value
            .as_deref(),
        Some("Not observed")
    );
    assert_eq!(
        harness
            .node("heat.legend")
            .expect("legend")
            .value
            .as_deref(),
        Some("-8 / 2 / 32; missing has no color")
    );
    assert_eq!(
        harness.node("heat").expect("heatmap").value.as_deref(),
        Some("stale")
    );
    harness.update(|window, cx| window.focus_next(cx));
    harness.keystrokes("enter");
    assert_eq!(
        reports.borrow().last().map(SharedString::as_ref),
        Some("zero")
    );
    assert!(
        harness
            .node("heat.cell.missing")
            .expect("missing cell")
            .selected,
        "selection is caller controlled until accepted"
    );
}

#[gpui::test]
fn raw_ohlc_keeps_caller_wording_in_shared_cartesian_semantics(cx: &mut TestAppContext) {
    use gpui_kit::display::{
        chart::{
            ChartSelection,
            cartesian::CartesianChart,
            data::{ChartScale, ValueAxis},
            scale::{NumericScale, ScaleKind},
        },
        plot::RawOhlc,
    };
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        let series = RawOhlc::series(
            "ohlc",
            "price",
            [RawOhlc::new(
                "day-b",
                2.0,
                [65.0, 75.0, 35.0, 40.0],
                "Day B",
                "O65 H75 L35 C40",
            )],
            gpui::rgb(0x00ff00).into(),
            gpui::rgb(0xff0000).into(),
        )
        .expect("valid OHLC");
        div()
            .w(px(420.0))
            .child(
                CartesianChart::new(
                    "ohlc",
                    "Synthetic OHLC",
                    ChartScale::Numeric(
                        NumericScale::new(ScaleKind::Time, [0.0, 4.0]).expect("time domain"),
                    ),
                    [ValueAxis {
                        id: "price".into(),
                        label: "Price".into(),
                        scale: NumericScale::new(ScaleKind::Linear, [0.0, 80.0])
                            .expect("price domain"),
                    }],
                )
                .series([series])
                .selected(Some(ChartSelection::new("ohlc", "day-b"))),
            )
            .into_any_element()
    });
    let mark = harness
        .node("ohlc.series.ohlc.point.day-b")
        .expect("raw business mark");
    assert_eq!(mark.text.as_deref(), Some("Day B"));
    assert_eq!(mark.value.as_deref(), Some("O65 H75 L35 C40"));
    assert!(mark.selected);
    assert!(mark.bounds.area() > 0.0);
}

#[gpui::test]
fn empty_specialized_input_is_not_a_ready_zero_measurement(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        SpecializedChart::new(
            "empty",
            "No observations",
            PlotState::Ready(SpecializedData::default()),
        )
        .into_any_element()
    });
    assert_eq!(
        harness.node("empty").expect("empty state").value.as_deref(),
        Some("empty")
    );
}

#[gpui::test]
fn statistics_localize_at_render_without_changing_values_or_selection(cx: &mut TestAppContext) {
    use gpui_kit::display::specialized::HierarchyNode;
    use gpui_kit::strings::{StringKey, TranslationPack};
    // Construct once before installing a locale, then reuse the same raw layout
    // after a runtime override. Neither localized label may become an identity.
    let box_data = SpecializedData::box_plot(
        "sample",
        "测量",
        &[1.0, 2.0, 4.0, 7.0, 8.0, 100.0],
        [0.0, 110.0],
    )
    .expect("valid box sample");
    let hierarchy = SpecializedData::sunburst(&HierarchyNode::branch(
        "total",
        "合计",
        vec![
            HierarchyNode::leaf(WeightedValue::new("a", "甲", 3.0)),
            HierarchyNode::leaf(WeightedValue::new("b", "乙", 7.0)),
        ],
    ))
    .expect("valid hierarchy");
    let mut harness = Harness::new(
        cx,
        |cx| {
            gpui_kit::install(cx);
            cx.set_global(TranslationPack::SimplifiedChinese.strings());
        },
        move |_, _| {
            div()
                .w(px(240.0))
                .child(
                    SpecializedChart::new("box", "样本", PlotState::Ready(box_data.clone()))
                        .current("sample.median"),
                )
                .child(SpecializedChart::new(
                    "sun",
                    "分类",
                    PlotState::Ready(hierarchy.clone()),
                ))
                .into_any_element()
        },
    );
    for (suffix, wording, value) in [
        ("low", "下须端点", "1"),
        ("q1", "第一四分位数", "2.5"),
        ("median", "中位数", "5.5"),
        ("q3", "第三四分位数", "7.75"),
        ("high", "上须端点", "8"),
        ("outlier-4059000000000000", "离群值", "100"),
    ] {
        for part in ["plot.mark", "key"] {
            let node = harness
                .node(&format!("box.{part}.sample.{suffix}"))
                .expect("localized statistic");
            assert_eq!(
                node.text.as_deref(),
                Some(format!("测量 · {wording}").as_str())
            );
            assert_eq!(node.value.as_deref(), Some(value));
            assert_eq!(node.selected, suffix == "median");
        }
    }
    for part in ["plot.mark", "key"] {
        let node = harness
            .node(&format!("sun.{part}.total"))
            .expect("subtotal");
        assert_eq!(node.text.as_deref(), Some("合计（小计）"));
        assert_eq!(node.value.as_deref(), Some("10"));
    }
    let before = harness
        .node("box.plot.mark.sample.median")
        .expect("median")
        .bounds;
    harness.update(|_, cx| {
        let mut strings = TranslationPack::English.strings();
        strings.set(StringKey::PlotBoxMedian, "Center of {0}");
        cx.set_global(strings);
    });
    for part in ["plot.mark", "key"] {
        let node = harness
            .node(&format!("box.{part}.sample.median"))
            .expect("overridden median");
        assert_eq!(node.text.as_deref(), Some("Center of 测量"));
        assert_eq!(node.value.as_deref(), Some("5.5"));
        assert!(node.selected);
    }
    assert_eq!(
        harness
            .node("box.plot.mark.sample.median")
            .expect("median")
            .bounds,
        before
    );
}

#[gpui::test]
fn heatmap_localizes_missing_legend_and_validation_errors(cx: &mut TestAppContext) {
    use gpui_kit::display::heatmap::{ContinuousHeatCell, ContinuousHeatmap, HeatColorScale};
    use gpui_kit::strings::{StringKey, TranslationPack};
    let scale =
        HeatColorScale::sequential([0.0, 10.0], gpui::rgb(0).into(), gpui::rgb(0xffffff).into())
            .expect("valid color domain");
    let mut harness = Harness::new(
        cx,
        |cx| {
            gpui_kit::install(cx);
            cx.set_global(TranslationPack::SimplifiedChinese.strings());
        },
        move |_, _| {
            ContinuousHeatmap::new(
                "heat",
                "测量",
                scale,
                PlotState::Ready(vec![ContinuousHeatCell::new(
                    "missing",
                    "r",
                    "c",
                    "无数据",
                    None,
                )]),
            )
            .rows(["r"])
            .columns(["c"])
            .into_any_element()
        },
    );
    assert_eq!(
        harness
            .node("heat.cell.missing")
            .expect("missing cell")
            .value
            .as_deref(),
        Some("未观测")
    );
    let legend = harness.node("heat.legend").expect("localized legend");
    assert_eq!(legend.text.as_deref(), Some("颜色值域"));
    assert_eq!(legend.value.as_deref(), Some("0 / 5 / 10；缺失值不着色"));
    for (rows, cells, expected) in [
        (vec!["r", "r"], vec![], "热力图坐标轴标识重复"),
        (
            vec!["r"],
            vec![
                ContinuousHeatCell::new("a", "r", "c", "甲", None),
                ContinuousHeatCell::new("b", "r", "c", "乙", Some(0.0)),
            ],
            "热力图单元格标识或坐标重复",
        ),
        (
            vec!["r"],
            vec![ContinuousHeatCell::new("a", "unknown", "c", "甲", None)],
            "未知的热力图行或列",
        ),
        (
            vec!["r"],
            vec![ContinuousHeatCell::new("a", "r", "c", "甲", Some(11.0))],
            "热力图观测值超出声明的值域",
        ),
    ] {
        harness.remount(move |_, _| {
            ContinuousHeatmap::new("heat", "测量", scale, PlotState::Ready(cells.clone()))
                .rows(rows.clone())
                .columns(["c"])
                .into_any_element()
        });
        let node = harness.node("heat").expect("validation error");
        assert_eq!(node.value.as_deref(), Some("error"));
        assert_eq!(node.description.as_deref(), Some(expected));
    }
    harness.update(|_, cx| {
        let mut strings = TranslationPack::English.strings();
        strings.set(StringKey::HeatmapOutsideDomain, "Caller domain refusal");
        cx.set_global(strings);
    });
    assert_eq!(
        harness
            .node("heat")
            .expect("overridden error")
            .description
            .as_deref(),
        Some("Caller domain refusal")
    );
}
