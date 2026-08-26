//! Checkbox, radio, switch, and slider report intent; they never decide.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    IntoElement, Modifiers, MouseButton, ParentElement, Styled, TestAppContext, div, point, px,
};
use gpui_kit::foundation::{LayoutDirection, set_layout_direction};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

type Calls<T> = Rc<RefCell<Vec<T>>>;

/// One list, held by the test and by the control's handler.
fn recorder<T: 'static>() -> (Calls<T>, Calls<T>) {
    let calls: Calls<T> = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

#[gpui::test]
fn a_checkbox_reports_the_opposite_of_what_it_shows(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<bool>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Checkbox::new("form.telemetry")
            .label("Send usage data")
            .checked(true)
            .on_change(move |next, _, _| sink.borrow_mut().push(next))
            .into_any_element()
    });

    harness.click("form.telemetry");
    assert_eq!(*calls.borrow(), vec![false]);

    let node = harness.node("form.telemetry").expect("published");
    assert_eq!(node.checked, Some(true));
    assert_eq!(node.text.as_deref(), Some("Send usage data"));
}

#[gpui::test]
fn a_mixed_checkbox_says_mixed_rather_than_guessing(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        Checkbox::new("form.providers")
            .label("Providers")
            .mixed()
            .on_change(|_, _, _| {})
            .into_any_element()
    });

    assert_eq!(
        harness.node("form.providers").expect("published").checked,
        None
    );
}

#[gpui::test]
fn a_disabled_checkbox_installs_no_handler(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<bool>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Checkbox::new("form.locked")
            .label("Managed by policy")
            .checked(true)
            .disabled(true)
            .on_change(move |next, _, _| sink.borrow_mut().push(next))
            .into_any_element()
    });

    harness.click("form.locked");
    assert!(calls.borrow().is_empty());
    assert!(harness.node("form.locked").expect("published").disabled);
}

#[gpui::test]
fn a_radio_reports_selection_without_selecting_itself(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<()>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        div()
            .flex()
            .flex_col()
            .child(
                Radio::new("mode.ask")
                    .label("Ask first")
                    .selected(true)
                    .on_select(|_, _| {}),
            )
            .child(
                Radio::new("mode.auto")
                    .label("Run without asking")
                    .on_select(move |_, _| sink.borrow_mut().push(())),
            )
            .into_any_element()
    });

    harness.click("mode.auto");
    assert_eq!(calls.borrow().len(), 1);
    // Nothing moved: the group is owned by the caller.
    assert_eq!(
        harness.node("mode.ask").expect("published").checked,
        Some(true)
    );
    assert_eq!(
        harness.node("mode.auto").expect("published").checked,
        Some(false)
    );
}

#[gpui::test]
fn a_switch_reports_the_flip(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<bool>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Switch::new("settings.preview")
            .label("Preview releases")
            .on(false)
            .on_change(move |next, _, _| sink.borrow_mut().push(next))
            .into_any_element()
    });

    harness.click("settings.preview");
    assert_eq!(*calls.borrow(), vec![true]);
    assert_eq!(
        harness.node("settings.preview").expect("published").checked,
        Some(false)
    );
}

#[gpui::test]
fn a_slider_publishes_its_range_and_position(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w(px(200.0))
            .child(
                Slider::new("settings.temperature")
                    .label("Temperature")
                    .range(0.0, 2.0)
                    .step(0.1)
                    .value(0.7)
                    .display("0.7")
                    .on_change(|_, _, _| {}),
            )
            .into_any_element()
    });

    let node = harness.node("settings.temperature").expect("published");
    assert_eq!(node.value_min, Some(0.0));
    assert_eq!(node.value_max, Some(2.0));
    assert_eq!(node.value_now, Some(0.7));
    assert_eq!(node.value.as_deref(), Some("0.7"));
}

#[gpui::test]
fn a_slider_reports_a_value_on_the_step_grid(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<f32>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        div()
            .w(px(200.0))
            .child(
                Slider::new("settings.volume")
                    .label("Volume")
                    .range(0.0, 10.0)
                    .step(1.0)
                    .value(0.0)
                    .on_change(move |value, _, _| sink.borrow_mut().push(value)),
            )
            .into_any_element()
    });

    harness.click("settings.volume");
    let reported = *calls.borrow().first().expect("the click reported a value");
    assert_eq!(reported, reported.round(), "value left the step grid");
    assert!((0.0..=10.0).contains(&reported));
}

#[gpui::test]
fn slider_endpoints_stay_inside_the_control_and_follow_reading_direction(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<f32>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        div()
            .w(px(200.0))
            .child(
                Slider::new("settings.endpoint")
                    .range(0.0, 10.0)
                    .value(5.0)
                    .on_change(move |value, _, _| sink.borrow_mut().push(value)),
            )
            .into_any_element()
    });

    let bounds = harness.bounds("settings.endpoint").expect("laid out");
    let left = point(bounds.left() + px(0.5), bounds.center().y);
    let right = point(bounds.right() - px(0.5), bounds.center().y);
    for at in [left, right] {
        harness
            .context()
            .simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        harness
            .context()
            .simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    }
    assert_eq!(*calls.borrow(), vec![0.0, 10.0]);

    calls.borrow_mut().clear();
    harness.update(|_, cx| set_layout_direction(LayoutDirection::RightToLeft, cx));
    let bounds = harness
        .bounds("settings.endpoint")
        .expect("laid out in RTL");
    let left = point(bounds.left() + px(0.5), bounds.center().y);
    let right = point(bounds.right() - px(0.5), bounds.center().y);
    for at in [left, right] {
        harness
            .context()
            .simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        harness
            .context()
            .simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    }
    assert_eq!(*calls.borrow(), vec![10.0, 0.0]);
}

#[gpui::test]
fn a_reversed_range_is_corrected_rather_than_rendered_backwards(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        div()
            .w(px(200.0))
            .child(
                Slider::new("settings.reversed")
                    .label("Reversed")
                    .range(10.0, 0.0)
                    .value(5.0)
                    .on_change(|_, _, _| {}),
            )
            .into_any_element()
    });

    let node = harness.node("settings.reversed").expect("published");
    assert_eq!(node.value_min, Some(0.0));
    assert_eq!(node.value_max, Some(10.0));
}
