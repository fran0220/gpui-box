//! Caller-owned navigation. Only per-visit native focus handles are retained.
use super::*;
use anyhow::{Result, bail};
use gpui::{FocusHandle, div};
use gpui_kit::navigation::*;

pub(super) const COMPONENTS: &[&str] = &[
    "AnchorList",
    "Breadcrumb",
    "Carousel",
    "Collapsible",
    "NavStack",
    "Sidebar",
    "UndoHistory",
    "Wizard",
];

#[derive(Default)]
pub(super) struct State {
    focus: RefCell<HashMap<Key, HashMap<String, FocusHandle>>>,
    overflow: RefCell<HashMap<Key, Rc<Overflow>>>,
}

/// Native overflow menus shared by the two families that own overflow policy.
pub(super) struct Overflow {
    pub(super) menu: Entity<Menu>,
    route: Rc<RefCell<Route>>,
    _subscription: Subscription,
}
impl Overflow {
    pub(super) fn new(
        node: &Node,
        event: &str,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> Rc<Self> {
        let menu = cx.new(|cx| Menu::new(format!("{}.overflow-menu", node.id), window, cx));
        let route = Rc::new(RefCell::new(Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        }));
        let weak = Rc::downgrade(&route);
        let event = event.to_owned();
        let subscription = cx.subscribe(&menu, move |_, e: &MenuEvent, _| {
            if let MenuEvent::Invoked(id) = e
                && let Some(route) = weak.upgrade()
            {
                route.borrow().send(&event, json!(id.as_ref()));
            }
        });
        Rc::new(Self {
            menu,
            route,
            _subscription: subscription,
        })
    }
    pub(super) fn update(&self, node: &Node, emit: Emit) {
        *self.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
    }
}

fn items<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}
fn s(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}
fn b(value: &Value, key: &str) -> bool {
    value[key].as_bool().unwrap_or(false)
}
fn slot(slots: &KitSlots, key: &str, window: &mut Window, cx: &mut App) -> AnyElement {
    slots
        .get(key)
        .map_or_else(|| div().into_any_element(), |build| build(window, cx))
}
fn history(node: &Node) -> Result<NavHistory> {
    let entries = node
        .props
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|entry| SharedString::from(entry["id"].as_str().unwrap_or_default().to_owned()))
        .collect();
    NavHistory::restore(
        entries,
        node.props
            .get("cursor")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize,
    )
    .ok_or_else(|| anyhow::anyhow!("invalid caller-owned navigation history"))
}

impl State {
    pub(super) fn reconcile(&self, root: &Node, _: &mut App) {
        fn visit(node: &Node, live: &mut HashMap<Key, Vec<String>>) {
            if node.component.as_deref() == Some("NavStack") {
                live.insert(
                    (node.instance, node.id.clone()),
                    node.props
                        .get("entries")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|v| v["id"].as_str().map(str::to_owned))
                        .collect(),
                );
            }
            for child in node.children.iter().chain(node.slots.values().flatten()) {
                visit(child, live);
            }
        }
        let mut live = HashMap::new();
        visit(root, &mut live);
        self.focus.borrow_mut().retain(|key, handles| {
            if let Some(entries) = live.get(key) {
                handles.retain(|id, _| entries.contains(id));
                true
            } else {
                false
            }
        });
        fn menus(node: &Node, live: &mut std::collections::HashSet<Key>) {
            if node.component.as_deref() == Some("AnchorList") && flag(node, "overflow") {
                live.insert((node.instance, node.id.clone()));
            }
            for child in node.children.iter().chain(node.slots.values().flatten()) {
                menus(child, live);
            }
        }
        let mut live = std::collections::HashSet::new();
        menus(root, &mut live);
        self.overflow
            .borrow_mut()
            .retain(|key, _| live.contains(key));
    }

    pub(super) fn render(
        &self,
        node: &Node,
        slots: KitSlots,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        if node.component.as_deref() != Some("NavStack") {
            let menu = if node.component.as_deref() == Some("AnchorList") && flag(node, "overflow")
            {
                let existing = self
                    .overflow
                    .borrow()
                    .get(&(node.instance, node.id.clone()))
                    .cloned();
                let entry = existing.unwrap_or_else(|| {
                    let entry = Overflow::new(node, "navigate", window, cx, emit.clone());
                    self.overflow
                        .borrow_mut()
                        .insert((node.instance, node.id.clone()), entry.clone());
                    entry
                });
                entry.update(node, emit.clone());
                Some(entry.menu.clone())
            } else {
                None
            };
            if menu.is_none() {
                return render(node, slots, window, cx, emit);
            }
            return render_with_menu(node, slots, window, cx, emit, menu);
        }
        let history = history(node).expect("validated navigation history");
        let focus = self
            .focus
            .borrow_mut()
            .entry((node.instance, node.id.clone()))
            .or_default()
            .entry(history.current().to_string())
            .or_insert_with(|| cx.focus_handle())
            .clone();
        let content = slot(&slots, history.current(), window, cx);
        NavStack::new(
            node.id.clone(),
            &history,
            text(node, "label"),
            focus,
            content,
        )
        .into_any_element()
    }

    pub(super) fn invoke(
        &self,
        _: &Node,
        _: &str,
        _: &Value,
        _: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Result<Value> {
        bail!(
            "navigation builders have no native commands or queries; history remains caller-owned"
        )
    }
}

pub(super) fn validate(node: &Node) -> Result<()> {
    static SCHEMAS: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!("fixture/schemas.json"))
            .expect("generated navigation schemas")
    });
    super::validation::validate(
        &Value::Object(node.props.clone()),
        &SCHEMAS[node.component.as_deref().unwrap_or_default()]["props"],
    )?;
    if node.component.as_deref() == Some("NavStack") {
        history(node)?;
    }
    if node.component.as_deref() == Some("Sidebar") {
        let rows = node
            .props
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let parents: HashMap<_, _> = rows
            .iter()
            .map(|v| (v["id"].as_str().unwrap_or_default(), v["within"].as_str()))
            .collect();
        let sections = node
            .props
            .get("sections")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for row in &rows {
            anyhow::ensure!(
                sections.iter().any(|v| v["id"] == row["section"]),
                "unknown sidebar section"
            );
            let mut seen = std::collections::HashSet::new();
            let mut next = row["within"].as_str();
            while let Some(parent) = next {
                anyhow::ensure!(
                    seen.insert(parent) && parents.contains_key(parent),
                    "invalid sidebar parent"
                );
                anyhow::ensure!(
                    rows.iter()
                        .any(|v| v["id"] == parent && v["section"] == row["section"]),
                    "cross-section sidebar parent"
                );
                next = parents[parent];
            }
        }
    }
    Ok(())
}

pub(super) fn render(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
) -> AnyElement {
    render_with_menu(node, slots, window, cx, emit, None)
}
fn render_with_menu(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
    menu: Option<Entity<Menu>>,
) -> AnyElement {
    let props = Value::Object(node.props.clone());
    let event = |name: &str| {
        if flag(node, "disabled") {
            None
        } else {
            node.events.get(name).cloned()
        }
    };
    match node.component.as_deref().unwrap_or_default() {
        "AnchorList" => {
            let mut control = AnchorList::new(node.id.clone())
                .anchors(
                    items(&props, "anchors")
                        .map(|v| Anchor::new(s(v, "id"), s(v, "label")).disabled(b(v, "disabled"))),
                )
                .disabled(flag(node, "disabled"))
                .control_size(size(node));
            if node.props.contains_key("active") {
                control = control.active(text(node, "active"));
            }
            if let Some(count) = props["overflowAfter"].as_u64() {
                control = control.overflow_after(count as usize);
            }
            if let Some(menu) = menu {
                control = control.overflow_menu(menu);
            }
            if let Some(action) = event("navigate") {
                control = control.on_navigate(move |id, _, _| emit(&action, json!(id.as_ref())));
            }
            control.into_any_element()
        }
        "Breadcrumb" => {
            let mut control = Breadcrumb::new(node.id.clone())
                .crumbs(items(&props, "crumbs").map(|v| Crumb::new(s(v, "id"), s(v, "label"))));
            if let Some(count) = props["maxVisible"].as_u64() {
                control = control.max_visible(count as usize);
            }
            if let Some(action) = event("select") {
                let emit = emit.clone();
                control = control.on_select(move |id, _, _| emit(&action, json!(id.as_ref())));
            }
            if let Some(action) = event("reveal") {
                control = control.on_reveal(move |ids, _, _| {
                    emit(
                        &action,
                        json!(ids.iter().map(|id| id.as_ref()).collect::<Vec<_>>()),
                    )
                });
            }
            control.into_any_element()
        }
        "Collapsible" => {
            let mut control = Collapsible::new(node.id.clone(), text(node, "title"))
                .open(flag(node, "open"))
                .disabled(flag(node, "disabled"))
                .control_size(size(node));
            if node.props.contains_key("description") {
                control = control.description(text(node, "description"));
            }
            if slots.contains_key("body") {
                control = control.body(slot(&slots, "body", window, cx));
            }
            if let Some(action) = event("toggle") {
                control = control.on_toggle(move |open, _, _| emit(&action, json!(open)));
            }
            control.into_any_element()
        }
        "Carousel" => {
            let mut control = Carousel::new(node.id.clone())
                .items(items(&props, "items").map(|v| {
                    CarouselItem::new(
                        s(v, "id"),
                        s(v, "label"),
                        slot(&slots, &s(v, "id"), window, cx),
                    )
                }))
                .looped(flag(node, "looped"))
                .stale(flag(node, "stale"))
                .control_size(size(node));
            if node.props.contains_key("active") {
                control = control.active(text(node, "active"));
            }
            if node.props.contains_key("reason") {
                control = control.reason(text(node, "reason"));
            }
            if let Some(phase) = props["phase"].as_str() {
                control = control.phase(match phase {
                    "idle" => gpui_kit::state::Phase::Idle,
                    "queued" => gpui_kit::state::Phase::Queued,
                    "blocked" => gpui_kit::state::Phase::Blocked,
                    "loading" => gpui_kit::state::Phase::Loading,
                    "refreshing" => gpui_kit::state::Phase::Refreshing,
                    "empty" => gpui_kit::state::Phase::Empty,
                    "unavailable" => gpui_kit::state::Phase::Unavailable,
                    "error" => gpui_kit::state::Phase::Error,
                    "cancelled" => gpui_kit::state::Phase::Cancelled,
                    _ => gpui_kit::state::Phase::Ready,
                });
            }
            if let Some(action) = event("event") {
                control = control.on_event(move |event, _, _| {
                    emit(
                        &action,
                        match event {
                            CarouselEvent::Previous => json!({"kind":"previous"}),
                            CarouselEvent::Next => json!({"kind":"next"}),
                            CarouselEvent::Selected(id) => {
                                json!({"kind":"selected","id":id.as_ref()})
                            }
                        },
                    )
                });
            }
            control.into_any_element()
        }
        "UndoHistory" => {
            let entries = items(&props, "entries").map(|v| {
                let mut entry = HistoryEntry::new(s(v, "id"), s(v, "label"));
                if v.get("description").is_some() {
                    entry = entry.description(s(v, "description"));
                }
                if v.get("time").is_some() {
                    entry = entry.time(s(v, "time"));
                }
                if v.get("source").is_some() {
                    entry = entry.source(s(v, "source"));
                }
                if v.get("unavailable").is_some() {
                    entry = entry.unavailable(s(v, "unavailable"));
                }
                entry
            });
            let mut control = UndoHistory::new(node.id.clone(), text(node, "label"))
                .entries(entries)
                .disabled(flag(node, "disabled"));
            if node.props.contains_key("current") {
                control = control.current(text(node, "current"));
            }
            if let Some(action) = event("jump") {
                control = control.on_jump(move |id, _, _| emit(&action, json!(id.as_ref())));
            }
            control.into_any_element()
        }
        "Wizard" => {
            let steps = items(&props, "steps").map(|v| {
                let mut step = WizardStep::new(s(v, "id"), s(v, "title"));
                if v.get("description").is_some() {
                    step = step.description(s(v, "description"));
                }
                step = match v["status"].as_str() {
                    Some("complete") => step.complete(),
                    Some("current") => step.current(),
                    Some("blocked") => step.blocked(s(v, "reason")),
                    Some("failed") => step.failed(s(v, "reason")),
                    _ => step.upcoming(),
                };
                if let Some(reachable) = v["reachable"].as_bool() {
                    step = step.reachable(reachable);
                }
                step
            });
            let mut control = Wizard::new(node.id.clone())
                .steps(steps)
                .disabled(flag(node, "disabled"))
                .control_size(size(node))
                .finish(flag(node, "finish"))
                .can_advance(props["canAdvance"].as_bool().unwrap_or(true));
            if text(node, "layout") == "vertical" {
                control = control.vertical();
            }
            if node.props.contains_key("backTo") {
                control = control.back_to(text(node, "backTo"));
            }
            if node.props.contains_key("backLabel") {
                control = control.back_label(text(node, "backLabel"));
            }
            if node.props.contains_key("nextLabel") {
                control = control.next_label(text(node, "nextLabel"));
            }
            if node.props.contains_key("finishLabel") {
                control = control.finish_label(text(node, "finishLabel"));
            }
            if slots.contains_key("body") {
                control = control.body(slot(&slots, "body", window, cx));
            }
            if let Some(action) = event("navigate") {
                control = control.on_navigate(move |intent, _, _| {
                    emit(
                        &action,
                        match intent {
                            WizardIntent::Step(id) => json!({"kind":"step","id":id.as_ref()}),
                            other => json!({"kind":other.as_str()}),
                        },
                    )
                });
            }
            control.into_any_element()
        }
        "Sidebar" => {
            fn row(v: &Value, rows: &[Value]) -> SidebarItem {
                let mut item =
                    SidebarItem::new(s(v, "id"), s(v, "label")).disabled(b(v, "disabled"));
                if v.get("badge").is_some() {
                    item = item.badge(s(v, "badge"));
                }
                if let Some(icon) = v.get("icon") {
                    item = item.icon(super::icon::resolve(icon).expect("validated built-in icon"));
                }
                item.children(
                    rows.iter()
                        .filter(|child| child["within"] == v["id"])
                        .map(|child| row(child, rows)),
                )
            }
            let rows = items(&props, "items").cloned().collect::<Vec<_>>();
            let sections = items(&props, "sections").map(|v| {
                let mut section = SidebarSection::new(s(v, "id"));
                if v.get("title").is_some() {
                    section = section.title(s(v, "title"));
                }
                section.items(
                    rows.iter()
                        .filter(|item| item["section"] == v["id"] && item.get("within").is_none())
                        .map(|v| row(v, &rows)),
                )
            });
            let mut control = Sidebar::new(node.id.clone())
                .sections(sections)
                .collapsed(flag(node, "collapsed"))
                .disabled(flag(node, "disabled"))
                .control_size(size(node));
            if node.props.contains_key("active") {
                control = control.active(text(node, "active"));
            }
            if slots.contains_key("header") {
                control = control.header(slot(&slots, "header", window, cx));
            }
            if slots.contains_key("footer") {
                control = control.footer(slot(&slots, "footer", window, cx));
            }
            if let Some(action) = event("select") {
                control = control.on_select(move |id, _, _| emit(&action, json!(id.as_ref())));
            }
            control.into_any_element()
        }
        _ => unreachable!("navigation family dispatch"),
    }
}

#[cfg(test)]
mod tests;
