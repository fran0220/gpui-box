//! Assigned root size and configured FLIP, with real displayed hit targets.
use super::support::*;
use crate::motion::{CubicBezier, Flipping, MotionSpec, Spring, flip};

#[derive(Default)]
struct Demo {
    grown: bool,
    direct: bool,
    spring: bool,
    hits: usize,
}

pub(super) fn flip_configuration(window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let state = crate::motion::keyed::slot::<Demo>(
        &"scene.flip.config-state".into(),
        window.window_handle().window_id(),
        cx,
    );
    let grown = state.borrow().grown;
    let change = state.clone();
    let direct = state.clone();
    let timing = state.clone();
    let hits = state.clone();
    let spec = if state.borrow().spring {
        MotionSpec::sprung(Spring::new(400.0, 28.0, 1.0))
    } else {
        MotionSpec::new(600, CubicBezier::new(0.0, 0.0, 1.0, 1.0)).with_delay(100)
    };
    let handle = flip("scene.flip.config-box", window, cx);
    let box_element = div()
        .id("scene.flip.config-hit")
        .w(px(if grown { 270.0 } else { 130.0 }))
        .h(px(if grown { 100.0 } else { 40.0 }))
        .bg(theme.colors.accent)
        .border_1()
        .border_color(theme.colors.text)
        .overflow_hidden()
        .p_token(&theme, Space::Xs)
        .child("Border box")
        .on_mouse_down(gpui::MouseButton::Left, move |_, window, _| {
            hits.borrow_mut().hits += 1;
            window.refresh();
        })
        .semantic_in(
            cx,
            NodeSpec::new("scene.flip.config-hit", Role::Button).text("Border box"),
        )
        .flip_size(&handle, window, cx)
        .animate(!state.borrow().direct)
        .animation(spec);
    stack(&theme).w(px(600.0))
        .child(caption(&theme,"Synthetic FLIP · delayed tween / spring · displayed size, hit target and semantic bounds agree"))
        .child(div().row().gap_token(&theme,Space::Sm)
            .child(Button::new("scene.flip.config.change").label("Move and resize").on_click(move |window,_|{let mut s=change.borrow_mut();s.grown = !s.grown;window.refresh();}))
            .child(Button::new("scene.flip.config.direct").label(if state.borrow().direct {"Enable animation"} else {"Snap directly"}).on_click(move |window,_|{let mut s=direct.borrow_mut();s.direct = !s.direct;window.refresh();}))
            .child(Button::new("scene.flip.config.timing").label(if state.borrow().spring {"Use tween"} else {"Use spring"}).on_click(move |window,_|{let mut s=timing.borrow_mut();s.spring = !s.spring;window.refresh();})))
        .child(div().w_full().h(px(150.0)).flex().flex_col().items_start().pl(px(if grown {180.0} else {0.0})).child(box_element))
        .child(div().child(format!("Displayed hits: {}",state.borrow().hits)).semantic_in(cx,NodeSpec::new("scene.flip.config.hits",Role::Status).value(state.borrow().hits.to_string())))
        .into_any_element()
}
