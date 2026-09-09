//! Host-owned grant routing. Native gestures do not confer clipboard authority.
use super::Node;
use gpui::{App, ClipboardOperation, EffectOwner};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    rc::Rc,
    sync::mpsc::SyncSender,
};

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Grants {
    pub read: bool,
    pub write: bool,
}

#[derive(Clone, Default)]
pub(super) struct Policy {
    owners: HashMap<u64, EffectOwner>,
    mounts: HashMap<String, (u64, super::Kind, Option<String>, EffectOwner)>,
    access: Rc<RefCell<HashMap<EffectOwner, (u64, Grants)>>>,
}
impl Policy {
    pub(super) fn install(outgoing: SyncSender<Value>, cx: &mut App) -> Self {
        let policy = Self::default();
        let access = policy.access.clone();
        cx.set_clipboard_policy(move |owner, operation| {
            let Some((instance, grants)) = access.borrow().get(&owner).copied() else { return false; };
            let read = matches!(operation, ClipboardOperation::Read | ClipboardOperation::ReadPrimary | ClipboardOperation::ReadFind);
            let allowed = if read { grants.read } else { grants.write };
            if !allowed {
                let _ = outgoing.try_send(json!({"kind":"native-permission", "instance":instance, "capability":if read {"clipboard.read"} else {"clipboard.write"}}));
            }
            allowed
        });
        policy
    }
    pub(super) fn owner(&self, instance: u64) -> Option<EffectOwner> {
        self.owners.get(&instance).copied()
    }
    pub(super) fn owner_of(&self, node: &Node) -> Option<EffectOwner> {
        self.mounts
            .get(&node.id)
            .filter(|(instance, kind, component, owner)| {
                *instance == node.instance
                    && *kind == node.kind
                    && *component == node.component
                    && self.access.borrow().contains_key(owner)
            })
            .map(|(_, _, _, owner)| *owner)
    }
    pub(super) fn principal(&self, instance: u64) -> Option<EffectOwner> {
        self.owner(instance)
    }
    pub(super) fn resource_aliases(
        &self,
        grants: &BTreeMap<u64, bool>,
    ) -> HashMap<EffectOwner, EffectOwner> {
        self.mounts
            .values()
            .filter_map(|(instance, _, _, mount)| {
                (grants.get(instance) == Some(&true))
                    .then(|| {
                        self.principal(*instance)
                            .map(|principal| (*mount, principal))
                    })
                    .flatten()
            })
            .collect()
    }
    pub(super) fn is_active(&self, instance: u64) -> bool {
        self.mounts.values().any(|(candidate, _, _, owner)| {
            *candidate == instance && self.access.borrow().contains_key(owner)
        })
    }
    pub(super) fn revoke(&self) {
        self.access.borrow_mut().clear();
    }
    pub(super) fn release(&self, cx: &mut App) {
        self.revoke();
        for (_, _, _, owner) in self.mounts.values() {
            gpui_kit::foundation::release_owner_state(*owner, cx);
        }
    }
    /// Mint and register actual mounts; release absent or replaced mounts before rendering.
    pub(super) fn reconcile(&mut self, root: &Node, grants: &BTreeMap<u64, Grants>, cx: &mut App) {
        fn visit<'a>(node: &'a Node, live: &mut HashSet<u64>, nodes: &mut Vec<&'a Node>) {
            if node.instance != 0 {
                live.insert(node.instance);
                nodes.push(node);
            }
            for child in node.children.iter().chain(node.slots.values().flatten()) {
                visit(child, live, nodes);
            }
        }
        let mut live = HashSet::new();
        let mut nodes = Vec::new();
        visit(root, &mut live, &mut nodes);
        self.owners.retain(|instance, _| live.contains(instance));
        for instance in live {
            self.owners.entry(instance).or_default();
        }
        let mut previous = std::mem::take(&mut self.mounts);
        let mut access = self.access.borrow_mut();
        access.clear();
        let mut retired = Vec::new();
        for node in nodes {
            let owner = match previous.remove(&node.id) {
                Some((instance, kind, component, owner))
                    if instance == node.instance
                        && kind == node.kind
                        && component == node.component =>
                {
                    owner
                }
                previous => {
                    if let Some((_, _, _, owner)) = previous {
                        retired.push(owner);
                    }
                    let owner = EffectOwner::new();
                    gpui_kit::foundation::register_owner_state(owner, cx);
                    owner
                }
            };
            self.mounts.insert(
                node.id.clone(),
                (node.instance, node.kind, node.component.clone(), owner),
            );
            access.insert(
                owner,
                (
                    node.instance,
                    grants.get(&node.instance).copied().unwrap_or_default(),
                ),
            );
        }
        retired.extend(previous.into_values().map(|(_, _, _, owner)| owner));
        for owner in retired {
            gpui_kit::foundation::release_owner_state(owner, cx);
        }
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests {
    use super::*;
    use gpui::{ClipboardItem, TestAppContext};
    use std::sync::mpsc;

    #[gpui::test]
    fn native_clipboard_requires_owner_and_separate_grants_and_revokes_old_generation(
        cx: &mut TestAppContext,
    ) {
        let (outgoing, requests) = mpsc::sync_channel(8);
        let mut policy = cx.update(|cx| Policy::install(outgoing, cx));
        let root: Node = serde_json::from_value(json!({"kind":"text","id":"view","instance":7}))
            .expect("fixture");
        cx.update(|cx| policy.reconcile(&root, &BTreeMap::new(), cx));
        let old = policy.owner_of(&root).expect("registered owner");
        cx.update(|cx| {
            assert!(
                cx.try_write_to_clipboard(ClipboardItem::new_string("missing".into()))
                    .is_err()
            );
            cx.with_effect_owner(Some(old), |cx| {
                assert!(
                    cx.try_write_to_clipboard(ClipboardItem::new_string("denied".into()))
                        .is_err()
                )
            });
        });
        assert_eq!(
            requests.try_recv().expect("consent request")["capability"],
            "clipboard.write"
        );
        cx.update(|cx| {
            policy.reconcile(
                &root,
                &BTreeMap::from([(
                    7,
                    Grants {
                        read: false,
                        write: true,
                    },
                )]),
                cx,
            )
        });
        cx.update(|cx| {
            cx.with_effect_owner(Some(old), |cx| {
                assert!(
                    cx.try_write_to_clipboard(ClipboardItem::new_string("allowed".into()))
                        .is_ok()
                );
                assert!(cx.try_read_from_clipboard().is_err());
            })
        });
        let replacement: Node =
            serde_json::from_value(json!({"kind":"text","id":"view","instance":8}))
                .expect("replacement fixture");
        cx.update(|cx| {
            policy.reconcile(
                &replacement,
                &BTreeMap::from([(
                    8,
                    Grants {
                        read: true,
                        write: true,
                    },
                )]),
                cx,
            )
        });
        cx.update(|cx| {
            cx.with_effect_owner(Some(old), |cx| {
                assert!(cx.try_read_from_clipboard().is_err())
            })
        });
        let current = policy.owner_of(&replacement).expect("replacement owner");
        cx.update(|cx| {
            cx.with_effect_owner(Some(current), |cx| {
                assert_eq!(
                    cx.try_read_from_clipboard()
                        .expect("read granted")
                        .expect("previous clipboard item")
                        .text()
                        .as_deref(),
                    Some("allowed")
                )
            })
        });
        policy.revoke();
        cx.update(|cx| {
            cx.with_effect_owner(Some(current), |cx| {
                assert!(cx.try_read_from_clipboard().is_err());
            });
        });
    }
}
