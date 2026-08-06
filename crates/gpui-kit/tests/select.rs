//! A select opens, moves, and reports a choice. It never decides one.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, IntoElement, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

fn options() -> Vec<SelectOption> {
    vec![
        SelectOption::new("anthropic", "Anthropic"),
        SelectOption::new("openai", "OpenAI").description("Requires a key"),
        SelectOption::new("local", "Local runtime").disabled(true),
    ]
}

fn select(cx: &mut TestAppContext) -> (Harness, Entity<Select>) {
    let slot: Rc<RefCell<Option<Entity<Select>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    Select::new("settings.provider", window, cx)
                        .options(options())
                        .selected("anthropic")
                        .placeholder("Choose a provider")
                })
            })
            .clone()
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("select was built");
    (harness, entity)
}

#[gpui::test]
fn a_closed_select_shows_the_current_answer(cx: &mut TestAppContext) {
    let (mut harness, _entity) = select(cx);
    let node = harness.node("settings.provider").expect("published");

    assert_eq!(node.value.as_deref(), Some("Anthropic"));
    assert_eq!(node.expanded, Some(false));
    assert!(
        harness.node("settings.provider.openai").is_none(),
        "a closed select must not publish its options"
    );
}

#[gpui::test]
fn clicking_opens_the_menu(cx: &mut TestAppContext) {
    let (mut harness, _entity) = select(cx);
    harness.click("settings.provider");

    assert_eq!(
        harness
            .node("settings.provider")
            .expect("published")
            .expanded,
        Some(true)
    );
    let option = harness
        .node("settings.provider.openai")
        .expect("options are published while open");
    assert_eq!(option.text.as_deref(), Some("OpenAI"));
    assert_eq!(option.checked, Some(false));
    assert_eq!(
        harness
            .node("settings.provider.anthropic")
            .expect("published")
            .checked,
        Some(true)
    );
}

#[gpui::test]
fn choosing_reports_the_option_without_changing_the_value(cx: &mut TestAppContext) {
    let (mut harness, entity) = select(cx);
    let chosen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = chosen.clone();
    harness.update({
        let entity = entity.clone();
        move |_, cx| {
            cx.subscribe(&entity, move |_, event: &SelectEvent, _| {
                if let SelectEvent::Selected(id) = event {
                    sink.borrow_mut().push(id.to_string());
                }
            })
            .detach();
        }
    });

    harness.click("settings.provider");
    harness.click("settings.provider.openai");

    assert_eq!(*chosen.borrow(), vec!["openai".to_string()]);
    // The owner has not applied it, so the control still reports the old one.
    assert_eq!(
        harness
            .node("settings.provider")
            .expect("published")
            .value
            .as_deref(),
        Some("Anthropic")
    );
    assert_eq!(
        harness
            .node("settings.provider")
            .expect("published")
            .expanded,
        Some(false),
        "choosing closes the menu"
    );
}

#[gpui::test]
fn a_refused_option_cannot_be_chosen(cx: &mut TestAppContext) {
    let (mut harness, entity) = select(cx);
    let chosen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = chosen.clone();
    harness.update({
        let entity = entity.clone();
        move |_, cx| {
            cx.subscribe(&entity, move |_, event: &SelectEvent, _| {
                if let SelectEvent::Selected(id) = event {
                    sink.borrow_mut().push(id.to_string());
                }
            })
            .detach();
        }
    });

    harness.click("settings.provider");
    harness.click("settings.provider.local");

    assert!(chosen.borrow().is_empty());
    assert!(
        harness
            .node("settings.provider.local")
            .expect("published")
            .disabled
    );
}

#[gpui::test]
fn the_keyboard_opens_moves_and_chooses(cx: &mut TestAppContext) {
    let (mut harness, entity) = select(cx);
    let chosen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = chosen.clone();
    harness.update({
        let entity = entity.clone();
        move |_, cx| {
            cx.subscribe(&entity, move |_, event: &SelectEvent, _| {
                if let SelectEvent::Selected(id) = event {
                    sink.borrow_mut().push(id.to_string());
                }
            })
            .detach();
        }
    });

    harness.click("settings.provider");
    harness.keystrokes("down enter");

    assert_eq!(*chosen.borrow(), vec!["openai".to_string()]);
}

#[gpui::test]
fn escape_closes_without_choosing(cx: &mut TestAppContext) {
    let (mut harness, entity) = select(cx);
    let chosen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = chosen.clone();
    harness.update({
        let entity = entity.clone();
        move |_, cx| {
            cx.subscribe(&entity, move |_, event: &SelectEvent, _| {
                if let SelectEvent::Selected(id) = event {
                    sink.borrow_mut().push(id.to_string());
                }
            })
            .detach();
        }
    });

    harness.click("settings.provider");
    harness.keystrokes("escape");

    assert!(chosen.borrow().is_empty());
    assert_eq!(
        harness
            .node("settings.provider")
            .expect("published")
            .expanded,
        Some(false)
    );
}

#[gpui::test]
fn a_disabled_select_never_opens(cx: &mut TestAppContext) {
    let slot: Rc<RefCell<Option<Entity<Select>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    Select::new("settings.provider", window, cx)
                        .options(options())
                        .disabled(true)
                })
            })
            .clone()
            .into_any_element()
    });

    harness.click("settings.provider");
    assert_eq!(
        harness
            .node("settings.provider")
            .expect("published")
            .expanded,
        Some(false)
    );
}

#[gpui::test]
fn replacing_the_options_drops_a_selection_no_longer_offered(cx: &mut TestAppContext) {
    let (mut harness, entity) = select(cx);
    harness.update(move |_, cx| {
        entity.update(cx, |select, cx| {
            select.set_options(vec![SelectOption::new("openai", "OpenAI")], cx);
        });
    });

    let node = harness.node("settings.provider").expect("published");
    assert_eq!(node.value, None);
    assert_eq!(node.placeholder.as_deref(), Some("Choose a provider"));
}
