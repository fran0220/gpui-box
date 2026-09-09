use super::*;
use gpui::{ParentElement, Styled, TestAppContext, div, px};
use gpui_kit_testkit::harness::Harness;

fn node(component: &str, id: &str, props: Value, events: Value) -> Node {
    let node = serde_json::from_value(
        json!({"kind":"kit","component":component,"id":id,"props":props,"events":events}),
    )
    .expect("fixture node");
    validate_descriptor(&node).expect("valid fixture");
    node
}

#[gpui::test]
fn choices_render_native_semantics_and_emit_typed_intents(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let captured = events.clone();
    let nodes = [
        node(
            "Checkbox",
            "choice.check",
            json!({"label":"Check","checked":null}),
            json!({"change":"check"}),
        ),
        node(
            "Radio",
            "choice.radio",
            json!({"label":"Radio"}),
            json!({"select":"radio"}),
        ),
        node(
            "Switch",
            "choice.switch",
            json!({"label":"Switch","on":true}),
            json!({"change":"switch"}),
        ),
        node(
            "SegmentedControl",
            "choice.segment",
            json!({"segments":[{"id":"alpha","label":"Alpha"},{"id":"beta","label":"Beta"}],"selected":"alpha"}),
            json!({"select":"segment"}),
        ),
        node(
            "Slider",
            "choice.slider",
            json!({"min":-10,"max":20,"value":-3}),
            json!({"change":"slider"}),
        ),
        node(
            "Checkbox",
            "choice.disabled",
            json!({"label":"Disabled","disabled":true}),
            json!({}),
        ),
    ];
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let mut state = KitState::default();
        let output = captured.clone();
        let emit: Emit =
            Rc::new(move |action, payload| output.borrow_mut().push((action.to_owned(), payload)));
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .children(
                nodes
                    .iter()
                    .map(|node| state.render(node, BTreeMap::new(), window, cx, emit.clone())),
            )
            .into_any_element()
    });
    harness.click("choice.check");
    harness.click("choice.radio");
    harness.click("choice.switch");
    harness.click("choice.segment.beta");
    harness.click("choice.disabled");
    assert_eq!(
        &*events.borrow(),
        &[
            ("check".into(), json!(true)),
            ("radio".into(), Value::Null),
            ("switch".into(), json!(false)),
            ("segment".into(), json!("beta"))
        ]
    );
    assert!(
        harness
            .node("choice.disabled")
            .expect("disabled semantic node")
            .disabled
    );
    assert!(harness.node("choice.slider").is_some());
}

#[gpui::test]
fn retained_input_routes_latest_actions_and_drops_removed_entities(cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KitState::default()));
    let descriptor = Rc::new(RefCell::new(node(
        "TextInput",
        "field",
        json!({"placeholder":"Type"}),
        json!({"change":"first"}),
    )));
    let build_state = state.clone();
    let build_node = descriptor.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let node = build_node.borrow();
        let mut state = build_state.borrow_mut();
        state.reconcile(&node, cx);
        let output = output.clone();
        state.render(
            &node,
            BTreeMap::new(),
            window,
            cx,
            Rc::new(move |action, payload| output.borrow_mut().push((action.to_owned(), payload))),
        )
    });
    harness.click("field");
    harness.keystrokes("a");
    let input = match &state.borrow().retained[&(0, "field".into())].control {
        Control::Input(entity) => entity.clone(),
        _ => unreachable!(),
    };
    descriptor
        .borrow_mut()
        .events
        .insert("change".into(), "latest".into());
    descriptor
        .borrow_mut()
        .props
        .insert("size".into(), json!("lg"));
    harness.frame();
    harness.keystrokes("b");
    assert_eq!(
        events.borrow().last(),
        Some(&("latest".into(), json!("ab")))
    );
    harness.update(|_, cx| assert_eq!(input.read(cx).value().as_ref(), "ab"));
    assert_eq!(state.borrow().retained.len(), 1);
    let empty: Node =
        serde_json::from_value(json!({"kind":"column","id":"root"})).expect("empty tree");
    harness.update(|_, cx| state.borrow_mut().reconcile(&empty, cx));
    assert!(state.borrow().retained.is_empty());
    let count = events.borrow().len();
    harness.update(|_, cx| input.update(cx, |input, cx| input.set_value("detached", cx)));
    assert_eq!(
        events.borrow().len(),
        count,
        "removed subscription must not dispatch"
    );
}

#[gpui::test]
fn select_preserves_menu_and_caller_refusal(cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(KitState::default()));
    let descriptor = node(
        "Select",
        "select",
        json!({"options":[{"id":"alpha","label":"Alpha"},{"id":"beta","label":"Beta"}],"selected":"alpha"}),
        json!({"change":"selected"}),
    );
    let output = Rc::new(RefCell::new(Vec::new()));
    let events = output.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let events = events.clone();
        state.borrow_mut().render(
            &descriptor,
            BTreeMap::new(),
            window,
            cx,
            Rc::new(move |action, value| events.borrow_mut().push((action.to_owned(), value))),
        )
    });
    harness.click("select");
    harness.frame();
    harness.click("select.beta");
    assert_eq!(&*output.borrow(), &[("selected".into(), json!("beta"))]);
    assert_eq!(
        harness
            .node("select")
            .expect("select semantic node")
            .value
            .as_deref(),
        Some("Alpha")
    );
}

#[gpui::test]
fn navigation_uses_business_ids_and_mounts_named_slots(cx: &mut TestAppContext) {
    let output = Rc::new(RefCell::new(Vec::new()));
    let events = output.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let events = events.clone();
        let emit: Emit =
            Rc::new(move |action, value| events.borrow_mut().push((action.to_owned(), value)));
        let tabs = node(
            "Tabs",
            "tabs",
            json!({"tabs":[{"id":"first","label":"First"},{"id":"other","label":"Other","closable":true}],"selected":"first"}),
            json!({"select":"tab"}),
        );
        let accordion = node(
            "Accordion",
            "details",
            json!({"sections":[{"id":"advanced","title":"Advanced"}],"expanded":["advanced"]}),
            json!({"toggle":"section"}),
        );
        let pages = node(
            "Pagination",
            "pages",
            json!({"page":3,"totalPages":8}),
            json!({"select":"page"}),
        );
        let mut state = KitState::default();
        let slots = BTreeMap::from([(
            "advanced".into(),
            vec![
                Button::new("slot.action")
                    .label("Nested native button")
                    .into_any_element(),
            ],
        )]);
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .children([
                state.render(&tabs, BTreeMap::new(), window, cx, emit.clone()),
                state.render(&accordion, slots, window, cx, emit.clone()),
                state.render(&pages, BTreeMap::new(), window, cx, emit),
            ])
            .into_any_element()
    });
    assert!(harness.node("slot.action").is_some());
    harness.click("tabs.other");
    harness.click("details.advanced");
    harness.click("pages.next");
    assert_eq!(
        &*output.borrow(),
        &[
            ("tab".into(), json!("other")),
            ("section".into(), json!({"id":"advanced","expanded":false})),
            ("page".into(), json!(4))
        ]
    );
}

#[gpui::test]
fn layout_slots_keep_asymmetric_split_geometry(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let mut state = KitState::default();
        let split = node(
            "SplitPane",
            "split",
            json!({"ratio":0.3,"minStart":0,"minEnd":0}),
            json!({}),
        );
        let slots = BTreeMap::from([
            (
                "start".into(),
                vec![Button::new("pane.start").label("Start").into_any_element()],
            ),
            (
                "end".into(),
                vec![Button::new("pane.end").label("End").into_any_element()],
            ),
        ]);
        div()
            .w(px(600.))
            .h(px(240.))
            .child(state.render(&split, slots, window, cx, Rc::new(|_, _| {})))
            .into_any_element()
    });
    let start = harness.bounds("pane.start").expect("start slot");
    let end = harness.bounds("pane.end").expect("end slot");
    assert!(start.origin.x < end.origin.x);
    assert!(
        end.origin.x > px(100.) && end.origin.x < px(300.),
        "30% split should be left of midpoint"
    );
}

#[gpui::test]
fn clipboard_refusal_routes_data_only_and_tears_down(cx: &mut TestAppContext) {
    let owner = gpui::EffectOwner::new();
    let state = Rc::new(RefCell::new(KitState::default()));
    let build_state = state.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let descriptor = node(
        "TextInput",
        "clipboard.field",
        json!({"text":"retained"}),
        json!({"clipboardDenied":"refusal","change":"change"}),
    );
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let output = output.clone();
        let rendered = build_state.borrow_mut().render(
            &descriptor,
            BTreeMap::new(),
            window,
            cx,
            Rc::new(move |action, payload| output.borrow_mut().push((action.to_owned(), payload))),
        );
        gpui::effect_owner(owner, rendered).into_any_element()
    });
    harness.update(|_, cx| cx.set_clipboard_policy(|_, _| false));
    harness.click("clipboard.field");
    let primary = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    harness.keystrokes(&format!("{primary}-a {primary}-x"));
    assert_eq!(&*events.borrow(), &[("refusal".into(), json!("denied"))]);
    harness.update(|_, cx| {
        let mut state = state.borrow_mut();
        let Control::Input(entity) = &state.retained.values().next().expect("retained").control
        else {
            panic!("input")
        };
        assert_eq!(entity.read(cx).value().as_ref(), "retained");
        let empty: Node = serde_json::from_value(json!({"kind":"column","id":"empty"}))
            .expect("empty root fixture");
        state.reconcile(&empty, cx);
        assert!(state.retained.is_empty());
    });
}
