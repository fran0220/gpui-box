//! Progress that reports what is actually known.

use gpui::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space};

use crate::foundation::Ident;
use crate::motion::{self, AnimationExt as _, MotionSpec};
use crate::strings::{ActiveStrings, StringKey};

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

    /// The node both surfaces publish: busy always, a position only when the
    /// extent is known.
    /// What a reader sees beside the label: the caller's own wording first,
    /// then a counted position worded by the catalogue.
    pub(crate) fn shown(&self, cx: &App) -> Option<SharedString> {
        self.display.clone().or_else(|| {
            self.count.map(|(done, total)| {
                cx.strings().format(
                    StringKey::CountOfTotal,
                    &[&done.to_string(), &total.to_string()],
                )
            })
        })
    }

    pub(crate) fn spec(&self, id: SharedString, label: Option<SharedString>, cx: &App) -> NodeSpec {
        let mut spec = NodeSpec::new(id, Role::Progress).busy(true);
        if let Some(fraction) = self.fraction {
            spec = spec.range(0.0, 1.0, fraction);
        }
        if let Some(label) = label {
            spec = spec.text(label);
        }
        if let Some(display) = self.shown(cx) {
            spec = spec.value(display);
        }
        spec
    }
}

/// A horizontal bar for work whose extent is known.
///
/// A bar without a value is indeterminate and says so, rather than crawling
/// to ninety percent and waiting there.
#[derive(Debug, IntoElement)]
pub struct ProgressBar {
    ident: Ident,
    label: Option<SharedString>,
    value: ProgressValue,
}

impl ProgressBar {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            value: ProgressValue::default(),
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
                motion::resize(&theme),
                window,
                cx,
            )
        });

        let spec = self
            .value
            .spec(self.ident.semantic_id(), self.label.clone(), cx);
        let display = self.value.shown(cx);

        div()
            .flex()
            .flex_col()
            .gap(px(theme.space(Space::Xs)))
            .w_full()
            .when_some(self.label.clone(), |element, label| {
                element.child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .text_size(px(theme.typography.body.size))
                        .text_color(theme.colors.text_muted)
                        .child(label)
                        .when_some(display, |element, display| {
                            element.child(div().text_color(theme.colors.text).child(display))
                        }),
                )
            })
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(4.0))
                    .rounded_full()
                    .overflow_hidden()
                    .bg(theme.colors.hairline_strong)
                    .when_some(drawn, |element, fraction| {
                        element.child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .bottom_0()
                                .rounded_full()
                                .bg(theme.colors.accent)
                                .w(relative(fraction)),
                        )
                    })
                    // An unknown extent fills the whole track faintly and
                    // sweeps a brighter segment across it. A partly filled bar
                    // would be read as a position, and there is none.
                    .when(indeterminate, |element| {
                        let period = MotionSpec::new(
                            theme.motion.pulse_ms * 2,
                            motion::CubicBezier::new(0.4, 0.0, 0.6, 1.0),
                        );
                        element
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .bg(theme.colors.accent.opacity(theme.opacity.muted)),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .w(relative(0.3))
                                    .rounded_full()
                                    .bg(theme.colors.accent)
                                    .with_animation(
                                        self.ident.child("sweep").element_id(),
                                        period.repeating(),
                                        |element, delta| element.left(relative(delta * 1.3 - 0.3)),
                                    ),
                            )
                    }),
            )
            .semantic_in(cx, spec)
    }
}
