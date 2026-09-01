//! A glyph, drawn from the bundled catalog.
//!
//! This is a display component: it reads a [`Glyph`] the caller chose and
//! paints it. It installs no handlers and emits nothing. A glyph that can be
//! clicked is [`IconButton`](crate::controls::button::IconButton), which
//! already exists and is not reimplemented here.
//!
//! The catalog has one resting visual language: Phosphor Regular. A caller may
//! choose the matching Fill drawing with [`Glyph::filled`] when the same symbol
//! reports a selected, checked, or committed state. Fill is state, not a second
//! decorative style.
//!
//! Three other things are deliberately not caller-supplied numbers:
//!
//! - **Size** comes from the token control scale, the same `iconSize` step
//!   [`Button`](crate::controls::button::Button) already resolves, so a glyph
//!   beside a small button is the size a small button's glyph is.
//! - **Colour** comes from a semantic role, not an `Hsla`, so a glyph cannot
//!   name a colour the theme does not have.
//! - **Direction** comes from the active [`LayoutDirection`], and whether the
//!   glyph responds to it comes from the drawing via
//!   [`Glyph::mirrors_in_rtl`].
//!
//! # What assistive technology hears
//!
//! Most glyphs in a real interface sit next to a label that already says what
//! they mean, and announcing both says everything twice. So [`Icon::new`] is
//! decorative: it publishes no semantic node at all, and a reader walks past
//! it. A glyph that is the only carrier of its meaning has to be named, and
//! [`Icon::named`] asks for both an [`Ident`] and the name, because a picture
//! has no text to fall back on and there is no safe guess. The default is the
//! quiet one, so forgetting to decide produces a silent icon rather than a
//! wrong announcement.

use gpui::{
    App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Svg, Transformation,
    Window, div, prelude::FluentBuilder, px, size,
};
use gpui_kit_assets::{Icon as Glyph, icon as glyph_svg};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, SemanticColor, TextTone, Theme};

use crate::foundation::direction::{ActiveDirection, LayoutDirection};
use crate::foundation::{Ident, Sizable};
use crate::motion;

/// The semantic colour role a glyph paints in.
///
/// Every variant names a role the theme already carries, so a glyph never
/// introduces a colour the token document has not authorised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconTone {
    #[default]
    Primary,
    Muted,
    Faint,
    OnAccent,
    Accent,
    AccentStrong,
    Danger,
    Warning,
    Success,
    Info,
}

impl IconTone {
    pub fn color(self, theme: &Theme) -> Hsla {
        match self {
            Self::Primary => theme.text_color(TextTone::Primary),
            Self::Muted => theme.text_color(TextTone::Muted),
            Self::Faint => theme.text_color(TextTone::Faint),
            Self::OnAccent => theme.text_color(TextTone::OnAccent),
            Self::Accent => theme.semantic_color(SemanticColor::Accent),
            Self::AccentStrong => theme.semantic_color(SemanticColor::AccentStrong),
            Self::Danger => theme.semantic_color(SemanticColor::Danger),
            Self::Warning => theme.semantic_color(SemanticColor::Warning),
            Self::Success => theme.semantic_color(SemanticColor::Success),
            Self::Info => theme.semantic_color(SemanticColor::Info),
        }
    }
}

/// How a glyph reaches assistive technology.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Announcement {
    /// Repeats something already said next to it, and is skipped.
    Decorative,
    /// Carries the meaning on its own, and is announced under this name.
    Named { ident: Ident, name: SharedString },
}

/// A glyph from the bundled catalog.
#[derive(Debug, Clone, IntoElement)]
pub struct Icon {
    glyph: Glyph,
    size: ControlSize,
    tone: IconTone,
    announcement: Announcement,
    follow_direction: bool,
    /// How this glyph reports work in progress, and the identity that
    /// animation runs under. `None` is a glyph reporting a settled state.
    activity: Option<(motion::Activity, Ident)>,
    /// A bounded response to a state change, distinct from ongoing activity.
    reaction: Option<(motion::Micro, Ident)>,
}

impl Icon {
    /// A glyph that repeats an adjacent label, and is not announced.
    pub fn new(glyph: Glyph) -> Self {
        Self {
            glyph,
            size: ControlSize::default(),
            tone: IconTone::default(),
            announcement: Announcement::Decorative,
            follow_direction: true,
            activity: None,
            reaction: None,
        }
    }

    /// Turns the glyph, for a state that is still running.
    ///
    /// This exists because the alternative kept being chosen by accident: a
    /// rotation glyph is the obvious drawing for "working", and a rotation
    /// glyph that does not rotate reads as one that has jammed. Whichever
    /// component reports running work, it reports it the same way through
    /// here.
    ///
    /// The identity is asked for rather than derived because a decorative
    /// glyph has none, and an animation needs something stable to run under.
    pub fn spinning(mut self, ident: impl Into<Ident>) -> Self {
        self.activity = Some((motion::Activity::Working, ident.into()));
        self.reaction = None;
        self
    }

    /// Breathes the glyph, for a state that is deliberating rather than
    /// getting through work. The quieter of the two claims.
    pub fn breathing(mut self, ident: impl Into<Ident>) -> Self {
        self.activity = Some((motion::Activity::Deliberating, ident.into()));
        self.reaction = None;
        self
    }

    /// Reacts once to a settled state change with a named micro-motion.
    ///
    /// Typical pairings are [`motion::Micro::Pop`] for success,
    /// [`motion::Micro::Wobble`] for refusal or error, and
    /// [`motion::Micro::Sparkle`] for a newly available result. Reduced-motion
    /// policy is resolved by the shared motion authority, not by the caller.
    pub fn reacting(mut self, ident: impl Into<Ident>, reaction: motion::Micro) -> Self {
        self.activity = None;
        self.reaction = Some((reaction, ident.into()));
        self
    }

    /// A glyph that is the only thing saying what it means.
    ///
    /// The name is a constructor argument rather than an option for the same
    /// reason [`IconButton`](crate::controls::button::IconButton)'s is: a
    /// picture nobody can name is a picture nobody can address.
    pub fn named(ident: impl Into<Ident>, glyph: Glyph, name: impl Into<SharedString>) -> Self {
        Self {
            announcement: Announcement::Named {
                ident: ident.into(),
                name: name.into(),
            },
            ..Self::new(glyph)
        }
    }

    pub fn tone(mut self, tone: IconTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn muted(self) -> Self {
        self.tone(IconTone::Muted)
    }

    pub fn faint(self) -> Self {
        self.tone(IconTone::Faint)
    }

    pub fn on_accent(self) -> Self {
        self.tone(IconTone::OnAccent)
    }

    pub fn accent(self) -> Self {
        self.tone(IconTone::Accent)
    }

    pub fn danger(self) -> Self {
        self.tone(IconTone::Danger)
    }

    pub fn warning(self) -> Self {
        self.tone(IconTone::Warning)
    }

    pub fn success(self) -> Self {
        self.tone(IconTone::Success)
    }

    pub fn info(self) -> Self {
        self.tone(IconTone::Info)
    }

    /// Whether the active reading direction may flip this glyph.
    ///
    /// On by default, and the glyph still decides: only a
    /// [`Directional`](gpui_kit_assets::Mirroring::Directional) drawing ever
    /// flips. Turning it off is for a caller that has already rotated the
    /// glyph into another meaning, where a horizontal flip would land it
    /// somewhere neither reading direction points.
    pub fn follow_direction(mut self, follow: bool) -> Self {
        self.follow_direction = follow;
        self
    }

    /// The painted edge length, in pixels, at the active theme and size.
    pub fn resolved_size(&self, theme: &Theme) -> f32 {
        theme.control.get(self.size).icon_size
    }

    pub fn resolved_color(&self, theme: &Theme) -> Hsla {
        self.tone.color(theme)
    }

    /// Whether this glyph would be drawn flipped in `direction`.
    pub fn flips_in(&self, direction: LayoutDirection) -> bool {
        self.follow_direction && direction.is_rtl() && self.glyph.mirrors_in_rtl()
    }
}

impl Sizable for Icon {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let edge = self.resolved_size(&theme);
        let drawing = paint(
            self.glyph,
            edge,
            self.resolved_color(&theme),
            self.flips_in(direction),
        );
        let drawing = match (self.activity, self.reaction) {
            (_, Some((reaction, ident))) => {
                motion::micro(drawing, ident.element_id(), reaction, cx)
            }
            (Some((motion::Activity::Working, ident)), None) => {
                motion::spin(drawing, ident.element_id(), &theme, cx)
            }
            (Some((_, ident)), None) => motion::breathe(drawing, ident.element_id(), &theme, cx),
            (None, None) => drawing.into_any_element(),
        };

        match self.announcement {
            // A decorative glyph is the bare drawing: no wrapper, no node, and
            // therefore nothing for a reader to stop on and nothing extra in
            // the layout either.
            Announcement::Decorative => drawing,
            Announcement::Named { ident, name } => div()
                .flex()
                .flex_none()
                .size(px(edge))
                .child(drawing)
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Image).text(name),
                )
                .into_any_element(),
        }
    }
}

/// One glyph, sized, coloured, and flipped or not.
///
/// Exposed so a component that draws a glyph inside a frame it already owns —
/// a disclosure triangle inside a hit target, a marker inside a step dot —
/// gets the same reading-direction behaviour without wrapping another
/// element around it.
pub fn paint(glyph: Glyph, edge: f32, color: Hsla, flipped: bool) -> Svg {
    glyph_svg(glyph)
        .size(px(edge))
        .text_color(color)
        .when(flipped, |svg| {
            svg.with_transformation(Transformation::scale(size(-1.0, 1.0)))
        })
}

/// Whether `glyph` should be drawn flipped when the interface reads
/// `direction`.
pub fn flips(glyph: Glyph, direction: LayoutDirection) -> bool {
    direction.is_rtl() && glyph.mirrors_in_rtl()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_glyph_at_two_token_sizes_is_two_sizes() {
        let theme = Theme::studio_dark();
        let small = Icon::new(Glyph::Check).small();
        let large = Icon::new(Glyph::Check).large();
        assert_ne!(small.resolved_size(&theme), large.resolved_size(&theme));
        assert!(small.resolved_size(&theme) < large.resolved_size(&theme));
        // The scale is the one buttons already resolve, so adopting it
        // cannot move a glyph that a button drew.
        assert_eq!(
            Icon::new(Glyph::Check).medium().resolved_size(&theme),
            theme.control.get(ControlSize::Md).icon_size
        );
    }

    #[test]
    fn a_tone_names_a_role_the_theme_carries() {
        let theme = Theme::studio_dark();
        assert_eq!(
            Icon::new(Glyph::Danger).danger().resolved_color(&theme),
            theme.colors.danger
        );
        assert_eq!(
            Icon::new(Glyph::Check).muted().resolved_color(&theme),
            theme.colors.text_muted
        );
        assert_ne!(
            Icon::new(Glyph::Check).resolved_color(&theme),
            Icon::new(Glyph::Check).muted().resolved_color(&theme)
        );
    }

    #[test]
    fn a_directional_glyph_flips_and_a_symbol_does_not() {
        let rtl = LayoutDirection::RightToLeft;
        let ltr = LayoutDirection::LeftToRight;
        assert!(Icon::new(Glyph::AltArrowRight).flips_in(rtl));
        assert!(!Icon::new(Glyph::AltArrowRight).flips_in(ltr));
        assert!(!Icon::new(Glyph::Check).flips_in(rtl));
        // A caller that has already rotated the glyph can decline.
        assert!(
            !Icon::new(Glyph::AltArrowRight)
                .follow_direction(false)
                .flips_in(rtl)
        );
    }
}
