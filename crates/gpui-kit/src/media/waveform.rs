//! A caller-owned measured envelope.
//!
//! Peaks are already normalized to `0..=1`. This component never decodes
//! audio. A missing playhead draws the envelope without a played/ahead split.

use gpui::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, canvas, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::ActiveTheme;

use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::Ident;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

const PEAK_GAP: f32 = 1.0;
const PEAK_FLOOR: f32 = 0.08;

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
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        self.ident.child("empty"),
                        cx.strings().text(StringKey::WaveformEmpty),
                    )
                    .kind(EmptyKind::Empty)
                    .into_any_element()
                })
            }
            AudioWaveformState::Unavailable(reason) => {
                self.slots.or_else(slot::EMPTY, window, cx, |_, _| {
                    EmptyState::new(self.ident.child("unavailable"), reason.clone())
                        .kind(EmptyKind::Unavailable)
                        .into_any_element()
                })
            }
            AudioWaveformState::Ready | AudioWaveformState::Empty => {
                let peaks = self.peaks;
                let count = peaks.len();
                let playhead = self.playhead;
                let played = theme.colors.accent;
                let ahead = theme.colors.text_faint;
                div()
                    .w_full()
                    .h(px(48.0))
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| {
                                let count = peaks.len();
                                if count == 0 || bounds.size.width <= px(0.0) {
                                    return;
                                }
                                let width = f32::from(bounds.size.width) / count as f32;
                                let bar = (width - PEAK_GAP).max(0.5);
                                let height = f32::from(bounds.size.height);
                                let centre = f32::from(bounds.origin.y) + height / 2.0;
                                let head = playhead.map(|fraction| fraction * count as f32);
                                for (index, peak) in peaks.iter().enumerate() {
                                    let extent = (peak.clamp(0.0, 1.0).max(PEAK_FLOOR) * height
                                        / 2.0)
                                        .max(0.5);
                                    let left = f32::from(bounds.origin.x) + index as f32 * width;
                                    let color = match head {
                                        Some(head) if (index as f32) < head => played,
                                        _ => ahead,
                                    };
                                    window.paint_quad(gpui::fill(
                                        gpui::Bounds {
                                            origin: gpui::point(px(left), px(centre - extent)),
                                            size: gpui::size(px(bar), px(extent * 2.0)),
                                        },
                                        color,
                                    ));
                                }
                            },
                        )
                        .size_full(),
                    )
                    .semantic_in(
                        cx,
                        NodeSpec::new(self.ident.semantic_id(), Role::Image)
                            .text(cx.strings().text(StringKey::Waveform))
                            .value(count.to_string()),
                    )
                    .into_any_element()
            }
        }
    }
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
