use gpui::{App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};

use crate::display::badge::Tone;
use crate::foundation::{Ident, StyledExt};
use crate::motion;

/// A tone-colored dot, the smallest state indicator in the system.
#[derive(Debug, IntoElement)]
pub struct StatusDot {
    tone: Tone,
    /// The identity a breathing dot animates under, when it is reporting
    /// work that is still going.
    busy: Option<Ident>,
}

impl StatusDot {
    pub fn new(tone: Tone) -> Self {
        Self { tone, busy: None }
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
}

impl RenderOnce for StatusDot {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let dot = div()
            .flex_none()
            .size(px(7.0))
            .rounded_full()
            .bg(self.tone.color(&theme));
        match self.busy {
            Some(ident) => motion::breathe(dot, ident.element_id(), &theme, cx),
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
}

impl StatusLine {
    pub fn new(label: impl Into<SharedString>, tone: Tone) -> Self {
        Self {
            ident: None,
            label: label.into(),
            tone,
        }
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }
}

impl RenderOnce for StatusLine {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let element = div()
            .row()
            .gap_token(&theme, Space::Sm)
            .type_scale(&theme, TypeScale::Label)
            .text_color(theme.colors.text_muted)
            .child(StatusDot::new(self.tone))
            .child(self.label.clone());
        match self.ident {
            Some(ident) => element
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Status).text(self.label.clone()),
                )
                .into_any_element(),
            None => element.into_any_element(),
        }
    }
}

/// A bordered message block.
///
/// Callouts carry host refusals and stale-data warnings verbatim; they never
/// summarize an error into a friendlier but less true sentence.
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
