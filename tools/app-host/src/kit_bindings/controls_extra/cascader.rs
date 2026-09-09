use super::*;
use gpui_kit::controls::cascader::{Cascader, CascaderEvent, CascaderOption};

pub(super) fn validate_options(value: Option<&Value>) -> anyhow::Result<()> {
    fn visit(
        value: Option<&Value>,
        ids: &mut std::collections::HashSet<String>,
    ) -> anyhow::Result<()> {
        for option in value.and_then(Value::as_array).into_iter().flatten() {
            let id = option["id"].as_str().unwrap_or_default();
            anyhow::ensure!(ids.insert(id.to_owned()), "duplicate Cascader identity");
            visit(
                option
                    .get("children")
                    .and_then(|children| children.get("value")),
                ids,
            )?;
        }
        Ok(())
    }
    visit(value, &mut Default::default())
}

fn options(value: Option<&Value>) -> Vec<CascaderOption> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|value| {
            let mut option = CascaderOption::new(
                value["id"].as_str().unwrap_or_default().to_owned(),
                value["label"].as_str().unwrap_or_default().to_owned(),
            )
            .disabled(value["disabled"].as_bool().unwrap_or(false));
            if let Some(children) = value.get("children") {
                option = match children["state"].as_str().unwrap_or_default() {
                    "idle" => option.idle_children(),
                    "loading" => option.loading_children(),
                    "empty" => option.empty_children(),
                    "unavailable" => option.unavailable_children(
                        children["reason"].as_str().unwrap_or_default().to_owned(),
                    ),
                    "error" => option
                        .error_children(children["reason"].as_str().unwrap_or_default().to_owned()),
                    "ready" => option.children(options(children.get("value"))),
                    _ => unreachable!("validated cascader state"),
                };
            }
            option
        })
        .collect()
}

impl State {
    pub(super) fn render_cascader(
        &self,
        node: &Node,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.cascaders.borrow().get(&key).cloned();
        let entry = existing.unwrap_or_else(|| {
            let entity = cx.new(|cx| Cascader::new(node.id.clone(), window, cx));
            let route = Rc::new(RefCell::new(Route {
                events: node.events.clone(),
                emit: emit.clone(),
                disabled: flag(node, "disabled"),
            }));
            let callback = Rc::downgrade(&route);
            let subscription = cx.subscribe(&entity, move |entity, event: &CascaderEvent, cx| {
                if entity.read(cx).is_disabled() {
                    return;
                }
                let (name, payload) = match event {
                    CascaderEvent::Selected(id) => ("selected", json!(id.as_ref())),
                    CascaderEvent::Expanded(id) => ("expanded", json!(id.as_ref())),
                    CascaderEvent::Retry(id) => ("retry", json!(id.as_ref())),
                    CascaderEvent::Opened => ("opened", Value::Null),
                    CascaderEvent::Closed => ("closed", Value::Null),
                };
                let target = callback.upgrade().and_then(|route| {
                    let route = route.borrow();
                    if route.disabled {
                        None
                    } else {
                        route
                            .events
                            .get(name)
                            .map(|action| (action.clone(), route.emit.clone()))
                    }
                });
                if let Some((action, emit)) = target {
                    emit(&action, payload);
                }
            });
            let entry = Rc::new(Entry {
                entity,
                route,
                props: Default::default(),
                _subscription: subscription,
            });
            self.cascaders.borrow_mut().insert(key, entry.clone());
            entry
        });
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if *entry.props.borrow() != node.props {
            entry.entity.update(cx, |control, cx| {
                if entry.props.borrow().get("options") != node.props.get("options") {
                    control.set_options(options(node.props.get("options")), cx);
                }
                if entry.props.borrow().get("selected") != node.props.get("selected") {
                    control.set_selected(
                        node.props
                            .get("selected")
                            .and_then(Value::as_str)
                            .map(|s| s.to_owned().into()),
                        cx,
                    );
                }
                control.set_name(text(node, "name"), cx);
                control.set_placeholder(
                    node.props
                        .get("placeholder")
                        .and_then(Value::as_str)
                        .map(|s| s.to_owned().into()),
                    cx,
                );
                control.set_control_size(size(node), cx);
                control.set_disabled(flag(node, "disabled"), cx);
            });
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }
    pub(super) fn invoke_cascader(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        let entity = self
            .cascaders
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .map(|entry| entry.entity.clone())
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        anyhow::ensure!(
            query || (!flag(node, "disabled") && !entity.read(cx).is_disabled()),
            "disabled target refuses invocation"
        );
        entity.update(cx, |control, cx| {
            if query {
                return match method {
                    "selected_id" => Ok(json!(control.selected_id().map(|s| s.as_ref()))),
                    "is_open" => Ok(json!(control.is_open())),
                    "is_disabled" => Ok(json!(control.is_disabled())),
                    "open_path" => Ok(json!(
                        control
                            .open_path()
                            .iter()
                            .map(|s| s.as_ref())
                            .collect::<Vec<_>>()
                    )),
                    _ => anyhow::bail!("unsupported Cascader query"),
                };
            }
            match method {
                "set_options" => {
                    validate_options(Some(&args["options"]))?;
                    control.set_options(options(Some(&args["options"])), cx);
                }
                "set_selected" => {
                    control.set_selected(args["selected"].as_str().map(|s| s.to_owned().into()), cx)
                }
                "set_name" => {
                    control.set_name(args["name"].as_str().unwrap_or_default().to_owned(), cx)
                }
                "set_placeholder" => control.set_placeholder(
                    args["placeholder"].as_str().map(|s| s.to_owned().into()),
                    cx,
                ),
                "set_disabled" => {
                    control.set_disabled(args["disabled"].as_bool().unwrap_or(false), cx)
                }
                "set_control_size" => {
                    let mut node = node.clone();
                    node.props.insert("size".into(), args["size"].clone());
                    control.set_control_size(size(&node), cx);
                }
                "open" => control.open(window, cx),
                "close" => control.close(cx),
                _ => anyhow::bail!("unsupported Cascader command"),
            }
            Ok(Value::Null)
        })
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use gpui_kit_testkit::harness::Harness;

    #[gpui::test]
    fn branches_retain_paths_and_never_turn_refusals_into_leaves(cx: &mut TestAppContext) {
        let owner = gpui::EffectOwner::new();
        let state = Rc::new(State::default());
        let descriptor=Rc::new(RefCell::new(serde_json::from_value::<Node>(json!({"kind":"kit","component":"Cascader","id":"cascade","props":{"name":"Fixture","placeholder":"Temporary","options":[{"id":"root","label":"Root","children":{"state":"loading"}}]},"events":{"expanded":"expand","selected":"select","retry":"retry"}})).expect("fixture")));
        let events = Rc::new(RefCell::new(Vec::new()));
        let (build_state, build_node, output) = (state.clone(), descriptor.clone(), events.clone());
        let mut harness = Harness::new(
            cx,
            move |cx| {
                gpui_kit::install(cx);
                gpui_kit::foundation::register_owner_state(owner, cx);
            },
            move |window, cx| {
                let output = output.clone();
                let element = cx.with_effect_owner(Some(owner), |cx| {
                    build_state.render_cascader(
                        &build_node.borrow(),
                        window,
                        cx,
                        Rc::new(move |name, value| {
                            output.borrow_mut().push((name.to_owned(), value))
                        }),
                    )
                });
                gpui::effect_owner(owner, element).into_any_element()
            },
        );
        harness.click("cascade");
        harness.click("cascade.root");
        assert_eq!(
            events.borrow().last(),
            Some(&("expand".into(), json!("root")))
        );
        let identity = harness.update(|_, cx| {
            state.cascaders.borrow()[&(0, "cascade".into())]
                .entity
                .read(cx)
                .open_path()
                .to_vec()
        });
        assert_eq!(identity, vec![gpui::SharedString::from("root")]);
        for children in [
            json!({"state":"idle"}),
            json!({"state":"empty"}),
            json!({"state":"unavailable","reason":"Host refused fixture"}),
            json!({"state":"error","reason":"Fixture failed"}),
            json!({"state":"ready","value":[{"id":"blocked","label":"Blocked","disabled":true},{"id":"leaf","label":"Leaf"}]}),
        ] {
            descriptor.borrow_mut().props.insert(
                "options".into(),
                json!([{"id":"root","label":"Root","children":children}]),
            );
            descriptor.borrow_mut().props.remove("placeholder");
            descriptor
                .borrow_mut()
                .props
                .insert("size".into(), json!("lg"));
            harness.update(|_, cx| cx.refresh_windows());
            harness.update(|window, cx| {
                assert_eq!(
                    state
                        .invoke_cascader(
                            &descriptor.borrow(),
                            "open_path",
                            &json!({}),
                            true,
                            window,
                            cx
                        )
                        .expect("path"),
                    json!(["root"])
                );
                assert_eq!(
                    state
                        .invoke_cascader(
                            &descriptor.borrow(),
                            "selected_id",
                            &json!({}),
                            true,
                            window,
                            cx
                        )
                        .expect("selection"),
                    Value::Null
                );
            });
            if children["state"] == "error" {
                harness.click("cascade.root.state.retry");
                assert_eq!(
                    events.borrow().last(),
                    Some(&("retry".into(), json!("root")))
                );
            }
        }
        events.borrow_mut().clear();
        harness.click("cascade.blocked");
        assert!(events.borrow().is_empty());
        harness.click("cascade.leaf");
        assert_eq!(
            events.borrow().last(),
            Some(&("select".into(), json!("leaf")))
        );
        harness.update(|window,cx|{
            assert_eq!(state.invoke_cascader(&descriptor.borrow(),"selected_id",&json!({}),true,window,cx).expect("still caller owned"),Value::Null);
            let bad=json!({"options":[{"id":"root","label":"Root","children":{"state":"ready","value":[{"id":"root","label":"Duplicate"}]}}]});
            assert!(state.invoke_cascader(&descriptor.borrow(),"set_options",&bad,false,window,cx).is_err());
            state.invoke_cascader(&descriptor.borrow(),"set_disabled",&json!({"disabled":true}),false,window,cx).expect("disabled");assert!(state.invoke_cascader(&descriptor.borrow(),"open",&json!({}),false,window,cx).is_err());
        });
    }
}
