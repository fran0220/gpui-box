//! A small identity mark.

use gpui::{
    App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
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
    tint: Option<Hsla>,
}

impl Avatar {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            ident: None,
            name: name.into(),
            image: None,
            size: 28.0,
            tint: None,
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

    /// The colour this identity is known by.
    ///
    /// An application that gives people or workspaces a colour needs the mark
    /// to wear it; without this the disc is the same neutral for everyone and
    /// the colour has to be redrawn beside it. The treatment is the tone
    /// language's — a carried wash behind the letters rather than a filled
    /// disc — so a roster of tinted avatars stays as quiet as the rest of the
    /// surface. A tint is never derived from the name: an application that
    /// wants a stable colour per name derives it and passes it, because this
    /// component cannot know which colours that application has already spent.
    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
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

        let (background, border, foreground) = match self.tint {
            Some(tint) => (tint.opacity(0.18), tint.opacity(0.35), tint),
            None => (
                theme.colors.raised,
                theme.colors.hairline,
                theme.colors.text_muted,
            ),
        };

        let element = div()
            .size(px(self.size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .rounded_full()
            .bg(background)
            .border(px(theme.borders.hairline))
            .border_color(border)
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .text_size(px(self.size * 0.36))
            .text_color(foreground)
            .when_some(self.image.clone(), |element, source| {
                element.child(gpui::img(source).size(px(self.size)))
            })
            .when(self.image.is_none() && !initials.is_empty(), |element| {
                element.child(initials.clone())
            });
        match spec {
            Some(spec) => element.semantic_in(cx, spec).into_any_element(),
            None => element.into_any_element(),
        }
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
