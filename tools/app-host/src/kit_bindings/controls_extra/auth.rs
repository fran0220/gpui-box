use super::*;
use gpui_kit::controls::auth::{
    OneTimeCodeInput, OneTimeCodeInputEvent, PasswordInput, PasswordInputEvent,
};

// Both sensitive controls retain one native editor. Their event grammars and
// extra methods remain explicit; this macro shares only retention/routing.
macro_rules! auth_adapter {
    ($render:ident, $invoke:ident, $map:ident, $control:ty, $event:ty, $event_value:expr, $configure:expr, $extra:expr, $query:expr) => {
        impl State {
            pub(super) fn $render(
                &self,
                node: &Node,
                window: &mut Window,
                cx: &mut App,
                emit: Emit,
            ) -> AnyElement {
                let key = (node.instance, node.id.clone());
                let existing = self.$map.borrow().get(&key).cloned();
                let entry = existing.unwrap_or_else(|| {
                    let entity = cx.new(|cx| <$control>::new(node.id.clone(), window, cx));
                    let route = Rc::new(RefCell::new(Route {
                        events: node.events.clone(),
                        emit: emit.clone(),
                        disabled: flag(node, "disabled"),
                    }));
                    let callback = Rc::downgrade(&route);
                    let subscription = cx.subscribe(&entity, move |entity, event: &$event, cx| {
                        if entity.read(cx).is_disabled() {
                            return;
                        }
                        let (name, value): (&str, Option<&gpui::SharedString>) =
                            ($event_value)(event);
                        let target = callback.upgrade().and_then(|route| {
                            let route = route.borrow();
                            (!route.disabled)
                                .then(|| {
                                    route
                                        .events
                                        .get(name)
                                        .map(|action| (action.clone(), route.emit.clone()))
                                })
                                .flatten()
                        });
                        if let Some((action, emit)) = target {
                            emit(&action, value.map_or(Value::Null, |s| json!(s.as_ref())));
                        }
                    });
                    let entry = Rc::new(Entry {
                        entity,
                        route,
                        props: Default::default(),
                        _subscription: subscription,
                    });
                    self.$map.borrow_mut().insert(key, entry.clone());
                    entry
                });
                *entry.route.borrow_mut() = Route {
                    events: node.events.clone(),
                    emit,
                    disabled: flag(node, "disabled"),
                };
                if *entry.props.borrow() != node.props {
                    let text_changed = entry.props.borrow().get("value") != node.props.get("value");
                    entry.entity.update(cx, |control, cx| {
                        control.set_name(
                            node.props
                                .get("name")
                                .and_then(Value::as_str)
                                .map(|s| s.to_owned().into()),
                            cx,
                        );
                        control.set_required(flag(node, "required"), cx);
                        control.set_read_only(flag(node, "readOnly"), cx);
                        control.set_invalid(flag(node, "invalid"), cx);
                        control.set_control_size(size(node), cx);
                        ($configure)(control, node, cx);
                        control.set_disabled(flag(node, "disabled"), cx);
                        if text_changed && node.props.contains_key("value") {
                            control.set_value(text(node, "value"), cx);
                        }
                    });
                    *entry.props.borrow_mut() = node.props.clone();
                }
                entry.entity.clone().into_any_element()
            }

            pub(super) fn $invoke(
                &self,
                node: &Node,
                method: &str,
                args: &Value,
                query: bool,
                cx: &mut App,
            ) -> anyhow::Result<Value> {
                let entity = self
                    .$map
                    .borrow()
                    .get(&(node.instance, node.id.clone()))
                    .map(|entry| entry.entity.clone())
                    .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
                anyhow::ensure!(
                    query || (!flag(node, "disabled") && !entity.read(cx).is_disabled()),
                    "disabled target refuses invocation"
                );
                if query {
                    let control = entity.read(cx);
                    return match method {
                        "value" => Ok(json!(control.value(cx).as_ref())),
                        "is_disabled" => Ok(json!(control.is_disabled())),
                        _ => ($query)(control, method, cx),
                    };
                }
                entity.update(cx, |control, cx| {
                    match method {
                        "set_value" => control
                            .set_value(args["value"].as_str().unwrap_or_default().to_owned(), cx),
                        "set_name" => {
                            control.set_name(args["name"].as_str().map(|s| s.to_owned().into()), cx)
                        }
                        "set_required" => {
                            control.set_required(args["required"].as_bool().unwrap_or(false), cx)
                        }
                        "set_read_only" => {
                            control.set_read_only(args["read_only"].as_bool().unwrap_or(false), cx)
                        }
                        "set_invalid" => {
                            control.set_invalid(args["invalid"].as_bool().unwrap_or(false), cx)
                        }
                        "set_disabled" => {
                            control.set_disabled(args["disabled"].as_bool().unwrap_or(false), cx)
                        }
                        "set_control_size" => {
                            let mut node = node.clone();
                            node.props.insert("size".into(), args["size"].clone());
                            control.set_control_size(size(&node), cx);
                        }
                        _ => return ($extra)(control, method, args, cx),
                    }
                    Ok(Value::Null)
                })
            }
        }
    };
}

fn password_event(event: &PasswordInputEvent) -> (&str, Option<&gpui::SharedString>) {
    match event {
        PasswordInputEvent::Change(value) => ("change", Some(value)),
        PasswordInputEvent::Submit => ("submit", None),
        PasswordInputEvent::Cancel => ("cancel", None),
        PasswordInputEvent::BackspaceAtStart => ("backspaceAtStart", None),
        PasswordInputEvent::Focus => ("focus", None),
        PasswordInputEvent::Blur => ("blur", None),
    }
}
fn code_event(event: &OneTimeCodeInputEvent) -> (&str, Option<&gpui::SharedString>) {
    match event {
        OneTimeCodeInputEvent::Change(value) => ("change", Some(value)),
        OneTimeCodeInputEvent::Submit => ("submit", None),
    }
}

auth_adapter!(
    render_password,
    invoke_password,
    passwords,
    PasswordInput,
    PasswordInputEvent,
    password_event,
    |control: &mut PasswordInput, node: &Node, cx: &mut gpui::Context<PasswordInput>| control
        .set_placeholder(
            node.props
                .get("placeholder")
                .and_then(Value::as_str)
                .map(|s| s.to_owned().into()),
            cx
        ),
    |control: &mut PasswordInput,
     method: &str,
     args: &Value,
     cx: &mut gpui::Context<PasswordInput>|
     -> anyhow::Result<Value> {
        anyhow::ensure!(
            method == "set_placeholder",
            "unsupported PasswordInput command"
        );
        control.set_placeholder(
            args["placeholder"].as_str().map(|s| s.to_owned().into()),
            cx,
        );
        Ok(Value::Null)
    },
    |control: &PasswordInput, method: &str, cx: &App| -> anyhow::Result<Value> {
        match method {
            "is_revealed" => Ok(json!(control.is_revealed())),
            "selected_range" => {
                let range = control.selected_range(cx);
                Ok(json!({"start":range.start,"end":range.end}))
            }
            _ => anyhow::bail!("unsupported PasswordInput query"),
        }
    }
);
auth_adapter!(
    render_code,
    invoke_code,
    codes,
    OneTimeCodeInput,
    OneTimeCodeInputEvent,
    code_event,
    |control: &mut OneTimeCodeInput, node: &Node, cx: &mut gpui::Context<OneTimeCodeInput>| control
        .set_slots(number(node, "slots", 6.) as usize, cx),
    |control: &mut OneTimeCodeInput,
     method: &str,
     args: &Value,
     cx: &mut gpui::Context<OneTimeCodeInput>|
     -> anyhow::Result<Value> {
        anyhow::ensure!(
            method == "set_slots",
            "unsupported OneTimeCodeInput command"
        );
        control.set_slots(args["slots"].as_u64().unwrap_or(6) as usize, cx);
        Ok(Value::Null)
    },
    |control: &OneTimeCodeInput, method: &str, cx: &App| -> anyhow::Result<Value> {
        match method {
            "len" => Ok(json!(control.len(cx))),
            "is_empty" => Ok(json!(control.is_empty(cx))),
            "is_complete" => Ok(json!(control.is_complete(cx))),
            "slot_count" => Ok(json!(control.slot_count())),
            _ => anyhow::bail!("unsupported OneTimeCodeInput query"),
        }
    }
);

#[cfg(all(test, feature = "capture"))]
mod tests {
    use super::*;
    use gpui::{Focusable, TestAppContext};
    use gpui_kit_testkit::harness::Harness;

    #[gpui::test]
    fn sensitive_options_retain_typed_text_focus_and_redaction(cx: &mut TestAppContext) {
        for kind in ["PasswordInput", "OneTimeCodeInput"] {
            let state = Rc::new(State::default());
            let props = if kind == "PasswordInput" {
                json!({"value":"seed","name":"Fixture","placeholder":"Old hint"})
            } else {
                json!({"value":"seed","name":"Fixture","slots":8})
            };
            let node: Node = serde_json::from_value(json!({"kind":"kit","component":kind,"id":"sensitive","props":props,"events":{"change":"changed"}})).expect("fixture descriptor");
            let descriptor = Rc::new(RefCell::new(node));
            let events = Rc::new(RefCell::new(Vec::new()));
            let (build_state, build_node, output) =
                (state.clone(), descriptor.clone(), events.clone());
            let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
                let output = output.clone();
                build_state.render(
                    &build_node.borrow(),
                    BTreeMap::new(),
                    window,
                    cx,
                    Rc::new(move |_, payload| output.borrow_mut().push(payload)),
                )
            });
            let (identity, focus) = harness.update(|_, cx| {
                let focus = if kind == "PasswordInput" {
                    state.passwords.borrow()[&(0, "sensitive".into())]
                        .entity
                        .read(cx)
                        .focus_handle(cx)
                } else {
                    state.codes.borrow()[&(0, "sensitive".into())]
                        .entity
                        .read(cx)
                        .focus_handle(cx)
                };
                (state.native_entity_id(&descriptor.borrow()), focus)
            });
            harness.click("sensitive");
            harness.keystrokes("end x shift-left");
            let selected = harness.update(|_, cx| {
                (kind == "PasswordInput").then(|| {
                    state.passwords.borrow()[&(0, "sensitive".into())]
                        .entity
                        .read(cx)
                        .selected_range(cx)
                })
            });
            {
                let mut node = descriptor.borrow_mut();
                node.props.remove("name");
                node.props.remove("placeholder");
                node.props.insert("size".into(), json!("lg"));
                node.props.insert("required".into(), json!(true));
            }
            harness.update(|_, cx| cx.refresh_windows());
            harness.update(|window, cx| {
                assert_eq!(state.native_entity_id(&descriptor.borrow()), identity);
                assert!(focus.is_focused(window));
                assert_eq!(
                    state
                        .invoke(&descriptor.borrow(), "value", &json!({}), true, window, cx)
                        .expect("current text"),
                    json!("seedx")
                );
                if let Some(selected) = selected {
                    assert_eq!(
                        state.passwords.borrow()[&(0, "sensitive".into())]
                            .entity
                            .read(cx)
                            .selected_range(cx),
                        selected
                    );
                }
                if kind == "OneTimeCodeInput" {
                    state
                        .invoke(
                            &descriptor.borrow(),
                            "set_slots",
                            &json!({"slots":2}),
                            false,
                            window,
                            cx,
                        )
                        .expect("lower future limit");
                    assert_eq!(
                        state
                            .invoke(&descriptor.borrow(), "value", &json!({}), true, window, cx)
                            .expect("retained text"),
                        json!("seedx")
                    );
                }
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
                assert!(
                    state
                        .invoke(
                            &descriptor.borrow(),
                            "set_value",
                            &json!({"value":"blocked"}),
                            false,
                            window,
                            cx
                        )
                        .is_err()
                );
            });
            let snapshot = serde_json::to_string(&harness.snapshot()).expect("semantic snapshot");
            assert!(
                !snapshot.contains("seed"),
                "sensitive text must not enter semantic snapshots"
            );
            descriptor
                .borrow_mut()
                .props
                .insert("disabled".into(), json!(false));
            harness.update(|_, cx| cx.refresh_windows());
            harness.click("sensitive");
            harness.keystrokes("end y");
            assert_eq!(events.borrow().last(), Some(&json!("seedxy")));
            if kind == "PasswordInput" {
                harness.click("sensitive.reveal");
                descriptor
                    .borrow_mut()
                    .props
                    .insert("placeholder".into(), json!("New hint"));
                harness.update(|_, cx| cx.refresh_windows());
                harness.update(|window, cx| {
                    assert_eq!(
                        state
                            .invoke(
                                &descriptor.borrow(),
                                "is_revealed",
                                &json!({}),
                                true,
                                window,
                                cx
                            )
                            .expect("reveal query"),
                        json!(true)
                    );
                    state
                        .invoke(
                            &descriptor.borrow(),
                            "set_read_only",
                            &json!({"read_only":true}),
                            false,
                            window,
                            cx,
                        )
                        .expect("read only");
                });
                harness.click("sensitive");
                harness.keystrokes("end z");
                assert!(
                    !serde_json::to_string(&harness.snapshot())
                        .expect("revealed semantics")
                        .contains("seed")
                );
                harness.update(|window, cx| {
                    assert_eq!(
                        state
                            .invoke(&descriptor.borrow(), "value", &json!({}), true, window, cx)
                            .expect("read-only value"),
                        json!("seedxy")
                    )
                });
            }
        }
    }
}
