//! The native player behind [`super::AudioPlayer`] and [`super::VideoPlayer`].
//!
//! [`PlatformMediaTransport`] adapts `gpui-box-media`'s AVFoundation (macOS)
//! and Media Foundation (Windows) service to [`super::MediaTransport`]. It
//! owns no product policy: a caller chooses the source, retains the transport,
//! and subscribes to native changes when its view needs invalidation.
//!
//! A video transport exposes [`frame`](PlatformMediaTransport::frame), which
//! is a GPUI platform-view element backed by the native `NSView` or child
//! `HWND`. The handle keeps the native player alive through the platform
//! host's delayed detach frame. Audio transports expose no frame.

use std::rc::Rc;

use gpui::{AnyElement, IntoElement, PlatformViewHandle, Styled, platform_view};
use media::{
    MediaAvailability as NativeAvailability, MediaCommand as NativeCommand,
    MediaCommandOutcome as NativeOutcome, MediaErrorKind as NativeErrorKind, MediaKind,
    PlaybackState,
};
pub use media::{
    MediaCapabilities as NativeMediaCapabilities, MediaError as NativeMediaError,
    MediaEvent as NativeMediaEvent, MediaPlayer as NativeMediaPlayer, MediaSource,
    MediaSubscription as NativeMediaSubscription,
};

use crate::content::transport::{BufferedRange, TransportDuration, TransportState};

use super::{
    MediaAvailability, MediaCapabilities, MediaCommand, MediaError, MediaErrorKind, MediaOrigin,
    MediaOutcome, MediaSnapshot, MediaTransport,
};

/// The AVFoundation or Media Foundation player used by media components.
pub struct PlatformMediaTransport {
    player: Rc<NativeMediaPlayer>,
    view: Option<PlatformViewHandle>,
}

impl std::fmt::Debug for PlatformMediaTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlatformMediaTransport")
            .field("kind", &self.player.kind())
            .field("snapshot", &self.player.snapshot())
            .field("has_view", &self.view.is_some())
            .finish()
    }
}

impl PlatformMediaTransport {
    /// Creates a native audio transport.
    pub fn audio() -> Rc<Self> {
        Self::new(MediaKind::Audio)
    }

    /// Creates a native video transport and its hosted platform view.
    pub fn video() -> Rc<Self> {
        Self::new(MediaKind::Video)
    }

    fn new(kind: MediaKind) -> Rc<Self> {
        let player = Rc::new(NativeMediaPlayer::new(kind));
        let view = player.native_video_view().map(|view| {
            #[cfg(target_os = "macos")]
            let handle = unsafe { PlatformViewHandle::from_ns_view(view.as_ptr()) };
            #[cfg(target_os = "windows")]
            let handle = unsafe {
                PlatformViewHandle::from_hwnd(windows::Win32::Foundation::HWND(view.as_ptr()))
            };
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let handle = {
                let _ = view;
                PlatformViewHandle::inert()
            };
            handle.keep_alive(Rc::clone(&player))
        });
        Rc::new(Self { player, view })
    }

    /// Replaces the source and begins opening it asynchronously.
    pub fn load(&self, source: impl Into<MediaSource>) -> Result<(), NativeMediaError> {
        self.player.load(source.into())
    }

    /// Registers a native change signal. The callback may arrive on a media
    /// thread; marshal it to the GPUI foreground executor and call
    /// `cx.notify()` there.
    pub fn subscribe(
        &self,
        handler: impl Fn(NativeMediaEvent) + Send + Sync + 'static,
    ) -> NativeMediaSubscription {
        self.player.subscribe(handler)
    }

    /// A full-size native frame for [`super::VideoPlayer::frame`]. Audio and
    /// unsupported transports return `None`.
    pub fn frame(&self) -> Option<AnyElement> {
        self.view
            .as_ref()
            .map(|handle| platform_view(handle.clone()).size_full().into_any_element())
    }

    /// A trait-object clone for [`super::AudioPlayer::transport`] or
    /// [`super::VideoPlayer::transport`].
    pub fn shared(self: &Rc<Self>) -> Rc<dyn MediaTransport> {
        self.clone()
    }

    /// Direct access to the product-neutral native service for capabilities
    /// outside the component transport contract.
    pub fn player(&self) -> &NativeMediaPlayer {
        &self.player
    }
}

impl MediaTransport for PlatformMediaTransport {
    fn origin(&self) -> MediaOrigin {
        MediaOrigin::Platform
    }

    fn capabilities(&self) -> MediaCapabilities {
        let native = self.player.capabilities();
        MediaCapabilities {
            audio: native.audio,
            video: native.video,
            seek: native.seek,
            volume: native.volume,
            rates: native.rates,
            native_tracks: native.native_tracks,
            output_selection: native.output_selection,
        }
    }

    fn snapshot(&self) -> MediaSnapshot {
        snapshot(self.player.snapshot())
    }

    fn apply(&self, command: MediaCommand) -> MediaOutcome {
        let native = match command {
            MediaCommand::Play => NativeCommand::Play,
            MediaCommand::Pause => NativeCommand::Pause,
            MediaCommand::Seek(seconds) => NativeCommand::Seek(seconds as f64),
            MediaCommand::SetVolume(volume) => NativeCommand::SetVolume(volume as f64),
            MediaCommand::ToggleMute => NativeCommand::SetMuted(!self.player.snapshot().muted),
            MediaCommand::SetSpeed(speed) => NativeCommand::SetRate(speed as f64),
            MediaCommand::Step(_) => return MediaOutcome::Unsupported,
        };
        match self.player.command(native) {
            NativeOutcome::Applied => MediaOutcome::Applied,
            NativeOutcome::Refused(error) => MediaOutcome::Refused(media_error(error)),
            NativeOutcome::Unsupported => MediaOutcome::Unsupported,
        }
    }
}

fn snapshot(native: media::MediaSnapshot) -> MediaSnapshot {
    MediaSnapshot {
        availability: match native.availability {
            NativeAvailability::Idle => MediaAvailability::Idle,
            NativeAvailability::Loading => MediaAvailability::Loading,
            NativeAvailability::Ready => MediaAvailability::Ready,
            NativeAvailability::NoBackend(error) => {
                MediaAvailability::NoBackend(media_error(error))
            }
            NativeAvailability::Failed(error) => MediaAvailability::Failed(media_error(error)),
        },
        state: match native.state {
            PlaybackState::Playing => TransportState::Playing,
            PlaybackState::Buffering => TransportState::Buffering,
            PlaybackState::Paused | PlaybackState::Ended => TransportState::Paused,
        },
        position: seconds(native.position),
        duration: native
            .duration
            .map(|duration| TransportDuration::Known(seconds(duration)))
            .unwrap_or(TransportDuration::Unknown),
        volume: native.volume.clamp(0.0, 1.0) as f32,
        muted: native.muted,
        speed: native.rate.max(f64::EPSILON).min(f32::MAX as f64) as f32,
        buffered: native
            .buffered
            .into_iter()
            .map(|range| BufferedRange::new(seconds(range.start), seconds(range.end)))
            .collect(),
    }
}

fn media_error(native: NativeMediaError) -> MediaError {
    let kind = match native.kind() {
        NativeErrorKind::NoBackend => MediaErrorKind::NoBackend,
        NativeErrorKind::InvalidSource => MediaErrorKind::InvalidSource,
        NativeErrorKind::Open => MediaErrorKind::Open,
        NativeErrorKind::Playback => MediaErrorKind::Playback,
    };
    MediaError::new(kind, native.message().to_owned())
}

fn seconds(value: f64) -> f32 {
    value.clamp(0.0, f32::MAX as f64) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use media::{MediaSnapshot as NativeSnapshot, TimeRange};

    #[test]
    fn native_snapshot_preserves_truthful_availability_and_buffering() {
        let mapped = snapshot(NativeSnapshot {
            availability: NativeAvailability::Ready,
            state: PlaybackState::Buffering,
            position: 12.0,
            duration: Some(60.0),
            volume: 0.5,
            muted: true,
            rate: 1.5,
            buffered: vec![TimeRange::new(0.0, 24.0)],
        });
        assert_eq!(mapped.availability, MediaAvailability::Ready);
        assert_eq!(mapped.state, TransportState::Buffering);
        assert_eq!(mapped.buffered, vec![BufferedRange::new(0.0, 24.0)]);
    }

    #[test]
    fn a_missing_native_backend_remains_no_backend() {
        let error = NativeMediaError::new(
            media::MediaErrorKind::NoBackend,
            "No native decoder is available.",
        );
        let mapped = snapshot(NativeSnapshot {
            availability: NativeAvailability::NoBackend(error),
            ..NativeSnapshot::default()
        });
        assert_eq!(mapped.availability.name(), "no-backend");
        assert_eq!(
            mapped.availability.error().map(MediaError::category),
            Some(MediaErrorKind::NoBackend)
        );
    }
}
