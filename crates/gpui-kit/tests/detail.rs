//! What a detail page is allowed to claim: SettingsRow, DescriptionList,
//! Timeline, and ProgressCircle.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, ParentElement, SharedString, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Calls = Rc<RefCell<Vec<String>>>;

// ------------------------------------------------------------ settings rows

fn switch_row(id: &'static str, label: &'static str, sink: Calls) -> SettingsRow {
    SettingsRow::new(id, label).control(
        Switch::new(format!("{id}.switch"))
            .on(true)
            .on_change(move |on, _, _| sink.borrow_mut().push(format!("{id}:{on}"))),
    )
}

#[gpui::test]
fn a_settings_row_publishes_its_label_and_the_value_it_was_given(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        SettingsSection::new("settings.general", "General")
            .description("How this workspace behaves")
            .row(
                SettingsRow::new("settings.general.autosave", "Save automatically")
                    .description("Write changes as they happen")
                    .value("On")
                    .control(Switch::new("settings.general.autosave.switch").on(true)),
            )
            .into_any_element()
    });

    let section = harness.node("settings.general").expect("published");
    assert_eq!(section.role, Role::Group);
    assert_eq!(section.text.as_deref(), Some("General"));
    assert!(!section.disabled);

    let row = harness
        .node("settings.general.autosave")
        .expect("published");
    assert_eq!(row.role, Role::Row);
    assert_eq!(row.text.as_deref(), Some("Save automatically"));
    assert_eq!(row.value.as_deref(), Some("On"));
    assert!(
        harness.node("settings.general.autosave.switch").is_some(),
        "an ordinary row renders the control it was handed"
    );
}

#[gpui::test]
fn a_managed_row_states_who_decides_and_renders_no_control(cx: &mut TestAppContext) {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        SettingsSection::new("settings.general", "General")
            .row(
                SettingsRow::new("settings.general.telemetry", "Usage reporting")
                    .value("Off")
                    .managed("your administrator")
                    .control(
                        Switch::new("settings.general.telemetry.switch")
                            .on(false)
                            .on_change(move |_, _, _| {
                                sink.borrow_mut().push("changed".to_string())
                            }),
                    ),
            )
            .into_any_element()
    });

    assert!(
        harness.node("settings.general.telemetry.switch").is_none(),
        "a managed row must not put an inoperable control on screen"
    );
    let status = harness
        .node("settings.general.telemetry.managed")
        .expect("published");
    assert_eq!(status.value.as_deref(), Some("managed"));
    assert_eq!(
        status.text.as_deref(),
        Some("Managed by your administrator")
    );
    assert!(
        harness
            .node("settings.general.telemetry")
            .expect("published")
            .disabled
    );
    harness.click("settings.general.telemetry");
    assert!(calls.borrow().is_empty());
}

#[gpui::test]
fn a_dimmed_section_says_why_and_withholds_every_control(cx: &mut TestAppContext) {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        SettingsSection::new("settings.sync", "Synchronisation")
            .dimmed_by("This workspace is local, so nothing synchronises.")
            .row(switch_row(
                "settings.sync.settings",
                "Sync settings",
                Rc::clone(&sink),
            ))
            .row(switch_row(
                "settings.sync.history",
                "Sync history",
                Rc::clone(&sink),
            ))
            .into_any_element()
    });

    let reason = harness.node("settings.sync.dimmed").expect("published");
    assert_eq!(
        reason.text.as_deref(),
        Some("This workspace is local, so nothing synchronises.")
    );
    assert_eq!(reason.value.as_deref(), Some("inapplicable"));
    assert!(harness.node("settings.sync").expect("published").disabled);
    for row in ["settings.sync.settings", "settings.sync.history"] {
        assert!(harness.node(row).expect("published").disabled);
        assert!(
            harness.node(&format!("{row}.switch")).is_none(),
            "a dimmed section installs no handler anywhere"
        );
        assert_eq!(
            harness
                .node(&format!("{row}.managed"))
                .expect("published")
                .value
                .as_deref(),
            Some("inapplicable")
        );
    }
    assert!(calls.borrow().is_empty());
}

#[gpui::test]
fn a_dimmed_section_drops_the_heading_action_it_cannot_honour(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        SettingsSection::new("settings.sync", "Synchronisation")
            .dimmed_by("This workspace is local.")
            .action(|_, _| {
                Button::new("settings.sync.now")
                    .label("Sync now")
                    .on_click(|_, _| {})
                    .into_any_element()
            })
            .row(SettingsRow::new("settings.sync.settings", "Sync settings"))
            .into_any_element()
    });

    assert!(
        harness.node("settings.sync.now").is_none(),
        "an action that cannot apply is not offered"
    );
}

// ---------------------------------------------------------- description list

fn facts(cx: &mut TestAppContext) -> (Harness, Calls) {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let harness = Harness::new(cx, gpui_kit::install, move |_, cx| {
        let sink = Rc::clone(&sink);
        DescriptionList::new("run.facts")
            .items([
                DescriptionItem::new("id", "Run", "run-4821").copyable(true),
                DescriptionItem::new("finished", "Finished", DescriptionValue::Unknown),
                DescriptionItem::new("artifact", "Artifact", DescriptionValue::NotApplicable),
                DescriptionItem::new(
                    "token",
                    "Access token",
                    DescriptionValue::redacted_from("s3cr3t", cx),
                )
                .copyable(true),
            ])
            .on_copy(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn unknown_and_not_applicable_are_two_different_facts(cx: &mut TestAppContext) {
    let (mut harness, _calls) = facts(cx);

    assert_eq!(
        harness.node("run.facts").expect("published").value,
        Some("4".to_string())
    );
    assert_eq!(
        harness
            .node("run.facts.finished")
            .expect("published")
            .value
            .as_deref(),
        Some("unknown")
    );
    assert_eq!(
        harness
            .node("run.facts.artifact")
            .expect("published")
            .value
            .as_deref(),
        Some("not applicable")
    );
    assert_eq!(
        harness
            .node("run.facts.id")
            .expect("published")
            .value
            .as_deref(),
        Some("run-4821")
    );
}

#[gpui::test]
fn a_redacted_value_publishes_its_shape_and_never_its_text(cx: &mut TestAppContext) {
    let (mut harness, _calls) = facts(cx);

    let node = harness.node("run.facts.token").expect("published");
    assert_eq!(node.value.as_deref(), Some("redacted, 6 characters"));

    let snapshot = harness.snapshot();
    let leaked = snapshot.nodes.iter().any(|node| {
        [
            node.text.as_deref(),
            node.value.as_deref(),
            node.labels.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|text| text.contains("s3cr3t"))
    });
    assert!(!leaked, "a secret must not reach the semantic tree");
}

#[gpui::test]
fn copying_reports_the_item_and_touches_no_clipboard(cx: &mut TestAppContext) {
    let (mut harness, calls) = facts(cx);

    harness.click("run.facts.id.copy");

    assert_eq!(*calls.borrow(), vec!["id".to_string()]);
    assert!(
        harness.node("run.facts.finished.copy").is_none(),
        "there is nothing to copy from a value nobody knows"
    );
    assert!(
        harness.node("run.facts.artifact.copy").is_none(),
        "a question that does not arise has no answer to copy"
    );
}

// ---------------------------------------------------------------- timeline

fn activity(cx: &mut TestAppContext) -> Harness {
    Harness::new(cx, gpui_kit::install, |_, _| {
        Timeline::new("run.activity")
            .group(
                TimelineGroup::new("today", "Today")
                    .entry(
                        TimelineEntry::new("queued", "Run queued")
                            .time("09:12")
                            .actor("fixture-owner"),
                    )
                    .entry(
                        TimelineEntry::new("failed", "Indexing failed")
                            .time("09:41")
                            .actor("scheduler")
                            .tone(Tone::Danger)
                            .detail(
                                gpui::div().child(SharedString::new_static("The host refused.")),
                            ),
                    ),
            )
            .group(
                TimelineGroup::new("earlier", "Earlier")
                    .entry(TimelineEntry::new("imported", "Workspace imported").time_unknown()),
            )
            .into_any_element()
    })
}

#[gpui::test]
fn every_entry_is_addressable_by_its_own_identity(cx: &mut TestAppContext) {
    let mut harness = activity(cx);

    assert_eq!(
        harness.node("run.activity").expect("published").value,
        Some("3".to_string())
    );
    let entry = harness.node("run.activity.queued").expect("published");
    assert_eq!(entry.role, Role::Row);
    assert_eq!(entry.text.as_deref(), Some("Run queued"));
    assert_eq!(
        entry.value.as_deref(),
        Some("09:12"),
        "the time is the caller's string, published as given"
    );
    assert_eq!(
        harness
            .node("run.activity.queued.actor")
            .expect("published")
            .text
            .as_deref(),
        Some("fixture-owner")
    );
}

#[gpui::test]
fn an_entry_with_no_known_time_says_so_rather_than_guessing(cx: &mut TestAppContext) {
    let mut harness = activity(cx);

    assert_eq!(
        harness
            .node("run.activity.imported")
            .expect("published")
            .value
            .as_deref(),
        Some("time unknown")
    );
    assert!(
        harness.node("run.activity.imported.actor").is_none(),
        "an entry nobody attributed publishes no actor"
    );
}

#[gpui::test]
fn a_day_heading_is_the_words_the_caller_chose(cx: &mut TestAppContext) {
    let mut harness = activity(cx);

    let heading = harness.node("run.activity.today").expect("published");
    assert_eq!(heading.role, Role::Heading);
    assert_eq!(heading.text.as_deref(), Some("Today"));
    assert_eq!(heading.value.as_deref(), Some("2"));
    assert_eq!(
        harness
            .node("run.activity.earlier")
            .expect("published")
            .value
            .as_deref(),
        Some("1")
    );
}

#[gpui::test]
fn ungrouped_entries_keep_the_order_they_arrived_in(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        Timeline::new("run.activity")
            .entries([
                TimelineEntry::new("first", "Queued").time("09:12"),
                TimelineEntry::new("second", "Started").time("09:13"),
            ])
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    let order: Vec<&str> = snapshot
        .nodes
        .iter()
        .filter(|node| node.role == Role::Row)
        .map(|node| node.id.as_str())
        .collect();
    assert_eq!(order, vec!["run.activity.first", "run.activity.second"]);
}

// --------------------------------------------------------- progress circle

#[gpui::test]
fn a_ring_publishes_a_position_only_when_the_extent_is_known(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        gpui::div()
            .child(
                ProgressCircle::new("upload.ring")
                    .count(3, 12)
                    .label("Uploading artifacts"),
            )
            .child(ProgressCircle::new("contact.ring").label("Contacting the host"))
            .into_any_element()
    });

    let known = harness.node("upload.ring").expect("published");
    assert_eq!(known.role, Role::Progress);
    assert!(known.busy);
    assert_eq!(known.value_now, Some(0.25));
    assert_eq!(known.value_min, Some(0.0));
    assert_eq!(known.value_max, Some(1.0));
    assert_eq!(known.value.as_deref(), Some("3 of 12"));
    assert_eq!(known.text.as_deref(), Some("Uploading artifacts"));

    let unknown = harness.node("contact.ring").expect("published");
    assert!(unknown.busy, "unknown extent is still work in progress");
    assert_eq!(
        unknown.value_now, None,
        "a ring with no extent must not invent a position"
    );
}

#[gpui::test]
fn a_ring_and_a_bar_report_the_same_work_the_same_way(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        gpui::div()
            .child(
                ProgressCircle::new("work.ring")
                    .count(3, 12)
                    .label("Uploading"),
            )
            .child(ProgressBar::new("work.bar").count(3, 12).label("Uploading"))
            .into_any_element()
    });

    let ring = harness.node("work.ring").expect("published").clone();
    let bar = harness.node("work.bar").expect("published").clone();
    assert_eq!(ring.role, bar.role);
    assert_eq!(ring.value_now, bar.value_now);
    assert_eq!(ring.value, bar.value);
    assert_eq!(ring.text, bar.text);
    assert_eq!(ring.busy, bar.busy);
}

#[gpui::test]
fn a_total_of_zero_leaves_the_extent_unknown(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ProgressCircle::new("work.ring")
            .count(0, 0)
            .label("Uploading")
            .into_any_element()
    });

    let node = harness.node("work.ring").expect("published");
    assert_eq!(node.value_now, None, "nothing out of nothing is not zero");
    assert_eq!(node.value.as_deref(), Some("0 of 0"));
}

#[gpui::test]
fn a_fraction_beyond_the_ends_is_held_to_them(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        gpui::div()
            .child(ProgressCircle::new("over.ring").fraction(4.0))
            .child(ProgressCircle::new("under.ring").fraction(-1.0))
            .into_any_element()
    });

    assert_eq!(
        harness.node("over.ring").expect("published").value_now,
        Some(1.0)
    );
    assert_eq!(
        harness.node("under.ring").expect("published").value_now,
        Some(0.0)
    );
}

#[gpui::test]
fn the_reading_in_the_middle_is_the_callers_words(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ProgressCircle::new("work.ring")
            .fraction(0.5)
            .display("halfway")
            .centre("1/2")
            .into_any_element()
    });

    assert_eq!(
        harness
            .node("work.ring")
            .expect("published")
            .value
            .as_deref(),
        Some("halfway"),
        "the ring never renders a percentage nobody asked for"
    );
}
