use super::*;
use gpui_kit::controls::keybinding_recorder::{KeybindingRecorder, KeybindingRecorderEvent};

impl State {
    pub(super) fn render_recorder(
        &self,
        node: &Node,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.recorders.borrow().get(&key).cloned();
        let entry = existing.unwrap_or_else(|| {
            let entity = cx.new(|cx| KeybindingRecorder::new(node.id.clone(), window, cx));
            let route = Rc::new(RefCell::new(Route {
                events: node.events.clone(),
                emit: emit.clone(),
                disabled: flag(node, "disabled"),
            }));
            let callback = Rc::downgrade(&route);
            let subscription = cx.subscribe(
                &entity,
                move |entity, event: &KeybindingRecorderEvent, cx| {
                    if entity.read(cx).is_disabled() {
                        return;
                    }
                    let name = match event {
                        KeybindingRecorderEvent::Started => "started",
                        KeybindingRecorderEvent::Captured(_) => "captured",
                        KeybindingRecorderEvent::Cancelled => "cancelled",
                    };
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
                        let payload = match event {
                            KeybindingRecorderEvent::Captured(value) => json!(value.as_ref()),
                            _ => Value::Null,
                        };
                        emit(&action, payload);
                    }
                },
            );
            let entry = Rc::new(Entry {
                entity,
                route,
                props: Default::default(),
                _subscription: subscription,
            });
            self.recorders.borrow_mut().insert(key, entry.clone());
            entry
        });
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if *entry.props.borrow() != node.props {
            entry.entity.update(cx, |control, cx| {
                control.set_label(
                    node.props
                        .get("label")
                        .and_then(Value::as_str)
                        .map(|s| s.to_owned().into()),
                    cx,
                );
                control.set_placeholder(
                    node.props
                        .get("placeholder")
                        .and_then(Value::as_str)
                        .map(|s| s.to_owned().into()),
                    cx,
                );
                control.set_binding(
                    node.props
                        .get("binding")
                        .and_then(Value::as_str)
                        .map(|s| s.to_owned().into()),
                    cx,
                );
                control.set_conflict(
                    node.props
                        .get("conflict")
                        .and_then(Value::as_str)
                        .map(|s| s.to_owned().into()),
                    cx,
                );
                control.set_allow_escape(flag(node, "allowEscape"), cx);
                control.set_control_size(size(node), cx);
                control.set_disabled(flag(node, "disabled"), cx);
            });
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }

    pub(super) fn invoke_recorder(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        let entity = self
            .recorders
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
                "is_recording" => Ok(json!(control.is_recording())),
                "current_binding" => Ok(json!(control.current_binding().map(|s| s.as_ref()))),
                "is_disabled" => Ok(json!(control.is_disabled())),
                _ => anyhow::bail!("unsupported KeybindingRecorder query"),
            };
        }
        entity.update(cx, |control, cx| {
            let optional = |key: &str| {
                args[key]
                    .as_str()
                    .map(|s| gpui::SharedString::from(s.to_owned()))
            };
            match method {
                "start" => control.start(window, cx),
                "cancel" => control.cancel(cx),
                "set_binding" => control.set_binding(optional("binding"), cx),
                "set_conflict" => control.set_conflict(optional("reason"), cx),
                "set_label" => control.set_label(optional("label"), cx),
                "set_placeholder" => control.set_placeholder(optional("placeholder"), cx),
                "set_allow_escape" => {
                    control.set_allow_escape(args["allow"].as_bool().unwrap_or(false), cx)
                }
                "set_disabled" => {
                    control.set_disabled(args["disabled"].as_bool().unwrap_or(false), cx)
                }
                "set_control_size" => {
                    let mut descriptor = node.clone();
                    descriptor.props.insert("size".into(), args["size"].clone());
                    control.set_control_size(size(&descriptor), cx);
                }
                _ => anyhow::bail!("unsupported KeybindingRecorder command"),
            }
            Ok(Value::Null)
        })
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests {
    use super::*;
    use gpui::{Focusable, TestAppContext};
    use gpui_kit_testkit::harness::Harness;

    #[gpui::test]
    fn recorder_preserves_session_on_options_and_cancels_when_disabled(cx: &mut TestAppContext) {
        let state = Rc::new(State::default());
        let node: Node = serde_json::from_value(json!({"kind":"kit","component":"KeybindingRecorder","id":"recorder","props":{"label":"Record save","placeholder":"Unassigned","binding":"ctrl-s","conflict":"Fixture conflict"},"events":{"captured":"capture","cancelled":"cancel"}})).expect("descriptor");
        let descriptor = Rc::new(RefCell::new(node));
        let events = Rc::new(RefCell::new(Vec::new()));
        let (build_state, build_node, output) = (state.clone(), descriptor.clone(), events.clone());
        let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
            let output = output.clone();
            build_state.render_recorder(
                &build_node.borrow(),
                window,
                cx,
                Rc::new(move |action, payload| {
                    output.borrow_mut().push((action.to_owned(), payload))
                }),
            )
        });
        let entity = state.recorders.borrow()[&(0, "recorder".into())]
            .entity
            .clone();
        let focus = harness.update(|window, cx| {
            state
                .invoke_recorder(&descriptor.borrow(), "start", &json!({}), false, window, cx)
                .expect("start");
            entity.read(cx).focus_handle(cx)
        });
        descriptor.borrow_mut().props =
            serde_json::from_value(json!({"size":"lg","allowEscape":true})).expect("props");
        harness.update(|_, cx| cx.refresh_windows());
        harness.update(|window, cx| {
            assert_eq!(
                state.recorders.borrow()[&(0, "recorder".into())].entity,
                entity
            );
            assert_eq!(entity.read(cx).focus_handle(cx), focus);
            assert!(entity.read(cx).is_recording());
            assert!(entity.read(cx).current_binding().is_none());
            for (method, args) in [
                ("set_label", json!({"label":null})),
                ("set_placeholder", json!({"placeholder":null})),
                ("set_conflict", json!({"reason":null})),
                ("set_binding", json!({"binding":"ctrl-b"})),
            ] {
                state
                    .invoke_recorder(&descriptor.borrow(), method, &args, false, window, cx)
                    .expect(method);
            }
        });
        harness.keystrokes("escape");
        assert_eq!(
            events.borrow().last(),
            Some(&("capture".into(), json!("escape")))
        );
        harness.update(|window, cx| {
            assert_eq!(
                entity.read(cx).current_binding().map(|s| s.as_ref()),
                Some("ctrl-b")
            );
            state
                .invoke_recorder(&descriptor.borrow(), "start", &json!({}), false, window, cx)
                .expect("restart");
            state
                .invoke_recorder(
                    &descriptor.borrow(),
                    "set_disabled",
                    &json!({"disabled":true}),
                    false,
                    window,
                    cx,
                )
                .expect("disable");
            assert!(!entity.read(cx).is_recording());
            assert!(
                state
                    .invoke_recorder(&descriptor.borrow(), "start", &json!({}), false, window, cx)
                    .is_err()
            );
            assert_eq!(
                state
                    .invoke_recorder(
                        &descriptor.borrow(),
                        "is_disabled",
                        &json!({}),
                        true,
                        window,
                        cx
                    )
                    .expect("read"),
                json!(true)
            );
        });
        descriptor
            .borrow_mut()
            .props
            .insert("disabled".into(), json!(false));
        harness.update(|_, cx| cx.refresh_windows());
        harness.update(|window, cx| {
            assert!(!entity.read(cx).is_recording());
            state
                .invoke_recorder(&descriptor.borrow(), "start", &json!({}), false, window, cx)
                .expect("reenabled start");
            state
                .invoke_recorder(
                    &descriptor.borrow(),
                    "set_allow_escape",
                    &json!({"allow":false}),
                    false,
                    window,
                    cx,
                )
                .expect("default escape");
        });
        harness.keystrokes("escape");
        assert_eq!(
            events.borrow().last(),
            Some(&("cancel".into(), Value::Null))
        );
    }
}
