//! Behavior tests for the component baseline.
//!
//! Every assertion goes through the semantic tree or simulated input, so the
//! tests stay true when the internal element structure changes.

use std::cell::{Cell, RefCell};
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

/// A tint answers whose the mark is; the tone still answers how it is going.
/// Painting cannot edit the claim, which is the only thing that makes a
/// caller-owned colour safe on a status surface.
#[gpui::test]
fn a_tinted_mark_still_reports_the_severity_it_claims(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, cx| {
        let tint = cx.theme().colors.info;
        div()
            .child(Badge::new("Ada").tint(tint).id("roster.ada.badge"))
            .child(
                StatusLine::new("Reviewing", Tone::Warning)
                    .tint(tint)
                    .busy("roster.ada.state")
                    .id("roster.ada.state"),
            )
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    // An untoned badge that got a colour is still making no severity claim.
    assert_eq!(
        visible(&snapshot, "roster.ada.badge")
            .expect("badge is visible")
            .value
            .as_deref(),
        Some("neutral")
    );
    let line = visible(&snapshot, "roster.ada.state").expect("status line is visible");
    assert_eq!(line.value.as_deref(), Some("warning"));
    assert!(line.busy, "a running line reports busy whatever it wears");
    text(&snapshot, "roster.ada.state", "Reviewing").expect("the label is the accessible name");
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
fn a_disabled_card_says_so_and_never_fires(cx: &mut TestAppContext) {
    let taken = Rc::new(Cell::new(0));
    let sink = taken.clone();
    let mut harness = harness(cx, move |_, _| {
        let sink = sink.clone();
        Card::new()
            .id("plans.enterprise")
            .disabled(true)
            .on_click(move |_, _| sink.set(sink.get() + 1))
            .child(div().child("Enterprise"))
            .into_any_element()
    });

    let node = harness.node("plans.enterprise").expect("card");
    assert!(
        node.disabled,
        "a card that refuses must publish the refusal"
    );
    assert_eq!(
        node.role,
        Role::Group,
        "an unavailable card is not a button anybody can reach"
    );
    harness.click("plans.enterprise");
    assert_eq!(taken.get(), 0);
}

#[gpui::test]
fn a_card_publishes_its_selection(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        div()
            .child(Card::new().id("plans.free").child(div().child("Free")))
            .child(
                Card::new()
                    .id("plans.team")
                    .selected(true)
                    .child(div().child("Team")),
            )
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    assert!(!present(&snapshot, "plans.free").expect("card").selected);
    assert!(present(&snapshot, "plans.team").expect("card").selected);
}

/// A card that is one action renders no second target inside itself, because
/// a click whose outcome depends on which pixel it landed on is not an action
/// a reader can predict or a test can address.
#[gpui::test]
fn a_header_action_withdraws_from_a_card_that_is_itself_an_action(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        div()
            .child(Card::new().id("runs.inert").header(
                CardHeader::new("Nightly").subtitle("Green").action(|_, _| {
                    Button::new("runs.inert.menu")
                        .label("Menu")
                        .on_click(|_, _| {})
                        .into_any_element()
                }),
            ))
            .child(Card::new().id("runs.open").on_click(|_, _| {}).header(
                CardHeader::new("Release").subtitle("Green").action(|_, _| {
                    Button::new("runs.open.menu")
                        .label("Menu")
                        .on_click(|_, _| {})
                        .into_any_element()
                }),
            ))
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    assert!(
        visible(&snapshot, "runs.inert.menu").is_ok(),
        "a grouping card carries its heading control"
    );
    assert!(
        visible(&snapshot, "runs.open.menu").is_err(),
        "a card that is one action drops the second target"
    );
}

#[gpui::test]
fn a_disabled_row_stays_a_row_and_installs_nothing(cx: &mut TestAppContext) {
    let taken = Rc::new(Cell::new(0));
    let sink = taken.clone();
    let mut harness = harness(cx, move |_, _| {
        let sink = sink.clone();
        Card::new()
            .id("providers")
            .child(
                ListRow::new()
                    .id("providers.retired")
                    .disabled(true)
                    .on_click(move |_, _| sink.set(sink.get() + 1))
                    .child(div().child("Retired")),
            )
            .into_any_element()
    });

    let node = harness.node("providers.retired").expect("row");
    assert_eq!(node.role, Role::Row);
    assert!(node.disabled);
    harness.click("providers.retired");
    assert_eq!(taken.get(), 0);
}

/// The slots exist so rows with and without one still line their text up.
#[gpui::test]
fn row_slots_hold_their_place_across_rows(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        Card::new()
            .id("providers")
            .child(
                ListRow::new()
                    .id("providers.anthropic")
                    .leading(div().w(px(16.0)).h(px(16.0)))
                    .child(div().flex_1().child("Anthropic"))
                    .trailing(div().child("Ready")),
            )
            .child(
                ListRow::new()
                    .id("providers.openai")
                    .leading(div().w(px(16.0)).h(px(16.0)))
                    .child(div().flex_1().child("OpenAI"))
                    .trailing(div().child("Ready")),
            )
            .into_any_element()
    });

    let first = harness.bounds("providers.anthropic").expect("first row");
    let second = harness.bounds("providers.openai").expect("second row");
    assert_eq!(
        first.origin.x, second.origin.x,
        "rows carrying the same slots start at the same edge"
    );
    assert_eq!(first.size.width, second.size.width);
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

/// A slot replaces a node the component authored, which is the difference
/// between configuring a component and composing with it.
#[gpui::test]
fn a_filled_slot_replaces_the_node_the_component_would_have_authored(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        UploadList::new("attachments")
            .slot(slot::EMPTY, |_, _| {
                Callout::new("Drop a build log here", Tone::Info)
                    .id("attachments.hint")
                    .into_any_element()
            })
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    visible(&snapshot, "attachments.hint").expect("the caller's node is the one rendered");
    assert!(
        visible(&snapshot, "attachments.empty").is_err(),
        "the component's own empty state withdraws rather than rendering behind"
    );
}

#[gpui::test]
fn an_unfilled_slot_leaves_the_component_its_own_node(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| UploadList::new("attachments").into_any_element());

    let snapshot = harness.snapshot();
    visible(&snapshot, "attachments.empty").expect("the component still answers for itself");
}

#[gpui::test]
#[should_panic(expected = "is not a slot this component names")]
fn a_slot_the_component_does_not_name_is_reported(cx: &mut TestAppContext) {
    harness(cx, |_, _| {
        UploadList::new("attachments")
            .slot("footer", |_, _| div().into_any_element())
            .into_any_element()
    });
}

/// The first frame reports the container as unmeasured, and the frame the
/// measurement asks for reports its width. Both are stated rather than
/// guessed, which is the whole point of the type.
#[gpui::test]
fn a_container_reports_its_own_width_from_the_frame_after_it_was_laid_out(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<ContainerSize>>> = Rc::new(RefCell::new(Vec::new()));
    let recorder = Rc::clone(&seen);
    let mut harness = harness(cx, move |_, _| {
        let recorder = Rc::clone(&recorder);
        div()
            .w(px(640.0))
            .child(Responsive::new("settings.body", move |size, _, _| {
                recorder.borrow_mut().push(size);
                div().child("body").into_any_element()
            }))
            .into_any_element()
    });

    harness.frame();
    harness.frame();

    let seen = seen.borrow();
    assert_eq!(
        seen.first(),
        Some(&ContainerSize::Unmeasured),
        "nothing has been laid out before the first layout"
    );
    let settled = seen
        .iter()
        .rev()
        .find_map(|size| size.width())
        .expect("a later frame carries the measured width");
    assert!(
        (settled - 640.0).abs() < 1.0,
        "the container reports the room it was given, not {settled}"
    );
}

#[gpui::test]
fn a_filling_container_reports_both_axes_of_its_parent(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<ContainerSize>>> = Rc::new(RefCell::new(Vec::new()));
    let recorder = Rc::clone(&seen);
    let mut harness = harness(cx, move |_, _| {
        let recorder = Rc::clone(&recorder);
        div()
            .w(px(640.0))
            .h(px(420.0))
            .child(
                Responsive::new("workspace.body", move |size, _, _| {
                    recorder.borrow_mut().push(size);
                    div().absolute().inset_0().into_any_element()
                })
                .fill(),
            )
            .into_any_element()
    });

    harness.frame();
    harness.frame();

    let measured = seen
        .borrow()
        .iter()
        .rev()
        .copied()
        .find(|size| size.width().is_some())
        .expect("a later frame carries the measured bounds");
    assert_eq!(measured.width(), Some(640.0));
    assert_eq!(measured.height(), Some(420.0));
}

#[gpui::test]
fn an_unmeasured_container_claims_neither_width(_cx: &mut TestAppContext) {
    let unmeasured = ContainerSize::Unmeasured;
    assert!(!unmeasured.at_least(320.0));
    assert!(!unmeasured.narrower_than(320.0));
    assert_eq!(unmeasured.width(), None);

    let measured = ContainerSize::Measured {
        width: 640.0,
        height: 48.0,
    };
    assert!(measured.at_least(640.0));
    assert!(!measured.narrower_than(640.0));
    assert!(measured.narrower_than(641.0));
}

/// An override reaches every component in its subtree without any of them
/// knowing about it, and reaches nothing outside it.
#[gpui::test]
fn a_theme_overlay_covers_its_subtree_and_stops_there(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        div()
            .child(
                Card::new()
                    .id("plans.standard")
                    .padded(true)
                    .child(div().child("Standard")),
            )
            .child(ThemeOverlay::new(
                |theme| {
                    theme.clone().modify(|theme| {
                        theme.spacing.lg *= 3.0;
                    })
                },
                Card::new()
                    .id("plans.brand")
                    .padded(true)
                    .child(div().child("Brand")),
            ))
            .into_any_element()
    });

    let plain = harness.bounds("plans.standard").expect("measured");
    let overridden = harness.bounds("plans.brand").expect("measured");
    assert!(
        overridden.size.height > plain.size.height,
        "the overlaid card reads the adjusted spacing: {overridden:?} against {plain:?}"
    );

    let after = harness.bounds("plans.standard").expect("measured");
    assert_eq!(
        plain.size, after.size,
        "a sibling outside the overlay is untouched by it"
    );
}

#[gpui::test]
fn an_overlay_restores_the_theme_it_found(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        div()
            .child(ThemeOverlay::new(
                |theme| {
                    theme.clone().modify(|theme| {
                        theme.spacing.lg *= 3.0;
                    })
                },
                Card::new()
                    .id("plans.brand")
                    .padded(true)
                    .child(div().child("Brand")),
            ))
            // Rendered after the overlay, so it sees whatever the overlay left
            // behind rather than what it was handed.
            .child(Card::new().id("plans.after").child(div().child("After")))
            .into_any_element()
    });

    let overridden = harness.bounds("plans.brand").expect("measured");
    let after = harness.bounds("plans.after").expect("measured");
    assert!(
        after.size.height < overridden.size.height,
        "the override was popped: {after:?} against {overridden:?}"
    );
}

/// A retry belongs to the row that failed, not to the column every row shares.
/// While it stood in that column it moved the size beside it by its own width,
/// so the sizes read down a ragged edge instead of a line.
#[gpui::test]
fn every_upload_row_dismisses_from_the_same_column(cx: &mut TestAppContext) {
    let mut harness = harness(cx, |_, _| {
        UploadList::new("attachments")
            .uploads([
                Upload::new("brief", "brief.pdf").size("1.2 MB").done(),
                Upload::new("capture", "capture.png")
                    .size("4.8 MB")
                    .uploading(0.4),
                Upload::new("archive", "archive.zip")
                    .size("240 MB")
                    .failed("The connection dropped."),
            ])
            .on_retry(|_, _, _| {})
            .on_cancel(|_, _, _| {})
            .on_remove(|_, _, _| {})
            .into_any_element()
    });

    let settled = harness
        .bounds("attachments.brief.remove")
        .expect("laid out");
    let running = harness
        .bounds("attachments.capture.cancel")
        .expect("laid out");
    let failed = harness
        .bounds("attachments.archive.remove")
        .expect("laid out");

    assert_eq!(settled.origin.x, running.origin.x);
    assert_eq!(
        settled.origin.x, failed.origin.x,
        "the row carrying a retry pulled the dismiss column out of line"
    );

    let retry = harness
        .bounds("attachments.archive.retry")
        .expect("laid out");
    assert!(
        retry.right() < failed.origin.x,
        "the retry reached into the column every row dismisses from"
    );
}

#[gpui::test]
fn a_prompt_slot_answers_the_keyboard_and_a_disabled_one_does_not(cx: &mut TestAppContext) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut active = harness(cx, move |_, _| {
        let sink = Rc::clone(&sink);
        PromptBuilder::new("prompt", "Review")
            .body("Review {path}")
            .slots([PromptSlot::new("path", "path").value("src/lib.rs")])
            .on_slot(move |slot, _, _| sink.borrow_mut().push(slot.id.to_string()))
            .into_any_element()
    });

    let slot = active.node("prompt.slot.path").expect("published");
    assert_eq!(slot.role, Role::Button);
    assert!(!slot.disabled);
    active.click("prompt.slot.path");
    calls.borrow_mut().clear();
    active.keystrokes("enter");
    assert_eq!(*calls.borrow(), vec!["path"]);

    drop(active);
    calls.borrow_mut().clear();
    let blocked = Rc::clone(&calls);
    let mut disabled = harness(cx, move |_, _| {
        let blocked = Rc::clone(&blocked);
        PromptBuilder::new("disabled-prompt", "Review")
            .body("Review {path}")
            .slots([PromptSlot::new("path", "path")])
            .disabled(true)
            .on_slot(move |slot, _, _| blocked.borrow_mut().push(slot.id.to_string()))
            .into_any_element()
    });

    assert!(
        disabled
            .node("disabled-prompt.slot.path")
            .expect("published")
            .disabled
    );
    disabled.click("disabled-prompt.slot.path");
    disabled.keystrokes("enter");
    assert!(calls.borrow().is_empty());
}
