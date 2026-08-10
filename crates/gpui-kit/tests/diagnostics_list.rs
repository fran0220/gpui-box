//! Simulated-input coverage for the caller-owned diagnostics surface.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, SharedString, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Calls<T> = Rc<RefCell<Vec<T>>>;

#[derive(Clone)]
struct Recorded {
    filters: Calls<DiagnosticFilter>,
    selections: Calls<String>,
    actions: Calls<(String, String)>,
}

impl Recorded {
    fn new() -> Self {
        Self {
            filters: Rc::new(RefCell::new(Vec::new())),
            selections: Rc::new(RefCell::new(Vec::new())),
            actions: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

fn fixture(id: &str, severity: DiagnosticSeverity, line: u32) -> Diagnostic {
    Diagnostic::new(
        id,
        severity,
        DiagnosticLocation::new(format!("fixture.rs:{line}")),
        SharedString::from(format!("Fixture diagnostic {id}")),
    )
    .action(DiagnosticAction::new("apply", "Apply fixture action"))
}

fn diagnostics() -> Vec<Diagnostic> {
    vec![
        fixture("error-a", DiagnosticSeverity::Error, 12),
        fixture("warning-b", DiagnosticSeverity::Warning, 24).disabled(true),
        fixture("information-c", DiagnosticSeverity::Information, 36).action(
            DiagnosticAction::new("disabled-action", "Unavailable fixture action").disabled(true),
        ),
        fixture("hint-d", DiagnosticSeverity::Hint, 48),
    ]
}

fn ready(
    cx: &mut TestAppContext,
    selected: &'static str,
    filter: DiagnosticFilter,
    disabled: bool,
) -> (Harness, Recorded) {
    let recorded = Recorded::new();
    let sink = recorded.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let filters = sink.filters.clone();
        let selections = sink.selections.clone();
        let actions = sink.actions.clone();
        DiagnosticsList::new("fixture.diagnostics", Loadable::Ready(diagnostics()))
            .filter(filter)
            .selected(selected)
            .visible_rows(6)
            .disabled(disabled)
            .on_filter(move |filter, _, _| filters.borrow_mut().push(filter))
            .on_select(move |id, _, _| selections.borrow_mut().push(id.to_string()))
            .on_action(move |diagnostic, action, _, _| {
                actions
                    .borrow_mut()
                    .push((diagnostic.to_string(), action.to_string()))
            })
            .into_any_element()
    });
    (harness, recorded)
}

#[test]
fn filter_values_are_caller_owned() {
    let filter =
        DiagnosticFilter::from_severities([DiagnosticSeverity::Error, DiagnosticSeverity::Warning]);
    let diagnostic = fixture("fixture-error", DiagnosticSeverity::Error, 12);

    assert!(filter.includes(&diagnostic));
    assert!(
        !filter
            .removing(DiagnosticSeverity::Error)
            .includes(&diagnostic)
    );
    assert!(filter.includes(&diagnostic));
    assert_eq!(filter.clearing(), DiagnosticFilter::none());
}

#[gpui::test]
fn selection_and_actions_report_ids_without_applying_or_bubbling(cx: &mut TestAppContext) {
    let (mut harness, recorded) = ready(cx, "information-c", DiagnosticFilter::all(), false);

    let row = harness
        .node("fixture.diagnostics.list.error-a")
        .expect("diagnostic row");
    assert_eq!(row.role, Role::Row);
    assert_eq!(row.text.as_deref(), Some("Fixture diagnostic error-a"));
    harness.click("fixture.diagnostics.list.error-a");
    assert_eq!(*recorded.selections.borrow(), vec!["error-a"]);
    assert!(
        harness
            .node("fixture.diagnostics.list.information-c")
            .expect("caller selection")
            .selected
    );

    recorded.selections.borrow_mut().clear();
    harness.click("fixture.diagnostics.list.error-a.action.apply");
    assert_eq!(
        *recorded.actions.borrow(),
        vec![("error-a".to_string(), "apply".to_string())]
    );
    assert!(
        recorded.selections.borrow().is_empty(),
        "an action does not also open or select its diagnostic"
    );
}

#[gpui::test]
fn list_keyboard_skips_a_disabled_diagnostic(cx: &mut TestAppContext) {
    let (mut harness, recorded) = ready(cx, "error-a", DiagnosticFilter::all(), false);

    harness.click("fixture.diagnostics.list.error-a");
    recorded.selections.borrow_mut().clear();
    harness.keystrokes("down");

    assert_eq!(*recorded.selections.borrow(), vec!["information-c"]);
    assert!(
        harness
            .node("fixture.diagnostics.list.warning-b")
            .expect("disabled row")
            .disabled
    );
}

#[gpui::test]
fn filter_remove_clear_and_toggle_report_new_values_without_applying_them(cx: &mut TestAppContext) {
    let filter = DiagnosticFilter::from_severities([DiagnosticSeverity::Error]);
    let (mut harness, recorded) = ready(cx, "error-a", filter, false);

    harness.click("fixture.diagnostics.filters.error.remove");
    assert_eq!(*recorded.filters.borrow(), vec![DiagnosticFilter::none()]);
    assert!(
        harness.node("fixture.diagnostics.filters.error").is_some(),
        "the caller has not applied removal"
    );

    recorded.filters.borrow_mut().clear();
    harness.click("fixture.diagnostics.filters.severities.warning");
    assert_eq!(
        *recorded.filters.borrow(),
        vec![DiagnosticFilter::from_severities([
            DiagnosticSeverity::Error,
            DiagnosticSeverity::Warning,
        ])]
    );

    recorded.filters.borrow_mut().clear();
    harness.click("fixture.diagnostics.filters.clear");
    assert_eq!(*recorded.filters.borrow(), vec![DiagnosticFilter::none()]);
}

fn state(cx: &mut TestAppContext, value: Loadable<Vec<Diagnostic>, SharedString>) -> Harness {
    Harness::new(cx, gpui_kit::install, move |_, _| {
        DiagnosticsList::new("fixture.diagnostics", value.clone())
            .on_retry(|_, _| {})
            .into_any_element()
    })
}

#[gpui::test]
fn loading_empty_unavailable_error_and_no_match_remain_distinct(cx: &mut TestAppContext) {
    let mut loading = state(cx, Loadable::Loading);
    assert!(
        loading
            .node("fixture.diagnostics.loading")
            .expect("loading")
            .busy
    );

    let mut empty = state(cx, Loadable::Empty);
    assert_eq!(
        empty
            .node("fixture.diagnostics.empty")
            .expect("empty")
            .value
            .as_deref(),
        Some("empty")
    );

    let mut unavailable = state(
        cx,
        Loadable::Unavailable("Synthetic host refusal".to_string()),
    );
    assert_eq!(
        unavailable
            .node("fixture.diagnostics.unavailable")
            .expect("unavailable")
            .value
            .as_deref(),
        Some("unavailable")
    );

    let mut error = state(cx, Loadable::Error("Synthetic query failure".into()));
    assert_eq!(
        error
            .node("fixture.diagnostics.error")
            .expect("error")
            .value
            .as_deref(),
        Some("failed")
    );
    assert!(error.node("fixture.diagnostics.retry").is_some());

    let (mut no_match, _) = ready(cx, "error-a", DiagnosticFilter::none(), false);
    assert_eq!(
        no_match
            .node("fixture.diagnostics.no-match")
            .expect("filtered empty")
            .value
            .as_deref(),
        Some("empty")
    );
    assert!(no_match.node("fixture.diagnostics.empty").is_none());
}

#[gpui::test]
fn retry_is_reported_and_disabled_surfaces_install_no_handlers(cx: &mut TestAppContext) {
    let retries: Calls<()> = Rc::new(RefCell::new(Vec::new()));
    let sink = retries.clone();
    let mut failed = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        DiagnosticsList::new(
            "fixture.diagnostics",
            Loadable::Error("Synthetic query failure".into()),
        )
        .on_retry(move |_, _| sink.borrow_mut().push(()))
        .into_any_element()
    });
    failed.click("fixture.diagnostics.retry");
    assert_eq!(retries.borrow().len(), 1);

    let (mut disabled, recorded) = ready(cx, "error-a", DiagnosticFilter::all(), true);
    disabled.click("fixture.diagnostics.list.error-a");
    disabled.click("fixture.diagnostics.list.error-a.action.apply");
    assert!(
        disabled
            .node("fixture.diagnostics.filters.error.remove")
            .is_none(),
        "a disabled filter has no removal action"
    );
    disabled.click("fixture.diagnostics.filters.severities.warning");
    disabled.keystrokes("down");
    assert!(recorded.selections.borrow().is_empty());
    assert!(recorded.actions.borrow().is_empty());
    assert!(recorded.filters.borrow().is_empty());
}

#[gpui::test]
fn diagnostic_ids_survive_caller_reordering(cx: &mut TestAppContext) {
    let rows = Rc::new(RefCell::new(diagnostics()));
    let render_rows = rows.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        DiagnosticsList::new(
            "fixture.diagnostics",
            Loadable::Ready(render_rows.borrow().clone()),
        )
        .into_any_element()
    });
    assert!(harness.node("fixture.diagnostics.list.error-a").is_some());

    harness.update(|window, _| {
        rows.borrow_mut().reverse();
        window.refresh();
    });

    assert!(harness.node("fixture.diagnostics.list.error-a").is_some());
    assert!(harness.node("fixture.diagnostics.list.0").is_none());
}
