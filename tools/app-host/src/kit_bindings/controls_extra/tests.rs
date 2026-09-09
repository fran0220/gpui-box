use super::*;
use gpui::{Styled, TestAppContext, div, px};
use gpui_kit_testkit::harness::Harness;

#[gpui::test]
fn typed_button_group_uses_guarded_child_actions_and_refuses_stale_context(
    cx: &mut TestAppContext,
) {
    use crate::construction::{NativeBuildContext, TypedSlots};
    use std::{cell::Cell, sync::mpsc};
    let descriptor: Node = serde_json::from_value(json!({
        "kind":"kit","component":"ButtonGroup","id":"group","instance":7,
        "props":{"size":"sm"},"slots":{"buttons":[
            {"kind":"button","id":"legacy","instance":7,"text":"Legacy","action":"legacy-action"},
            {"kind":"kit","component":"Button","id":"native","instance":7,"props":{"label":"Native","checkedState":true},"events":{"click":"native-action"}},
            {"kind":"kit","component":"Button","id":"disabled","instance":7,"props":{"label":"Refused","disabled":true}}
        ]}
    })).expect("typed group fixture");
    let kit = Rc::new(KitState::default());
    let (outgoing, events) = mpsc::sync_channel(8);
    let mut clipboard = crate::clipboard::Policy::default();
    cx.update(|cx| clipboard.reconcile(&descriptor, &BTreeMap::new(), cx));
    let revision = Rc::new(Cell::new(1));
    let renderer = crate::NodeRenderer {
        outgoing,
        kit: Rc::downgrade(&kit),
        rendered_revision: revision.clone(),
        clipboard,
    };
    let typed = TypedSlots::new(renderer, &descriptor, 1);
    let build_typed = typed.clone();
    let build_node = descriptor.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        button_group(
            &build_node,
            NativeBuildContext {
                typed: build_typed.clone(),
                ..Default::default()
            },
            window,
            cx,
        )
        .expect("guarded group")
        .into_any_element()
    });
    harness.click("legacy");
    harness.click("native");
    harness.click("disabled");
    assert_eq!(
        events.try_recv().expect("legacy event")["action"],
        "legacy-action"
    );
    assert_eq!(
        events.try_recv().expect("native event")["action"],
        "native-action"
    );
    assert!(events.try_recv().is_err());
    harness.update(|window, cx| {
        revision.set(2);
        assert!(
            button_group(
                &descriptor,
                NativeBuildContext {
                    typed: typed.clone(),
                    ..Default::default()
                },
                window,
                cx
            )
            .is_err()
        );
        revision.set(1);
    });
}

#[gpui::test]
fn keymap_reports_native_identities_and_keeps_recording_until_hidden(cx: &mut TestAppContext) {
    let state = Rc::new(KitState::default());
    let commands = json!([
        {"id":"save","label":"Save","context":"Editor","defaults":["ctrl-s"],"bindings":[{"id":"custom","keystroke":"ctrl-shift-s","conflict":"Other action","provenance":"Fixture"}],"keywords":["persist"]},
        {"id":"locked","label":"Locked","refusal":"Policy"}
    ]);
    let descriptor = Rc::new(RefCell::new(node(
        "KeymapEditor",
        "keymap",
        json!({"commands":commands}),
        json!({"remove":"remove","reset":"reset","addCaptured":"add","recordingCancelled":"cancel"}),
    )));
    let events = Rc::new(RefCell::new(Vec::new()));
    let (build_state, build_node, output) = (state.clone(), descriptor.clone(), events.clone());
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let node = build_node.borrow();
        build_state.reconcile(&node, cx);
        let output = output.clone();
        build_state.render(
            &node,
            BTreeMap::new(),
            window,
            cx,
            Rc::new(move |action, value| output.borrow_mut().push((action.to_owned(), value))),
        )
    });
    assert!(harness.node("keymap.locked.add").is_none());
    harness.click("keymap.save.binding.custom.remove");
    harness.click("keymap.save.reset");
    assert_eq!(
        *events.borrow(),
        vec![
            (
                "remove".into(),
                json!({"command_id":"save","binding_id":"custom"})
            ),
            ("reset".into(), json!({"command_id":"save"}))
        ]
    );
    harness.click("keymap.save.add");
    harness.update(|window,cx| {
        assert_eq!(state.invoke(&descriptor.borrow(),"active_command",&json!({}),true,window,cx).expect("active"),json!("save"));
        let result = state.invoke(&descriptor.borrow(),"current_commands",&json!({}),true,window,cx).expect("commands");
        assert_eq!(result[0]["bindings"][0],json!({"id":"custom","keystroke":"ctrl-shift-s","conflict":"Other action","provenance":"Fixture"}));
        assert_eq!(result[1]["refusal"],json!("Policy"));
        state.invoke(&descriptor.borrow(),"set_commands",&json!({"commands":commands}),false,window,cx).expect("replace commands");
        assert_eq!(state.invoke(&descriptor.borrow(),"active_command",&json!({}),true,window,cx).expect("retained active"),json!("save"));
        state.invoke(&descriptor.borrow(),"set_query",&json!({"query":"unmatched"}),false,window,cx).expect("filter");
    });
    assert_eq!(
        events.borrow().last(),
        Some(&("cancel".into(), json!({"command_id":"save"})))
    );
    harness.update(|window, cx| {
        assert_eq!(
            state
                .invoke(
                    &descriptor.borrow(),
                    "active_command",
                    &json!({}),
                    true,
                    window,
                    cx
                )
                .expect("cancelled"),
            Value::Null
        );
        state
            .invoke(
                &descriptor.borrow(),
                "set_query",
                &json!({"query":""}),
                false,
                window,
                cx,
            )
            .expect("clear filter");
    });
    harness.click("keymap.save.add");
    harness.keystrokes("ctrl-k");
    assert_eq!(
        events.borrow().last(),
        Some(&(
            "add".into(),
            json!({"command_id":"save","keystroke":"ctrl-k"})
        ))
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
            .expect("disable");
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
                .expect("disabled"),
            json!(true)
        );
        assert!(
            state
                .invoke(
                    &descriptor.borrow(),
                    "set_query",
                    &json!({"query":"save"}),
                    false,
                    window,
                    cx
                )
                .is_err()
        );
    });
    assert!(harness.node("keymap.save.add").is_none());
    *descriptor.borrow_mut() = node("Button", "replacement", json!({}), json!({}));
    harness.update(|_, cx| {
        state.reconcile(&descriptor.borrow(), cx);
        cx.refresh_windows();
    });
    assert!(state.controls_extra.keymaps.borrow().is_empty());
}

#[gpui::test]
fn typed_containers_preserve_child_effect_owners_after_native_transforms(cx: &mut TestAppContext) {
    use gpui::{EffectOwner, EffectScoped};
    use gpui_kit::controls::button::ButtonGroup;
    use gpui_kit::controls::settings_row::{SettingsList, SettingsSection};
    let first = EffectOwner::new();
    let second = EffectOwner::new();
    let section_owner = EffectOwner::new();
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let button = |id: &'static str| {
            let output = output.clone();
            Button::new(id).label(id).on_click(move |_, cx| {
                output.borrow_mut().push((id, cx.current_effect_owner()));
            })
        };
        div()
            .flex()
            .flex_col()
            .child(ButtonGroup::new("group").children([
                EffectScoped::new(first, button("first")),
                EffectScoped::new(second, button("second")),
            ]))
            .child(
                ButtonGroup::new("disabled-group")
                    .disabled(true)
                    .child(EffectScoped::new(first, button("disabled-child"))),
            )
            .child(
                SettingsList::new("settings")
                    .query("needle")
                    .section(EffectScoped::new(
                        section_owner,
                        SettingsSection::new("section", "General").rows([
                            EffectScoped::new(
                                second,
                                SettingsRow::new("matching", "Needle")
                                    .control(button("row-action")),
                            ),
                            EffectScoped::new(
                                first,
                                SettingsRow::new("excluded", "Unrelated")
                                    .control(button("excluded-action")),
                            ),
                        ]),
                    )),
            )
            .into_any_element()
    });
    harness.click("first");
    harness.click("second");
    harness.click("row-action");
    harness.click("disabled-child");
    assert!(harness.node("excluded-action").is_none());
    assert_eq!(
        *events.borrow(),
        vec![
            ("first", Some(first)),
            ("second", Some(second)),
            ("row-action", Some(second)),
        ]
    );
}

fn node(component: &str, id: &str, props: Value, events: Value) -> Node {
    serde_json::from_value(
        json!({"kind":"kit","component":component,"id":id,"props":props,"events":events}),
    )
    .expect("fixture descriptor")
}

#[gpui::test]
fn number_native_steps_queries_options_and_disabled_commands(cx: &mut TestAppContext) {
    let state = Rc::new(KitState::default());
    let descriptor = Rc::new(RefCell::new(node(
        "NumberInput",
        "number",
        json!({"value":7.5,"min":9,"max":-3,"step":2,"precision":1,"name":"Amount"}),
        json!({"change":"changed"}),
    )));
    let events = Rc::new(RefCell::new(Vec::new()));
    let (build_state, build_node, output) = (state.clone(), descriptor.clone(), events.clone());
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let node = build_node.borrow();
        build_state.reconcile(&node, cx);
        let output = output.clone();
        build_state.render(
            &node,
            BTreeMap::new(),
            window,
            cx,
            Rc::new(move |_, value| output.borrow_mut().push(value)),
        )
    });
    harness.click("number.increment");
    assert_eq!(*events.borrow(), vec![json!(9.0)]);
    harness.update(|window, cx| {
        for (method, args, expected) in [
            ("current", json!({}), json!(7.5)),
            ("shown", json!({}), json!(9.0)),
            ("can_step", json!({"delta":2}), json!(false)),
            ("can_step", json!({"delta":-2}), json!(true)),
            ("is_invalid", json!({}), json!(false)),
            ("invalid_reason", json!({}), Value::Null),
            ("is_disabled", json!({}), json!(false)),
        ] {
            assert_eq!(
                state
                    .invoke(&descriptor.borrow(), method, &args, true, window, cx)
                    .expect(method),
                expected
            );
        }
    });
    let entity = state.controls_extra.numbers.borrow()[&(0, "number".into())]
        .entity
        .clone();
    let field = harness.update(|_, cx| entity.read(cx).field().clone());
    harness.click("number.field");
    harness.keystrokes("home shift-right");
    let selection = harness.update(|_, cx| field.read(cx).selected_range());
    descriptor
        .borrow_mut()
        .props
        .insert("unit".into(), json!("ms"));
    harness.update(|_, cx| cx.refresh_windows());
    harness.update(|window, cx| {
        assert_eq!(field.read(cx).selected_range(), selection);
        assert_eq!(entity.read(cx).shown(cx), Some(9.0));
        assert_eq!(entity.read(cx).field().entity_id(), field.entity_id());
        for (method, args) in [
            ("set_range", json!({"min":-2,"max":8})),
            ("set_steps", json!({"step":0.25,"page_step":3})),
            ("set_precision", json!({"precision":2})),
            (
                "set_presentation",
                json!({"name":null,"unit":null,"prefix":"$","size":"sm"}),
            ),
            ("set_required", json!({"required":true})),
            ("set_invalid", json!({"invalid":false})),
            ("set_value", json!({"value":-4.25})),
        ] {
            assert_eq!(
                state
                    .invoke(&descriptor.borrow(), method, &args, false, window, cx)
                    .expect(method),
                Value::Null
            );
        }
        assert_eq!(entity.read(cx).shown(cx), Some(-4.25));
        assert!(entity.read(cx).is_invalid(cx));
        assert!(
            state
                .invoke(
                    &descriptor.borrow(),
                    "invalid_reason",
                    &json!({}),
                    true,
                    window,
                    cx
                )
                .expect("reason")
                .is_string()
        );
        state
            .invoke(
                &descriptor.borrow(),
                "set_range",
                &json!({"min":null,"max":null}),
                false,
                window,
                cx,
            )
            .expect("remove range");
        assert!(!entity.read(cx).is_invalid(cx));
        state
            .invoke(
                &descriptor.borrow(),
                "set_disabled",
                &json!({"disabled":true}),
                false,
                window,
                cx,
            )
            .expect("disable");
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
                .expect("disabled query"),
            json!(true)
        );
        assert!(
            state
                .invoke(
                    &descriptor.borrow(),
                    "set_disabled",
                    &json!({"disabled":false}),
                    false,
                    window,
                    cx
                )
                .is_err()
        );
        assert!(
            state
                .invoke(
                    &descriptor.borrow(),
                    "set_value",
                    &json!({"value":3}),
                    false,
                    window,
                    cx
                )
                .is_err()
        );
    });
    harness.click("number.increment");
    assert_eq!(*events.borrow(), vec![json!(9.0)]);
    let removed = node("Button", "replacement", json!({}), json!({}));
    harness.update(|_, cx| state.reconcile(&removed, cx));
    assert!(state.controls_extra.numbers.borrow().is_empty());
    harness.update(|window, cx| {
        assert!(
            state
                .invoke(&descriptor.borrow(), "shown", &json!({}), true, window, cx)
                .is_err()
        );
        entity.update(cx, |_, cx| cx.emit(NumberInputEvent::Changed(27.)));
    });
    assert_eq!(*events.borrow(), vec![json!(9.0)]);
}

#[test]
fn native_descriptors_and_methods_reject_closed_shape_and_relational_errors() {
    for (component, props) in [
        (
            "ColorPicker",
            json!({"value":{"h":0,"s":1,"l":0.5,"a":1.01}}),
        ),
        ("Button", json!({"iconOnly":true})),
        (
            "Button",
            json!({"color":{"palette":"blue","semantic":"info"}}),
        ),
        (
            "IconButton",
            json!({"icon":{"key":"plus-circle","path":"/tmp/secret"},"accessibleName":"Add"}),
        ),
        ("FormField", json!({"label":"Field","validation":"invalid"})),
        ("FilterBar", json!({"countState":"unavailable"})),
        ("TransferList", json!({"sourceSelected":[1]})),
        (
            "SettingsRow",
            json!({"label":"Setting","bind":{"signal":1}}),
        ),
    ] {
        assert!(
            validate_descriptor(&node(component, "invalid", props, json!({}))).is_err(),
            "{component}"
        );
    }
    assert!(
        validate_descriptor(&node(
            "Button",
            "disabled",
            json!({"disabled":true}),
            json!({"click":"action"})
        ))
        .is_err()
    );
    assert!(
        super::super::validation::invocation(
            "TransferList",
            "set_items",
            &json!({"source":[]}),
            false
        )
        .is_err()
    );
    assert!(
        super::super::validation::invocation("SearchInput", "value", &json!({"extra":1}), true)
            .is_err()
    );
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

#[gpui::test]
fn transfer_panes_remain_controlled_and_keep_query_across_options(cx: &mut TestAppContext) {
    let descriptor = Rc::new(RefCell::new(node(
        "TransferList",
        "transfer",
        json!({
            "source":[{"id":"a","label":"Alpha"},{"id":"b","label":"Beta"},{"id":"locked","label":"Locked","disabled":true}],
            "target":[{"id":"z","label":"Zeta"}],"sourceSelected":["b"],"targetSelected":["z"]
        }),
        json!({"toggleSource":"source","toggleTarget":"target","moveToTarget":"right","moveToSource":"left","queryChange":"query"}),
    )));
    let state = Rc::new(KitState::default());
    let events = Rc::new(RefCell::new(Vec::new()));
    let (build_state, build_node, output) = (state.clone(), descriptor.clone(), events.clone());
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let node = build_node.borrow();
        build_state.reconcile(&node, cx);
        let output = output.clone();
        build_state.render(
            &node,
            BTreeMap::new(),
            window,
            cx,
            Rc::new(move |action, value| output.borrow_mut().push((action.to_owned(), value))),
        )
    });
    for id in [
        "transfer.source.list.a",
        "transfer.target.list.z",
        "transfer.source.list.locked",
        "transfer.move-to-target",
        "transfer.move-to-source",
    ] {
        harness.click(id);
    }
    assert_eq!(
        *events.borrow(),
        vec![
            ("source".into(), json!("a")),
            ("target".into(), json!("z")),
            ("right".into(), Value::Null),
            ("left".into(), Value::Null)
        ]
    );
    assert!(
        !harness
            .node("transfer.source.list.a")
            .expect("source row")
            .selected
    );
    assert!(
        harness
            .node("transfer.source.list.b")
            .expect("selected row")
            .selected
    );
    let entity_id = state.controls_extra.transfers.borrow()[&(0, "transfer".into())]
        .entity
        .entity_id();
    harness.update(|window, cx| {
        state
            .invoke(
                &descriptor.borrow(),
                "set_query",
                &json!({"query":"Beta"}),
                false,
                window,
                cx,
            )
            .expect("filter");
    });
    descriptor
        .borrow_mut()
        .props
        .insert("sourceLabel".into(), json!("Unassigned"));
    harness.frame();
    assert!(harness.node("transfer.source.list.a").is_none());
    assert!(harness.node("transfer.source.list.b").is_some());
    assert_eq!(
        entity_id,
        state.controls_extra.transfers.borrow()[&(0, "transfer".into())]
            .entity
            .entity_id()
    );
    harness.update(|window, cx| {
        for (method, args) in [
            ("set_query", json!({"query":""})),
            (
                "set_items",
                json!({"source":[{"id":"new","label":"New item"}],"target":[]}),
            ),
            ("set_selection", json!({"source":["new"],"target":[]})),
            ("set_labels", json!({"source":"Pool","target":"Assigned"})),
            ("set_control_size", json!({"size":"sm"})),
        ] {
            assert_eq!(
                state
                    .invoke(&descriptor.borrow(), method, &args, false, window, cx)
                    .expect("native setter"),
                Value::Null
            );
        }
    });
    harness.frame();
    assert!(
        harness
            .node("transfer.source.list.new")
            .expect("new selected item")
            .selected
    );
    assert!(harness.node("transfer.target.list.z").is_none());
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
            .expect("disable");
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
                .expect("disabled query"),
            json!(true)
        );
        assert!(
            state
                .invoke(
                    &descriptor.borrow(),
                    "set_query",
                    &json!({"query":"forbidden"}),
                    false,
                    window,
                    cx
                )
                .is_err()
        );
        let replacement = node("Button", "replacement", json!({}), json!({}));
        state.reconcile(&replacement, cx);
        assert!(
            state
                .invoke(
                    &descriptor.borrow(),
                    "is_disabled",
                    &json!({}),
                    true,
                    window,
                    cx
                )
                .is_err()
        );
        *descriptor.borrow_mut() = replacement;
    });
    assert!(state.controls_extra.transfers.borrow().is_empty());
}
