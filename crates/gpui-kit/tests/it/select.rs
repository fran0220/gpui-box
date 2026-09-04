//! A select opens, moves, and reports a choice. It never decides one.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AppContext as _, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    TestAppContext, div, prelude::*, px, size,
};
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
                        .name("Provider")
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
fn select_native_role_name_value_and_identity_are_stable(cx: &mut TestAppContext) {
    let (mut harness, entity) = select(cx);
    let tree = harness.accessibility_tree();
    let field = tree["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"settings.provider\")"
                    && node["aria"]["role"] == "ComboBox"
            })
        })
        .expect("native select");
    assert_eq!(field["aria"]["label"], "Provider");
    assert_eq!(field["aria"]["value"], "Anthropic");
    let native_id = field["accesskit_id"].clone();

    harness.update(|_, cx| {
        entity.update(cx, |select, cx| {
            select.set_selected(Some("openai".into()), cx)
        });
    });
    let tree = harness.accessibility_tree();
    let field = tree["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"settings.provider\")"
                    && node["aria"]["role"] == "ComboBox"
            })
        })
        .expect("updated native select");
    assert_eq!(field["accesskit_id"], native_id);
    assert_eq!(field["aria"]["value"], "OpenAI");
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
                        .name("Provider")
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
    let tree = harness.accessibility_tree();
    let field = tree["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"settings.provider\")"
                    && node["aria"]["role"] == "ComboBox"
            })
        })
        .expect("disabled native select");
    assert_eq!(field["aria"]["label"], "Provider");
    assert_eq!(field["aria"]["disabled"], true);
    let actions = field["aria"]["on_action"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for action in ["Focus", "Click"] {
        assert!(!actions.iter().any(|candidate| candidate == action));
    }
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

fn popup_options() -> Vec<SelectOption> {
    let mut options = (0..14)
        .map(|index| {
            SelectOption::new(
                format!("model-{index:02}"),
                format!("Agent model {index:02}"),
            )
        })
        .collect::<Vec<_>>();
    options.push(SelectOption::new("unknown", "Unknown model").description(
        "This model may not support chat in direct mode.\nChoose a chat-capable model.",
    ));
    options.push(SelectOption::new("managed", "Managed model").disabled(true));
    options
}

fn popup_select(
    cx: &mut TestAppContext,
    near_bottom: bool,
    selected: &'static str,
) -> (Harness, Entity<Select>, Rc<Cell<usize>>) {
    let slot: Rc<RefCell<Option<Entity<Select>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let footer_hits = Rc::new(Cell::new(0));
    let build_footer_hits = footer_hits.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let select = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    Select::new("models.picker", window, cx)
                        .name("All Agent models")
                        .options(popup_options())
                        .selected(selected)
                })
            })
            .clone();
        let trigger = if near_bottom {
            div()
                .h_full()
                .flex()
                .flex_col()
                .justify_end()
                .pb(px(44.0))
                .child(select)
                .into_any_element()
        } else {
            div().pt(px(12.0)).child(select).into_any_element()
        };
        let footer_hits = build_footer_hits.clone();
        let theme = cx.theme().clone();
        let footer = gpui::deferred(
            div()
                .absolute()
                // Ends immediately before the trigger but overlaps the
                // selected row after the menu flips above it.
                .left_0()
                .w_full()
                .when(near_bottom, |element| element.bottom(px(80.0)).h(px(150.0)))
                .when(!near_bottom, |element| element.bottom_0().h(px(44.0)))
                .occlude()
                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                    footer_hits.set(footer_hits.get() + 1);
                }),
        )
        .priority(gpui_kit::overlay::priority(&theme, Layer::Dock));

        div()
            .relative()
            .size_full()
            .child(
                div()
                    .id("models.surface")
                    .w(px(300.0))
                    .h_full()
                    .overflow_y_scroll()
                    .child(trigger),
            )
            .child(footer)
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("select was built");
    (harness, entity, footer_hits)
}

fn resize(harness: &mut Harness, width: f32, height: f32) {
    harness
        .context()
        .simulate_resize(size(px(width), px(height)));
    // The first frame measures the moved trigger; the second consumes it.
    harness.frame();
    harness.frame();
}

fn assert_inside(inner: gpui::Bounds<gpui::Pixels>, outer: gpui::Bounds<gpui::Pixels>) {
    let epsilon = px(0.5);
    assert!(
        inner.top() + epsilon >= outer.top(),
        "{inner:?} above {outer:?}"
    );
    assert!(
        inner.bottom() <= outer.bottom() + epsilon,
        "{inner:?} below {outer:?}"
    );
}

#[gpui::test]
fn popup_flips_above_the_footer_and_reveals_the_whole_selected_row(cx: &mut TestAppContext) {
    let (mut harness, entity, footer_hits) = popup_select(cx, true, "unknown");
    let selected = Rc::new(Cell::new(0));
    let selected_sink = selected.clone();
    harness.update(move |_, cx| {
        cx.subscribe(&entity, move |_, event: &SelectEvent, _| {
            if matches!(event, SelectEvent::Selected(id) if id.as_ref() == "unknown") {
                selected_sink.set(selected_sink.get() + 1);
            }
        })
        .detach();
    });
    resize(&mut harness, 520.0, 480.0);

    harness.click("models.picker");
    let margin = harness.update(|_, cx| cx.theme().spacing.sm);
    let trigger = harness.bounds("models.picker").expect("trigger bounds");
    let menu = harness.bounds("models.picker.menu").expect("menu bounds");
    let active = harness
        .bounds("models.picker.unknown")
        .expect("selected row bounds");

    assert!(menu.bottom() <= trigger.top(), "the menu must flip above");
    assert!(f32::from(menu.top()) >= margin - 0.5);
    assert!(f32::from(menu.bottom()) <= 480.0 - margin + 0.5);
    assert!(active.size.height > px(40.0), "the warning is multi-line");
    assert_inside(active, menu);

    // The dock-priority footer overlaps this row geometrically. The popover
    // layer must still own the hit and report the selection.
    harness.click("models.picker.unknown");
    assert_eq!(selected.get(), 1);
    assert_eq!(footer_hits.get(), 0);
}

#[gpui::test]
fn narrow_popup_stays_below_the_trigger_and_home_end_reveal_enabled_edges(cx: &mut TestAppContext) {
    let (mut harness, entity, _footer_hits) = popup_select(cx, false, "model-00");
    resize(&mut harness, 360.0, 240.0);
    harness.click("models.picker");

    let margin = harness.update(|_, cx| cx.theme().spacing.sm);
    let trigger = harness.bounds("models.picker").expect("trigger bounds");
    let menu = harness.bounds("models.picker.menu").expect("menu bounds");
    assert!(menu.top() >= trigger.bottom(), "the menu must remain below");
    assert!(f32::from(menu.top()) >= margin - 0.5);
    assert!(f32::from(menu.bottom()) <= 240.0 - margin + 0.5);

    harness.keystrokes("end");
    let end = harness
        .node("models.picker.unknown")
        .expect("last enabled row");
    assert!(end.hovered);
    assert_inside(
        harness.bounds("models.picker.unknown").expect("end bounds"),
        harness.bounds("models.picker.menu").expect("menu bounds"),
    );
    assert!(
        !harness
            .node("models.picker.managed")
            .expect("disabled edge")
            .hovered,
        "End skips a disabled final row"
    );

    harness.keystrokes("home");
    assert!(
        harness
            .node("models.picker.model-00")
            .expect("first row")
            .hovered
    );
    assert_inside(
        harness
            .bounds("models.picker.model-00")
            .expect("home bounds"),
        harness.bounds("models.picker.menu").expect("menu bounds"),
    );

    harness.update(move |_, cx| {
        entity.update(cx, |select, cx| {
            select.set_selected(Some("unknown".into()), cx)
        })
    });
    assert!(
        harness
            .node("models.picker.unknown")
            .expect("new selected row")
            .hovered
    );
    assert_inside(
        harness
            .bounds("models.picker.unknown")
            .expect("selected change bounds"),
        harness.bounds("models.picker.menu").expect("menu bounds"),
    );
}
