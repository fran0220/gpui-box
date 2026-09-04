//! Progress that reports what is actually known.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space};

use crate::controls::button::Button;
use crate::display::signature;
use crate::foundation::{Ident, Sizable};
use crate::motion::{self, MotionPolicy, MotionRole};
use crate::overlay::tooltip::Tooltipped;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type CancelHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Whether the work is still moving.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ProgressPace {
    #[default]
    Running,
    Stalled,
    Paused,
}

impl ProgressPace {
    pub(crate) fn name(self) -> Option<&'static str> {
        match self {
            Self::Running => None,
            Self::Stalled => Some("stalled"),
            Self::Paused => Some("paused"),
        }
    }
}

/// What a progress surface knows about the work, and how it says so.
///
/// `ProgressBar` and [`crate::display::progress_circle::ProgressCircle`] draw
/// different shapes over exactly this state, so the two cannot drift into
/// telling different stories about the same work.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ProgressValue {
    /// `None` means the extent of the work is unknown.
    pub fraction: Option<f32>,
    /// What to show beside the label, such as `"3 of 12"`, when the caller
    /// worded it itself.
    pub display: Option<SharedString>,
    /// The work counted, when the caller gave a count rather than a wording.
    /// It stays a pair of numbers until render, because the sentence around
    /// them belongs to the installed catalogue and not to this struct.
    pub count: Option<(usize, usize)>,
    pub pace: ProgressPace,
}

impl ProgressValue {
    pub(crate) fn set_fraction(&mut self, fraction: f32) {
        self.fraction = Some(fraction.clamp(0.0, 1.0));
    }

    /// Reports `done` out of `total`, and stays indeterminate when the total
    /// is zero, because no fraction exists to report.
    pub(crate) fn set_count(&mut self, done: usize, total: usize) {
        self.fraction = (total > 0).then(|| (done as f32 / total as f32).clamp(0.0, 1.0));
        self.count = Some((done, total));
    }

    pub(crate) fn is_indeterminate(&self) -> bool {
        self.fraction.is_none()
    }

    pub(crate) fn is_moving(&self) -> bool {
        matches!(self.pace, ProgressPace::Running)
    }

    /// The node both surfaces publish: busy always, a position only when the
    /// extent is known.
    /// What a reader sees beside the label: the caller's own wording first,
    /// then a counted position worded by the catalogue.
    pub(crate) fn shown(&self, cx: &App) -> Option<SharedString> {
        self.display.clone().or_else(|| {
            self.count
                .map(|(done, total)| cx.numbers().count_of_total(done, total))
        })
    }

    pub(crate) fn spec(&self, id: SharedString, label: Option<SharedString>, cx: &App) -> NodeSpec {
        let mut spec = NodeSpec::new(id, Role::Progress).busy(self.is_moving());
        if let Some(fraction) = self.fraction {
            spec = spec.range(0.0, 1.0, fraction);
        }
        if let Some(label) = label {
            spec = spec.text(label);
        }
        if let Some(display) = self.shown(cx) {
            spec = spec.value(display);
        } else if let Some(pace) = self.pace.name() {
            spec = spec.value(pace);
        }
        spec
    }
}

/// A horizontal bar for work whose extent is known.
///
/// A bar without a value is indeterminate and says so, rather than crawling
/// to ninety percent and waiting there.
#[derive(IntoElement)]
pub struct ProgressBar {
    ident: Ident,
    label: Option<SharedString>,
    value: ProgressValue,
    on_cancel: Option<CancelHandler>,
}

impl std::fmt::Debug for ProgressBar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProgressBar")
            .field("ident", &self.ident)
            .field("value", &self.value)
            .finish()
    }
}

impl ProgressBar {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            value: ProgressValue::default(),
            on_cancel: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// How much of the work is done, between zero and one.
    pub fn fraction(mut self, fraction: f32) -> Self {
        self.value.set_fraction(fraction);
        self
    }

    /// Reports `done` out of `total`, and stays indeterminate when the total
    /// is zero, because no fraction exists to report.
    pub fn count(mut self, done: usize, total: usize) -> Self {
        self.value.set_count(done, total);
        self
    }

    pub fn display(mut self, display: impl Into<SharedString>) -> Self {
        self.value.display = Some(display.into());
        self
    }

    /// The work has a known position but is no longer advancing.
    pub fn stalled(mut self, stalled: bool) -> Self {
        if stalled {
            self.value.pace = ProgressPace::Stalled;
        }
        self
    }

    /// The work was paused. Distinct from stalled: somebody asked it to stop.
    pub fn paused(mut self, paused: bool) -> Self {
        if paused {
            self.value.pace = ProgressPace::Paused;
        }
        self
    }

    pub fn on_cancel(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ProgressBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let indeterminate = self.value.is_indeterminate();
        // The published range stays the caller's number from the frame it
        // changes; only the fill takes its time getting there.
        let drawn = self.value.fraction.map(|fraction| {
            motion::tracked(
                &self.ident.semantic_id(),
                fraction,
                MotionPolicy::spec(MotionRole::Resize, &theme),
                window,
                cx,
            )
        });

        let moving = self.value.is_moving();
        let mut spec = self
            .value
            .spec(self.ident.semantic_id(), self.label.clone(), cx);
        let display = self.value.shown(cx);
        let pace = match self.value.pace {
            ProgressPace::Running => None,
            ProgressPace::Stalled => Some((
                cx.strings().text(StringKey::ProgressStalled),
                Icon::Danger,
                theme.colors.warning,
            )),
            ProgressPace::Paused => Some((
                cx.strings().text(StringKey::ProgressPaused),
                Icon::Pause,
                theme.colors.text_muted,
            )),
        };
        if let Some((wording, _, _)) = &pace {
            spec = spec.description(wording.clone());
        }
        let cancel = self.on_cancel.map(|handler| {
            // Stopping a run is a control, so it wears a boundary. Ghost
            // chrome put it in the same tone as the count beside it, which
            // left the only thing on the row a reader can press looking like
            // the two things they cannot.
            Button::new(self.ident.child("cancel"))
                .label(cx.strings().text(StringKey::ProgressCancel))
                .secondary()
                .control_size(ControlSize::Sm)
                .semantic_parent(self.ident.semantic_id())
                .on_click(move |window, cx| handler(window, cx))
        });

        div()
            .flex()
            .flex_col()
            .gap(px(theme.space(Space::Xs)))
            .w_full()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .when_some(self.label.clone(), |element, label| {
                element.child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .text_size(px(theme.typography.body.size))
                        .gap(px(theme.space(Space::Sm)))
                        .text_color(theme.colors.text_muted)
                        .child(div().flex_1().min_w_0().child(label))
                        // Count, pace and control are one trailing group. Left
                        // to `justify_between` they were spread across
                        // whatever width was going spare, so two bars that
                        // report the same thing started their counts at two
                        // different places.
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_none()
                                .items_center()
                                .gap(px(theme.space(Space::Sm)))
                                .when_some(display, |element, display| {
                                    element
                                        .child(div().text_color(theme.colors.text).child(display))
                                })
                                .when_some(pace, |element, (wording, glyph, color)| {
                                    let mark_ident = self.ident.child("pace");
                                    element.child(
                                        div()
                                            .id(mark_ident.element_id())
                                            .flex()
                                            .items_center()
                                            .child(
                                                icon(glyph)
                                                    .size(px(theme.control.sm.icon_size))
                                                    .text_color(color),
                                            )
                                            .tip(mark_ident, wording),
                                    )
                                })
                                .children(cancel),
                        ),
                )
            })
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(theme.measures.progress_track_height))
                    .rounded_full()
                    .overflow_hidden()
                    .bg(signature::track(&theme))
                    .when_some(drawn, |element, fraction| {
                        element.child(signature::filled(signature::mark(&theme), fraction))
                    })
                    // An unknown extent sweeps only while the work is moving.
                    // A still sweep would be read as a position, and a stalled
                    // or paused bar is not claiming one.
                    .when(indeterminate && moving, |element| {
                        element.child(signature::unknown(
                            self.ident.child("sweep").element_id(),
                            &theme,
                            cx,
                        ))
                    }),
            )
            .semantic_in(cx, spec)
    }
}
