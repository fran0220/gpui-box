use std::rc::Rc;

use gpui::{
    AnyElement, App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::Icon as Glyph;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, TypeScale};

use crate::controls::button::IconButton;
use crate::display::badge::Tone;
use crate::foundation::{Ident, Sizable, StyledExt};
use crate::motion;
use crate::strings::{ActiveStrings, StringKey};

type DismissHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A tone-colored dot, the smallest state indicator in the system.
#[derive(Debug, IntoElement)]
pub struct StatusDot {
    tone: Tone,
    tint: Option<Hsla>,
    /// The identity a busy dot animates under, when it is reporting
    /// work that is still going.
    busy: Option<Ident>,
    /// Which claim the motion makes. A circle cannot turn, so working and
    /// deliberating both breathe; advancing sweeps a band across the mark.
    activity: motion::Activity,
}

impl StatusDot {
    pub fn new(tone: Tone) -> Self {
        Self {
            tone,
            tint: None,
            busy: None,
            activity: motion::Activity::Deliberating,
        }
    }

    /// Paints the dot in a caller-owned colour without changing the severity
    /// the surface around it reports.
    ///
    /// A dot is the smallest identity mark the library has, and an
    /// application that colours people or workspaces needs one that is not
    /// limited to the six severities. See [`Tone`] for what stays true.
    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    /// Breathes the dot, for a state that is still running.
    ///
    /// A dot breathes where a glyph would turn, because there is nothing in a
    /// circle for a rotation to be visible against. It is the same claim made
    /// with the only motion this shape can carry.
    pub fn busy(mut self, ident: impl Into<Ident>) -> Self {
        self.busy = Some(ident.into());
        self
    }

    /// Which of the three busy claims the motion makes.
    ///
    /// Without [`StatusDot::busy`] this is ignored: a settled dot does not
    /// move, whatever activity was named.
    pub fn activity(mut self, activity: motion::Activity) -> Self {
        self.activity = activity;
        self
    }
}

impl RenderOnce for StatusDot {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let color = self.tone.mark_color(self.tint, &theme);
        let dot = div().flex_none().size(px(7.0)).rounded_full().bg(color);
        match self.busy {
            Some(ident) if self.activity == motion::Activity::Advancing => {
                let mark = div()
                    .relative()
                    .flex_none()
                    .size(px(7.0))
                    .rounded_full()
                    .overflow_hidden()
                    .bg(color)
                    .children(motion::sweep(
                        ident.element_id(),
                        &theme,
                        theme.colors.text,
                        cx,
                    ));
                mark.into_any_element()
            }
            Some(ident) => motion::breathe_as(dot, ident.element_id(), self.activity, &theme, cx),
            None => dot.into_any_element(),
        }
    }
}

/// A dot plus a short label, for inline state.
#[derive(Debug, IntoElement)]
pub struct StatusLine {
    ident: Option<Ident>,
    label: SharedString,
    tone: Tone,
    tint: Option<Hsla>,
    busy: Option<Ident>,
    activity: motion::Activity,
}

impl StatusLine {
    pub fn new(label: impl Into<SharedString>, tone: Tone) -> Self {
        Self {
            ident: None,
            label: label.into(),
            tone,
            tint: None,
            busy: None,
            activity: motion::Activity::Deliberating,
        }
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    /// Paints the dot in a caller-owned colour, leaving the reported severity
    /// alone. See [`Tone`].
    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    /// Breathes the dot, for state that is still running.
    ///
    /// The same claim [`StatusDot::busy`] makes, reachable from the labelled
    /// form, so a running row does not have to be rebuilt out of parts to
    /// move.
    pub fn busy(mut self, ident: impl Into<Ident>) -> Self {
        self.busy = Some(ident.into());
        self
    }

    /// Which of the three busy claims the motion makes. See [`StatusDot::activity`].
    pub fn activity(mut self, activity: motion::Activity) -> Self {
        self.activity = activity;
        self
    }
}

impl RenderOnce for StatusLine {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut dot = StatusDot::new(self.tone);
        if let Some(tint) = self.tint {
            dot = dot.tint(tint);
        }
        if let Some(busy) = self.busy.clone() {
            dot = dot.busy(busy).activity(self.activity);
        }
        let element = div()
            .row()
            .gap_token(&theme, Space::Sm)
            .type_scale(&theme, TypeScale::Label)
            .text_color(theme.colors.text_muted)
            .child(dot)
            .child(self.label.clone());
        match self.ident {
            Some(ident) => element
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Status)
                        .text(self.label.clone())
                        // The severity by name: a tinted dot no longer says
                        // it in paint, and a running line is busy whatever
                        // colour it wears.
                        .value(self.tone.name())
                        .busy(self.busy.is_some()),
                )
                .into_any_element(),
            None => element.into_any_element(),
        }
    }
}

/// A tinted message block.
///
/// Callouts carry host refusals and stale-data warnings verbatim; they never
/// summarize an error into a friendlier but less true sentence. They take a
/// [`Tone`] and no tint: a refusal is a severity, not an identity, and
/// painting one in a person's colour would make the report look like it
/// belonged to them.
#[derive(Debug, IntoElement)]
pub struct Callout {
    ident: Option<Ident>,
    message: SharedString,
    tone: Tone,
}

impl Callout {
    pub fn new(message: impl Into<SharedString>, tone: Tone) -> Self {
        Self {
            ident: None,
            message: message.into(),
            tone,
        }
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }
}

impl RenderOnce for Callout {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let color = self.tone.color(&theme);
        let content = div()
            .w_full()
            .flex()
            .flex_row()
            .items_start()
            .gap_token(&theme, Space::Sm)
            .child(div().mt(px(5.0)).child(StatusDot::new(self.tone)))
            .child(div().min_w_0().child(self.message.clone()));

        let frame = div()
            .w_full()
            .px_token(&theme, Space::Lg)
            .py_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .bg(color.opacity(0.14))
            .type_scale(&theme, TypeScale::Label)
            .line_height(px(theme.typography.body.line_height))
            .text_color(color.opacity(0.92));

        // A callout is a report arriving, so it arrives rather than appearing.
        // The travel is inside the frame that publishes the node, so the
        // published box never moves. Without an identity there is nothing to
        // key an animation to, and a callout nothing can address gets none.
        match self.ident {
            Some(ident) => frame
                .child(motion::content_in(
                    ident.child("in").element_id(),
                    &theme,
                    content,
                ))
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Status).text(self.message.clone()),
                )
                .into_any_element(),
            None => frame.child(content).into_any_element(),
        }
    }
}

/// A page-level tinted report: title, action, and a way to close it.
///
/// [`Callout`] stays the inline form. This one is for a strip that sits
/// above a page and can be dismissed.
#[derive(IntoElement)]
pub struct Banner {
    ident: Ident,
    message: SharedString,
    title: Option<SharedString>,
    tone: Tone,
    action: Option<AnyElement>,
    on_dismiss: Option<DismissHandler>,
}

impl std::fmt::Debug for Banner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Banner")
            .field("ident", &self.ident)
            .field("tone", &self.tone)
            .finish()
    }
}

impl Banner {
    pub fn new(ident: impl Into<Ident>, message: impl Into<SharedString>, tone: Tone) -> Self {
        Self {
            ident: ident.into(),
            message: message.into(),
            title: None,
            tone,
            action: None,
            on_dismiss: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }

    pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Banner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let color = self.tone.color(&theme);
        let dismiss = self.on_dismiss.map(|handler| {
            IconButton::new(
                self.ident.child("dismiss"),
                Glyph::Close,
                cx.strings().text(StringKey::Dismiss),
            )
            .control_size(ControlSize::Sm)
            .semantic_parent(self.ident.semantic_id())
            .on_click(move |window, cx| handler(window, cx))
        });

        div()
            .w_full()
            .row()
            .items_start()
            .gap_token(&theme, Space::Sm)
            .px_token(&theme, Space::Lg)
            .py_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .bg(color.opacity(0.14))
            .child(div().mt(px(5.0)).child(StatusDot::new(self.tone)))
            .child(
                div()
                    .column()
                    .gap_token(&theme, Space::Xs)
                    .flex_1()
                    .min_w_0()
                    .when_some(self.title.clone(), |element, title| {
                        element.child(
                            div()
                                .type_scale(&theme, TypeScale::Label)
                                .text_color(color)
                                .child(title),
                        )
                    })
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Body)
                            .text_color(color.opacity(0.92))
                            .child(self.message.clone()),
                    )
                    .children(self.action),
            )
            .children(dismiss)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Status)
                    .text(self.message)
                    .value(self.tone.name()),
            )
    }
}

/// The mark that a verified value is still on screen after a failed refresh.
#[derive(Debug, IntoElement)]
pub struct StaleMark {
    ident: Ident,
    reason: SharedString,
    updated: Option<SharedString>,
}

impl StaleMark {
    pub fn new(ident: impl Into<Ident>, reason: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            reason: reason.into(),
            updated: None,
        }
    }

    /// The host's own wording for when the value was last verified.
    pub fn updated(mut self, updated: impl Into<SharedString>) -> Self {
        self.updated = Some(updated.into());
        self
    }
}

impl RenderOnce for StaleMark {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .column()
            .gap_token(&theme, Space::Xs)
            .child(StatusLine::new(self.reason.clone(), Tone::Warning).id(self.ident.child("line")))
            .when_some(self.updated.clone(), |element, updated| {
                element.child(
                    div()
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_faint)
                        .child(
                            cx.strings()
                                .format(StringKey::StaleUpdated, &[updated.as_ref()]),
                        ),
                )
            })
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Status)
                    .text(self.reason)
                    .value("stale"),
            )
    }
}
