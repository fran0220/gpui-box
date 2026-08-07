//! The ordinary-application vocabulary: toggles, a collapsible region, a
//! hover card that can be reached, a menubar, a copy button that does not lie,
//! and a frame that holds its ratio.
//!
//! Every assertion here goes through the public API and the semantic tree, and
//! every interaction is simulated input rather than a method call standing in
//! for one.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AppContext as _, Entity, IntoElement, Modifiers, Point, SharedString, TestAppContext, div,
    point, prelude::*, px,
};
use gpui_kit::prelude::*;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_testkit::harness::Harness;

type Calls<T> = Rc<RefCell<Vec<T>>>;

fn recorder<T: 'static>() -> (Calls<T>, Calls<T>) {
    let calls: Calls<T> = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

/// Moves the pointer without a button down, which is what makes hover happen.
fn hover_at(harness: &mut Harness, position: Point<gpui::Pixels>) {
    harness
        .context()
        .simulate_mouse_move(position, None, Modifiers::none());
    harness.context().run_until_parked();
}

fn hover(harness: &mut Harness, id: &str) {
    let at = harness.point_in(id);
    hover_at(harness, at);
}

// ---------------------------------------------------------------- Toggle

#[gpui::test]
fn a_toggle_reports_in_and_out_as_two_different_answers(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .child(
                Toggle::new("format.bold")
                    .label("Bold")
                    .pressed(true)
                    .on_press(|_, _, _| {}),
            )
            .child(
                Toggle::new("format.italic")
                    .label("Italic")
                    .pressed(false)
                    .on_press(|_, _, _| {}),
            )
            .into_any_element()
    });

    let pressed = harness.node("format.bold").expect("published");
    let released = harness.node("format.italic").expect("published");

    assert_eq!(
        pressed.role,
        Role::Button,
        "a toggle is a button, not a switch"
    );
    assert_eq!(pressed.checked, Some(true));
    assert_eq!(
        released.checked,
        Some(false),
        "out is a state, not the absence of one"
    );
}

#[gpui::test]
fn pressing_a_toggle_reports_the_state_it_asks_for_and_moves_nothing(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<bool>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Toggle::new("format.bold")
            .label("Bold")
            .pressed(true)
            .on_press(move |next, _, _| sink.borrow_mut().push(next))
            .into_any_element()
    });

    harness.click("format.bold");

    assert_eq!(*calls.borrow(), vec![false]);
    assert_eq!(
        harness.node("format.bold").expect("published").checked,
        Some(true),
        "the answer is the caller's, so nothing moved"
    );
}

#[gpui::test]
fn a_disabled_toggle_installs_no_handler(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<bool>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Toggle::new("format.bold")
            .label("Bold")
            .pressed(false)
            .disabled(true)
            .on_press(move |next, _, _| sink.borrow_mut().push(next))
            .into_any_element()
    });

    harness.click("format.bold");
    harness.keystrokes("enter");
    harness.keystrokes("space");

    assert!(calls.borrow().is_empty());
    assert!(harness.node("format.bold").expect("published").disabled);
}

// ----------------------------------------------------------- ToggleGroup

fn group(
    cx: &mut TestAppContext,
    selection: ToggleSelection,
    pressed: &'static [&'static str],
) -> (Harness, Calls<Vec<String>>) {
    let (calls, sink) = recorder::<Vec<String>>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        ToggleGroup::new("format")
            .label("Formatting")
            .selection(selection)
            .items([
                ToggleItem::new("bold", "Bold"),
                ToggleItem::new("italic", "Italic"),
                ToggleItem::new("underline", "Underline").disabled(true),
            ])
            .pressed_ids(pressed)
            .on_change(move |next, _, _, _| {
                sink.borrow_mut()
                    .push(next.iter().map(|id| id.to_string()).collect())
            })
            .into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn a_group_that_takes_several_adds_to_the_set(cx: &mut TestAppContext) {
    let (mut harness, calls) = group(cx, ToggleSelection::Any, &["bold"]);

    assert_eq!(
        harness.node("format.bold").expect("published").checked,
        Some(true)
    );
    assert_eq!(
        harness.node("format.italic").expect("published").checked,
        Some(false)
    );

    harness.click("format.italic");

    assert_eq!(
        *calls.borrow(),
        vec![vec!["bold".to_string(), "italic".to_string()]]
    );
}

#[gpui::test]
fn a_group_that_takes_at_most_one_replaces_and_can_be_emptied(cx: &mut TestAppContext) {
    let (mut harness, calls) = group(cx, ToggleSelection::AtMostOne, &["bold"]);

    harness.click("format.italic");
    assert_eq!(*calls.borrow(), vec![vec!["italic".to_string()]]);

    calls.borrow_mut().clear();
    harness.click("format.bold");
    assert_eq!(
        *calls.borrow(),
        vec![Vec::<String>::new()],
        "pressing the one that is in empties the set, which a radio group cannot do"
    );
}

#[gpui::test]
fn a_refused_toggle_in_a_group_installs_no_handler(cx: &mut TestAppContext) {
    let (mut harness, calls) = group(cx, ToggleSelection::Any, &["bold"]);

    harness.click("format.underline");

    assert!(calls.borrow().is_empty());
    assert!(
        harness
            .node("format.underline")
            .expect("published")
            .disabled
    );
}

#[gpui::test]
fn every_toggle_in_a_group_names_the_group_as_its_parent(cx: &mut TestAppContext) {
    let (mut harness, _calls) = group(cx, ToggleSelection::Any, &[]);
    let node = harness.node("format.bold").expect("published");
    assert_eq!(node.parent.as_deref(), Some("format"));
    assert_eq!(
        harness.node("format").expect("published").text.as_deref(),
        Some("Formatting")
    );
}

// ------------------------------------------------------------ Collapsible

fn collapsible(cx: &mut TestAppContext, open: bool) -> (Harness, Calls<bool>) {
    let (calls, sink) = recorder::<bool>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, cx| {
        let sink = sink.clone();
        Collapsible::new("panel.advanced", "Advanced")
            .open(open)
            .body(
                div()
                    .w(px(200.0))
                    .h(px(40.0))
                    .semantic_in(cx, NodeSpec::new("panel.advanced.body", Role::Region)),
            )
            .on_toggle(move |next, _, _| sink.borrow_mut().push(next))
            .into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn a_shut_collapsible_has_no_body_in_the_tree_at_all(cx: &mut TestAppContext) {
    let (mut harness, _calls) = collapsible(cx, false);
    let snapshot = harness.snapshot();

    assert!(
        !snapshot.contains("panel.advanced.body"),
        "a shut body is gone, not merely invisible"
    );
    assert_eq!(
        harness
            .node("panel.advanced.header")
            .expect("published")
            .expanded,
        Some(false)
    );
}

#[gpui::test]
fn an_open_collapsible_publishes_its_body(cx: &mut TestAppContext) {
    let (mut harness, _calls) = collapsible(cx, true);
    harness.advance(Duration::from_millis(600));

    assert!(harness.snapshot().contains("panel.advanced.body"));
    assert_eq!(
        harness
            .node("panel.advanced.header")
            .expect("published")
            .expanded,
        Some(true)
    );
}

#[gpui::test]
fn the_header_reports_the_state_it_asks_for_by_click_and_by_key(cx: &mut TestAppContext) {
    let (mut harness, calls) = collapsible(cx, false);

    harness.click("panel.advanced.header");
    assert_eq!(*calls.borrow(), vec![true]);

    calls.borrow_mut().clear();
    harness.keystrokes("enter");
    assert_eq!(
        *calls.borrow(),
        vec![true],
        "the keyboard reaches the header the pointer reaches"
    );
}

#[gpui::test]
fn a_refused_collapsible_installs_no_handler(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<bool>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Collapsible::new("panel.policy", "Managed by policy")
            .disabled(true)
            .body(div().child("Set by the administrator."))
            .on_toggle(move |next, _, _| sink.borrow_mut().push(next))
            .into_any_element()
    });

    harness.click("panel.policy.header");

    assert!(calls.borrow().is_empty());
    assert!(
        harness
            .node("panel.policy.header")
            .expect("published")
            .disabled
    );
}

// -------------------------------------------------------------- HoverCard

fn hover_card(cx: &mut TestAppContext) -> (Harness, Entity<HoverCard>) {
    let slot: Rc<RefCell<Option<Entity<HoverCard>>>> = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let card = build
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    HoverCard::new("run.preview", window, cx)
                        .name("Run 4821")
                        .open_delay(Duration::from_millis(100))
                        .grace(Duration::from_millis(300))
                        .trigger(|_, _| div().w(px(80.0)).h(px(20.0)).into_any_element())
                        .content(|_, _| div().w(px(160.0)).h(px(80.0)).into_any_element())
                })
            })
            .clone();
        div()
            .w(px(400.0))
            .h(px(300.0))
            .p(px(40.0))
            .child(card)
            .into_any_element()
    });
    harness.snapshot();
    let card = slot.borrow().clone().expect("built");
    (harness, card)
}

#[gpui::test]
fn a_pointer_passing_through_opens_nothing(cx: &mut TestAppContext) {
    let (mut harness, card) = hover_card(cx);

    hover(&mut harness, "run.preview.trigger");
    // Gone again before the opening countdown could run out.
    hover_at(&mut harness, point(px(0.0), px(0.0)));
    harness.advance(Duration::from_millis(400));

    assert!(!harness.update(|_, cx| card.read(cx).is_open()));
}

#[gpui::test]
fn resting_on_the_trigger_opens_the_card(cx: &mut TestAppContext) {
    let (mut harness, card) = hover_card(cx);

    hover(&mut harness, "run.preview.trigger");
    harness.advance(Duration::from_millis(200));

    assert!(harness.update(|_, cx| card.read(cx).is_open()));
    assert!(harness.snapshot().contains("run.preview.card"));
}

#[gpui::test]
fn the_card_survives_the_trip_from_the_trigger_into_it(cx: &mut TestAppContext) {
    let (mut harness, card) = hover_card(cx);

    hover(&mut harness, "run.preview.trigger");
    harness.advance(Duration::from_millis(200));
    assert!(harness.update(|_, cx| card.read(cx).is_open()));

    // The gap: the pointer has left the trigger and has not reached the card.
    // Without a grace period this is where the card would vanish and the trip
    // would be unwinnable.
    hover_at(&mut harness, point(px(0.0), px(0.0)));
    harness.advance(Duration::from_millis(100));
    assert!(
        harness.update(|_, cx| card.read(cx).is_open()),
        "the card is leaving and has not left"
    );
    assert!(harness.update(|_, cx| card.read(cx).is_leaving()));

    // Arriving in the card calls the departure off.
    hover(&mut harness, "run.preview.card");
    assert!(!harness.update(|_, cx| card.read(cx).is_leaving()));

    harness.advance(Duration::from_millis(1000));
    assert!(
        harness.update(|_, cx| card.read(cx).is_open()),
        "the pointer is on the card, so no amount of time closes it"
    );
}

#[gpui::test]
fn leaving_both_surfaces_closes_the_card_once_the_grace_runs_out(cx: &mut TestAppContext) {
    let (mut harness, card) = hover_card(cx);

    hover(&mut harness, "run.preview.trigger");
    harness.advance(Duration::from_millis(200));
    hover_at(&mut harness, point(px(0.0), px(0.0)));
    harness.advance(Duration::from_millis(500));

    assert!(!harness.update(|_, cx| card.read(cx).is_open()));
    assert!(!harness.snapshot().contains("run.preview.card"));
}

#[gpui::test]
fn escape_closes_the_card_and_hands_the_keyboard_back(cx: &mut TestAppContext) {
    let (mut harness, card) = hover_card(cx);

    harness.update({
        let card = card.clone();
        move |_, cx| card.update(cx, |card, cx| card.open(cx))
    });
    assert!(harness.snapshot().contains("run.preview.card"));

    harness.click("run.preview.trigger");
    harness.keystrokes("escape");

    assert!(!harness.update(|_, cx| card.read(cx).is_open()));
    assert!(
        harness
            .node("run.preview.trigger")
            .expect("published")
            .focused,
        "the keyboard came back to where it was"
    );
}

// ---------------------------------------------------------------- Menubar

fn menus() -> Vec<MenubarMenu> {
    vec![
        MenubarMenu::new(
            "file",
            "File",
            [
                MenuItem::command("file.new", "New run"),
                MenuItem::submenu(
                    "file.export",
                    "Export",
                    [MenuItem::command("file.export.json", "As JSON")],
                ),
            ],
        ),
        MenubarMenu::new("edit", "Edit", [MenuItem::command("edit.undo", "Undo")]),
        MenubarMenu::new("view", "View", [MenuItem::command("view.zoom", "Zoom in")]),
        MenubarMenu::new("policy", "Policy", []).disabled(true),
    ]
}

fn menubar(cx: &mut TestAppContext) -> (Harness, Entity<Menubar>) {
    let slot: Rc<RefCell<Option<Entity<Menubar>>>> = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let bar = build
            .borrow_mut()
            .get_or_insert_with(|| cx.new(|cx| Menubar::new("app.menubar", menus(), window, cx)))
            .clone();
        div()
            .w(px(600.0))
            .h(px(400.0))
            .child(bar)
            .into_any_element()
    });
    harness.snapshot();
    let bar = slot.borrow().clone().expect("built");
    (harness, bar)
}

fn open_menu(harness: &mut Harness, bar: &Entity<Menubar>) -> Option<String> {
    harness.update({
        let bar = bar.clone();
        move |_, cx| bar.read(cx).open_menu().map(|id| id.to_string())
    })
}

#[gpui::test]
fn hovering_a_title_before_anything_is_open_opens_nothing(cx: &mut TestAppContext) {
    let (mut harness, bar) = menubar(cx);

    hover(&mut harness, "app.menubar.edit.trigger");

    assert_eq!(open_menu(&mut harness, &bar), None);
    assert!(!harness.snapshot().contains("app.menubar.edit.edit.undo"));
}

#[gpui::test]
fn hovering_a_sibling_once_one_is_open_moves_the_open_menu(cx: &mut TestAppContext) {
    let (mut harness, bar) = menubar(cx);

    harness.click("app.menubar.file.trigger");
    assert_eq!(open_menu(&mut harness, &bar).as_deref(), Some("file"));

    hover(&mut harness, "app.menubar.edit.trigger");

    assert_eq!(open_menu(&mut harness, &bar).as_deref(), Some("edit"));
    assert!(
        harness.snapshot().contains("app.menubar.edit.edit.undo"),
        "the sibling's rows are on screen"
    );
    assert!(
        !harness.snapshot().contains("app.menubar.file.file.new"),
        "and the first menu's are not: at most one is open"
    );
}

#[gpui::test]
fn a_refused_title_never_opens(cx: &mut TestAppContext) {
    let (mut harness, bar) = menubar(cx);

    harness.click("app.menubar.file.trigger");

    hover(&mut harness, "app.menubar.policy");
    assert_eq!(
        open_menu(&mut harness, &bar).as_deref(),
        Some("file"),
        "hovering a refused title does not move the open menu onto it"
    );

    // Clicking it dismisses whatever was open, because a click outside a menu
    // always does. What must not happen is the refused title opening: there is
    // no menu behind it and no handler on it.
    harness.click("app.menubar.policy");
    assert_ne!(open_menu(&mut harness, &bar).as_deref(), Some("policy"));
    assert!(
        harness
            .node("app.menubar.policy")
            .expect("published")
            .disabled
    );
}

#[gpui::test]
fn the_reading_order_arrows_step_between_titles(cx: &mut TestAppContext) {
    let (mut harness, bar) = menubar(cx);

    harness.click("app.menubar.file.trigger");
    harness.keystrokes("right");
    assert_eq!(open_menu(&mut harness, &bar).as_deref(), Some("edit"));

    harness.keystrokes("right");
    assert_eq!(open_menu(&mut harness, &bar).as_deref(), Some("view"));

    harness.keystrokes("right");
    assert_eq!(
        open_menu(&mut harness, &bar).as_deref(),
        Some("view"),
        "the row has ends: the refused title is stepped over and nothing wraps"
    );

    harness.keystrokes("left");
    assert_eq!(open_menu(&mut harness, &bar).as_deref(), Some("edit"));
}

#[gpui::test]
fn an_open_submenu_keeps_the_sideways_keys(cx: &mut TestAppContext) {
    let (mut harness, bar) = menubar(cx);

    harness.click("app.menubar.file.trigger");
    // Down onto `New run`, down onto `Export`, right to enter it.
    harness.keystrokes("down right");

    assert_eq!(
        open_menu(&mut harness, &bar).as_deref(),
        Some("file"),
        "right entered the submenu rather than moving to the next title"
    );
    assert!(
        harness
            .snapshot()
            .contains("app.menubar.file.file.export.json")
    );
}

#[gpui::test]
fn escape_closes_the_menu_and_returns_focus_to_its_title(cx: &mut TestAppContext) {
    let (mut harness, bar) = menubar(cx);

    harness.click("app.menubar.file.trigger");
    harness.keystrokes("escape");

    assert_eq!(open_menu(&mut harness, &bar), None);
    assert!(
        harness
            .node("app.menubar.file.trigger")
            .expect("published")
            .focused,
        "the keyboard came back to the title that was opened"
    );
}

// ------------------------------------------------------------- CopyButton

fn copy_button(
    cx: &mut TestAppContext,
    outcome: Result<(), &'static str>,
) -> (Harness, Entity<CopyButton>) {
    let slot: Rc<RefCell<Option<Entity<CopyButton>>>> = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let button = build
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    CopyButton::new("token.copy", window, cx)
                        .text("sk-secret-value")
                        .confirmation(Duration::from_millis(200))
                        .copier(move |_, _| {
                            outcome.map_err(|reason| SharedString::from(reason.to_string()))
                        })
                })
            })
            .clone();
        div().w(px(300.0)).child(button).into_any_element()
    });
    harness.snapshot();
    let button = slot.borrow().clone().expect("built");
    (harness, button)
}

#[gpui::test]
fn a_copy_that_worked_confirms_it(cx: &mut TestAppContext) {
    let (mut harness, button) = copy_button(cx, Ok(()));

    assert!(
        harness.node("token.copy.status").is_none(),
        "nothing is claimed before anything is tried"
    );

    harness.click("token.copy.action");

    let status = harness.node("token.copy.status").expect("published");
    assert!(!status.invalid);
    assert!(harness.update(|_, cx| button.read(cx).state().is_copied()));
}

#[gpui::test]
fn a_copy_that_failed_does_not_claim_success(cx: &mut TestAppContext) {
    let (mut harness, button) = copy_button(cx, Err("The clipboard did not take it."));

    harness.click("token.copy.action");

    let status = harness.node("token.copy.status").expect("published");
    assert!(status.invalid, "the failure is published as a failure");
    assert_eq!(
        status.text.as_deref(),
        Some("The clipboard did not take it."),
        "the host's reason is shown verbatim"
    );
    assert!(harness.update(|_, cx| button.read(cx).state().is_failed()));
    assert!(!harness.update(|_, cx| button.read(cx).state().is_copied()));
    assert!(harness.node("token.copy").expect("published").invalid);
}

#[gpui::test]
fn a_confirmation_expires_and_a_refusal_does_not(cx: &mut TestAppContext) {
    let (mut harness, button) = copy_button(cx, Ok(()));
    harness.click("token.copy.action");
    harness.advance(Duration::from_millis(400));
    assert!(
        harness.node("token.copy.status").is_none(),
        "the confirmation had nothing left to say"
    );
    assert!(harness.update(|_, cx| matches!(button.read(cx).state(), CopyState::Idle)));

    let (mut harness, _) = copy_button(cx, Err("Refused"));
    harness.click("token.copy.action");
    harness.advance(Duration::from_millis(2000));
    assert!(
        harness
            .node("token.copy.status")
            .expect("published")
            .invalid,
        "a failure nobody saw is a failure that was never reported"
    );
}

#[gpui::test]
fn the_payload_never_reaches_the_tree(cx: &mut TestAppContext) {
    let (mut harness, _button) = copy_button(cx, Ok(()));
    harness.click("token.copy.action");

    let leaked = harness
        .snapshot()
        .nodes
        .iter()
        .any(|node| node.text.as_deref() == Some("sk-secret-value"));
    assert!(!leaked, "a copy button publishes itself, never its payload");
}

#[gpui::test]
fn a_disabled_copy_button_installs_no_handler(cx: &mut TestAppContext) {
    let slot: Rc<RefCell<Option<Entity<CopyButton>>>> = Rc::new(RefCell::new(None));
    let build = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let button = build
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    CopyButton::new("token.copy", window, cx)
                        .text("sk-secret-value")
                        .disabled(true)
                        .copier(|_, _| Ok(()))
                })
            })
            .clone();
        div().w(px(300.0)).child(button).into_any_element()
    });
    harness.snapshot();
    let button = slot.borrow().clone().expect("built");

    harness.click("token.copy.action");

    assert!(harness.node("token.copy.status").is_none());
    assert!(harness.update(|_, cx| matches!(button.read(cx).state(), CopyState::Idle)));
}

// ------------------------------------------------------------ AspectRatio

fn ratio_bounds(
    cx: &mut TestAppContext,
    fit: AspectFit,
    ratio: f32,
    box_width: f32,
    box_height: f32,
) -> (f32, f32) {
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        div()
            .w(px(box_width))
            .h(px(box_height))
            .child(
                AspectRatio::new("preview.frame", ratio)
                    .fit(fit)
                    .child(div().size_full()),
            )
            .into_any_element()
    });
    let node = harness.node("preview.frame").expect("published");
    (node.bounds.width, node.bounds.height)
}

#[gpui::test]
fn a_width_driven_frame_takes_its_height_from_the_ratio(cx: &mut TestAppContext) {
    let (width, height) = ratio_bounds(cx, AspectFit::Width, 2.0, 300.0, 400.0);
    assert!(
        (width - 300.0).abs() < 0.5,
        "width came from the parent: {width}"
    );
    assert!(
        (height - 150.0).abs() < 0.5,
        "height came from the ratio: {height}"
    );
}

#[gpui::test]
fn a_height_driven_frame_takes_its_width_from_the_ratio(cx: &mut TestAppContext) {
    let (width, height) = ratio_bounds(cx, AspectFit::Height, 2.0, 500.0, 200.0);
    assert!(
        (height - 200.0).abs() < 0.5,
        "height came from the parent: {height}"
    );
    assert!(
        (width - 400.0).abs() < 0.5,
        "width came from the ratio: {width}"
    );
}

#[gpui::test]
fn the_ratio_holds_even_where_the_parent_constrains_both(cx: &mut TestAppContext) {
    // The box is 300 by 400 and the ratio is 2:1. Width-driven, the frame is
    // 300 by 150 and leaves space below; height-driven it is 800 by 400 and
    // overflows sideways. Both hold the ratio, which is the promise, and
    // `fit` is what decides which of the two happens.
    let (wide, tall) = ratio_bounds(cx, AspectFit::Width, 2.0, 300.0, 400.0);
    assert!((wide / tall - 2.0).abs() < 0.02);

    let (wide, tall) = ratio_bounds(cx, AspectFit::Height, 2.0, 300.0, 400.0);
    assert!((wide / tall - 2.0).abs() < 0.02);
    assert!(tall > 399.0, "the named dimension is the one that is kept");
}

#[gpui::test]
fn a_ratio_that_is_not_a_ratio_falls_back_to_a_square(cx: &mut TestAppContext) {
    let (width, height) = ratio_bounds(cx, AspectFit::Width, 0.0, 200.0, 400.0);
    assert!((width - height).abs() < 0.5, "{width} by {height}");
}
