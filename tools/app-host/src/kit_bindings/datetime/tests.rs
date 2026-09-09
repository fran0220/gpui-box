// These tests deliberately panic at the operation that violates a fixture's contract.
#![allow(clippy::unwrap_used)]
use super::*;
fn data() -> Value {
    serde_json::from_str(include_str!("fixture/data.json")).unwrap()
}
#[test]
fn data_adapter_respects_opaque_months_labels_refusals_and_range_availability() {
    let data = Rc::new(RefCell::new(Data::parse(&data()).unwrap()));
    let adapter = Adapter(data.clone());
    assert_eq!(adapter.shift_month(MonthKey(90), 1), Some(MonthKey(4)));
    assert_eq!(adapter.shift_month(MonthKey(4), 1), None);
    assert_eq!(adapter.parse_day("alpha"), Ok(Day(11)));
    assert_eq!(adapter.parse_day("ALPHA"), Err("unknown spelling".into()));
    assert_eq!(
        adapter.days_in(Day(11), Day(31)),
        Some(vec![Day(11), Day(17), Day(31)])
    );
    assert_eq!(
        adapter.is_selectable(Day(17)),
        Selectability::blocked("maintenance")
    );
    assert_eq!(
        adapter.format_time(TimeOfDay::new(3, 1).with_second(2)),
        "three·01·c"
    );
    data.borrow_mut().complete_range = false;
    assert_eq!(adapter.days_in(Day(11), Day(31)), None);
}
#[test]
fn caller_tables_reject_ambiguous_aliases_ragged_grids_and_unknown_days() {
    let mut v = data();
    v["days"][1]["aliases"] = json!(["alpha"]);
    assert!(Data::parse(&v).is_err());
    let mut v = data();
    v["months"][0]["weeks"][0] = json!([{"day":11}]);
    assert!(Data::parse(&v).is_err());
    let mut v = data();
    v["months"][0]["weeks"][0][0]["day"] = json!(999);
    assert!(Data::parse(&v).is_err());
    let mut v = data();
    v["days"][1]["day"] = json!(10);
    assert!(Data::parse(&v).is_err());
}

#[cfg(feature = "capture")]
#[gpui::test]
fn native_date_commands_queries_retention_and_teardown(cx: &mut gpui::TestAppContext) {
    use gpui_kit_testkit::harness::Harness;
    let methods: Value = serde_json::from_str(include_str!("fixture/methods.json")).unwrap();
    for component in COMPONENTS {
        let descriptor:Node=serde_json::from_value(json!({"kind":"kit","id":"date","component":component,"props":{"adapter":data()},"events":{}})).unwrap();
        validate(&descriptor).unwrap();
        let state = Rc::new(State::default());
        let build = state.clone();
        let n = descriptor.clone();
        let mut harness = Harness::new(cx, gpui_kit::install, move |w, cx| {
            build.render(&n, KitSlots::new(), w, cx, Rc::new(|_, _| {}))
        });
        let invoke = |h: &mut Harness, name: &str, args: Value, query: bool| {
            h.update(|w, cx| state.invoke(&descriptor, name, &args, query, w, cx))
        };
        for (method, _) in methods[component]["invoke"].as_object().unwrap() {
            let args = match method.as_str() {
                "set_disabled" => json!({"disabled":false}),
                "set_invalid" => json!({"invalid":true}),
                "set_required" => json!({"required":true}),
                "set_control_size" => json!({"size":"lg"}),
                "set_selection" => json!({"days":[31,11]}),
                "set_multi" => json!({"multi":true}),
                "set_overlay" => json!({"marks":[{"day":31,"label":"review","tone":"warning"}]}),
                "set_range" => json!({"range":{"start":31,"end":11}}),
                "set_hovered_day" => json!({"day":17}),
                "shift" => json!({"delta":1}),
                "show_month" => json!({"month":4}),
                "set_seconds" => json!({"seconds":true}),
                "set_value" if *component == "TimeInput" => {
                    json!({"value":{"hour":3,"minute":1,"second":2}})
                }
                "set_value" => json!({"value":31}),
                "open" | "close" | "toggle" | "reset_navigation" => json!({}),
                _ => panic!("missing independent test input {component}.{method}"),
            };
            assert_eq!(
                invoke(&mut harness, method, args, false).unwrap(),
                Value::Null,
                "{component}.{method}"
            );
        }
        for (method, _) in methods[component]["query"].as_object().unwrap() {
            assert!(
                invoke(&mut harness, method, json!({}), true).is_ok(),
                "{component}.{method}"
            );
        }
        match *component {
            "Calendar" => {
                assert_eq!(
                    invoke(&mut harness, "selection", json!({}), true).unwrap(),
                    json!([31, 11])
                );
                assert_eq!(
                    invoke(&mut harness, "shown_month", json!({}), true).unwrap(),
                    json!(4)
                );
            }
            "DateInput" => {
                assert_eq!(
                    invoke(&mut harness, "current", json!({}), true).unwrap(),
                    json!(31)
                );
                assert_eq!(
                    invoke(&mut harness, "shown_text", json!({}), true).unwrap(),
                    json!("second C")
                );
                invoke(&mut harness, "open", json!({}), false).unwrap();
                harness.frame();
                assert_eq!(
                    invoke(&mut harness, "is_open", json!({}), true).unwrap(),
                    json!(true)
                );
                invoke(&mut harness, "close", json!({}), false).unwrap();
                invoke(&mut harness, "open", json!({}), false).unwrap();
            }
            "RangePicker" => {
                assert_eq!(
                    invoke(&mut harness, "current_range", json!({}), true).unwrap(),
                    json!({"start":31,"end":11})
                );
                assert_eq!(
                    invoke(&mut harness, "state", json!({}), true).unwrap(),
                    json!("end before start")
                );
                invoke(
                    &mut harness,
                    "set_range",
                    json!({"range":{"start":11,"end":31}}),
                    false,
                )
                .unwrap();
                assert_eq!(
                    invoke(&mut harness, "blocked", json!({}), true).unwrap(),
                    json!({"kind":"blocked","days":[{"day":17,"reason":"maintenance"}]})
                );
            }
            "TimeInput" => assert_eq!(
                invoke(&mut harness, "current", json!({}), true).unwrap(),
                json!({"hour":3,"minute":1,"second":2,"meridiem":null})
            ),
            _ => unreachable!(),
        }
        let before = state.entries.borrow()[&(0, "date".into())].clone();
        harness.frame();
        assert!(Rc::ptr_eq(
            &before,
            &state.entries.borrow()[&(0, "date".into())]
        ));
        assert!(
            invoke(
                &mut harness,
                "set_disabled",
                json!({"disabled":"yes"}),
                false
            )
            .is_err()
        );
        invoke(
            &mut harness,
            "set_disabled",
            json!({"disabled":true}),
            false,
        )
        .unwrap();
        harness.frame();
        assert!(
            invoke(
                &mut harness,
                "set_disabled",
                json!({"disabled":false}),
                false
            )
            .is_err(),
            "native disabled survives rerender"
        );
        let method = methods[component]["query"]
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap();
        assert!(invoke(&mut harness, method, json!({}), true).is_ok());
        let empty: Node = serde_json::from_value(json!({"kind":"column","id":"empty"})).unwrap();
        harness.update(|_, cx| state.reconcile(&empty, cx));
        assert!(state.entries.borrow().is_empty());
        assert!(invoke(&mut harness, method, json!({}), true).is_err());
    }
}

#[cfg(feature = "capture")]
#[gpui::test]
fn native_date_pointer_keyboard_and_typed_refusal(cx: &mut gpui::TestAppContext) {
    use gpui_kit_testkit::harness::Harness;
    for (component, event, target, key, expected) in [
        ("Calendar", "pick", "date.day-31", "", json!(31)),
        (
            "RangePicker",
            "startPick",
            "date.calendar.day-31",
            "",
            json!(31),
        ),
        (
            "TimeInput",
            "change",
            "date.hour",
            "up",
            json!({"hour":1,"minute":0,"second":null,"meridiem":null}),
        ),
        (
            "DateInput",
            "unparsable",
            "date.field",
            "x",
            json!({"text":"x","message":"unknown spelling"}),
        ),
    ] {
        let n:Node=serde_json::from_value(json!({"kind":"kit","id":"date","component":component,"props":{"adapter":data()},"events":{event:"action"}})).unwrap();
        let events = Rc::new(RefCell::new(Vec::new()));
        let output = events.clone();
        let state = State::default();
        let mut harness = Harness::new(cx, gpui_kit::install, move |w, cx| {
            let output = output.clone();
            state.render(
                &n,
                KitSlots::new(),
                w,
                cx,
                Rc::new(move |_, v| output.borrow_mut().push(v)),
            )
        });
        harness.click(target);
        if !key.is_empty() {
            harness.keystrokes(key);
        }
        assert_eq!(events.borrow().last(), Some(&expected), "{component}");
    }
}

#[cfg(feature = "capture")]
#[gpui::test]
fn locale_and_size_rerender_preserve_live_editor_and_navigation(cx: &mut gpui::TestAppContext) {
    use gpui_kit_testkit::harness::Harness;
    let state = Rc::new(State::default());
    let build = state.clone();
    let node = Rc::new(RefCell::new(
        serde_json::from_value::<Node>(
            json!({"kind":"kit","id":"date","component":"DateInput","props":{"adapter":data()}}),
        )
        .unwrap(),
    ));
    let current = node.clone();
    let mut h = Harness::new(cx, gpui_kit::install, move |w, cx| {
        build.render(
            &current.borrow(),
            KitSlots::new(),
            w,
            cx,
            Rc::new(|_, _| {}),
        )
    });
    h.click("date.field");
    h.keystrokes("x");
    let before = state.entries.borrow()[&(0, "date".into())].clone();
    node.borrow_mut().props.get_mut("adapter").unwrap()["months"][0]["label"] =
        json!("Renamed month");
    node.borrow_mut().props.insert("size".into(), json!("lg"));
    node.borrow_mut()
        .props
        .insert("required".into(), json!(true));
    h.frame();
    h.update(|w, cx| {
        assert_eq!(
            state
                .invoke(&node.borrow(), "shown_text", &json!({}), true, w, cx)
                .unwrap(),
            json!("x")
        )
    });
    assert!(Rc::ptr_eq(
        &before,
        &state.entries.borrow()[&(0, "date".into())]
    ));
    h.update(|w, cx| {
        state
            .invoke(&node.borrow(), "open", &json!({}), false, w, cx)
            .unwrap()
    });
    node.borrow_mut().props.get_mut("adapter").unwrap()["parseError"] =
        json!("New refusal wording");
    h.frame();
    h.update(|w, cx| {
        assert_eq!(
            state
                .invoke(&node.borrow(), "is_open", &json!({}), true, w, cx)
                .unwrap(),
            json!(true)
        )
    });
}
