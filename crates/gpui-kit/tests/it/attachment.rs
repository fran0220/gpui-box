use std::cell::Cell;
use std::rc::Rc;

use gpui::{TestAppContext, div, prelude::*};
use gpui_kit::display::attachment::{AttachmentState, AttachmentTile};
use gpui_kit::foundation::{Disableable, Slotted};
use gpui_kit_testkit::harness::Harness;

#[gpui::test]
fn attachment_transfer_processing_and_refusal_remain_distinct(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .children(
                [
                    (
                        "known",
                        AttachmentState::Transferring {
                            completed: 3,
                            total: Some(8),
                        },
                    ),
                    (
                        "unknown",
                        AttachmentState::Transferring {
                            completed: 3,
                            total: None,
                        },
                    ),
                    (
                        "zero",
                        AttachmentState::Transferring {
                            completed: 3,
                            total: Some(0),
                        },
                    ),
                    (
                        "complete-transfer",
                        AttachmentState::Transferring {
                            completed: 8,
                            total: Some(8),
                        },
                    ),
                    ("processing", AttachmentState::Processing),
                    (
                        "paused",
                        AttachmentState::Paused {
                            completed: 3,
                            total: Some(8),
                        },
                    ),
                    (
                        "refused",
                        AttachmentState::Failed("Host refused transfer".into()),
                    ),
                ]
                .map(|(id, state)| AttachmentTile::new(id, "Fixture attachment").state(state)),
            )
            .into_any_element()
    });
    assert_eq!(
        harness
            .node("known.progress")
            .expect("known progress")
            .value_now,
        Some(0.375)
    );
    for id in ["unknown", "zero", "processing"] {
        assert!(
            harness
                .node(&format!("{id}.progress"))
                .expect("indeterminate progress")
                .value_now
                .is_none()
        );
        assert!(harness.node(id).expect("busy state").busy);
    }
    assert_eq!(
        harness
            .node("complete-transfer")
            .expect("still transferring")
            .value
            .as_deref(),
        Some("transferring")
    );
    assert!(
        !harness
            .node("paused.progress")
            .expect("paused progress")
            .busy
    );
    assert_eq!(
        harness
            .node("refused.status")
            .expect("host refusal")
            .text
            .as_deref(),
        Some("Host refused transfer")
    );
}

#[gpui::test]
fn attachment_slots_keep_known_media_and_disabled_actions_never_build(cx: &mut TestAppContext) {
    let calls = Rc::new(Cell::new(0));
    let mut harness = Harness::new(cx, gpui_kit::install, {
        let calls = calls.clone();
        move |_, _| {
            AttachmentTile::new("attachment", "Known attachment")
                .state(AttachmentState::Failed("Refresh refused".into()))
                .disabled(true)
                .slot("media", |_, _| {
                    div().child("Verified local preview").into_any_element()
                })
                .slot("title", |_, _| {
                    div().child("Custom title presentation").into_any_element()
                })
                .slot("description", |_, _| {
                    div().child("Custom description").into_any_element()
                })
                .slot("actions", {
                    let calls = calls.clone();
                    move |_, _| {
                        calls.set(calls.get() + 1);
                        div().into_any_element()
                    }
                })
                .into_any_element()
        }
    });
    assert!(harness.node("attachment.media").is_some());
    assert!(harness.node("attachment.title").is_some());
    assert!(harness.node("attachment.description").is_some());
    assert_eq!(calls.get(), 0, "disabled action slot is not invoked");
    assert!(
        harness
            .node("attachment")
            .expect("disabled attachment")
            .disabled
    );
}
