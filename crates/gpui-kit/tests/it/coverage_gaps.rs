//! Focused behavior coverage for the MUI/shadcn-derived component families.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext as _, Entity, Modifiers, MouseButton, TestAppContext, div, point, prelude::*, px,
};
use gpui_kit::prelude::*;
use gpui_kit::semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Calls<T> = Rc<RefCell<Vec<T>>>;

fn recorder<T: 'static>() -> (Calls<T>, Calls<T>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

#[gpui::test]
fn disabled_multi_select_has_no_remove_action_or_tab_stop(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<MultiSelectEvent>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let control = cx.new(|cx| {
            MultiSelect::new("disabled-select", window, cx)
                .name("Disabled selection")
                .selected(["native"])
                .options([SelectOption::new("native", "Native")])
                .disabled(true)
        });
        let sink = sink.clone();
        cx.subscribe(&control, move |_, event: &MultiSelectEvent, _| {
            sink.borrow_mut().push(event.clone())
        })
        .detach();
        control.into_any_element()
    });
    assert!(harness.node("disabled-select").unwrap().disabled);
    assert!(harness.node("disabled-select.tag.native").unwrap().disabled);
    assert!(harness.node("disabled-select.tag.native.remove").is_none());
    harness.click("disabled-select.tag.native");
    harness.keystrokes("tab space enter backspace");
    assert!(calls.borrow().is_empty());
    assert!(!harness.node("disabled-select").unwrap().focused);
    assert!(!harness.node("disabled-select.tag.native").unwrap().focused);
}

#[gpui::test]
fn multi_select_keeps_selection_controlled_and_ignores_disabled_options(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<MultiSelectEvent>();
    let slot: Rc<RefCell<Option<Entity<MultiSelect>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let control = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    MultiSelect::new("filters.providers", window, cx)
                        .name("Providers")
                        .selected(["native"])
                        .options([
                            SelectOption::new("native", "Native"),
                            SelectOption::new("remote", "Remote"),
                            SelectOption::new("managed", "Managed").disabled(true),
                        ])
                })
            })
            .clone();
        control.into_any_element()
    });
    let control = slot.borrow().as_ref().expect("entity").clone();
    harness.update(move |window, cx| {
        cx.subscribe(&control, move |_, event: &MultiSelectEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
        control.update(cx, |control, cx| control.open(window, cx));
    });

    harness.click("filters.providers.option.remote");
    harness.click("filters.providers.option.managed");

    assert_eq!(
        *calls.borrow(),
        vec![
            MultiSelectEvent::Opened,
            MultiSelectEvent::Toggled("remote".into()),
        ]
    );
    assert_eq!(
        harness
            .node("filters.providers")
            .expect("published")
            .value
            .as_deref(),
        Some("1"),
        "the control still renders the caller-owned selection"
    );
}

#[gpui::test]
fn rating_reports_half_steps_and_unrated_is_not_zero(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<Option<f32>>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Rating::new("review.score")
            .value(Some(2.5))
            .precision(RatingPrecision::Half)
            .label("Score")
            .on_change(move |value, _, _| sink.borrow_mut().push(value))
            .into_any_element()
    });

    let star = harness.bounds("review.score.value-3").expect("third star");
    let left = point(star.left() + px(1.0), star.center().y);
    let right = point(star.right() - px(1.0), star.center().y);
    harness
        .context()
        .simulate_mouse_down(left, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(left, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_down(right, MouseButton::Left, Modifiers::none());
    harness
        .context()
        .simulate_mouse_up(right, MouseButton::Left, Modifiers::none());

    assert_eq!(*calls.borrow(), vec![Some(2.5), Some(3.0)]);

    let mut unrated = Harness::new(cx, gpui_kit::install, |_, _| {
        Rating::new("review.unrated")
            .label("Score")
            .value(None)
            .on_change(|_, _, _| {})
            .into_any_element()
    });
    let node = unrated.node("review.unrated").expect("published");
    assert_eq!(node.value_now, Some(0.0));
    assert_eq!(node.value.as_deref(), Some("Not rated"));
}

#[gpui::test]
fn transfer_list_reports_item_and_move_intents_without_mutating_panes(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<TransferListEvent>();
    let slot: Rc<RefCell<Option<Entity<TransferList>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let control = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    TransferList::new("assignment", window, cx)
                        .source([
                            TransferItem::new("runtime", "Runtime"),
                            TransferItem::new("managed", "Managed").disabled(true),
                        ])
                        .target([TransferItem::new("logs", "Logs")])
                        .source_selected(["runtime"])
                })
            })
            .clone();
        control.into_any_element()
    });
    let control = slot.borrow().as_ref().expect("entity").clone();
    harness.update(move |_, cx| {
        cx.subscribe(&control, move |_, event: &TransferListEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });

    harness.click("assignment.source.list.runtime");
    harness.click("assignment.source.list.managed");
    harness.click("assignment.move-to-target");

    assert_eq!(
        *calls.borrow(),
        vec![
            TransferListEvent::ToggleSource("runtime".into()),
            TransferListEvent::MoveToTarget,
        ]
    );
    assert!(harness.node("assignment.source.list.runtime").is_some());
}

#[gpui::test]
fn vertical_slider_uses_vertical_bounds_and_value_mapping(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<f32>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        div()
            .w(px(80.0))
            .child(
                Slider::new("settings.vertical")
                    .orientation(SliderOrientation::Vertical)
                    .range(0.0, 100.0)
                    .value(50.0)
                    .on_change(move |value, _, _| sink.borrow_mut().push(value)),
            )
            .into_any_element()
    });

    let node = harness.node("settings.vertical").expect("published");
    assert_eq!(node.role, Role::Slider);
    let bounds = harness.bounds("settings.vertical").expect("laid out");
    assert!(bounds.size.height > bounds.size.width);
    let top = point(bounds.center().x, bounds.top() + px(1.0));
    let bottom = point(bounds.center().x, bounds.bottom() - px(1.0));
    for at in [top, bottom] {
        harness
            .context()
            .simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        harness
            .context()
            .simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    }
    assert_eq!(*calls.borrow(), vec![100.0, 0.0]);
}

#[gpui::test]
fn grid_container_image_list_and_masonry_publish_stable_items(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _cx| {
        div()
            .w(px(820.0))
            .child(
                Grid::new("layout.grid")
                    .columns(1)
                    .columns_at(Breakpoint::Medium, 2)
                    .items([
                        GridItem::new("first", Card::new().id("first.card")),
                        GridItem::new("second", Card::new().id("second.card")),
                    ]),
            )
            .child(
                Container::new("layout.container")
                    .width(ContainerWidth::Dialog)
                    .child(div().child("content")),
            )
            .child(
                ImageList::new("layout.images")
                    .items([ImageListItem::new("cover", "Cover", div().size_full())])
                    .on_select(|_, _, _| {}),
            )
            .child(
                Masonry::new("layout.masonry")
                    .columns(1)
                    .items([MasonryItem::new("tile", div().size_full(), 40.0)]),
            )
            .into_any_element()
    });

    for id in [
        "layout.grid",
        "layout.container",
        "layout.images",
        "layout.masonry",
    ] {
        assert!(harness.node(id).is_some(), "missing semantic node {id}");
    }
    let first = harness
        .bounds("layout.grid.item.first")
        .expect("first grid item");
    let second = harness
        .bounds("layout.grid.item.second")
        .expect("second grid item");
    assert_eq!(
        first.origin.y, second.origin.y,
        "an 820px measured width crosses the 768px Medium breakpoint"
    );
    assert!(second.origin.x > first.origin.x);
    assert_eq!(
        harness
            .node("layout.images.items.cover")
            .expect("tile")
            .text,
        Some("Cover".into())
    );
    assert!(harness.node("layout.masonry.item.tile").is_some());
}

#[gpui::test]
fn autosize_bubble_and_carousel_have_explicit_semantic_contracts(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<CarouselEvent>();
    let area_slot: Rc<RefCell<Option<Entity<TextArea>>>> = Rc::new(RefCell::new(None));
    let build_area_slot = area_slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let sink = sink.clone();
        let area = build_area_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    TextArea::new("editor.notes", window, cx)
                        .autosize(2, 4)
                        .text("A note")
                })
            })
            .clone();
        div()
            .child(area)
            .child(Bubble::new("message.start", "Incoming").content("Hello"))
            .child(
                Bubble::new("message.end", "Outgoing")
                    .placement(BubblePlacement::End)
                    .content("Goodbye"),
            )
            .child(
                Carousel::new("gallery")
                    .items([
                        CarouselItem::new("one", "One", div().child("First")),
                        CarouselItem::new("two", "Two", div().child("Second")),
                    ])
                    .active("one")
                    .on_event(move |event, _, _| sink.borrow_mut().push(event)),
            )
            .into_any_element()
    });

    assert_eq!(
        harness.node("editor.notes").expect("textarea").role,
        Role::MultilineInput
    );
    assert_eq!(
        harness.node("message.start").expect("bubble").role,
        Role::Group
    );
    let start = harness
        .bounds("message.start.surface")
        .expect("start bubble");
    let end = harness.bounds("message.end.surface").expect("end bubble");
    assert!(end.origin.x > start.origin.x);
    harness.click("gallery.next");
    assert_eq!(*calls.borrow(), vec![CarouselEvent::Next]);

    let mut empty = Harness::new(cx, gpui_kit::install, |_, _| {
        Carousel::new("gallery.empty").into_any_element()
    });
    assert_eq!(
        empty
            .node("gallery.empty.state")
            .expect("empty state")
            .value
            .as_deref(),
        Some("empty")
    );
}
