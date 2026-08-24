//! A player for one piece of audio, over a transport this crate does not own.
//!
//! # What this component will not do
//!
//! **It does not play anything.** Every control asks the
//! [`MediaTransport`] and reports what the transport answered. A refused
//! command leaves the state that still holds, because the next frame draws
//! the transport's snapshot rather than what the control wished for.
//!
//! **It does not run a clock.** The position is the transport's, read once per
//! frame. A player with no transport draws no scrubber at all: a track with a
//! head at zero would say playback exists and has not started, and neither
//! half of that is true.
//!
//! **It does not draw a waveform it did not measure.** Peaks are a caller
//! input, the way an image's natural size is. A player given none shows the
//! transport and nothing above it, rather than a decorative envelope that
//! would read as the shape of this particular sound.
//!
//! **It does not format a time.** Elapsed and remaining are the host's own
//! strings, exactly as [`TransportBar`] takes them.
//!
//! **It says where its facts came from.** A fixture transport is published as
//! one and drawn as one.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, TextTone, TypeScale};

use crate::content::transport::{TransportBar, TransportDuration, TransportEvent};
use crate::display::badge::Badge;
use crate::display::signature;
use crate::foundation::{Disableable, Ident, StyledExt, text};
use crate::media::notice;
use crate::media::transport::{
    MediaAvailability, MediaCapabilities, MediaCommand, MediaEvent, MediaTransport,
};
use crate::media::waveform::{band, peak_band};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// How tall the frame is when there is a sentence in it instead of a player.
const NOTICE_HEIGHT: f32 = 96.0;

type EventHandler = Rc<dyn Fn(&MediaEvent, &mut Window, &mut App)>;

/// A player for one piece of audio.
#[derive(IntoElement)]
pub struct AudioPlayer {
    ident: Ident,
    title: Option<SharedString>,
    subtitle: Option<SharedString>,
    transport: Option<Rc<dyn MediaTransport>>,
    elapsed: Option<SharedString>,
    remaining: Option<SharedString>,
    peaks: Vec<f32>,
    step: Option<f32>,
    speeds: Vec<f32>,
    disabled: bool,
    on_event: Option<EventHandler>,
}

impl std::fmt::Debug for AudioPlayer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AudioPlayer")
            .field("ident", &self.ident)
            .field("title", &self.title)
            .field("transport", &self.transport)
            .field("peaks", &self.peaks.len())
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_event.is_some())
            .finish()
    }
}

impl AudioPlayer {
    /// A player with no transport, which is a player that says so.
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            title: None,
            subtitle: None,
            transport: None,
            elapsed: None,
            remaining: None,
            peaks: Vec::new(),
            step: None,
            speeds: Vec::new(),
            disabled: false,
            on_event: None,
        }
    }

    /// What is playing.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// A second line under the title: a speaker, an artist, a source.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The player behind the controls.
    ///
    /// Without one there is no playback on this machine, and this component
    /// says that rather than drawing controls that would do nothing.
    pub fn transport(mut self, transport: Rc<dyn MediaTransport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// How far in the audio is, in the host's own words.
    pub fn elapsed(mut self, elapsed: impl Into<SharedString>) -> Self {
        self.elapsed = Some(elapsed.into());
        self
    }

    /// How much is left, in the host's own words.
    pub fn remaining(mut self, remaining: impl Into<SharedString>) -> Self {
        self.remaining = Some(remaining.into());
        self
    }

    /// The peak amplitude of each slice of the audio, between zero and one,
    /// as somebody who read the samples measured it.
    ///
    /// Supplying none draws no waveform. There is no decoder here, so an
    /// envelope this component invented would be a picture of nothing.
    pub fn peaks(mut self, peaks: impl IntoIterator<Item = f32>) -> Self {
        self.peaks = peaks.into_iter().map(|peak| peak.clamp(0.0, 1.0)).collect();
        self
    }

    /// How far one arrow key jumps, in seconds.
    pub fn step_seconds(mut self, seconds: f32) -> Self {
        self.step = Some(seconds.max(0.0));
        self
    }

    /// The speeds the transport offers. Offering none leaves the control out.
    pub fn speeds(mut self, speeds: impl IntoIterator<Item = f32>) -> Self {
        self.speeds = speeds.into_iter().filter(|speed| *speed > 0.0).collect();
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(&MediaEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

impl Disableable for AudioPlayer {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for AudioPlayer {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();
        let strings = cx.strings().clone();

        let snapshot = self
            .transport
            .as_ref()
            .map(|transport| transport.snapshot());
        let capabilities = self
            .transport
            .as_ref()
            .map(|transport| transport.capabilities())
            .unwrap_or_else(MediaCapabilities::none);
        let origin = self.transport.as_ref().map(|transport| transport.origin());
        let state = match &snapshot {
            Some(snapshot) => snapshot.availability.name(),
            None => NO_TRANSPORT,
        };

        let titles = div()
            .column()
            .gap_token(&theme, Space::Xs)
            .children(
                self.title
                    .clone()
                    .map(|title| text(&theme, TypeScale::Subtitle, title)),
            )
            .children(self.subtitle.clone().map(|subtitle| {
                text(&theme, TypeScale::Caption, subtitle).text_tone(&theme, TextTone::Muted)
            }));
        // Where the facts came from is published for every transport and drawn
        // only for the one a reader could mistake for a player.
        let titles = match origin {
            Some(origin) => titles
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child("origin").semantic_id(), Role::Status)
                        .parent(ident.semantic_id())
                        .value(origin.name()),
                )
                .into_any_element(),
            None => titles.into_any_element(),
        };

        let heading = div()
            .row()
            .w_full()
            .items_start()
            .justify_between()
            .gap_token(&theme, Space::Sm)
            .child(titles)
            .children(
                origin
                    .filter(|origin| origin.is_fixture())
                    .map(|_| Badge::new(strings.text(StringKey::MediaFixture)).warning()),
            );

        let framed = |body: gpui::AnyElement| {
            div()
                .relative()
                .w_full()
                .h(px(NOTICE_HEIGHT))
                .radius(&theme, Radius::Card)
                .frame(&theme, Surface::Raised, Elevation::Raised)
                .child(body)
        };

        let actionable = !self.disabled
            && capabilities.audio
            && snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.availability.is_ready());

        let body = match (&snapshot, &self.transport) {
            (Some(snapshot), Some(transport)) if snapshot.availability.is_ready() => {
                let fraction = snapshot
                    .duration
                    .seconds()
                    .map(|total| (snapshot.position / total).clamp(0.0, 1.0));
                let mut ready = div().column().w_full().gap_token(&theme, Space::Sm);
                if !self.peaks.is_empty() {
                    ready = ready.child(
                        band(&theme)
                            .child(peak_band(
                                self.peaks.clone(),
                                fraction,
                                signature::mark(&theme),
                                theme.colors.hairline_strong,
                            ))
                            .semantic_in(
                                cx,
                                NodeSpec::new(ident.child("peaks").semantic_id(), Role::Image)
                                    .parent(ident.semantic_id())
                                    .text(strings.text(StringKey::MediaWaveform))
                                    .value(cx.numbers().count(self.peaks.len())),
                            ),
                    );
                }
                ready
                    .child(bar(
                        &ident,
                        &self,
                        snapshot,
                        capabilities,
                        Rc::clone(transport),
                        actionable,
                    ))
                    .into_any_element()
            }
            (Some(snapshot), _) => framed(unready(
                &theme,
                &strings,
                &snapshot.availability,
                self.title.clone(),
                Some(gpui_kit_assets::Icon::SoundWave),
                super::NoticePlace::Middle,
            ))
            .into_any_element(),
            (None, _) => framed(notice(
                &theme,
                theme.colors.warning,
                Some(gpui_kit_assets::Icon::SoundWave),
                self.title
                    .clone()
                    .unwrap_or_else(|| strings.text(StringKey::MediaNoTransport)),
                strings.text(StringKey::MediaNoTransportDetail),
            ))
            .into_any_element(),
        };

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Group)
            .disabled(self.disabled)
            .busy(matches!(
                snapshot.as_ref().map(|snapshot| &snapshot.availability),
                Some(MediaAvailability::Loading)
            ))
            .invalid(matches!(
                snapshot.as_ref().map(|snapshot| &snapshot.availability),
                Some(MediaAvailability::Failed(_))
            ))
            .value(state);
        if let Some(title) = self.title.clone() {
            spec = spec.text(title);
        }

        div()
            .id(ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .child(heading)
            .child(body)
            .semantic_in(cx, spec)
    }
}

/// The state name for a player nobody gave a transport.
///
/// It is distinct from `idle`: a transport holding nothing can be handed
/// something, and a player with no transport cannot play anything at all.
const NO_TRANSPORT: &str = "no-transport";

/// The transport bar, wired to ask the transport and report the answer.
fn bar(
    ident: &Ident,
    player: &AudioPlayer,
    snapshot: &crate::media::transport::MediaSnapshot,
    capabilities: MediaCapabilities,
    transport: Rc<dyn MediaTransport>,
    actionable: bool,
) -> TransportBar {
    let mut bar = TransportBar::new(ident.child("transport"))
        .state(snapshot.state)
        .position(snapshot.position)
        .volume(snapshot.volume)
        .muted(snapshot.muted)
        .buffered(snapshot.buffered.iter().copied())
        .seekable(capabilities.seek)
        .volume_control(capabilities.volume)
        .disabled(!actionable);
    bar = match snapshot.duration {
        TransportDuration::Known(seconds) => bar.duration(seconds),
        TransportDuration::Unknown => bar.unknown_duration(),
    };
    if let Some(elapsed) = player.elapsed.clone() {
        bar = bar.elapsed(elapsed);
    }
    if let Some(remaining) = player.remaining.clone() {
        bar = bar.remaining(remaining);
    }
    if let Some(step) = player.step {
        bar = bar.step_seconds(step);
    }
    if capabilities.rates && !player.speeds.is_empty() {
        bar = bar.speeds(player.speeds.iter().copied(), snapshot.speed);
    }
    if actionable {
        let handler = player.on_event.clone();
        bar = bar.on_event(move |event, window, cx| {
            let Some(command) = command_for(event) else {
                return;
            };
            let outcome = transport.apply(command);
            if let Some(handler) = &handler {
                handler(&MediaEvent::of(command, outcome), window, cx);
            }
            // The transport changed outside anything GPUI observes, so the
            // frame that would draw the new snapshot has to be asked for.
            window.refresh();
        });
    }
    bar
}

/// What a control asked the transport for, if it asked for anything.
///
/// A scrub preview is not a command: it is where the pointer is while the head
/// has not moved, and applying one would seek once per pixel.
pub(crate) fn command_for(event: &TransportEvent) -> Option<MediaCommand> {
    Some(match event {
        TransportEvent::PlayRequested => MediaCommand::Play,
        TransportEvent::PauseRequested => MediaCommand::Pause,
        TransportEvent::SeekRequested(seconds) => MediaCommand::Seek(*seconds),
        TransportEvent::VolumeRequested(volume) => MediaCommand::SetVolume(*volume),
        TransportEvent::MuteToggled => MediaCommand::ToggleMute,
        TransportEvent::SpeedRequested(speed) => MediaCommand::SetSpeed(*speed),
        TransportEvent::Stepped(step) => MediaCommand::Step(*step),
        TransportEvent::SeekPreview(_) => return None,
    })
}

/// The sentence a player draws when there is nothing to move.
pub(crate) fn unready(
    theme: &gpui_kit_theme::Theme,
    strings: &crate::strings::Strings,
    availability: &MediaAvailability,
    title: Option<SharedString>,
    mark: Option<gpui_kit_assets::Icon>,
    place: super::NoticePlace,
) -> gpui::AnyElement {
    let (tint, key) = match availability {
        MediaAvailability::Loading => (theme.colors.text_muted, StringKey::Loading),
        MediaAvailability::NoBackend(_) => (theme.colors.warning, StringKey::MediaNoBackend),
        MediaAvailability::Failed(_) => (theme.colors.danger, StringKey::MediaFailed),
        _ => (theme.colors.text_muted, StringKey::MediaEmpty),
    };
    let headline = title.unwrap_or_else(|| strings.text(key));
    // A backend's own sentence is the detail whenever it gave one, so a
    // refusal is shown as the refusal it is rather than as an absence.
    let detail = availability.reason().unwrap_or_else(|| strings.text(key));
    super::notice_at(theme, tint, mark, headline, detail, place)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::transport::TrackStep;

    #[test]
    fn every_control_but_a_scrub_preview_is_a_command() {
        assert_eq!(
            command_for(&TransportEvent::PlayRequested),
            Some(MediaCommand::Play)
        );
        assert_eq!(
            command_for(&TransportEvent::SeekRequested(12.0)),
            Some(MediaCommand::Seek(12.0))
        );
        assert_eq!(
            command_for(&TransportEvent::Stepped(TrackStep::Next)),
            Some(MediaCommand::Step(TrackStep::Next))
        );
        assert_eq!(
            command_for(&TransportEvent::SeekPreview(12.0)),
            None,
            "a preview is where the pointer is, not where the head went"
        );
    }

    #[test]
    fn a_player_with_no_transport_is_not_a_player_holding_nothing() {
        assert_ne!(NO_TRANSPORT, MediaAvailability::Idle.name());
    }
}
