//! A small identity mark.

use gpui::{
    App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, SemanticWash, Theme, TypeScale};

use crate::foundation::{Ident, StyledExt};
use crate::strings::ActiveNumbers;

/// Whether the identity behind the mark is reachable, in the host's terms.
///
/// Presence is a fact the host holds, so the component neither derives it nor
/// times it out. `Unknown` is the default and draws nothing: a mark with no
/// dot says nothing about presence, which is different from saying offline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AvatarPresence {
    #[default]
    Unknown,
    Online,
    Away,
    Busy,
    Offline,
}

impl AvatarPresence {
    pub fn name(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Online => "online",
            Self::Away => "away",
            Self::Busy => "busy",
            Self::Offline => "offline",
        }
    }

    /// The fill of the dot, and whether the dot is filled at all.
    ///
    /// Offline is a ring rather than a disc: the two states a reader most
    /// needs to tell apart are online and not, and a dim disc beside a bright
    /// one is one difference where a filled shape beside a hollow one is two.
    fn paint(self, theme: &Theme) -> Option<(Hsla, bool)> {
        match self {
            Self::Unknown => None,
            Self::Online => Some((theme.colors.success, true)),
            Self::Away => Some((theme.colors.warning, true)),
            Self::Busy => Some((theme.colors.danger, true)),
            Self::Offline => Some((theme.colors.text_faint, false)),
        }
    }
}

/// How much of the disc the lettering takes, and the smallest it is drawn at.
///
/// The share is of the diameter rather than one size for every avatar: a 56px
/// mark set at the 28px mark's size reads as a letter that failed to grow with
/// the circle around it. The floor is where two capitals stop being letters
/// and start being a smudge, and a smudge that could be anybody's initials is
/// a worse answer than an honest mark.
const INITIALS_RATIO: f32 = 0.40;
const INITIALS_FLOOR: f32 = 9.0;

/// The size initials are drawn at on a disc of `size`, when they are drawn.
fn initials_size(size: f32) -> Option<f32> {
    let scaled = (size * INITIALS_RATIO).round();
    (scaled >= INITIALS_FLOOR).then_some(scaled)
}

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
    presence: AvatarPresence,
    /// The colour the mark is cut out of when it overlaps another one. Only a
    /// stack sets this; a mark standing on its own has nothing to cut.
    stacked_on: Option<Hsla>,
}

impl Avatar {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            ident: None,
            name: name.into(),
            image: None,
            size: 28.0,
            tint: None,
            presence: AvatarPresence::default(),
            stacked_on: None,
        }
    }

    /// Whether the identity is reachable, when the host knows.
    pub fn presence(mut self, presence: AvatarPresence) -> Self {
        self.presence = presence;
        self
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
    /// surface. A caller that has already spent a colour on this identity
    /// passes it here. A tint is never derived from the name: an application
    /// that wants a stable colour per identity derives it and passes it,
    /// because this component cannot know which colours that application has
    /// already spent.
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
        let anonymous = self.image.is_none() && initials.is_empty();
        // Lettering that has fallen under the legibility floor is not drawn,
        // so a mark stands where it would have been: the disc still says an
        // identity is here, and it stops claiming to say whose in glyphs
        // nobody can read.
        let lettering = (!initials.is_empty())
            .then(|| initials_size(self.size))
            .flatten();
        let marked = self.image.is_none() && lettering.is_none();
        let spec = self.ident.as_ref().map(|ident| {
            NodeSpec::new(ident.semantic_id(), Role::Image)
                .text(self.name.clone())
                .value(self.presence.name())
        });

        let (background, foreground) = match (self.tint, anonymous) {
            (Some(tint), _) => (theme.color_wash(tint, SemanticWash::Strong), tint),
            // Nothing to derive a mark from is its own state, and it is drawn
            // as one: a recessed well with a quiet center mark separates it
            // from a filled disc whose picture failed to arrive.
            (None, true) => (theme.colors.sunken, theme.colors.text_faint),
            (None, false) => (theme.colors.raised, theme.colors.text_muted),
        };

        let disc = div()
            .size(px(self.size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .rounded_full()
            .bg(background)
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .when_some(lettering, |element, size| element.text_size(px(size)))
            .text_color(foreground)
            .when_some(self.image.clone(), |element, source| {
                element.child(gpui::img(source).size(px(self.size)))
            })
            .when(self.image.is_none() && lettering.is_some(), |element| {
                element.child(initials.clone())
            })
            .when(marked, |element| {
                element.child(
                    div()
                        .size(px((self.size * 0.34).max(8.0)))
                        .rounded_full()
                        .bg(theme
                            .colors
                            .text_faint
                            .opacity(theme.effects.semantic_wash_strong_alpha)),
                )
            });

        // A stack seats each mark on the surface behind it, so overlapping
        // never slices a neighbour's presence dot in half.
        let cut = self.stacked_on.map(|_| theme.borders.thick * 1.5);
        let element = div()
            .relative()
            .flex_none()
            .size(px(self.size + 2.0 * cut.unwrap_or(0.0)))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .when_some(self.stacked_on, |element, behind| element.bg(behind))
            .child(disc)
            .children(self.presence.paint(&theme).map(|(color, filled)| {
                let diameter = (self.size * 0.32).max(8.0);
                div()
                    .absolute()
                    .right(px(cut.unwrap_or(0.0)))
                    .bottom(px(cut.unwrap_or(0.0)))
                    .size(px(diameter))
                    .rounded_full()
                    .bg(theme.colors.canvas)
                    .p(px(theme.borders.thick))
                    .child(
                        div()
                            .size_full()
                            .rounded_full()
                            .bg(color.opacity(if filled {
                                1.0
                            } else {
                                theme.effects.semantic_wash_strong_alpha
                            }))
                            .when(filled, |element| element.shadow(theme.glow(color))),
                    )
            }));

        match spec {
            Some(spec) => element.semantic_in(cx, spec).into_any_element(),
            None => element.into_any_element(),
        }
    }
}

/// Several identities in one place, overlapping, with the rest counted.
///
/// The stack exists because a row of separate discs takes the width of the
/// list and a roster does not have it. What it must not do is let one mark
/// eat the edge or the presence dot of the one beside it, which is what the
/// cut-out ring on every member is for.
#[derive(IntoElement)]
pub struct AvatarGroup {
    ident: Option<Ident>,
    members: Vec<Avatar>,
    size: f32,
    /// How many identities are in the group beyond the ones drawn. The host
    /// counts them; the component never infers a remainder it was not told.
    overflow: Option<SharedString>,
}

impl std::fmt::Debug for AvatarGroup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AvatarGroup")
            .field("ident", &self.ident)
            .field("members", &self.members.len())
            .finish()
    }
}

impl Default for AvatarGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl AvatarGroup {
    pub fn new() -> Self {
        Self {
            ident: None,
            members: Vec::new(),
            size: 28.0,
            overflow: None,
        }
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    pub fn members(mut self, members: impl IntoIterator<Item = Avatar>) -> Self {
        self.members = members.into_iter().collect();
        self
    }

    /// The diameter every member is drawn at. A stack of two sizes is two
    /// stacks.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// The host's own wording for what is not shown, such as "+4".
    pub fn overflow(mut self, overflow: impl Into<SharedString>) -> Self {
        self.overflow = Some(overflow.into());
        self
    }
}

impl RenderOnce for AvatarGroup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let size = self.size;
        let overlap = size * 0.32;
        let members = self.members.len();
        let element = div()
            .row()
            .flex_none()
            .children(self.members.into_iter().enumerate().map(|(index, member)| {
                let mut member = member.size(size);
                member.stacked_on = Some(theme.colors.canvas);
                div()
                    .flex_none()
                    .when(index > 0, |element| element.ml(px(-overlap)))
                    .child(member)
            }))
            .children(self.overflow.map(|overflow| {
                // The remainder is a count, not a person, so it is quieter
                // than the marks it follows rather than louder.
                div()
                    .flex_none()
                    .ml(px(-overlap))
                    .size(px(size))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme.colors.raised)
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.text_muted)
                    .child(overflow)
            }));
        match self.ident {
            Some(ident) => element
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Group)
                        .value(cx.numbers().count(members)),
                )
                .into_any_element(),
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

    #[test]
    fn lettering_grows_with_the_disc_and_stops_at_the_floor() {
        assert_eq!(initials_size(56.0), Some(22.0));
        assert_eq!(initials_size(40.0), Some(16.0));
        assert_eq!(initials_size(28.0), Some(11.0));
        assert_eq!(
            initials_size(20.0),
            None,
            "eight pixels of two capitals is a smudge, so the disc marks instead"
        );
    }
}
