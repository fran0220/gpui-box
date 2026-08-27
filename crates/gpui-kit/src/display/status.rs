use std::rc::Rc;

use gpui::{
    AnyElement, App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::Icon as Glyph;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, SemanticWash, Space, Surface, TypeScale};

use crate::controls::button::IconButton;
use crate::display::badge::Tone;
use crate::foundation::direction::ActiveDirection;
use crate::foundation::{Ident, Sizable, StyledExt};
use crate::motion;
use crate::strings::{ActiveStrings, StringKey};

type DismissHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The glyph a severity is drawn with.
///
/// A dot says *something has a state*; it cannot say which one without being
/// read for its colour, which is the one channel a reader may not have. The
/// four severities that name an outcome therefore lead with a picture of that
/// outcome, and the two that do not — neutral and accent — keep the dot,
/// because there is nothing for them to draw.
fn tone_glyph(tone: Tone) -> Option<Glyph> {
    match tone {
        Tone::Success => Some(Glyph::Check),
        Tone::Warning => Some(Glyph::Danger),
        Tone::Danger => Some(Glyph::CloseCircle),
        Tone::Info => Some(Glyph::Info),
        Tone::Neutral | Tone::Accent => None,
    }
}

/// The mark a report leads with, sized and aligned to the first line of text.
fn severity_mark(tone: Tone, theme: &gpui_kit_theme::Theme) -> AnyElement {
    let color = tone.color(theme);
    let edge = theme.control.sm.icon_size;
    match tone_glyph(tone) {
        Some(glyph) => div()
            .flex_none()
            .h(px(theme.typography.body.line_height))
            .flex()
            .items_center()
            .child(crate::display::icon::paint(glyph, edge, color, false))
            .into_any_element(),
        None => div()
            .flex_none()
            .h(px(theme.typography.body.line_height))
            .flex()
            .items_center()
            .child(StatusDot::new(tone))
            .into_any_element(),
    }
}

/// The tone a page-level report wears across its whole surface.
///
/// The faint semantic wash is sized for a report spanning a whole row rather
/// than for a compact chip, and every tone takes the same theme-owned weight.
fn banner_wash(theme: &gpui_kit_theme::Theme, color: gpui::Hsla) -> gpui::Hsla {
    theme.color_wash(color, SemanticWash::Faint)
}

/// The band at the reading edge that carries the severity.
///
/// A report used to be a wash of its own colour across the whole surface,
/// which made a yellow one heavier than the red one above it for no reason
/// anybody meant. The colour is spent on a rail and a glyph instead, so two
/// severities differ by hue and by picture and never by weight.
pub(crate) fn tone_rail(
    theme: &gpui_kit_theme::Theme,
    color: gpui::Hsla,
    direction: crate::foundation::LayoutDirection,
) -> gpui::Div {
    // Inset by the corner it sits inside. `overflow_hidden` masks to the
    // frame's box and not to its radius, so a rail run to the full height
    // kept its square corners against a rounded report.
    let bar = div()
        .absolute()
        .top(px(theme.radii.card))
        .bottom(px(theme.radii.card))
        .w(px(theme.effects.selection_rail_width))
        .rounded_full()
        .flex_none()
        .bg(color);
    if direction.is_rtl() {
        bar.right_0()
    } else {
        bar.left_0()
    }
}

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
        let dot = div()
            .flex_none()
            .size(px(theme.measures.status_mark))
            .rounded_full()
            .bg(color);
        match self.busy {
            Some(ident) if self.activity == motion::Activity::Advancing => {
                let mark = div()
                    .relative()
                    .flex_none()
                    .size(px(theme.measures.status_mark))
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
        let direction = cx.layout_direction();
        let content = div()
            .w_full()
            .flex()
            .flex_row()
            .items_start()
            .gap_token(&theme, Space::Sm)
            .child(severity_mark(self.tone, &theme))
            .child(div().min_w_0().child(self.message.clone()));

        let frame = div()
            .relative()
            .w_full()
            .overflow_hidden()
            .px_token(&theme, Space::Lg)
            .py_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .surface(&theme, Surface::Panel)
            .child(tone_rail(&theme, color, direction))
            .type_scale(&theme, TypeScale::Label)
            .line_height(px(theme.typography.body.line_height))
            .text_color(theme.colors.text);

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
            .relative()
            .w_full()
            .overflow_hidden()
            .row()
            .items_start()
            .gap_token(&theme, Space::Sm)
            .px_token(&theme, Space::Lg)
            .py_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .surface(&theme, Surface::Panel)
            // A page-level report is read from across the window, where a
            // rail two and a half pixels wide is not a colour anybody has
            // seen yet. The wash is the Light tier's colour at a fraction of
            // its strength, so the tone reaches the surface without the
            // surface being spent on it.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .radius(&theme, Radius::Card)
                    .bg(banner_wash(&theme, color)),
            )
            .child(tone_rail(&theme, color, cx.layout_direction()))
            .child(severity_mark(self.tone, &theme))
            .child(
                div()
                    .column()
                    .gap_token(&theme, Space::Xs)
                    .items_start()
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
                            .text_color(theme.colors.text)
                            .child(self.message.clone()),
                    )
                    // An action inside a report is a control, not a second
                    // bar: it takes the width of its own label and no more.
                    .children(
                        self.action
                            .map(|action| div().row().flex_none().child(action)),
                    ),
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
