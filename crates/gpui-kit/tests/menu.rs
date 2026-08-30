//! A menu presents what can be done and reports what was taken. It never
//! takes anything itself, and it never lands the keyboard on a rule, a label,
//! or a row the host refused.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext as _, Entity, IntoElement, Modifiers, MouseButton, MouseDownEvent, TestAppContext,
    div, prelude::*, px,
};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

fn items() -> Vec<MenuItem> {
    vec![
        MenuItem::section("group", "Edit"),
        MenuItem::command("undo", "Undo").shortcut("cmd-z"),
        MenuItem::separator("rule"),
        MenuItem::command("paste", "Paste").disabled(true),
        MenuItem::check("wrap", "Wrap lines", true),
        MenuItem::submenu(
            "share",
            "Share",
            [
                MenuItem::command("share.link", "Copy link"),
                MenuItem::command("share.mail", "Send by mail"),
            ],
        ),
    ]
}

fn menu(cx: &mut TestAppContext) -> (Harness, Entity<Menu>) {
    let slot: Rc<RefCell<Option<Entity<Menu>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let menu = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    Menu::new("workspace.edit", window, cx)
                        .trigger("Edit")
                        .items(items())
                })
            })
            .clone();
        div()
            .w(px(600.0))
            .h(px(400.0))
            .child(menu)
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("menu was built");
    (harness, entity)
}

fn events(harness: &mut Harness, menu: &Entity<Menu>) -> Rc<RefCell<Vec<MenuEvent>>> {
    let seen: Rc<RefCell<Vec<MenuEvent>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    harness.update({
        let menu = menu.clone();
        move |_, cx| {
            cx.subscribe(&menu, move |_, event: &MenuEvent, _| {
                sink.borrow_mut().push(event.clone());
            })
            .detach();
        }
    });
    seen
}

fn taken(seen: &Rc<RefCell<Vec<MenuEvent>>>) -> Vec<String> {
    seen.borrow()
        .iter()
        .filter_map(|event| match event {
            MenuEvent::Invoked(id) => Some(id.to_string()),
            _ => None,
        })
        .collect()
}

fn open(harness: &mut Harness, menu: &Entity<Menu>) {
    let menu = menu.clone();
    harness.update(move |window, cx| {
        menu.update(cx, |menu, cx| menu.open(window, cx));
    });
}

#[gpui::test]
fn a_closed_menu_publishes_its_trigger_and_nothing_else(cx: &mut TestAppContext) {
    let (mut harness, _menu) = menu(cx);

    assert_eq!(
        harness.node("workspace.edit").expect("published").expanded,
        Some(false)
    );
    assert!(harness.node("workspace.edit.trigger").is_some());
    assert!(
        harness.node("workspace.edit.undo").is_none(),
        "a closed menu must not publish its commands"
    );
}

#[gpui::test]
fn opening_publishes_every_row_with_the_state_the_host_holds(cx: &mut TestAppContext) {
    let (mut harness, menu) = menu(cx);
    open(&mut harness, &menu);

    assert_eq!(
        harness.node("workspace.edit").expect("published").expanded,
        Some(true)
    );
    let tree = harness.accessibility_tree();
    assert!(tree["nodes"].as_object().is_some_and(|nodes| {
        nodes.values().any(|node| {
            node["element_id"] == "Name(\"workspace.edit.menu\")" && node["aria"]["role"] == "Menu"
        })
    }));
    assert_eq!(
        harness.node("workspace.edit.menu").expect("published").role,
        gpui_kit_semantics::Role::Menu
    );
    assert_eq!(
        harness
            .node("workspace.edit.undo")
            .expect("published")
            .text
            .as_deref(),
        Some("Undo")
    );
    assert_eq!(
        harness
            .node("workspace.edit.wrap")
            .expect("published")
            .checked,
        Some(true)
    );
    assert!(
        harness
            .node("workspace.edit.paste")
            .expect("published")
            .disabled
    );
    assert!(harness.node("workspace.edit.rule").is_some());
    assert_eq!(
        harness
            .node("workspace.edit.group")
            .expect("published")
            .text
            .as_deref(),
        Some("Edit")
    );
    assert_eq!(
        harness
            .node("workspace.edit.share")
            .expect("published")
            .expanded,
        Some(false)
    );
}

#[gpui::test]
fn native_menu_owns_named_rows_actions_and_lifetime(cx: &mut TestAppContext) {
    let (mut harness, menu) = menu(cx);
    let seen = events(&mut harness, &menu);
    open(&mut harness, &menu);

    let tree = harness.accessibility_tree();
    let nodes = tree["nodes"].as_object().expect("native nodes");
    let find = |id: &str| {
        nodes
            .iter()
            .find(|(_, node)| node["element_id"] == format!("Name(\"{id}\")"))
            .unwrap_or_else(|| panic!("native node {id}"))
    };
    let (menu_key, native_menu) = find("workspace.edit.menu");
    let (undo_key, undo) = find("workspace.edit.undo");
    let (_, paste) = find("workspace.edit.paste");
    let (_, wrap) = find("workspace.edit.wrap");
    let (_, share) = find("workspace.edit.share");

    assert_eq!(native_menu["aria"]["role"], "Menu");
    assert_eq!(native_menu["aria"]["label"], "Edit");
    assert_eq!(tree["gpui_focus"], menu_key.as_str());
    assert!(
        native_menu["children"]
            .as_array()
            .is_some_and(|children| children.iter().any(|child| child == undo_key))
    );
    assert_eq!(undo["aria"]["role"], "MenuItem");
    assert_eq!(undo["aria"]["label"], "Undo");
    assert!(
        undo["aria"]["on_action"]
            .as_array()
            .is_some_and(|actions| actions.iter().any(|action| action == "Click"))
    );
    assert_eq!(paste["aria"]["disabled"], true);
    assert!(
        !paste["aria"]["on_action"]
            .as_array()
            .is_some_and(|actions| actions.iter().any(|action| action == "Click"))
    );
    assert_eq!(wrap["aria"]["toggled"], "True");
    assert_eq!(share["aria"]["expanded"], false);

    let native_id = native_menu["accesskit_id"].clone();
    let undo_id = gpui::accesskit::NodeId(
        undo["accesskit_id"]
            .as_str()
            .expect("undo native id")
            .parse()
            .expect("numeric undo id"),
    );
    let window = harness.window();
    harness.context().dispatch_accessibility_action(
        window,
        gpui::accesskit::ActionRequest {
            action: gpui::accesskit::Action::Click,
            target_tree: gpui::accesskit::TreeId::ROOT,
            target_node: undo_id,
            data: None,
        },
    );
    assert_eq!(taken(&seen), vec!["undo".to_string()]);
    assert!(harness.node("workspace.edit.menu").is_none());
    let closed = harness.accessibility_tree();
    assert!(!closed["nodes"].as_object().is_some_and(|nodes| {
        nodes
            .values()
            .any(|node| node["element_id"] == "Name(\"workspace.edit.menu\")")
    }));

    open(&mut harness, &menu);
    let reopened = harness.accessibility_tree();
    let reopened_menu = reopened["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes
                .values()
                .find(|node| node["element_id"] == "Name(\"workspace.edit.menu\")")
        })
        .expect("reopened native menu");
    assert_eq!(reopened_menu["accesskit_id"], native_id);
}

#[gpui::test]
fn native_menu_focus_follows_the_active_stable_item_only_while_the_container_is_focused(
    cx: &mut TestAppContext,
) {
    let (mut harness, menu) = menu(cx);
    open(&mut harness, &menu);

    let opened = harness.accessibility_tree();
    let opened_nodes = opened["nodes"].as_object().expect("native nodes");
    let (menu_key, _) = opened_nodes
        .iter()
        .find(|(_, node)| node["element_id"] == "Name(\"workspace.edit.menu\")")
        .expect("native menu");
    let (undo_key, undo) = opened_nodes
        .iter()
        .find(|(_, node)| node["element_id"] == "Name(\"workspace.edit.undo\")")
        .expect("native Undo item");
    let undo_native_id = undo["accesskit_id"].clone();
    assert_eq!(opened["gpui_focus"], menu_key.as_str());
    assert_eq!(opened["active_descendant_focus"], undo_key.as_str());

    harness.keystrokes("down");
    let moved = harness.accessibility_tree();
    let moved_nodes = moved["nodes"].as_object().expect("moved native nodes");
    let (moved_undo_key, moved_undo) = moved_nodes
        .iter()
        .find(|(_, node)| node["element_id"] == "Name(\"workspace.edit.undo\")")
        .expect("stable native Undo item");
    let (wrap_key, _) = moved_nodes
        .iter()
        .find(|(_, node)| node["element_id"] == "Name(\"workspace.edit.wrap\")")
        .expect("native Wrap lines item");
    assert_eq!(moved_undo_key, undo_key);
    assert_eq!(moved_undo["accesskit_id"], undo_native_id);
    assert_eq!(moved["gpui_focus"], menu_key.as_str());
    assert_eq!(moved["active_descendant_focus"], wrap_key.as_str());

    let (trigger_key, trigger) = moved_nodes
        .iter()
        .find(|(_, node)| node["element_id"] == "Name(\"workspace.edit.trigger\")")
        .expect("native menu trigger");
    let trigger_id = gpui::accesskit::NodeId(
        trigger["accesskit_id"]
            .as_str()
            .expect("trigger native id")
            .parse()
            .expect("numeric trigger id"),
    );
    let window = harness.window();
    harness.context().dispatch_accessibility_action(
        window,
        gpui::accesskit::ActionRequest {
            action: gpui::accesskit::Action::Focus,
            target_tree: gpui::accesskit::TreeId::ROOT,
            target_node: trigger_id,
            data: None,
        },
    );
    let unfocused = harness.accessibility_tree();
    assert_eq!(unfocused["gpui_focus"], trigger_key.as_str());
    assert!(unfocused["active_descendant_focus"].is_null());
    assert!(harness.update(|_, cx| menu.read(cx).is_open()));
}

#[gpui::test]
fn the_keyboard_steps_over_rules_labels_and_refusals(cx: &mut TestAppContext) {
    let (mut harness, menu) = menu(cx);
    open(&mut harness, &menu);

    assert!(
        harness
            .node("workspace.edit.undo")
            .expect("published")
            .hovered,
        "the cursor opens on the first row that can be taken"
    );

    harness.keystrokes("down");
    assert!(
        harness
            .node("workspace.edit.wrap")
            .expect("published")
            .hovered,
        "the rule and the refused row are stepped over"
    );
    assert!(
        !harness
            .node("workspace.edit.paste")
            .expect("published")
            .hovered
    );

    harness.keystrokes("down down");
    assert!(
        harness
            .node("workspace.edit.undo")
            .expect("published")
            .hovered,
        "the cursor wraps past the section label"
    );
}

#[gpui::test]
fn typing_a_letter_jumps_to_the_row_that_starts_with_it(cx: &mut TestAppContext) {
    let (mut harness, menu) = menu(cx);
    open(&mut harness, &menu);

    harness.keystrokes("w");
    assert!(
        harness
            .node("workspace.edit.wrap")
            .expect("published")
            .hovered
    );
}

#[gpui::test]
fn escape_closes_without_taking_anything(cx: &mut TestAppContext) {
    let (mut harness, menu) = menu(cx);
    let seen = events(&mut harness, &menu);
    open(&mut harness, &menu);

    harness.keystrokes("escape");

    assert!(taken(&seen).is_empty(), "escape reports no command");
    assert_eq!(
        *seen.borrow(),
        vec![MenuEvent::Opened, MenuEvent::Dismissed, MenuEvent::Closed]
    );
    assert_eq!(
        harness.node("workspace.edit").expect("published").expanded,
        Some(false)
    );
    let tree = harness.accessibility_tree();
    assert!(!tree["nodes"].as_object().is_some_and(|nodes| {
        nodes.values().any(|node| {
            node["element_id"] == "Name(\"workspace.edit.menu\")" && node["aria"]["role"] == "Menu"
        })
    }));
    assert!(harness.node("workspace.edit.menu").is_none());
}

#[gpui::test]
fn taking_a_command_reports_it_once_and_closes_the_chain(cx: &mut TestAppContext) {
    let (mut harness, menu) = menu(cx);
    let seen = events(&mut harness, &menu);
    open(&mut harness, &menu);

    // Open a submenu first, so closing has a chain to close.
    harness.keystrokes("down down right");
    assert!(harness.node("workspace.edit.share.link").is_some());

    harness.keystrokes("enter");

    assert_eq!(taken(&seen), vec!["share.link".to_string()]);
    assert_eq!(
        harness.node("workspace.edit").expect("published").expanded,
        Some(false)
    );
    assert!(
        harness.node("workspace.edit.share.link").is_none(),
        "the submenu closes with the menu that opened it"
    );
    assert_eq!(seen.borrow().last(), Some(&MenuEvent::Closed));
}

#[gpui::test]
fn a_submenu_opens_to_the_side_and_folds_back(cx: &mut TestAppContext) {
    let (mut harness, menu) = menu(cx);
    let seen = events(&mut harness, &menu);
    open(&mut harness, &menu);

    harness.keystrokes("down down");
    assert!(
        harness
            .node("workspace.edit.share")
            .expect("published")
            .hovered
    );

    harness.keystrokes("right");
    assert_eq!(
        harness
            .node("workspace.edit.share")
            .expect("published")
            .expanded,
        Some(true)
    );
    assert!(
        harness
            .node("workspace.edit.share.link")
            .expect("published")
            .hovered
    );
    let submenu = harness.bounds("workspace.edit.share.link").expect("bounds");
    let parent = harness.bounds("workspace.edit.share").expect("bounds");
    assert!(
        submenu.origin.x > parent.origin.x,
        "a submenu opens beside the row that owns it"
    );

    harness.keystrokes("left");
    assert!(
        harness.node("workspace.edit.share.link").is_none(),
        "leaving folds the submenu away"
    );
    assert!(
        harness
            .node("workspace.edit.share")
            .expect("published")
            .hovered,
        "the cursor returns to the row that opened it"
    );
    assert!(taken(&seen).is_empty(), "moving takes nothing");
}

#[gpui::test]
fn escape_folds_one_submenu_before_it_closes_the_menu(cx: &mut TestAppContext) {
    let (mut harness, menu) = menu(cx);
    open(&mut harness, &menu);
    harness.keystrokes("down down right");

    harness.keystrokes("escape");
    assert_eq!(
        harness.node("workspace.edit").expect("published").expanded,
        Some(true),
        "the first escape only folds the submenu away"
    );

    harness.keystrokes("escape");
    assert_eq!(
        harness.node("workspace.edit").expect("published").expanded,
        Some(false)
    );
}

#[gpui::test]
fn a_checkable_row_reports_its_intent_and_does_not_toggle_itself(cx: &mut TestAppContext) {
    let (mut harness, menu) = menu(cx);
    let seen = events(&mut harness, &menu);
    open(&mut harness, &menu);

    harness.click("workspace.edit.wrap");
    assert_eq!(taken(&seen), vec!["wrap".to_string()]);

    open(&mut harness, &menu);
    assert_eq!(
        harness
            .node("workspace.edit.wrap")
            .expect("published")
            .checked,
        Some(true),
        "the host still holds the answer, so the check has not moved"
    );
}

#[gpui::test]
fn a_refused_row_ignores_a_click(cx: &mut TestAppContext) {
    let (mut harness, menu) = menu(cx);
    let seen = events(&mut harness, &menu);
    open(&mut harness, &menu);

    harness.click("workspace.edit.paste");

    assert!(taken(&seen).is_empty());
    assert_eq!(
        harness.node("workspace.edit").expect("published").expanded,
        Some(true),
        "a refused row does not close the menu either"
    );
}

#[gpui::test]
fn clicking_outside_dismisses_the_menu(cx: &mut TestAppContext) {
    let (mut harness, menu) = menu(cx);
    let seen = events(&mut harness, &menu);
    open(&mut harness, &menu);

    harness
        .context()
        .simulate_click(gpui::point(px(560.0), px(380.0)), Modifiers::none());
    harness.context().run_until_parked();

    assert!(taken(&seen).is_empty());
    assert_eq!(
        harness.node("workspace.edit").expect("published").expanded,
        Some(false)
    );
}

fn context_menu(cx: &mut TestAppContext) -> (Harness, Entity<ContextMenu>) {
    let slot: Rc<RefCell<Option<Entity<ContextMenu>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let menu = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    ContextMenu::new("records.row", window, cx)
                        .name("Record actions")
                        .target("record-a04")
                        .menu(items())
                        .content(|_, _| {
                            div()
                                .w(px(240.0))
                                .h(px(80.0))
                                .child("Fixture record 0004")
                                .into_any_element()
                        })
                })
            })
            .clone();
        div()
            .w(px(600.0))
            .h(px(400.0))
            .child(menu)
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("context menu was built");
    (harness, entity)
}

fn context_events(
    harness: &mut Harness,
    menu: &Entity<ContextMenu>,
) -> Rc<RefCell<Vec<ContextMenuEvent>>> {
    let seen: Rc<RefCell<Vec<ContextMenuEvent>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    harness.update({
        let menu = menu.clone();
        move |_, cx| {
            cx.subscribe(&menu, move |_, event: &ContextMenuEvent, _| {
                sink.borrow_mut().push(event.clone());
            })
            .detach();
        }
    });
    seen
}

#[gpui::test]
fn a_right_click_opens_the_menu_at_the_pointer_and_reports_the_target(cx: &mut TestAppContext) {
    let (mut harness, menu) = context_menu(cx);
    let seen = context_events(&mut harness, &menu);

    let position = harness.point_in("records.row");
    harness.context().simulate_event(MouseDownEvent {
        button: MouseButton::Right,
        position,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    harness.context().run_until_parked();

    assert_eq!(
        *seen.borrow(),
        vec![ContextMenuEvent::Opened("record-a04".into())],
        "opening reports what was pointed at and selects nothing"
    );
    assert_eq!(
        harness
            .node("records.row.menu")
            .expect("published")
            .expanded,
        Some(true)
    );
    let tree = harness.accessibility_tree();
    let native = tree["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"records.row.menu\")" && node["aria"]["role"] == "Menu"
            })
        })
        .expect("native context menu");
    assert_eq!(native["aria"]["label"], "Record actions");
    let menu_bounds = harness.bounds("records.row.menu").expect("bounds");
    assert!(
        menu_bounds.origin.x >= position.x - px(1.0),
        "the menu opens at the pointer"
    );
}

#[gpui::test]
fn native_context_menu_dispatches_and_reports_native_dismissal(cx: &mut TestAppContext) {
    let (mut harness, menu) = context_menu(cx);
    let seen = context_events(&mut harness, &menu);
    let window = harness.window();
    harness
        .context()
        .set_native_context_menus_supported(window, true);

    let position = harness.point_in("records.row");
    harness.context().simulate_event(MouseDownEvent {
        button: MouseButton::Right,
        position,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    harness.context().run_until_parked();

    assert_eq!(
        harness.context().pending_context_menu_position(window),
        Some(position)
    );
    assert!(
        harness.node("records.row.menu").is_none(),
        "the OS owns native presentation rather than duplicating it in-window"
    );
    harness.context().select_context_menu_item(window, &[1]);
    harness.context().run_until_parked();
    assert_eq!(
        *seen.borrow(),
        vec![
            ContextMenuEvent::Opened("record-a04".into()),
            ContextMenuEvent::Invoked("undo".into()),
            ContextMenuEvent::Closed,
        ]
    );

    harness.update({
        let menu = menu.clone();
        move |window, cx| {
            menu.update(cx, |menu, cx| {
                menu.open_at(gpui::point(px(180.0), px(140.0)), window, cx);
            });
        }
    });
    harness.context().dismiss_context_menu(window);
    harness.context().run_until_parked();
    assert_eq!(
        &seen.borrow()[3..],
        &[
            ContextMenuEvent::Opened("record-a04".into()),
            ContextMenuEvent::Dismissed,
            ContextMenuEvent::Closed,
        ]
    );
}

#[gpui::test]
fn a_context_menu_reports_the_command_and_closes(cx: &mut TestAppContext) {
    let (mut harness, menu) = context_menu(cx);
    let seen = context_events(&mut harness, &menu);
    harness.update({
        let menu = menu.clone();
        move |window, cx| {
            menu.update(cx, |menu, cx| {
                menu.open_at(gpui::point(px(120.0), px(120.0)), window, cx);
            });
        }
    });

    harness.click("records.row.undo");

    assert_eq!(
        *seen.borrow(),
        vec![
            ContextMenuEvent::Opened("record-a04".into()),
            ContextMenuEvent::Invoked("undo".into()),
            ContextMenuEvent::Closed
        ]
    );
    assert!(harness.node("records.row.menu").is_none());
    assert!(
        harness.node("records.row").is_some(),
        "the wrapped region stays after the menu closes"
    );
}

#[gpui::test]
fn an_end_aligned_menu_hangs_off_the_trailing_edge_of_its_trigger(cx: &mut TestAppContext) {
    // A trigger sitting against the right of its container opens a menu wider
    // than itself. Lined up the usual way the surface would run off the page
    // and be slid back until it no longer pointed at anything; lined up on the
    // trailing edge it stays attached to what opened it.
    let slot: Rc<RefCell<Option<Entity<Menu>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let menu = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    Menu::new("workspace.edit", window, cx)
                        .trigger("Edit")
                        .items(items())
                        .hang(Hang::End)
                })
            })
            .clone();
        div()
            .w(px(600.0))
            .h(px(400.0))
            .flex()
            .justify_end()
            .child(menu)
            .into_any_element()
    });
    harness.snapshot();

    let menu = slot.borrow().clone().expect("built");
    open(&mut harness, &menu);

    let trigger = harness.bounds("workspace.edit.trigger").expect("laid out");
    let surface = harness.bounds("workspace.edit.undo").expect("laid out");

    assert!(
        surface.right() <= trigger.right() + px(1.0),
        "the surface ends where its trigger ends: trigger {trigger:?}, row {surface:?}"
    );
    assert!(
        surface.left() < trigger.left(),
        "and grows back across the page rather than off it"
    );
}
