use accesskit::{Action, ActionRequest, Node, NodeId, TreeId, TreeUpdate};
use std::collections::{HashMap, HashSet};

/// Stable Android virtual ids are allocated from AccessKit business identities,
/// never from tree positions. Only reachable nodes are exported.
#[derive(Default)]
pub(super) struct Accessibility {
    nodes: HashMap<NodeId, Node>,
    ids: HashMap<NodeId, i32>,
    next: i32,
    root: Option<NodeId>,
    tree: Option<TreeId>,
    focus: Option<NodeId>,
}
impl Accessibility {
    pub fn update(&mut self, update: TreeUpdate) {
        self.tree = Some(update.tree_id);
        self.focus = Some(update.focus);
        if let Some(tree) = update.tree {
            self.root = Some(tree.root);
        }
        self.nodes.extend(update.nodes);
        let mut reachable = HashSet::new();
        let mut pending: Vec<NodeId> = self.root.into_iter().collect();
        while let Some(id) = pending.pop() {
            if !reachable.insert(id) {
                continue;
            }
            if let Some(node) = self.nodes.get(&id) {
                pending.extend(node.children());
            }
            if !self.ids.contains_key(&id) {
                self.next = self
                    .next
                    .checked_add(1)
                    .expect("Android virtual accessibility id space exhausted");
                self.ids.insert(id, self.next);
            }
        }
        self.nodes.retain(|id, _| reachable.contains(id));
    }
    pub fn json(&self) -> String {
        let nodes: Vec<_> = self.nodes.iter().map(|(id, node)| {
            // GPUI's element boundary has already applied scale and clipping.
            // AccessKit bounds are physical pixels, not logical layout pixels.
            let bounds = node.bounds().unwrap_or_default();
            let secret = node.role() == accesskit::Role::PasswordInput;
            serde_json::json!({
                "id": self.ids[id], "label": node.label().unwrap_or_default(),
                "value": if secret { "" } else { node.value().unwrap_or_default() }, "role": format!("{:?}", node.role()),
                "children": node.children().iter().filter_map(|id| self.ids.get(id)).collect::<Vec<_>>(),
                "bounds": [bounds.x0, bounds.y0, bounds.x1, bounds.y1],
                "disabled": node.is_disabled(), "hidden": node.is_hidden() || node.bounds().is_some_and(|b| b.is_empty()), "password": secret,
                "click": node.supports_action(Action::Click), "focusable": node.supports_action(Action::Focus),
                "setValue": node.supports_action(Action::SetValue), "focused": self.focus == Some(*id),
            })
        }).collect();
        serde_json::json!({"root": self.root.and_then(|id| self.ids.get(&id)), "nodes": nodes})
            .to_string()
    }
    pub fn action(
        &self,
        virtual_id: i32,
        action: i32,
        value: Option<String>,
    ) -> Option<ActionRequest> {
        let id = self
            .ids
            .iter()
            .find_map(|(id, virtual_node)| (*virtual_node == virtual_id).then_some(*id))?;
        let node = self.nodes.get(&id)?;
        let action = match action {
            16 => Action::Click,
            1 => Action::Focus,
            2097152 => Action::SetValue,
            _ => return None,
        };
        if node.is_disabled()
            || !node.supports_action(action)
            || (action == Action::SetValue && value.is_none())
        {
            return None;
        }
        Some(ActionRequest {
            action,
            target_tree: self.tree?,
            target_node: id,
            data: if action == Action::SetValue {
                value.map(|value| accesskit::ActionData::Value(value.into()))
            } else {
                None
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::{Rect, Role, Tree};

    fn update(nodes: Vec<(NodeId, Node)>) -> TreeUpdate {
        TreeUpdate {
            nodes,
            tree: Some(Tree::new(NodeId(99))),
            focus: NodeId(7),
            tree_id: TreeId::ROOT,
        }
    }

    #[test]
    fn physical_bounds_stable_ids_removal_and_password_redaction() {
        let mut root = Node::new(Role::Window);
        root.set_children(vec![NodeId(7), NodeId(22)]);
        let mut button = Node::new(Role::Button);
        button.set_bounds(Rect::new(18., 27., 219., 90.));
        button.add_action(Action::Click);
        let mut secret = Node::new(Role::PasswordInput);
        secret.set_value("private value");
        let mut a11y = Accessibility::default();
        a11y.update(update(vec![
            (NodeId(99), root.clone()),
            (NodeId(7), button),
            (NodeId(22), secret),
        ]));
        let id = a11y.ids[&NodeId(7)];
        let value: serde_json::Value =
            serde_json::from_str(&a11y.json()).expect("valid adapter JSON");
        let button = value["nodes"]
            .as_array()
            .expect("node array")
            .iter()
            .find(|v| v["id"] == id)
            .expect("button node");
        assert_eq!(
            button["bounds"],
            serde_json::json!([18.0, 27.0, 219.0, 90.0])
        );
        assert!(!a11y.json().contains("private value"));
        assert_eq!(
            a11y.action(id, 16, None)
                .expect("supported click")
                .target_node,
            NodeId(7)
        );
        assert!(a11y.action(id, 2097152, Some("x".into())).is_none());
        root.set_children(vec![NodeId(22), NodeId(7)]);
        a11y.update(update(vec![(NodeId(99), root.clone())]));
        assert_eq!(a11y.ids[&NodeId(7)], id);
        root.set_children(vec![NodeId(22)]);
        a11y.update(update(vec![(NodeId(99), root)]));
        assert!(a11y.action(id, 16, None).is_none());
        assert!(!a11y.nodes.contains_key(&NodeId(7)));
    }
}
