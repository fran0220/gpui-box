//! Owner-scoped, weak native capabilities. Call `reconcile` before processing
//! requests against a newly mounted tree. Native arguments grant no IO rights.
use super::{Kind, Node};
use anyhow::{Result, bail, ensure};
use gpui::{App, EffectOwner, Entity, EntityId, FocusHandle, WeakFocusHandle, Window};
use serde_json::{Value, json};
use std::{
    any::TypeId,
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

pub type EntityDispatch<T> =
    fn(&Entity<T>, &str, &Value, bool, &mut Window, &mut App, &Registration<'_>) -> Result<Value>;

#[derive(Clone, PartialEq)]
struct Scope {
    instance: u64,
    id: String,
    component: Option<String>,
    kind: Kind,
    owner: EffectOwner,
}

impl Scope {
    fn new(node: &Node, owner: EffectOwner) -> Self {
        Self {
            instance: node.instance,
            id: node.id.clone(),
            component: node.component.clone(),
            kind: node.kind,
            owner,
        }
    }
}

#[derive(Clone, PartialEq)]
enum Target {
    Entity(EntityId),
    Focus(WeakFocusHandle),
}

type Invoke = dyn Fn(&str, &Value, bool, &mut Window, &mut App, &Registration<'_>) -> Result<Value>;
struct Entry {
    scope: Scope,
    kind: &'static str,
    ancestor: Option<Rc<Entry>>,
    anchor: EntityId,
    parent: EntityId,
    target: Target,
    valid: Box<dyn Fn(&App) -> bool>,
    allowed: Box<dyn Fn(&App) -> bool>,
    invoke: Box<Invoke>,
}

impl Entry {
    fn is_valid(&self, cx: &App) -> bool {
        (self.valid)(cx)
            && self
                .ancestor
                .as_ref()
                .is_none_or(|entry| entry.is_valid(cx))
    }

    fn command_allowed(&self, cx: &App) -> bool {
        (self.allowed)(cx)
            && self
                .ancestor
                .as_ref()
                .is_none_or(|entry| entry.command_allowed(cx))
    }
}

#[derive(Default)]
struct State {
    revoked: bool,
    entries: HashMap<String, Rc<Entry>>,
    types: HashMap<&'static str, TypeId>,
}

#[derive(Clone, Default)]
pub struct Registry(Rc<RefCell<State>>);

pub struct Registration<'a> {
    registry: &'a Registry,
    scope: Scope,
    anchor: Option<EntityId>,
    ancestor: Option<Rc<Entry>>,
}

fn parse(reference: &Value) -> Result<(&str, &str)> {
    let object = reference
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("invalid native reference"))?;
    ensure!(object.len() == 2, "invalid native reference fields");
    let id = object
        .get("$nativeRef")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing native reference id"))?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing native reference type"))?;
    Ok((id, kind))
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn registration(&self, node: &Node, owner: EffectOwner) -> Registration<'_> {
        Registration {
            registry: self,
            scope: Scope::new(node, owner),
            anchor: None,
            ancestor: None,
        }
    }

    fn lookup(&self, reference: &Value) -> Result<Rc<Entry>> {
        let (id, kind) = parse(reference)?;
        let state = self.0.borrow();
        ensure!(!state.revoked, "native registry revoked");
        let entry = state
            .entries
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("unknown or expired native reference"))?;
        ensure!(entry.kind == kind, "native reference type mismatch");
        Ok(entry.clone())
    }

    fn entry(&self, reference: &Value, owner: EffectOwner) -> Result<Rc<Entry>> {
        let entry = self.lookup(reference)?;
        ensure!(
            entry.scope.owner == owner,
            "native reference belongs to another owner"
        );
        Ok(entry)
    }

    pub fn source(&self, reference: &Value, instance: u64) -> Result<(String, Option<String>)> {
        let entry = self.lookup(reference)?;
        ensure!(
            entry.scope.instance == instance,
            "native reference belongs to another worker"
        );
        Ok((entry.scope.id.clone(), entry.scope.component.clone()))
    }

    pub fn release(&self, reference: &Value, owner: EffectOwner) -> Result<()> {
        self.entry(reference, owner)?;
        let (id, _) = parse(reference)?;
        self.0.borrow_mut().entries.remove(id);
        Ok(())
    }

    pub fn revoke(&self) {
        let mut state = self.0.borrow_mut();
        state.revoked = true;
        state.entries.clear();
    }

    pub fn reconcile(
        &self,
        root: &Node,
        owner_for: impl Fn(&Node) -> Option<EffectOwner>,
        parent_for: impl Fn(&Node) -> Option<EntityId>,
        cx: &App,
    ) {
        fn collect(
            node: &Node,
            owners: &impl Fn(&Node) -> Option<EffectOwner>,
            parents: &impl Fn(&Node) -> Option<EntityId>,
            scopes: &mut Vec<(Scope, Option<EntityId>)>,
        ) {
            if let Some(owner) = owners(node) {
                scopes.push((Scope::new(node, owner), parents(node)));
            }
            for child in node.children.iter().chain(node.slots.values().flatten()) {
                collect(child, owners, parents, scopes);
            }
        }
        let mut scopes = Vec::new();
        collect(root, &owner_for, &parent_for, &mut scopes);
        self.0.borrow_mut().entries.retain(|_, entry| {
            scopes
                .iter()
                .any(|(scope, parent)| scope == &entry.scope && *parent == Some(entry.anchor))
                && entry.is_valid(cx)
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn invoke(
        &self,
        owner: EffectOwner,
        reference: &Value,
        method: &str,
        args: &Value,
        query: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Value> {
        // Do not retain a RefCell borrow across validation or native dispatch.
        let entry = self.entry(reference, owner)?;
        if !entry.is_valid(cx) {
            self.release(reference, owner)?;
            bail!("native reference is no longer current");
        }
        ensure!(
            query || entry.command_allowed(cx),
            "native command is disabled"
        );
        let registration = Registration {
            registry: self,
            scope: entry.scope.clone(),
            anchor: Some(entry.anchor),
            ancestor: Some(entry.clone()),
        };
        cx.with_effect_owner(Some(owner), |cx| {
            (entry.invoke)(method, args, query, window, cx, &registration)
        })
    }

    fn insert(&self, entry: Entry, rust_type: TypeId) -> Result<Value> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let mut state = self.0.borrow_mut();
        ensure!(!state.revoked, "native registry revoked");
        ensure!(
            state
                .types
                .get(entry.kind)
                .is_none_or(|old| *old == rust_type),
            "native kind registered with inconsistent Rust type"
        );
        ensure!(
            state.types.contains_key(entry.kind) || state.types.len() < 1024,
            "native kind quota exceeded"
        );
        // Descendant references keep the original mounted root anchor. Their
        // immediate native parent may differ without replacing that root.
        state
            .entries
            .retain(|_, old| old.scope != entry.scope || old.anchor == entry.anchor);
        if let Some((id, old)) = state.entries.iter().find(|(_, old)| {
            old.scope == entry.scope
                && old.kind == entry.kind
                && old.parent == entry.parent
                && old.target == entry.target
        }) {
            return Ok(json!({"$nativeRef": id, "type": old.kind}));
        }
        ensure!(
            state.entries.len() < 1024,
            "native reference global quota exceeded"
        );
        ensure!(
            state
                .entries
                .values()
                .filter(|old| old.scope.instance == entry.scope.instance)
                .count()
                < 128,
            "native reference owner quota exceeded"
        );
        let number = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| anyhow::anyhow!("native reference id space exhausted"))?;
        let id = format!("native-{number}");
        let result = json!({"$nativeRef": id, "type": entry.kind});
        state.types.insert(entry.kind, rust_type);
        state.entries.insert(id, Rc::new(entry));
        Ok(result)
    }
}

impl Registration<'_> {
    #[allow(clippy::too_many_arguments)]
    pub fn entity<P: 'static, T: 'static>(
        &self,
        kind: &'static str,
        parent: &Entity<P>,
        target: &Entity<T>,
        current: fn(&P, &App) -> Option<Entity<T>>,
        command_allowed: fn(&P, &App) -> bool,
        dispatch: EntityDispatch<T>,
    ) -> Result<Value> {
        ensure!(kind != "FocusHandle", "reserved native kind");
        let weak_parent = parent.downgrade();
        let allowed_parent = weak_parent.clone();
        let weak_target = target.downgrade();
        let dispatch_target = weak_target.clone();
        self.registry.insert(
            Entry {
                scope: self.scope.clone(),
                kind,
                ancestor: self.ancestor.clone(),
                anchor: self.anchor.unwrap_or(parent.entity_id()),
                parent: parent.entity_id(),
                target: Target::Entity(target.entity_id()),
                valid: Box::new(move |cx| {
                    let (Some(parent), Some(target)) =
                        (weak_parent.upgrade(), weak_target.upgrade())
                    else {
                        return false;
                    };
                    current(parent.read(cx), cx)
                        .is_some_and(|actual| actual.entity_id() == target.entity_id())
                }),
                allowed: Box::new(move |cx| {
                    allowed_parent
                        .upgrade()
                        .is_some_and(|parent| command_allowed(parent.read(cx), cx))
                }),
                invoke: Box::new(move |method, args, query, window, cx, registration| {
                    let target = dispatch_target
                        .upgrade()
                        .ok_or_else(|| anyhow::anyhow!("native target released"))?;
                    dispatch(&target, method, args, query, window, cx, registration)
                }),
            },
            TypeId::of::<T>(),
        )
    }

    pub fn focus<P: 'static>(
        &self,
        parent: &Entity<P>,
        target: &FocusHandle,
        current: fn(&P, &FocusHandle, &App) -> bool,
        command_allowed: fn(&P, &App) -> bool,
    ) -> Result<Value> {
        let weak_parent = parent.downgrade();
        let allowed_parent = weak_parent.clone();
        let weak_target = target.downgrade();
        let dispatch_target = weak_target.clone();
        self.registry.insert(
            Entry {
                scope: self.scope.clone(),
                kind: "FocusHandle",
                ancestor: self.ancestor.clone(),
                anchor: self.anchor.unwrap_or(parent.entity_id()),
                parent: parent.entity_id(),
                target: Target::Focus(target.downgrade()),
                valid: Box::new(move |cx| {
                    let (Some(parent), Some(target)) =
                        (weak_parent.upgrade(), weak_target.upgrade())
                    else {
                        return false;
                    };
                    current(parent.read(cx), &target, cx)
                }),
                allowed: Box::new(move |cx| {
                    allowed_parent
                        .upgrade()
                        .is_some_and(|parent| command_allowed(parent.read(cx), cx))
                }),
                invoke: Box::new(move |method, args, query, window, cx, _| {
                    ensure!(
                        args.as_object().is_some_and(|args| args.is_empty()),
                        "focus arguments must be an empty object"
                    );
                    let target = dispatch_target
                        .upgrade()
                        .ok_or_else(|| anyhow::anyhow!("native focus released"))?;
                    match (method, query) {
                        ("focus", false) => {
                            target.focus(window, cx);
                            Ok(Value::Null)
                        }
                        ("is_focused", true) => Ok(json!(target.is_focused(window))),
                        ("contains_focused", true) => {
                            Ok(json!(target.contains_focused(window, cx)))
                        }
                        ("within_focused", true) => Ok(json!(target.within_focused(window, cx))),
                        _ => bail!("unknown focus operation or mode"),
                    }
                }),
            },
            TypeId::of::<FocusHandle>(),
        )
    }

    pub fn borrow_focus(&self, reference: &Value, cx: &App) -> Result<FocusHandle> {
        let entry = self.registry.lookup(reference)?;
        ensure!(
            entry.scope.instance == self.scope.instance,
            "native reference belongs to another worker"
        );
        if !entry.is_valid(cx) {
            self.registry.release(reference, entry.scope.owner)?;
            bail!("native focus is no longer current");
        }
        let Target::Focus(target) = &entry.target else {
            bail!("expected FocusHandle reference");
        };
        target
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("native focus released"))
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests {
    use super::*;
    use gpui::{AppContext, IntoElement, TestAppContext, div};
    use gpui_kit_testkit::harness::Harness;

    struct Parent {
        child: Option<Entity<u32>>,
        focus: FocusHandle,
        disabled: bool,
    }

    fn current(parent: &Parent, _: &App) -> Option<Entity<u32>> {
        parent.child.clone()
    }
    fn allowed(parent: &Parent, _: &App) -> bool {
        !parent.disabled
    }
    fn focus_current(parent: &Parent, focus: &FocusHandle, _: &App) -> bool {
        parent.focus == *focus
    }
    fn dispatch(
        target: &Entity<u32>,
        method: &str,
        args: &Value,
        query: bool,
        _: &mut Window,
        cx: &mut App,
        registration: &Registration<'_>,
    ) -> Result<Value> {
        ensure!(
            method == "value" && query && args == &json!({}),
            "invalid test operation"
        );
        // Reentrant access proves invocation does not retain the map borrow.
        let _ = registration.registry.0.borrow_mut();
        Ok(json!(*target.read(cx)))
    }
    fn node(instance: u64) -> Result<Node> {
        Ok(serde_json::from_value(
            json!({"kind":"kit", "id":format!("source-{instance}"), "instance":instance, "component":"TextInput"}),
        )?)
    }

    #[gpui::test]
    fn nested_focus_inherits_parent_validity_and_command_guard(cx: &mut TestAppContext) {
        struct Root {
            child: Option<Entity<Parent>>,
            disabled: bool,
        }
        fn child_dispatch(
            target: &Entity<Parent>,
            _: &str,
            _: &Value,
            _: bool,
            _: &mut Window,
            cx: &mut App,
            refs: &Registration<'_>,
        ) -> Result<Value> {
            refs.focus(target, &target.read(cx).focus, focus_current, allowed)
        }
        let mut harness = Harness::new(cx, gpui_kit::install, |_, _| div().into_any_element());
        harness.update(|window, cx| {
            let registry = Registry::new();
            let owner = EffectOwner::new();
            let child = cx.new(|cx| Parent {
                child: None,
                focus: cx.focus_handle(),
                disabled: false,
            });
            let root = cx.new(|_| Root {
                child: Some(child.clone()),
                disabled: false,
            });
            let source = node(1).expect("source descriptor");
            let reference = registry
                .registration(&source, owner)
                .entity(
                    "Parent",
                    &root,
                    &child,
                    |root, _| root.child.clone(),
                    |root, _| !root.disabled,
                    child_dispatch,
                )
                .expect("child reference");
            let focus = registry
                .invoke(
                    owner,
                    &reference,
                    "focus_handle",
                    &json!({}),
                    true,
                    window,
                    cx,
                )
                .expect("nested focus reference");
            registry
                .invoke(owner, &focus, "focus", &json!({}), false, window, cx)
                .expect("initial focus permitted");
            root.update(cx, |root, _| root.disabled = true);
            assert!(
                registry
                    .invoke(owner, &focus, "focus", &json!({}), false, window, cx)
                    .is_err()
            );
            assert_eq!(
                registry
                    .invoke(owner, &focus, "is_focused", &json!({}), true, window, cx)
                    .expect("disabled parent still permits query"),
                json!(true)
            );
            root.update(cx, |root, _| {
                root.disabled = false;
                root.child = None;
            });
            assert!(
                registry
                    .invoke(owner, &focus, "is_focused", &json!({}), true, window, cx)
                    .is_err()
            );
            assert!(
                registry
                    .registration(&source, owner)
                    .borrow_focus(&focus, cx)
                    .is_err()
            );
            // Keeping the old native child alive must not keep its capability valid.
            assert!(!child.read(cx).disabled);
        });
    }

    #[gpui::test]
    fn native_lifecycle_and_focus(cx: &mut TestAppContext) {
        let mut harness = Harness::new(cx, gpui_kit::install, |_, _| div().into_any_element());
        harness
            .update(|window, cx| -> Result<()> {
                let registry = Registry::new();
                let owner = EffectOwner::new();
                let source = node(1)?;
                let child = cx.new(|_| 42_u32);
                let focus = cx.focus_handle();
                let parent = cx.new(|_| Parent {
                    child: Some(child.clone()),
                    focus: focus.clone(),
                    disabled: false,
                });
                let registration = registry.registration(&source, owner);
                let reference =
                    registration.entity("Counter", &parent, &child, current, allowed, dispatch)?;
                assert_eq!(
                    reference,
                    registration.entity("Counter", &parent, &child, current, allowed, dispatch)?
                );
                let focus_ref = registration.focus(&parent, &focus, focus_current, allowed)?;
                assert_eq!(registration.borrow_focus(&focus_ref, cx)?, focus);
                assert!(
                    registry
                        .invoke(
                            EffectOwner::new(),
                            &reference,
                            "value",
                            &json!({}),
                            true,
                            window,
                            cx
                        )
                        .is_err()
                );
                let mut forged = reference.clone();
                forged["type"] = json!("FocusHandle");
                assert!(registry.release(&forged, owner).is_err());
                assert!(registry.release(&reference, EffectOwner::new()).is_err());
                registry.reconcile(&source, |_| Some(owner), |_| Some(parent.entity_id()), cx);
                assert_eq!(
                    registry.invoke(owner, &reference, "value", &json!({}), true, window, cx)?,
                    json!(42)
                );
                assert_eq!(
                    registry.invoke(owner, &focus_ref, "focus", &json!({}), false, window, cx)?,
                    Value::Null
                );
                assert_eq!(
                    registry.invoke(
                        owner,
                        &focus_ref,
                        "is_focused",
                        &json!({}),
                        true,
                        window,
                        cx
                    )?,
                    json!(true)
                );
                parent.update(cx, |parent, _| parent.disabled = true);
                assert!(
                    registry
                        .invoke(owner, &focus_ref, "focus", &json!({}), false, window, cx)
                        .is_err()
                );
                assert_eq!(
                    registry.invoke(
                        owner,
                        &focus_ref,
                        "is_focused",
                        &json!({}),
                        true,
                        window,
                        cx
                    )?,
                    json!(true)
                );
                for (method, args, query) in [
                    ("focus", json!({}), true),
                    ("unknown", json!({}), true),
                    ("is_focused", json!({"extra":1}), true),
                ] {
                    assert!(
                        registry
                            .invoke(owner, &focus_ref, method, &args, query, window, cx)
                            .is_err()
                    );
                }
                let replacement = cx.new(|_| 7_u32);
                parent.update(cx, |parent, _| parent.child = Some(replacement.clone()));
                assert!(
                    registry
                        .invoke(owner, &reference, "value", &json!({}), true, window, cx)
                        .is_err()
                );
                assert_eq!(*child.read(cx), 42); // Old child is still alive, but not current.
                let replacement_ref = registration.entity(
                    "Counter",
                    &parent,
                    &replacement,
                    current,
                    allowed,
                    dispatch,
                )?;
                parent.update(cx, |parent, _| parent.child = None);
                drop(replacement);
                assert!(
                    registry
                        .invoke(
                            owner,
                            &replacement_ref,
                            "value",
                            &json!({}),
                            true,
                            window,
                            cx
                        )
                        .is_err()
                );
                registry.release(&focus_ref, owner)?;
                assert!(registration.borrow_focus(&focus_ref, cx).is_err());
                let new_focus = registration.focus(&parent, &focus, focus_current, allowed)?;
                assert_ne!(focus_ref, new_focus);
                let mut replaced = source.clone();
                replaced.component = Some("Button".into());
                registry.reconcile(&replaced, |_| Some(owner), |_| Some(parent.entity_id()), cx);
                assert!(registration.borrow_focus(&new_focus, cx).is_err());
                let removed = registration.focus(&parent, &focus, focus_current, allowed)?;
                registry.reconcile(&node(2)?, |_| Some(owner), |_| Some(parent.entity_id()), cx);
                assert!(registration.borrow_focus(&removed, cx).is_err());
                let changed_owner = registration.focus(&parent, &focus, focus_current, allowed)?;
                registry.reconcile(
                    &source,
                    |_| Some(EffectOwner::new()),
                    |_| Some(parent.entity_id()),
                    cx,
                );
                assert!(registration.borrow_focus(&changed_owner, cx).is_err());
                let revoked = registration.focus(&parent, &focus, focus_current, allowed)?;
                registry.clone().revoke();
                assert!(registration.borrow_focus(&revoked, cx).is_err());
                assert!(
                    registration
                        .focus(&parent, &focus, focus_current, allowed)
                        .is_err()
                );
                Ok(())
            })
            .expect("native reference lifecycle");
    }

    #[gpui::test]
    fn quotas_count_unique_live_references(cx: &mut TestAppContext) {
        let mut harness = Harness::new(cx, gpui_kit::install, |_, _| div().into_any_element());
        harness
            .update(|_, cx| -> Result<()> {
                let registry = Registry::default();
                let focus = cx.focus_handle();
                let parent = cx.new(|_| Parent {
                    child: None,
                    focus: focus.clone(),
                    disabled: false,
                });
                for owner_index in 0..8 {
                    let owner = EffectOwner::new();
                    for index in 0..128 {
                        let mut source = node(owner_index + 1)?;
                        source.id = format!("source-{owner_index}-{index}");
                        let registration = registry.registration(&source, owner);
                        let reference =
                            registration.focus(&parent, &focus, focus_current, allowed)?;
                        assert_eq!(
                            reference,
                            registration.focus(&parent, &focus, focus_current, allowed)?
                        );
                    }
                    assert!(
                        registry
                            .registration(&node(owner_index + 1)?, owner)
                            .focus(&parent, &focus, focus_current, allowed)
                            .is_err()
                    );
                }
                assert!(
                    registry
                        .registration(&node(4096)?, EffectOwner::new())
                        .focus(&parent, &focus, focus_current, allowed)
                        .is_err()
                );
                Ok(())
            })
            .expect("native reference quotas");
    }
}
