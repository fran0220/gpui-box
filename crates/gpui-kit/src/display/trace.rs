//! A caller-owned execution trace as a waterfall of spans.
//!
//! Times are already normalized by the host. This component does not invent a
//! clock, a duration, or a locale: `start` and `end` are positions on a unit
//! interval, and every label is a finished string. Clicking a span reports
//! its identity; the host decides whether a payload is shown.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, TypeScale};

use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::status::StatusDot;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Ident, StyledExt};
use crate::motion;
use crate::strings::{ActiveStrings, StringKey};

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// What a span is doing, as the host already knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpanState {
    #[default]
    Pending,
    Running,
    Succeeded,
    Failed,
}

impl SpanState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    fn tone(self) -> Tone {
        match self {
            Self::Pending => Tone::Neutral,
            Self::Running => Tone::Info,
            Self::Succeeded => Tone::Success,
            Self::Failed => Tone::Danger,
        }
    }

    fn label(self, cx: &App) -> SharedString {
        cx.strings().text(match self {
            Self::Pending => StringKey::TracePending,
            Self::Running => StringKey::TraceRunning,
            Self::Succeeded => StringKey::TraceSucceeded,
            Self::Failed => StringKey::TraceFailed,
        })
    }
}

/// One already-timed interval on a unit axis.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceSpan {
    pub id: SharedString,
    pub label: SharedString,
    /// Inclusive start on the host's normalized axis, in `0..=1`.
    pub start: f32,
    /// Exclusive end on the same axis. Values outside the unit interval are
    /// clamped when drawn, never rewritten.
    pub end: f32,
    pub depth: u32,
    pub state: SpanState,
    pub detail: Option<SharedString>,
}

impl TraceSpan {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        start: f32,
        end: f32,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            start,
            end,
            depth: 0,
            state: SpanState::Pending,
            detail: None,
        }
    }

    pub fn depth(mut self, depth: u32) -> Self {
        self.depth = depth;
        self
    }

    pub fn state(mut self, state: SpanState) -> Self {
        self.state = state;
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Hierarchical labels beside a waterfall of caller-owned spans.
#[derive(IntoElement)]
pub struct TraceView {
    ident: Ident,
    label: SharedString,
    spans: Vec<TraceSpan>,
    axis_start: Option<SharedString>,
    axis_end: Option<SharedString>,
    current: Option<SharedString>,
    on_select: Option<SelectHandler>,
    slots: Slots,
}

impl TraceView {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            spans: Vec::new(),
            axis_start: None,
            axis_end: None,
            current: None,
            on_select: None,
            slots: Slots::default(),
        }
    }

    pub fn spans(mut self, spans: impl IntoIterator<Item = TraceSpan>) -> Self {
        self.spans = spans.into_iter().collect();
        self
    }

    /// Exact wording for the two ends of the host's time axis.
    pub fn axis(mut self, start: impl Into<SharedString>, end: impl Into<SharedString>) -> Self {
        self.axis_start = Some(start.into());
        self.axis_end = Some(end.into());
        self
    }

    pub fn current(mut self, id: impl Into<SharedString>) -> Self {
        self.current = Some(id.into());
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Slotted for TraceView {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for TraceView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let empty = self.spans.is_empty();
        let body = if empty {
            self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(
                    self.ident.child("empty"),
                    cx.strings().text(StringKey::TraceEmpty),
                )
                .kind(EmptyKind::Empty)
                .into_any_element()
            })
        } else {
            let rows: Vec<_> = self
                .spans
                .iter()
                .map(|span| {
                    span_row(
                        &self.ident,
                        span,
                        true,
                        self.current.as_ref() == Some(&span.id),
                        self.on_select.clone(),
                        &theme,
                        cx,
                    )
                })
                .collect();
            div()
                .column()
                .w_full()
                .gap_token(&theme, Space::Xs)
                .children(rows)
                .children(axis_row(
                    self.axis_start.clone(),
                    self.axis_end.clone(),
                    true,
                    &theme,
                ))
                .into_any_element()
        };

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .child(
                div()
                    .type_scale(&theme, TypeScale::Label)
                    .text_color(theme.colors.text)
                    .child(self.label.clone()),
            )
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::List)
                    .text(self.label)
                    .value(if empty { "empty" } else { "ready" }),
            )
    }
}

/// The waterfall alone: bars and a time axis, without the label column.
#[derive(IntoElement)]
pub struct SpanTimeline {
    ident: Ident,
    label: SharedString,
    spans: Vec<TraceSpan>,
    axis_start: Option<SharedString>,
    axis_end: Option<SharedString>,
    current: Option<SharedString>,
    on_select: Option<SelectHandler>,
    slots: Slots,
}

impl SpanTimeline {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            spans: Vec::new(),
            axis_start: None,
            axis_end: None,
            current: None,
            on_select: None,
            slots: Slots::default(),
        }
    }

    pub fn spans(mut self, spans: impl IntoIterator<Item = TraceSpan>) -> Self {
        self.spans = spans.into_iter().collect();
        self
    }

    pub fn axis(mut self, start: impl Into<SharedString>, end: impl Into<SharedString>) -> Self {
        self.axis_start = Some(start.into());
        self.axis_end = Some(end.into());
        self
    }

    pub fn current(mut self, id: impl Into<SharedString>) -> Self {
        self.current = Some(id.into());
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Slotted for SpanTimeline {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for SpanTimeline {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let empty = self.spans.is_empty();
        let body = if empty {
            self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(
                    self.ident.child("empty"),
                    cx.strings().text(StringKey::TraceEmpty),
                )
                .kind(EmptyKind::Empty)
                .into_any_element()
            })
        } else {
            let rows: Vec<_> = self
                .spans
                .iter()
                .map(|span| {
                    span_row(
                        &self.ident,
                        span,
                        false,
                        self.current.as_ref() == Some(&span.id),
                        self.on_select.clone(),
                        &theme,
                        cx,
                    )
                })
                .collect();
            div()
                .column()
                .w_full()
                .gap_token(&theme, Space::Xs)
                .children(rows)
                .children(axis_row(
                    self.axis_start.clone(),
                    self.axis_end.clone(),
                    false,
                    &theme,
                ))
                .into_any_element()
        };

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .child(
                div()
                    .type_scale(&theme, TypeScale::Label)
                    .text_color(theme.colors.text)
                    .child(self.label.clone()),
            )
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::List)
                    .text(self.label)
                    .value(if empty { "empty" } else { "ready" }),
            )
    }
}

fn span_row(
    parent: &Ident,
    span: &TraceSpan,
    with_label: bool,
    current: bool,
    on_select: Option<SelectHandler>,
    theme: &gpui_kit_theme::Theme,
    cx: &App,
) -> gpui::AnyElement {
    let ident = parent.child(span.id.as_ref());
    let start = span.start.clamp(0.0, 1.0);
    let end = span.end.clamp(start, 1.0);
    let width = (end - start).max(0.02);
    let tone = span.state.tone();
    let color = tone.mark_color(None, theme);
    let status = span.state.label(cx);
    let mut mark = StatusDot::new(tone);
    if span.state == SpanState::Running {
        mark = mark
            .busy(ident.child("running"))
            .activity(motion::Activity::Advancing);
    }

    let bar = div()
        .relative()
        .flex_1()
        .min_w_0()
        .h(px(16.0))
        .radius(theme, Radius::Small)
        .surface(theme, Surface::Canvas)
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(start))
                .w(relative(width))
                .radius(theme, Radius::Small)
                .bg(color.opacity(if current { 0.92 } else { 0.72 })),
        );

    let mut row = div()
        .id(ident.element_id())
        .row()
        .w_full()
        .items_center()
        .gap_token(theme, Space::Sm)
        .h(px(28.0));
    if with_label {
        row = row.child(
            div()
                .row()
                .items_center()
                .w(px(168.0))
                .flex_none()
                .gap_token(theme, Space::Xs)
                .pl(px(span.depth as f32 * theme.space(Space::Md)))
                .child(mark)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .type_scale(theme, TypeScale::Caption)
                        .text_color(theme.colors.text)
                        .child(span.label.clone()),
                ),
        );
    }
    row = row.child(bar);
    if let Some(handler) = on_select {
        let id = span.id.clone();
        row = row
            .cursor_pointer()
            .on_click(move |_, window, cx| handler(id.clone(), window, cx));
    }

    let mut spec = NodeSpec::new(ident.semantic_id(), Role::TreeItem)
        .parent(parent.semantic_id())
        .text(span.label.clone())
        .value(span.state.name())
        .selected(current)
        .level(span.depth.saturating_add(1));
    if let Some(detail) = &span.detail {
        spec = spec.description(detail.clone());
    } else {
        spec = spec.description(status);
    }
    row.semantic_in(cx, spec).into_any_element()
}

fn axis_row(
    start: Option<SharedString>,
    end: Option<SharedString>,
    with_label: bool,
    theme: &gpui_kit_theme::Theme,
) -> Option<gpui::AnyElement> {
    (start.is_some() || end.is_some()).then(|| {
        let mut row = div()
            .row()
            .w_full()
            .items_center()
            .gap_token(theme, Space::Sm)
            .type_scale(theme, TypeScale::Caption)
            .text_color(theme.colors.text_faint);
        if with_label {
            row = row.child(div().w(px(168.0)).flex_none());
        }
        row.child(
            div()
                .row()
                .flex_1()
                .min_w_0()
                .justify_between()
                .children(start)
                .children(end),
        )
        .into_any_element()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_keeps_the_host_interval_and_names_its_state() {
        let span = TraceSpan::new("gen", "Generate", 0.1, 0.4)
            .depth(1)
            .state(SpanState::Running)
            .detail("model.reply");
        assert_eq!(span.id.as_ref(), "gen");
        assert_eq!(span.start, 0.1);
        assert_eq!(span.end, 0.4);
        assert_eq!(span.state.name(), "running");
        assert_ne!(SpanState::Succeeded, SpanState::Failed);
    }
}
