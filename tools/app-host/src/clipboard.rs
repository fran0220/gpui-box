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
    pub(super) fn revoke(&self) {
        self.access.borrow_mut().clear();
    }
    pub(super) fn reconcile(&mut self, root: &Node, grants: &BTreeMap<u64, Grants>) {
        fn visit(node: &Node, live: &mut HashSet<u64>) {
            if node.instance != 0 {
                live.insert(node.instance);
            }
            for child in node.children.iter().chain(node.slots.values().flatten()) {
                visit(child, live);
            }
        }
        let mut live = HashSet::new();
        visit(root, &mut live);
        self.owners.retain(|instance, _| live.contains(instance));
        let mut access = self.access.borrow_mut();
        access.clear();
        for instance in live {
            let owner = *self.owners.entry(instance).or_default();
            access.insert(
                owner,
                (instance, grants.get(&instance).copied().unwrap_or_default()),
            );
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
        policy.reconcile(&root, &BTreeMap::new());
        let old = policy.owner(7).expect("registered owner");
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
        policy.reconcile(
            &root,
            &BTreeMap::from([(
                7,
                Grants {
                    read: false,
                    write: true,
                },
            )]),
        );
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
        policy.reconcile(
            &replacement,
            &BTreeMap::from([(
                8,
                Grants {
                    read: true,
                    write: true,
                },
            )]),
        );
        cx.update(|cx| {
            cx.with_effect_owner(Some(old), |cx| {
                assert!(cx.try_read_from_clipboard().is_err())
            })
        });
        let current = policy.owner(8).expect("replacement owner");
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
