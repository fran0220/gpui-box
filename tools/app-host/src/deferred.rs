//! Host-owned release-time decisions. Wire numbers only correlate requests;
//! resolution always uses the original typed ID retained by DeferredDrop.
use super::{Node, clipboard, kit_bindings};
use anyhow::{Result, ensure};
use gpui::{App, EffectOwner, Window};
use gpui_kit::interaction::dnd::{DeferredDrop, DropDecision, DropDecisionEvent, DropRefusal};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::mpsc::SyncSender,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Response {
    kind: String,
    id: u64,
    instance: u64,
    revision: u64,
    accepted: bool,
}

impl Response {
    pub(super) fn validate(&self) -> Result<()> {
        ensure!(
            self.kind == "drop-response"
                && self.id > 0
                && self.id <= 9_007_199_254_740_991
                && self.instance > 0
                && self.revision > 0,
            "invalid drop response"
        );
        Ok(())
    }
}

type Key = (u64, String);
struct Entry {
    owner: EffectOwner,
    revision: u64,
    controller: DeferredDrop,
}

#[derive(Clone, Default)]
pub(super) struct Router(Rc<RefCell<HashMap<Key, Entry>>>);

impl Router {
    pub(super) fn controller(&self, node: &Node, revision: u64) -> Option<(DeferredDrop, u64)> {
        self.0
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .filter(|entry| entry.revision == revision)
            .map(|entry| (entry.controller.clone(), revision))
    }

    pub(super) fn reconcile(
        &self,
        root: &Node,
        revision: u64,
        renderer: &super::NodeRenderer,
        window: &mut Window,
        cx: &mut App,
    ) {
        fn collect(node: &Node, nodes: &mut Vec<Node>) {
            if node.predicates.contains_key("accepts") {
                nodes.push(node.clone());
            }
            for child in node.children.iter().chain(node.slots.values().flatten()) {
                collect(child, nodes);
            }
        }
        let mut nodes = Vec::new();
        collect(root, &mut nodes);
        let stale: Vec<_> = {
            let mut entries = self.0.borrow_mut();
            let keys: Vec<_> = entries
                .iter()
                .filter(|((instance, id), entry)| {
                    entry.revision != revision
                        || !nodes.iter().any(|node| {
                            node.instance == *instance
                                && node.id == *id
                                && renderer.clipboard.owner_of(node) == Some(entry.owner)
                        })
                })
                .map(|(key, _)| key.clone())
                .collect();
            keys.into_iter()
                .filter_map(|key| entries.remove(&key))
                .collect()
        };
        // Never retain the map borrow across native callbacks.
        for entry in stale {
            entry.controller.revoke(window, cx);
        }
        for node in nodes {
            if !matches!(node.component.as_deref(), Some("List" | "Tabs" | "Tree"))
                || node.props.get("disabled").and_then(Value::as_bool) == Some(true)
            {
                continue;
            }
            let Some(owner) = renderer.clipboard.owner_of(&node) else {
                continue;
            };
            let key = (node.instance, node.id.clone());
            if self.0.borrow().contains_key(&key) {
                continue;
            }
            let policy: clipboard::Policy = renderer.clipboard.clone();
            let live_revision: Rc<Cell<u64>> = renderer.rendered_revision.clone();
            let source = node.clone();
            let outgoing = renderer.outgoing.clone();
            let weak = Rc::downgrade(&self.0);
            let lookup = key.clone();
            let controller = DeferredDrop::new(
                Duration::from_secs(3),
                move |intent, _, _| {
                    // These families retain their native own-surface acceptance policy;
                    // the worker may further refuse it, never widen it.
                    live_revision.get() == revision
                        && policy.owner_of(&source) == Some(owner)
                        && policy.is_active(source.instance)
                        && intent.item.source.as_ref() == source.id
                },
                move |event, window, cx| match event {
                    DropDecisionEvent::Requested(request) => {
                        let deadline = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .expect("clock after epoch")
                            .as_millis() as u64
                            + 3000;
                        let packet = json!({"kind":"drop-request","id":request.id.get(),"instance":node.instance,"revision":revision,
                            "target":node.id,"component":node.component,"reference":node.predicates["accepts"],
                            "payload":kit_bindings::deferred_payload(&node, &request.intent),"deadline":deadline});
                        if outgoing.try_send(packet).is_err() {
                            let controller = weak.upgrade().and_then(|entries| {
                                entries
                                    .borrow()
                                    .get(&lookup)
                                    .map(|entry| entry.controller.clone())
                            });
                            if let Some(controller) = controller {
                                controller.resolve(
                                    request.id,
                                    DropDecision::Refused(DropRefusal::Policy),
                                    window,
                                    cx,
                                );
                            }
                        }
                    }
                    DropDecisionEvent::Finished { id, .. } => {
                        let _ = outgoing.try_send(json!({"kind":"drop-cancel","id":id.get(),"instance":node.instance,"revision":revision}));
                    }
                },
            );
            self.0.borrow_mut().insert(
                key,
                Entry {
                    owner,
                    revision,
                    controller,
                },
            );
        }
    }

    pub(super) fn resolve(
        &self,
        response: Response,
        revision: u64,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        if response.revision != revision {
            return false;
        }
        let candidate = self
            .0
            .borrow()
            .iter()
            .filter(|((instance, _), entry)| {
                *instance == response.instance && entry.revision == response.revision
            })
            .find_map(|(_, entry)| {
                entry
                    .controller
                    .status()
                    .filter(|(request, decision)| {
                        request.id.get() == response.id && *decision == DropDecision::Pending
                    })
                    .map(|(request, _)| (entry.controller.clone(), request.id))
            });
        let Some((controller, id)) = candidate else {
            return false;
        };
        controller.resolve(
            id,
            if response.accepted {
                DropDecision::Accepted
            } else {
                DropDecision::Refused(DropRefusal::Policy)
            },
            window,
            cx,
        )
    }

    pub(super) fn clear(&self, outgoing: &SyncSender<Value>) {
        for ((instance, _), entry) in self.0.borrow_mut().drain() {
            if let Some((request, DropDecision::Pending)) = entry.controller.status() {
                let _ = outgoing.try_send(json!({"kind":"drop-cancel","id":request.id.get(),"instance":instance,"revision":entry.revision}));
            }
        }
    }
}
