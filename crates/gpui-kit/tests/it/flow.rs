//! Wizard, FilterBar, and InlineEdit report where the typist wants to go and
//! what they typed. None of them applies any of it.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{InteractiveElement, IntoElement, ParentElement, SharedString, Styled, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Calls<T> = Rc<RefCell<Vec<T>>>;

fn recorder<T: 'static>() -> (Calls<T>, Calls<T>) {
    let calls: Calls<T> = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

// ------------------------------------------------------------------- wizard

fn release_steps() -> Vec<WizardStep> {
    vec![
        WizardStep::new("prepare", "Prepare").complete(),
        WizardStep::new("build", "Build").failed("The build failed on the test target."),
        WizardStep::new("sign", "Sign").current(),
        WizardStep::new("publish", "Publish").blocked("Approval is required."),
    ]
}

fn wizard(cx: &mut TestAppContext, build: fn(Wizard) -> Wizard) -> (Harness, Calls<String>) {
    let (calls, sink) = recorder::<String>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        build(
            Wizard::new("release.flow")
                .steps(release_steps())
                .body(gpui::div().child(SharedString::new_static("Signing the artifacts"))),
        )
        .on_navigate(move |intent, _, _| {
            let described = match intent {
                WizardIntent::Step(id) => format!("step:{id}"),
                other => other.as_str().to_string(),
            };
            sink.borrow_mut().push(described);
        })
        .into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn a_step_publishes_the_status_the_caller_gave_it(cx: &mut TestAppContext) {
    let (mut harness, _calls) = wizard(cx, |wizard| wizard);

    assert_eq!(
        harness.node("release.flow").expect("published").value,
        Some("4".into())
    );
    let current = harness.node("release.flow.sign").expect("published");
    assert_eq!(current.role, Role::Tab);
    assert!(current.selected);
    assert_eq!(current.value.as_deref(), Some("current"));
    assert_eq!(
        harness
            .node("release.flow.prepare")
            .expect("published")
            .value
            .as_deref(),
        Some("complete")
    );
}

#[gpui::test]
fn a_blocked_step_shows_its_reason_rather_than_a_grey_dot(cx: &mut TestAppContext) {
    let (mut harness, _calls) = wizard(cx, |wizard| wizard);

    let blocked = harness
        .node("release.flow.publish.reason")
        .expect("a blocked step publishes why");
    assert_eq!(blocked.text.as_deref(), Some("Approval is required."));
    assert!(!blocked.invalid, "blocked is not the same as failed");

    let failed = harness
        .node("release.flow.build.reason")
        .expect("a failed step publishes why");
    assert_eq!(
        failed.text.as_deref(),
        Some("The build failed on the test target.")
    );
    assert!(failed.invalid);
}

#[gpui::test]
fn an_unreachable_step_installs_no_handler(cx: &mut TestAppContext) {
    let (mut harness, calls) = wizard(cx, |wizard| wizard);

    harness.click("release.flow.publish");
    harness.click("release.flow.build");

    assert!(
        calls.borrow().is_empty(),
        "a step nobody may jump to reports nothing"
    );
    assert!(
        harness
            .node("release.flow.publish")
            .expect("published")
            .disabled
    );
    assert!(
        harness
            .node("release.flow.build")
            .expect("published")
            .disabled
    );
}

#[gpui::test]
fn a_completed_step_may_be_revisited_and_reports_the_jump(cx: &mut TestAppContext) {
    let (mut harness, calls) = wizard(cx, |wizard| wizard);

    harness.click("release.flow.prepare");

    assert_eq!(*calls.borrow(), vec!["step:prepare".to_string()]);
    // The wizard moved nothing: the caller still owns which step is current.
    assert!(
        harness
            .node("release.flow.sign")
            .expect("published")
            .selected
    );
}

#[gpui::test]
fn a_step_the_caller_marks_reachable_becomes_operable(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<String>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Wizard::new("release.flow")
            .steps([
                WizardStep::new("prepare", "Prepare").complete(),
                WizardStep::new("build", "Build")
                    .failed("The build failed.")
                    .reachable(true),
                WizardStep::new("sign", "Sign").current(),
            ])
            .on_navigate(move |intent, _, _| {
                if let WizardIntent::Step(id) = intent {
                    sink.borrow_mut().push(id.to_string());
                }
            })
            .into_any_element()
    });

    harness.click("release.flow.build");

    assert_eq!(*calls.borrow(), vec!["build".to_string()]);
    assert!(
        !harness
            .node("release.flow.build")
            .expect("published")
            .disabled,
        "a step the caller opened is operable however it ended"
    );
}

#[gpui::test]
fn finishing_is_a_different_report_from_advancing(cx: &mut TestAppContext) {
    let (mut harness, calls) = wizard(cx, |wizard| wizard);
    harness.click("release.flow.next");
    assert_eq!(*calls.borrow(), vec!["next".to_string()]);

    let (mut harness, calls) = wizard(cx, |wizard| wizard.finish(true));
    assert!(
        harness.node("release.flow.next").is_none(),
        "a finishing flow offers no next"
    );
    harness.click("release.flow.finish");
    assert_eq!(*calls.borrow(), vec!["finish".to_string()]);
}

#[gpui::test]
fn back_exists_only_where_the_caller_named_somewhere_to_go(cx: &mut TestAppContext) {
    let (mut harness, calls) = wizard(cx, |wizard| wizard);
    assert!(
        harness.node("release.flow.back").is_none(),
        "no revisitable step means no back control"
    );

    let (mut harness, calls_back) = wizard(cx, |wizard| wizard.back_to("prepare"));
    harness.click("release.flow.back");

    assert!(calls.borrow().is_empty());
    assert_eq!(*calls_back.borrow(), vec!["back".to_string()]);
    assert_eq!(
        harness
            .node("release.flow.back-target")
            .expect("published")
            .value
            .as_deref(),
        Some("prepare"),
        "the step back returns to is the caller's fact and is published"
    );
}

#[gpui::test]
fn a_refused_flow_installs_nothing_at_all(cx: &mut TestAppContext) {
    let (mut harness, calls) = wizard(cx, |wizard| wizard.back_to("prepare").disabled(true));

    harness.click("release.flow.prepare");

    assert!(calls.borrow().is_empty());
    assert!(
        harness.node("release.flow.back").is_none(),
        "a refused flow offers no way on"
    );
    assert!(harness.node("release.flow.next").is_none());
}

// --------------------------------------------------------------- filter bar

fn conditions() -> Vec<FilterCondition> {
    vec![
        FilterCondition::new("status", "Status", "is", "failed"),
        FilterCondition::new("owner", "Owner", "is", "fixture-owner"),
        FilterCondition::new("started", "Started", "after", "09:00"),
    ]
}

struct BarReports {
    removed: Calls<String>,
    cleared: Calls<String>,
    added: Calls<String>,
}

fn filter_bar(cx: &mut TestAppContext, count: ResultCount) -> (Harness, Rc<BarReports>) {
    let reports = Rc::new(BarReports {
        removed: Rc::new(RefCell::new(Vec::new())),
        cleared: Rc::new(RefCell::new(Vec::new())),
        added: Rc::new(RefCell::new(Vec::new())),
    });
    let sinks = Rc::clone(&reports);
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sinks = Rc::clone(&sinks);
        let count = count.clone();
        FilterBar::new("runs.filters")
            .conditions(conditions())
            .count(count)
            .noun("runs")
            .on_add({
                let sink = Rc::clone(&sinks);
                move |_, _| sink.added.borrow_mut().push("add".to_string())
            })
            .on_remove({
                let sink = Rc::clone(&sinks);
                move |id, _, _| sink.removed.borrow_mut().push(id.to_string())
            })
            .on_clear({
                let sink = Rc::clone(&sinks);
                move |_, _| sink.cleared.borrow_mut().push("clear".to_string())
            })
            .into_any_element()
    });
    (harness, reports)
}

#[gpui::test]
fn every_condition_is_a_chip_that_reports_its_own_removal(cx: &mut TestAppContext) {
    let (mut harness, reports) = filter_bar(cx, ResultCount::Known(14));

    assert_eq!(
        harness.node("runs.filters").expect("published").value,
        Some("3".into())
    );
    assert_eq!(
        harness
            .node("runs.filters.owner")
            .expect("published")
            .text
            .as_deref(),
        Some("Owner is fixture-owner")
    );

    harness.click("runs.filters.owner.remove");

    assert_eq!(*reports.removed.borrow(), vec!["owner".to_string()]);
    // The bar removed nothing: the conditions are still the caller's.
    assert!(harness.node("runs.filters.owner").is_some());
}

#[gpui::test]
fn counting_is_a_different_state_from_zero(cx: &mut TestAppContext) {
    let (mut harness, _reports) = filter_bar(cx, ResultCount::Counting);
    let counting = harness.node("runs.filters.count").expect("published");
    assert_eq!(counting.value.as_deref(), Some("counting"));
    assert!(counting.busy);

    let (mut harness, _reports) = filter_bar(cx, ResultCount::Known(0));
    let zero = harness.node("runs.filters.count").expect("published");
    assert_eq!(zero.value.as_deref(), Some("known"));
    assert_eq!(zero.text.as_deref(), Some("0 runs"));
    assert!(!zero.busy);
}

#[gpui::test]
fn a_count_nobody_established_is_not_rendered_at_all(cx: &mut TestAppContext) {
    let (mut harness, _reports) = filter_bar(cx, ResultCount::Unknown);

    assert!(
        harness.node("runs.filters.count").is_none(),
        "the bar never guesses a number it was not given"
    );
}

#[gpui::test]
fn a_count_the_host_could_not_take_is_shown_as_the_refusal_it_is(cx: &mut TestAppContext) {
    let (mut harness, _reports) = filter_bar(
        cx,
        ResultCount::Unavailable(SharedString::new_static("The host refused to count.")),
    );

    let node = harness.node("runs.filters.count").expect("published");
    assert_eq!(node.value.as_deref(), Some("unavailable"));
    assert_eq!(node.text.as_deref(), Some("The host refused to count."));
}

#[gpui::test]
fn adding_and_clearing_report_and_change_nothing(cx: &mut TestAppContext) {
    let (mut harness, reports) = filter_bar(cx, ResultCount::Known(14));

    harness.click("runs.filters.add");
    harness.click("runs.filters.clear");

    assert_eq!(*reports.added.borrow(), vec!["add".to_string()]);
    assert_eq!(*reports.cleared.borrow(), vec!["clear".to_string()]);
    assert_eq!(
        harness.node("runs.filters").expect("published").value,
        Some("3".into())
    );
}

#[gpui::test]
fn a_refused_bar_installs_no_handler(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<String>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        FilterBar::new("runs.filters")
            .conditions(conditions())
            .count(ResultCount::Known(14))
            .disabled(true)
            .on_add({
                let sink = sink.clone();
                move |_, _| sink.borrow_mut().push("add".to_string())
            })
            .on_remove(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .on_clear(|_, _| {})
            .into_any_element()
    });

    assert!(harness.node("runs.filters.add").is_none());
    assert!(harness.node("runs.filters.clear").is_none());
    assert!(
        harness.node("runs.filters.owner.remove").is_none(),
        "a refused chip offers no way to remove it"
    );
    assert!(calls.borrow().is_empty());
}

// -------------------------------------------------------------- inline edit

#[derive(Default)]
struct EditReports {
    edits: Calls<String>,
    commits: Calls<String>,
    cancels: Calls<String>,
}

fn inline_edit(
    cx: &mut TestAppContext,
    build: fn(InlineEdit) -> InlineEdit,
) -> (Harness, Rc<EditReports>) {
    let reports = Rc::new(EditReports::default());
    let sinks = Rc::clone(&reports);
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sinks = Rc::clone(&sinks);
        build(InlineEdit::new("run.title", "Indexing"))
            .on_edit({
                let sink = Rc::clone(&sinks);
                move |_, _| sink.edits.borrow_mut().push("edit".to_string())
            })
            .on_commit({
                let sink = Rc::clone(&sinks);
                move |value, _, _| sink.commits.borrow_mut().push(value.to_string())
            })
            .on_cancel({
                let sink = Rc::clone(&sinks);
                move |_, _| sink.cancels.borrow_mut().push("cancel".to_string())
            })
            .into_any_element()
    });
    (harness, reports)
}

#[gpui::test]
fn reading_text_reports_the_request_and_opens_nothing(cx: &mut TestAppContext) {
    let (mut harness, reports) = inline_edit(cx, |edit| edit);

    let reading = harness.node("run.title").expect("published");
    assert_eq!(reading.role, Role::Button);
    assert_eq!(reading.text.as_deref(), Some("Indexing"));
    assert_eq!(reading.value.as_deref(), Some("reading"));

    harness.click("run.title");

    assert_eq!(*reports.edits.borrow(), vec!["edit".to_string()]);
    // The component never opens itself: the caller owns whether it is editing.
    assert!(harness.node("run.title.field").is_none());
    assert_eq!(
        harness
            .node("run.title")
            .expect("published")
            .value
            .as_deref(),
        Some("reading")
    );
}

#[gpui::test]
fn reading_text_wraps_only_when_the_caller_says_it_is_multiline(cx: &mut TestAppContext) {
    const VALUE: &str = "A lantern-lit harbour at dusk, tall ships waiting beyond the breakwater";
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        gpui::div()
            .w(gpui::px(200.0))
            .flex()
            .flex_col()
            .child(
                InlineEdit::new("prompt.single", VALUE)
                    .on_edit(|_, _| {})
                    .into_any_element(),
            )
            .child(
                InlineEdit::new("prompt.multiline", VALUE)
                    .multiline(true)
                    .on_edit(|_, _| {})
                    .into_any_element(),
            )
            .into_any_element()
    });

    let single = harness.node("prompt.single").expect("single-line reading");
    let multiline = harness
        .node("prompt.multiline")
        .expect("multi-line reading");
    assert!(
        multiline.bounds.height > single.bounds.height,
        "multi-line reading {:?} should wrap beyond one row {:?}",
        multiline.bounds,
        single.bounds
    );
    assert_eq!(multiline.text.as_deref(), Some(VALUE));
}

#[gpui::test]
fn a_single_line_value_yields_to_the_pen_inside_a_bounded_parent(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        gpui::div()
            .w(gpui::px(200.0))
            .debug_selector(|| "prompt.parent".to_string())
            .child(
                InlineEdit::new(
                    "prompt.single",
                    "A lantern-lit harbour at dusk, tall ships waiting beyond the breakwater",
                )
                .on_edit(|_, _| {}),
            )
            .into_any_element()
    });

    let parent = harness
        .context()
        .debug_bounds("prompt.parent")
        .expect("bounded parent");
    let value = harness
        .context()
        .debug_bounds("prompt.single.reading-value")
        .expect("reading value");
    let pen = harness
        .context()
        .debug_bounds("prompt.single.pen")
        .expect("edit affordance");

    assert!(value.right() <= pen.left(), "the value yields to the pen");
    assert!(
        pen.right() <= parent.right(),
        "the pen {pen:?} stays inside its parent {parent:?}"
    );
}

#[gpui::test]
fn an_empty_value_offers_its_placeholder_as_something_to_aim_at(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<String>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        InlineEdit::new("run.title", "")
            .placeholder("Name this run")
            .on_edit(move |_, _| sink.borrow_mut().push("edit".to_string()))
            .into_any_element()
    });

    let node = harness.node("run.title").expect("published");
    assert_eq!(node.placeholder.as_deref(), Some("Name this run"));
    assert_eq!(node.value.as_deref(), Some("empty"));

    harness.click("run.title");
    assert_eq!(*calls.borrow(), vec!["edit".to_string()]);
}

#[gpui::test]
fn a_refused_field_installs_no_handler_and_never_opens(cx: &mut TestAppContext) {
    let (mut harness, reports) = inline_edit(cx, |edit| edit.editing(true).disabled(true));

    harness.click("run.title");

    assert!(reports.edits.borrow().is_empty());
    assert!(
        harness.node("run.title.field").is_none(),
        "a refused field does not open even when the caller says it is editing"
    );
    let node = harness.node("run.title").expect("published");
    assert_eq!(node.role, Role::Text);
    assert!(node.disabled);
}

#[gpui::test]
fn an_open_field_commits_what_was_typed_and_writes_nothing(cx: &mut TestAppContext) {
    let (mut harness, reports) = inline_edit(cx, |edit| edit.editing(true));

    assert!(harness.node("run.title.field").is_some());
    harness.keystrokes("space x");
    harness.keystrokes("enter");

    assert_eq!(*reports.commits.borrow(), vec!["Indexing x".to_string()]);
    // The value on screen is still the caller's until the caller changes it.
    assert_eq!(
        harness
            .node("run.title")
            .expect("published")
            .text
            .as_deref(),
        Some("Indexing")
    );
}

#[gpui::test]
fn escape_abandons_the_edit_and_reports_no_value(cx: &mut TestAppContext) {
    let (mut harness, reports) = inline_edit(cx, |edit| edit.editing(true));

    harness.keystrokes("space x");
    harness.keystrokes("escape");

    assert_eq!(*reports.cancels.borrow(), vec!["cancel".to_string()]);
    assert!(reports.commits.borrow().is_empty());
}

#[gpui::test]
fn a_save_that_failed_keeps_the_typed_text(cx: &mut TestAppContext) {
    let failed: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let flag = Rc::clone(&failed);
    let (calls, sink) = recorder::<String>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        let mut edit = InlineEdit::new("run.title", "Indexing").editing(true);
        if *flag.borrow() {
            edit = edit.failure("The host refused this change.");
        }
        edit.on_commit(move |value, _, _| sink.borrow_mut().push(value.to_string()))
            .on_cancel(|_, _| {})
            .into_any_element()
    });

    harness.keystrokes("space x");
    harness.keystrokes("enter");
    assert_eq!(*calls.borrow(), vec!["Indexing x".to_string()]);

    // The host refuses, and answers by rendering the failure with the field
    // still open.
    *failed.borrow_mut() = true;
    harness.frame();

    let field = harness
        .node("run.title.field")
        .expect("the field is still open");
    assert_eq!(
        field.value.as_deref(),
        Some("Indexing x"),
        "a failed save must not take back what was typed"
    );
    let failure = harness.node("run.title.failure").expect("published");
    assert_eq!(
        failure.text.as_deref(),
        Some("The host refused this change.")
    );
    assert!(failure.invalid);
    assert_eq!(
        harness
            .node("run.title")
            .expect("published")
            .value
            .as_deref(),
        Some("failed")
    );
}

#[gpui::test]
fn a_multiline_edit_uses_the_area_and_keeps_enter_for_a_new_line(cx: &mut TestAppContext) {
    let (mut harness, reports) = inline_edit(cx, |edit| edit.editing(true).multiline(true).rows(2));

    harness.keystrokes("enter");

    assert!(
        reports.commits.borrow().is_empty(),
        "enter inserts a line in a multi-line field"
    );
    assert!(harness.node("run.title.field").is_some());
}
