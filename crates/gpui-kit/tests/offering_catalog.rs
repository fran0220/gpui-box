//! Cross-server offering search and activation through published semantics and simulated input.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, ParentElement, TestAppContext, div};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

fn result(offering: Offering, searchable_text: &str) -> SearchableOffering {
    SearchableOffering::new(offering, searchable_text.to_string())
}

fn ready_sources() -> Vec<OfferingSource> {
    vec![
        OfferingSource::new(
            "left",
            "Workspace",
            OfferingSourceState::Ready(vec![result(
                Offering::tool("read", "Read"),
                "read file workspace",
            )]),
        ),
        OfferingSource::new(
            "right",
            "Archive",
            OfferingSourceState::Ready(vec![
                result(Offering::tool("read", "Read"), "read file archive"),
                result(Offering::skill("review", "Review"), "review a change"),
                result(Offering::resource("changes", "Changes"), "release history"),
            ]),
        ),
    ]
}

#[gpui::test]
fn duplicate_names_keep_composite_identity_and_server_attribution(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        OfferingCatalog::new("catalog")
            .sources(ready_sources())
            .on_activate(move |identity, _, _| sink.borrow_mut().push(identity))
            .into_any_element()
    });

    let left = harness
        .node("catalog.results.left.read")
        .expect("left result");
    let right = harness
        .node("catalog.results.right.read")
        .expect("right result");
    assert_eq!(left.text, right.text);
    assert_ne!(left.id, right.id);
    assert_eq!(left.role, Role::Row);
    assert_eq!(
        harness
            .node("catalog.results.right.read.server")
            .and_then(|node| node.value)
            .as_deref(),
        Some("right")
    );

    harness.click("catalog.results.right.read");
    assert_eq!(
        calls.borrow().as_slice(),
        [OfferingIdentity::new("right", "read")]
    );
}

#[gpui::test]
fn delimiter_like_business_ids_remain_distinct(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        OfferingCatalog::new("catalog")
            .sources([
                OfferingSource::new(
                    "a.b",
                    "First",
                    OfferingSourceState::Ready(vec![result(
                        Offering::tool("c", "First result"),
                        "first",
                    )]),
                ),
                OfferingSource::new(
                    "a",
                    "Second",
                    OfferingSourceState::Ready(vec![result(
                        Offering::tool("b.c", "Second result"),
                        "second",
                    )]),
                ),
            ])
            .on_activate(move |identity, _, _| sink.borrow_mut().push(identity))
            .into_any_element()
    });
    let first = "catalog.results.a%2Eb.c";
    let second = "catalog.results.a.b%2Ec";
    assert_ne!(
        harness.node(first).expect("first composite result").id,
        harness.node(second).expect("second composite result").id
    );
    harness.click(first);
    harness.click(second);
    assert_eq!(
        calls.borrow().as_slice(),
        [
            OfferingIdentity::new("a.b", "c"),
            OfferingIdentity::new("a", "b.c")
        ]
    );
}

#[gpui::test]
fn caller_query_and_kind_filter_use_caller_searchable_text(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        OfferingCatalog::new("catalog")
            .sources(ready_sources())
            .query("release")
            .kinds([OfferingKind::Resource])
            .into_any_element()
    });
    assert!(harness.node("catalog.results.right.changes").is_some());
    assert!(harness.node("catalog.results.right.review").is_none());
    assert_eq!(
        harness
            .node("catalog.results")
            .and_then(|node| node.value)
            .as_deref(),
        Some("1")
    );
}

#[gpui::test]
fn mixed_sources_keep_each_state_and_verified_results(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        OfferingCatalog::new("catalog")
            .sources([
                OfferingSource::new("loading", "Loading", OfferingSourceState::Loading),
                OfferingSource::new("empty", "Empty", OfferingSourceState::Empty),
                OfferingSource::new(
                    "unavailable",
                    "Unavailable",
                    OfferingSourceState::Unavailable("not supported".into()),
                ),
                OfferingSource::new(
                    "error",
                    "Error",
                    OfferingSourceState::Error("source failed".into()),
                ),
                OfferingSource::new(
                    "ready",
                    "Ready",
                    OfferingSourceState::Ready(vec![result(
                        Offering::tool("current", "Current"),
                        "current",
                    )]),
                ),
                OfferingSource::new(
                    "stale",
                    "Stale",
                    OfferingSourceState::Stale {
                        offerings: vec![result(
                            Offering::skill("verified", "Verified"),
                            "verified",
                        )],
                        reason: "refresh failed".into(),
                    },
                ),
            ])
            .into_any_element()
    });
    assert_eq!(
        harness
            .node("catalog")
            .and_then(|node| node.value)
            .as_deref(),
        Some("mixed")
    );
    for (id, state) in [
        ("loading", "loading"),
        ("empty", "empty"),
        ("unavailable", "unavailable"),
        ("error", "error"),
        ("stale", "stale"),
    ] {
        assert_eq!(
            harness
                .node(&format!("catalog.source.{id}"))
                .and_then(|node| node.value)
                .as_deref(),
            Some(state)
        );
    }
    assert!(harness.node("catalog.results.ready.current").is_some());
    assert!(harness.node("catalog.results.stale.verified").is_some());
}

#[gpui::test]
fn homogeneous_source_states_reach_their_declared_slots(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .child(
                OfferingCatalog::new("loading-catalog")
                    .source(OfferingSource::new(
                        "loading",
                        "Loading",
                        OfferingSourceState::Loading,
                    ))
                    .slot(slot::LOADING, |_, _| {
                        Callout::new("Custom loading", Tone::Info)
                            .id("loading-catalog.custom")
                            .into_any_element()
                    }),
            )
            .child(
                OfferingCatalog::new("failed-catalog")
                    .source(OfferingSource::new(
                        "failed",
                        "Failed",
                        OfferingSourceState::Error("offline".into()),
                    ))
                    .slot(slot::FAILED, |_, _| {
                        Callout::new("Custom failure", Tone::Danger)
                            .id("failed-catalog.custom")
                            .into_any_element()
                    }),
            )
            .into_any_element()
    });

    assert!(harness.node("loading-catalog.custom").is_some());
    assert!(harness.node("failed-catalog.custom").is_some());
}

#[gpui::test]
fn disabled_catalog_installs_no_activation(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(0));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        OfferingCatalog::new("catalog")
            .sources(ready_sources())
            .disabled(true)
            .on_activate(move |_, _, _| *sink.borrow_mut() += 1)
            .into_any_element()
    });
    harness.click("catalog.results.left.read");
    assert_eq!(*calls.borrow(), 0);
    assert!(
        harness
            .node("catalog.results.left.read")
            .is_some_and(|node| node.disabled)
    );
}
