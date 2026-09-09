//! Virtualized caller-owned rows. Each slot factory builds a fresh native row
//! only when List requests it; this module never consumes a row slot early.
use super::*;
use gpui::{ParentElement, div};

pub(super) fn render_list(node: &Node, slots: KitSlots, emit: Emit) -> AnyElement {
    let rows = node
        .props
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let keys = rows
        .iter()
        .map(|row| SharedString::from(row["id"].as_str().unwrap_or_default().to_owned()))
        .collect::<Vec<_>>();
    let mut list = List::new(node.id.clone(), rows.len(), move |index, window, cx| {
        let row = &rows[index];
        let id = row["id"].as_str().unwrap_or_default();
        let label = row["label"].as_str().unwrap_or_default().to_owned();
        let content = slots.get(id).map_or_else(
            || div().child(label.clone()).into_any_element(),
            |factory| factory(window, cx),
        );
        let mut item = ListItem::new(id.to_owned(), content)
            .text(label)
            .disabled(row["disabled"].as_bool().unwrap_or(false));
        if let Some(parent) = row["within"].as_str() {
            item = item.within(parent.to_owned());
        }
        item
    })
    .keys(keys)
    .disabled(flag(node, "disabled"))
    .control_size(size(node))
    .reorderable(flag(node, "reorderable"));
    if node.props.contains_key("selected") {
        list = list.selected(text(node, "selected"));
    }
    if node.props.contains_key("rowHeight") {
        list = list.row_height(number(node, "rowHeight", 32.));
    }
    if let Some(rows) = node.props.get("visibleRows").and_then(Value::as_u64) {
        list = list.visible_rows(rows as usize);
    }
    if flag(node, "flowing") {
        list = list.flowing();
    }
    if flag(node, "anchoredToEnd") {
        list = list.anchored_to_end();
    }
    if flag(node, "fills") {
        list = list.fills();
    }
    if flag(node, "arriving") {
        list = list.arriving();
    }
    if !flag(node, "disabled") {
        if let Some(action) = node.events.get("select").cloned() {
            let emit = emit.clone();
            list = list.on_select(move |id, _, _| emit(&action, json!(id.as_ref())));
        }
        if let Some(action) = node.events.get("reorder").cloned() {
            list = list.on_reorder(move |intent, _, _| emit(&action, json!({
                "id":intent.item.id.as_ref(), "source":intent.item.source.as_ref(),
                "anchor":intent.position.anchor().as_ref(), "position":intent.position.verb(),
            })));
        }
    }
    list.into_any_element()
}
