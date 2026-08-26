//! An approval prompt defaults to refusal, states the scope of every
//! "always", and keeps declined, expired and superseded apart.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, TestAppContext, prelude::*};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

fn scene(cx: &mut TestAppContext, status: ApprovalStatus) -> (Harness, Entity<ApprovalPrompt>) {
    let slot: Rc<RefCell<Option<Entity<ApprovalPrompt>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let status = status.clone();
        let prompt = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    ApprovalPrompt::new(
                        "approval.write",
                        "Write to /work/report/summary.md",
                        window,
                        cx,
                    )
                    .detail(DescriptionItem::new(
                        "path",
                        "Path",
                        "/work/report/summary.md",
                    ))
                    .always(AlwaysScope::Session)
                    .always(AlwaysScope::path("/work/report"))
                    .status(status)
                })
            })
            .clone();
        prompt.into_any_element()
    });
    harness.snapshot();
    let prompt = slot.borrow().clone().expect("built");
    (harness, prompt)
}

fn events(
    harness: &mut Harness,
    prompt: &Entity<ApprovalPrompt>,
) -> Rc<RefCell<Vec<ApprovalEvent>>> {
    let seen: Rc<RefCell<Vec<ApprovalEvent>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    harness.update({
        let prompt = prompt.clone();
        move |_, cx| {
            cx.subscribe(&prompt, move |_, event: &ApprovalEvent, _| {
                sink.borrow_mut().push(event.clone());
            })
            .detach();
        }
    });
    seen
}

#[gpui::test]
fn the_keyboard_lands_on_decline(cx: &mut TestAppContext) {
    let (mut harness, _prompt) = scene(cx, ApprovalStatus::Pending);

    let decline = harness.node("approval.write.decline").expect("published");
    let approve = harness.node("approval.write.approve").expect("published");
    assert!(decline.focused, "the default answer holds the keyboard");
    assert!(!approve.focused, "approval is never focused by default");
}

#[gpui::test]
fn return_at_rest_declines(cx: &mut TestAppContext) {
    let (mut harness, prompt) = scene(cx, ApprovalStatus::Pending);
    let seen = events(&mut harness, &prompt);

    harness.keystrokes("enter");
    assert_eq!(seen.borrow().as_slice(), [ApprovalEvent::Declined]);
}

#[gpui::test]
fn escape_declines(cx: &mut TestAppContext) {
    let (mut harness, prompt) = scene(cx, ApprovalStatus::Pending);
    let seen = events(&mut harness, &prompt);

    harness.keystrokes("escape");
    assert_eq!(seen.borrow().as_slice(), [ApprovalEvent::Declined]);
}

#[gpui::test]
fn approving_takes_a_deliberate_act(cx: &mut TestAppContext) {
    let (mut harness, prompt) = scene(cx, ApprovalStatus::Pending);
    let seen = events(&mut harness, &prompt);

    harness.click("approval.write.approve");
    assert_eq!(
        seen.borrow().as_slice(),
        [ApprovalEvent::Approved(ApprovalDecision::Once)]
    );
}

#[gpui::test]
fn return_approves_only_once_approval_holds_the_keyboard(cx: &mut TestAppContext) {
    let (mut harness, prompt) = scene(cx, ApprovalStatus::Pending);
    let seen = events(&mut harness, &prompt);

    harness.keystrokes("tab");
    assert!(
        harness
            .node("approval.write.approve")
            .expect("published")
            .focused,
        "the test only means something once approval has the keyboard"
    );
    harness.keystrokes("enter");
    assert_eq!(
        seen.borrow().as_slice(),
        [ApprovalEvent::Approved(ApprovalDecision::Once)]
    );
}

#[gpui::test]
fn every_always_states_its_scope(cx: &mut TestAppContext) {
    let (mut harness, prompt) = scene(cx, ApprovalStatus::Pending);
    let seen = events(&mut harness, &prompt);

    let session = harness
        .node("approval.write.always.session")
        .expect("published");
    let path = harness
        .node("approval.write.always.path")
        .expect("published");
    let standing = harness
        .node("approval.write.standing")
        .expect("standing grants are grouped and named");
    assert_eq!(standing.role, gpui_kit::semantics::Role::Group);
    assert_eq!(standing.text.as_deref(), Some("Standing approvals"));
    assert_eq!(session.text.as_deref(), Some("Always for this session"));
    assert_eq!(path.text.as_deref(), Some("Always in /work/report"));
    assert_eq!(session.parent.as_deref(), Some("approval.write.standing"));
    assert_eq!(path.parent.as_deref(), Some("approval.write.standing"));

    harness.click("approval.write.always.path");
    assert_eq!(
        seen.borrow().as_slice(),
        [ApprovalEvent::Approved(ApprovalDecision::Always(
            AlwaysScope::path("/work/report")
        ))]
    );
}

#[gpui::test]
fn an_always_scope_always_names_what_it_covers(_cx: &mut TestAppContext) {
    // There is no `AlwaysScope` variant meaning "always, everywhere": the
    // three that name something take that name as an argument, and the fourth
    // *is* its own scope. A caller cannot construct an unscoped standing
    // permission, which is why this is a construction test rather than a
    // rendering one.
    for scope in [
        AlwaysScope::tool("write-file"),
        AlwaysScope::path("/work"),
        AlwaysScope::host("build.internal"),
    ] {
        assert!(scope.subject().is_some(), "{scope:?} names nothing");
    }
    assert_eq!(AlwaysScope::Session.subject(), None);
    assert_eq!(AlwaysScope::Session.name(), "session");
}

#[gpui::test]
fn the_request_is_specific(cx: &mut TestAppContext) {
    let (mut harness, _prompt) = scene(cx, ApprovalStatus::Pending);

    assert_eq!(
        harness
            .node("approval.write.action")
            .expect("published")
            .text
            .as_deref(),
        Some("Write to /work/report/summary.md")
    );
    assert_eq!(
        harness
            .node("approval.write.detail.path")
            .expect("published")
            .value
            .as_deref(),
        Some("/work/report/summary.md"),
        "the exact thing being approved is on screen, not a category"
    );
}

#[gpui::test]
fn declined_expired_and_superseded_are_three_presentations(cx: &mut TestAppContext) {
    let mut published = Vec::new();
    for status in [
        ApprovalStatus::Declined,
        ApprovalStatus::Expired,
        ApprovalStatus::Superseded {
            by: "a later request".into(),
        },
    ] {
        let (mut harness, _prompt) = scene(cx, status.clone());
        let root = harness.node("approval.write").expect("published");
        let outcome = harness.node("approval.write.outcome").expect("published");
        assert_eq!(root.value.as_deref(), Some(status.name()));
        published.push((
            root.value.expect("a state"),
            outcome.text.expect("a sentence"),
        ));
    }

    let mut values: Vec<_> = published.iter().map(|(value, _)| value.clone()).collect();
    let mut sentences: Vec<_> = published.into_iter().map(|(_, text)| text).collect();
    values.sort();
    values.dedup();
    sentences.sort();
    sentences.dedup();
    assert_eq!(values.len(), 3, "three states, three published names");
    assert_eq!(sentences.len(), 3, "three states, three sentences");
}

#[gpui::test]
fn a_resolved_prompt_offers_nothing_and_reports_nothing(cx: &mut TestAppContext) {
    let (mut harness, prompt) = scene(cx, ApprovalStatus::Expired);
    let seen = events(&mut harness, &prompt);

    assert!(harness.node("approval.write.approve").is_none());
    assert!(harness.node("approval.write.decline").is_none());
    assert!(harness.node("approval.write.always.session").is_none());

    harness.keystrokes("enter");
    harness.keystrokes("escape");
    harness.update({
        let prompt = prompt.clone();
        move |_, cx| {
            prompt.update(cx, |prompt, cx| {
                prompt.approve(ApprovalDecision::Once, cx);
                prompt.decline(cx);
            })
        }
    });
    assert!(
        seen.borrow().is_empty(),
        "a request that is no longer open answers nothing"
    );
}
