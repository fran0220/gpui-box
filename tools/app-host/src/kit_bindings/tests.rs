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
        let state = KitState::default();
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
    let state = Rc::new(KitState::default());
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
        let state = &build_state;
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
    let input = match &state.retained.borrow()[&(0, "field".into())].control {
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
    assert_eq!(state.retained.borrow().len(), 1);
    let empty: Node =
        serde_json::from_value(json!({"kind":"column","id":"root"})).expect("empty tree");
    harness.update(|_, cx| state.reconcile(&empty, cx));
    assert!(state.retained.borrow().is_empty());
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
    let state = Rc::new(KitState::default());
    let descriptor = node(
        "Select",
        "select",
        json!({"options":[{"id":"alpha","label":"Alpha","description":"First description","group":"Primary"},{"id":"beta","label":"Beta","description":"Second description","group":"Secondary"}],"selected":"alpha"}),
        json!({"change":"selected"}),
    );
    let output = Rc::new(RefCell::new(Vec::new()));
    let events = output.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let events = events.clone();
        state.render(
            &descriptor,
            BTreeMap::new(),
            window,
            cx,
            Rc::new(move |action, value| events.borrow_mut().push((action.to_owned(), value))),
        )
    });
    harness.click("select");
    harness.frame();
    assert!(harness.node("select.group.Primary").is_some());
    assert!(harness.node("select.group.Secondary").is_some());
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
        let state = KitState::default();
        let mut slots = KitSlots::new();
        slots.insert(
            "advanced".into(),
            Rc::new(|_, _| {
                Button::new("slot.action")
                    .label("Nested native button")
                    .into_any_element()
            }),
        );
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
        let state = KitState::default();
        let split = node(
            "SplitPane",
            "split",
            json!({"ratio":0.3,"minStart":0,"minEnd":0}),
            json!({}),
        );
        let mut slots = KitSlots::new();
        slots.insert(
            "start".into(),
            Rc::new(|_, _| Button::new("pane.start").label("Start").into_any_element()),
        );
        slots.insert(
            "end".into(),
            Rc::new(|_, _| Button::new("pane.end").label("End").into_any_element()),
        );
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
    let state = Rc::new(KitState::default());
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
        let rendered = build_state.render(
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
        let entry = state
            .retained
            .borrow()
            .values()
            .next()
            .expect("retained")
            .clone();
        let Control::Input(entity) = &entry.control else {
            panic!("input")
        };
        assert_eq!(entity.read(cx).value().as_ref(), "retained");
        let empty: Node = serde_json::from_value(json!({"kind":"column","id":"empty"}))
            .expect("empty root fixture");
        state.reconcile(&empty, cx);
        assert!(state.retained.borrow().is_empty());
    });
}

#[gpui::test]
fn reusable_slot_recurses_into_shared_state_without_borrowing_outer_map(cx: &mut TestAppContext) {
    let state = Rc::new(KitState::default());
    let nested_state = state.clone();
    let mut slots = KitSlots::new();
    slots.insert(
        "content".into(),
        Rc::new(move |window, cx| {
            nested_state.render(
                &node("TextInput", "nested.field", json!({}), json!({})),
                KitSlots::new(),
                window,
                cx,
                Rc::new(|_, _| {}),
            )
        }),
    );
    let container = node(
        "ScrollArea",
        "nested.scroll",
        json!({"height":180}),
        json!({}),
    );
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        state.render(&container, slots.clone(), window, cx, Rc::new(|_, _| {}))
    });
    harness.click("nested.field");
    harness.keystrokes("a");
    harness.frame();
    harness.keystrokes("b");
    assert_eq!(
        harness
            .node("nested.field")
            .expect("retained slot")
            .value
            .as_deref(),
        Some("ab")
    );
}

#[gpui::test]
fn lazy_list_rebuilds_scrolled_slots_and_retains_nested_entities(cx: &mut TestAppContext) {
    let state = Rc::new(KitState::default());
    let counts = Rc::new(RefCell::new(vec![0usize; 40]));
    let mut slots = KitSlots::new();
    for index in 0..40 {
        let counts = counts.clone();
        let state = state.clone();
        slots.insert(
            format!("item-{index}"),
            Rc::new(move |window, cx| {
                counts.borrow_mut()[index] += 1;
                state.render(
                    &node("TextInput", &format!("input-{index}"), json!({}), json!({})),
                    KitSlots::new(),
                    window,
                    cx,
                    Rc::new(|_, _| {}),
                )
            }),
        );
    }
    let rows = (0..40)
        .map(|index| json!({"id":format!("item-{index}"),"label":format!("Row {index}")}))
        .collect::<Vec<_>>();
    let list = node(
        "List",
        "virtual",
        json!({"rows":rows,"visibleRows":3,"rowHeight":40}),
        json!({}),
    );
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        div()
            .w(px(400.))
            .child(state.render(&list, slots.clone(), window, cx, Rc::new(|_, _| {})))
            .into_any_element()
    });
    assert_eq!(
        counts.borrow()[20],
        0,
        "offscreen rows must not be eagerly constructed"
    );
    harness.click("input-0");
    harness.keystrokes("a");
    harness.scroll("virtual", 800.);
    harness.frame();
    assert!(
        counts.borrow()[20] > 0,
        "scroll must construct new visible native rows"
    );
    assert!(harness.node("input-20").is_some());
    harness.scroll("virtual", -800.);
    harness.frame();
    harness.click("input-0");
    harness.keystrokes("b");
    assert_eq!(
        harness
            .node("input-0")
            .expect("returned row input")
            .value
            .as_deref(),
        Some("ab")
    );
    assert!(
        counts.borrow()[0] > 1,
        "returning row requires fresh elements"
    );
}

#[gpui::test]
fn native_methods_preserve_typed_results_and_actual_disabled_refusal(cx: &mut TestAppContext) {
    let state = Rc::new(KitState::default());
    let input = node(
        "TextInput",
        "methods.input",
        json!({}),
        json!({"change":"changed"}),
    );
    let select = node(
        "Select",
        "methods.select",
        json!({"options":[{"id":"alpha","label":"Alpha"},{"id":"beta","label":"Beta","description":"Chosen description","group":"Secondary"}],"selected":"alpha"}),
        json!({}),
    );
    let build_state = state.clone();
    let build_nodes = [input.clone(), select.clone()];
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        div()
            .flex()
            .flex_col()
            .children(build_nodes.iter().map(|node| {
                let output = output.clone();
                build_state.render(
                    node,
                    KitSlots::new(),
                    window,
                    cx,
                    Rc::new(move |action, payload| {
                        output.borrow_mut().push((action.to_owned(), payload))
                    }),
                )
            }))
            .into_any_element()
    });
    harness.update(|window, cx| {
        assert!(
            state
                .invoke(&input, "set_value", &json!({"value":42}), false, window, cx)
                .is_err()
        );
        assert_eq!(
            state
                .invoke(
                    &input,
                    "set_value",
                    &json!({"value":"é🙂"}),
                    false,
                    window,
                    cx
                )
                .expect("set value"),
            Value::Null
        );
        assert_eq!(
            state
                .invoke(&input, "value", &json!({}), true, window, cx)
                .expect("query value"),
            json!("é🙂")
        );
        state
            .invoke(
                &select,
                "set_selected",
                &json!({"id":"beta"}),
                false,
                window,
                cx,
            )
            .expect("set selection");
        assert_eq!(
            state
                .invoke(&select, "selected_option", &json!({}), true, window, cx)
                .expect("query option"),
            json!({"id":"beta","label":"Beta","disabled":false,"description":"Chosen description","group":"Secondary"})
        );
        state.invoke(&select,"set_options",&json!({"options":[{"id":"beta","label":"Updated Beta","description":"Updated description","group":"Updated group"}]}),false,window,cx).expect("update option metadata");
        assert_eq!(state.invoke(&select,"selected_option",&json!({}),true,window,cx).expect("updated metadata"),json!({"id":"beta","label":"Updated Beta","disabled":false,"description":"Updated description","group":"Updated group"}));
    });
    assert_eq!(&*events.borrow(), &[("changed".into(), json!("é🙂"))]);
    harness.click("methods.input");
    harness.keystrokes(if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    });
    harness.update(|window, cx| {
        assert_eq!(
            state
                .invoke(&input, "selected_range", &json!({}), true, window, cx)
                .expect("byte selection"),
            json!({"start":0,"end":6})
        );
        state
            .invoke(
                &input,
                "set_text_quietly",
                &json!({"value":"quiet"}),
                false,
                window,
                cx,
            )
            .expect("quiet text");
        state
            .invoke(
                &input,
                "set_disabled",
                &json!({"disabled":true}),
                false,
                window,
                cx,
            )
            .expect("disable input");
        state
            .invoke(
                &select,
                "set_disabled",
                &json!({"disabled":true}),
                false,
                window,
                cx,
            )
            .expect("disable select");
    });
    assert_eq!(
        events.borrow().len(),
        1,
        "quiet command does not echo change"
    );
    harness.frame();
    harness.update(|window, cx| {
        assert_eq!(
            state
                .invoke(&select, "selected_id", &json!({}), true, window, cx)
                .expect("controlled selection"),
            json!("alpha"),
            "explicit caller selection wins on the next render"
        );
        assert_eq!(
            state
                .invoke(&input, "is_disabled", &json!({}), true, window, cx)
                .expect("disabled query"),
            json!(true)
        );
        assert!(
            state
                .invoke(
                    &input,
                    "set_value",
                    &json!({"value":"must refuse"}),
                    false,
                    window,
                    cx
                )
                .is_err()
        );
        assert!(
            state
                .invoke(
                    &select,
                    "set_selected",
                    &json!({"id":"alpha"}),
                    false,
                    window,
                    cx
                )
                .is_err()
        );
        let mut stale = input.clone();
        stale.instance = 99;
        assert!(
            state
                .invoke(&stale, "value", &json!({}), true, window, cx)
                .is_err()
        );
        let mut wrong = input.clone();
        wrong.component = Some("Select".into());
        assert!(
            state
                .invoke(&wrong, "is_open", &json!({}), true, window, cx)
                .is_err()
        );
    });
}

#[gpui::test]
fn every_declared_method_has_native_dispatch(cx: &mut TestAppContext) {
    fn props(component: &str) -> Value {
        if datetime::COMPONENTS.contains(&component) {
            let adapter: Value = serde_json::from_str(include_str!("datetime/fixture/data.json"))
                .expect("asymmetric caller calendar");
            json!({"adapter": adapter})
        } else if component == "FormField" {
            json!({"label":"Field"})
        } else if component == "AspectRatio" {
            json!({"ratio":1.75})
        } else {
            json!({})
        }
    }
    fn example(schema: &Value) -> Value {
        if schema["nullable"] == true {
            return Value::Null;
        }
        if let Some(branches) = schema["oneOf"].as_array() {
            return example(&branches[0]);
        }
        if let Some(values) = schema["enum"].as_array() {
            return values[0].clone();
        }
        match schema["type"].as_str().expect("schema type") {
            "string" => json!("fixture"),
            "boolean" => json!(false),
            "number" => schema["min"].clone(),
            "array" => json!([]),
            "object" => Value::Object(
                schema["fields"]
                    .as_object()
                    .expect("fields")
                    .iter()
                    .map(|(key, value)| (key.clone(), example(value)))
                    .collect(),
            ),
            _ => panic!("unsupported example schema"),
        }
    }
    let methods: Value = serde_json::from_str(include_str!("methods.json")).expect("method schema");
    let state = Rc::new(KitState::default());
    let build_state = state.clone();
    let nodes = methods
        .as_object()
        .expect("components")
        .keys()
        .map(|component| {
            let descriptor = node(component, component, props(component), json!({}));
            validate_descriptor(&descriptor).expect("valid method fixture");
            descriptor
        })
        .collect::<Vec<_>>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        div()
            .flex()
            .flex_col()
            .children(nodes.iter().map(|node| {
                build_state.render(node, KitSlots::new(), window, cx, Rc::new(|_, _| {}))
            }))
            .into_any_element()
    });
    harness.update(|window, cx| {
        for (component, modes) in methods.as_object().expect("components") {
            for (mode, methods) in modes.as_object().expect("modes") {
                for (method, schema) in methods.as_object().expect("methods") {
                    let descriptor = node(component, component, props(component), json!({}));
                    let args = match (component.as_str(), method.as_str()) {
                        ("TransferList", "set_items") => json!({"source":[{"id":"a","label":"Alpha"},{"id":"b","label":"Beta"}],"target":[{"id":"z","label":"Zulu"}]}),
                        ("TransferList", "set_selection") => json!({"source":["b"],"target":["z"]}),
                        ("Calendar", "set_selection") => json!({"days":[31,11]}),
                        ("Calendar", "set_overlay") => json!({"marks":[{"day":31,"label":"Review","tone":"warning"}]}),
                        ("Calendar", "set_hovered_day") => json!({"day":17}),
                        ("Calendar", "show_month") => json!({"month":4}),
                        ("Calendar", "shift") => json!({"delta":1}),
                        ("DateInput", "set_value") => json!({"value":31}),
                        ("Calendar" | "RangePicker", "set_range") => json!({"range":{"start":31,"end":11}}),
                        ("TimeInput", "set_value") => json!({"value":{"hour":3,"minute":1,"second":2}}),
                        _ => example(&schema["args"]),
                    };
                    let registry = crate::references::Registry::new();
                    let owner = gpui::EffectOwner::new();
                    let refs = registry.registration(&descriptor, owner);
                    let result = state.invoke_registered(
                        &descriptor,
                        method,
                        &args,
                        mode == "query",
                        window,
                        cx,
                        &refs,
                    );
                    assert!(
                        result.is_ok(),
                        "{component}.{method} has no valid native dispatch: {result:?}"
                    );
                }
            }
        }
    });
}

#[gpui::test]
fn merged_date_dispatch_validates_calls_routes_native_input_and_releases_owner(
    cx: &mut TestAppContext,
) {
    let adapter: Value = serde_json::from_str(include_str!("datetime/fixture/data.json"))
        .expect("caller date table");
    let mut date = node(
        "DateInput",
        "date",
        json!({"adapter":adapter}),
        json!({"unparsable":"refused"}),
    );
    date.instance = 19;
    let state = Rc::new(KitState::default());
    let build = state.clone();
    let descriptor = date.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let output = output.clone();
        build.render(
            &descriptor,
            KitSlots::new(),
            window,
            cx,
            Rc::new(move |action, value| {
                output.borrow_mut().push((action.to_owned(), value));
            }),
        )
    });
    harness.click("date.field");
    harness.keystrokes("x");
    assert_eq!(
        events.borrow().last(),
        Some(&(
            "refused".into(),
            json!({"text":"x","message":"unknown spelling"})
        ))
    );
    harness.update(|window, cx| {
        assert_eq!(
            state
                .invoke(&date, "shown_text", &json!({}), true, window, cx)
                .expect("native edit"),
            json!("x")
        );
        for args in [
            json!({"value":"31"}),
            json!({"value":999}),
            json!({"value":31,"extra":true}),
        ] {
            assert!(
                state
                    .invoke(&date, "set_value", &args, false, window, cx)
                    .is_err()
            );
        }
        assert_eq!(
            state
                .invoke(&date, "shown_text", &json!({}), true, window, cx)
                .expect("rejected calls preserve edit"),
            json!("x")
        );
        assert!(
            state
                .invoke(&date, "calendar", &json!({}), true, window, cx)
                .is_err()
        );
        let mut other_owner = date.clone();
        other_owner.instance = 20;
        assert!(
            state
                .invoke(&other_owner, "current", &json!({}), true, window, cx)
                .is_err()
        );
        state
            .invoke(&date, "set_value", &json!({"value":31}), false, window, cx)
            .expect("valid setter");
        assert_eq!(
            state
                .invoke(&date, "current", &json!({}), true, window, cx)
                .expect("value query"),
            json!(31)
        );
        assert_eq!(
            state
                .invoke(&date, "shown_text", &json!({}), true, window, cx)
                .expect("formatted caller label"),
            json!("second C")
        );
        state.reconcile(&node("Button", "replacement", json!({}), json!({})), cx);
        assert!(
            state
                .invoke(&date, "current", &json!({}), true, window, cx)
                .is_err()
        );
    });
}

#[gpui::test]
fn host_overlay_factories_reopen_with_retained_input_and_release_state(cx: &mut TestAppContext) {
    for component in ["Popover", "Dialog"] {
        let state = Rc::new(KitState::default());
        let weak = Rc::downgrade(&state);
        let mut surface = node(component, "surface", json!({}), json!({}));
        surface.slots.insert(
            "content".into(),
            vec![node("TextInput", "surface.input", json!({}), json!({}))],
        );
        let descriptor = Rc::new(RefCell::new(surface));
        let build_descriptor = descriptor.clone();
        let (outgoing, _incoming) = std::sync::mpsc::sync_channel(32);
        let renderer = crate::NodeRenderer {
            outgoing,
            kit: weak.clone(),
            rendered_revision: Rc::new(std::cell::Cell::new(1)),
            clipboard: crate::clipboard::Policy::default(),
        };
        let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
            let descriptor = build_descriptor.borrow();
            if let Some(state) = renderer.kit.upgrade() {
                state.reconcile(&descriptor, cx);
            }
            renderer.node(&descriptor, 1, window, cx)
        });
        harness.update(|window, cx| {
            state
                .invoke(&descriptor.borrow(), "open", &json!({}), false, window, cx)
                .expect("open surface");
        });
        harness.frame();
        harness.click("surface.input");
        harness.keystrokes("a");
        {
            let mut descriptor = descriptor.borrow_mut();
            descriptor.props.insert(
                if component == "Popover" {
                    "trigger"
                } else {
                    "title"
                }
                .into(),
                json!("Updated while open"),
            );
            descriptor.slots.get_mut("content").expect("body")[0]
                .props
                .insert("placeholder".into(), json!("Updated nested option"));
        }
        harness.frame();
        harness.update(|window, cx| {
            assert_eq!(
                state
                    .invoke(
                        &descriptor.borrow(),
                        "is_open",
                        &json!({}),
                        true,
                        window,
                        cx
                    )
                    .expect("open query"),
                json!(true)
            );
        });
        assert_eq!(
            harness
                .node("surface.input")
                .expect("updated body")
                .value
                .as_deref(),
            Some("a")
        );
        harness.update(|window, cx| {
            state
                .invoke(&descriptor.borrow(), "close", &json!({}), false, window, cx)
                .expect("close surface");
        });
        harness.frame();
        harness.update(|window, cx| {
            state
                .invoke(&descriptor.borrow(), "open", &json!({}), false, window, cx)
                .expect("reopen surface");
        });
        harness.frame();
        harness.click("surface.input");
        harness.keystrokes("b");
        assert_eq!(
            harness
                .node("surface.input")
                .expect("fresh reopened body")
                .value
                .as_deref(),
            Some("ab"),
            "{component} body must rebuild without replacing retained input"
        );
        drop(state);
        assert!(
            weak.upgrade().is_none(),
            "{component} body factory must not retain KitState"
        );
        harness.frame();
        assert!(
            harness.node("surface.input").is_none(),
            "closed host must remove nested action target"
        );
    }
}

#[gpui::test]
fn native_list_and_tabs_reorder_report_intent_without_mutating_caller_order(
    cx: &mut TestAppContext,
) {
    for component in ["List", "Tabs"] {
        for (disabled, delay) in [(false, 0), (true, 0), (false, 200)] {
            let rows = json!([{"id":"alpha","label":"Alpha"},{"id":"beta","label":"Beta"},{"id":"gamma","label":"Gamma"}]);
            let mut props = json!({"reorderable":true,"disabled":disabled});
            props[if component == "List" { "rows" } else { "tabs" }] = rows;
            let descriptor = node(
                component,
                "reorder-control",
                props,
                if disabled {
                    json!({})
                } else {
                    json!({"reorder":"dropped"})
                },
            );
            let events = Rc::new(RefCell::new(Vec::new()));
            let output = events.clone();
            let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
                let output = output.clone();
                div()
                    .w(px(520.))
                    .child(KitState::default().render(
                        &descriptor,
                        KitSlots::new(),
                        window,
                        cx,
                        Rc::new(move |action, payload| {
                            output.borrow_mut().push((action.to_owned(), payload))
                        }),
                    ))
                    .into_any_element()
            });
            let before = harness
                .snapshot()
                .children_of("reorder-control")
                .iter()
                .map(|node| node.id.clone())
                .collect::<Vec<_>>();
            harness.drag_start("reorder-control.gamma");
            if !disabled {
                assert!(
                    harness.node("dnd.drag").is_some(),
                    "{component} starts native drag"
                );
            }
            let target = if component == "List" {
                harness.point_down("reorder-control.alpha", 0.2)
            } else {
                harness.point_across("reorder-control.alpha", 0.2)
            };
            harness.drag_to(target);
            if delay > 0 {
                harness.advance(std::time::Duration::from_millis(delay));
            }
            harness.drop_here();
            if disabled {
                assert!(
                    events.borrow().is_empty(),
                    "disabled {component} cannot reorder"
                );
            } else {
                assert_eq!(
                    &*events.borrow(),
                    &[(
                        "dropped".into(),
                        json!({"id":"gamma","source":"reorder-control","label":"Gamma","kind":"row","anchor":"alpha","position":"before","velocity":{"x":0.,"y":0.}})
                    )],
                    "{component} emits native reorder"
                );
            }
            assert_eq!(
                harness
                    .snapshot()
                    .children_of("reorder-control")
                    .iter()
                    .map(|node| node.id.clone())
                    .collect::<Vec<_>>(),
                before,
                "{component} retains caller order"
            );
        }
    }
}

#[test]
fn drop_velocity_serialization_preserves_axes_and_signs() {
    use gpui_kit::{
        interaction::{DragItem, DropIntent, DropPosition},
        motion::Velocity,
    };
    let payload = drop_payload(&DropIntent {
        item: DragItem::new("source", "item", "Label"),
        position: DropPosition::After("anchor".into()),
        velocity: Velocity::new(-12.5, 84.),
    });
    assert_eq!(payload["velocity"], json!({"x":-12.5,"y":84.}));
    assert_eq!(payload["position"], "after");
}
