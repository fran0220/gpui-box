//! What a reading direction changes, and what it must not.
//!
//! Every assertion here is either "this moved because it is logical" or "this
//! stayed because it is physical". The second kind matters more: a right-to-
//! left mode that also reversed the things that are genuinely about the screen
//! would be a different bug wearing the same name.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, ParentElement, Styled, TestAppContext, div, px};
use gpui_kit::prelude::*;
use gpui_kit_assets::Icon as Glyph;
use gpui_kit_testkit::harness::Harness;

/// What a handler was told, shared between the handler and the assertion.
type Recorded<T> = Rc<RefCell<Vec<T>>>;

fn recorder<T>() -> (Recorded<T>, Recorded<T>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

// -- The Icon component ---------------------------------------------------

#[gpui::test]
fn an_icon_renders_and_two_token_sizes_are_two_sizes(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .column()
            .child(Icon::named("glyph.small", Glyph::Danger, "Run failed").xs())
            .child(Icon::named("glyph.large", Glyph::Danger, "Run failed").large())
            .into_any_element()
    });

    let small = harness.node("glyph.small").expect("published");
    let large = harness.node("glyph.large").expect("published");

    assert!(small.visible, "an icon that draws nothing is not an icon");
    assert!(large.visible);
    assert!(
        large.bounds.width > small.bounds.width,
        "the token control scale has to actually reach the glyph"
    );
    assert_eq!(small.bounds.width, small.bounds.height, "a glyph is square");
}

#[gpui::test]
fn a_decorative_icon_is_not_announced_and_a_meaningful_one_is(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .row()
            .child(
                // Beside a label that already says it: announcing it again
                // would say the same thing twice.
                div()
                    .row()
                    .child(Icon::new(Glyph::Trash))
                    .child("Delete run"),
            )
            .child(Icon::named("status.failed", Glyph::Danger, "Run failed"))
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    assert!(
        snapshot.find("status.failed").is_some(),
        "a glyph carrying the meaning on its own has to be nameable"
    );
    assert_eq!(
        snapshot.find("status.failed").expect("published").text,
        Some("Run failed".into())
    );
    // The decorative one publishes nothing at all, so there is no node for a
    // reader to stop on. That is the default, and forgetting to decide
    // produces silence rather than a second announcement.
    assert!(
        snapshot
            .nodes
            .iter()
            .all(|node| node.text.as_deref() != Some("Trash")),
        "a decorative glyph must not appear in the tree"
    );
}

#[gpui::test]
fn a_mirroring_icon_mirrors_and_a_fixed_one_does_not(_cx: &mut TestAppContext) {
    let rtl = LayoutDirection::RightToLeft;
    let ltr = LayoutDirection::LeftToRight;

    // The property belongs to the drawing, so it answers the same question
    // whether it is asked through the component or through the catalog.
    assert!(Glyph::AltArrowRight.mirrors_in_rtl());
    assert!(Icon::new(Glyph::AltArrowRight).flips_in(rtl));
    assert!(!Icon::new(Glyph::AltArrowRight).flips_in(ltr));

    assert!(!Glyph::Check.mirrors_in_rtl());
    assert!(!Icon::new(Glyph::Check).flips_in(rtl));
    assert!(!Icon::new(Glyph::Settings).flips_in(rtl));
    assert!(!Icon::new(Glyph::Global).flips_in(rtl));
}

// -- Direction reaching a component ---------------------------------------

fn trail(cx: &mut TestAppContext) -> Harness {
    Harness::new(cx, gpui_kit::install, |_, _| {
        Breadcrumb::new("trail")
            .crumbs([
                Crumb::new("workspace", "Workspace"),
                Crumb::new("runs", "Runs"),
                Crumb::new("run-4821", "Indexing"),
            ])
            .on_select(|_, _, _| {})
            .into_any_element()
    })
}

#[gpui::test]
fn the_default_direction_is_left_to_right(cx: &mut TestAppContext) {
    let mut harness = trail(cx);
    assert_eq!(
        harness.update(|_, cx| cx.layout_direction()),
        LayoutDirection::LeftToRight,
        "a host that installed the library and set nothing gets what it had"
    );

    let root = harness.node("trail.workspace").expect("published");
    let last = harness.node("trail.run-4821").expect("published");
    assert!(root.bounds.x < last.bounds.x);
}

#[gpui::test]
fn a_direction_the_host_sets_reaches_a_component(cx: &mut TestAppContext) {
    let mut harness = trail(cx);
    let before = harness.node("trail.workspace").expect("published").bounds.x;

    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));

    let after = harness.node("trail.workspace").expect("published").bounds.x;
    let last = harness.node("trail.run-4821").expect("published").bounds.x;
    assert!(
        after > before,
        "the trail did not move, so the direction never arrived"
    );
    assert!(
        after > last,
        "the root of the trail belongs where reading starts"
    );
}

// -- Logical against physical ---------------------------------------------

#[gpui::test]
fn a_logical_edge_swaps_and_a_physical_one_does_not(cx: &mut TestAppContext) {
    let mut harness =
        Harness::new(cx, gpui_kit::install, |_, _| {
            div()
                .column()
                .w(px(320.0))
                .child(
                    Breadcrumb::new("trail")
                        .crumbs([
                            Crumb::new("workspace", "Workspace"),
                            Crumb::new("run", "Indexing"),
                        ])
                        .on_select(|_, _, _| {}),
                )
                .child(ScrollArea::new("log").vertical().height(80.0).child(
                    div().column().children(
                        (0..40).map(|line| div().h(px(20.0)).child(format!("line {line}"))),
                    ),
                ))
                .into_any_element()
        });

    // The gutter learns its own track during prepaint, so the scrollbar is
    // published from the second frame on.
    harness.frame();
    let trail_ltr = harness.node("trail.workspace").expect("published").bounds.x;
    let bar_ltr = harness
        .node("log.scrollbar.vertical")
        .expect("published")
        .bounds
        .x;

    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));

    let trail_rtl = harness.node("trail.workspace").expect("published").bounds.x;
    let bar_rtl = harness
        .node("log.scrollbar.vertical")
        .expect("published")
        .bounds
        .x;

    // The trail is logical: it runs from where reading begins.
    assert!(trail_rtl > trail_ltr);
    // The gutter of a vertical scroll region is physical. It is not saying
    // anything about reading order; it is saying which side of the window the
    // thumb is on, and that side does not move when the text does.
    assert_eq!(bar_rtl, bar_ltr, "a physical edge is not a logical one");
}

// -- Arrow keys -----------------------------------------------------------

fn tabs(cx: &mut TestAppContext) -> (Harness, Rc<RefCell<Vec<String>>>) {
    let (calls, sink) = recorder::<String>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Tabs::new("strip")
            .tabs([
                TabItem::new("overview", "Overview"),
                TabItem::new("runs", "Runs"),
                TabItem::new("logs", "Logs"),
            ])
            .selected("runs")
            .on_select(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn arrow_keys_step_the_way_the_reading_direction_says(cx: &mut TestAppContext) {
    let (mut harness, calls) = tabs(cx);
    harness.click("strip.runs");
    calls.borrow_mut().clear();

    harness.keystrokes("right");
    assert_eq!(
        *calls.borrow(),
        vec!["logs".to_string()],
        "reading left to right, the right arrow is the next tab"
    );

    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));
    harness.click("strip.runs");
    calls.borrow_mut().clear();

    harness.keystrokes("right");
    assert_eq!(
        *calls.borrow(),
        vec!["overview".to_string()],
        "reading right to left, the right arrow is the previous tab"
    );

    calls.borrow_mut().clear();
    harness.keystrokes("left");
    assert_eq!(*calls.borrow(), vec!["logs".to_string()]);
}

#[gpui::test]
fn home_and_end_are_not_arrow_keys(cx: &mut TestAppContext) {
    let (mut harness, calls) = tabs(cx);
    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));
    harness.click("strip.runs");
    calls.borrow_mut().clear();

    // Home means the first tab in reading order, which is what it already
    // meant. Only the arrows carried a physical name that had to be reread.
    harness.keystrokes("home");
    assert_eq!(*calls.borrow(), vec!["overview".to_string()]);

    calls.borrow_mut().clear();
    harness.keystrokes("end");
    assert_eq!(*calls.borrow(), vec!["logs".to_string()]);
}
