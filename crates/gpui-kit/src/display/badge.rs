use gpui::{
    App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::Icon as Glyph;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, ColorChoice, ControlSize, SemanticWash, Theme, TypeScale, Variant,
};

use crate::foundation::{Ident, Sizable, StyledExt};

/// The severity a status surface claims.
///
/// A tone is a statement about the state, not a decoration: Success on an idle
/// thing says the thing succeeded, which is a different and untrue sentence.
///
/// The mark surfaces built on a tone — [`Badge`], [`crate::prelude::StatusDot`]
/// and [`crate::prelude::StatusLine`] — additionally take a caller-owned
/// `tint`. A tint answers *whose* the mark is, the tone still answers *how it
/// is going*, and the two are independent: tinting never edits the reported
/// severity, which is why those surfaces publish the tone by name once they
/// can be painted a colour that is not derived from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
    Info,
}

impl Tone {
    /// The name a semantic node publishes, so a test can assert the severity
    /// a surface reported rather than the color it painted.
    pub fn name(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Accent => "accent",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Danger => "danger",
            Self::Info => "info",
        }
    }

    pub(crate) fn color(self, theme: &Theme) -> Hsla {
        match self {
            Self::Neutral => theme.colors.text_faint,
            Self::Accent => theme.colors.accent,
            Self::Success => theme.colors.success,
            Self::Warning => theme.colors.warning,
            Self::Danger => theme.colors.danger,
            Self::Info => theme.colors.info,
        }
    }

    /// The paint a mark uses: the caller's tint when it gave one, the tone's
    /// own colour otherwise.
    pub(crate) fn mark_color(self, tint: Option<Hsla>, theme: &Theme) -> Hsla {
        tint.unwrap_or_else(|| self.color(theme))
    }
}

/// A compact status label.
///
/// A badge only carries a semantic node when the caller gives it an id, so
/// decorative badges do not add noise to assertion snapshots.
#[derive(Debug, IntoElement)]
pub struct Badge {
    ident: Option<Ident>,
    label: SharedString,
    tone: Tone,
    tint: Option<Hsla>,
    variant: Option<Variant>,
    color: Option<ColorChoice>,
    size: ControlSize,
    glyph: Option<Glyph>,
    dot: bool,
}

impl Badge {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            ident: None,
            label: label.into(),
            tone: Tone::default(),
            tint: None,
            variant: None,
            color: None,
            size: ControlSize::Sm,
            glyph: None,
            dot: false,
        }
    }

    /// A glyph before the word, for a badge whose meaning has a picture.
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// A mark before the word, for a badge that reports a live state rather
    /// than a fixed label. Ignored when the badge already carries a glyph:
    /// two marks for one claim is one mark too many.
    pub fn dot(mut self, dot: bool) -> Self {
        self.dot = dot;
        self
    }

    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn neutral(self) -> Self {
        self.tone(Tone::Neutral)
    }

    pub fn accent(self) -> Self {
        self.tone(Tone::Accent)
    }

    pub fn success(self) -> Self {
        self.tone(Tone::Success)
    }

    pub fn warning(self) -> Self {
        self.tone(Tone::Warning)
    }

    pub fn danger(self) -> Self {
        self.tone(Tone::Danger)
    }

    pub fn info(self) -> Self {
        self.tone(Tone::Info)
    }

    /// Paints the badge in a caller-owned colour, leaving the severity it
    /// reports alone.
    ///
    /// For a badge that belongs to a colour-identified thing — a person, a
    /// branch, a label. The tinted badge keeps the tone language's own
    /// treatment, so a colour cannot turn one badge into a second badge
    /// shape, and [`Tone::name`] is still what the node publishes.
    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    /// Puts the badge on the shared presentation tiers.
    ///
    /// The tone still answers *how it is going* and is still what the node
    /// publishes; the tier only decides how loudly the colour is worn. See
    /// [`Theme::variant_colors`].
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = Some(variant);
        self
    }

    /// The colour the shared tiers resolve against, when the badge is on
    /// them. Without one, the tone's own colour (or the tint) is used.
    pub fn color(mut self, color: impl Into<ColorChoice>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl Sizable for Badge {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        // A badge is the tone itself, as a block. The tint is carried far
        // enough that the block reads on any surface the badge can land on,
        // which is what lets the outline go: an outline around a shape that
        // is already a colour was only ever compensating for a tint too weak
        // to see.
        // The wash is carried far enough to read and no further. A badge is a
        // word, and a word sitting in a saturated pill is a word that has to
        // be read through its own background; what identifies it is the
        // colour of the text, with just enough behind it to bound the shape.
        let (foreground, background) = if let Some(variant) = self.variant {
            // On the shared tiers the badge reads the same resolver as every
            // other coloured surface. The colour is the caller's choice,
            // falling back to the tint or to the tone's own colour, so a
            // badge that only sets a tier keeps saying what it already said.
            let choice = self.color.clone().unwrap_or_else(|| match self.tint {
                Some(tint) => ColorChoice::Custom(tint),
                None => match self.tone {
                    Tone::Neutral => ColorChoice::Custom(theme.colors.text_muted),
                    tone => ColorChoice::Custom(tone.color(&theme)),
                },
            });
            let resolved = theme.variant_colors(variant, &choice);
            (resolved.text, resolved.background)
        } else {
            let (foreground, background) = match (self.tint, self.tone) {
                // A tint is a colour the caller chose on purpose, so it takes
                // the coloured treatment even at Neutral: the tone says
                // nothing about severity there, which is exactly the case an
                // identity colour is for.
                (Some(tint), _) => (tint, theme.color_wash(tint, SemanticWash::Standard)),
                (None, Tone::Neutral) => (theme.colors.text_muted, theme.colors.hover),
                (None, tone) => {
                    let color = tone.color(&theme);
                    (color, theme.color_wash(color, SemanticWash::Standard))
                }
            };
            (foreground, background)
        };

        let step = theme.control.get(self.size);
        let element = div()
            .flex_none()
            .row()
            .gap(px(step.gap * 0.5))
            .h(px(step.height * 0.72))
            .px(px(step.padding_x * 0.6))
            .rounded_full()
            .bg(background)
            .type_scale(&theme, TypeScale::Caption)
            .font_weight(gpui::FontWeight(theme.typography.label.weight))
            .text_color(foreground)
            .children(self.glyph.map(|glyph| {
                crate::display::icon::paint(glyph, step.icon_size * 0.8, foreground, false)
            }))
            .when(self.glyph.is_none() && self.dot, |element| {
                element.child(
                    div()
                        .flex_none()
                        .size(px(theme.measures.status_mark * 0.5))
                        .rounded_full()
                        .bg(foreground),
                )
            })
            .child(self.label.clone());
        match self.ident {
            Some(ident) => element
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Status)
                        .text(self.label.clone())
                        // The severity by name, because a tint can paint this
                        // badge a colour no tone maps to and a reader would
                        // then have no way to ask what it claimed.
                        .value(self.tone.name()),
                )
                .into_any_element(),
            None => element.into_any_element(),
        }
    }
}
