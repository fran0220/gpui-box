use super::*;
use crate::foundation::{FocusRing, Ident, StyledExt};
use crate::layout::measure;
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};
use gpui::{
    App, Bounds, FillOptions, FillRule, Hsla, MouseButton, PathBuilder, PathStyle, Pixels,
    RenderOnce, Window, canvas, div, point, prelude::*, px,
};
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
/// and point overlays. Click selects projected geometry. Wheel pans; Ctrl-wheel
/// zooms about the pointer. Arrow keys pan, +/- zoom, Home resets, [/] cycle
/// selection and Escape clears it. No handler means a read-only map.
#[derive(IntoElement)]
pub struct GeoMap {
    ident: Ident,
    label: SharedString,
    state: GeoState,
    viewport: GeoViewport,
    selected: Option<SharedString>,
    status_text: Option<SharedString>,
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
        if let Some(data) = data {
            let viewport = self.viewport;
            let measured = measure::cell(&self.ident.child("bounds").semantic_id(), window, cx);
            let paint_data = data.clone();
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
                            let at = |p| {
                                let p = viewport.screen(p, size);
                                point(
                                    bounds.left() + px(p[0] as f32),
                                    bounds.top() + px(p[1] as f32),
                                )
                            };
                            for (polygons, feature) in
                                paint_data.polygons.iter().zip(&paint_data.features)
                            {
                                let color = color(&paint_data, feature.value, &paint_theme);
                                for polygon in polygons {
                                    let mut fill = PathBuilder::fill().with_style(PathStyle::Fill(
                                        FillOptions::default().with_fill_rule(FillRule::EvenOdd),
                                    ));
                                    let mut stroke = PathBuilder::stroke(px(
                                        if paint_selected.as_ref() == Some(&feature.id) {
                                            paint_theme.borders.thick
                                        } else {
                                            paint_theme.borders.hairline
                                        },
                                    ));
                                    for ring in
                                        std::iter::once(&polygon.exterior).chain(&polygon.holes)
                                    {
                                        fill.move_to(at(ring[0]));
                                        stroke.move_to(at(ring[0]));
                                        for p in &ring[1..] {
                                            fill.line_to(at(*p));
                                            stroke.line_to(at(*p));
                                        }
                                        fill.close();
                                        stroke.close();
                                    }
                                    if let Ok(path) = fill.build() {
                                        window.paint_path(path, color);
                                    }
                                    if let Ok(path) = stroke.build() {
                                        window.paint_path(
                                            path,
                                            if paint_selected.as_ref() == Some(&feature.id) {
                                                paint_theme.colors.text
                                            } else {
                                                paint_theme.colors.hairline_strong
                                            },
                                        );
                                    }
                                }
                            }
                            for (p, source) in
                                paint_data.projected_points.iter().zip(&paint_data.points)
                            {
                                let center = at(*p);
                                let mut circle = PathBuilder::fill();
                                for i in 0..24 {
                                    let angle = i as f32 * std::f32::consts::TAU / 24.0;
                                    let p = point(
                                        center.x + px(5.0 * angle.cos()),
                                        center.y + px(5.0 * angle.sin()),
                                    );
                                    if i == 0 {
                                        circle.move_to(p);
                                    } else {
                                        circle.line_to(p);
                                    }
                                }
                                circle.close();
                                if let Ok(path) = circle.build() {
                                    window.paint_path(
                                        path,
                                        if paint_selected.as_ref() == Some(&source.id) {
                                            paint_theme.colors.text
                                        } else {
                                            paint_theme.colors.warning
                                        },
                                    );
                                }
                            }
                        },
                    )
                    .size_full(),
                );
            for (index, bounds) in data.visual_bounds(viewport, extent(measured.get())) {
                let (id, label, value) = if let Some(f) = data.features.get(index) {
                    (
                        &f.id,
                        f.label.clone(),
                        if f.value.is_some() {
                            f.formatted_value.clone()
                        } else {
                            data.domain.missing_label.clone()
                        },
                    )
                } else {
                    let p = &data.points[index - data.features.len()];
                    (&p.id, p.label.clone(), SharedString::default())
                };
                frame = frame.child(
                    div()
                        .absolute()
                        .left(px(bounds[0] as f32))
                        .top(px(bounds[1] as f32))
                        .w(px(bounds[2] as f32))
                        .h(px(bounds[3] as f32))
                        .semantic_in(
                            cx,
                            NodeSpec::new(
                                self.ident
                                    .child("geometry")
                                    .child(id.as_ref())
                                    .semantic_id(),
                                Role::Image,
                            )
                            .parent(self.ident.child("map").semantic_id())
                            .text(label)
                            .value(value)
                            .selected(self.selected.as_ref() == Some(id))
                            .read_only(true),
                        ),
                );
            }
            if let Some(handler) = self.on_event.clone() {
                let pointer_data = data.clone();
                let pointer_bounds = measured.clone();
                let pointer_handler = handler.clone();
                frame = frame.tab_index(0).focus_ring(&theme).on_mouse_down(
                    MouseButton::Left,
                    move |event, window, cx| {
                        let bounds = pointer_bounds.get();
                        let local = [
                            f64::from(f32::from(event.position.x - bounds.left())),
                            f64::from(f32::from(event.position.y - bounds.top())),
                        ];
                        pointer_handler(
                            GeoEvent::Select(pointer_data.hit_test(
                                viewport,
                                extent(bounds),
                                local,
                            )),
                            window,
                            cx,
                        );
                    },
                );
                let wheel_handler = handler.clone();
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
                    if let Some(action) =
                        key_event(&event.keystroke.key, viewport, &ids, selected.as_ref())
                    {
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
            let mut readout = div().row().flex_wrap().gap_token(&theme, Space::Sm);
            for (id, label, value) in data
                .features
                .iter()
                .map(|f| {
                    (
                        &f.id,
                        &f.label,
                        if f.value.is_some() {
                            f.formatted_value.clone()
                        } else {
                            data.domain.missing_label.clone()
                        },
                    )
                })
                .chain(
                    data.points
                        .iter()
                        .map(|p| (&p.id, &p.label, SharedString::default())),
                )
            {
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

fn color(data: &GeoData, value: Option<f64>, theme: &gpui_kit_theme::Theme) -> Hsla {
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
