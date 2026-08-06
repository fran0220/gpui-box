//! Hover help appears only after the pointer rests, explains a control that
//! is already usable without it, and disappears when the pointer leaves.

use std::time::Duration;

use gpui::{Modifiers, TestAppContext, div, prelude::*, px};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

/// Longer than GPUI's hover delay, so a settled pointer has shown the tooltip.
const RESTED: Duration = Duration::from_millis(800);

fn scene(cx: &mut TestAppContext) -> Harness {
    Harness::new(cx, gpui_kit::install, |_window, _cx| {
        div()
            .id("settings.export.host")
            .tip("settings.export", "Writes the theme to a file on disk")
            .child(
                Button::new("settings.export")
                    .label("Export theme")
                    .secondary()
                    .on_click(|_, _| {}),
            )
            .into_any_element()
    })
}

fn hover(harness: &mut Harness, id: &str) {
    let point = harness.point_in(id);
    harness
        .context()
        .simulate_mouse_move(point, None, Modifiers::none());
    harness.advance(RESTED);
}

#[gpui::test]
fn help_is_absent_until_the_pointer_rests_on_the_control(cx: &mut TestAppContext) {
    let mut harness = scene(cx);
    assert!(harness.node("settings.export.tooltip").is_none());

    harness.advance(RESTED);
    assert!(
        harness.node("settings.export.tooltip").is_none(),
        "time alone must not produce hover help"
    );
}

#[gpui::test]
fn resting_on_a_control_publishes_help_linked_to_it(cx: &mut TestAppContext) {
    let mut harness = scene(cx);
    hover(&mut harness, "settings.export");

    let node = harness
        .node("settings.export.tooltip")
        .expect("hover help is published");
    assert_eq!(node.role, Role::Tooltip);
    assert_eq!(
        node.text.as_deref(),
        Some("Writes the theme to a file on disk")
    );
    assert_eq!(
        node.parent.as_deref(),
        Some("settings.export"),
        "help names the control it explains"
    );
    assert!(node.visible);
    assert!(!node.disabled);
}

#[gpui::test]
fn the_control_stays_usable_without_hovering_it(cx: &mut TestAppContext) {
    let mut harness = scene(cx);
    let control = harness.node("settings.export").expect("published");

    assert_eq!(control.role, Role::Button);
    assert_eq!(control.text.as_deref(), Some("Export theme"));
    assert!(
        !control.disabled,
        "help is never the only way to reach an action"
    );
}

#[gpui::test]
fn help_leaves_with_the_pointer(cx: &mut TestAppContext) {
    let mut harness = scene(cx);
    hover(&mut harness, "settings.export");
    assert!(harness.node("settings.export.tooltip").is_some());

    harness.context().simulate_mouse_move(
        gpui::point(px(600.0), px(600.0)),
        None,
        Modifiers::none(),
    );
    harness.advance(RESTED);

    assert!(harness.node("settings.export.tooltip").is_none());
}
