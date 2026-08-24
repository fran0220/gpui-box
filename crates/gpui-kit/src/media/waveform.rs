//! A caller-owned measured envelope.
//!
//! Peaks are already normalized to `0..=1`. This component never decodes
//! audio. A missing playhead draws the envelope without a played/ahead split.

use gpui::{
    AnyElement, App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    canvas, div, point, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, Theme, TypeScale};

use crate::display::signature;
use crate::foundation::Ident;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{StyledExt, text};
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// The space kept between two bars, and the shortest bar a peak of nearly
/// nothing still draws — so a quiet passage reads as quiet rather than as a
/// gap in the measurement.
const PEAK_GAP: f32 = 1.0;
const PEAK_FLOOR: f32 = 0.08;

/// How tall the band is when nobody says otherwise.
///
/// One height for every envelope this library draws, so a waveform under a
/// player and a waveform on its own are the same object seen twice.
pub(crate) const BAND_HEIGHT: f32 = 56.0;

/// How the envelope was asked for.
#[derive(Debug, Clone, PartialEq)]
pub enum AudioWaveformState {
    Ready,
    Empty,
    Unavailable(SharedString),
}

impl AudioWaveformState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Empty => "empty",
            Self::Unavailable(_) => "unavailable",
        }
    }
}

impl HasPhase for AudioWaveformState {
    fn phase(&self) -> Phase {
        match self {
            Self::Ready => Phase::Ready,
            Self::Empty => Phase::Empty,
            Self::Unavailable(_) => Phase::Unavailable,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(reason) => Some(reason.as_ref()),
            _ => None,
        }
    }
}

/// A measured waveform the host already sampled.
#[derive(IntoElement)]
pub struct AudioWaveform {
    ident: Ident,
    peaks: Vec<f32>,
    playhead: Option<f32>,
    state: AudioWaveformState,
    slots: Slots,
}

impl AudioWaveform {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            peaks: Vec::new(),
            playhead: None,
            state: AudioWaveformState::Ready,
            slots: Slots::default(),
        }
    }

    pub fn peaks(mut self, peaks: impl IntoIterator<Item = f32>) -> Self {
        self.peaks = peaks.into_iter().map(|peak| peak.clamp(0.0, 1.0)).collect();
        self
    }

    /// Normalized playhead. Without one, nothing is drawn as played.
    pub fn playhead(mut self, fraction: f32) -> Self {
        self.playhead = Some(fraction.clamp(0.0, 1.0));
        self
    }

    pub fn state(mut self, state: AudioWaveformState) -> Self {
        self.state = state;
        self
    }
}

impl Slotted for AudioWaveform {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for AudioWaveform {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        match &self.state {
            AudioWaveformState::Empty | AudioWaveformState::Ready if self.peaks.is_empty() => {
                let ident = self.ident.child("empty");
                let strings = cx.strings().clone();
                self.slots.or_else(slot::EMPTY, window, cx, move |_, cx| {
                    vacant(
                        &ident,
                        strings.text(StringKey::WaveformEmpty),
                        None,
                        false,
                        &theme,
                        cx,
                    )
                })
            }
            AudioWaveformState::Unavailable(reason) => {
                let ident = self.ident.child("unavailable");
                let strings = cx.strings().clone();
                let reason = reason.clone();
                self.slots.or_else(slot::EMPTY, window, cx, move |_, cx| {
                    vacant(
                        &ident,
                        strings.text(StringKey::WaveformUnavailable),
                        Some(reason.clone()),
                        true,
                        &theme,
                        cx,
                    )
                })
            }
            AudioWaveformState::Ready | AudioWaveformState::Empty => {
                let count = self.peaks.len();
                band(&theme)
                    .child(peak_band(
                        self.peaks,
                        self.playhead,
                        signature::mark(&theme),
                        theme.colors.hairline_strong,
                    ))
                    .semantic_in(
                        cx,
                        NodeSpec::new(self.ident.semantic_id(), Role::Image)
                            .text(cx.strings().text(StringKey::Waveform))
                            .value(cx.numbers().count(count)),
                    )
                    .into_any_element()
            }
        }
    }
}

/// The frame every envelope is drawn in: a recess, at one height, with the
/// zero line the bars are measured from already in it.
///
/// A waveform floating on the page has no zero and no extent, so a reader
/// cannot tell a quiet passage from a short one.
pub(crate) fn band(theme: &Theme) -> gpui::Div {
    div()
        .relative()
        .w_full()
        .h(px(BAND_HEIGHT))
        .px_token(theme, Space::Xs)
        .radius(theme, Radius::Card)
        .surface(theme, Surface::Sunken)
        .hairline(theme)
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(BAND_HEIGHT / 2.0))
                .h(px(theme.borders.hairline))
                .bg(theme.colors.hairline),
        )
}

/// The band with a sentence in it instead of an envelope.
///
/// Empty and unavailable are the same shape as ready and are told apart by
/// what is written in them and by the mark beside it, rather than by one of
/// them being a line of text on the bare page.
fn vacant(
    ident: &Ident,
    headline: SharedString,
    detail: Option<SharedString>,
    refused: bool,
    theme: &Theme,
    cx: &mut App,
) -> AnyElement {
    let tint = if refused {
        theme.colors.warning
    } else {
        theme.colors.text_faint
    };
    band(theme)
        .flex()
        .items_center()
        .justify_center()
        .gap_token(theme, Space::Sm)
        .child(
            gpui_kit_assets::icon(if refused {
                Icon::CloseCircle
            } else {
                Icon::SoundWave
            })
            .size(px(theme.typography.subtitle.line_height))
            .text_color(tint),
        )
        .child(
            div()
                .column()
                .min_w_0()
                .child(text(theme, TypeScale::Label, headline.clone()).text_color(tint))
                .children(detail.clone().map(|detail| {
                    text(theme, TypeScale::Caption, detail).text_color(theme.colors.text_muted)
                })),
        )
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Status)
                .text(headline)
                .value(detail.unwrap_or_else(|| SharedString::new_static("empty"))),
        )
        .into_any_element()
}

/// The measured envelope, with the part behind the head drawn as played.
///
/// Bars are placed on whole pixels and are all one whole-pixel width. Left to
/// fractions, a bar landing at `x.5` is drawn across two columns at half
/// weight and its neighbour is not, which reads as a measurement that varied
/// rather than as a rasterisation that did.
pub(crate) fn peak_band(
    peaks: Vec<f32>,
    fraction: Option<f32>,
    played: Hsla,
    ahead: Hsla,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let count = peaks.len();
            if count == 0 || bounds.size.width <= px(0.0) {
                return;
            }
            let pitch = f32::from(bounds.size.width) / count as f32;
            let bar = (pitch - PEAK_GAP).floor().max(1.0);
            let height = f32::from(bounds.size.height);
            let centre = f32::from(bounds.origin.y) + height / 2.0;
            // With no duration there is no fraction, so nothing is drawn as
            // played rather than everything or nothing being guessed at.
            let head = fraction.map(|fraction| fraction * count as f32);
            for (index, peak) in peaks.iter().enumerate() {
                let extent = (peak.clamp(0.0, 1.0).max(PEAK_FLOOR) * height / 2.0).max(0.5);
                let left = (f32::from(bounds.origin.x) + index as f32 * pitch).round();
                let color = match head {
                    Some(head) if (index as f32) < head => played,
                    _ => ahead,
                };
                window.paint_quad(gpui::fill(
                    gpui::Bounds {
                        origin: point(px(left), px((centre - extent).round())),
                        size: gpui::size(px(bar), px((extent * 2.0).round().max(1.0))),
                    },
                    color,
                ));
            }
        },
    )
    .size_full()
}

#[cfg(test)]
mod waveform_phase_tests {
    use super::*;

    #[test]
    fn unavailable_is_not_empty() {
        let state = AudioWaveformState::Unavailable("no samples".into());
        assert_eq!(state.phase(), Phase::Unavailable);
        assert_eq!(state.name(), "unavailable");
        assert_eq!(state.reason(), Some("no samples"));
    }
}
