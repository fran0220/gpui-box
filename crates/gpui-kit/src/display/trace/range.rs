//! Temporal adapter: shared controlled range arithmetic, measured family UI.

use super::{NumericScale, TimeFormatter, TimeSelectionHandler, TraceOptions, format_endpoint};
use crate::{
    foundation::{FocusRing, Ident, StyledExt},
    interaction::range::{
        RangeError, RangeIntent, RangeInteraction, RangeKey, RangeMapping, RangeTarget,
    },
    layout::measure,
    motion::keyed,
    strings::{ActiveStrings, StringKey, Strings},
};
use gpui::{App, Bounds, FocusHandle, MouseButton, Pixels, Window, div, prelude::*, px, relative};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TypeScale};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, PartialEq)]
struct Mapping(NumericScale);
impl RangeMapping for Mapping {
    fn project(&self, value: f64) -> Option<f64> {
        self.0.map(value)
    }
    fn unproject(&self, fraction: f64) -> Option<f64> {
        self.0.invert(fraction)
    }
}

#[derive(Default)]
struct State {
    edit: RangeInteraction<Mapping>,
    handler: Option<TimeSelectionHandler>,
    captured_bounds: Option<Bounds<Pixels>>,
    captured_scale: Option<NumericScale>,
    requested_scale: Option<NumericScale>,
}

/// Keep the time picture under a captured hand still. An unrelated caller
/// viewport replacement cancels; releasing resumes navigation from this picture.
pub(super) fn prepare(
    ident: &Ident,
    requested: NumericScale,
    value: Option<[f64; 2]>,
    handler: Option<TimeSelectionHandler>,
    window: &mut Window,
    cx: &mut App,
) -> Option<NumericScale> {
    let cell = keyed::slot::<State>(&ident.semantic_id(), window.window_handle().window_id(), cx);
    let (cancel, previous, held) = {
        let mut state = cell.borrow_mut();
        let mapping = Mapping(state.captured_scale.unwrap_or(requested));
        let cancel = if handler.is_none()
            || window.captured_hitbox().is_none()
            || state.requested_scale != Some(requested)
        {
            state.edit.cancel()
        } else {
            state.edit.sync(&mapping, value)
        };
        let previous = state.handler.clone();
        state.handler = handler;
        state.requested_scale = Some(requested);
        if state.edit.preview().is_none() {
            state.captured_bounds = None;
            state.captured_scale = None;
        }
        (cancel, previous, state.captured_scale)
    };
    if let Some(cancel) = cancel {
        window.release_pointer();
        if let Some(report) = previous {
            window.defer(cx, move |window, cx| report(cancel, window, cx));
        }
        window.refresh();
    }
    held
}

pub(super) fn validate(value: Option<[f64; 2]>) -> Result<(), RangeError> {
    if let Some([start, end]) = value {
        if !start.is_finite() || !end.is_finite() {
            return Err(RangeError::NonFinite);
        }
        if start > end {
            return Err(RangeError::InvalidRange);
        }
    }
    Ok(())
}

fn fraction(bounds: Bounds<Pixels>, x: Pixels) -> Option<f64> {
    let width = f32::from(bounds.size.width);
    (width > 0.).then(|| f64::from(f32::from(x - bounds.origin.x)) / f64::from(width))
}

fn text(
    value: Option<[f64; 2]>,
    preview: bool,
    formatter: Option<&TimeFormatter>,
    strings: &Strings,
) -> gpui::SharedString {
    value.map_or_else(
        || strings.text(StringKey::TimeSelectionEmpty),
        |[a, b]| {
            strings.format(
                if preview {
                    StringKey::TimeSelectionPreview
                } else {
                    StringKey::TimeSelectionRange
                },
                &[
                    &format_endpoint(a, formatter, strings),
                    &format_endpoint(b, formatter, strings),
                ],
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn control<E: StatefulInteractiveElement + Styled>(
    element: E,
    intent: RangeIntent,
    target: RangeTarget,
    focus: FocusHandle,
    state: Rc<RefCell<State>>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    mapping: Mapping,
    value: Option<[f64; 2]>,
    handler: TimeSelectionHandler,
) -> E {
    let down = state.clone();
    let down_bounds = bounds.clone();
    let down_handler = handler.clone();
    let down_mapping = mapping.clone();
    let down_focus = focus.clone();
    let moving = state.clone();
    let move_handler = handler.clone();
    let released = state.clone();
    let release_handler = handler.clone();
    element
        .tab_index(0)
        .track_focus(&focus)
        .on_mouse_down_with_pointer_capture(MouseButton::Left, move |event, window, cx| {
            let Some(at) = fraction(down_bounds.get(), event.position.x) else {
                window.release_pointer();
                return;
            };
            let intent = if intent == RangeIntent::Create
                && !event.modifiers.shift
                && value.is_some_and(|[a, b]| {
                    down_mapping
                        .project(a)
                        .zip(down_mapping.project(b))
                        .is_some_and(|(a, b)| at >= a.min(b) && at <= a.max(b))
                }) {
                RangeIntent::Move
            } else {
                intent
            };
            let proposal = down
                .borrow_mut()
                .edit
                .begin(down_mapping.clone(), value, intent, at);
            match proposal {
                Ok(proposal) => {
                    down.borrow_mut().captured_bounds = Some(down_bounds.get());
                    down.borrow_mut().captured_scale = Some(down_mapping.0);
                    down_focus.focus(window, cx);
                    down_handler(proposal, window, cx);
                    window.refresh();
                }
                Err(_) => window.release_pointer(),
            }
            cx.stop_propagation();
        })
        .on_mouse_move(move |event, window, cx| {
            if event.pressed_button != Some(MouseButton::Left) {
                return;
            }
            let captured = moving.borrow().captured_bounds;
            if let Some(at) = captured.and_then(|bounds| fraction(bounds, event.position.x)) {
                let proposal = moving.borrow_mut().edit.update(at);
                if let Ok(Some(proposal)) = proposal {
                    move_handler(proposal, window, cx);
                    window.refresh();
                    cx.stop_propagation();
                }
            }
        })
        .on_mouse_up(MouseButton::Left, move |event, window, cx| {
            let captured = released.borrow_mut().captured_bounds.take();
            if let Some(at) = captured.and_then(|bounds| fraction(bounds, event.position.x)) {
                let proposal = released.borrow_mut().edit.release(at);
                if let Ok(Some(proposal)) = proposal {
                    release_handler(proposal, window, cx);
                    window.refresh();
                    cx.stop_propagation();
                }
            }
        })
        .on_key_down(move |event, window, cx| {
            if event.keystroke.key == "escape" {
                let proposal = state.borrow_mut().edit.cancel();
                if let Some(proposal) = proposal {
                    window.release_pointer();
                    handler(proposal, window, cx);
                    window.refresh();
                    cx.stop_propagation();
                }
                return;
            }
            let key = match event.keystroke.key.as_str() {
                "left" => RangeKey::Step(-0.01),
                "right" => RangeKey::Step(0.01),
                "home" => RangeKey::Home,
                "end" => RangeKey::End,
                _ => return,
            };
            if let Some(value) = value {
                let proposal = state
                    .borrow_mut()
                    .edit
                    .keyboard(&mapping, value, target, key);
                if let Ok(proposal) = proposal {
                    handler(proposal, window, cx);
                    window.refresh();
                    cx.stop_propagation();
                }
            }
        })
}

pub(super) fn render(
    ident: &Ident,
    options: &TraceOptions,
    gutters: (f32, f32),
    window: &mut Window,
    cx: &mut App,
) -> Option<gpui::AnyElement> {
    let scale = options.viewport;
    let value = options.selected_time;
    let mapping = Mapping(scale);
    // Off-domain caller values are retained visibly, never clamped into data.
    // The shared state machine rejects them; do not advertise working handles.
    let handler = options.on_time_selection.clone().filter(|_| {
        value.is_none_or(|[a, b]| {
            scale
                .map(a)
                .zip(scale.map(b))
                .is_some_and(|(a, b)| (0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b))
        })
    });
    let state = keyed::slot::<State>(&ident.semantic_id(), window.window_handle().window_id(), cx);
    if value.is_none() && handler.is_none() {
        return None;
    }
    let theme = cx.theme().clone();
    let bounds = measure::cell(&ident.child("measure").semantic_id(), window, cx);
    let focus = keyed::slot::<Option<[FocusHandle; 3]>>(
        &ident.child("focus").semantic_id(),
        window.window_handle().window_id(),
        cx,
    );
    let focus = focus
        .borrow_mut()
        .get_or_insert_with(|| std::array::from_fn(|_| cx.focus_handle()))
        .clone();
    let preview = state.borrow().edit.preview();
    let shown = preview.or(value);
    let projected = shown.and_then(|[a, b]| scale.map(a).zip(scale.map(b)));
    let reach = theme.space(Space::Md);
    let mut track = div()
        .id(ident.child("track").element_id())
        .relative()
        .w_full()
        .h(px(reach * 2.))
        .bg(theme.colors.divider.opacity(theme.opacity.muted));
    if let Some((a, b)) = projected {
        let a = a.clamp(0., 1.);
        let b = b.clamp(0., 1.);
        track = track.child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(a.min(b) as f32))
                .w(relative((b - a).abs() as f32))
                .bg(theme.colors.accent.opacity(theme.opacity.muted)),
        );
        for (index, at, target, name) in [
            (1, a, RangeTarget::Start, "start"),
            (2, b, RangeTarget::End, "end"),
        ] {
            let id = ident.child(name);
            let mut thumb = div()
                .id(id.element_id())
                .absolute()
                .top_0()
                .h_full()
                .left(relative(at as f32))
                .ml(px(-reach / 2.))
                .w(px(reach))
                .border_1()
                .border_color(theme.colors.accent)
                .bg(theme.colors.control)
                .focus_ring(&theme);
            if let Some(handler) = &handler {
                thumb = control(
                    thumb,
                    RangeIntent::Resize(target),
                    target,
                    focus[index].clone(),
                    state.clone(),
                    bounds.clone(),
                    mapping.clone(),
                    value,
                    handler.clone(),
                );
            }
            track = track.child(
                thumb.semantic_in(
                    cx,
                    NodeSpec::new(id.semantic_id(), Role::Slider)
                        .text(cx.strings().text(if target == RangeTarget::Start {
                            StringKey::RangeSelectionStart
                        } else {
                            StringKey::RangeSelectionEnd
                        }))
                        .description(shown.map_or_else(String::new, |v| {
                            format_endpoint(v[index - 1], options.formatter.as_ref(), cx.strings())
                                .to_string()
                        }))
                        .range(0., 1., at as f32)
                        .orientation(gpui::accesskit::Orientation::Horizontal)
                        .value(shown.map_or_else(String::new, |v| v[index - 1].to_string()))
                        .disabled(handler.is_none()),
                ),
            );
        }
    }
    if let Some(handler) = &handler {
        track = control(
            track,
            RangeIntent::Create,
            RangeTarget::Window,
            focus[0].clone(),
            state.clone(),
            bounds.clone(),
            mapping,
            value,
            handler.clone(),
        );
        let cancel_state = state.clone();
        let report = handler.clone();
        track = track.child(crate::interaction::on_pointer_cancel(move |window, cx| {
            // The framework already released capture; no synthetic commit.
            let proposal = cancel_state.borrow_mut().edit.cancel();
            if let Some(proposal) = proposal {
                report(proposal, window, cx);
            }
        }));
    }
    let measured = bounds.clone();
    let track = track.semantic_in(
        cx,
        NodeSpec::new(ident.child("track").semantic_id(), Role::Group)
            .text(cx.strings().text(StringKey::TimeSelection))
            .description(cx.strings().text(StringKey::RangeSelectionInstructions))
            .disabled(handler.is_none())
            .value(value.map_or_else(String::new, |[a, b]| format!("{a},{b}"))),
    );
    let readout = text(
        shown,
        preview.is_some() && preview != value,
        options.formatter.as_ref(),
        cx.strings(),
    );
    Some(
        div()
            .column()
            .w_full()
            .pl(px((gutters.0 - reach / 2.).max(0.)))
            .pr(px((gutters.1 - reach / 2.).max(0.)))
            .gap_token(&theme, Space::Xs)
            .child(
                div()
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.text)
                    .child(readout.clone())
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.child("readout").semantic_id(), Role::Status)
                            .text(readout),
                    ),
            )
            .child(
                div()
                    .px(px(reach / 2.))
                    .on_children_prepainted(move |children, window, _| {
                        if let Some(first) = children.first() {
                            measure::record(&measured, *first, window);
                        }
                    })
                    .child(track),
            )
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Group)
                    .text(cx.strings().text(StringKey::TimeSelection))
                    .value(if preview.is_some() {
                        "proposal"
                    } else {
                        "controlled"
                    }),
            )
            .into_any_element(),
    )
}
