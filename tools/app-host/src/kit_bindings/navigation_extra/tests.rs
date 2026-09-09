// These tests deliberately panic at the operation that violates a fixture's contract.
#![allow(clippy::unwrap_used)]
use super::*;

fn node(component: &str, props: Value) -> Node {
    serde_json::from_value(
        json!({"kind":"kit","id":"navigation","component":component,"props":props}),
    )
    .unwrap()
}

#[test]
fn restored_history_keeps_the_forward_branch_and_rejects_invalid_cursors() {
    let n = node(
        "NavStack",
        json!({"entries":[{"id":"root"},{"id":"detail-a"},{"id":"detail-b"}],"cursor":1}),
    );
    let h = history(&n).unwrap();
    assert_eq!(h.current(), "detail-a");
    assert!(h.can_pop());
    assert!(h.can_forward());
    assert_eq!(h.entries().len(), 3);
    assert!(
        history(&node(
            "NavStack",
            json!({"entries":[{"id":"root"}],"cursor":1})
        ))
        .is_err()
    );
    assert!(
        history(&node(
            "NavStack",
            json!({"entries":[{"id":"root"},{"id":"root"}],"cursor":0})
        ))
        .is_err()
    );
}

#[test]
fn sidebar_rejects_cycles_missing_and_cross_section_parents() {
    let sections = json!([{"id":"first"},{"id":"second"}]);
    for parent in ["missing", "child", "other"] {
        assert!(validate(&node("Sidebar", json!({"sections":sections,"items":[{"id":"child","label":"Child","section":"first","within":parent},{"id":"other","label":"Other","section":"second"}]}))).is_err());
    }
    assert!(validate(&node("Sidebar", json!({"sections":sections,"items":[{"id":"parent","label":"Parent","section":"first"},{"id":"child","label":"Child","section":"first","within":"parent"}]}))).is_ok());
}

#[cfg(feature = "capture")]
#[gpui::test]
fn native_navigation_actions_use_identity_not_position(cx: &mut gpui::TestAppContext) {
    use gpui_kit_testkit::harness::Harness;
    let header = Collapsible::header_id(&"navigation".into());
    let cases = [
        (
            "AnchorList",
            json!({"anchors":[{"id":"alpha","label":"A"},{"id":"omega","label":"Z"}],"active":"omega"}),
            "navigate",
            "navigation.alpha",
            json!("alpha"),
        ),
        (
            "Breadcrumb",
            json!({"crumbs":[{"id":"alpha","label":"A"},{"id":"omega","label":"Z"}]}),
            "select",
            "navigation.alpha",
            json!("alpha"),
        ),
        (
            "Collapsible",
            json!({"title":"Details","open":true}),
            "toggle",
            header.as_ref(),
            json!(false),
        ),
        (
            "Wizard",
            json!({"steps":[{"id":"alpha","title":"A","status":"complete"},{"id":"omega","title":"Z","status":"current"}],"backTo":"alpha","finish":true}),
            "navigate",
            "navigation.finish",
            json!({"kind":"finish"}),
        ),
        (
            "Carousel",
            json!({"items":[{"id":"alpha","label":"A"},{"id":"omega","label":"Z"}],"active":"omega","looped":true}),
            "event",
            "navigation.previous",
            json!({"kind":"previous"}),
        ),
        (
            "UndoHistory",
            json!({"label":"History","entries":[{"id":"alpha","label":"A"},{"id":"omega","label":"Z"}],"current":"omega"}),
            "jump",
            "navigation.alpha",
            json!("alpha"),
        ),
        (
            "Sidebar",
            json!({"sections":[{"id":"files"}],"items":[{"id":"alpha","label":"A","section":"files"},{"id":"omega","label":"Z","section":"files"}],"active":"omega"}),
            "select",
            "navigation.alpha",
            json!("alpha"),
        ),
    ];
    for (component, props, event, target, expected) in cases {
        let mut descriptor = node(component, props);
        descriptor.events.insert(event.into(), "action".into());
        validate(&descriptor).unwrap();
        let events = Rc::new(RefCell::new(Vec::new()));
        let output = events.clone();
        let state = State::default();
        let mut harness = Harness::new(cx, gpui_kit::install, move |w, cx| {
            let output = output.clone();
            state.render(
                &descriptor,
                KitSlots::new(),
                w,
                cx,
                Rc::new(move |_, v| output.borrow_mut().push(v)),
            )
        });
        assert!(
            harness.node(target).is_some(),
            "{component}: missing {target}"
        );
        harness.click(target);
        assert_eq!(events.borrow().last(), Some(&expected), "{component}");
    }
}

#[cfg(feature = "capture")]
#[gpui::test]
fn history_mounts_only_active_slots_and_retains_focus_until_removal(cx: &mut gpui::TestAppContext) {
    use gpui::{ParentElement, div};
    use gpui_kit_testkit::harness::Harness;
    let state = Rc::new(State::default());
    let build = state.clone();
    let descriptor = Rc::new(RefCell::new(node(
        "NavStack",
        json!({"entries":[{"id":"alpha"},{"id":"omega"}],"cursor":0,"label":"Visits"}),
    )));
    let current = descriptor.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |w, cx| {
        let n = current.borrow();
        build.reconcile(&n, cx);
        let mut slots = KitSlots::new();
        for key in ["alpha", "omega"] {
            slots.insert(
                key.into(),
                Rc::new(move |_, _| div().child(key).into_any_element()),
            );
        }
        build.render(&n, slots, w, cx, Rc::new(|_, _| {}))
    });
    assert!(harness.node("navigation.alpha").is_some());
    assert!(harness.node("navigation.omega").is_none());
    let focus = state.focus.borrow()[&(0, "navigation".into())]["alpha"].clone();
    harness.update(|w, cx| focus.focus(w, cx));
    descriptor
        .borrow_mut()
        .props
        .insert("cursor".into(), json!(1));
    harness.frame();
    assert!(harness.node("navigation.alpha").is_none());
    assert!(harness.node("navigation.omega").is_some());
    descriptor
        .borrow_mut()
        .props
        .insert("cursor".into(), json!(0));
    harness.frame();
    harness.update(|w, _| assert!(focus.is_focused(w)));
    let empty: Node = serde_json::from_value(json!({"kind":"column","id":"empty"})).unwrap();
    harness.update(|_, cx| state.reconcile(&empty, cx));
    assert!(state.focus.borrow().is_empty());
}
