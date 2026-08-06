//! Progress, tags, and empty surfaces report what is true, including when
//! what is true is that nothing is known.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, ParentElement, Styled, TestAppContext, div};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

#[gpui::test]
fn a_known_extent_is_published_as_a_position(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ProgressBar::new("index.progress")
            .label("Indexing")
            .count(3, 12)
            .into_any_element()
    });

    let node = harness.node("index.progress").expect("published");
    assert_eq!(node.value_min, Some(0.0));
    assert_eq!(node.value_max, Some(1.0));
    assert_eq!(node.value_now, Some(0.25));
    assert_eq!(node.value.as_deref(), Some("3 of 12"));
    assert!(node.busy);
}

#[gpui::test]
fn an_unknown_extent_reports_no_position(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ProgressBar::new("index.progress")
            .label("Contacting host")
            .into_any_element()
    });

    let node = harness.node("index.progress").expect("published");
    assert_eq!(node.value_now, None, "an unknown extent must not claim one");
    assert!(node.busy);
}

#[gpui::test]
fn a_total_of_zero_stays_indeterminate(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ProgressBar::new("index.progress")
            .label("Indexing")
            .count(0, 0)
            .into_any_element()
    });

    assert_eq!(
        harness.node("index.progress").expect("published").value_now,
        None
    );
}

#[gpui::test]
fn a_tag_reports_its_own_removal(cx: &mut TestAppContext) {
    let removals = Rc::new(RefCell::new(0usize));
    let sink = removals.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Tag::new("filter.rust", "rust")
            .on_remove(move |_, _| *sink.borrow_mut() += 1)
            .into_any_element()
    });

    harness.click("filter.rust.remove");
    assert_eq!(*removals.borrow(), 1);
    assert_eq!(
        harness
            .node("filter.rust.remove")
            .expect("published")
            .text
            .as_deref(),
        Some("Remove rust")
    );
}

#[gpui::test]
fn a_tag_that_cannot_be_removed_offers_no_way_to(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        Tag::new("filter.pinned", "pinned")
            .disabled(true)
            .on_remove(|_, _| {})
            .into_any_element()
    });

    assert!(
        harness.node("filter.pinned.remove").is_none(),
        "a refused removal must not publish a control"
    );
}

#[gpui::test]
fn an_empty_surface_says_which_fact_it_is_reporting(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .flex()
            .flex_col()
            .child(
                EmptyState::new("runs.empty", "No runs yet")
                    .kind(EmptyKind::Unstarted)
                    .detail("A run appears here once one has been started."),
            )
            .child(
                EmptyState::new("runs.refused", "The host refused the request")
                    .kind(EmptyKind::Unavailable),
            )
            .into_any_element()
    });

    assert_eq!(
        harness
            .node("runs.empty")
            .expect("published")
            .value
            .as_deref(),
        Some("unstarted")
    );
    assert_eq!(
        harness
            .node("runs.refused")
            .expect("published")
            .value
            .as_deref(),
        Some("unavailable"),
        "a refusal must not be reported as an absence of data"
    );
}

#[gpui::test]
fn an_avatar_and_a_divider_appear_only_when_named(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .flex()
            .flex_col()
            .child(Avatar::new("Ada Lovelace").id("member.ada"))
            .child(Avatar::new("Unnamed"))
            .child(Divider::new().id("section.rule").label("Filters"))
            .child(Divider::new())
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    assert_eq!(
        snapshot
            .find("member.ada")
            .and_then(|node| node.text.as_deref()),
        Some("Ada Lovelace")
    );
    assert_eq!(
        snapshot
            .find("section.rule")
            .and_then(|node| node.text.as_deref()),
        Some("Filters")
    );
    assert_eq!(
        snapshot.nodes.len(),
        2,
        "decorative marks must not add noise to the tree"
    );
}
