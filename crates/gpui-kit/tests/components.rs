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

#[gpui::test]
fn a_selected_button_publishes_its_selection(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        div()
            .child(
                Button::new("view.list")
                    .label("List")
                    .secondary()
                    .selected(true)
                    .on_click(|_, _| {}),
            )
            .child(
                Button::new("view.grid")
                    .label("Grid")
                    .secondary()
                    .on_click(|_, _| {}),
            )
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    assert_eq!(
        present(&snapshot, "view.list").expect("node").checked,
        Some(true)
    );
    assert_eq!(present(&snapshot, "view.grid").expect("node").checked, None);
}

#[gpui::test]
fn switching_the_theme_repaints_components_with_the_new_appearance(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, cx| {
        let theme = cx.theme();
        div()
            .child(Callout::new(theme.id.clone(), Tone::Neutral).id("appearance"))
            .into_any_element()
    });

    text(&harness.snapshot(), "appearance", "studio-dark").expect("dark is the default theme");

    harness.update(|_, cx| {
        assert!(activate_theme("studio-light", cx));
    });

    text(&harness.snapshot(), "appearance", "studio-light")
        .expect("switching the theme rebuilds the tree");
}

#[gpui::test]
fn compact_density_shrinks_a_rendered_control(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        Button::new("settings.save")
            .label("Save")
            .primary()
            .on_click(|_, _| {})
            .into_any_element()
    });

    let comfortable = harness.bounds("settings.save").expect("measured button");

    harness.update(|_, cx| set_density(Density::Compact, cx));

    let compact = harness.bounds("settings.save").expect("measured button");
    assert!(compact.size.height < comfortable.size.height);
}

#[gpui::test]
fn a_card_becomes_an_action_only_when_it_is_given_one(cx: &mut TestAppContext) {
    let taken = Rc::new(Cell::new(0));
    let sink = taken.clone();
    let mut harness = harness(cx, move |_, _| {
        let sink = sink.clone();
        div()
            .child(Card::new().id("plans.free").child(div().child("Free")))
            .child(
                Card::new()
                    .id("plans.team")
                    .on_click(move |_, _| sink.set(sink.get() + 1))
                    .child(div().child("Team")),
            )
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    assert_eq!(
        present(&snapshot, "plans.free").expect("card").role,
        Role::Group,
        "a card nobody can act on stays a grouping"
    );
    assert_eq!(
        present(&snapshot, "plans.team").expect("card").role,
        Role::Button
    );

    harness.click("plans.free");
    assert_eq!(taken.get(), 0);
    harness.click("plans.team");
    assert_eq!(taken.get(), 1);
}

#[gpui::test]
fn an_actionable_row_reports_itself_and_stays_a_row(cx: &mut TestAppContext) {
    let taken = Rc::new(Cell::new(0));
    let sink = taken.clone();
    let mut harness = harness(cx, move |_, _| {
        let sink = sink.clone();
        Card::new()
            .id("providers")
            .child(
                ListRow::new()
                    .id("providers.anthropic")
                    .first(true)
                    .on_click(move |_, _| sink.set(sink.get() + 1))
                    .child(div().child("Anthropic")),
            )
            .into_any_element()
    });

    assert_eq!(
        harness.node("providers.anthropic").expect("row").role,
        Role::Row,
        "acting on a row does not stop it being a row"
    );
    harness.click("providers.anthropic");
    assert_eq!(taken.get(), 1);
}
