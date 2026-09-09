use super::*;
use crate::references::Registration;
use gpui_kit::controls::search::{
    FindReplace, FindReplaceEvent, HitCount, SearchField, SearchFieldEvent,
};

fn hit_count(value: Option<&Value>) -> HitCount {
    let value = value.unwrap_or(&Value::Null);
    match value["state"].as_str().unwrap_or("unsearched") {
        "counting" => HitCount::Counting,
        "none" => HitCount::None,
        "known" => HitCount::Known {
            total: value["total"].as_u64().unwrap_or(0) as usize,
            current: value["current"].as_u64().map(|v| v as usize),
        },
        "tooMany" => HitCount::TooMany {
            counted: value["counted"].as_u64().unwrap_or(0) as usize,
        },
        "unavailable" => HitCount::Unavailable(
            value["reason"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
                .into(),
        ),
        _ => HitCount::Unsearched,
    }
}
fn count_value(count: &HitCount) -> Value {
    match count {
        HitCount::Unsearched => json!({"state":"unsearched"}),
        HitCount::Counting => json!({"state":"counting"}),
        HitCount::None => json!({"state":"none"}),
        HitCount::Known { total, current } => {
            json!({"state":"known","total":total,"current":current})
        }
        HitCount::TooMany { counted } => json!({"state":"tooMany","counted":counted}),
        HitCount::Unavailable(reason) => json!({"state":"unavailable","reason":reason.as_ref()}),
    }
}
fn search_event(event: &SearchFieldEvent) -> (&'static str, Value) {
    match event {
        SearchFieldEvent::QueryChanged(value) => ("queryChanged", json!(value.as_ref())),
        SearchFieldEvent::Next => ("next", Value::Null),
        SearchFieldEvent::Previous => ("previous", Value::Null),
        SearchFieldEvent::Cancelled => ("cancelled", Value::Null),
        SearchFieldEvent::MatchCaseToggled(value) => ("matchCaseToggled", json!(value)),
        SearchFieldEvent::WholeWordToggled(value) => ("wholeWordToggled", json!(value)),
    }
}

fn search_refs(
    target: &Entity<SearchField>,
    method: &str,
    args: &Value,
    cx: &App,
    refs: &Registration<'_>,
) -> Option<anyhow::Result<Value>> {
    if !matches!(method, "query_input" | "focus_handle") {
        return None;
    }
    Some((|| {
        let schema = super::super::validation::invocation("SearchField", method, args, true)?;
        let value = if method == "query_input" {
            refs.entity(
                "TextInput",
                target,
                target.read(cx).query_input(),
                |parent, _| Some(parent.query_input().clone()),
                |parent, _| !parent.is_disabled(),
                super::super::reference_dispatch::text_input,
            )?
        } else {
            let focus = gpui::Focusable::focus_handle(target.read(cx), cx);
            refs.focus(
                target,
                &focus,
                |parent, focus, cx| gpui::Focusable::focus_handle(parent, cx) == *focus,
                |parent, _| !parent.is_disabled(),
            )?
        };
        super::super::validation::validate(&value, schema)?;
        Ok(value)
    })())
}
fn search_value(
    target: &Entity<SearchField>,
    method: &str,
    args: &Value,
    query: bool,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<Value> {
    let schema = super::super::validation::invocation("SearchField", method, args, query)?;
    anyhow::ensure!(
        query || !target.read(cx).is_disabled(),
        "disabled target refuses invocation"
    );
    let result = if query {
        let control = target.read(cx);
        match method {
            "count" => count_value(control.count()),
            "query_text" => json!(control.query_text(cx).as_ref()),
            "is_disabled" => json!(control.is_disabled()),
            _ => anyhow::bail!("unsupported SearchField query"),
        }
    } else {
        target.update(cx, |control, cx| -> anyhow::Result<()> {
            match method {
                "set_query" => {
                    control.set_query(args["text"].as_str().unwrap_or_default().to_owned(), cx)
                }
                "set_count" => control.set_count(hit_count(args.get("count")), cx),
                "set_match_case" => control.set_match_case(args["on"].as_bool(), cx),
                "set_whole_word" => control.set_whole_word(args["on"].as_bool(), cx),
                "set_placeholder" => control.set_placeholder(
                    args["placeholder"].as_str().map(|s| s.to_owned().into()),
                    cx,
                ),
                "set_disabled" => {
                    control.set_disabled(args["disabled"].as_bool().unwrap_or(false), cx)
                }
                "set_control_size" => control.set_control_size(search_size(args.get("size")), cx),
                "focus" => control.focus(window, cx),
                _ => anyhow::bail!("unsupported SearchField command"),
            }
            Ok(())
        })?;
        Value::Null
    };
    super::super::validation::validate(&result, schema)?;
    Ok(result)
}
fn search_size(value: Option<&Value>) -> gpui_kit_theme::ControlSize {
    match value.and_then(Value::as_str) {
        Some("xs") => gpui_kit_theme::ControlSize::Xs,
        Some("md") => gpui_kit_theme::ControlSize::Md,
        Some("lg") => gpui_kit_theme::ControlSize::Lg,
        _ => gpui_kit_theme::ControlSize::Sm,
    }
}
/// Borrowed SearchField dispatch uses exactly the descriptor method contract.
pub(super) fn search_field_dispatch(
    target: &Entity<SearchField>,
    method: &str,
    args: &Value,
    query: bool,
    window: &mut Window,
    cx: &mut App,
    refs: &Registration<'_>,
) -> anyhow::Result<Value> {
    if query && let Some(result) = search_refs(target, method, args, cx, refs) {
        return result;
    }
    search_value(target, method, args, query, window, cx)
}

impl State {
    pub(super) fn search_reference_query(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        cx: &App,
        refs: &Registration<'_>,
    ) -> Option<anyhow::Result<Value>> {
        let key = (node.instance, node.id.clone());
        match node.component.as_deref()? {
            "SearchField" => {
                let entity = self
                    .search_fields
                    .borrow()
                    .get(&key)
                    .map(|entry| entry.entity.clone());
                match entity {
                    Some(entity) => search_refs(&entity, method, args, cx, refs),
                    None => Some(Err(anyhow::anyhow!("native target is not mounted"))),
                }
            }
            "FindReplace"
                if matches!(
                    method,
                    "search_field" | "replacement_input" | "focus_handle"
                ) =>
            {
                Some((|| {
                    let schema =
                        super::super::validation::invocation("FindReplace", method, args, true)?;
                    let entity = self
                        .find_replaces
                        .borrow()
                        .get(&key)
                        .map(|entry| entry.entity.clone())
                        .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
                    let value = match method {
                        "search_field" => refs.entity(
                            "SearchField",
                            &entity,
                            entity.read(cx).search_field(),
                            |parent, _| Some(parent.search_field().clone()),
                            |parent, _| !parent.is_disabled(),
                            search_field_dispatch,
                        )?,
                        "replacement_input" => refs.entity(
                            "TextInput",
                            &entity,
                            entity.read(cx).replacement_input(),
                            |parent, _| Some(parent.replacement_input().clone()),
                            |parent, _| !parent.is_disabled(),
                            super::super::reference_dispatch::text_input,
                        )?,
                        _ => {
                            let focus = gpui::Focusable::focus_handle(entity.read(cx), cx);
                            refs.focus(
                                &entity,
                                &focus,
                                |parent, focus, cx| {
                                    gpui::Focusable::focus_handle(parent, cx) == *focus
                                },
                                |parent, _| !parent.is_disabled(),
                            )?
                        }
                    };
                    super::super::validation::validate(&value, schema)?;
                    Ok(value)
                })())
            }
            _ => None,
        }
    }

    pub(super) fn render_search_field(
        &self,
        node: &Node,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.search_fields.borrow().get(&key).cloned();
        let entry = existing.unwrap_or_else(|| {
            let entity = cx.new(|cx| SearchField::new(node.id.clone(), window, cx));
            let route = Rc::new(RefCell::new(Route {
                events: node.events.clone(),
                emit: emit.clone(),
                disabled: flag(node, "disabled"),
            }));
            let callback = Rc::downgrade(&route);
            let subscription =
                cx.subscribe(&entity, move |entity, event: &SearchFieldEvent, cx| {
                    if entity.read(cx).is_disabled() {
                        return;
                    }
                    let (name, payload) = search_event(event);
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
                        emit(&action, payload);
                    }
                });
            let entry = Rc::new(Entry {
                entity,
                route,
                props: Default::default(),
                _subscription: subscription,
            });
            self.search_fields.borrow_mut().insert(key, entry.clone());
            entry
        });
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if *entry.props.borrow() != node.props {
            let query_changed = entry.props.borrow().get("query") != node.props.get("query");
            entry.entity.update(cx, |control, cx| {
                control.set_placeholder(
                    node.props
                        .get("placeholder")
                        .and_then(Value::as_str)
                        .map(|s| s.to_owned().into()),
                    cx,
                );
                control.set_match_case(node.props.get("matchCase").and_then(Value::as_bool), cx);
                control.set_whole_word(node.props.get("wholeWord").and_then(Value::as_bool), cx);
                control.set_count(hit_count(node.props.get("count")), cx);
                control.set_control_size(search_size(node.props.get("size")), cx);
                control.set_disabled(flag(node, "disabled"), cx);
                if query_changed && node.props.contains_key("query") {
                    control.set_query(text(node, "query"), cx);
                }
            });
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }

    pub(super) fn render_find_replace(
        &self,
        node: &Node,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.find_replaces.borrow().get(&key).cloned();
        let entry = existing.unwrap_or_else(|| {
            let entity = cx.new(|cx| FindReplace::new(node.id.clone(), window, cx));
            let route = Rc::new(RefCell::new(Route {
                events: node.events.clone(),
                emit: emit.clone(),
                disabled: flag(node, "disabled"),
            }));
            let callback = Rc::downgrade(&route);
            let subscription =
                cx.subscribe(&entity, move |entity, event: &FindReplaceEvent, cx| {
                    if entity.read(cx).is_disabled() {
                        return;
                    }
                    let (name, payload) = match event {
                        FindReplaceEvent::Search(event) => {
                            let (kind, value) = search_event(event);
                            ("search", json!({"kind":kind,"value":value}))
                        }
                        FindReplaceEvent::ReplacementChanged(text) => {
                            ("replacementChanged", json!(text.as_ref()))
                        }
                        FindReplaceEvent::ReplaceOne => ("replaceOne", Value::Null),
                        FindReplaceEvent::ReplaceAll { count } => {
                            ("replaceAll", json!({"count":count}))
                        }
                        FindReplaceEvent::Close => ("close", Value::Null),
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
                        emit(&action, payload);
                    }
                });
            let entry = Rc::new(Entry {
                entity,
                route,
                props: Default::default(),
                _subscription: subscription,
            });
            self.find_replaces.borrow_mut().insert(key, entry.clone());
            entry
        });
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if *entry.props.borrow() != node.props {
            entry.entity.update(cx, |control, cx| {
                control.set_count(hit_count(node.props.get("count")), cx);
                control.set_control_size(search_size(node.props.get("size")), cx);
                control.set_disabled(flag(node, "disabled"), cx);
            });
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }

    pub(super) fn invoke_search(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        anyhow::ensure!(
            query || !flag(node, "disabled"),
            "disabled target refuses invocation"
        );
        if node.component.as_deref() == Some("SearchField") {
            let entity = self
                .search_fields
                .borrow()
                .get(&(node.instance, node.id.clone()))
                .map(|entry| entry.entity.clone())
                .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
            return search_value(&entity, method, args, query, window, cx);
        }
        let entity = self
            .find_replaces
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .map(|entry| entry.entity.clone())
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        anyhow::ensure!(
            query || !entity.read(cx).is_disabled(),
            "disabled target refuses invocation"
        );
        if query {
            let control = entity.read(cx);
            return match method {
                "count" => Ok(count_value(&control.count(cx))),
                "replacement_text" => Ok(json!(control.replacement_text(cx).as_ref())),
                "is_disabled" => Ok(json!(control.is_disabled())),
                _ => anyhow::bail!("unsupported FindReplace query"),
            };
        }
        entity.update(cx, |control, cx| {
            match method {
                "set_count" => control.set_count(hit_count(args.get("count")), cx),
                "set_disabled" => {
                    control.set_disabled(args["disabled"].as_bool().unwrap_or(false), cx)
                }
                "set_control_size" => control.set_control_size(search_size(args.get("size")), cx),
                _ => anyhow::bail!("unsupported FindReplace command"),
            };
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
    fn search_field_removes_optional_toggles_without_replacing_query(cx: &mut TestAppContext) {
        let state = Rc::new(State::default());
        let descriptor=Rc::new(RefCell::new(serde_json::from_value::<Node>(json!({"kind":"kit","component":"SearchField","id":"search","props":{"query":"needle","placeholder":"Custom search","matchCase":true,"wholeWord":false}})).expect("fixture")));
        let (build_state, build_node) = (state.clone(), descriptor.clone());
        let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
            build_state.render_search_field(&build_node.borrow(), window, cx, Rc::new(|_, _| {}))
        });
        let entity = state.search_fields.borrow()[&(0, "search".into())]
            .entity
            .clone();
        let input = harness.update(|_, cx| entity.read(cx).query_input().clone());
        harness.click("search.query");
        harness.keystrokes("end x shift-left");
        let range = harness.update(|_, cx| input.read(cx).selected_range());
        assert!(harness.snapshot().contains("search.match-case"));
        {
            let mut node = descriptor.borrow_mut();
            node.props.remove("placeholder");
            node.props.remove("matchCase");
            node.props.remove("wholeWord");
            node.props.insert("size".into(), json!("lg"));
        }
        harness.update(|_, cx| cx.refresh_windows());
        assert!(!harness.snapshot().contains("search.match-case"));
        assert!(!harness.snapshot().contains("search.whole-word"));
        harness.update(|window, cx| {
            assert_eq!(entity.read(cx).query_input(), &input);
            assert_eq!(input.read(cx).selected_range(), range);
            assert_eq!(input.read(cx).value().as_ref(), "needlex");
            for (method, args) in [
                ("set_match_case", json!({"on":null})),
                ("set_whole_word", json!({"on":null})),
                ("set_placeholder", json!({"placeholder":null})),
                ("set_control_size", json!({"size":"xs"})),
            ] {
                state
                    .invoke_search(&descriptor.borrow(), method, &args, false, window, cx)
                    .expect(method);
            }
            state
                .invoke_search(
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
                    .invoke_search(&descriptor.borrow(), "focus", &json!({}), false, window, cx)
                    .is_err()
            );
        });
    }

    #[gpui::test]
    fn search_refs_keep_inputs_and_enforce_count_and_ancestor_authority(cx: &mut TestAppContext) {
        let owner = gpui::EffectOwner::new();
        let state = Rc::new(State::default());
        let descriptor=Rc::new(RefCell::new(serde_json::from_value::<Node>(json!({"kind":"kit","component":"FindReplace","id":"find","props":{"count":{"state":"known","total":7,"current":2}},"events":{"replaceAll":"all","search":"search"}})).expect("fixture")));
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
                    build_state.render_find_replace(
                        &build_node.borrow(),
                        window,
                        cx,
                        Rc::new(move |name, payload| {
                            output.borrow_mut().push((name.to_owned(), payload))
                        }),
                    )
                });
                gpui::effect_owner(owner, element).into_any_element()
            },
        );
        let registry = crate::references::Registry::new();
        let (search, input, focus, native_search, native_input) = harness.update(|window, cx| {
            let node = descriptor.borrow();
            let registration = registry.registration(&node, owner);
            let search = state
                .search_reference_query(&node, "search_field", &json!({}), cx, &registration)
                .expect("query")
                .expect("SearchField ref");
            let input = registry
                .invoke(owner, &search, "query_input", &json!({}), true, window, cx)
                .expect("input ref");
            let focus = registry
                .invoke(owner, &input, "focus_handle", &json!({}), true, window, cx)
                .expect("nested focus");
            registry
                .invoke(
                    owner,
                    &search,
                    "set_query",
                    &json!({"text":"fixture"}),
                    false,
                    window,
                    cx,
                )
                .expect("set native query");
            let native_search = state.find_replaces.borrow()[&(0, "find".into())]
                .entity
                .read(cx)
                .search_field()
                .clone();
            let native_input = native_search.read(cx).query_input().clone();
            assert_eq!(native_input.read(cx).value().as_ref(), "fixture");
            (search, input, focus, native_search, native_input)
        });
        harness.click("find.replace-all");
        assert_eq!(
            events.borrow().last(),
            Some(&("all".into(), json!({"count":7})))
        );
        descriptor.borrow_mut().props =
            serde_json::from_value(json!({"size":"lg","count":{"state":"tooMany","counted":500}}))
                .expect("props");
        harness.update(|_, cx| cx.refresh_windows());
        events.borrow_mut().clear();
        harness.click("find.replace-all");
        assert!(events.borrow().is_empty());
        harness.update(|window, cx| {
            assert_eq!(native_search.read(cx).query_input(), &native_input);
            assert!(matches!(
                native_search.read(cx).count(),
                HitCount::TooMany { counted: 500 }
            ));
            for count in [
                json!({"state":"unsearched"}),
                json!({"state":"counting"}),
                json!({"state":"none"}),
                json!({"state":"known","total":9,"current":null}),
                json!({"state":"unavailable","reason":"Host refused search"}),
            ] {
                registry
                    .invoke(
                        owner,
                        &search,
                        "set_count",
                        &json!({"count":count}),
                        false,
                        window,
                        cx,
                    )
                    .expect("count update");
                assert_eq!(
                    registry
                        .invoke(owner, &search, "count", &json!({}), true, window, cx)
                        .expect("count query"),
                    count
                );
            }
            state
                .invoke_search(
                    &descriptor.borrow(),
                    "set_disabled",
                    &json!({"disabled":true}),
                    false,
                    window,
                    cx,
                )
                .expect("parent disable");
            assert!(
                registry
                    .invoke(owner, &search, "focus", &json!({}), false, window, cx)
                    .is_err()
            );
            assert!(
                registry
                    .invoke(
                        owner,
                        &input,
                        "set_value",
                        &json!({"value":"blocked"}),
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
                    .invoke(owner, &input, "value", &json!({}), true, window, cx)
                    .expect("read while disabled"),
                json!("fixture")
            );
            let replacement: Node =
                serde_json::from_value(json!({"kind":"text","id":"replacement","text":"Removed"}))
                    .expect("replacement");
            state.reconcile(&replacement, cx);
            registry.reconcile(&replacement, |_| Some(owner), |_| None, cx);
            assert!(
                registry
                    .invoke(owner, &input, "value", &json!({}), true, window, cx)
                    .is_err()
            );
            assert!(
                registry
                    .invoke(owner, &search, "query_text", &json!({}), true, window, cx)
                    .is_err()
            );
            assert!(
                registry
                    .invoke(owner, &focus, "is_focused", &json!({}), true, window, cx)
                    .is_err()
            );
        });
    }
}
