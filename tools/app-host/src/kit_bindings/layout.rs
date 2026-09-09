//! Native navigation and layout slots. Slot contents have already passed host
//! validation; host factories rebuild their elements without running guest code.
use super::*;
use gpui::div;

fn content(slots: &KitSlots, name: &str, window: &mut Window, cx: &mut App) -> AnyElement {
    slots
        .get(name)
        .map_or_else(|| div().into_any_element(), |factory| factory(window, cx))
}
fn integer(node: &Node, key: &str, default: usize) -> usize {
    node.props
        .get(key)
        .and_then(Value::as_u64)
        .map_or(default, |n| n as usize)
}

pub(super) fn render(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
) -> AnyElement {
    let id = SharedString::from(node.id.clone());
    let event = |name: &str| {
        if flag(node, "disabled") {
            None
        } else {
            node.events.get(name).cloned()
        }
    };
    match node.component.as_deref().unwrap_or_default() {
        "Pagination" => {
            let mut control = Pagination::new(id)
                .page(integer(node, "page", 1))
                .siblings(integer(node, "siblings", 1))
                .disabled(flag(node, "disabled"));
            if node.props.contains_key("size") {
                control = control.control_size(size(node));
            }
            control = if node.props.contains_key("totalPages") {
                control.total_pages(integer(node, "totalPages", 1))
            } else {
                control.unknown_total(flag(node, "hasNext"))
            };
            if let Some(action) = event("select") {
                control = control.on_select(move |page, _, _| emit(&action, json!(page)));
            }
            control.into_any_element()
        }
        "Tabs" => {
            let tabs = node
                .props
                .get("tabs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|item| {
                    let mut tab = TabItem::new(
                        item["id"].as_str().unwrap_or_default().to_owned(),
                        item["label"].as_str().unwrap_or_default().to_owned(),
                    )
                    .disabled(item["disabled"].as_bool().unwrap_or(false))
                    .closable(item["closable"].as_bool().unwrap_or(false));
                    if let Some(badge) = item["badge"].as_str() {
                        tab = tab.badge(badge.to_owned());
                    }
                    tab
                });
            let mut control = Tabs::new(id).tabs(tabs).disabled(flag(node, "disabled"));
            if node.props.contains_key("size") {
                control = control.control_size(size(node));
            }
            if node.props.contains_key("selected") {
                control = control.selected(text(node, "selected"));
            }
            if node.props.contains_key("overflowAfter") {
                control = control.overflow_after(integer(node, "overflowAfter", 10));
            }
            if flag(node, "capsules") {
                control = control.capsules();
            }
            if flag(node, "scrolling") {
                control = control.scrolling();
            }
            if let Some(action) = event("select") {
                let emit = emit.clone();
                control = control.on_select(move |id, _, _| emit(&action, json!(id.as_ref())));
            }
            if let Some(action) = event("close") {
                control = control.on_close(move |id, _, _| emit(&action, json!(id.as_ref())));
            }
            control.into_any_element()
        }
        "Accordion" => {
            let sections = node
                .props
                .get("sections")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|item| {
                    let id = item["id"].as_str().unwrap_or_default();
                    let mut section = AccordionSection::new(
                        id.to_owned(),
                        item["title"].as_str().unwrap_or_default().to_owned(),
                    )
                    .disabled(item["disabled"].as_bool().unwrap_or(false));
                    if let Some(description) = item["description"].as_str() {
                        section = section.description(description.to_owned());
                    }
                    if slots.contains_key(id) {
                        section = section.body(content(&slots, id, window, cx));
                    }
                    section
                })
                .collect::<Vec<_>>();
            let expanded = node
                .props
                .get("expanded")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|id| SharedString::from(id.to_owned()));
            let mut control = Accordion::new(id)
                .sections(sections)
                .expanded(expanded)
                .exclusive(flag(node, "exclusive"));
            if node.props.contains_key("size") {
                control = control.control_size(size(node));
            }
            if let Some(action) = event("toggle") {
                control = control.on_toggle(move |id, expanded, _, _| {
                    emit(&action, json!({"id":id.as_ref(),"expanded":expanded}))
                });
            }
            control.into_any_element()
        }
        "ScrollArea" => {
            let mut control = ScrollArea::new(id).child(content(&slots, "content", window, cx));
            control = match text(node, "axis").as_str() {
                "horizontal" => control.horizontal(),
                "both" => control.both(),
                _ => control.vertical(),
            };
            if node.props.contains_key("label") {
                control = control.label(text(node, "label"));
            }
            if node.props.contains_key("width") {
                control = control.width(number(node, "width", 300.));
            }
            if node.props.contains_key("height") {
                control = control.height(number(node, "height", 300.));
            }
            if flag(node, "fitHeight") {
                control = control.fit_height();
            }
            control.into_any_element()
        }
        "SplitPane" => {
            let mut control = SplitPane::new(id)
                .start(content(&slots, "start", window, cx))
                .end(content(&slots, "end", window, cx))
                .collapsible(flag(node, "collapsible"));
            if text(node, "axis") == "vertical" {
                control = control.vertical();
            }
            if node.props.contains_key("ratio") {
                control = control.ratio(number(node, "ratio", 0.5));
            }
            if node.props.contains_key("minStart") || node.props.contains_key("minEnd") {
                control =
                    control.min_sizes(number(node, "minStart", 0.), number(node, "minEnd", 0.));
            }
            if node.props.contains_key("step") {
                control = control.step(number(node, "step", 10.));
            }
            if node.props.contains_key("handleLabel") {
                control = control.handle_label(text(node, "handleLabel"));
            }
            if let Some(action) = event("resize") {
                let emit = emit.clone();
                control = control.on_resize(move |ratio, _, _| emit(&action, json!(ratio)));
            }
            if let Some(action) = event("collapse") {
                control = control.on_collapse(move |side, _, _| emit(&action, json!(side.name())));
            }
            control.into_any_element()
        }
        "Divider" => {
            let mut control = Divider::new().id(id);
            if node.props.contains_key("label") {
                control = control.label(text(node, "label"));
            }
            if text(node, "axis") == "vertical" {
                control = control.vertical();
            }
            control.into_any_element()
        }
        _ => unreachable!("host admitted unsupported Kit component"),
    }
}
