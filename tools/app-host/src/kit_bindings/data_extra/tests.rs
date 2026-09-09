use super::*;
use gpui::{Styled, TestAppContext, px};
use gpui_kit_testkit::harness::Harness;

fn node(component: &str, props: Value, events: Value) -> Node {
    serde_json::from_value(
        json!({"kind":"kit","component":component,"id":"data","props":props,"events":events}),
    )
    .expect("data fixture descriptor")
}

#[gpui::test]
fn wide_grid_adapters_build_only_visible_cells_and_emit_selection(cx: &mut TestAppContext) {
    for component in ["DataGrid", "TreeGrid"] {
        let built = Rc::new(RefCell::new(Vec::new()));
        let output = built.clone();
        let events = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        let rows=(0..137).map(|r| {
            let mut row=json!({"id":format!("r{r}"),"label":format!("Row {r}"),"cells":(0..7).map(|c|json!({"id":format!("c{c}"),"text":format!("{r}:{c}"),"slot":format!("r{r}c{c}")})).collect::<Vec<_>>()});
            if component == "TreeGrid" {row["level"]=json!(1);}
            row
        }).collect::<Vec<_>>();
        let mut props = json!({"rows":rows,"columns":(0..7).map(|c|json!({"id":format!("c{c}"),"header":format!("Column {c}"),"fixed":197,"pinned":c==0})).collect::<Vec<_>>(),"visibleRows":4,"slotNames":(0..137).flat_map(|r|(0..7).map(move |c|json!({"id":format!("r{r}c{c}")}))).collect::<Vec<_>>()});
        if component == "DataGrid" {
            props["selectionMode"] = json!("single");
        }
        let n = node(component, props, json!({"select":"selection"}));
        super::super::validation::validate_descriptor(&n).expect("closed grid fixture");
        let mut harness = Harness::new(cx, gpui_kit::install, move |w, cx| {
            let mut slots = KitSlots::new();
            for r in 0..137 {
                for c in 0..7 {
                    let output = output.clone();
                    slots.insert(
                        format!("r{r}c{c}"),
                        Rc::new(move |_, _| {
                            output.borrow_mut().push((r, c));
                            div().child(format!("{r}:{c}")).into_any_element()
                        }),
                    );
                }
            }
            let sink = sink.clone();
            div()
                .w(px(460.))
                .child(render(
                    &n,
                    slots,
                    w,
                    cx,
                    Rc::new(move |_, v| sink.borrow_mut().push(v)),
                ))
                .into_any_element()
        });
        harness.frame();
        assert!(!built.borrow().is_empty());
        // The first layout uses the native estimated viewport; subsequent
        // frames use the measured 460px viewport. Neither builds all columns.
        assert!(built.borrow().iter().all(|(r, c)| *r < 10 && *c < 12));
        built.borrow_mut().clear();
        harness.frame();
        assert!(!built.borrow().is_empty());
        assert!(built.borrow().iter().all(|(r, c)| *r < 10 && *c < 4));
        harness.click("data.r1.c0");
        assert_eq!(
            events.borrow().last(),
            Some(&if component == "DataGrid" {
                json!({"kind":"replace","id":"r1"})
            } else {
                json!("r1")
            })
        );
        built.borrow_mut().clear();
        harness.scroll("data", 1600.);
        harness.frame();
        assert!(built.borrow().iter().any(|(r, _)| *r > 20));
        assert!(built.borrow().iter().all(|(_, c)| *c < 4));
    }
}

#[gpui::test]
fn flow_factories_are_lazy_reusable_and_follow_stable_keys(cx: &mut TestAppContext) {
    let built = Rc::new(RefCell::new(Vec::new()));
    let output = built.clone();
    let rows = (0..257)
        .map(|i| json!({"id":format!("row-{i}"),"revision":i%3}))
        .collect::<Vec<_>>();
    let n = node(
        "Flow",
        json!({"rows":rows,"visibleRows":5,"estimate":31}),
        json!({}),
    );
    let mut harness = Harness::new(cx, gpui_kit::install, move |w, cx| {
        let mut slots = KitSlots::new();
        for i in 0..257 {
            let output = output.clone();
            slots.insert(
                format!("row-{i}"),
                Rc::new(move |_, _| {
                    output.borrow_mut().push(i);
                    div()
                        .h(px(27. + (i % 3) as f32 * 9.))
                        .child(format!("{i}"))
                        .into_any_element()
                }),
            );
        }
        div()
            .w(px(450.))
            .child(render(&n, slots, w, cx, Rc::new(|_, _| {})))
            .into_any_element()
    });
    harness.frame();
    assert!(!built.borrow().is_empty());
    assert!(built.borrow().iter().all(|i| *i < 15));
    built.borrow_mut().clear();
    harness.update(|w, cx| gpui_kit::data::scroll_to_row(&Ident::new("data"), 180, w, cx));
    harness.frame();
    assert!(built.borrow().iter().any(|i| *i >= 180));
    assert!(built.borrow().len() < 100);
}

#[gpui::test]
fn caller_owned_tree_emits_toggle_without_expanding_controlled_data(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let n = node(
        "Tree",
        json!({"nodes":[{"id":"root","label":"Root","children":[{"id":"child","label":"Child"}]}],"expanded":[],"visibleRows":3}),
        json!({"toggle":"toggle"}),
    );
    let mut h = Harness::new(cx, gpui_kit::install, move |w, cx| {
        let sink = sink.clone();
        render(
            &n,
            KitSlots::new(),
            w,
            cx,
            Rc::new(move |_, v| sink.borrow_mut().push(v)),
        )
    });
    h.click("data.root.toggle");
    assert_eq!(
        events.borrow().last(),
        Some(&json!({"id":"root","expanded":true}))
    );
    assert!(h.node("data.child").is_none());
}

#[gpui::test]
fn native_grid_edit_survives_render_and_reports_caller_owned_commit(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let n = node(
        "DataGrid",
        json!({"rows":[{"id":"record","label":"Unchanged label","cells":[{"id":"value","text":"seed"}]}],"columns":[{"id":"value","header":"Value","editable":true}],"editing":{"row":"record","column":"value","value":"seed"},"visibleRows":2}),
        json!({"edit":"edit"}),
    );
    let mut h = Harness::new(cx, gpui_kit::install, move |w, cx| {
        let sink = sink.clone();
        render(
            &n,
            KitSlots::new(),
            w,
            cx,
            Rc::new(move |_, v| sink.borrow_mut().push(v)),
        )
    });
    assert!(h.node("data.edit").is_some());
    h.keystrokes("x");
    h.frame();
    h.keystrokes("enter");
    assert_eq!(
        &*events.borrow(),
        &[json!({"row":"record","column":"value","value":"seedx","outcome":"commit","next":null})]
    );
    assert_eq!(
        h.node("data.record").expect("retained row").text.as_deref(),
        Some("Unchanged label")
    );
}

#[gpui::test]
fn tree_reorder_reports_full_native_intent_and_does_not_reorder_caller_data(
    cx: &mut TestAppContext,
) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let n = node(
        "Tree",
        json!({"nodes":[{"id":"alpha","label":"Alpha"},{"id":"beta","label":"Beta"},{"id":"gamma","label":"Gamma"}],"reorderable":true,"visibleRows":3}),
        json!({"move":"move"}),
    );
    let mut h = Harness::new(cx, gpui_kit::install, move |w, cx| {
        let sink = sink.clone();
        render(
            &n,
            KitSlots::new(),
            w,
            cx,
            Rc::new(move |_, v| sink.borrow_mut().push(v)),
        )
    });
    let destination = h.point_down("data.gamma", 0.9);
    h.drag_start("data.alpha");
    h.drag_to(destination);
    h.drop_here();
    let received = events.borrow();
    let intent = received.last().expect("native reorder intent");
    assert_eq!(intent["id"], "alpha");
    assert_eq!(intent["label"], "Alpha");
    assert_eq!(intent["source"], "data");
    assert_eq!(intent["anchor"], "gamma");
    assert_eq!(intent["position"], "after");
    assert!(intent["velocity"]["x"].is_number());
    assert!(intent["velocity"]["y"].is_number());
    drop(received);
    assert!(
        h.bounds("data.alpha").expect("alpha").origin.y
            < h.bounds("data.beta").expect("beta").origin.y
    );
}

#[test]
fn native_closed_schemas_reject_ambiguous_data() {
    for n in [
        node(
            "Tree",
            json!({"nodes":[{"id":"a","label":"A","children":[{"id":"a","label":"Duplicate"}]}]}),
            json!({}),
        ),
        node(
            "DataGrid",
            json!({"rows":[{"id":"r","cells":[{"id":"missing","text":"Value"}]}],"columns":[]}),
            json!({}),
        ),
        node(
            "TreeGrid",
            json!({"rows":[{"id":"child","level":2,"parent":"missing","cells":[]}],"columns":[]}),
            json!({}),
        ),
        node(
            "Masonry",
            json!({"items":[{"id":"one","height":-1}]}),
            json!({}),
        ),
    ] {
        assert!(super::super::validate_descriptor(&n).is_err());
    }
}
