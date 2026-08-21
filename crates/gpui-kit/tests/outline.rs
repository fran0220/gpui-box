//! An outline of a long surface keeps a fixed footprint, says which place you
//! are in, and never loses one.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, TestAppContext, div, prelude::*, px};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

/// A conversation of `turns` exchanges beside an outline of the prompts.
///
/// The list is real, because the outline reads its scroll position and moves
/// it: an outline tested over nothing would only be testing the arithmetic
/// that is already unit-tested.
fn outlined(
    cx: &mut TestAppContext,
    turns: usize,
    slots: Option<usize>,
) -> (Harness, Rc<RefCell<Vec<String>>>) {
    let chosen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let into = chosen.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let into = into.clone();
        let marks = (0..turns).map(|turn| {
            Mark::new(format!("ask-{turn}"), turn * 2, format!("Question {turn}"))
                .detail(format!("Answer {turn}"))
        });
        let mut outline = Outline::new("thread.outline")
            .over("thread.rows")
            .marks(marks)
            .on_select(move |id, _, _| into.borrow_mut().push(id.to_string()));
        if let Some(slots) = slots {
            outline = outline.slots(slots);
        }
        div()
            .flex()
            .h(px(400.0))
            .child(outline)
            .child(
                div().w(px(400.0)).child(
                    List::new("thread.rows", turns * 2, |row, _, _| {
                        ListItem::new(format!("row-{row}"), div().child(format!("Row {row}")))
                            .text(format!("Row {row}"))
                    })
                    .flowing(),
                ),
            )
            .into_any_element()
    });
    (harness, chosen)
}

#[gpui::test]
fn a_short_conversation_gets_one_mark_per_place(cx: &mut TestAppContext) {
    let (mut harness, _) = outlined(cx, 4, Some(12));

    for turn in 0..4 {
        let node = harness
            .node(&format!("thread.outline.ask-{turn}"))
            .unwrap_or_else(|| panic!("question {turn} has no mark"));
        assert_eq!(node.role, Role::Button);
        assert_eq!(node.text.as_deref(), Some(&*format!("Question {turn}")));
        assert_eq!(
            node.value.as_deref(),
            Some("1"),
            "nothing is condensed while everything fits"
        );
    }
}

#[gpui::test]
fn a_long_conversation_condenses_without_losing_the_ends(cx: &mut TestAppContext) {
    // The footprint is the point: a hundred questions still draw a glanceable
    // outline rather than a solid line of marks.
    let (mut harness, _) = outlined(cx, 100, Some(8));

    let drawn: Vec<_> = (0..100)
        .filter(|turn| {
            harness
                .node(&format!("thread.outline.ask-{turn}"))
                .is_some()
        })
        .collect();
    assert_eq!(drawn.len(), 8, "eight slots, eight marks: {drawn:?}");
    assert_eq!(drawn[0], 0, "the outline still starts at the beginning");

    let held: usize = drawn
        .iter()
        .map(|turn| {
            harness
                .node(&format!("thread.outline.ask-{turn}"))
                .expect("drawn")
                .value
                .expect("a mark says how many places it stands for")
                .parse::<usize>()
                .expect("a count")
        })
        .sum();
    assert_eq!(held, 100, "every question is inside exactly one mark");
    assert_eq!(
        harness
            .node("thread.outline")
            .expect("published")
            .value
            .as_deref(),
        Some("100"),
        "and the outline itself says how much it maps"
    );
}

#[gpui::test]
fn the_mark_for_the_place_you_are_reading_is_the_selected_one(cx: &mut TestAppContext) {
    let (mut harness, _) = outlined(cx, 4, Some(12));

    assert!(
        harness
            .node("thread.outline.ask-0")
            .expect("drawn")
            .selected,
        "at the top, the first question is the one being read"
    );
    for turn in 1..4 {
        assert!(
            !harness
                .node(&format!("thread.outline.ask-{turn}"))
                .expect("drawn")
                .selected
        );
    }
}

#[gpui::test]
fn choosing_a_mark_reports_it(cx: &mut TestAppContext) {
    let (mut harness, chosen) = outlined(cx, 4, Some(12));

    harness.click("thread.outline.ask-2");

    assert_eq!(*chosen.borrow(), vec!["ask-2".to_string()]);
}

#[gpui::test]
fn an_outline_of_one_place_is_not_drawn_at_all(cx: &mut TestAppContext) {
    // A control that cannot take you anywhere else is not navigation, so it is
    // not on screen.
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        Outline::new("thread.outline")
            .over("thread.rows")
            .mark(Mark::new("only", 0, "The one question"))
            .into_any_element()
    });

    assert!(harness.node("thread.outline").is_none());
    assert!(harness.node("thread.outline.only").is_none());
}
