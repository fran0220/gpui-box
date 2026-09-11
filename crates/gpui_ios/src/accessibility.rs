//! Persistent AccessKit projection into UIKit container-point coordinates.
use crate::ffi::{AccessibilityNode, Rect};
use accesskit::{Action, ActionRequest, Role, TreeUpdate};
use accesskit_consumer::{FilterResult, Node, Tree, TreeChangeHandler, common_filter};
use std::{collections::HashMap, ffi::CString};

pub(crate) struct ProjectedNode {
    pub native: AccessibilityNode,
    label: CString,
    value: CString,
}

impl ProjectedNode {
    pub fn native(&self) -> AccessibilityNode {
        AccessibilityNode {
            label: self.label.as_ptr(),
            value: self.value.as_ptr(),
            ..self.native
        }
    }
}

#[derive(Default)]
pub(crate) struct AccessibilityTree {
    tree: Option<Tree>,
    ids: HashMap<u128, u64>,
    targets: HashMap<u64, (accesskit::NodeId, accesskit::TreeId, Vec<Action>)>,
    next_id: u64,
}

struct Rebuild;
impl TreeChangeHandler for Rebuild {
    fn node_added(&mut self, _: &Node<'_>) {}
    fn node_updated(&mut self, _: &Node<'_>, _: &Node<'_>) {}
    fn focus_moved(&mut self, _: Option<&Node<'_>>, _: Option<&Node<'_>>) {}
    fn node_removed(&mut self, _: &Node<'_>) {}
}
fn filter(node: &Node<'_>) -> FilterResult {
    if node.is_hidden() {
        FilterResult::ExcludeSubtree
    } else {
        common_filter(node)
    }
}

impl AccessibilityTree {
    pub fn update(&mut self, update: TreeUpdate, focused: bool) -> anyhow::Result<()> {
        if let Some(tree) = self.tree.as_mut() {
            tree.update_and_process_changes(update, &mut Rebuild);
            tree.update_host_focus_state_and_process_changes(focused, &mut Rebuild);
        } else {
            anyhow::ensure!(
                update.tree.is_some(),
                "UIKit accessibility initial update has no root tree"
            );
            self.tree = Some(Tree::new(update, focused));
        }
        Ok(())
    }

    pub fn project(&mut self, scale: f64) -> Vec<ProjectedNode> {
        let Some(tree) = self.tree.as_ref() else {
            return Vec::new();
        };
        let mut stack = vec![tree.state().root()];
        let mut projected = Vec::new();
        let mut ids = HashMap::new();
        self.targets.clear();
        while let Some(node) = stack.pop() {
            match filter(&node) {
                FilterResult::ExcludeSubtree => continue,
                FilterResult::ExcludeNode => {}
                FilterResult::Include => {
                    if let Some(bounds) = node.bounding_box().filter(|r| r.x1 > r.x0 && r.y1 > r.y0)
                    {
                        let key = u128::from(node.id());
                        let id = *self.ids.entry(key).or_insert_with(|| {
                            self.next_id += 1;
                            self.next_id
                        });
                        ids.insert(key, id);
                        let actions = [Action::Click, Action::Increment, Action::Decrement]
                            .into_iter()
                            .filter(|a| !node.is_disabled() && node.supports_action(*a, &filter))
                            .collect();
                        let (target_node, target_tree) = node.locate();
                        self.targets.insert(id, (target_node, target_tree, actions));
                        let secure = node.role() == Role::PasswordInput;
                        let label = node
                            .label()
                            .or_else(|| {
                                (!secure && node.label_comes_from_value())
                                    .then(|| node.value())
                                    .flatten()
                            })
                            .unwrap_or_default();
                        let value = if secure {
                            String::new()
                        } else {
                            node.value().unwrap_or_default()
                        };
                        // AccessKit emits physical window pixels. UIKit's
                        // container-space accessibility API consumes points.
                        projected.push(ProjectedNode {
                            native: AccessibilityNode {
                                id,
                                bounds: Rect {
                                    x: bounds.x0 / scale,
                                    y: bounds.y0 / scale,
                                    width: (bounds.x1 - bounds.x0) / scale,
                                    height: (bounds.y1 - bounds.y0) / scale,
                                },
                                label: std::ptr::null(),
                                value: std::ptr::null(),
                                role: match node.role() {
                                    Role::Button => 1,
                                    Role::Slider | Role::SpinButton => 2,
                                    Role::TextInput
                                    | Role::MultilineTextInput
                                    | Role::PasswordInput => 3,
                                    Role::Image => 4,
                                    _ => 0,
                                },
                                disabled: node.is_disabled(),
                                selected: node.is_selected().unwrap_or(false),
                            },
                            label: CString::new(label.replace('\0', "�")).expect("NUL replaced"),
                            value: CString::new(value.replace('\0', "�")).expect("NUL replaced"),
                        });
                    }
                }
            }
            stack.extend(node.children().rev());
        }
        self.ids = ids;
        projected
    }

    pub fn action(&self, id: u64, action: u32) -> Option<ActionRequest> {
        let action = match action {
            0 => Action::Click,
            1 => Action::Increment,
            2 => Action::Decrement,
            _ => return None,
        };
        let (target_node, target_tree, actions) = self.targets.get(&id)?;
        actions.contains(&action).then_some(ActionRequest {
            action,
            target_node: *target_node,
            target_tree: *target_tree,
            data: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::{NodeId, TreeId};

    fn button(label: &str) -> accesskit::Node {
        let mut node = accesskit::Node::new(Role::Button);
        node.set_label(label);
        node.set_bounds(accesskit::Rect {
            x0: 30.0,
            y0: 60.0,
            x1: 300.0,
            y1: 180.0,
        });
        node.add_action(Action::Click);
        node
    }

    #[test]
    fn password_value_is_not_exported_to_uikit() {
        let mut tree = AccessibilityTree::default();
        let mut password = accesskit::Node::new(Role::PasswordInput);
        password.set_label("Password");
        password.set_value("secret-must-not-escape");
        password.set_bounds(accesskit::Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 100.0,
            y1: 40.0,
        });
        tree.update(
            TreeUpdate {
                nodes: vec![(NodeId(1), password)],
                tree: Some(accesskit::Tree::new(NodeId(1))),
                tree_id: TreeId::ROOT,
                focus: NodeId(1),
            },
            true,
        )
        .expect("valid password tree");
        let nodes = tree.project(2.0);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].label.to_str().expect("UTF8"), "Password");
        assert_eq!(nodes[0].value.to_bytes(), b"");
    }

    #[test]
    fn incremental_tree_preserves_identity_and_refuses_removed_or_disabled_actions() {
        let mut tree = AccessibilityTree::default();
        let mut root = accesskit::Node::new(Role::Window);
        root.set_children(vec![NodeId(10), NodeId(20)]);
        tree.update(
            TreeUpdate {
                nodes: vec![
                    (NodeId(1), root),
                    (NodeId(10), button("first")),
                    (NodeId(20), button("second")),
                ],
                tree: Some(accesskit::Tree::new(NodeId(1))),
                tree_id: TreeId::ROOT,
                focus: NodeId(10),
            },
            true,
        )
        .expect("valid tree");
        let first = tree.project(3.0);
        assert_eq!(first.len(), 2);
        assert_eq!(
            first[0].native.bounds,
            Rect {
                x: 10.0,
                y: 20.0,
                width: 90.0,
                height: 40.0
            }
        );
        let retained = first[0].native.id;
        let removed = first[1].native.id;
        assert_eq!(
            tree.action(retained, 0).expect("click").target_node,
            NodeId(10)
        );
        let mut root = accesskit::Node::new(Role::Window);
        root.set_children(vec![NodeId(10)]);
        let mut disabled = button("changed");
        disabled.set_disabled();
        tree.update(
            TreeUpdate {
                nodes: vec![(NodeId(1), root), (NodeId(10), disabled)],
                tree: None,
                tree_id: TreeId::ROOT,
                focus: NodeId(10),
            },
            true,
        )
        .expect("incremental");
        let next = tree.project(2.0);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].native.id, retained);
        assert_eq!(
            unsafe { std::ffi::CStr::from_ptr(next[0].native().label) }
                .to_str()
                .expect("UTF8"),
            "changed"
        );
        assert_eq!(next[0].label.to_str().expect("UTF8"), "changed");
        assert_eq!(next[0].native.bounds.width, 135.0);
        assert!(tree.action(retained, 0).is_none());
        assert!(tree.action(removed, 0).is_none());
    }
}
