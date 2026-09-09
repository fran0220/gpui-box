//! Caller-owned agent evidence. Rendering never runs a tool or resolves a request.
use super::{Emit, KitSlots, Node, flag, text};
use gpui::{
    AnyElement, App, AppContext as _, Entity, IntoElement, SharedString, StyledImage as _,
    Subscription, Window,
};
use gpui_kit::{agent::*, foundation::slot::Slotted, prelude::*};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub(super) const COMPONENTS: &[&str] = &[
    "AgentPlan",
    "ThinkingBlock",
    "FeedbackRating",
    "PromptBuilder",
    "CostMeter",
    "ContextGauge",
    "AgentAvatar",
    "AgentActivityLine",
    "AgentCard",
    "AgentGroup",
    "AgentRoster",
    "SubagentTree",
    "AgentRunIssues",
    "ApprovalPrompt",
    "ClarificationPanel",
    "ArtifactPreview",
    "ToolCall",
    "ServerList",
    "OfferingCatalog",
    "PermissionMatrix",
    "VoiceReactive",
    "PersonaPortrait",
    "PersonaDialogue",
    "AgentRunCanvas",
];

enum Control {
    Approval(Entity<ApprovalPrompt>),
    Clarification(Entity<ClarificationPanel>),
}
struct Retained {
    control: Control,
    props: RefCell<serde_json::Map<String, Value>>,
    route: Rc<RefCell<(std::collections::BTreeMap<String, String>, Emit)>>,
    _subscription: Subscription,
}
#[derive(Default)]
pub(super) struct State {
    retained: RefCell<HashMap<(u64, String), Rc<Retained>>>,
}

fn scope(value: &Value) -> AlwaysScope {
    match value["kind"].as_str() {
        Some("tool") => AlwaysScope::tool(string(value, "subject")),
        Some("path") => AlwaysScope::path(string(value, "subject")),
        Some("host") => AlwaysScope::host(string(value, "subject")),
        _ => AlwaysScope::Session,
    }
}
fn decision(value: &Value) -> ApprovalDecision {
    if value["kind"] == "always" {
        ApprovalDecision::Always(scope(&value["scope"]))
    } else {
        ApprovalDecision::Once
    }
}
fn decision_value(value: &ApprovalDecision) -> Value {
    match value {
        ApprovalDecision::Once => json!({"kind":"once"}),
        ApprovalDecision::Always(scope) => {
            let mut value = json!({"kind":scope.name()});
            if let Some(subject) = scope.subject() {
                value["subject"] = json!(subject.as_ref());
            }
            json!({"kind":"always","scope":value})
        }
    }
}
fn approval_status(value: &Value) -> ApprovalStatus {
    match value["kind"].as_str() {
        Some("declined") => ApprovalStatus::Declined,
        Some("expired") => ApprovalStatus::Expired,
        Some("approved") => ApprovalStatus::Approved(decision(&value["decision"])),
        Some("superseded") => ApprovalStatus::Superseded {
            by: string(value, "by").into(),
        },
        _ => ApprovalStatus::Pending,
    }
}
fn approval_value(status: &ApprovalStatus) -> Value {
    let mut value = json!({"kind":status.name()});
    match status {
        ApprovalStatus::Approved(decision) => value["decision"] = decision_value(decision),
        ApprovalStatus::Superseded { by } => value["by"] = json!(by.as_ref()),
        _ => {}
    }
    value
}
fn clarification_status(value: &Value) -> ClarificationStatus {
    match value["kind"].as_str() {
        Some("answered") => ClarificationStatus::Answered(decode(&value["ids"])),
        Some("skipped") => ClarificationStatus::Skipped,
        Some("withdrawn") => ClarificationStatus::Withdrawn(string(value, "reason").into()),
        Some("superseded") => ClarificationStatus::Superseded {
            by: string(value, "by").into(),
        },
        _ => ClarificationStatus::Pending,
    }
}
fn clarification_value(status: &ClarificationStatus) -> Value {
    let mut value = json!({"kind":status.name()});
    match status {
        ClarificationStatus::Answered(ids) => value["ids"] = json!(ids),
        ClarificationStatus::Withdrawn(reason) => value["reason"] = json!(reason.as_ref()),
        ClarificationStatus::Superseded { by } => value["by"] = json!(by.as_ref()),
        _ => {}
    }
    value
}
fn option(value: &Value) -> ClarificationOption {
    let mut option = ClarificationOption::new(string(value, "id"), string(value, "label"));
    if value.get("detail").is_some() {
        option = option.detail(string(value, "detail"));
    }
    if value.get("unavailable").is_some() {
        option = option.unavailable(string(value, "unavailable"));
    }
    option
}

fn tool_body(value: &Value) -> ToolBody {
    let body = ToolBody::new(string(value, "text"));
    match value.get("maxLines") {
        Some(Value::Null) => body.all_lines(),
        Some(value) => body.max_lines(value.as_u64().expect("validated maxLines") as usize),
        None => body,
    }
}
fn offering_kind(value: &str) -> OfferingKind {
    match value {
        "tool" => OfferingKind::Tool,
        "skill" => OfferingKind::Skill,
        _ => OfferingKind::Resource,
    }
}
fn offering(value: &Value) -> Offering {
    let mut offering = Offering::new(
        string(value, "id"),
        offering_kind(value["kind"].as_str().expect("validated offering kind")),
        string(value, "name"),
    );
    if value.get("summary").is_some() {
        offering = offering.summary(string(value, "summary"));
    }
    if value.get("qualifier").is_some() {
        offering = offering.qualifier(string(value, "qualifier"));
    }
    offering
}
fn permission(value: &str) -> PermissionState {
    match value {
        "allowed" => PermissionState::Allowed,
        "denied" => PermissionState::Denied,
        "ask" => PermissionState::Ask,
        _ => PermissionState::NotApplicable,
    }
}
fn voice(value: &Value) -> VoiceSample {
    let state = match value["state"]["kind"].as_str() {
        Some("listening") => VoiceState::Listening,
        Some("speaking") => VoiceState::Speaking,
        Some("unavailable") => VoiceState::Unavailable(string(&value["state"], "reason").into()),
        _ => VoiceState::Silent,
    };
    VoiceSample::new(
        state,
        value["level"].as_f64().expect("validated voice level") as f32,
        value["envelope"]
            .as_f64()
            .expect("validated voice envelope") as f32,
    )
    .expect("bounded voice sample")
}
fn viewport(value: &Value) -> gpui_kit::canvas::GraphViewport {
    gpui_kit::canvas::GraphViewport::new(
        gpui::point(
            value["x"].as_f64().expect("validated viewport x") as f32,
            value["y"].as_f64().expect("validated viewport y") as f32,
        ),
        value["zoom"].as_f64().expect("validated viewport zoom") as f32,
    )
}

impl State {
    pub(super) fn reconcile(&self, root: &Node, _cx: &mut App) {
        fn visit(node: &Node, live: &mut HashMap<(u64, String), String>) {
            if let Some(component) = &node.component {
                live.insert((node.instance, node.id.clone()), component.clone());
            }
            for child in node.children.iter().chain(node.slots.values().flatten()) {
                visit(child, live);
            }
        }
        let mut live = HashMap::new();
        visit(root, &mut live);
        self.retained.borrow_mut().retain(|key, entry| {
            matches!(
                (live.get(key).map(String::as_str), &entry.control),
                (Some("ApprovalPrompt"), Control::Approval(_))
                    | (Some("ClarificationPanel"), Control::Clarification(_))
            )
        });
    }

    pub(super) fn render(
        &self,
        node: &Node,
        slots: KitSlots,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let component = node.component.as_deref().unwrap_or_default();
        if component != "ApprovalPrompt" && component != "ClarificationPanel" {
            return render(node, slots, window, cx, emit);
        }
        let key = (node.instance, node.id.clone());
        let mut held = self.retained.borrow().get(&key).cloned();
        // Native constructor options are immutable. Replacing the question or offered
        // scopes is a new visual request, whereas status updates retain focus/choices.
        if held.as_ref().is_some_and(|entry| {
            let mut old = entry.props.borrow().clone();
            let mut new = node.props.clone();
            old.remove("status");
            new.remove("status");
            old != new
                || !matches!(
                    (&entry.control, component),
                    (Control::Approval(_), "ApprovalPrompt")
                        | (Control::Clarification(_), "ClarificationPanel")
                )
        }) {
            held = None;
        }
        let entry = held.unwrap_or_else(|| {
            let route = Rc::new(RefCell::new((node.events.clone(), emit.clone())));
            let callback = route.clone();
            let send = move |name: &str, value: Value| {
                let (action, emit) = {
                    let route = callback.borrow();
                    (route.0.get(name).cloned(), route.1.clone())
                };
                if let Some(action) = action {
                    emit(&action, value);
                }
            };
            let (control, subscription) = if component == "ApprovalPrompt" {
                let entity = cx.new(|cx| {
                    let mut control = ApprovalPrompt::new(
                        SharedString::from(node.id.clone()),
                        text(node, "action"),
                        window,
                        cx,
                    )
                    .status(approval_status(&node.props["status"]))
                    .details(items(node, "details").map(|item| {
                        DescriptionItem::new(
                            string(item, "id"),
                            string(item, "term"),
                            string(item, "value"),
                        )
                    }));
                    for value in items(node, "always") {
                        control = control.always(scope(value));
                    }
                    control
                });
                let subscription =
                    cx.subscribe(&entity, move |_, event: &ApprovalEvent, _| match event {
                        ApprovalEvent::Approved(value) => send("approve", decision_value(value)),
                        ApprovalEvent::Declined => send("decline", Value::Null),
                    });
                (Control::Approval(entity), subscription)
            } else {
                let entity = cx.new(|cx| {
                    let mut control = ClarificationPanel::new(
                        SharedString::from(node.id.clone()),
                        text(node, "question"),
                        window,
                        cx,
                    )
                    .options(items(node, "options").map(option))
                    .status(clarification_status(&node.props["status"]));
                    if flag(node, "multiple") {
                        control = control.multiple();
                    }
                    if flag(node, "skippable") {
                        control = control.skippable();
                    }
                    control
                });
                let subscription = cx.subscribe(
                    &entity,
                    move |_, event: &ClarificationEvent, _| match event {
                        ClarificationEvent::Answered(ids) => send("answer", json!(ids)),
                        ClarificationEvent::Skipped => send("skip", Value::Null),
                    },
                );
                (Control::Clarification(entity), subscription)
            };
            let entry = Rc::new(Retained {
                control,
                props: RefCell::new(node.props.clone()),
                route,
                _subscription: subscription,
            });
            self.retained.borrow_mut().insert(key, entry.clone());
            entry
        });
        *entry.route.borrow_mut() = (node.events.clone(), emit);
        let changed = entry.props.borrow().get("status") != node.props.get("status");
        if changed {
            match &entry.control {
                Control::Approval(entity) => entity.update(cx, |control, cx| {
                    control.set_status(approval_status(&node.props["status"]), cx)
                }),
                Control::Clarification(entity) => entity.update(cx, |control, cx| {
                    control.set_status(clarification_status(&node.props["status"]), cx)
                }),
            }
        }
        *entry.props.borrow_mut() = node.props.clone();
        match &entry.control {
            Control::Approval(entity) => entity.clone().into_any_element(),
            Control::Clarification(entity) => entity.clone().into_any_element(),
        }
    }

    pub(super) fn invoke(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        _window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        if matches!(
            node.component.as_deref(),
            Some("AgentPlan" | "ContextGauge")
        ) {
            return invoke(node, method, args, query);
        }
        let entry = self
            .retained
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("native agent target not mounted"))?;
        match &entry.control {
            Control::Approval(entity) => {
                anyhow::ensure!(
                    node.component.as_deref() == Some("ApprovalPrompt"),
                    "native target type mismatch"
                );
                if query {
                    anyhow::ensure!(method == "current_status", "unknown approval query");
                    return Ok(approval_value(entity.read(cx).current_status()));
                }
                if method != "set_status" {
                    anyhow::ensure!(
                        matches!(entity.read(cx).current_status(), ApprovalStatus::Pending),
                        "resolved approval refuses command"
                    );
                }
                entity.update(cx, |control, cx| -> anyhow::Result<()> {
                    match method {
                        "set_status" => control.set_status(approval_status(&args["status"]), cx),
                        "approve" => {
                            let value = decision(&args["decision"]);
                            if let ApprovalDecision::Always(ref requested) = value {
                                anyhow::ensure!(
                                    entry
                                        .props
                                        .borrow()
                                        .get("always")
                                        .and_then(Value::as_array)
                                        .is_some_and(|scopes| scopes
                                            .iter()
                                            .any(|value| &scope(value) == requested)),
                                    "unoffered standing approval scope"
                                );
                            }
                            control.approve(value, cx);
                        }
                        "decline" => control.decline(cx),
                        _ => anyhow::bail!("unknown approval command"),
                    }
                    Ok(())
                })?;
            }
            Control::Clarification(entity) => {
                anyhow::ensure!(
                    node.component.as_deref() == Some("ClarificationPanel"),
                    "native target type mismatch"
                );
                if query {
                    let control = entity.read(cx);
                    return Ok(match method {
                        "current_status" => clarification_value(control.current_status()),
                        "chosen" => json!(control.chosen()),
                        "candidates" => json!(control.candidates().iter().map(|option| {
                            let mut value = json!({"id":option.id().as_ref(),"label":option.label().as_ref()});
                            if let Some(detail) = option.detail_text() { value["detail"] = json!(detail.as_ref()); }
                            if let Some(reason) = option.unavailable_reason() { value["unavailable"] = json!(reason.as_ref()); }
                            value
                        }).collect::<Vec<_>>()),
                        _ => anyhow::bail!("unknown clarification query"),
                    });
                }
                if method != "set_status" {
                    anyhow::ensure!(
                        entity.read(cx).current_status().is_pending(),
                        "resolved clarification refuses command"
                    );
                }
                entity.update(cx, |control, cx| -> anyhow::Result<()> {
                    match method {
                        "set_status" => {
                            control.set_status(clarification_status(&args["status"]), cx)
                        }
                        "choose" => {
                            let id = string(args, "id");
                            anyhow::ensure!(
                                control
                                    .candidates()
                                    .iter()
                                    .any(|option| option.id().as_ref() == id
                                        && option.is_available()),
                                "unavailable candidate"
                            );
                            control.choose(id, cx);
                        }
                        "answer" => {
                            anyhow::ensure!(!control.chosen().is_empty(), "empty answer refused");
                            control.answer(cx);
                        }
                        "skip" => {
                            anyhow::ensure!(
                                entry.props.borrow().get("skippable") == Some(&Value::Bool(true)),
                                "skip not offered"
                            );
                            control.skip(cx);
                        }
                        _ => anyhow::bail!("unknown clarification command"),
                    }
                    Ok(())
                })?;
            }
        }
        Ok(Value::Null)
    }
}

pub(super) fn decode<T: serde::de::DeserializeOwned>(value: &Value) -> T {
    serde_json::from_value(value.clone()).expect("closed native model schema")
}

pub(super) fn tint(value: &Value) -> gpui::Hsla {
    gpui::hsla(
        value["h"].as_f64().expect("validated hue") as f32,
        value["s"].as_f64().expect("validated saturation") as f32,
        value["l"].as_f64().expect("validated lightness") as f32,
        value["a"].as_f64().expect("validated alpha") as f32,
    )
}

pub(super) fn image(value: &Value, cx: &App) -> gpui::ImageSource {
    let result = serde_json::from_value::<crate::resources::ResourceRef>(value.clone())
        .map_err(anyhow::Error::from)
        .and_then(|reference| crate::resources::Resources::image(&reference, cx));
    result.unwrap_or_else(|error| {
        let error = std::sync::Arc::new(error);
        gpui::ImageSource::Custom(std::sync::Arc::new(move |_, _| {
            Some(Err(gpui::ImageCacheError::Other(error.clone())))
        }))
    })
}

pub(super) fn validate(node: &Node) -> anyhow::Result<()> {
    fn visit(value: &Value) -> anyhow::Result<()> {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    if key == "image" || key == "resource" {
                        let reference: crate::resources::ResourceRef =
                            serde_json::from_value(value.clone())?;
                        anyhow::ensure!(
                            !reference.key.is_empty()
                                && reference.key.len() <= 128
                                && reference
                                    .key
                                    .bytes()
                                    .all(|byte| byte.is_ascii_alphanumeric()
                                        || byte == b'-'
                                        || byte == b'_'),
                            "invalid image resource key"
                        );
                    } else {
                        visit(value)?;
                    }
                }
            }
            Value::Array(values) => {
                for value in values {
                    visit(value)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    visit(&Value::Object(node.props.clone()))
}

fn appearances(node: &Node, cx: &App) -> Vec<AgentAppearance> {
    items(node, "appearances")
        .map(|item| {
            let mut appearance = AgentAppearance::new(string(item, "id"));
            if let Some(value) = item.get("image") {
                appearance = appearance.image_source(image(value, cx));
            }
            if item.get("tint").is_some() {
                appearance = appearance.tint(tint(&item["tint"]));
            }
            appearance
        })
        .collect()
}

fn action_value(action: AgentUiAction) -> Value {
    match action {
        AgentUiAction::SelectAgent(id) => json!({"kind":"select-agent", "id":id.as_str()}),
        AgentUiAction::OpenTask(id) => json!({"kind":"open-task", "id":id.as_str()}),
        AgentUiAction::RequestCancel(subject) => {
            json!({"kind":"request-cancel", "subject":subject})
        }
        AgentUiAction::RequestRetry(subject) => json!({"kind":"request-retry", "subject":subject}),
        AgentUiAction::FocusResult(subject) => json!({"kind":"focus-result", "subject":subject}),
    }
}

fn string(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or_default().to_owned()
}

fn items<'a>(node: &'a Node, key: &str) -> impl Iterator<Item = &'a Value> {
    node.props
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn event(node: &Node, name: &str) -> Option<String> {
    (!flag(node, "disabled"))
        .then(|| node.events.get(name).cloned())
        .flatten()
}

fn reading(value: &Value) -> Reading {
    let amount = value["amount"].as_f64().unwrap_or_default();
    match value["basis"].as_str() {
        Some("measured") => Reading::measured(amount, string(value, "text")),
        Some("estimated") => Reading::estimated(amount, string(value, "text")),
        _ => Reading::Unavailable {
            reason: value["reason"].as_str().map(|s| s.to_owned().into()),
        },
    }
}

fn plan(node: &Node) -> AgentPlan {
    AgentPlan::new(SharedString::from(node.id.clone())).items(items(node, "items").map(|item| {
        PlanItem::new(string(item, "id"), string(item, "label")).state(
            match item["state"].as_str() {
                Some("doing") => PlanState::Doing,
                Some("done") => PlanState::Done,
                Some("blocked") => PlanState::Blocked(string(item, "reason").into()),
                Some("dropped") => PlanState::Dropped(string(item, "reason").into()),
                _ => PlanState::Ahead,
            },
        )
    }))
}

fn gauge(node: &Node) -> ContextGauge {
    let mut control = ContextGauge::new(
        SharedString::from(node.id.clone()),
        reading(&node.props["used"]),
    );
    if let Some(value) = node.props.get("limit") {
        control = control.limit(match reading(value) {
            Reading::Known(quantity) => Limit::Known(quantity),
            Reading::Unavailable { .. } => Limit::Unknown,
        });
    }
    if node.props.contains_key("label") {
        control = control.label(text(node, "label"));
    }
    if node.props.contains_key("stale") {
        control = control.stale(LastVerified::at(text(node, "stale")));
    }
    control
}

pub(super) fn invoke(
    node: &Node,
    method: &str,
    _args: &Value,
    query: bool,
) -> anyhow::Result<Value> {
    match (node.component.as_deref(), method, query) {
        (Some("AgentPlan"), "done", true) => Ok(json!(plan(node).done())),
        (Some("ContextGauge"), "fraction", true) => Ok(json!(gauge(node).fraction())),
        _ => anyhow::bail!("unsupported agent method"),
    }
}

pub(super) fn render(
    node: &Node,
    slots: KitSlots,
    _window: &mut Window,
    _cx: &mut App,
    emit: Emit,
) -> AnyElement {
    let id = SharedString::from(node.id.clone());
    match node.component.as_deref().unwrap_or_default() {
        "AgentPlan" => {
            let mut control = plan(node);
            if let Some(action) = event(node, "pick") {
                control = control.on_pick(move |id, _, _| emit(&action, json!(id.as_ref())));
            }
            control.into_any_element()
        }
        "ThinkingBlock" => {
            let value = &node.props["reasoning"];
            let reasoning = match value["kind"].as_str() {
                Some("present") => Reasoning::present(string(value, "text")),
                Some("withheld") => Reasoning::withheld(string(value, "text")),
                _ => Reasoning::Absent,
            };
            let mut control = ThinkingBlock::new(id, reasoning)
                .expanded(flag(node, "expanded"))
                .thinking(flag(node, "thinking"))
                .presentation(if text(node, "presentation") == "flow" {
                    AgentDisclosurePresentation::Flow
                } else {
                    AgentDisclosurePresentation::Inset
                });
            if node.props.contains_key("elapsed") {
                control = control.elapsed(text(node, "elapsed"));
            }
            if let Some(action) = event(node, "toggle") {
                control = control.on_toggle(move |expanded, _, _| emit(&action, json!(expanded)));
            }
            control.into_any_element()
        }
        "FeedbackRating" => {
            let mut control = FeedbackRating::new(id)
                .disabled(flag(node, "disabled"))
                .vote(match text(node, "vote").as_str() {
                    "up" => Some(FeedbackVote::Up),
                    "down" => Some(FeedbackVote::Down),
                    _ => None,
                })
                .tags(items(node, "tags").map(|item| (string(item, "id"), string(item, "label"))));
            if node.props.contains_key("currentTag") {
                control = control.current_tag(text(node, "currentTag"));
            }
            if let Some(action) = event(node, "vote") {
                let emit = emit.clone();
                control = control.on_vote(move |vote, _, _| emit(&action, json!(vote.name())));
            }
            if let Some(action) = event(node, "tag") {
                control = control.on_tag(move |tag, _, _| emit(&action, json!(tag.as_ref())));
            }
            control.into_any_element()
        }
        "PromptBuilder" => {
            let state = &node.props["state"];
            let mut control = PromptBuilder::new(id, text(node, "label"))
                .body(text(node, "body"))
                .disabled(flag(node, "disabled"))
                .slots(items(node, "slots").map(|slot| {
                    PromptSlot::new(string(slot, "id"), string(slot, "name"))
                        .value(string(slot, "value"))
                }))
                .state(match state["kind"].as_str() {
                    Some("loading") => PromptBuilderState::Loading,
                    Some("empty") => PromptBuilderState::Empty,
                    Some("unavailable") => {
                        PromptBuilderState::Unavailable(string(state, "reason").into())
                    }
                    Some("error") => PromptBuilderState::Error(string(state, "reason").into()),
                    _ => PromptBuilderState::Ready,
                });
            if let Some(slot) = slots.get("empty").cloned() {
                control = control.slot("empty", move |window, cx| slot(window, cx));
            }
            if let Some(action) = event(node, "slot") {
                control = control.on_slot(move |slot, _, _| emit(&action, json!({"id":slot.id.as_ref(),"name":slot.name.as_ref(),"value":slot.value.as_ref()})));
            }
            control.into_any_element()
        }
        "CostMeter" => {
            let mut control = CostMeter::new(id).lines(items(node, "lines").map(|line| {
                let mut native = CostLine::new(
                    string(line, "id"),
                    string(line, "label"),
                    reading(&line["reading"]),
                );
                if line["stale"].is_string() {
                    native = native.stale(LastVerified::at(string(line, "stale")));
                }
                native
            }));
            if node.props.contains_key("label") {
                control = control.label(text(node, "label"));
            }
            control.into_any_element()
        }
        "ContextGauge" => gauge(node).into_any_element(),
        "AgentAvatar" => {
            let mut control = AgentAvatar::new(id, decode(&node.props["agent"]));
            if let Some(value) = node.props.get("image") {
                control = control.image_source(image(value, _cx));
            }
            if let Some(value) = node.props.get("tint") {
                control = control.tint(tint(value));
            }
            if node.props.contains_key("size") {
                control = control.size(super::number(node, "size", 32.));
            }
            if node.props.contains_key("parent") {
                control = control.parent(SharedString::from(text(node, "parent")));
            }
            control.into_any_element()
        }
        "AgentActivityLine" => {
            let mut control = AgentActivityLine::new(id, decode(&node.props["execution"]));
            if let Some(value) = node.props.get("tint") {
                control = control.tint(tint(value));
            }
            control.into_any_element()
        }
        "AgentCard" => {
            let mut control =
                AgentCard::new(id, decode(&node.props["agent"])).selected(flag(node, "selected"));
            if let Some(value) = node.props.get("image") {
                control = control.image_source(image(value, _cx));
            }
            if let Some(value) = node.props.get("tint") {
                control = control.tint(tint(value));
            }
            if node.props.contains_key("taskLabel") {
                control = control.task_label(text(node, "taskLabel"));
            }
            if let Some(action) = event(node, "action") {
                control = control.on_action(move |value, _, _| emit(&action, action_value(value)));
            }
            control.into_any_element()
        }
        "AgentGroup" => {
            let mut control =
                AgentGroup::new(id, items(node, "agents").map(decode::<AgentSnapshot>))
                    .appearances(appearances(node, _cx));
            if node.props.contains_key("size") {
                control = control.size(super::number(node, "size", 32.));
            }
            if let Some(value) = node.props.get("maxVisible") {
                control =
                    control.max_visible(value.as_u64().expect("validated maxVisible") as usize);
            }
            control.into_any_element()
        }
        "AgentRoster" => {
            let mut control = if let Some(run) = node.props.get("run") {
                AgentRoster::from_run(id, &decode(run))
            } else {
                AgentRoster::new(id, items(node, "agents").map(decode::<AgentSnapshot>))
                    .tasks(items(node, "tasks").map(decode::<AgentTaskSnapshot>))
            }
            .appearances(appearances(node, _cx));
            if node.props.contains_key("selected") {
                control = control.selected(text(node, "selected"));
            }
            if let Some(value) = node.props.get("visibleRows") {
                control =
                    control.visible_rows(value.as_u64().expect("validated visibleRows") as usize);
            }
            if let Some(action) = event(node, "action") {
                control = control.on_action(move |value, _, _| emit(&action, action_value(value)));
            }
            control.into_any_element()
        }
        "SubagentTree" => {
            let mut control = SubagentTree::new(id, decode(&node.props["run"]))
                .expanded(items(node, "expanded").map(decode::<AgentId>));
            if node.props.contains_key("selected") {
                control = control.selected(text(node, "selected"));
            }
            if let Some(value) = node.props.get("visibleRows") {
                control =
                    control.visible_rows(value.as_u64().expect("validated visibleRows") as usize);
            }
            if let Some(action) = event(node, "toggle") {
                let emit = emit.clone();
                control = control.on_toggle(move |id, expanded, _, _| {
                    emit(&action, json!({"id":id.as_str(),"expanded":expanded}))
                });
            }
            if let Some(action) = event(node, "action") {
                control = control.on_action(move |value, _, _| emit(&action, action_value(value)));
            }
            control.into_any_element()
        }
        "AgentRunIssues" => {
            if let Some(run) = node.props.get("run") {
                AgentRunIssues::from_run(id, &decode(run)).into_any_element()
            } else {
                AgentRunIssues::new(
                    id,
                    items(node, "issues").map(|value| {
                        let id = string(value, "id");
                        match value["kind"].as_str().expect("validated issue kind") {
                            "missing-root" => AgentModelIssue::MissingRoot(id.into()),
                            "duplicate-agent" => AgentModelIssue::DuplicateAgent(id.into()),
                            "duplicate-task" => AgentModelIssue::DuplicateTask(id.into()),
                            "duplicate-link" => AgentModelIssue::DuplicateLink(id.into()),
                            "self-link" => AgentModelIssue::SelfLink(id.into()),
                            "missing-task-owner" => AgentModelIssue::MissingTaskOwner {
                                task: string(value, "task").into(),
                                owner: string(value, "owner").into(),
                            },
                            "missing-link-endpoint" => AgentModelIssue::MissingLinkEndpoint {
                                link: string(value, "link").into(),
                                endpoint: decode(&value["endpoint"]),
                            },
                            _ => unreachable!("closed model issue"),
                        }
                    }),
                )
                .into_any_element()
            }
        }
        "ArtifactPreview" => {
            let state = &node.props["state"];
            let mut control = ArtifactPreview::new(id, text(node, "title"))
                .body(text(node, "body"))
                .kind(match text(node, "kind").as_str() {
                    "document" => ArtifactKind::Document,
                    "markup" => ArtifactKind::Markup,
                    _ => ArtifactKind::Code,
                })
                .state(match state["kind"].as_str() {
                    Some("loading") => ArtifactPreviewState::Loading,
                    Some("empty") => ArtifactPreviewState::Empty,
                    Some("unavailable") => {
                        ArtifactPreviewState::Unavailable(string(state, "reason").into())
                    }
                    Some("error") => ArtifactPreviewState::Error(string(state, "reason").into()),
                    _ => ArtifactPreviewState::Ready,
                });
            if node.props.contains_key("language") {
                control = control.language(
                    gpui_kit::content::Language::named(&text(node, "language"))
                        .expect("closed language"),
                );
            }
            for name in ["empty", "failed", "loading"] {
                if let Some(slot) = slots.get(name).cloned() {
                    control = control.slot(name, move |window, cx| slot(window, cx));
                }
            }
            control.into_any_element()
        }
        "ToolCall" => {
            let state = &node.props["state"];
            let mut control = ToolCall::new(
                id,
                match text(node, "family").as_str() {
                    "read" => ToolFamily::Read,
                    "network" => ToolFamily::Network,
                    "shell" => ToolFamily::Shell,
                    "edit" => ToolFamily::Edit,
                    _ => ToolFamily::External,
                },
                text(node, "tool"),
            )
            .expanded(flag(node, "expanded"))
            .presentation(if text(node, "presentation") == "flow" {
                AgentDisclosurePresentation::Flow
            } else {
                AgentDisclosurePresentation::Inset
            })
            .state(match state["kind"].as_str() {
                Some("running") => ToolCallState::Running,
                Some("succeeded") => ToolCallState::Succeeded {
                    output: if state["output"]["kind"] == "silent" {
                        ToolOutput::Silent
                    } else {
                        ToolOutput::Body(tool_body(&state["output"]["body"]))
                    },
                },
                Some("failed") => ToolCallState::failed(string(state, "error")),
                Some("refused") => ToolCallState::refused(string(state, "reason")),
                _ => ToolCallState::PendingApproval,
            });
            if node.props.contains_key("summary") {
                control = control.summary(text(node, "summary"));
            }
            if let Some(value) = node.props.get("arguments") {
                control = control.arguments(tool_body(value));
            }
            if let Some(value) = node.props.get("elapsed") {
                control = control.elapsed(if value.is_null() {
                    Elapsed::Unknown
                } else {
                    Elapsed::Took(
                        value
                            .as_str()
                            .expect("validated elapsed text")
                            .to_owned()
                            .into(),
                    )
                });
            }
            if let Some(slot) = slots.get("diff") {
                control = control.diff(slot(_window, _cx));
            }
            if let Some(action) = event(node, "toggle") {
                let emit = emit.clone();
                control = control.on_toggle(move |expanded, _, _| emit(&action, json!(expanded)));
            }
            if let Some(action) = event(node, "retry") {
                control = control.on_retry(move |_, _| emit(&action, Value::Null));
            }
            control.into_any_element()
        }
        "ServerList" => {
            let servers = items(node, "servers").map(|value| {
                let state = &value["state"];
                let catalog = &value["catalog"];
                let mut server = ServerEntry::new(string(value, "id"), string(value, "name"))
                    .state(match state["kind"].as_str() {
                        Some("connected") => ServerState::Connected,
                        Some("connecting") => ServerState::Connecting,
                        Some("failed") => ServerState::Failed {
                            reason: string(state, "reason").into(),
                        },
                        Some("disabled") => ServerState::Disabled {
                            reason: state
                                .get("reason")
                                .and_then(Value::as_str)
                                .map(|s| s.to_owned().into()),
                        },
                        _ => ServerState::Disconnected,
                    })
                    .catalog(match catalog["kind"].as_str() {
                        Some("asking") => Catalog::Asking,
                        Some("offers") => Catalog::Offers(
                            catalog["offerings"]
                                .as_array()
                                .expect("validated offerings")
                                .iter()
                                .map(offering)
                                .collect(),
                        ),
                        Some("unavailable") => {
                            Catalog::Unavailable(string(catalog, "reason").into())
                        }
                        _ => Catalog::Unasked,
                    });
                if value.get("detail").is_some() {
                    server = server.detail(string(value, "detail"));
                }
                server
            });
            let mut control = ServerList::new(id)
                .servers(servers)
                .control_size(super::size(node))
                .disabled(flag(node, "disabled"))
                .expanded(items(node, "expanded").map(|value| {
                    SharedString::from(value.as_str().expect("validated expanded id").to_owned())
                }));
            if node.props.contains_key("selected") {
                control = control.selected(text(node, "selected"));
            }
            for name in ["empty", "loading"] {
                if let Some(slot) = slots.get(name).cloned() {
                    control = control.slot(name, move |window, cx| slot(window, cx));
                }
            }
            if let Some(action) = event(node, "select") {
                let emit = emit.clone();
                control = control.on_select(move |id, _, _| emit(&action, json!(id.as_ref())));
            }
            if let Some(action) = event(node, "retry") {
                let emit = emit.clone();
                control = control.on_retry(move |id, _, _| emit(&action, json!(id.as_ref())));
            }
            if let Some(action) = event(node, "toggle") {
                control = control.on_toggle(move |id, expanded, _, _| {
                    emit(&action, json!({"id":id.as_ref(),"expanded":expanded}))
                });
            }
            control.into_any_element()
        }
        "OfferingCatalog" => {
            let sources = items(node, "sources").map(|value| {
                let state = &value["state"];
                let offerings = || {
                    state["offerings"]
                        .as_array()
                        .expect("validated searchable offerings")
                        .iter()
                        .map(|value| {
                            SearchableOffering::new(
                                offering(&value["offering"]),
                                string(value, "searchableText"),
                            )
                        })
                        .collect()
                };
                OfferingSource::new(
                    string(value, "id"),
                    string(value, "name"),
                    match state["kind"].as_str() {
                        Some("loading") => OfferingSourceState::Loading,
                        Some("empty") => OfferingSourceState::Empty,
                        Some("unavailable") => {
                            OfferingSourceState::Unavailable(string(state, "reason").into())
                        }
                        Some("error") => OfferingSourceState::Error(string(state, "reason").into()),
                        Some("stale") => OfferingSourceState::Stale {
                            offerings: offerings(),
                            reason: string(state, "reason").into(),
                        },
                        _ => OfferingSourceState::Ready(offerings()),
                    },
                )
            });
            let mut control = OfferingCatalog::new(id)
                .sources(sources)
                .control_size(super::size(node))
                .disabled(flag(node, "disabled"));
            if node.props.contains_key("query") {
                control = control.query(text(node, "query"));
            }
            if node.props.contains_key("kinds") {
                control = control.kinds(
                    items(node, "kinds")
                        .map(|value| offering_kind(value.as_str().expect("validated kind"))),
                );
            }
            if let Some(value) = node.props.get("selected") {
                control = control.selected(OfferingIdentity::new(
                    string(value, "serverId"),
                    string(value, "offeringId"),
                ));
            }
            for name in ["empty", "failed", "loading"] {
                if let Some(slot) = slots.get(name).cloned() {
                    control = control.slot(name, move |window, cx| slot(window, cx));
                }
            }
            if let Some(action) = event(node, "activate") {
                control = control.on_activate(move |identity, _, _| emit(&action, json!({"serverId":identity.server_id.as_ref(),"offeringId":identity.offering_id.as_ref()})));
            }
            control.into_any_element()
        }
        "PermissionMatrix" => {
            let mut control = PermissionMatrix::new(id)
                .actions(items(node, "actions").map(|value| {
                    PermissionAction::new(string(value, "key"), string(value, "label"))
                }))
                .subjects(items(node, "subjects").map(|value| {
                    let mut subject =
                        PermissionSubject::new(string(value, "id"), string(value, "label"));
                    for cell in value["cells"]
                        .as_array()
                        .expect("validated permission cells")
                    {
                        let state =
                            permission(cell["state"].as_str().expect("validated permission state"));
                        subject = subject.cell(
                            string(cell, "action"),
                            if cell.get("inherited").is_some() {
                                PermissionEntry::inherited(state, string(cell, "inherited"))
                            } else {
                                PermissionEntry::new(state)
                            },
                        );
                    }
                    subject
                }));
            if let Some(action) = event(node, "change") {
                control = control.on_change(move |value, _, _| emit(&action, json!({"subject":value.subject.as_ref(),"action":value.action.as_ref(),"next":value.next.name()})));
            }
            control.into_any_element()
        }
        "VoiceReactive" => {
            let mut control = VoiceReactive::new(id, voice(&node.props["sample"]));
            if let Some(value) = node.props.get("sampleAt") {
                control = control.sample_at(std::time::Duration::from_millis(
                    value.as_u64().expect("validated sampleAt"),
                ));
            }
            control.into_any_element()
        }
        "PersonaPortrait" => {
            let mut control = PersonaPortrait::new(id, decode(&node.props["agent"]));
            if let Some(value) = node.props.get("image") {
                control = control.image_source(image(value, _cx));
            }
            if let Some(value) = node.props.get("expression") {
                control = control.expression(super::game_effects::expression(value));
            }
            if let Some(value) = node.props.get("voice") {
                control = control.voice(voice(value));
            }
            if let Some(value) = node.props.get("tint") {
                control = control.tint(tint(value));
            }
            if let Some(value) = node.props.get("effect") {
                control = control.effect(super::game_effects::plan(value));
            }
            if let Some(value) = node.props.get("sampleAt") {
                control = control.sample_at(std::time::Duration::from_millis(
                    value.as_u64().expect("validated sampleAt"),
                ));
            }
            if let Some(value) = node.props.get("size") {
                control = control.size(value.as_f64().expect("validated portrait size") as f32);
            }
            control.into_any_element()
        }
        "PersonaDialogue" => {
            let value = &node.props["turn"];
            let mut turn = DialogueTurn::new(
                string(value, "id"),
                decode(&value["agent"]),
                super::content::message_body(&value["body"]),
            )
            .streaming(value["streaming"].as_bool().unwrap_or(false));
            for choice in value["choices"]
                .as_array()
                .expect("validated dialogue choices")
            {
                let mut native = DialogueChoice::new(string(choice, "id"), string(choice, "label"))
                    .selected(choice["selected"].as_bool().unwrap_or(false));
                if choice.get("detail").is_some() {
                    native = native.detail(string(choice, "detail"));
                }
                if choice.get("unavailable").is_some() {
                    native = native.unavailable(string(choice, "unavailable"));
                }
                turn = turn.choice(native);
            }
            let mut control = PersonaDialogue::new(id, turn);
            if let Some(value) = node.props.get("image") {
                control = control.image_source(image(value, _cx));
            }
            if let Some(value) = node.props.get("expression") {
                control = control.expression(super::game_effects::expression(value));
            }
            if let Some(value) = node.props.get("voice") {
                control = control.voice(voice(value));
            }
            if let Some(value) = node.props.get("tint") {
                control = control.tint(tint(value));
            }
            let highlights = items(node, "highlights").cloned().collect::<Vec<_>>();
            if !highlights.is_empty() {
                control = control.markdown_highlighter(move |_, code| {
                    highlights
                        .iter()
                        .find(|item| {
                            item["text"].as_str() == Some(code.text.as_ref())
                                && item["language"].as_str() == code.language.as_deref()
                        })
                        .map_or_else(Vec::new, |item| super::content::spans(item, "spans"))
                });
            }
            let images = items(node, "images").cloned().collect::<Vec<_>>();
            if !images.is_empty() {
                let owner_id = node.id.clone();
                control = control.markdown_image(move |_, request, _, cx| {
                    images
                        .iter()
                        .find(|image| image["src"].as_str() == Some(request.src.as_ref()))
                        .map(|value| {
                            let unavailable_id = SharedString::from(format!(
                                "{owner_id}.markdown-image.{}.unavailable",
                                string(&value["resource"], "key")
                            ));
                            gpui::img(image(&value["resource"], cx))
                                .with_fallback(move || {
                                    gpui_kit::display::icon::Icon::named(
                                        unavailable_id.clone(),
                                        gpui_kit::assets::Icon::Danger,
                                        "Resource unavailable/refused",
                                    )
                                    .warning()
                                    .into_any_element()
                                })
                                .into_any_element()
                        })
                });
            }
            let events = node.events.clone();
            if !events.is_empty() {
                control = control.on_event(move |event, _, _| {
                    let (name, payload) = match event {
                        PersonaDialogueEvent::ChoiceRequested { turn_id, choice_id } => ("choiceRequested", json!({"turnId":turn_id.as_ref(),"choiceId":choice_id.as_ref()})),
                        PersonaDialogueEvent::Markdown { turn_id, event } => ("markdown", json!({"turnId":turn_id.as_ref(),"event":super::content::markdown_event(event)})),
                    };
                    if let Some(action) = events.get(name) {
                        emit(action, payload);
                    }
                });
            }
            control.into_any_element()
        }
        "AgentRunCanvas" => {
            let mut control = AgentRunCanvas::new(id, decode(&node.props["run"]))
                .layout(if text(node, "layout") == "vertical" {
                    AgentRunLayout::LayeredVertical
                } else {
                    AgentRunLayout::LayeredHorizontal
                })
                .selected(items(node, "selected").map(decode::<RunSubjectId>))
                .arrangeable(flag(node, "arrangeable"));
            for value in items(node, "positions") {
                control = control.position(
                    decode(&value["subject"]),
                    value["x"].as_f64().expect("validated position x") as f32,
                    value["y"].as_f64().expect("validated position y") as f32,
                );
            }
            if let Some(value) = node.props.get("viewport") {
                control = control.viewport(viewport(value));
            }
            let events = node.events.clone();
            if !events.is_empty() {
                control = control.on_event(move |event, _, _| {
                    let (name, payload) = match event {
                        AgentRunCanvasEvent::SelectionChanged(subjects) => {
                            ("selectionChanged", json!(subjects))
                        }
                        AgentRunCanvasEvent::ViewportChanged(value) => (
                            "viewportChanged",
                            json!({"x":value.offset.x,"y":value.offset.y,"zoom":value.zoom}),
                        ),
                        AgentRunCanvasEvent::PositionChanged { subject, position } => (
                            "positionChanged",
                            json!({"subject":subject,"x":position.x,"y":position.y}),
                        ),
                    };
                    if let Some(action) = events.get(name) {
                        emit(action, payload);
                    }
                });
            }
            control.into_any_element()
        }
        _ => unreachable!("validated agent registration"),
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests;
