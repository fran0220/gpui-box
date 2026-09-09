use super::*;
use gpui_kit::controls::dropzone::{Dropzone, DropzoneState};
use gpui_kit::controls::upload_list::{OverallProgress, Upload, UploadList, UploadState};

pub(super) fn dropzone(node: &Node, emit: Emit) -> Dropzone {
    let mut zone = Dropzone::new(node.id.clone(), text(node, "label"))
        .disabled(flag(node, "disabled"))
        .invalid(flag(node, "invalid"));
    if node.props.contains_key("accepts") {
        zone = zone.accepts(strings(node.props.get("accepts")));
    }
    if node.props.contains_key("hint") {
        zone = zone.hint(text(node, "hint"));
    }
    // External file reads are not authorized. This reason remains truthful even
    // if an OS drag supplies a payload with the native file kind.
    zone = zone.refusal(
        node.props
            .get("refusal")
            .and_then(Value::as_str)
            .unwrap_or("External file access is not authorized by this host")
            .to_owned(),
    );
    if let Some(icon) = node.props.get("icon") {
        zone = zone.icon(super::super::icon::resolve(icon).expect("validated builtin icon"));
    }
    if let Some(state) = node.props.get("state").and_then(Value::as_str) {
        zone = zone.state(match state {
            "accepting" => DropzoneState::Accepting,
            "refusing" => DropzoneState::Refusing,
            _ => DropzoneState::Idle,
        });
    }
    if !flag(node, "disabled") {
        if let Some(action) = node.events.get("drop").cloned() {
            let emit = emit.clone();
            zone = zone
                .on_drop(move |item, _, _| emit(&action, super::super::drag_item_payload(item)));
        }
        if let Some(action) = node.events.get("filesRefused").cloned() {
            zone=zone.on_files(move|_,_,_|emit(&action,json!({"state":"unavailable","reason":"External file access is not authorized by this host"})));
        }
    }
    zone
}
fn uploads(node: &Node) -> Vec<Upload> {
    node.props
        .get("uploads")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|v| {
            let mut upload = Upload::new(
                v["id"].as_str().unwrap_or_default().to_owned(),
                v["name"].as_str().unwrap_or_default().to_owned(),
            );
            if let Some(size) = v["size"].as_str() {
                upload = upload.size(size.to_owned());
            }
            upload.state(match v["state"]["state"].as_str() {
                Some("uploading") => UploadState::Uploading {
                    fraction: v["state"]["fraction"].as_f64().map(|v| v as f32),
                },
                Some("done") => UploadState::Done,
                Some("cancelled") => UploadState::Cancelled,
                Some("failed") => UploadState::Failed {
                    reason: v["state"]["reason"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned()
                        .into(),
                },
                Some("refused") => UploadState::Refused {
                    reason: v["state"]["reason"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned()
                        .into(),
                },
                _ => UploadState::Queued,
            })
        })
        .collect()
}
pub(super) fn list(node: &Node, slots: KitSlots, emit: Emit) -> UploadList {
    let mut list = UploadList::new(node.id.clone())
        .uploads(uploads(node))
        .disabled(flag(node, "disabled"))
        .control_size(size(node))
        .show_overall(
            node.props
                .get("showOverall")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        );
    if let Some(slot) = slots.get("empty").cloned() {
        list = list.slot("empty", move |window, cx| slot(window, cx));
    }
    if !flag(node, "disabled") {
        if let Some(action) = node.events.get("retry").cloned() {
            let emit = emit.clone();
            list = list.on_retry(move |id, _, _| emit(&action, json!(id.as_ref())));
        }
        if let Some(action) = node.events.get("cancel").cloned() {
            let emit = emit.clone();
            list = list.on_cancel(move |id, _, _| emit(&action, json!(id.as_ref())));
        }
        if let Some(action) = node.events.get("remove").cloned() {
            list = list.on_remove(move |id, _, _| emit(&action, json!(id.as_ref())));
        }
    }
    list
}
pub(in crate::kit_bindings) fn constructed(
    node: &Node,
    context: crate::construction::NativeBuildContext,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
) -> anyhow::Result<UploadList> {
    let zones = context.typed.build(
        "dropzone",
        "Dropzone",
        window,
        cx,
        |child, _, _, emit, _, _| {
            Ok(dropzone(child, emit).disabled(flag(node, "disabled") || flag(child, "disabled")))
        },
    )?;
    anyhow::ensure!(zones.len() <= 1, "UploadList accepts one native Dropzone");
    let mut list = list(node, context.slots, emit);
    if let Some(zone) = zones.into_iter().next() {
        list = list.dropzone(zone);
    }
    Ok(list)
}
pub(super) fn overall(node: &Node) -> Value {
    match UploadList::new(node.id.clone())
        .uploads(uploads(node))
        .overall()
    {
        OverallProgress::Known(fraction) => json!({"state":"known","fraction":fraction}),
        OverallProgress::Indeterminate => json!({"state":"indeterminate"}),
        OverallProgress::Settled => json!({"state":"settled"}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Styled, TestAppContext};
    use gpui_kit_testkit::harness::Harness;
    fn node(component: &str, props: Value, events: Value) -> Node {
        serde_json::from_value(
            json!({"kind":"kit","component":component,"id":"target","props":props,"events":events}),
        )
        .expect("fixture")
    }
    #[gpui::test]
    fn native_internal_drop_and_disabled_refusal(cx: &mut TestAppContext) {
        let owner = gpui::EffectOwner::new();
        let descriptor = Rc::new(RefCell::new(node(
            "Dropzone",
            json!({"label":"Drop rows","accepts":["row"]}),
            json!({"drop":"dropped"}),
        )));
        let events = Rc::new(RefCell::new(Vec::new()));
        let (n, e) = (descriptor.clone(), events.clone());
        let mut harness = Harness::new(
            cx,
            move |cx| {
                gpui_kit::install(cx);
                gpui_kit::foundation::register_owner_state(owner, cx);
            },
            move |_, cx| {
                let e = e.clone();
                let element = cx.with_effect_owner(Some(owner), |_| {
                    gpui::div()
                        .w(gpui::px(420.))
                        .column()
                        .child(
                            gpui_kit::data::List::new("queue", 1, |_, _, _| {
                                gpui_kit::data::ListItem::new("gamma", "Row gamma")
                                    .text("Row gamma")
                            })
                            .row_height(32.)
                            .reorderable(true)
                            .on_select(|_, _, _| {})
                            .on_reorder(|_, _, _| {}),
                        )
                        .child(dropzone(
                            &n.borrow(),
                            Rc::new(move |name, value| {
                                e.borrow_mut().push((name.to_owned(), value))
                            }),
                        ))
                });
                gpui::effect_owner(owner, element).into_any_element()
            },
        );
        harness.drag("queue.gamma", "target");
        assert_eq!(events.borrow().len(), 1);
        assert_eq!(events.borrow()[0].0, "dropped");
        assert_eq!(
            events.borrow()[0].1,
            json!({"id":"gamma","source":"queue","label":"Row gamma","kind":"row","icon":null})
        );
        descriptor
            .borrow_mut()
            .props
            .insert("disabled".into(), json!(true));
        harness.update(|_, cx| cx.refresh_windows());
        harness.drag("queue.gamma", "target");
        assert_eq!(events.borrow().len(), 1);
    }
    #[gpui::test]
    fn upload_actions_respect_six_states_and_unknown_extent(cx: &mut TestAppContext) {
        let owner = gpui::EffectOwner::new();
        let descriptor = node(
            "UploadList",
            json!({"uploads":[{"id":"done","name":"Done","state":{"state":"done"}},{"id":"running","name":"Running","state":{"state":"uploading","fraction":null}},{"id":"failed","name":"Failed","state":{"state":"failed","reason":"Fixture failure"}},{"id":"refused","name":"Refused","state":{"state":"refused","reason":"Fixture policy"}}]}),
            json!({"retry":"retry","cancel":"cancel","remove":"remove"}),
        );
        assert_eq!(overall(&descriptor), json!({"state":"indeterminate"}));
        let events = Rc::new(RefCell::new(Vec::new()));
        let e = events.clone();
        let mut harness = Harness::new(
            cx,
            move |cx| {
                gpui_kit::install(cx);
                gpui_kit::foundation::register_owner_state(owner, cx);
            },
            move |_, cx| {
                let e = e.clone();
                let element = cx.with_effect_owner(Some(owner), |_| {
                    list(
                        &descriptor,
                        Default::default(),
                        Rc::new(move |name, value| e.borrow_mut().push((name.to_owned(), value))),
                    )
                });
                gpui::effect_owner(owner, element).into_any_element()
            },
        );
        assert!(harness.node("target.refused.retry").is_none());
        harness.click("target.failed.retry");
        harness.click("target.running.cancel");
        harness.click("target.done.remove");
        assert_eq!(
            *events.borrow(),
            vec![
                ("retry".into(), json!("failed")),
                ("cancel".into(), json!("running")),
                ("remove".into(), json!("done"))
            ]
        );
        let known = node(
            "UploadList",
            json!({"uploads":[{"id":"done","name":"Done","state":{"state":"done"}},{"id":"running","name":"Running","state":{"state":"uploading","fraction":0.2}},{"id":"refused","name":"Refused","state":{"state":"refused","reason":"Policy"}}]}),
            json!({}),
        );
        assert!((overall(&known)["fraction"].as_f64().expect("known") - 0.6).abs() < 0.0001);
    }
}
