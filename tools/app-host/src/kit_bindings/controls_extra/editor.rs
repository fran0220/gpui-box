use super::text_area::{bounds_value, range, range_value};
use super::*;
use gpui_kit::controls::editor::{
    Editor, EditorEvent, EditorFold, EditorHover, EditorReplacement, EditorServiceEffect,
    EditorServiceItem, EditorServiceKind, EditorServiceRequest, EditorServiceResult,
};
use gpui_kit::controls::textarea::TextAreaEvent;

fn service_kind(value: &Value) -> EditorServiceKind {
    match value.as_str() {
        Some("hover") => EditorServiceKind::Hover,
        Some("definition") => EditorServiceKind::Definition,
        Some("codeActions") => EditorServiceKind::CodeActions,
        _ => EditorServiceKind::Completion,
    }
}
fn request_value(request: &EditorServiceRequest) -> Value {
    json!({"id":request.id,"revision":request.revision,"position":request.position,"selection":range_value(request.selection.clone()),"document":request.document.text().as_ref(),"kind":match request.kind{EditorServiceKind::Completion=>"completion",EditorServiceKind::Hover=>"hover",EditorServiceKind::Definition=>"definition",EditorServiceKind::CodeActions=>"codeActions"}})
}
fn service_result(
    value: &Value,
) -> gpui_kit::state::AsyncValue<EditorServiceResult, gpui::SharedString> {
    use gpui_kit::state::{AsyncStatus, AsyncValue};
    let result = value.get("value").filter(|v| !v.is_null()).map(|v| {
        if v["kind"] == "hover" {
            EditorServiceResult::Hover(EditorHover {
                range: range(&v["range"]),
                contents: v["contents"].as_str().unwrap_or_default().to_owned().into(),
            })
        } else {
            EditorServiceResult::Items(
                v["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|item| EditorServiceItem {
                        id: item["id"].as_str().unwrap_or_default().to_owned().into(),
                        label: item["label"].as_str().unwrap_or_default().to_owned().into(),
                        detail: item["detail"].as_str().map(|s| s.to_owned().into()),
                        effect: match item["effect"]["kind"].as_str() {
                            Some("definition") => EditorServiceEffect::Definition {
                                target: item["effect"]["target"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_owned()
                                    .into(),
                                range: range(&item["effect"]["range"]),
                            },
                            Some("action") => EditorServiceEffect::Action(
                                item["effect"]["id"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_owned()
                                    .into(),
                            ),
                            _ => EditorServiceEffect::Edits(
                                item["effect"]["edits"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .map(|edit| EditorReplacement {
                                        range: range(&edit["range"]),
                                        text: edit["text"]
                                            .as_str()
                                            .unwrap_or_default()
                                            .to_owned()
                                            .into(),
                                    })
                                    .collect(),
                            ),
                        },
                    })
                    .collect(),
            )
        }
    });
    AsyncValue {
        value: result,
        status: match value["state"].as_str() {
            Some("loading") => AsyncStatus::Loading,
            Some("refreshing") => AsyncStatus::Refreshing,
            Some("ready") => AsyncStatus::Ready,
            Some("empty") => AsyncStatus::Empty,
            Some("unavailable") => {
                AsyncStatus::Unavailable(value["reason"].as_str().unwrap_or_default().to_owned())
            }
            Some("error") => AsyncStatus::Error(
                value["reason"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
                    .into(),
            ),
            _ => AsyncStatus::Idle,
        },
        attempts: value["attempts"].as_u64().unwrap_or(0) as usize,
    }
}

fn event_name(event: &EditorEvent) -> &'static str {
    match event {
        EditorEvent::Changed(_) => "changed",
        EditorEvent::Edited(_) => "edited",
        EditorEvent::SelectionChanged(_) => "selectionChanged",
        EditorEvent::FoldChanged { .. } => "foldChanged",
        EditorEvent::Pasted(_) => "pasteRefused",
        EditorEvent::Submitted => "submitted",
        EditorEvent::Cancelled => "cancelled",
        EditorEvent::Focused => "focused",
        EditorEvent::Blurred => "blurred",
        EditorEvent::ServiceRequested(_) => "serviceRequested",
        EditorEvent::ServiceAccepted(_) => "serviceAccepted",
        EditorEvent::DefinitionRequested { .. } => "definitionRequested",
        EditorEvent::CodeActionRequested(_) => "codeActionRequested",
    }
}
fn event_value(event: &EditorEvent) -> Value {
    match event {
        EditorEvent::Changed(snapshot) => json!(snapshot.text().as_ref()),
        EditorEvent::Edited(edit) => text_area::event_value(&TextAreaEvent::Edited(edit.clone())),
        EditorEvent::SelectionChanged(range) => range_value(range.clone()),
        EditorEvent::FoldChanged { id, collapsed } => {
            json!({"id":id.as_ref(),"collapsed":collapsed})
        }
        EditorEvent::Pasted(pasted) => {
            text_area::event_value(&TextAreaEvent::Pasted(pasted.clone()))
        }
        EditorEvent::ServiceAccepted(id) | EditorEvent::CodeActionRequested(id) => {
            json!(id.as_ref())
        }
        EditorEvent::DefinitionRequested { target, range } => {
            json!({"target":target.as_ref(),"range":range_value(range.clone())})
        }
        EditorEvent::ServiceRequested(request) => request_value(request),
        _ => Value::Null,
    }
}

impl State {
    pub(super) fn render_editor(
        &self,
        node: &Node,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.editors.borrow().get(&key).cloned();
        let entry = existing.unwrap_or_else(|| {
            let entity = cx.new(|cx| {
                Editor::new(
                    node.id.clone(),
                    text(node, "label"),
                    text(node, "value"),
                    window,
                    cx,
                )
            });
            let route = Rc::new(RefCell::new(Route {
                events: node.events.clone(),
                emit: emit.clone(),
                disabled: flag(node, "disabled"),
            }));
            let callback = Rc::downgrade(&route);
            let subscription = cx.subscribe(&entity, move |entity, event: &EditorEvent, cx| {
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
            self.editors.borrow_mut().insert(key, entry.clone());
            entry
        });
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if *entry.props.borrow() != node.props {
            entry.entity.update(cx, |editor, cx| {
                editor.set_label(text(node, "label"), cx);
                editor.set_rows(
                    node.props.get("rows").and_then(Value::as_u64).unwrap_or(12) as usize,
                    cx,
                );
                editor.set_line_numbers(
                    node.props
                        .get("lineNumbers")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                    cx,
                );
                editor.set_read_only(flag(node, "readOnly"), cx);
                editor.set_language_services(flag(node, "languageServices"), cx);
                editor.set_disabled(flag(node, "disabled"), cx);
                if entry.props.borrow().get("value") != node.props.get("value")
                    && node.props.contains_key("value")
                {
                    editor.set_value(text(node, "value"), cx);
                }
            });
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }
    pub(super) fn editor_reference_query(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        cx: &App,
        refs: &crate::references::Registration<'_>,
    ) -> Option<anyhow::Result<Value>> {
        if method != "text_area" {
            return None;
        }
        Some((|| {
            let schema = super::super::validation::invocation("Editor", method, args, true)?;
            let key = (node.instance, node.id.clone());
            let entity = self
                .editors
                .borrow()
                .get(&key)
                .map(|entry| entry.entity.clone())
                .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
            let value = refs.entity(
                "TextArea",
                &entity,
                entity.read(cx).text_area(),
                |editor, _| Some(editor.text_area().clone()),
                |editor, _| !editor.is_disabled(),
                text_area::text_area_dispatch,
            )?;
            super::super::validation::validate(&value, schema)?;
            Ok(value)
        })())
    }
    pub(super) fn invoke_editor(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        _window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        let entity = self
            .editors
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .map(|entry| entry.entity.clone())
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        anyhow::ensure!(
            query || (!flag(node, "disabled") && !entity.read(cx).is_disabled()),
            "disabled editor refuses invocation"
        );
        if query {
            let editor = entity.read(cx);
            return Ok(match method{
            "snapshot"=>{let snapshot=editor.snapshot(cx);json!({"revision":snapshot.revision,"text":snapshot.text.as_ref()})},"is_disabled"=>json!(editor.is_disabled()),"is_read_only"=>json!(editor.is_read_only()),"selected_range"=>range_value(editor.selected_range(cx)),"selections"=>json!(editor.selections(cx).into_iter().map(|(range,reversed)|json!({"range":range_value(range),"reversed":reversed})).collect::<Vec<_>>()),"is_fold_collapsed"=>json!(editor.is_fold_collapsed(args["id"].as_str().unwrap_or_default())),"geometry"=>editor.geometry(cx).map_or(Value::Null,|g|json!({"revision":g.revision,"viewport":bounds_value(g.viewport),"horizontal_scroll":f32::from(g.horizontal_scroll),"vertical_scroll":f32::from(g.vertical_scroll),"lines":g.lines.into_iter().map(|line|json!({"line":line.line,"range":range_value(line.range),"bounds":bounds_value(line.bounds)})).collect::<Vec<_>>()})),_=>anyhow::bail!("unsupported Editor query")});
        }
        entity.update(cx, |editor, cx| {
            match method {
                "set_diagnostics" => return Ok(json!(editor.set_diagnostics(args["revision"].as_u64().unwrap_or(0),args["diagnostics"].as_array().into_iter().flatten().map(|v|gpui_kit::controls::editor::EditorDiagnostic{id:v["id"].as_str().unwrap_or_default().to_owned().into(),range:range(&v["range"]),message:v["message"].as_str().unwrap_or_default().to_owned().into(),severity:match v["severity"].as_str(){Some("warning")=>gpui_kit::controls::editor::EditorDiagnosticSeverity::Warning,Some("information")=>gpui_kit::controls::editor::EditorDiagnosticSeverity::Information,Some("hint")=>gpui_kit::controls::editor::EditorDiagnosticSeverity::Hint,_=>gpui_kit::controls::editor::EditorDiagnosticSeverity::Error}}).collect(),cx))),
                "set_semantic_tokens" => return Ok(json!(editor.set_semantic_tokens(args["revision"].as_u64().unwrap_or(0),args["tokens"].as_array().into_iter().flatten().map(|v|gpui_kit::controls::editor::EditorSemanticToken{range:range(&v["range"]),class:match v["class"].as_str(){Some("keyword")=>gpui_kit_theme::SyntaxColor::Keyword,Some("stringLiteral")=>gpui_kit_theme::SyntaxColor::StringLiteral,Some("comment")=>gpui_kit_theme::SyntaxColor::Comment,Some("number")=>gpui_kit_theme::SyntaxColor::Number,Some("inlineWash")=>gpui_kit_theme::SyntaxColor::InlineWash,Some("added")=>gpui_kit_theme::SyntaxColor::Added,Some("addedWash")=>gpui_kit_theme::SyntaxColor::AddedWash,Some("removed")=>gpui_kit_theme::SyntaxColor::Removed,Some("removedWash")=>gpui_kit_theme::SyntaxColor::RemovedWash,_=>gpui_kit_theme::SyntaxColor::Inline}}).collect(),cx))),
                "set_language_services"=>editor.set_language_services(args["enabled"].as_bool().unwrap_or(false),cx),
                "request_service"=>return Ok(editor.request_service(service_kind(&args["kind"]),args["position"].as_u64().unwrap_or(0) as usize,cx).as_ref().map_or(Value::Null,request_value)),
                "set_service_result"=>return Ok(json!(editor.set_service_result(args["request"].as_u64().unwrap_or(0),service_result(&args["result"]),cx))),
                "dismiss_service"=>editor.dismiss_service(cx),
                "accept_service_item"=>return Ok(json!(editor.accept_service_item(args["id"].as_str().unwrap_or_default(),cx))),
                "set_value" => {
                    editor.set_value(args["value"].as_str().unwrap_or_default().to_owned(), cx)
                }
                "set_label" => {
                    editor.set_label(args["label"].as_str().unwrap_or_default().to_owned(), cx)
                }
                "set_rows" => editor.set_rows(args["rows"].as_u64().unwrap_or(1) as usize, cx),
                "set_line_numbers" => {
                    editor.set_line_numbers(args["visible"].as_bool().unwrap_or(false), cx)
                }
                "set_read_only" => {
                    editor.set_read_only(args["read_only"].as_bool().unwrap_or(false), cx)
                }
                "set_disabled" => {
                    editor.set_disabled(args["disabled"].as_bool().unwrap_or(false), cx)
                }
                "set_folds" => {
                    return Ok(json!(
                        editor.set_folds(
                            args["revision"].as_u64().unwrap_or(0),
                            args["folds"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .map(|v| EditorFold {
                                    id: v["id"].as_str().unwrap_or_default().to_owned().into(),
                                    lines: range(&v["lines"])
                                })
                                .collect(),
                            cx
                        )
                    ));
                }
                "set_fold_collapsed" => {
                    return Ok(json!(editor.set_fold_collapsed(
                        args["id"].as_str().unwrap_or_default(),
                        args["collapsed"].as_bool().unwrap_or(false),
                        cx
                    )));
                }
                "set_selections" => {
                    return Ok(json!(
                        editor.set_selections(
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
                "apply_edits" => {
                    return Ok(json!(editor.apply_edits(
                        args["revision"].as_u64().unwrap_or(0),
                        args["edits"].as_array().into_iter().flatten().map(|v| (
                            range(&v["range"]),
                            v["text"].as_str().unwrap_or_default().to_owned().into()
                        )),
                        cx
                    )));
                }
                _ => anyhow::bail!("unsupported Editor command"),
            }
            Ok(Value::Null)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use gpui_kit_testkit::harness::Harness;

    #[gpui::test]
    fn editor_revision_services_and_retained_native_child(cx: &mut TestAppContext) {
        let owner = gpui::EffectOwner::new();
        let state = Rc::new(State::default());
        let node = Rc::new(RefCell::new(serde_json::from_value::<Node>(json!({"kind":"kit","component":"Editor","id":"editor","props":{"value":"AλZ\nsecond\nlast","label":"Fixture","languageServices":true,"rows":4},"events":{}})).expect("fixture")));
        let (build_state, build_node) = (state.clone(), node.clone());
        let mut harness = Harness::new(
            cx,
            move |cx| {
                gpui_kit::install(cx);
                gpui_kit::foundation::register_owner_state(owner, cx);
            },
            move |window, cx| {
                let element = cx.with_effect_owner(Some(owner), |cx| {
                    build_state.render_editor(&build_node.borrow(), window, cx, Rc::new(|_, _| {}))
                });
                gpui::effect_owner(owner, element).into_any_element()
            },
        );
        let child = harness.update(|window,cx|{
            let invoke=|method,args,query,window:&mut Window,cx:&mut App|state.invoke_editor(&node.borrow(),method,&args,query,window,cx).expect("native method");
            let snapshot=invoke("snapshot",json!({}),true,window,cx);
            assert_eq!(snapshot["text"],"AλZ\nsecond\nlast");
            let revision=snapshot["revision"].as_u64().expect("revision");
            assert_eq!(invoke("set_folds",json!({"revision":revision,"folds":[{"id":"tail","lines":{"start":1,"end":3}}]}),false,window,cx),true);
            assert_eq!(invoke("set_fold_collapsed",json!({"id":"tail","collapsed":true}),false,window,cx),true);
            let request=invoke("request_service",json!({"kind":"completion","position":3}),false,window,cx);
            assert_eq!(request["document"],snapshot["text"]);
            let response=json!({"request":request["id"],"result":{"state":"ready","value":{"kind":"items","items":[{"id":"replace","label":"Replace lambda","effect":{"kind":"edits","edits":[{"range":{"start":1,"end":3},"text":"BETA"}]}}]}}});
            assert_eq!(invoke("set_service_result",response.clone(),false,window,cx),true);
            assert_eq!(invoke("accept_service_item",json!({"id":"replace"}),false,window,cx),true);
            assert_eq!(invoke("snapshot",json!({}),true,window,cx)["text"],"ABETAZ\nsecond\nlast");
            assert_eq!(invoke("set_service_result",response,false,window,cx),false);
            assert_eq!(invoke("apply_edits",json!({"revision":revision,"edits":[{"range":{"start":0,"end":1},"text":"stale"}]}),false,window,cx),false);
            state.editors.borrow()[&(0,"editor".into())].entity.read(cx).text_area().clone()
        });
        node.borrow_mut().props.insert("rows".into(), json!(2));
        node.borrow_mut()
            .props
            .insert("lineNumbers".into(), json!(false));
        harness.update(|_, cx| cx.refresh_windows());
        harness.update(|window, cx| {
            let entity = state.editors.borrow()[&(0, "editor".into())].entity.clone();
            assert_eq!(&child, entity.read(cx).text_area());
            assert_eq!(
                entity.read(cx).snapshot(cx).text.as_ref(),
                "ABETAZ\nsecond\nlast"
            );
            state
                .invoke_editor(
                    &node.borrow(),
                    "set_disabled",
                    &json!({"disabled":true}),
                    false,
                    window,
                    cx,
                )
                .expect("disable");
            assert!(
                state
                    .invoke_editor(
                        &node.borrow(),
                        "set_value",
                        &json!({"value":"bad"}),
                        false,
                        window,
                        cx
                    )
                    .is_err()
            );
            assert_eq!(
                state
                    .invoke_editor(&node.borrow(), "snapshot", &json!({}), true, window, cx)
                    .expect("disabled readable")["text"],
                "ABETAZ\nsecond\nlast"
            );
        });
    }
}
