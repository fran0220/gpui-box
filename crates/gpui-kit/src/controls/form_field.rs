//! The label, description, and error a control wears.
//!
//! A field never decides whether a control is valid. It shows the error the
//! host handed it, publishes that the control is invalid and whether it is
//! required, and otherwise stays out of the way.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TypeScale};

use crate::foundation::{Ident, StyledExt, text as foundation_text};
use crate::overlay::Kbd;

/// A labelled control, with the secondary text a typist needs around it.
///
/// The description and the error are both shown. An error is information the
/// description did not already carry — what went wrong this time, on top of
/// what the field is for — so it is added rather than substituted. The one
/// exception is an error worded exactly like the description, which would
/// otherwise be printed twice; then only the error is drawn, because it is
/// the one that also carries the failure.
#[derive(IntoElement)]
pub struct FormField {
    ident: Ident,
    label: SharedString,
    control: Option<SharedString>,
    description: Option<SharedString>,
    error: Option<SharedString>,
    hint: Option<SharedString>,
    required: bool,
    children: Vec<AnyElement>,
}

impl std::fmt::Debug for FormField {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FormField")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("control", &self.control)
            .field("required", &self.required)
            .field("invalid", &self.error.is_some())
            .finish()
    }
}

impl FormField {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            control: None,
            description: None,
            error: None,
            hint: None,
            required: false,
            children: Vec::new(),
        }
    }

    /// The semantic id of the control this label names.
    ///
    /// The label publishes the association, so a test that knows only what a
    /// field is called can find the control it belongs to.
    pub fn control(mut self, control: impl Into<SharedString>) -> Self {
        self.control = Some(control.into());
        self
    }

    /// What the field is for, shown whether or not anything went wrong.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// What the host says is wrong, in the host's own words.
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// The keystroke that operates the control without the pointer.
    pub fn hint(mut self, keystroke: impl Into<SharedString>) -> Self {
        self.hint = Some(keystroke.into());
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn is_invalid(&self) -> bool {
        self.error.is_some()
    }

    /// The description, unless the error would repeat it word for word.
    fn shown_description(&self) -> Option<SharedString> {
        match (&self.description, &self.error) {
            (Some(description), Some(error)) if description == error => None,
            (description, _) => description.clone(),
        }
    }
}

impl ParentElement for FormField {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for FormField {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let invalid = self.error.is_some();
        let field_id = self.ident.semantic_id();
        let label_ident = self.ident.child("label");
        let description = self.shown_description();

        let mut label_spec = NodeSpec::new(label_ident.semantic_id(), Role::Text)
            .parent(field_id.clone())
            .text(self.label.clone());
        if let Some(control) = self.control.clone() {
            label_spec = label_spec.labels(control);
        }

        let label_element = foundation_text(&theme, TypeScale::Label, self.label.clone())
            .row()
            .gap_token(&theme, Space::Xs)
            .when(self.required, |element| {
                element.child(
                    foundation_text(&theme, TypeScale::Label, SharedString::from("*"))
                        .text_color(theme.colors.danger),
                )
            })
            .when_some(self.hint.clone(), |element, keystroke| {
                element.child(
                    div()
                        .ml_auto()
                        .child(Kbd::new(keystroke).id(self.ident.child("hint"))),
                )
            })
            .semantic_in(cx, label_spec);

        let description = description.map(|text| {
            foundation_text(&theme, TypeScale::Caption, text.clone())
                .text_tone(&theme, gpui_kit_theme::TextTone::Muted)
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("description").semantic_id(), Role::Text)
                        .parent(field_id.clone())
                        .text(text),
                )
        });

        let error = self.error.clone().map(|text| {
            foundation_text(&theme, TypeScale::Caption, text.clone())
                .text_color(theme.colors.danger)
                .mt_token(&theme, Space::Sm)
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("error").semantic_id(), Role::Status)
                        .parent(field_id.clone())
                        .invalid(true)
                        .text(text),
                )
        });

        div()
            .column()
            .w_full()
            .gap(px(theme.space(Space::Xs)))
            .child(label_element)
            .children(self.children)
            .children(description)
            .children(error)
            .semantic_in(
                cx,
                NodeSpec::new(field_id, Role::Field)
                    .text(self.label.clone())
                    .required(self.required)
                    .invalid(invalid),
            )
    }
}
