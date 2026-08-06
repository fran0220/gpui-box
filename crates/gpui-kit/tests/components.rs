//! Behavior tests for the component baseline.
//!
//! Every assertion goes through the semantic tree or simulated input, so the
//! tests stay true when the internal element structure changes.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{IntoElement, TestAppContext, div, prelude::*, px};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;
use gpui_kit_testkit::{actionable, present, text, visible};

fn harness(
    cx: &mut TestAppContext,
    build: impl Fn(&mut gpui::Window, &mut gpui::App) -> gpui::AnyElement + 'static,
) -> Harness {
    Harness::new(cx, gpui_kit::install, build)
}

#[gpui::test]
fn a_button_publishes_its_label_and_role(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        Button::new("settings.save")
            .label("Save")
            .primary()
            .on_click(|_, _| {})
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    let node = visible(&snapshot, "settings.save").expect("button is visible");
    assert_eq!(node.role, Role::Button);
    text(&snapshot, "settings.save", "Save").expect("label is the accessible name");
    actionable(&snapshot, "settings.save").expect("an enabled button is actionable");
}

#[gpui::test]
fn a_disabled_button_never_fires(cx: &mut TestAppContext) {
    let fired = Rc::new(Cell::new(false));
    let recorder = Rc::clone(&fired);
    let mut harness = harness(cx, move |_, _| {
        let recorder = Rc::clone(&recorder);
        Button::new("settings.save")
            .label("Save")
            .disabled(true)
            .on_click(move |_, _| recorder.set(true))
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    assert!(
        present(&snapshot, "settings.save").expect("node").disabled,
        "a disabled button reports itself as disabled"
    );
    assert!(actionable(&snapshot, "settings.save").is_err());

    harness.click("settings.save");
    assert!(
        !fired.get(),
        "a disabled button must not install its action"
    );
}

#[gpui::test]
fn a_loading_button_is_busy_and_inert(cx: &mut TestAppContext) {
    let fired = Rc::new(Cell::new(false));
    let recorder = Rc::clone(&fired);
    let mut harness = harness(cx, move |_, _| {
        let recorder = Rc::clone(&recorder);
        Button::new("settings.save")
            .label("Saving")
            .loading(true)
            .on_click(move |_, _| recorder.set(true))
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    let node = present(&snapshot, "settings.save").expect("node");
    assert!(node.busy, "an in-flight action reports busy");
    assert!(node.disabled);

    harness.click("settings.save");
    assert!(!fired.get());
}

#[gpui::test]
fn an_enabled_button_fires_once_per_click(cx: &mut TestAppContext) {
    let count = Rc::new(Cell::new(0_usize));
    let recorder = Rc::clone(&count);
    let mut harness = harness(cx, move |_, _| {
        let recorder = Rc::clone(&recorder);
        Button::new("settings.save")
            .label("Save")
            .on_click(move |_, _| recorder.set(recorder.get() + 1))
            .into_any_element()
    });

    harness.click("settings.save");
    assert_eq!(count.get(), 1);
    harness.click("settings.save");
    assert_eq!(count.get(), 2);
}

#[gpui::test]
fn control_sizes_change_measured_height(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        div()
            .flex()
            .flex_col()
            .child(Button::new("small").label("Small").small())
            .child(Button::new("large").label("Large").large())
            .into_any_element()
    });

    let small = harness.node("small").expect("small button");
    let large = harness.node("large").expect("large button");
    assert!(
        large.bounds.height > small.bounds.height,
        "large {:?} should exceed small {:?}",
        large.bounds,
        small.bounds
    );
}

#[gpui::test]
fn rows_report_selection_and_keep_business_ids(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        Card::new()
            .id("providers")
            .child(
                ListRow::new()
                    .id("providers.anthropic")
                    .first(true)
                    .child(div().child("Anthropic")),
            )
            .child(
                ListRow::new()
                    .id("providers.openai")
                    .selected(true)
                    .child(div().child("OpenAI")),
            )
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    assert_eq!(
        present(&snapshot, "providers").expect("card").role,
        Role::Group
    );
    assert!(
        !present(&snapshot, "providers.anthropic")
            .expect("row")
            .selected
    );
    assert!(
        present(&snapshot, "providers.openai")
            .expect("row")
            .selected
    );
}

#[gpui::test]
fn a_callout_preserves_the_hosts_exact_wording(cx: &mut TestAppContext) {
    const REFUSAL: &str = "The host refused: model catalog is not configured.";
    let mut harness = harness(cx, |_, _| {
        Callout::new(REFUSAL, Tone::Danger)
            .id("refusal")
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    text(&snapshot, "refusal", REFUSAL).expect("refusal text is preserved verbatim");
}

#[gpui::test]
fn decorative_badges_stay_out_of_the_semantic_tree(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        div()
            .child(Badge::new("Ready").success())
            .child(Badge::new("Stale").warning().id("row.state"))
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    assert_eq!(snapshot.ids(), vec!["row.state"]);
    text(&snapshot, "row.state", "Stale").expect("identified badges publish their label");
}

#[gpui::test]
fn credentials_never_reach_a_snapshot(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        div()
            .w(px(320.0))
            .child(Callout::new("sk-live-not-a-real-key", Tone::Danger).id("leak"))
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    assert_eq!(
        present(&snapshot, "leak").expect("node").text.as_deref(),
        Some("[REDACTED]")
    );
}
