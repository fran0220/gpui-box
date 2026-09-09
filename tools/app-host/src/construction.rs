//! Retained, typed slot construction from host-validated descriptors.
//!
//! A native value may install callbacks only when its parent later renders it.
//! A typed EffectScoped wrapper preserves each child's owner until that render.
use super::{Kind, Node, NodeRenderer, kit_bindings};
use anyhow::{Result, ensure};
use gpui::{App, Window};
use std::rc::Rc;

pub(super) type Emit = Rc<dyn Fn(&str, serde_json::Value)>;

#[derive(Default)]
pub(super) struct NativeBuildContext {
    pub(super) deferred: Option<(gpui_kit::interaction::dnd::DeferredDrop, u64)>,
    pub(super) typed: TypedSlots,
    pub(super) slots: kit_bindings::KitSlots,
}

#[derive(Clone, Default)]
pub(super) struct TypedSlots {
    source: Option<Rc<Source>>,
}

struct Source {
    renderer: NodeRenderer,
    parent: Node,
    revision: u64,
}

pub(super) enum TypedSlotContent<T> {
    Typed(gpui::EffectScoped<T>),
    Element(gpui::AnyElement),
}

impl TypedSlots {
    /// Snapshot an already validated descriptor without retaining native state.
    pub(super) fn new(renderer: NodeRenderer, parent: &Node, revision: u64) -> Self {
        Self {
            source: Some(Rc::new(Source {
                renderer,
                parent: parent.clone(),
                revision,
            })),
        }
    }

    pub(super) fn build<T>(
        &self,
        slot: &str,
        expected_component: &str,
        window: &mut Window,
        cx: &mut App,
        builder: impl FnMut(
            &Node,
            NativeBuildContext,
            &kit_bindings::KitState,
            Emit,
            &mut Window,
            &mut App,
        ) -> Result<T>,
    ) -> Result<Vec<gpui::EffectScoped<T>>> {
        Ok(self
            .build_checked(slot, Some(expected_component), window, cx, builder)?
            .into_iter()
            .map(|(owner, value)| gpui::EffectScoped::new(owner, value))
            .collect())
    }

    /// Preserve one descriptor sequence containing typed children and ordinary
    /// content. Ordinary elements use the same guarded host factory as slots.
    pub(super) fn build_mixed<T>(
        &self,
        slot: &str,
        expected_component: &str,
        window: &mut Window,
        cx: &mut App,
        mut builder: impl FnMut(
            &Node,
            NativeBuildContext,
            &kit_bindings::KitState,
            Emit,
            &mut Window,
            &mut App,
        ) -> Result<T>,
    ) -> Result<Vec<TypedSlotContent<T>>> {
        let source = self
            .source
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("typed slot context unavailable"))?;
        Ok(self
            .build_checked(
                slot,
                None,
                window,
                cx,
                |node, context, kit, emit, window, cx| {
                    let component = match node.kind {
                        Kind::Button => Some("Button"),
                        Kind::Kit => node.component.as_deref(),
                        _ => None,
                    };
                    if component == Some(expected_component) {
                        let owner = cx.current_effect_owner().expect("validated child owner");
                        Ok(TypedSlotContent::Typed(gpui::EffectScoped::new(
                            owner,
                            builder(node, context, kit, emit, window, cx)?,
                        )))
                    } else {
                        Ok(TypedSlotContent::Element(source.renderer.node(
                            node,
                            source.revision,
                            window,
                            cx,
                        )))
                    }
                },
            )?
            .into_iter()
            .map(|(_, value)| value)
            .collect())
    }

    fn build_checked<T>(
        &self,
        slot: &str,
        expected_component: Option<&str>,
        window: &mut Window,
        cx: &mut App,
        mut builder: impl FnMut(
            &Node,
            NativeBuildContext,
            &kit_bindings::KitState,
            Emit,
            &mut Window,
            &mut App,
        ) -> Result<T>,
    ) -> Result<Vec<(gpui::EffectOwner, T)>> {
        let source = self
            .source
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("typed slot context unavailable"))?;
        let renderer = &source.renderer;
        let current = || -> Result<Rc<kit_bindings::KitState>> {
            ensure!(
                source.revision != 0 && renderer.rendered_revision.get() == source.revision,
                "typed slot revision expired"
            );
            renderer
                .kit
                .upgrade()
                .ok_or_else(|| anyhow::anyhow!("typed slot native state expired"))
        };
        let validate = |child: &Node| -> Result<()> {
            let component = match child.kind {
                Kind::Button => Some("Button"),
                Kind::Kit => child.component.as_deref(),
                _ => None,
            };
            ensure!(
                expected_component.is_none_or(|expected| component == Some(expected)),
                "wrong typed slot component"
            );
            let owner = renderer.clipboard.owner_of(&source.parent);
            ensure!(
                child.instance == source.parent.instance
                    && owner.is_some()
                    && renderer.clipboard.owner_of(child).is_some()
                    && renderer.clipboard.is_active(source.parent.instance)
                    && renderer.clipboard.is_active(child.instance),
                "typed slot requires active owners in the same worker generation"
            );
            Ok(())
        };
        // Preflight the entire slot: a malformed later sibling must not allow
        // earlier builders to register effects. Recheck after each callback too.
        current()?;
        let children = source
            .parent
            .slots
            .get(slot)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for child in children {
            validate(child)?;
        }
        let mut values = Vec::with_capacity(children.len());
        for child in children {
            let kit = current()?;
            validate(child)?;
            let context = renderer.build_context(child, source.revision);
            let emit = renderer.emitter(source.revision);
            let owner = renderer
                .clipboard
                .owner_of(child)
                .expect("validated active child owner");
            let value = cx.with_effect_owner(Some(owner), |cx| {
                builder(child, context, &kit, emit, window, cx)
            })?;
            current()?;
            validate(child)?;
            values.push((owner, value));
        }
        Ok(values)
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests {
    use super::*;
    use gpui::{IntoElement, SharedString, TestAppContext, div};
    use gpui_kit::prelude::Button;
    use gpui_kit_testkit::harness::Harness;
    use serde_json::json;
    use std::{cell::Cell, collections::BTreeMap, sync::mpsc};

    fn fixture() -> Node {
        serde_json::from_value(json!({
            "kind":"kit", "component":"SettingsList", "id":"parent", "instance":7,
            "events":{"click":"parent-action"},
            "slots":{"items":[
                {"kind":"button", "id":"first", "instance":7, "action":"first-action"},
                {"kind":"kit", "component":"Button", "id":"second", "instance":7,
                 "events":{"click":"second-action"}, "slots":{"items":[
                    {"kind":"button", "id":"nested", "instance":7, "action":"nested-action"}
                 ]}}
            ]}
        }))
        .expect("descriptor fixture")
    }

    #[gpui::test]
    fn typed_values_preserve_order_recursive_context_and_child_actions(cx: &mut TestAppContext) {
        let parent = fixture();
        let kit = Rc::new(kit_bindings::KitState::default());
        let (outgoing, events) = mpsc::sync_channel(8);
        let mut clipboard = super::super::clipboard::Policy::default();
        cx.update(|cx| clipboard.reconcile(&parent, &BTreeMap::new(), cx));
        let renderer = NodeRenderer {
            outgoing,
            kit: Rc::downgrade(&kit),
            rendered_revision: Rc::new(Cell::new(1)),
            clipboard,
        };
        let typed = TypedSlots::new(renderer, &parent, 1);
        let mut harness = Harness::new(cx, gpui_kit::install, |_, _| div().into_any_element());
        harness.update(|window, cx| {
            (|| -> Result<()> {
                let mut ids = Vec::new();
                let values: Vec<gpui::EffectScoped<Button>> = typed.build(
                    "items",
                    "Button",
                    window,
                    cx,
                    |node, context, _, emit, window, cx| {
                        ids.push(node.id.clone());
                        emit(
                            node.action
                                .as_deref()
                                .unwrap_or_else(|| &node.events["click"]),
                            json!(node.id),
                        );
                        let nested: Vec<gpui::EffectScoped<Button>> = context.typed.build(
                            "items",
                            "Button",
                            window,
                            cx,
                            |node, _, _, emit, _, _| {
                                ids.push(node.id.clone());
                                emit(
                                    node.action.as_deref().expect("child action"),
                                    json!(node.id),
                                );
                                Ok(Button::new(SharedString::from(node.id.clone())))
                            },
                        )?;
                        assert_eq!(nested.len(), usize::from(node.id == "second"));
                        Ok(Button::new(SharedString::from(node.id.clone())))
                    },
                )?;
                assert_eq!(values.len(), 2);
                assert_eq!(ids, ["first", "second", "nested"]);
                assert!(
                    typed
                        .build::<Button>(
                            "missing",
                            "Button",
                            window,
                            cx,
                            |_, _, _, _, _, _| panic!("missing slot builder")
                        )
                        .expect("empty slot")
                        .is_empty()
                );
                Ok(())
            })()
            .expect("typed construction");
        });
        for action in ["first-action", "second-action", "nested-action"] {
            let event = events.try_recv().expect("child event");
            assert_eq!(event["action"], action);
            assert_eq!(event["revision"], 1);
        }
    }

    #[gpui::test]
    fn mixed_content_preserves_order_and_typed_click_owner(cx: &mut TestAppContext) {
        use gpui::{ParentElement, Styled};
        let mut parent = fixture();
        let items = parent.slots.get_mut("items").expect("fixture items");
        items.insert(
            1,
            serde_json::from_value(
                json!({"kind":"text","id":"middle","instance":7,"text":"Interleaved content"}),
            )
            .expect("middle text"),
        );
        items.push(
            serde_json::from_value(
                json!({"kind":"text","id":"tail","instance":7,"text":"Trailing content"}),
            )
            .expect("tail text"),
        );
        let kit = Rc::new(kit_bindings::KitState::default());
        let (outgoing, _) = mpsc::sync_channel(8);
        let mut clipboard = super::super::clipboard::Policy::default();
        cx.update(|cx| clipboard.reconcile(&parent, &BTreeMap::new(), cx));
        let expected = clipboard
            .owner_of(&parent.slots["items"][2])
            .expect("second typed owner");
        let renderer = NodeRenderer {
            outgoing,
            kit: Rc::downgrade(&kit),
            rendered_revision: Rc::new(Cell::new(1)),
            clipboard,
        };
        let typed = TypedSlots::new(renderer.clone(), &parent, 1);
        let clicked = Rc::new(Cell::new(None));
        let observe = clicked.clone();
        let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
            let values = typed
                .build_mixed(
                    "items",
                    "Button",
                    window,
                    cx,
                    |node, context, _, _, _, _| {
                        assert_eq!(context.slots.len(), node.slots.len());
                        let observe = observe.clone();
                        Ok(Button::new(node.id.clone())
                            .label(node.id.clone())
                            .on_click(move |_, cx| observe.set(cx.current_effect_owner())))
                    },
                )
                .expect("mixed children");
            assert!(matches!(
                &values[..],
                [
                    TypedSlotContent::Typed(_),
                    TypedSlotContent::Element(_),
                    TypedSlotContent::Typed(_),
                    TypedSlotContent::Element(_)
                ]
            ));
            div()
                .flex()
                .flex_col()
                .children(values.into_iter().map(|value| match value {
                    TypedSlotContent::Typed(value) => value.into_any_element(),
                    TypedSlotContent::Element(value) => value,
                }))
                .into_any_element()
        });
        let bounds = ["first", "middle", "second", "tail"].map(|id| {
            harness
                .node(id)
                .expect("mixed semantic child")
                .bounds
        });
        assert!(
            bounds
                .windows(2)
                .all(|pair| pair[0].y < pair[1].y)
        );
        harness.click("second");
        assert_eq!(clicked.get(), Some(expected));
        renderer.clipboard.revoke();
        harness.update(|window, cx| {
            assert!(
                TypedSlots::new(renderer.clone(), &parent, 1)
                    .build_mixed::<Button>(
                        "items",
                        "Button",
                        window,
                        cx,
                        |_, _, _, _, _, _| panic!("revoked builder")
                    )
                    .is_err()
            );
        });
    }

    #[gpui::test]
    fn invalid_sources_refuse_before_any_builder(cx: &mut TestAppContext) {
        let parent = fixture();
        let kit = Rc::new(kit_bindings::KitState::default());
        let (outgoing, _) = mpsc::sync_channel(8);
        let revision = Rc::new(Cell::new(1));
        let mut clipboard = super::super::clipboard::Policy::default();
        cx.update(|cx| clipboard.reconcile(&parent, &BTreeMap::new(), cx));
        let renderer = NodeRenderer {
            outgoing,
            kit: Rc::downgrade(&kit),
            rendered_revision: revision.clone(),
            clipboard,
        };
        let mut harness = Harness::new(cx, gpui_kit::install, |_, _| div().into_any_element());
        harness.update(|window, cx| {
            let mut refuse = |typed: TypedSlots| {
                assert!(
                    typed
                        .build::<Button>("items", "Button", window, cx, |_, _, _, _, _, _| panic!(
                            "guard must precede all builders"
                        ))
                        .is_err()
                );
            };
            refuse(TypedSlots::default());
            let mut wrong = parent.clone();
            wrong.slots.get_mut("items").expect("fixture items")[1].component =
                Some("TextInput".into());
            refuse(TypedSlots::new(renderer.clone(), &wrong, 1));
            let mut mixed = parent.clone();
            mixed.slots.get_mut("items").expect("fixture items")[1].instance = 8;
            refuse(TypedSlots::new(renderer.clone(), &mixed, 1));
            let mut missing = renderer.clone();
            missing.clipboard = Default::default();
            refuse(TypedSlots::new(missing, &parent, 1));
            refuse(TypedSlots::new(renderer.clone(), &parent, 0));
            revision.set(2);
            refuse(TypedSlots::new(renderer.clone(), &parent, 1));
            revision.set(1);
            renderer.clipboard.revoke();
            refuse(TypedSlots::new(renderer.clone(), &parent, 1));
            drop(kit);
            refuse(TypedSlots::new(renderer, &parent, 1));
        });
    }
}
