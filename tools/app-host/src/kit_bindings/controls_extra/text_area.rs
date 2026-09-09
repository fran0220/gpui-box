use super::*;
use gpui_kit::controls::textarea::{Enter, Frame, TextArea, TextAreaEvent, TextAreaWrap};

pub(super) fn range(value: &Value) -> std::ops::Range<usize> {
    value["start"].as_u64().unwrap_or(0) as usize..value["end"].as_u64().unwrap_or(0) as usize
}
pub(super) fn range_value(range: std::ops::Range<usize>) -> Value {
    json!({"start":range.start,"end":range.end})
}
pub(super) fn bounds_value(bounds: gpui::Bounds<gpui::Pixels>) -> Value {
    json!({"x":f32::from(bounds.origin.x),"y":f32::from(bounds.origin.y),"width":f32::from(bounds.size.width),"height":f32::from(bounds.size.height)})
}
fn wrap(value: &Value) -> TextAreaWrap {
    if value == "none" {
        TextAreaWrap::None
    } else {
        TextAreaWrap::Soft
    }
}
fn frame(value: &Value) -> Frame {
    if value == "host" {
        Frame::Host
    } else {
        Frame::Own
    }
}
fn enter(value: &Value) -> Enter {
    if value == "submits" {
        Enter::Submits
    } else {
        Enter::Opens
    }
}

fn event_name(event: &TextAreaEvent) -> &'static str {
    match event {
        TextAreaEvent::Edited(_) => "edited",
        TextAreaEvent::Change(_) => "change",
        TextAreaEvent::Submit => "submit",
        TextAreaEvent::Cancel => "cancel",
        TextAreaEvent::MoveUp => "moveUp",
        TextAreaEvent::MoveDown => "moveDown",
        TextAreaEvent::AcceptCompletion => "acceptCompletion",
        TextAreaEvent::DismissCompletion => "dismissCompletion",
        TextAreaEvent::IndentRequested => "indentRequested",
        TextAreaEvent::OutdentRequested => "outdentRequested",
        TextAreaEvent::SelectionChanged(_) => "selectionChanged",
        TextAreaEvent::GeometryChanged => "geometryChanged",
        TextAreaEvent::Focus => "focus",
        TextAreaEvent::Blur => "blur",
        // OS attachments need the shared host capability bridge; paths or metadata
        // are not a serialized substitute for files/images.
        TextAreaEvent::Pasted(_) => "pasteRefused",
    }
}
pub(super) fn event_value(event: &TextAreaEvent) -> Value {
    match event {
        TextAreaEvent::Change(snapshot) => json!(snapshot.text().as_ref()),
        TextAreaEvent::Edited(edit) => {
            json!({"revision":edit.revision,"replaced":range_value(edit.replaced.clone()),"inserted":edit.inserted.as_ref()})
        }
        TextAreaEvent::SelectionChanged(range) => range_value(range.clone()),
        TextAreaEvent::Pasted(pasted) => {
            json!({"state":"unavailable","kind":match pasted{gpui_kit::controls::textarea::Pasted::Images(_)=>"images",gpui_kit::controls::textarea::Pasted::Paths(_)=>"paths"},"reason":"External paste attachments are not authorized by this host"})
        }
        _ => Value::Null,
    }
}

impl State {
    pub(super) fn render_text_area(
        &self,
        node: &Node,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.text_areas.borrow().get(&key).cloned();
        let entry = existing.unwrap_or_else(|| {
            let entity = cx.new(|cx| TextArea::new(node.id.clone(), window, cx));
            let route = Rc::new(RefCell::new(Route {
                events: node.events.clone(),
                emit: emit.clone(),
                disabled: flag(node, "disabled"),
            }));
            let callback = Rc::downgrade(&route);
            let subscription = cx.subscribe(&entity, move |entity, event: &TextAreaEvent, cx| {
                if entity.read(cx).is_disabled() {
                    return;
                }
                let name = event_name(event);
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
                // Resolve the matching handler before any legacy flattening.
                if let Some((action, emit)) = target {
                    emit(&action, event_value(event));
                }
            });
            let entry = Rc::new(Entry {
                entity,
                route,
                props: Default::default(),
                _subscription: subscription,
            });
            self.text_areas.borrow_mut().insert(key, entry.clone());
            entry
        });
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if *entry.props.borrow() != node.props {
            entry.entity.update(cx, |area, cx| {
                area.set_placeholder(text(node, "placeholder"), cx);
                area.set_frame(frame(node.props.get("frame").unwrap_or(&Value::Null)), cx);
                area.set_wrap(wrap(node.props.get("wrap").unwrap_or(&Value::Null)), cx);
                area.set_required(flag(node, "required"), cx);
                area.set_invalid(flag(node, "invalid"), cx);
                area.set_read_only(flag(node, "readOnly"), cx);
                area.set_control_size(size(node), cx);
                area.set_rows(
                    node.props.get("rows").and_then(Value::as_u64).unwrap_or(3) as usize,
                    cx,
                );
                area.set_max_rows(
                    node.props
                        .get("maxRows")
                        .and_then(Value::as_u64)
                        .map(|n| n as usize),
                    cx,
                );
                if let Some(value) = node.props.get("autosize") {
                    area.set_autosize(
                        Some((
                            value["min"].as_u64().unwrap_or(1) as usize,
                            value["max"].as_u64().unwrap_or(1) as usize,
                        )),
                        cx,
                    );
                }
                area.set_enter(enter(node.props.get("enter").unwrap_or(&Value::Null)), cx);
                area.set_max_length(
                    node.props
                        .get("maxLength")
                        .and_then(Value::as_u64)
                        .map(|n| n as usize),
                    cx,
                );
                area.set_arrows_claimed(flag(node, "arrowsClaimed"));
                area.set_completion_claimed(flag(node, "completionClaimed"));
                area.set_disabled(flag(node, "disabled"), cx);
                if entry.props.borrow().get("value") != node.props.get("value")
                    && node.props.contains_key("value")
                {
                    area.set_value(text(node, "value"), cx);
                }
            });
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }

    pub(super) fn invoke_text_area(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        let entity = self
            .text_areas
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .map(|entry| entry.entity.clone())
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        anyhow::ensure!(
            query || !flag(node, "disabled"),
            "disabled descriptor refuses invocation"
        );
        text_area_value(&entity, method, args, query, window, cx)
    }
}

pub(super) fn text_area_dispatch(
    target: &Entity<TextArea>,
    method: &str,
    args: &Value,
    query: bool,
    window: &mut Window,
    cx: &mut App,
    refs: &crate::references::Registration<'_>,
) -> anyhow::Result<Value> {
    let schema = super::super::validation::invocation("TextArea", method, args, query)?;
    let result = if query && method == "focus_handle" {
        let focus = gpui::Focusable::focus_handle(target.read(cx), cx);
        refs.focus(
            target,
            &focus,
            |area, focus, cx| gpui::Focusable::focus_handle(area, cx) == *focus,
            |area, _| !area.is_disabled(),
        )?
    } else {
        text_area_value(target, method, args, query, window, cx)?
    };
    super::super::validation::validate(&result, schema)?;
    Ok(result)
}

fn text_area_value(
    target: &Entity<TextArea>,
    method: &str,
    args: &Value,
    query: bool,
    window: &mut Window,
    cx: &mut App,
) -> anyhow::Result<Value> {
    anyhow::ensure!(
        query || !target.read(cx).is_disabled(),
        "disabled native editor refuses invocation"
    );
    if query {
        let area = target.read(cx);
        return Ok(match method{
            "value"=>json!(area.value().as_ref()),"snapshot"=>{let snapshot=area.snapshot();json!({"revision":snapshot.revision,"text":snapshot.text.as_ref()})},
            "revision"=>json!(area.revision()),"is_empty"=>json!(area.is_empty()),"is_disabled"=>json!(area.is_disabled()),"is_read_only"=>json!(area.is_read_only()),"wrap_mode"=>json!(match area.wrap_mode(){TextAreaWrap::Soft=>"soft",TextAreaWrap::None=>"none"}),
            "selected_range"=>range_value(area.selected_range()),"cursor_offset"=>json!(area.cursor_offset()),"cursor_row"=>json!(area.cursor_row()),"arrows_claimed"=>json!(area.arrows_claimed()),"completion_claimed"=>json!(area.completion_claimed()),
            "selections"=>json!(area.selections().into_iter().map(|(range,reversed)|json!({"range":range_value(range),"reversed":reversed})).collect::<Vec<_>>()),
            "bounds_for_range"=>area.bounds_for_range(range(&args["range"])).map_or(Value::Null,|bounds|json!(bounds.into_iter().map(bounds_value).collect::<Vec<_>>())),"bounds_for_position"=>area.bounds_for_position(args["offset"].as_u64().unwrap_or(0) as usize).map_or(Value::Null,bounds_value),"caret_bounds"=>area.caret_bounds().map_or(Value::Null,bounds_value),
            "measured"=>area.measured().map_or(Value::Null,|m|json!({"text":f32::from(m.text),"height":f32::from(m.height),"wrapped":f32::from(m.wrapped),"pass":m.pass})),"horizontal_scroll_offset"=>json!(f32::from(area.horizontal_scroll_offset())),
            _=>anyhow::bail!("unsupported TextArea query"),
        });
    }
    if method == "focus" {
        gpui::Focusable::focus_handle(target.read(cx), cx).focus(window, cx);
        return Ok(Value::Null);
    }
    target.update(cx, |area, cx| {
        match method {
            "set_value" => {
                area.set_value(args["value"].as_str().unwrap_or_default().to_owned(), cx)
            }
            "insert" => area.insert(args["text"].as_str().unwrap_or_default(), cx),
            "replace_range" => {
                return Ok(area
                    .replace_range(
                        range(&args["range"]),
                        args["text"].as_str().unwrap_or_default(),
                        cx,
                    )
                    .map_or(Value::Null, range_value));
            }
            "replace_ranges" => {
                return Ok(json!(area.replace_ranges(
                    args["edits"].as_array().into_iter().flatten().map(|v| (
                        range(&v["range"]),
                        v["text"].as_str().unwrap_or_default().to_owned().into()
                    )),
                    cx
                )));
            }
            "set_selected_range" => area.set_selected_range(range(&args["range"]), cx),
            "set_selections" => {
                return Ok(json!(
                    area.set_selections(
                        args["selections"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|v| (
                                range(&v["range"]),
                                v["reversed"].as_bool().unwrap_or(false)
                            )),
                        cx
                    )
                ));
            }
            "select_rectangle" => {
                return Ok(json!(area.select_rectangle(
                    gpui::point(
                        gpui::px(args["anchor"]["x"].as_f64().unwrap_or(0.) as f32),
                        gpui::px(args["anchor"]["y"].as_f64().unwrap_or(0.) as f32)
                    ),
                    gpui::point(
                        gpui::px(args["focus"]["x"].as_f64().unwrap_or(0.) as f32),
                        gpui::px(args["focus"]["y"].as_f64().unwrap_or(0.) as f32)
                    ),
                    cx
                )));
            }
            "set_placeholder" => area.set_placeholder(
                args["placeholder"].as_str().unwrap_or_default().to_owned(),
                cx,
            ),
            "set_frame" => area.set_frame(frame(&args["frame"]), cx),
            "set_wrap" => area.set_wrap(wrap(&args["wrap"]), cx),
            "set_enter" => area.set_enter(enter(&args["enter"]), cx),
            "set_rows" => area.set_rows(args["rows"].as_u64().unwrap_or(1) as usize, cx),
            "set_max_rows" => area.set_max_rows(args["max_rows"].as_u64().map(|n| n as usize), cx),
            "set_autosize" => area.set_autosize(
                args["rows"].as_object().map(|v| {
                    (
                        v["min"].as_u64().unwrap_or(1) as usize,
                        v["max"].as_u64().unwrap_or(1) as usize,
                    )
                }),
                cx,
            ),
            "set_max_length" => {
                area.set_max_length(args["max_length"].as_u64().map(|n| n as usize), cx)
            }
            "set_disabled" => area.set_disabled(args["disabled"].as_bool().unwrap_or(false), cx),
            "set_read_only" => area.set_read_only(args["read_only"].as_bool().unwrap_or(false), cx),
            "set_invalid" => area.set_invalid(args["invalid"].as_bool().unwrap_or(false), cx),
            "set_required" => area.set_required(args["required"].as_bool().unwrap_or(false), cx),
            "set_arrows_claimed" => {
                area.set_arrows_claimed(args["claimed"].as_bool().unwrap_or(false))
            }
            "set_completion_claimed" => {
                area.set_completion_claimed(args["claimed"].as_bool().unwrap_or(false))
            }
            "set_control_size" => area.set_control_size(
                match args["size"].as_str() {
                    Some("xs") => ControlSize::Xs,
                    Some("sm") => ControlSize::Sm,
                    Some("lg") => ControlSize::Lg,
                    _ => ControlSize::Md,
                },
                cx,
            ),
            _ => anyhow::bail!("unsupported TextArea command"),
        }
        Ok(Value::Null)
    })
}

#[cfg(all(test, feature = "capture"))]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use gpui_kit_testkit::harness::Harness;
    #[gpui::test]
    fn area_options_preserve_history_and_refuse_invalid_unicode_edits(cx: &mut TestAppContext) {
        let owner = gpui::EffectOwner::new();
        let state = Rc::new(State::default());
        let descriptor=Rc::new(RefCell::new(serde_json::from_value::<Node>(json!({"kind":"kit","component":"TextArea","id":"area","props":{"value":"AλZ","placeholder":"Temporary","rows":2,"maxRows":8,"maxLength":20},"events":{"edited":"edit","pasteRefused":"refused"}})).expect("fixture")));
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
                    build_state.render_text_area(
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
        harness.click("area");
        harness.keystrokes("end x");
        descriptor.borrow_mut().props.remove("maxRows");
        descriptor.borrow_mut().props.remove("maxLength");
        descriptor.borrow_mut().props.remove("placeholder");
        descriptor
            .borrow_mut()
            .props
            .insert("wrap".into(), json!("none"));
        descriptor
            .borrow_mut()
            .props
            .insert("size".into(), json!("lg"));
        harness.update(|_, cx| cx.refresh_windows());
        harness.update(|window, cx| {
            assert_eq!(
                state
                    .invoke_text_area(&descriptor.borrow(), "value", &json!({}), true, window, cx)
                    .expect("value"),
                json!("AλZx")
            );
            assert_eq!(
                state
                    .invoke_text_area(
                        &descriptor.borrow(),
                        "replace_range",
                        &json!({"range":{"start":2,"end":3},"text":"bad"}),
                        false,
                        window,
                        cx
                    )
                    .expect("native boundary refusal"),
                Value::Null
            );
            assert_eq!(
                state
                    .invoke_text_area(
                        &descriptor.borrow(),
                        "wrap_mode",
                        &json!({}),
                        true,
                        window,
                        cx
                    )
                    .expect("wrap"),
                json!("none")
            );
        });
        harness.keystrokes("ctrl-z");
        harness.update(|window,cx|{
            assert_eq!(state.invoke_text_area(&descriptor.borrow(),"value",&json!({}),true,window,cx).expect("undo survived"),json!("AλZ"));
            let result=state.invoke_text_area(&descriptor.borrow(),"replace_ranges",&json!({"edits":[{"range":{"start":1,"end":3},"text":"BC"},{"range":{"start":3,"end":4},"text":"!"}]}),false,window,cx).expect("atomic edits");assert_eq!(result,json!(true));
            assert_eq!(state.invoke_text_area(&descriptor.borrow(),"value",&json!({}),true,window,cx).expect("edited value"),json!("ABC!"));
            state.invoke_text_area(&descriptor.borrow(),"set_read_only",&json!({"read_only":true}),false,window,cx).expect("readonly");
            assert_eq!(state.invoke_text_area(&descriptor.borrow(),"replace_range",&json!({"range":{"start":0,"end":1},"text":"bad"}),false,window,cx).expect("readonly refusal"),Value::Null);
            let entity=state.text_areas.borrow()[&(0,"area".into())].entity.clone();entity.update(cx,|_,cx|cx.emit(TextAreaEvent::Pasted(gpui_kit::controls::textarea::Pasted::Paths(vec!["/private/not-exposed".into()]))));
        });
        assert!(events.borrow().iter().any(|(name, value)| name == "refused"
            && value["kind"] == "paths"
            && !value.to_string().contains("private")));
    }
}
