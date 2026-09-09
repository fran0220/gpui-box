use super::*;
use gpui::{Styled, TestAppContext, div, px};
use gpui_kit_testkit::harness::Harness;

fn node(component: &str, id: &str, props: Value, events: Value) -> Node {
    serde_json::from_value(
        json!({"kind":"kit","component":component,"id":id,"props":props,"events":events}),
    )
    .expect("fixture descriptor")
}

#[gpui::test]
fn swatch_reports_exact_native_color_and_disabled_does_not_dispatch(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let nodes = [
        node(
            "ColorSwatch",
            "enabled",
            json!({"color":{"h":0.125,"s":0.75,"l":0.25,"a":0.5},"selected":true}),
            json!({"click":"selected"}),
        ),
        node(
            "ColorSwatch",
            "disabled",
            json!({"color":{"h":0.625,"s":0.25,"l":0.75,"a":1.0},"disabled":true}),
            json!({"click":"forbidden"}),
        ),
    ];
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let output = output.clone();
        let emit: Emit =
            Rc::new(move |action, value| output.borrow_mut().push((action.to_owned(), value)));
        div()
            .flex()
            .gap(px(20.))
            .children(
                nodes
                    .iter()
                    .map(|node| render(node, BTreeMap::new(), window, cx, emit.clone())),
            )
            .into_any_element()
    });
    harness.click("enabled");
    harness.click("disabled");
    assert_eq!(
        *events.borrow(),
        vec![(
            "selected".into(),
            json!({"h":0.125,"s":0.75,"l":0.25,"a":0.5})
        )]
    );
    assert!(harness.node("enabled").expect("enabled swatch").selected);
    assert!(harness.node("disabled").expect("disabled swatch").disabled);
}

#[test]
fn form_queries_distinguish_pending_busy_invalid_and_valid() {
    for (state, invalid, busy) in [
        ("pending", false, false),
        ("validating", false, true),
        ("invalid", true, false),
        ("valid", false, false),
    ] {
        let node = node(
            "FormField",
            "field",
            json!({"label":"Name","validation":state,"reason":"Refused"}),
            json!({}),
        );
        assert_eq!(
            invoke(&node, "is_invalid", &json!({}), true).expect("invalid query"),
            json!(invalid)
        );
        assert_eq!(
            invoke(&node, "is_validating", &json!({}), true).expect("validating query"),
            json!(busy)
        );
        assert!(invoke(&node, "is_invalid", &json!({}), false).is_err());
    }
}

#[gpui::test]
fn native_button_and_asymmetric_toggle_selection_intents(cx: &mut TestAppContext) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let nodes = [
        node(
            "Button",
            "button",
            json!({"label":"Run","variant":"light","color":{"semantic":"info"}}),
            json!({"click":"run"}),
        ),
        node(
            "Button",
            "loading",
            json!({"label":"Waiting","loading":true}),
            json!({"click":"forbidden"}),
        ),
        node(
            "Toggle",
            "toggle",
            json!({"label":"Enabled","pressed":true}),
            json!({"press":"toggle"}),
        ),
        node(
            "ToggleGroup",
            "many",
            json!({"items":[{"id":"alpha","label":"Alpha"},{"id":"beta","label":"Beta"},{"id":"gamma","label":"Gamma","disabled":true}],"pressed":["beta"]}),
            json!({"change":"many"}),
        ),
        node(
            "ToggleGroup",
            "single",
            json!({"items":[{"id":"alpha","label":"Alpha"},{"id":"beta","label":"Beta"}],"pressed":["beta"],"selection":"atMostOne"}),
            json!({"change":"single"}),
        ),
    ];
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let output = output.clone();
        let emit: Emit =
            Rc::new(move |action, value| output.borrow_mut().push((action.to_owned(), value)));
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .children(
                nodes
                    .iter()
                    .map(|node| render(node, BTreeMap::new(), window, cx, emit.clone())),
            )
            .into_any_element()
    });
    for id in [
        "button",
        "loading",
        "toggle",
        "many.alpha",
        "many.gamma",
        "single.alpha",
        "single.beta",
    ] {
        harness.click(id);
    }
    assert_eq!(
        *events.borrow(),
        vec![
            ("run".into(), Value::Null),
            ("toggle".into(), json!(false)),
            (
                "many".into(),
                json!({"pressed":["alpha","beta"],"changed":"alpha"})
            ),
            (
                "single".into(),
                json!({"pressed":["alpha"],"changed":"alpha"})
            ),
            ("single".into(), json!({"pressed":[],"changed":"beta"})),
        ]
    );
}

#[gpui::test]
fn search_retains_entity_selection_and_routes_and_refuses_disabled_commands(
    cx: &mut TestAppContext,
) {
    let state = Rc::new(KitState::default());
    let descriptor = Rc::new(RefCell::new(node(
        "SearchInput",
        "search",
        json!({"name":"Find","placeholder":"Query"}),
        json!({"change":"first"}),
    )));
    let output = Rc::new(RefCell::new(Vec::new()));
    let build_state = state.clone();
    let build_node = descriptor.clone();
    let build_output = output.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let node = build_node.borrow();
        build_state.reconcile(&node, cx);
        let output = build_output.clone();
        build_state.render(
            &node,
            BTreeMap::new(),
            window,
            cx,
            Rc::new(move |action, value| output.borrow_mut().push((action.to_owned(), value))),
        )
    });
    let entity = state.controls_extra.searches.borrow()[&(0, "search".into())]
        .entity
        .clone();
    harness.update(|window, cx| {
        for (method, args) in [
            ("set_value", json!({"value":"aé🙂z"})),
            ("set_name", json!({"name":"Updated name"})),
            ("set_placeholder", json!({"placeholder":"Updated hint"})),
            (
                "set_presentation",
                json!({"name":null,"placeholder":"New hint","size":"lg"}),
            ),
        ] {
            assert_eq!(
                state
                    .invoke(&descriptor.borrow(), method, &args, false, window, cx)
                    .expect("search command"),
                Value::Null
            );
        }
        assert_eq!(
            state
                .invoke(&descriptor.borrow(), "value", &json!({}), true, window, cx)
                .expect("search value"),
            json!("aé🙂z")
        );
        assert!(
            state
                .invoke(
                    &descriptor.borrow(),
                    "set_value",
                    &json!({"value":7}),
                    false,
                    window,
                    cx
                )
                .is_err()
        );
    });
    harness.click("search.query");
    harness.keystrokes("home right shift-right shift-right");
    descriptor
        .borrow_mut()
        .props
        .insert("size".into(), json!("sm"));
    descriptor
        .borrow_mut()
        .events
        .insert("change".into(), "latest".into());
    harness.frame();
    harness.update(|window, cx| {
        assert_eq!(
            entity.entity_id(),
            state.controls_extra.searches.borrow()[&(0, "search".into())]
                .entity
                .entity_id()
        );
        assert_eq!(entity.read(cx).input().read(cx).selected_range(), 1..7);
        state
            .invoke(
                &descriptor.borrow(),
                "set_value",
                &json!({"value":"later"}),
                false,
                window,
                cx,
            )
            .expect("updated route setter");
    });
    assert_eq!(
        output.borrow().last(),
        Some(&("latest".into(), json!("later")))
    );
    harness.update(|window, cx| {
        state
            .invoke(
                &descriptor.borrow(),
                "set_disabled",
                &json!({"disabled":true}),
                false,
                window,
                cx,
            )
            .expect("disable search");
        assert_eq!(
            state
                .invoke(
                    &descriptor.borrow(),
                    "is_disabled",
                    &json!({}),
                    true,
                    window,
                    cx
                )
                .expect("actual disabled query"),
            json!(true)
        );
        for (method, args) in [
            ("set_value", json!({"value":"forbidden"})),
            ("set_disabled", json!({"disabled":false})),
        ] {
            assert!(
                state
                    .invoke(&descriptor.borrow(), method, &args, false, window, cx)
                    .is_err()
            );
        }
        let root = node("Button", "replacement", json!({}), json!({}));
        state.reconcile(&root, cx);
        assert!(
            state
                .invoke(&descriptor.borrow(), "value", &json!({}), true, window, cx)
                .is_err()
        );
        *descriptor.borrow_mut() = root;
    });
    assert!(state.controls_extra.searches.borrow().is_empty());
}

#[gpui::test]
fn managed_settings_never_construct_the_withheld_slot(cx: &mut TestAppContext) {
    let builds = Rc::new(RefCell::new(0usize));
    let build_count = builds.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let mut slots: KitSlots = BTreeMap::new();
        let builds = build_count.clone();
        slots.insert(
            "control".into(),
            Rc::new(move |_, _| {
                *builds.borrow_mut() += 1;
                Button::new("editable.control")
                    .label("Edit")
                    .into_any_element()
            }),
        );
        let managed = node(
            "SettingsRow",
            "managed",
            json!({"label":"Retention","value":"90 days","managed":"Workspace policy","description":"Kept by the workspace","labelWidth":180}),
            json!({}),
        );
        let mut forbidden: KitSlots = BTreeMap::new();
        forbidden.insert(
            "control".into(),
            Rc::new(|_, _| panic!("managed row constructed its control")),
        );
        let editable = node(
            "SettingsRow",
            "editable",
            json!({"label":"Theme","value":"Studio","badge":"Local","searchTerms":["appearance"]}),
            json!({}),
        );
        div()
            .flex()
            .flex_col()
            .child(render(&managed, forbidden, window, cx, Rc::new(|_, _| {})))
            .child(render(&editable, slots, window, cx, Rc::new(|_, _| {})))
            .into_any_element()
    });
    harness.frame();
    assert!(*builds.borrow() > 0);
    assert!(harness.node("managed").expect("managed row").disabled);
    assert!(harness.node("editable.control").is_some());
}
