//! Dispatch real framework touch events through measured component trees.

use super::{
    refresh::{PullToRefresh, PullToRefreshEvent, RefreshState},
    swipe::{SwipeActions, SwipeActionsEvent, SwipeSide},
};
use crate::overlay::sheet::{BottomSheet, BottomSheetEvent, SheetAction, SheetDetent};
use gpui::{
    AppContext, InputEvent, IntoElement, ParentElement, Pixels, Point, ScrollHandle, Styled,
    TestAppContext, TouchEvent, TouchId, TouchPhase, div, point, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_testkit::harness::Harness;
use std::{cell::RefCell, rc::Rc};

fn touch(harness: &mut Harness, phase: TouchPhase, position: Point<Pixels>) {
    harness.update(|window, cx| {
        window.dispatch_event(
            TouchEvent {
                id: TouchId(71),
                phase,
                position,
                predicted_position: None,
                force: None,
            }
            .to_platform_input(),
            cx,
        );
    });
    harness.frame();
}

#[gpui::test]
fn refresh_dispatch_cancels_reverses_and_retains_failed_content(cx: &mut TestAppContext) {
    let slot = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let collect = events.clone();
    let mut harness = Harness::new(cx, crate::install, move |_, cx| {
        let refresh = build
            .borrow_mut()
            .get_or_insert_with(|| {
                let scroll = ScrollHandle::new();
                let refresh =
                    cx.new(|_| PullToRefresh::new("refresh", scroll, "Refresh", "Release"));
                refresh.update(cx, |refresh, cx| {
                    refresh.set_content(
                        Some(Rc::new(|_, cx| {
                            div()
                                .h(px(200.0))
                                .w_full()
                                .child("Verified row")
                                .semantic_in(
                                    cx,
                                    NodeSpec::new("verified", Role::Text).text("Verified row"),
                                )
                                .into_any_element()
                        })),
                        cx,
                    )
                });
                let collect = collect.clone();
                cx.subscribe(&refresh, move |_, event, _| {
                    collect.borrow_mut().push(*event)
                })
                .detach();
                refresh
            })
            .clone();
        div()
            .w(px(330.0))
            .h(px(380.0))
            .child(refresh)
            .into_any_element()
    });
    let refresh = slot.borrow().clone().expect("refresh mounted");
    let at = harness.point_in("verified");
    touch(&mut harness, TouchPhase::Started, at);
    touch(
        &mut harness,
        TouchPhase::Moved,
        at + point(px(2.0), px(96.0)),
    );
    touch(
        &mut harness,
        TouchPhase::Cancelled,
        at + point(px(2.0), px(96.0)),
    );
    assert!(events.borrow().is_empty());
    touch(&mut harness, TouchPhase::Started, at);
    touch(
        &mut harness,
        TouchPhase::Moved,
        at + point(px(1.0), px(92.0)),
    );
    touch(
        &mut harness,
        TouchPhase::Moved,
        at + point(px(1.0), px(15.0)),
    );
    touch(
        &mut harness,
        TouchPhase::Ended,
        at + point(px(1.0), px(15.0)),
    );
    assert!(
        events.borrow().is_empty(),
        "reversal below threshold disarms"
    );
    touch(&mut harness, TouchPhase::Started, at);
    touch(
        &mut harness,
        TouchPhase::Moved,
        at + point(px(3.0), px(84.0)),
    );
    touch(
        &mut harness,
        TouchPhase::Ended,
        at + point(px(3.0), px(84.0)),
    );
    assert_eq!(*events.borrow(), vec![PullToRefreshEvent::RefreshRequested]);
    harness.update(|_, cx| {
        refresh.update(cx, |refresh, cx| {
            refresh.set_state(RefreshState::Loading("Loading".into()), cx)
        })
    });
    harness.frame();
    harness.click("refresh.refresh");
    assert_eq!(events.borrow().len(), 1);
    assert!(harness.node("verified").is_some());
    harness.update(|_, cx| {
        refresh.update(cx, |refresh, cx| {
            refresh.set_state(RefreshState::Error("Failed".into()), cx)
        })
    });
    harness.frame();
    assert_eq!(
        harness
            .node("verified")
            .expect("verified content retained")
            .text
            .as_deref(),
        Some("Verified row")
    );
    assert_eq!(
        harness
            .node("refresh.status")
            .expect("error status present")
            .text
            .as_deref(),
        Some("Failed")
    );
    let bounds = harness
        .bounds("refresh.refresh")
        .expect("refresh button measured");
    assert!(bounds.size.height >= px(48.0));
}

#[gpui::test]
fn swipe_dispatch_rejects_vertical_and_cancel_never_activates(cx: &mut TestAppContext) {
    let slot = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let collect = events.clone();
    let mut harness = Harness::new(cx, crate::install, move |_, cx| {
        let swipe = build
            .borrow_mut()
            .get_or_insert_with(|| {
                let swipe = cx.new(|_| SwipeActions::new("swipe", "Actions"));
                swipe.update(cx, |swipe, cx| {
                    swipe.set_actions(
                        SwipeSide::Right,
                        vec![SheetAction::new("remove", "Delete")],
                        cx,
                    );
                    swipe.set_content(
                        Some(Rc::new(|_, cx| {
                            div()
                                .h(px(70.0))
                                .child("Retained")
                                .semantic_in(
                                    cx,
                                    NodeSpec::new("swipe.content", Role::Text).text("Retained"),
                                )
                                .into_any_element()
                        })),
                        cx,
                    );
                });
                let collect = collect.clone();
                cx.subscribe(&swipe, move |_, event, _| {
                    collect.borrow_mut().push(event.clone())
                })
                .detach();
                swipe
            })
            .clone();
        div().w(px(360.0)).child(swipe).into_any_element()
    });
    let swipe = slot.borrow().clone().expect("swipe mounted");
    let at = harness.point_in("swipe.content");
    touch(&mut harness, TouchPhase::Started, at);
    touch(
        &mut harness,
        TouchPhase::Moved,
        at + point(px(-3.0), px(52.0)),
    );
    touch(
        &mut harness,
        TouchPhase::Ended,
        at + point(px(-3.0), px(52.0)),
    );
    assert!(events.borrow().is_empty());
    touch(&mut harness, TouchPhase::Started, at);
    touch(
        &mut harness,
        TouchPhase::Moved,
        at + point(px(-105.0), px(2.0)),
    );
    touch(
        &mut harness,
        TouchPhase::Cancelled,
        at + point(px(-105.0), px(2.0)),
    );
    assert!(events.borrow().is_empty());
    touch(&mut harness, TouchPhase::Started, at);
    touch(
        &mut harness,
        TouchPhase::Moved,
        at + point(px(-105.0), px(2.0)),
    );
    touch(
        &mut harness,
        TouchPhase::Ended,
        at + point(px(-105.0), px(2.0)),
    );
    assert_eq!(
        *events.borrow(),
        vec![SwipeActionsEvent::Revealed(Some(SwipeSide::Right))]
    );
    harness.click("swipe.action.remove");
    assert_eq!(
        events.borrow().last(),
        Some(&SwipeActionsEvent::ActionRequested("remove".into()))
    );
    harness.update(|_, cx| swipe.update(cx, |swipe, cx| swipe.set_enabled(false, cx)));
    harness.frame();
    harness.click("swipe.action.remove");
    assert_eq!(events.borrow().len(), 2);
    assert!(harness.node("swipe.content").is_some());
}

#[gpui::test]
fn sheet_drag_requests_only_on_release_and_caller_interrupt_cancels(cx: &mut TestAppContext) {
    let slot = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let collect = events.clone();
    let mut harness = Harness::new(cx, crate::install, move |window, cx| {
        build
            .borrow_mut()
            .get_or_insert_with(|| {
                let sheet = cx.new(|cx| BottomSheet::new("sheet", window, cx));
                sheet.update(cx, |sheet, cx| {
                    sheet.set_detents(
                        vec![
                            SheetDetent::new("short", 220.0),
                            SheetDetent::new("tall", 470.0),
                        ],
                        "tall",
                        cx,
                    );
                    sheet.open(window, cx);
                    sheet.settle(cx);
                });
                let collect = collect.clone();
                cx.subscribe(&sheet, move |_, event, _| {
                    if let BottomSheetEvent::DetentRequested(id) = event {
                        collect.borrow_mut().push(id.clone());
                    }
                })
                .detach();
                sheet
            })
            .clone()
            .into_any_element()
    });
    let sheet = slot.borrow().clone().expect("sheet mounted");
    let at = harness.point_in("sheet.handle");
    let before = harness.bounds("sheet").expect("sheet measured");
    touch(&mut harness, TouchPhase::Started, at);
    touch(
        &mut harness,
        TouchPhase::Moved,
        at + point(px(2.0), px(170.0)),
    );
    let preview = harness.bounds("sheet").expect("preview measured");
    assert!(preview.size.height < before.size.height);
    touch(
        &mut harness,
        TouchPhase::Cancelled,
        at + point(px(2.0), px(170.0)),
    );
    assert_eq!(
        harness
            .bounds("sheet")
            .expect("cancelled sheet measured")
            .size
            .height,
        before.size.height
    );
    assert!(events.borrow().is_empty());
    touch(&mut harness, TouchPhase::Started, at);
    touch(
        &mut harness,
        TouchPhase::Moved,
        at + point(px(2.0), px(190.0)),
    );
    touch(
        &mut harness,
        TouchPhase::Ended,
        at + point(px(2.0), px(190.0)),
    );
    assert_eq!(
        events.borrow().as_slice(),
        &[gpui::SharedString::from("short")]
    );
    assert_eq!(
        harness.update(|_, cx| sheet.read(cx).selected_detent().clone()),
        "tall"
    );
    touch(&mut harness, TouchPhase::Started, at);
    touch(
        &mut harness,
        TouchPhase::Moved,
        at + point(px(1.0), px(170.0)),
    );
    harness.update(|_, cx| {
        sheet.update(cx, |sheet, cx| {
            sheet.set_detent("short", cx);
        })
    });
    touch(
        &mut harness,
        TouchPhase::Ended,
        at + point(px(1.0), px(170.0)),
    );
    assert_eq!(
        events.borrow().len(),
        1,
        "caller update cancels old drag commit"
    );
    harness.update(|_, cx| {
        sheet.update(cx, |sheet, cx| {
            sheet.set_detents(
                vec![
                    SheetDetent::new("short", 220.0),
                    SheetDetent::new("huge", 10000.0),
                ],
                "huge",
                cx,
            );
        })
    });
    harness.frame();
    let limited = harness.bounds("sheet").expect("viewport-constrained sheet");
    assert!(limited.size.height < px(10000.0));
    let at = harness.point_in("sheet.handle");
    touch(&mut harness, TouchPhase::Started, at);
    touch(
        &mut harness,
        TouchPhase::Moved,
        at + point(px(1.0), px(40.0)),
    );
    assert_eq!(
        harness
            .bounds("sheet")
            .expect("constrained drag preview")
            .size
            .height,
        limited.size.height - px(40.0),
        "no offscreen-height dead zone"
    );
    touch(
        &mut harness,
        TouchPhase::Cancelled,
        at + point(px(1.0), px(40.0)),
    );
    assert_eq!(
        harness
            .bounds("sheet")
            .expect("restored constrained sheet")
            .size
            .height,
        limited.size.height
    );
}

#[gpui::test]
fn sheet_nested_scroll_handoff_conserves_residual_reverses_and_cancels(cx: &mut TestAppContext) {
    use gpui::{InteractiveElement, StatefulInteractiveElement};
    let slot = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let scroll = ScrollHandle::new();
    let build_scroll = scroll.clone();
    let mut harness = Harness::new(cx, crate::install, move |window, cx| {
        build
            .borrow_mut()
            .get_or_insert_with(|| {
                let sheet = cx.new(|cx| BottomSheet::new("handoff", window, cx));
                sheet.update(cx, |sheet, cx| {
                    sheet.set_detents(
                        vec![
                            SheetDetent::new("short", 220.0),
                            SheetDetent::new("tall", 470.0),
                        ],
                        "tall",
                        cx,
                    );
                    sheet.set_scroll_handle(Some(build_scroll.clone()), cx);
                    let scroll = build_scroll.clone();
                    sheet.set_content(
                        Some(Rc::new(move |_, cx| {
                            div()
                                .id("handoff.viewport")
                                .size_full()
                                .overflow_y_scroll()
                                .track_scroll(&scroll)
                                .child(div().h(px(1300.0)).w_full().child("Scrollable fixture"))
                                .semantic_in(cx, NodeSpec::new("handoff.viewport", Role::Group))
                                .into_any_element()
                        })),
                        cx,
                    );
                    sheet.open(window, cx);
                    sheet.settle(cx);
                });
                sheet
            })
            .clone()
            .into_any_element()
    });
    scroll.set_offset(point(px(0.0), px(-37.0)));
    harness.frame();
    assert_eq!(scroll.offset().y, px(-37.0));
    let at = harness.point_in("handoff.viewport");
    touch(&mut harness, TouchPhase::Started, at);
    touch(
        &mut harness,
        TouchPhase::Moved,
        at + point(px(1.0), px(60.0)),
    );
    assert_eq!(scroll.offset().y, px(0.0));
    assert_eq!(
        harness
            .bounds("handoff")
            .expect("handoff preview measured")
            .size
            .height,
        px(447.0),
        "37 scrolled + 23 sheet = 60 total"
    );
    touch(
        &mut harness,
        TouchPhase::Moved,
        at + point(px(1.0), px(45.0)),
    );
    assert_eq!(scroll.offset().y, px(0.0), "captured sheet owns reversal");
    assert_eq!(
        harness
            .bounds("handoff")
            .expect("reversed sheet measured")
            .size
            .height,
        px(462.0),
        "37 scrolled + 8 sheet = 45 total"
    );
    touch(
        &mut harness,
        TouchPhase::Cancelled,
        at + point(px(1.0), px(45.0)),
    );
    assert_eq!(
        harness
            .bounds("handoff")
            .expect("cancelled sheet measured")
            .size
            .height,
        px(470.0)
    );
    assert_eq!(
        scroll.offset().y,
        px(0.0),
        "cancel restores sheet preview, not already consumed scroll"
    );
}

#[gpui::test]
fn action_sheet_keyboard_skips_disabled_and_pending_retains_rows(cx: &mut TestAppContext) {
    use crate::overlay::sheet::{ActionSheet, ActionSheetEvent, SheetActionState};
    let slot = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let collect = events.clone();
    let mut harness = Harness::new(cx, crate::install, move |window, cx| {
        build
            .borrow_mut()
            .get_or_insert_with(|| {
                let sheet = cx.new(|cx| ActionSheet::new("actions", window, cx));
                sheet.update(cx, |sheet, cx| {
                    let mut disabled = SheetAction::new("disabled", "Unavailable");
                    disabled.disabled = true;
                    sheet.set_actions(
                        vec![
                            SheetAction::new("save", "Save"),
                            disabled,
                            SheetAction::new("remove", "Remove"),
                        ],
                        cx,
                    );
                    sheet.open(window, cx);
                    sheet.sheet().update(cx, |sheet, cx| sheet.settle(cx));
                });
                let collect = collect.clone();
                cx.subscribe(&sheet, move |_, event, _| {
                    if let ActionSheetEvent::ActionRequested(id) = event {
                        collect.borrow_mut().push(id.clone());
                    }
                })
                .detach();
                sheet
            })
            .clone()
            .into_any_element()
    });
    let sheet = slot.borrow().clone().expect("action sheet mounted");
    harness.keystrokes("enter");
    assert_eq!(
        events.borrow().as_slice(),
        &[gpui::SharedString::from("save")]
    );
    harness.keystrokes("tab enter");
    assert_eq!(
        events.borrow().last().expect("keyboard action emitted"),
        "remove"
    );
    assert!(
        harness
            .bounds("actions.close")
            .expect("close target measured")
            .size
            .height
            >= px(48.0)
    );
    harness.update(|_, cx| {
        sheet.update(cx, |sheet, cx| {
            sheet.set_state(SheetActionState::Pending("Waiting".into()), cx)
        })
    });
    harness.frame();
    harness.click("actions.action.save");
    assert_eq!(events.borrow().len(), 2);
    assert!(harness.node("actions.action.remove").is_some());
    assert!(
        harness
            .node("actions.action.save")
            .expect("pending row retained")
            .disabled
    );
    harness.keystrokes("tab enter");
    assert!(!harness.update(|_, cx| sheet.read(cx).sheet().read(cx).is_open(cx)));
}
