//! Overlay placement, focus containment, and dismissal, asserted through a
//! real window.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{FocusHandle, TestAppContext, div, prelude::*, px};
use gpui_kit::overlay::{FocusTrap, Frost, Overlay, Placement};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_testkit::harness::Harness;
use gpui_kit_testkit::present;

#[derive(Default)]
struct Scene {
    trap: FocusTrap,
    handles: Vec<FocusHandle>,
    open: bool,
}

type SharedScene = Rc<RefCell<Scene>>;

/// A page with one focusable button behind an overlay holding three stops.
fn scene(cx: &mut TestAppContext, state: SharedScene) -> Harness {
    Harness::new(cx, gpui_kit::install, move |_window, cx| {
        let mut scene = state.borrow_mut();
        if scene.handles.is_empty() {
            scene.handles = (0..4).map(|_| cx.focus_handle()).collect();
        }
        let handles = scene.handles.clone();
        let open = scene.open;

        let page = div()
            .child(
                div()
                    .id("page.button")
                    .track_focus(&handles[0])
                    .w(px(40.0))
                    .h(px(10.0))
                    .semantic_in(cx, NodeSpec::new("page.button", Role::Button)),
            )
            .into_any_element();

        if !open {
            scene.trap.begin_frame();
            return page;
        }

        scene.trap.begin_frame();
        for handle in handles.iter().skip(1) {
            scene.trap.register(handle.clone());
        }
        let overlay = Overlay::modal("dialog").placement(Placement::Center).child(
            div()
                .w(px(100.0))
                .h(px(60.0))
                .children(handles.iter().skip(1).enumerate().map(|(index, handle)| {
                    div()
                        .id(format!("dialog.stop.{index}"))
                        .track_focus(handle)
                        .w(px(20.0))
                        .h(px(10.0))
                        .semantic_in(
                            cx,
                            NodeSpec::new(format!("dialog.stop.{index}"), Role::Button),
                        )
                }))
                .semantic_in(cx, NodeSpec::new("dialog", Role::Dialog)),
        );

        div().child(page).child(overlay).into_any_element()
    })
}

fn open(harness: &mut Harness, state: &SharedScene) {
    harness.update(|window, cx| {
        let mut scene = state.borrow_mut();
        scene.open = true;
        scene.trap.engage(window, cx);
        cx.refresh_windows();
    });
    // The trap can only focus stops the open frame has registered.
    harness.update(|window, cx| {
        state.borrow().trap.focus_first(window, cx);
    });
}

#[gpui::test]
fn an_overlay_publishes_its_content_only_while_it_is_open(cx: &mut TestAppContext) {
    let state = SharedScene::default();
    let mut harness = scene(cx, state.clone());
    assert!(present(&harness.snapshot(), "dialog").is_err());

    open(&mut harness, &state);
    assert!(present(&harness.snapshot(), "dialog").is_ok());
}

#[gpui::test]
fn opening_an_overlay_moves_focus_into_it(cx: &mut TestAppContext) {
    let state = SharedScene::default();
    let mut harness = scene(cx, state.clone());
    harness.update(|window, cx| {
        let handle = state.borrow().handles[0].clone();
        handle.focus(window, cx);
    });

    open(&mut harness, &state);

    harness.update(|window, cx| {
        assert!(
            state.borrow().trap.contains_focus(window, cx),
            "focus must land inside the overlay"
        );
    });
}

#[gpui::test]
fn tabbing_cycles_within_the_overlay_instead_of_escaping_it(cx: &mut TestAppContext) {
    let state = SharedScene::default();
    let mut harness = scene(cx, state.clone());
    open(&mut harness, &state);

    harness.update(|window, cx| {
        let scene = state.borrow();
        // Three stops: forward from the last one wraps to the first.
        scene.trap.focus_next(window, cx);
        scene.trap.focus_next(window, cx);
        scene.trap.focus_next(window, cx);
        assert_eq!(window.focused(cx).as_ref(), scene.handles.get(1));
    });
}

#[gpui::test]
fn tabbing_backward_from_the_first_stop_wraps_to_the_last(cx: &mut TestAppContext) {
    let state = SharedScene::default();
    let mut harness = scene(cx, state.clone());
    open(&mut harness, &state);

    harness.update(|window, cx| {
        let scene = state.borrow();
        scene.trap.focus_prev(window, cx);
        assert_eq!(window.focused(cx).as_ref(), scene.handles.last());
    });
}

#[gpui::test]
fn closing_an_overlay_returns_focus_to_where_it_came_from(cx: &mut TestAppContext) {
    let state = SharedScene::default();
    let mut harness = scene(cx, state.clone());
    harness.update(|window, cx| {
        let handle = state.borrow().handles[0].clone();
        handle.focus(window, cx);
    });

    open(&mut harness, &state);
    harness.update(|window, cx| {
        let mut scene = state.borrow_mut();
        scene.open = false;
        scene.trap.release(window, cx);
        cx.refresh_windows();
    });

    harness.update(|window, cx| {
        assert_eq!(window.focused(cx).as_ref(), state.borrow().handles.first());
    });
    assert!(present(&harness.snapshot(), "dialog").is_err());
}

#[gpui::test]
fn clicking_the_scrim_reports_a_dismissal(cx: &mut TestAppContext) {
    let dismissed = Rc::new(RefCell::new(false));
    let flag = dismissed.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, cx| {
        let flag = flag.clone();
        div()
            .w(px(400.0))
            .h(px(400.0))
            .child(
                Overlay::modal("dialog")
                    .on_dismiss(move |_, _| *flag.borrow_mut() = true)
                    .child(
                        div()
                            .w(px(40.0))
                            .h(px(40.0))
                            .semantic_in(cx, NodeSpec::new("dialog", Role::Dialog)),
                    ),
            )
            .into_any_element()
    });

    // A corner of the window is scrim, never the centered dialog.
    harness
        .context()
        .simulate_click(gpui::point(px(8.0), px(8.0)), gpui::Modifiers::none());
    harness.context().run_until_parked();

    assert!(*dismissed.borrow(), "a scrim click must report a dismissal");
}

#[gpui::test]
fn glass_keeps_its_content_in_the_tree_and_on_the_screen(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, cx| {
        div()
            .w(px(400.0))
            .h(px(300.0))
            .child(
                Frost::new("rename.glass").child(div().w(px(200.0)).h(px(80.0)).semantic_in(
                    cx,
                    NodeSpec::new("rename.title", Role::Heading).text("Rename"),
                )),
            )
            .into_any_element()
    });

    let snapshot = harness.snapshot();
    let glass = present(&snapshot, "rename.glass").expect("the surface is published");
    assert_eq!(glass.role, Role::Region);
    assert!(
        glass.bounds.width > 0.0 && glass.bounds.height > 0.0,
        "a surface painted in its own layer still occupies the layout it asked for"
    );
    // Whether the renderer blurred anything is invisible to the tree, which is
    // the point: the content is legible either way.
    let title = present(&snapshot, "rename.title").expect("the content is published");
    assert_eq!(title.text.as_deref(), Some("Rename"));
    assert!(title.visible);
}
