use super::*;
use gpui_kit::controls::{
    combobox::{Combobox, ComboboxEvent},
    multi_select::{MultiSelect, MultiSelectEvent},
    tag_input::{TagInput, TagInputEvent},
};

fn optional(value: &Value) -> Option<gpui::SharedString> {
    value.as_str().map(|s| s.to_owned().into())
}
fn strings(value: &Value) -> Vec<gpui::SharedString> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(optional)
        .collect()
}

impl State {
    pub(super) fn selection_reference_query(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        cx: &App,
        refs: &crate::references::Registration<'_>,
    ) -> Option<anyhow::Result<Value>> {
        let component = node.component.as_deref()?;
        if !matches!(method, "focus_handle" | "query_input" | "field") {
            return None;
        }
        Some((|| {
            let schema = super::super::validation::invocation(component, method, args, true)?;
            let key = (node.instance, node.id.clone());
            macro_rules! reference {
                ($map:ident,$getter:ident) => {{
                    let entity = self
                        .$map
                        .borrow()
                        .get(&key)
                        .map(|entry| entry.entity.clone())
                        .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
                    if method == "focus_handle" {
                        focus_reference(&self.$map, &key, cx, refs, |control, _| {
                            !control.is_disabled()
                        })?
                    } else {
                        refs.entity(
                            "TextInput",
                            &entity,
                            entity.read(cx).$getter(),
                            |parent, _| Some(parent.$getter().clone()),
                            |parent, _| !parent.is_disabled(),
                            super::super::reference_dispatch::text_input,
                        )?
                    }
                }};
            }
            let value = match component {
                "Combobox" => reference!(comboboxes, query_input),
                "MultiSelect" => reference!(multi_selects, query_input),
                "TagInput" => reference!(tag_inputs, field),
                _ => anyhow::bail!("unsupported selection reference"),
            };
            super::super::validation::validate(&value, schema)?;
            Ok(value)
        })())
    }
}

// These controls share retention and authority, not selection policy. Every
// control keeps its native editor and emits its own caller-owned intents.
macro_rules! selection_adapter {
    ($render:ident,$invoke:ident,$map:ident,$control:ty,$event:ty,$event_value:expr,$configure:expr,$dispatch:expr) => {
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
                        let (name, value) = ($event_value)(event);
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
                            emit(&action, value);
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
                    entry.entity.update(cx, |control, cx| {
                        ($configure)(control, node, &entry.props.borrow(), cx);
                        control
                            .set_placeholder(node.props.get("placeholder").and_then(optional), cx);
                        control.set_invalid(flag(node, "invalid"), cx);
                        control.set_control_size(size(node), cx);
                        control.set_disabled(flag(node, "disabled"), cx);
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
                window: &mut Window,
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
                entity.update(cx, |control, cx| {
                    if query && method == "is_disabled" {
                        return Ok(json!(control.is_disabled()));
                    }
                    if !query {
                        match method {
                            "set_placeholder" => {
                                control.set_placeholder(optional(&args["placeholder"]), cx)
                            }
                            "set_invalid" => {
                                control.set_invalid(args["invalid"].as_bool().unwrap_or(false), cx)
                            }
                            "set_disabled" => control
                                .set_disabled(args["disabled"].as_bool().unwrap_or(false), cx),
                            "set_control_size" => {
                                let mut node = node.clone();
                                node.props.insert("size".into(), args["size"].clone());
                                control.set_control_size(size(&node), cx);
                            }
                            _ => return ($dispatch)(control, method, args, query, window, cx),
                        }
                        return Ok(Value::Null);
                    }
                    ($dispatch)(control, method, args, query, window, cx)
                })
            }
        }
    };
}

selection_adapter!(
    render_combobox,
    invoke_combobox,
    comboboxes,
    Combobox,
    ComboboxEvent,
    |event: &ComboboxEvent| match event {
        ComboboxEvent::QueryChanged(s) => ("queryChanged", json!(s.as_ref())),
        ComboboxEvent::Selected(s) => ("selected", json!(s.as_ref())),
        ComboboxEvent::Custom(s) => ("custom", json!(s.as_ref())),
        ComboboxEvent::Opened => ("opened", Value::Null),
        ComboboxEvent::Closed => ("closed", Value::Null),
    },
    |control: &mut Combobox,
     node: &Node,
     previous: &serde_json::Map<String, Value>,
     cx: &mut gpui::Context<Combobox>| {
        if previous.get("options") != node.props.get("options") {
            control.set_options(super::super::select_options(node.props.get("options")), cx);
        }
        if previous.get("selected") != node.props.get("selected") {
            control.set_selected(node.props.get("selected").and_then(optional), cx);
        }
        if previous.get("query") != node.props.get("query") && node.props.contains_key("query") {
            control.set_query(text(node, "query"), cx);
        }
        control.set_name(text(node, "name"), cx);
        control.set_allow_custom(flag(node, "allowCustom"), cx);
    },
    |control: &mut Combobox,
     method: &str,
     args: &Value,
     query: bool,
     window: &mut Window,
     cx: &mut gpui::Context<Combobox>|
     -> anyhow::Result<Value> {
        if query {
            return match method {
            "selected_id"=>Ok(json!(control.selected_id().map(|s|s.as_ref()))),
            "selected_option"=>Ok(control.selected_option().map_or(Value::Null,|o|json!({"id":o.id.as_ref(),"label":o.label.as_ref(),"description":o.description.as_ref().map(|s|s.as_ref()),"group":o.group.as_ref().map(|s|s.as_ref()),"disabled":o.disabled}))),
            "query_text"=>Ok(json!(control.query_text(cx).as_ref())),"is_open"=>Ok(json!(control.is_open())),
            _=>anyhow::bail!("unsupported Combobox query"),
        };
        }
        match method {
            "set_options" => {
                control.set_options(super::super::select_options(Some(&args["options"])), cx)
            }
            "set_selected" => control.set_selected(optional(&args["selected"]), cx),
            "set_query" => {
                control.set_query(args["text"].as_str().unwrap_or_default().to_owned(), cx)
            }
            "set_name" => {
                control.set_name(args["name"].as_str().unwrap_or_default().to_owned(), cx)
            }
            "set_allow_custom" => {
                control.set_allow_custom(args["allow"].as_bool().unwrap_or(false), cx)
            }
            "open" => control.open(cx),
            "toggle" => control.toggle(window, cx),
            _ => anyhow::bail!("unsupported Combobox command"),
        }
        Ok(Value::Null)
    }
);

#[cfg(all(test, feature = "capture"))]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use gpui_kit_testkit::harness::Harness;

    #[gpui::test]
    fn tag_limit_removal_preserves_refused_draft_and_emits_only_intent(cx: &mut TestAppContext) {
        let owner = gpui::EffectOwner::new();
        let state = Rc::new(State::default());
        let descriptor=Rc::new(RefCell::new(serde_json::from_value::<Node>(json!({"kind":"kit","component":"TagInput","id":"tags","props":{"tags":["alpha","omega"],"max":2},"events":{"added":"add","refused":"refusal","duplicate":"duplicate"}})).expect("fixture")));
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
                    build_state.render_tag_input(
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
        harness.click("tags.field");
        harness.keystrokes("b e t a enter");
        assert_eq!(
            events.borrow().last().map(|e| e.0.as_str()),
            Some("refusal")
        );
        let input = harness.update(|_, cx| {
            state.tag_inputs.borrow()[&(0, "tags".into())]
                .entity
                .read(cx)
                .field()
                .clone()
        });
        harness.update(|_, cx| assert_eq!(input.read(cx).value().as_ref(), "beta"));
        descriptor.borrow_mut().props.remove("max");
        harness.update(|_, cx| cx.refresh_windows());
        harness.keystrokes("enter");
        assert_eq!(events.borrow().last(), Some(&("add".into(), json!("beta"))));
        harness.update(|_, cx| {
            let entity = state.tag_inputs.borrow()[&(0, "tags".into())]
                .entity
                .clone();
            assert_eq!(entity.read(cx).field(), &input);
            assert_eq!(
                entity
                    .read(cx)
                    .current()
                    .iter()
                    .map(|s| s.as_ref())
                    .collect::<Vec<_>>(),
                vec!["alpha", "omega"]
            );
        });
        harness.keystrokes("a l p h a enter");
        assert_eq!(
            events.borrow().last(),
            Some(&("duplicate".into(), json!("alpha")))
        );
    }

    #[gpui::test]
    fn selection_options_keep_real_drafts_and_revoke_disabled_child_refs(cx: &mut TestAppContext) {
        for component in ["Combobox", "MultiSelect", "TagInput"] {
            let owner = gpui::EffectOwner::new();
            let state = Rc::new(State::default());
            let props = if component == "TagInput" {
                json!({"tags":["alpha","omega"],"placeholder":"Temporary","max":2,"collapseAt":1})
            } else {
                json!({"options":[{"id":"alpha","label":"Alpha","description":"First","group":"Letters"},{"id":"omega","label":"Omega","disabled":true}],"placeholder":"Temporary"})
            };
            let descriptor=Rc::new(RefCell::new(serde_json::from_value::<Node>(json!({"kind":"kit","component":component,"id":"selection","props":props,"events":{}})).expect("descriptor")));
            let (build_state, build_node) = (state.clone(), descriptor.clone());
            let mut harness = Harness::new(
                cx,
                move |cx| {
                    gpui_kit::install(cx);
                    gpui_kit::foundation::register_owner_state(owner, cx);
                },
                move |window, cx| {
                    let element = cx.with_effect_owner(Some(owner), |cx| {
                        build_state.render(
                            &build_node.borrow(),
                            BTreeMap::new(),
                            window,
                            cx,
                            Rc::new(|_, _| {}),
                        )
                    });
                    gpui::effect_owner(owner, element).into_any_element()
                },
            );
            let registry = crate::references::Registry::new();
            let getter = if component == "TagInput" {
                "field"
            } else {
                "query_input"
            };
            let (input_ref, focus, identity) = harness.update(|window, cx| {
                let node = descriptor.borrow();
                let registration = registry.registration(&node, owner);
                let input = state
                    .selection_reference_query(&node, getter, &json!({}), cx, &registration)
                    .expect("getter")
                    .expect("actual input");
                registry
                    .invoke(
                        owner,
                        &input,
                        "set_value",
                        &json!({"value":"Draft λ"}),
                        false,
                        window,
                        cx,
                    )
                    .expect("set draft");
                let focus = registry
                    .invoke(owner, &input, "focus_handle", &json!({}), true, window, cx)
                    .expect("focus");
                registry
                    .invoke(owner, &focus, "focus", &json!({}), false, window, cx)
                    .expect("focus input");
                (
                    input,
                    focus,
                    state.native_entity_id(&node).expect("identity"),
                )
            });
            harness.keystrokes("end x shift-left");
            descriptor.borrow_mut().props.remove("placeholder");
            descriptor.borrow_mut().props.remove("max");
            descriptor.borrow_mut().props.remove("collapseAt");
            descriptor
                .borrow_mut()
                .props
                .insert("size".into(), json!("lg"));
            harness.update(|_, cx| cx.refresh_windows());
            let snapshot = harness.snapshot();
            assert!(
                snapshot
                    .nodes
                    .iter()
                    .all(|node| node.placeholder.as_deref() != Some("Temporary"))
            );
            assert!(snapshot.nodes.iter().any(|node| node.placeholder.is_some()));
            harness.update(|window, cx| {
                assert_eq!(state.native_entity_id(&descriptor.borrow()), Some(identity));
                assert_eq!(
                    registry
                        .invoke(owner, &input_ref, "value", &json!({}), true, window, cx)
                        .expect("retained draft"),
                    json!("Draft λx")
                );
                let native = match component {
                    "Combobox" => state.comboboxes.borrow()[&(0, "selection".into())]
                        .entity
                        .read(cx)
                        .query_input()
                        .clone(),
                    "MultiSelect" => state.multi_selects.borrow()[&(0, "selection".into())]
                        .entity
                        .read(cx)
                        .query_input()
                        .clone(),
                    _ => state.tag_inputs.borrow()[&(0, "selection".into())]
                        .entity
                        .read(cx)
                        .field()
                        .clone(),
                };
                let range = native.read(cx).selected_range();
                assert_eq!(range.end - range.start, 1);
                if component == "TagInput" {
                    let entity = state.tag_inputs.borrow()[&(0, "selection".into())]
                        .entity
                        .clone();
                    assert_eq!(entity.read(cx).current().len(), 2);
                    state
                        .invoke(
                            &descriptor.borrow(),
                            "set_max",
                            &json!({"max":null}),
                            false,
                            window,
                            cx,
                        )
                        .expect("remove limit");
                } else {
                    state
                        .invoke(&descriptor.borrow(), "open", &json!({}), false, window, cx)
                        .expect("open");
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
                assert!(native.read(cx).is_disabled());
                assert!(
                    registry
                        .invoke(
                            owner,
                            &input_ref,
                            "set_value",
                            &json!({"value":"Denied"}),
                            false,
                            window,
                            cx
                        )
                        .is_err()
                );
                assert!(
                    registry
                        .invoke(owner, &focus, "focus", &json!({}), false, window, cx)
                        .is_err()
                );
                assert_eq!(
                    registry
                        .invoke(owner, &input_ref, "value", &json!({}), true, window, cx)
                        .expect("disabled readable"),
                    json!("Draft λx")
                );
                if component != "TagInput" {
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
                            .expect("closed"),
                        json!(false)
                    );
                }
            });
        }
    }
}

selection_adapter!(
    render_multi_select,
    invoke_multi_select,
    multi_selects,
    MultiSelect,
    MultiSelectEvent,
    |event: &MultiSelectEvent| match event {
        MultiSelectEvent::QueryChanged(s) => ("queryChanged", json!(s.as_ref())),
        MultiSelectEvent::Toggled(s) => ("toggled", json!(s.as_ref())),
        MultiSelectEvent::Removed(s) => ("removed", json!(s.as_ref())),
        MultiSelectEvent::Cleared => ("cleared", Value::Null),
        MultiSelectEvent::Opened => ("opened", Value::Null),
        MultiSelectEvent::Closed => ("closed", Value::Null),
    },
    |control: &mut MultiSelect,
     node: &Node,
     previous: &serde_json::Map<String, Value>,
     cx: &mut gpui::Context<MultiSelect>| {
        if previous.get("options") != node.props.get("options") {
            control.set_options(super::super::select_options(node.props.get("options")), cx);
        }
        if previous.get("selected") != node.props.get("selected") {
            control.set_selected(
                strings(node.props.get("selected").unwrap_or(&Value::Null)),
                cx,
            );
        }
        control.set_name(text(node, "name").into(), cx);
        control.set_clearable(flag(node, "clearable"), cx);
    },
    |control: &mut MultiSelect,
     method: &str,
     args: &Value,
     query: bool,
     window: &mut Window,
     cx: &mut gpui::Context<MultiSelect>|
     -> anyhow::Result<Value> {
        if query {
            return match method {
                "selected_ids" => Ok(json!(
                    control
                        .selected_ids()
                        .iter()
                        .map(|s| s.as_ref())
                        .collect::<Vec<_>>()
                )),
                "is_open" => Ok(json!(control.is_open())),
                _ => anyhow::bail!("unsupported MultiSelect query"),
            };
        }
        match method {
            "set_options" => {
                control.set_options(super::super::select_options(Some(&args["options"])), cx)
            }
            "set_selected" => control.set_selected(strings(&args["selected"]), cx),
            "set_name" => control.set_name(
                args["name"].as_str().unwrap_or_default().to_owned().into(),
                cx,
            ),
            "set_clearable" => {
                control.set_clearable(args["clearable"].as_bool().unwrap_or(false), cx)
            }
            "open" => control.open(window, cx),
            _ => anyhow::bail!("unsupported MultiSelect command"),
        }
        Ok(Value::Null)
    }
);

selection_adapter!(
    render_tag_input,
    invoke_tag_input,
    tag_inputs,
    TagInput,
    TagInputEvent,
    |event: &TagInputEvent| match event {
        TagInputEvent::Added(s) => ("added", json!(s.as_ref())),
        TagInputEvent::Removed(s) => ("removed", json!(s.as_ref())),
        TagInputEvent::Duplicate(s) => ("duplicate", json!(s.as_ref())),
        TagInputEvent::Refused(s) => ("refused", json!(s.as_ref())),
        TagInputEvent::EditRequested(s) => ("editRequested", json!(s.as_ref())),
        TagInputEvent::Moved { from, to } => ("moved", json!({"from":from,"to":to})),
    },
    |control: &mut TagInput,
     node: &Node,
     previous: &serde_json::Map<String, Value>,
     cx: &mut gpui::Context<TagInput>| {
        if previous.get("tags") != node.props.get("tags") {
            control.set_tags(strings(node.props.get("tags").unwrap_or(&Value::Null)), cx);
        }
        control.set_max(
            node.props
                .get("max")
                .and_then(Value::as_u64)
                .map(|n| n as usize),
            cx,
        );
        control.set_collapse_at(
            node.props
                .get("collapseAt")
                .and_then(Value::as_u64)
                .map(|n| n as usize),
            cx,
        );
        control.set_reorderable(flag(node, "reorderable"), cx);
    },
    |control: &mut TagInput,
     method: &str,
     args: &Value,
     query: bool,
     _window: &mut Window,
     cx: &mut gpui::Context<TagInput>|
     -> anyhow::Result<Value> {
        if query {
            return match method {
                "current" => Ok(json!(
                    control
                        .current()
                        .iter()
                        .map(|s| s.as_ref())
                        .collect::<Vec<_>>()
                )),
                "targeted" => Ok(json!(control.targeted().map(|s| s.as_ref()))),
                "refusal" => Ok(json!(control.refusal().map(|s| s.as_ref()))),
                _ => anyhow::bail!("unsupported TagInput query"),
            };
        }
        match method {
            "set_tags" => control.set_tags(strings(&args["tags"]), cx),
            "set_max" => control.set_max(args["max"].as_u64().map(|n| n as usize), cx),
            "set_collapse_at" => {
                control.set_collapse_at(args["visible"].as_u64().map(|n| n as usize), cx)
            }
            "set_reorderable" => {
                control.set_reorderable(args["reorderable"].as_bool().unwrap_or(false), cx)
            }
            _ => anyhow::bail!("unsupported TagInput command"),
        };
        Ok(Value::Null)
    }
);
