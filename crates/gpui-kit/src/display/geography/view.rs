use super::*;
use crate::foundation::{FocusRing, Ident, StyledExt};
use crate::layout::measure;
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};
use gpui::{App, Bounds, Hsla, Pixels, RenderOnce, Window, canvas, div, point, prelude::*, px};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, Surface, TypeScale};
use std::rc::Rc;

/// Caller-owned loading and verified-data states. Stale retains the last data.
#[derive(Clone, Debug)]
pub enum GeoState {
    Loading,
    Empty,
    Ready(Rc<GeoData>),
    Stale {
        data: Rc<GeoData>,
        reason: SharedString,
    },
    Unavailable(SharedString),
    Error(SharedString),
    Refused(GeoRefusal),
}

impl HasPhase for GeoState {
    fn phase(&self) -> Phase {
        match self {
            Self::Loading => Phase::Loading,
            Self::Empty => Phase::Empty,
            Self::Ready(data) if data.features.is_empty() && data.points.is_empty() => Phase::Empty,
            Self::Ready(_) => Phase::Ready,
            Self::Stale { .. } | Self::Error(_) => Phase::Error,
            Self::Unavailable(_) | Self::Refused(_) => Phase::Unavailable,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Stale { reason, .. } | Self::Unavailable(reason) | Self::Error(reason) => {
                Some(reason)
            }
            Self::Refused(reason) => Some(reason.message()),
            _ => None,
        }
    }

    fn is_stale(&self) -> bool {
        matches!(self, Self::Stale { .. })
    }
}

/// Proposals only. The caller must apply an event and rerender to accept it.
#[derive(Clone, Debug, PartialEq)]
pub enum GeoEvent {
    Select(Option<SharedString>),
    Viewport(GeoViewport),
}

type Handler = Rc<dyn Fn(GeoEvent, &mut Window, &mut App)>;

/// A controlled local map with choropleth legend, selectable feature readout,
/// and point overlays. Click selects projected geometry; captured drag/touch
/// pans and pinch/Ctrl-wheel zooms about the pointer. Arrow keys pan, +/- zoom,
/// Home resets, F fits, [/] cycle selection. Escape cancels a drag or clears
/// selection. No handler means a read-only map.
#[derive(IntoElement)]
pub struct GeoMap {
    ident: Ident,
    label: SharedString,
    state: GeoState,
    viewport: GeoViewport,
    selected: Option<SharedString>,
    status_text: Option<SharedString>,
    animate: bool,
    on_event: Option<Handler>,
}

impl GeoMap {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: Ident::new(id),
            label: label.into(),
            state: GeoState::Loading,
            viewport: GeoViewport::default(),
            selected: None,
            status_text: None,
            animate: true,
            on_event: None,
        }
    }
    pub fn state(mut self, state: GeoState) -> Self {
        self.state = state;
        self
    }
    pub fn viewport(mut self, viewport: GeoViewport) -> Self {
        self.viewport = viewport;
        self
    }
    pub fn selected(mut self, selected: Option<SharedString>) -> Self {
        self.selected = selected;
        self
    }
    /// Animate accepted camera, style, and keyed geometry opacity targets.
    /// Direct input and reduced motion snap. Retired shapes are decorative;
    /// vertices never interpolate through invalid topology. Readouts use target values.
    pub fn animate(mut self, animate: bool) -> Self {
        self.animate = animate;
        self
    }
    /// Caller-owned complete status/reason wording, including localized input
    /// refusal messages. Does not change machine-readable status or phase.
    pub fn status_text(mut self, text: impl Into<SharedString>) -> Self {
        self.status_text = Some(text.into());
        self
    }
    pub fn on_event(mut self, handler: impl Fn(GeoEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for GeoMap {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state = self
            .viewport
            .validate()
            .err()
            .map_or(self.state, GeoState::Refused);
        let phase = state.phase();
        let stale = state.is_stale();
        let (status, data, reason) = match state {
            GeoState::Loading => ("loading", None, None),
            GeoState::Empty => ("empty", None, None),
            GeoState::Ready(data) if data.features.is_empty() && data.points.is_empty() => {
                ("empty", None, None)
            }
            GeoState::Ready(data) => ("ready", Some(data), None),
            GeoState::Stale { data, reason } => ("stale", Some(data), Some(reason)),
            GeoState::Unavailable(reason) => ("unavailable", None, Some(reason)),
            GeoState::Error(reason) => ("error", None, Some(reason)),
            GeoState::Refused(reason) => (
                "refused",
                None,
                Some(
                    reason
                        .string_key()
                        .map_or_else(|| reason.to_string().into(), |key| cx.strings().text(key)),
                ),
            ),
        };
        let status_text = self.status_text.unwrap_or_else(|| {
            let label = if stale {
                cx.strings().text(StringKey::StatusStale)
            } else {
                match phase {
                    Phase::Loading => cx.strings().text(StringKey::Loading),
                    Phase::Empty => cx.strings().text(StringKey::StateViewEmpty),
                    Phase::Unavailable => cx.strings().text(StringKey::StateViewUnavailable),
                    _ => SharedString::default(),
                }
            };
            match &reason {
                Some(reason) if !label.is_empty() => format!("{label}: {reason}").into(),
                Some(reason) => reason.clone(),
                None => label,
            }
        });
        let mut body = div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .type_scale(&theme, TypeScale::Caption)
            .child(self.label.clone())
            .child(
                div().child(status_text.clone()).semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("status").semantic_id(), Role::Status)
                        .value(status)
                        .text(status_text)
                        .description(phase.name()),
                ),
            );
        let measured = measure::cell(&self.ident.child("bounds").semantic_id(), window, cx);
        let exploration = crate::foundation::window_state::with_key(
            &self.ident.child("exploration").semantic_id(),
            window.window_handle().window_id(),
            cx,
            |state: &mut Rc<std::cell::RefCell<exploration::Exploration>>| state.clone(),
        );
        let manipulated = exploration.borrow().direct || exploration.borrow().active();
        let presentation = crate::motion::keyed::slot::<presentation::Presentation>(
            &self.ident.child("geometry-motion").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        if data.is_none() {
            presentation.borrow_mut().clear();
        }
        exploration.borrow_mut().sync(
            data.as_ref(),
            extent(measured.get()),
            self.on_event.is_some(),
            window,
        );
        if let Some(data) = data {
            let direct = {
                let mut interaction = exploration.borrow_mut();
                let direct = interaction.direct || interaction.active();
                interaction.direct = false;
                direct
            };
            let viewport = crate::motion::tracked_or_snap(
                &self.ident.child("camera-motion").semantic_id(),
                self.viewport,
                crate::motion::resize(&theme),
                direct || !self.animate,
                window,
                cx,
            );
            let presented = presentation.borrow_mut().sample(
                data.clone(),
                crate::motion::state_change(&theme),
                manipulated || !self.animate,
                window,
                cx,
            );
            let mut colors = std::collections::HashMap::new();
            for index in data.visible_indices(viewport, extent(measured.get())) {
                if let Some(feature) = data.features.get(index) {
                    let fill = crate::motion::tracked_or_snap(
                        &self
                            .ident
                            .child("fill-motion")
                            .child(feature.id.as_ref())
                            .semantic_id(),
                        color(&data, feature.value, &theme),
                        crate::motion::state_change(&theme),
                        !self.animate,
                        window,
                        cx,
                    );
                    colors.insert(index, fill);
                }
            }
            let paint_data = data.clone();
            let paint_frame = presented.clone();
            let paint_selected = self.selected.clone();
            let paint_theme = theme.clone();
            let paint_bounds = measured.clone();
            let mut frame = div()
                .id(self.ident.child("map").element_id())
                .relative()
                .w_full()
                .h(px(280.0))
                .overflow_hidden()
                .surface(&theme, Surface::Canvas)
                .child(
                    canvas(
                        move |bounds, window, _| measure::record(&paint_bounds, bounds, window),
                        move |bounds, _, window, _| {
                            let size = extent(bounds);
                            let mut paint = painting::Paint {
                                viewport,
                                bounds,
                                theme: &paint_theme,
                                selected: None,
                            };
                            for (shape, alpha) in &paint_frame.retired {
                                paint.draw(&shape.data, shape.index, None, *alpha, window);
                            }
                            paint.selected = paint_selected.as_ref();
                            let candidates = paint_data.visible_indices(viewport, size);
                            for index in candidates {
                                paint.draw(
                                    &paint_data,
                                    index,
                                    colors.get(&index).copied(),
                                    paint_frame.opacity(index),
                                    window,
                                );
                            }
                        },
                    )
                    .size_full(),
                );
            let geometry_data = data.clone();
            let geometry_frame = presented.clone();
            let geometry_ident = self.ident.child("geometry");
            let geometry_selected = self.selected.clone();
            frame = frame.child(
                gpui_kit_semantics::MeasuredLeafBatch::new(
                    self.ident.child("geometry-targets").semantic_id(),
                    move |current, _, _| {
                        geometry_data
                            .visual_bounds(viewport, extent(current))
                            .into_iter()
                            .filter(|(index, _)| geometry_frame.opacity(*index) > 0.0)
                            .map(|(index, bounds)| {
                                let (id, label, value) = geometry_data.readout(index);
                                gpui_kit_semantics::MeasuredLeaf::new(
                                    geometry_ident.child(id.as_ref()).semantic_id(),
                                    gpui_kit_semantics::MeasuredLeafRole::Image,
                                    Bounds::new(
                                        current.origin
                                            + point(px(bounds[0] as f32), px(bounds[1] as f32)),
                                        gpui::size(px(bounds[2] as f32), px(bounds[3] as f32)),
                                    ),
                                )
                                .text(label.clone())
                                .value(value)
                                .selected(geometry_selected.as_ref() == Some(id))
                                .read_only(true)
                            })
                            .collect()
                    },
                )
                .diagnostic_parent(self.ident.child("map").semantic_id())
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
            if let Some(at) = exploration
                .borrow()
                .hover
                .filter(|at| measured.get().contains(at))
                && let Some(id) = presented.hit_test(
                    viewport,
                    extent(measured.get()),
                    exploration::local(at, measured.get()),
                )
            {
                let index = data
                    .features
                    .iter()
                    .position(|f| f.id == id)
                    .or_else(|| {
                        data.points
                            .iter()
                            .position(|p| p.id == id)
                            .map(|i| i + data.features.len())
                    })
                    .expect("hit names source geometry");
                let (_, label, value) = data.readout(index);
                let text = if value.is_empty() {
                    label.to_string()
                } else {
                    format!("{label}: {value}")
                };
                body = body.child(
                    crate::overlay::Overlay::new(self.ident.child("hover"))
                        .layer(gpui_kit_theme::Layer::Tooltip)
                        .placement(crate::overlay::Placement::At(
                            at + point(px(12.0), px(12.0)),
                        ))
                        .child(
                            div()
                                .surface(&theme, Surface::Raised)
                                .p_token(&theme, Space::Sm)
                                .child(text.clone())
                                .semantic_in(
                                    cx,
                                    NodeSpec::new(
                                        self.ident.child("hover-readout").semantic_id(),
                                        Role::Status,
                                    )
                                    .text(text)
                                    .value(id),
                                ),
                        ),
                );
            }
            if let Some(handler) = self.on_event.clone() {
                frame = exploration::install(
                    frame.tab_index(0).focus_ring(&theme),
                    exploration.clone(),
                    measured.clone(),
                    viewport,
                    presented.clone(),
                    handler.clone(),
                    self.ident.child("gesture").element_id(),
                );
                let wheel_handler = handler.clone();
                let fit_bounds = measured.clone();
                let fit_data = data.clone();
                let wheel_state = exploration.clone();
                frame = frame.on_scroll_wheel(move |event, window, cx| {
                    let bounds = measured.get();
                    let size = extent(bounds);
                    if size[0].min(size[1]) <= 0.0 {
                        return;
                    }
                    let delta = event.delta.pixel_delta(px(20.0));
                    let next = if event.modifiers.control {
                        let anchor = viewport.world(
                            [
                                f64::from(f32::from(event.position.x - bounds.left())),
                                f64::from(f32::from(event.position.y - bounds.top())),
                            ],
                            size,
                        );
                        viewport.zoom_at((-f64::from(f32::from(delta.y)) / 200.0).exp(), anchor)
                    } else {
                        let scale = size[0].min(size[1]) * viewport.zoom;
                        viewport.pan(
                            -f64::from(f32::from(delta.x)) / scale,
                            -f64::from(f32::from(delta.y)) / scale,
                        )
                    };
                    wheel_state.borrow_mut().direct = true;
                    wheel_handler(GeoEvent::Viewport(next), window, cx);
                    cx.stop_propagation();
                });
                let ids: Vec<_> = data
                    .features
                    .iter()
                    .map(|f| f.id.clone())
                    .chain(data.points.iter().map(|p| p.id.clone()))
                    .collect();
                let selected = self.selected.clone();
                frame = frame.on_key_down(move |event, window, cx| {
                    if event.keystroke.key == "escape" && exploration.borrow().active() {
                        let restore = exploration.borrow_mut().cancel();
                        window.release_pointer();
                        if let Some(camera) = restore {
                            handler(GeoEvent::Viewport(camera), window, cx);
                        }
                        cx.stop_propagation();
                        window.refresh();
                        return;
                    }
                    if event.keystroke.key == "f" {
                        if let Some(fit) = fit_data.fit_viewport(extent(fit_bounds.get()), 16.0) {
                            handler(GeoEvent::Viewport(fit), window, cx);
                        }
                        cx.stop_propagation();
                        return;
                    }
                    if let Some(action) =
                        key_event(&event.keystroke.key, viewport, &ids, selected.as_ref())
                    {
                        exploration.borrow_mut().direct = true;
                        handler(action, window, cx);
                        cx.stop_propagation();
                    }
                });
            }
            body = body.child(
                frame.semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("map").semantic_id(), Role::Image)
                        .text(self.label.clone())
                        .value(format!(
                            "zoom {}; center {}, {}",
                            viewport.zoom, viewport.center.x, viewport.center.y
                        ))
                        .read_only(self.on_event.is_none()),
                ),
            );
            let mut ramp = div().row();
            for i in 0..=10 {
                let value = data.domain.minimum
                    + (data.domain.maximum - data.domain.minimum) * f64::from(i) / 10.0;
                ramp = ramp.child(div().w(px(8.0)).h(px(12.0)).bg(color(
                    &data,
                    Some(value),
                    &theme,
                )));
            }
            let legend = div()
                .row()
                .flex_wrap()
                .items_center()
                .gap_token(&theme, Space::Xs)
                .child(data.domain.minimum_label.clone())
                .child(ramp);
            body = body.child(
                legend
                    .child(data.domain.maximum_label.clone())
                    .child(div().w(px(14.0)).h(px(12.0)).bg(theme.colors.control_hover))
                    .child(data.domain.missing_label.clone())
                    .semantic_in(
                        cx,
                        NodeSpec::new(self.ident.child("legend").semantic_id(), Role::Group).text(
                            format!(
                                "{} — {}; {}",
                                data.domain.minimum_label,
                                data.domain.maximum_label,
                                data.domain.missing_label
                            ),
                        ),
                    ),
            );
            let count = data.features.len() + data.points.len();
            if count > 32 {
                let source = data.clone();
                let ident = self.ident.clone();
                let read_only = self.on_event.is_none();
                let mut readout = crate::data::List::new(
                    self.ident.child("feature"),
                    count,
                    move |index, _, cx| {
                        let (id, label, value) = source.readout(index);
                        let text = if value.is_empty() {
                            label.to_string()
                        } else {
                            format!("{label}: {value}")
                        };
                        crate::data::ListItem::new(
                            id.clone(),
                            div().child(text.clone()).semantic_in(
                                cx,
                                NodeSpec::new(
                                    ident.child("reading").child(id.as_ref()).semantic_id(),
                                    Role::Text,
                                )
                                .text(label.clone())
                                .value(value)
                                .read_only(read_only),
                            ),
                        )
                        .text(text)
                    },
                )
                .visible_rows(6)
                .keys(
                    data.features
                        .iter()
                        .map(|f| f.id.clone())
                        .chain(data.points.iter().map(|p| p.id.clone())),
                );
                if let Some(selected) = &self.selected {
                    readout = readout.selected(selected.clone());
                }
                if let Some(handler) = self.on_event.clone() {
                    readout = readout.on_select(move |id, window, cx| {
                        handler(GeoEvent::Select(Some(id)), window, cx)
                    });
                }
                body = body.child(readout);
            } else {
                let mut readout = div().row().flex_wrap().gap_token(&theme, Space::Sm);
                for index in 0..count {
                    let (id, label, value) = data.readout(index);
                    let selected = self.selected.as_ref() == Some(id);
                    let mut target = div()
                        .id(self.ident.child("feature").child(id.as_ref()).element_id())
                        .px(px(theme.spacing.xs))
                        .py(px(theme.spacing.xs))
                        .bg(theme
                            .colors
                            .control_hover
                            .opacity(if selected { 1.0 } else { 0.0 }))
                        .child(if value.is_empty() {
                            label.to_string()
                        } else {
                            format!("{label}: {value}")
                        });
                    if let Some(handler) = self.on_event.clone() {
                        let click_id = id.clone();
                        let key_id = id.clone();
                        let key_handler = handler.clone();
                        target = target
                            .tab_index(0)
                            .focus_ring(&theme)
                            .on_click(move |_, window, cx| {
                                handler(GeoEvent::Select(Some(click_id.clone())), window, cx)
                            })
                            .on_key_down(move |event, window, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    key_handler(GeoEvent::Select(Some(key_id.clone())), window, cx);
                                    cx.stop_propagation();
                                }
                            });
                    }
                    readout = readout.child(
                        target.semantic_in(
                            cx,
                            NodeSpec::new(
                                self.ident.child("feature").child(id.as_ref()).semantic_id(),
                                Role::Button,
                            )
                            .text(label.clone())
                            .value(value)
                            .selected(selected)
                            .read_only(self.on_event.is_none()),
                        ),
                    );
                }
                body = body.child(readout);
            }
        }
        body.semantic_in(
            cx,
            NodeSpec::new(self.ident.semantic_id(), Role::Group)
                .text(self.label)
                .value(status)
                .description(phase.name())
                .busy(phase.is_busy()),
        )
    }
}

fn extent(bounds: Bounds<Pixels>) -> [f64; 2] {
    [
        f64::from(f32::from(bounds.size.width)),
        f64::from(f32::from(bounds.size.height)),
    ]
}

pub(super) fn color(data: &GeoData, value: Option<f64>, theme: &gpui_kit_theme::Theme) -> Hsla {
    value.map_or(theme.colors.control_hover, |value| {
        let fraction =
            ((value - data.domain.minimum) / (data.domain.maximum - data.domain.minimum)) as f32;
        theme
            .colors
            .info
            .blend(theme.colors.accent.opacity(fraction))
    })
}

fn key_event(
    key: &str,
    viewport: GeoViewport,
    ids: &[SharedString],
    selected: Option<&SharedString>,
) -> Option<GeoEvent> {
    let step = 0.1 / viewport.zoom;
    let next = match key {
        "left" => viewport.pan(-step, 0.0),
        "right" => viewport.pan(step, 0.0),
        "up" => viewport.pan(0.0, -step),
        "down" => viewport.pan(0.0, step),
        "+" | "=" => viewport.zoom_at(1.25, viewport.center),
        "-" => viewport.zoom_at(0.8, viewport.center),
        "home" => GeoViewport::default(),
        "escape" => return Some(GeoEvent::Select(None)),
        "[" | "]" if !ids.is_empty() => {
            let index = selected.and_then(|id| ids.iter().position(|candidate| candidate == id));
            let next = match (key, index) {
                ("[", Some(i)) => (i + ids.len() - 1) % ids.len(),
                ("]", Some(i)) => (i + 1) % ids.len(),
                ("[", None) => ids.len() - 1,
                _ => 0,
            };
            return Some(GeoEvent::Select(Some(ids[next].clone())));
        }
        _ => return None,
    };
    Some(GeoEvent::Viewport(next))
}
