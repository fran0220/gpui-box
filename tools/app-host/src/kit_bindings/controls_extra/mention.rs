use super::*;
use gpui_kit::controls::mention::{MentionCandidate, MentionInput, MentionInputEvent};
use gpui_kit::controls::textarea::{TextArea, TextAreaEvent};
use gpui_kit::state::{AsyncStatus, AsyncValue};

fn suggestions(value: &Value) -> AsyncValue<Vec<MentionCandidate>, gpui::SharedString> {
    let items = value.get("value").and_then(Value::as_array).map(|items| {
        items
            .iter()
            .map(|v| {
                let mut candidate = MentionCandidate::new(
                    v["id"].as_str().unwrap_or_default().to_owned(),
                    v["label"].as_str().unwrap_or_default().to_owned(),
                );
                if let Some(s) = v["description"].as_str() {
                    candidate = candidate.description(s.to_owned());
                }
                if let Some(s) = v["replacement"].as_str() {
                    candidate = candidate.replacement(s.to_owned());
                }
                if let Some(s) = v["refusal"].as_str() {
                    candidate = candidate.unavailable(s.to_owned());
                }
                candidate.search_terms(
                    v["searchTerms"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::to_owned),
                )
            })
            .collect()
    });
    AsyncValue {
        value: items,
        status: match value["state"].as_str() {
            Some("loading") => AsyncStatus::Loading,
            Some("refreshing") => AsyncStatus::Refreshing,
            Some("ready") => AsyncStatus::Ready,
            Some("empty") => AsyncStatus::Empty,
            Some("error") => AsyncStatus::Error(
                value["reason"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
                    .into(),
            ),
            Some("unavailable") => {
                AsyncStatus::Unavailable(value["reason"].as_str().unwrap_or_default().to_owned())
            }
            _ => AsyncStatus::Idle,
        },
        attempts: value["attempts"].as_u64().unwrap_or(0) as usize,
    }
}
fn query_value(query: Option<&gpui_kit::controls::mention::MentionQuery>) -> Value {
    query.map_or(
        Value::Null,
        |q| json!({"text":q.text.as_ref(),"range":text_area::range_value(q.range.clone())}),
    )
}
impl State {
    pub(super) fn render_mention(
        &self,
        node: &Node,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.mentions.borrow().get(&key).cloned();
        let entry=existing.unwrap_or_else(||{
            let area=cx.new(|cx|TextArea::new(format!("{}.editor",node.id),window,cx));
            let entity=cx.new(|cx|MentionInput::new(node.id.clone(),area,cx));
            let route=Rc::new(RefCell::new(Route{events:node.events.clone(),emit:emit.clone(),disabled:flag(node,"disabled")}));
            let callback=Rc::downgrade(&route);
            let subscription=cx.subscribe(&entity,move|entity,event:&MentionInputEvent,cx|{
                if entity.read(cx).editor().read(cx).is_disabled(){return;}
                let name=match event{MentionInputEvent::Changed(_)=>"changed",MentionInputEvent::Submitted=>"submitted",MentionInputEvent::Cancelled=>"cancelled",MentionInputEvent::Pasted(_)=>"pasteRefused",MentionInputEvent::Focused=>"focused",MentionInputEvent::Blurred=>"blurred",MentionInputEvent::QueryChanged(_)=>"queryChanged",MentionInputEvent::Accepted{..}=>"accepted"};
                let target=callback.upgrade().and_then(|r|{let r=r.borrow();(!r.disabled).then(||r.events.get(name).map(|a|(a.clone(),r.emit.clone()))).flatten()});
                if let Some((action,emit))=target{let value=match event{MentionInputEvent::Changed(text)=>json!(text.as_ref()),MentionInputEvent::Pasted(p)=>text_area::event_value(&TextAreaEvent::Pasted(p.clone())),MentionInputEvent::QueryChanged(q)=>query_value(q.as_ref()),MentionInputEvent::Accepted{id,range}=>json!({"id":id.as_ref(),"range":text_area::range_value(range.clone())}),_=>Value::Null};emit(&action,value);}
            });
            let entry=Rc::new(Entry{entity,route,props:Default::default(),_subscription:subscription});self.mentions.borrow_mut().insert(key,entry.clone());entry
        });
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if *entry.props.borrow() != node.props {
            let area = entry.entity.read(cx).editor().clone();
            area.update(cx, |area, cx| {
                area.set_placeholder(text(node, "placeholder"), cx);
                area.set_rows(
                    node.props.get("rows").and_then(Value::as_u64).unwrap_or(3) as usize,
                    cx,
                );
                area.set_read_only(flag(node, "readOnly"), cx);
                area.set_disabled(flag(node, "disabled"), cx);
                if entry.props.borrow().get("value") != node.props.get("value")
                    && node.props.contains_key("value")
                {
                    area.set_value(text(node, "value"), cx);
                }
            });
            if entry.props.borrow().get("suggestions") != node.props.get("suggestions") {
                entry.entity.update(cx, |input, cx| {
                    input.set_suggestions(
                        suggestions(node.props.get("suggestions").unwrap_or(&Value::Null)),
                        cx,
                    )
                });
            }
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }
    pub(super) fn mention_reference_query(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        cx: &App,
        refs: &crate::references::Registration<'_>,
    ) -> Option<anyhow::Result<Value>> {
        if method != "editor" {
            return None;
        }
        Some((|| {
            super::super::validation::invocation("MentionInput", method, args, true)?;
            let entity = self
                .mentions
                .borrow()
                .get(&(node.instance, node.id.clone()))
                .map(|e| e.entity.clone())
                .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
            refs.entity(
                "TextArea",
                &entity,
                entity.read(cx).editor(),
                |input, _| Some(input.editor().clone()),
                |input, cx| !input.editor().read(cx).is_disabled(),
                text_area::text_area_dispatch,
            )
        })())
    }
    pub(super) fn invoke_mention(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        let entity = self
            .mentions
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .map(|e| e.entity.clone())
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        anyhow::ensure!(
            query || (!flag(node, "disabled") && !entity.read(cx).editor().read(cx).is_disabled()),
            "disabled mention input refuses invocation"
        );
        if query {
            return Ok(match method {
                "active_query" => query_value(entity.read(cx).active_query()),
                "is_open" => json!(entity.read(cx).is_open()),
                _ => anyhow::bail!("unsupported MentionInput query"),
            });
        }
        match method {
            "set_suggestions" => entity.update(cx, |input, cx| {
                input.set_suggestions(suggestions(&args["suggestions"]), cx)
            }),
            _ => anyhow::bail!("unsupported MentionInput command"),
        };
        Ok(Value::Null)
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use gpui_kit_testkit::harness::Harness;
    #[gpui::test]
    fn mention_retains_editor_and_accepts_only_available_candidate(cx: &mut TestAppContext) {
        let owner = gpui::EffectOwner::new();
        let state = Rc::new(State::default());
        let node=Rc::new(RefCell::new(serde_json::from_value::<Node>(json!({"kind":"kit","component":"MentionInput","id":"mention","props":{"suggestions":{"state":"ready","value":[{"id":"denied","label":"Alpha unavailable","refusal":"Policy"},{"id":"ok","label":"Alpha usable","replacement":"@accepted"}]}},"events":{"accepted":"accepted"}})).expect("fixture")));
        let events = Rc::new(RefCell::new(Vec::new()));
        let (s, n, e) = (state.clone(), node.clone(), events.clone());
        let mut harness = Harness::new(
            cx,
            move |cx| {
                gpui_kit::install(cx);
                gpui_kit::foundation::register_owner_state(owner, cx);
            },
            move |window, cx| {
                let e = e.clone();
                let element = cx.with_effect_owner(Some(owner), |cx| {
                    s.render_mention(
                        &n.borrow(),
                        window,
                        cx,
                        Rc::new(move |name, value| e.borrow_mut().push((name.to_owned(), value))),
                    )
                });
                gpui::effect_owner(owner, element).into_any_element()
            },
        );
        harness.click("mention.editor");
        harness.keystrokes("@ A l");
        let child = harness.update(|_, cx| {
            let entity = state.mentions.borrow()[&(0, "mention".into())]
                .entity
                .clone();
            assert!(entity.read(cx).is_open());
            assert_eq!(
                entity.read(cx).active_query().expect("query").text.as_ref(),
                "Al"
            );
            entity.read(cx).editor().clone()
        });
        node.borrow_mut()
            .props
            .insert("placeholder".into(), json!("Retained"));
        harness.update(|_, cx| cx.refresh_windows());
        harness.keystrokes("enter");
        harness.update(|_, cx| {
            let entity = state.mentions.borrow()[&(0, "mention".into())]
                .entity
                .clone();
            assert_eq!(&child, entity.read(cx).editor());
            assert!(child.read(cx).value().as_ref().contains("@accepted"));
        });
        assert!(
            events
                .borrow()
                .iter()
                .any(|(name, value)| name == "accepted" && value["id"] == "ok")
        );
        assert!(
            !events
                .borrow()
                .iter()
                .any(|(_, value)| value["id"] == "denied")
        );
        node.borrow_mut()
            .props
            .insert("disabled".into(), json!(true));
        harness.update(|_, cx| cx.refresh_windows());
        harness.update(|_, cx| {
            assert!(child.read(cx).is_disabled());
            assert!(
                state
                    .invoke_mention(
                        &node.borrow(),
                        "set_suggestions",
                        &json!({"suggestions":{"state":"empty"}}),
                        false,
                        cx
                    )
                    .is_err()
            );
            assert_eq!(
                state
                    .invoke_mention(&node.borrow(), "active_query", &json!({}), true, cx)
                    .expect("disabled query"),
                json!({"text":"accepted","range":{"start":0,"end":9}})
            );
        });
    }
}
