//! Permission and cost surfaces say exactly what they know.
//!
//! Four cell states rather than two, provenance on every cell that has a
//! state, a read-only matrix that installs nothing, an estimate labelled
//! everywhere it appears, no proportion of an unknown total, and a failed
//! refresh that keeps the last verified value.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

fn actions() -> Vec<PermissionAction> {
    vec![
        PermissionAction::new("read", "Read files"),
        PermissionAction::new("network", "Reach the network"),
    ]
}

fn subjects() -> Vec<PermissionSubject> {
    vec![
        PermissionSubject::new("workspace", "This workspace")
            .cell("read", PermissionEntry::new(PermissionState::Allowed))
            .cell(
                "network",
                PermissionEntry::inherited(PermissionState::Denied, "the organisation policy"),
            ),
        // States nothing about reading, because a calculator has no files.
        PermissionSubject::new("calculator", "The calculator tool")
            .cell("network", PermissionEntry::new(PermissionState::Ask)),
    ]
}

#[gpui::test]
fn four_cell_states_present_differently(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        PermissionMatrix::new("permissions")
            .actions(actions())
            .subjects(subjects())
            .on_change(|_, _, _| {})
            .into_any_element()
    });

    let allowed = harness
        .node("permissions.workspace.read")
        .expect("published");
    let denied = harness
        .node("permissions.workspace.network")
        .expect("published");
    let ask = harness
        .node("permissions.calculator.network")
        .expect("published");
    let inapplicable = harness
        .node("permissions.calculator.read")
        .expect("published");

    assert_eq!(allowed.value.as_deref(), Some("allowed"));
    assert_eq!(denied.value.as_deref(), Some("denied"));
    assert_eq!(ask.value.as_deref(), Some("ask"));
    assert_eq!(inapplicable.value.as_deref(), Some("not-applicable"));

    let names = [allowed, denied, ask, inapplicable].map(|node| node.text.expect("named"));
    let mut unique = names.to_vec();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 4, "four states read as four sentences");
}

#[gpui::test]
fn not_applicable_is_not_denied(cx: &mut TestAppContext) {
    let changes: Rc<RefCell<Vec<PermissionChange>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        PermissionMatrix::new("permissions")
            .actions(actions())
            .subjects(subjects())
            .on_change(move |change, _, _| sink.borrow_mut().push(change))
            .into_any_element()
    });

    let inapplicable = harness
        .node("permissions.calculator.read")
        .expect("published");
    let denied = harness
        .node("permissions.workspace.network")
        .expect("published");
    assert_ne!(inapplicable.value, denied.value);
    assert_ne!(inapplicable.text, denied.text);

    // A question that does not arise cannot be answered by clicking, even in
    // an editable matrix.
    harness.click("permissions.calculator.read");
    assert!(
        changes.borrow().is_empty(),
        "a not-applicable cell reported a change"
    );
    assert_eq!(
        PermissionState::NotApplicable.next(),
        None,
        "not applicable has no next state to cycle to"
    );
}

#[gpui::test]
fn a_cell_says_where_its_state_came_from(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        PermissionMatrix::new("permissions")
            .actions(actions())
            .subjects(subjects())
            .into_any_element()
    });

    let here = harness
        .node("permissions.workspace.read.source")
        .expect("published");
    let inherited = harness
        .node("permissions.workspace.network.source")
        .expect("published");
    assert_eq!(here.value.as_deref(), Some("here"));
    assert_eq!(inherited.value.as_deref(), Some("inherited"));
    assert!(
        inherited
            .text
            .as_deref()
            .expect("worded")
            .contains("the organisation policy"),
        "an inherited cell names the rule it came from"
    );
    assert!(
        harness.node("permissions.calculator.read.source").is_none(),
        "a state nobody set has no provenance to claim"
    );
}

#[gpui::test]
fn a_read_only_matrix_installs_no_handler(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        PermissionMatrix::new("permissions")
            .actions(actions())
            .subjects(subjects())
            .into_any_element()
    });

    let cell = harness
        .node("permissions.workspace.read")
        .expect("published");
    assert_eq!(
        cell.role,
        gpui_kit::semantics::Role::Cell,
        "a read-only cell is not published as something to operate"
    );
    // Nothing to assert about a handler that was never installed except that
    // operating the cell does nothing, which is what the click checks.
    harness.click("permissions.workspace.read");
    assert_eq!(
        harness
            .node("permissions.workspace.read")
            .expect("published")
            .value
            .as_deref(),
        Some("allowed"),
        "a read-only matrix changed nothing, because it applies nothing"
    );
}

#[gpui::test]
fn an_editable_cell_reports_the_next_state_and_applies_none(cx: &mut TestAppContext) {
    let changes: Rc<RefCell<Vec<PermissionChange>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        PermissionMatrix::new("permissions")
            .actions(actions())
            .subjects(subjects())
            .on_change(move |change, _, _| sink.borrow_mut().push(change))
            .into_any_element()
    });

    harness.click("permissions.workspace.read");
    assert_eq!(
        changes.borrow().as_slice(),
        [PermissionChange {
            subject: "workspace".into(),
            action: "read".into(),
            next: PermissionState::Ask,
        }]
    );
    assert_eq!(
        harness
            .node("permissions.workspace.read")
            .expect("published")
            .value
            .as_deref(),
        Some("allowed"),
        "the matrix reports the request and keeps showing what still holds"
    );
}

#[gpui::test]
fn an_estimate_is_labelled_wherever_it_appears(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        CostMeter::new("cost")
            .line(CostLine::new(
                "spend",
                "Spend",
                Reading::measured(1.24, "1.24 credits"),
            ))
            .line(CostLine::new(
                "projected",
                "Projected",
                Reading::estimated(4.0, "4.00 credits"),
            ))
            .into_any_element()
    });

    let measured = harness.node("cost.spend").expect("published");
    let estimated = harness.node("cost.projected").expect("published");
    assert_eq!(measured.value.as_deref(), Some("1.24 credits"));
    assert_eq!(
        estimated.value.as_deref(),
        Some("4.00 credits (estimated)"),
        "the semantic tree carries the label, not just the pixels"
    );
    assert!(
        harness.node("cost.projected.estimate").is_some(),
        "an estimate carries a mark beside the number"
    );
    assert!(
        harness.node("cost.spend.estimate").is_none(),
        "a measured number is not marked as an estimate"
    );
}

#[gpui::test]
fn unavailable_is_not_zero(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        CostMeter::new("cost")
            .line(CostLine::new(
                "storage",
                "Storage",
                Reading::unavailable_because("The billing host refused the request."),
            ))
            .into_any_element()
    });

    let node = harness.node("cost.storage").expect("published");
    let value = node.value.as_deref().expect("a state, not a blank");
    assert_eq!(value, "The billing host refused the request.");
    assert!(!value.contains('0'), "a refusal was drawn as a number");
    assert_eq!(Reading::unavailable().name(), "unavailable");
    assert!(Reading::unavailable().quantity().is_none());
}

#[gpui::test]
fn a_known_limit_draws_a_proportion(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ContextGauge::new("context", Reading::measured(48_000.0, "48,000 tokens"))
            .label("Context used")
            .limit(Limit::measured(128_000.0, "128,000 tokens"))
            .into_any_element()
    });

    let node = harness.node("context").expect("published");
    assert_eq!(node.value_min, Some(0.0));
    assert_eq!(node.value_max, Some(1.0));
    assert_eq!(node.value_now, Some(0.375));
    assert_eq!(
        node.value.as_deref(),
        Some("48,000 tokens of 128,000 tokens")
    );
}

#[gpui::test]
fn an_unknown_limit_draws_no_proportion(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ContextGauge::new("context", Reading::estimated(48_000.0, "48,000 tokens"))
            .label("Context used")
            .into_any_element()
    });

    let node = harness.node("context").expect("published");
    assert_eq!(
        node.value_now, None,
        "a proportion of an unknown total is invented"
    );
    assert_ne!(
        node.role,
        gpui_kit::semantics::Role::Progress,
        "a range role without a range claims a position it does not have"
    );
    assert_eq!(
        node.value.as_deref(),
        Some("48,000 tokens (estimated)"),
        "the estimate is still labelled with no limit to compare it to"
    );
    let limit = harness.node("context.limit").expect("published");
    assert_eq!(limit.value.as_deref(), Some("unknown-limit"));
}

#[gpui::test]
fn an_unavailable_reading_draws_no_proportion(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ContextGauge::new("context", Reading::unavailable())
            .label("Context used")
            .limit(Limit::measured(128_000.0, "128,000 tokens"))
            .into_any_element()
    });

    let node = harness.node("context").expect("published");
    assert_eq!(node.value_now, None);
    assert_eq!(node.value.as_deref(), Some("Unavailable of 128,000 tokens"));
}

#[gpui::test]
fn a_failed_refresh_keeps_the_last_value_and_says_when(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        CostMeter::new("cost")
            .line(
                CostLine::new(
                    "account",
                    "Account balance",
                    Reading::measured(112.0, "112.00 credits"),
                )
                .stale(LastVerified::at("09:41 today")),
            )
            .into_any_element()
    });

    let node = harness.node("cost.account").expect("published");
    assert_eq!(
        node.value.as_deref(),
        Some("112.00 credits"),
        "the last verified value stays on screen"
    );
    let stale = harness.node("cost.account.stale").expect("published");
    assert_eq!(stale.value.as_deref(), Some("stale"));
    assert_eq!(
        stale.text.as_deref(),
        Some("Last verified 09:41 today"),
        "a stale value says when it was from"
    );
}

#[gpui::test]
fn a_host_replaces_the_estimate_wording_without_forking_the_component(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        CostMeter::new("cost")
            .line(CostLine::new(
                "projected",
                "Projected",
                Reading::estimated(4.0, "4.00 credits"),
            ))
            .into_any_element()
    });

    harness.update(|_, cx| {
        gpui_kit::strings::set_strings(
            [(
                gpui_kit::strings::StringKey::CostEstimated,
                "about {0}".into(),
            )],
            cx,
        );
    });

    assert_eq!(
        harness
            .node("cost.projected")
            .expect("published")
            .value
            .as_deref(),
        Some("about 4.00 credits")
    );
    harness.update(|_, cx| gpui_kit::strings::reset_strings(cx));
}
