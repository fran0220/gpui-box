use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    ptr::NonNull,
    sync::Arc,
};

use super::{
    EventHub, MediaAvailability, MediaCommand, MediaCommandOutcome, MediaError, MediaErrorKind,
    MediaEvent, MediaKind, MediaSnapshot, MediaSource, NativeVideoView, PlaybackState, TimeRange,
};

const ERROR_BUFFER_SIZE: usize = 1024;

#[repr(C)]
#[derive(Default)]
struct RawSnapshot {
    availability: c_int,
    playback: c_int,
    position: f64,
    duration: f64,
    volume: f64,
    rate: f64,
    muted: bool,
}

type EventCallback = unsafe extern "C" fn(*mut c_void, c_int);

unsafe extern "C" fn event_callback(context: *mut c_void, event: c_int) {
    let hub = unsafe { &*(context.cast::<EventHub>()) };
    hub.emit(if event == 1 {
        MediaEvent::Ended
    } else {
        MediaEvent::Changed
    });
}

pub(super) struct Player {
    raw: NonNull<c_void>,
    event_context: *const EventHub,
}

impl Player {
    pub(super) fn new(kind: MediaKind, events: Arc<EventHub>) -> Result<Self, MediaError> {
        let event_context = Arc::into_raw(events);
        let raw = unsafe {
            gpui_media_player_create(
                matches!(kind, MediaKind::Video),
                Some(event_callback),
                event_context.cast_mut().cast(),
            )
        };
        let Some(raw) = NonNull::new(raw) else {
            unsafe { drop(Arc::from_raw(event_context)) };
            return Err(MediaError::new(
                MediaErrorKind::Open,
                "AVFoundation could not create a media player.",
            ));
        };
        Ok(Self { raw, event_context })
    }

    pub(super) fn load(&self, source: MediaSource) -> Result<(), MediaError> {
        let (source, is_file) = match source {
            MediaSource::File(path) => (
                path.into_os_string().into_string().map_err(|_| {
                    MediaError::new(
                        MediaErrorKind::InvalidSource,
                        "AVFoundation requires a UTF-8 local media path.",
                    )
                })?,
                true,
            ),
            MediaSource::Url(url) => (url, false),
        };
        let source = CString::new(source).map_err(|_| {
            MediaError::new(
                MediaErrorKind::InvalidSource,
                "The media source contains a null byte.",
            )
        })?;
        let loaded = unsafe {
            if is_file {
                gpui_media_player_load_file(self.raw.as_ptr(), source.as_ptr())
            } else {
                gpui_media_player_load_url(self.raw.as_ptr(), source.as_ptr())
            }
        };
        if loaded {
            Ok(())
        } else {
            Err(self.error(MediaErrorKind::InvalidSource))
        }
    }

    pub(super) fn command(&self, command: MediaCommand) -> MediaCommandOutcome {
        if matches!(
            command,
            MediaCommand::Play | MediaCommand::Pause | MediaCommand::Seek(_)
        ) && !self.snapshot().availability.is_ready()
        {
            return MediaCommandOutcome::Refused(MediaError::new(
                MediaErrorKind::Playback,
                "The media source is not ready for playback.",
            ));
        }
        let valid = match command {
            MediaCommand::Seek(seconds) => seconds.is_finite() && seconds >= 0.0,
            MediaCommand::SetVolume(volume) => volume.is_finite(),
            MediaCommand::SetRate(rate) => rate.is_finite() && rate > 0.0,
            MediaCommand::Play | MediaCommand::Pause | MediaCommand::SetMuted(_) => true,
        };
        if !valid {
            return MediaCommandOutcome::Refused(MediaError::new(
                MediaErrorKind::Playback,
                "The playback command contains an invalid numeric value.",
            ));
        }

        unsafe {
            match command {
                MediaCommand::Play => gpui_media_player_play(self.raw.as_ptr()),
                MediaCommand::Pause => gpui_media_player_pause(self.raw.as_ptr()),
                MediaCommand::Seek(seconds) => gpui_media_player_seek(self.raw.as_ptr(), seconds),
                MediaCommand::SetVolume(volume) => {
                    gpui_media_player_set_volume(self.raw.as_ptr(), volume.clamp(0.0, 1.0))
                }
                MediaCommand::SetMuted(muted) => {
                    gpui_media_player_set_muted(self.raw.as_ptr(), muted)
                }
                MediaCommand::SetRate(rate) => gpui_media_player_set_rate(self.raw.as_ptr(), rate),
            }
        }
        MediaCommandOutcome::Applied
    }

    pub(super) fn snapshot(&self) -> MediaSnapshot {
        let mut raw = RawSnapshot::default();
        unsafe { gpui_media_player_snapshot(self.raw.as_ptr(), &mut raw) };
        let error = || self.error(MediaErrorKind::Open);
        let availability = match raw.availability {
            0 => MediaAvailability::Idle,
            1 => MediaAvailability::Loading,
            2 => MediaAvailability::Ready,
            4 => {
                let mut error = error();
                error.kind = MediaErrorKind::NoBackend;
                MediaAvailability::NoBackend(error)
            }
            _ => MediaAvailability::Failed(error()),
        };
        let state = match raw.playback {
            1 => PlaybackState::Playing,
            2 => PlaybackState::Buffering,
            3 => PlaybackState::Ended,
            _ => PlaybackState::Paused,
        };
        let range_count = unsafe { gpui_media_player_buffered_count(self.raw.as_ptr()) };
        let mut buffered = Vec::with_capacity(range_count);
        for index in 0..range_count {
            let mut start = 0.0;
            let mut end = 0.0;
            if unsafe {
                gpui_media_player_buffered_range(self.raw.as_ptr(), index, &mut start, &mut end)
            } && start.is_finite()
                && end.is_finite()
            {
                buffered.push(TimeRange::new(start.max(0.0), end.max(0.0)));
            }
        }
        MediaSnapshot {
            availability,
            state,
            position: finite_nonnegative(raw.position).unwrap_or(0.0),
            duration: finite_nonnegative(raw.duration).filter(|duration| *duration > 0.0),
            volume: raw.volume.clamp(0.0, 1.0),
            muted: raw.muted,
            rate: finite_nonnegative(raw.rate)
                .filter(|rate| *rate > 0.0)
                .unwrap_or(1.0),
            buffered,
        }
    }

    pub(super) fn native_video_view(&self) -> Option<NativeVideoView> {
        let raw = unsafe { gpui_media_player_view(self.raw.as_ptr()) };
        unsafe { NativeVideoView::from_ptr(raw) }
    }

    fn error(&self, fallback_kind: MediaErrorKind) -> MediaError {
        let mut buffer = [0_i8; ERROR_BUFFER_SIZE];
        let kind = unsafe {
            gpui_media_player_copy_error(self.raw.as_ptr(), buffer.as_mut_ptr(), buffer.len())
        };
        let message = unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        let kind = match kind {
            1 => MediaErrorKind::NoBackend,
            2 => MediaErrorKind::InvalidSource,
            3 => MediaErrorKind::Playback,
            _ => fallback_kind,
        };
        MediaError::new(
            kind,
            if message.is_empty() {
                "AVFoundation could not open the media.".to_owned()
            } else {
                message
            },
        )
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        unsafe {
            gpui_media_player_destroy(self.raw.as_ptr());
            drop(Arc::from_raw(self.event_context));
        }
    }
}

fn finite_nonnegative(value: f64) -> Option<f64> {
    (value.is_finite() && value >= 0.0).then_some(value)
}

unsafe extern "C" {
    fn gpui_media_player_create(
        video: bool,
        callback: Option<EventCallback>,
        context: *mut c_void,
    ) -> *mut c_void;
    fn gpui_media_player_destroy(player: *mut c_void);
    fn gpui_media_player_view(player: *mut c_void) -> *mut c_void;
    fn gpui_media_player_load_file(player: *mut c_void, path: *const c_char) -> bool;
    fn gpui_media_player_load_url(player: *mut c_void, url: *const c_char) -> bool;
    fn gpui_media_player_play(player: *mut c_void);
    fn gpui_media_player_pause(player: *mut c_void);
    fn gpui_media_player_seek(player: *mut c_void, seconds: f64);
    fn gpui_media_player_set_volume(player: *mut c_void, volume: f64);
    fn gpui_media_player_set_muted(player: *mut c_void, muted: bool);
    fn gpui_media_player_set_rate(player: *mut c_void, rate: f64);
    fn gpui_media_player_snapshot(player: *mut c_void, snapshot: *mut RawSnapshot);
    fn gpui_media_player_buffered_count(player: *mut c_void) -> usize;
    fn gpui_media_player_buffered_range(
        player: *mut c_void,
        index: usize,
        start: *mut f64,
        end: *mut f64,
    ) -> bool;
    fn gpui_media_player_copy_error(
        player: *mut c_void,
        buffer: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
