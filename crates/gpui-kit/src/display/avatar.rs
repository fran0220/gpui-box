//! A small identity mark.

use gpui::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::ActiveTheme;

use crate::foundation::Ident;

/// A circular mark for a person or a workspace.
///
/// With no image it falls back to initials, and with nothing to take initials
/// from it stays an empty circle rather than inventing a letter.
#[derive(Debug, IntoElement)]
pub struct Avatar {
    ident: Option<Ident>,
    name: SharedString,
    image: Option<SharedString>,
    size: f32,
}

impl Avatar {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            ident: None,
            name: name.into(),
            image: None,
            size: 28.0,
        }
    }

    /// Gives the avatar a semantic identity, for a test that has to find it.
    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    /// A resource path or URI the asset source can resolve.
    pub fn image(mut self, image: impl Into<SharedString>) -> Self {
        self.image = Some(image.into());
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// At most two letters, taken from the first two words.
    fn initials(&self) -> SharedString {
        let letters: String = self
            .name
            .split_whitespace()
            .filter_map(|word| word.chars().next())
            .take(2)
            .flat_map(|letter| letter.to_uppercase())
            .collect();
        SharedString::from(letters)
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let initials = self.initials();
        let spec = self
            .ident
            .as_ref()
            .map(|ident| NodeSpec::new(ident.semantic_id(), Role::Image).text(self.name.clone()));

        div()
            .size(px(self.size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .rounded_full()
            .bg(theme.colors.raised)
            .border(px(theme.borders.hairline))
            .border_color(theme.colors.hairline)
            .text_size(px(self.size * 0.36))
            .text_color(theme.colors.text_muted)
            .when_some(self.image.clone(), |element, source| {
                element.child(gpui::img(source).size(px(self.size)))
            })
            .when(self.image.is_none() && !initials.is_empty(), |element| {
                element.child(initials.clone())
            })
            .when_some(spec, |element, spec| element.semantic_in(cx, spec))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_take_at_most_two_words() {
        assert_eq!(Avatar::new("Ada Lovelace King").initials().as_ref(), "AL");
        assert_eq!(Avatar::new("ada").initials().as_ref(), "A");
    }

    #[test]
    fn a_nameless_avatar_invents_no_letter() {
        assert_eq!(Avatar::new("   ").initials().as_ref(), "");
    }
}
