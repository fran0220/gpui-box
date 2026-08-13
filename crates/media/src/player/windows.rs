use std::{
    cell::{Cell, RefCell},
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use url::Url;
use windows::{
    Win32::{
        Foundation::{HWND, RPC_E_CHANGED_MODE, S_FALSE, S_OK},
        Media::MediaFoundation::*,
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
            LibraryLoader::GetModuleHandleW,
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
            WS_EX_NOACTIVATE, WS_POPUP, WS_VISIBLE,
        },
    },
    core::{BSTR, Error as WindowsError, HRESULT, PCWSTR, implement, w},
};

use super::{
    EventHub, MediaAvailability, MediaCommand, MediaCommandOutcome, MediaError, MediaErrorKind,
    MediaEvent, MediaKind, MediaSnapshot, MediaSource, NativeVideoView, PlaybackState, TimeRange,
};

const IDLE: u8 = 0;
const LOADING: u8 = 1;
const READY: u8 = 2;
const FAILED: u8 = 3;
const NO_BACKEND: u8 = 4;

#[derive(Clone)]
struct ReportedState {
    availability: u8,
    buffering: bool,
    ended: bool,
    error: Option<MediaError>,
}

impl Default for ReportedState {
    fn default() -> Self {
        Self {
            availability: IDLE,
            buffering: false,
            ended: false,
            error: None,
        }
    }
}

#[derive(Default)]
struct CallbackState {
    next_generation: AtomicU64,
    active_generation: RwLock<u64>,
    reported: Mutex<ReportedState>,
}

impl CallbackState {
    fn begin_load(&self) -> u64 {
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let mut active = self
            .active_generation
            .write()
            .expect("media generation lock poisoned");
        let mut reported = self
            .reported
            .lock()
            .expect("media callback state mutex poisoned");
        *active = generation;
        *reported = ReportedState {
            availability: LOADING,
            ..ReportedState::default()
        };
        generation
    }

    fn fail(&self, generation: u64, kind: MediaErrorKind, message: impl Into<String>) {
        let active = self
            .active_generation
            .read()
            .expect("media generation lock poisoned");
        if *active != generation {
            return;
        }
        let mut reported = self
            .reported
            .lock()
            .expect("media callback state mutex poisoned");
        reported.availability = if kind == MediaErrorKind::NoBackend {
            NO_BACKEND
        } else {
            FAILED
        };
        reported.buffering = false;
        reported.ended = false;
        reported.error = Some(MediaError::new(kind, message));
    }

    fn reported(&self) -> ReportedState {
        self.reported
            .lock()
            .expect("media callback state mutex poisoned")
            .clone()
    }

    fn is_ready(&self) -> bool {
        self.reported
            .lock()
            .expect("media callback state mutex poisoned")
            .availability
            == READY
    }

    fn clear_ended(&self) {
        self.reported
            .lock()
            .expect("media callback state mutex poisoned")
            .ended = false;
    }

    fn invalidate(&self) {
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed) + 1;
        *self
            .active_generation
            .write()
            .expect("media generation lock poisoned") = generation;
    }

    fn transition(
        &self,
        generation: u64,
        event: u32,
        param1: usize,
        param2: u32,
    ) -> Option<MediaEvent> {
        let active = self
            .active_generation
            .read()
            .expect("media generation lock poisoned");
        if *active != generation {
            return None;
        }

        let mut reported = self
            .reported
            .lock()
            .expect("media callback state mutex poisoned");
        match event {
            value if value == MF_MEDIA_ENGINE_EVENT_LOADSTART.0 as u32 => {
                if reported.availability != LOADING {
                    return None;
                }
                *reported = ReportedState {
                    availability: LOADING,
                    ..ReportedState::default()
                };
            }
            value
                if value == MF_MEDIA_ENGINE_EVENT_LOADEDMETADATA.0 as u32
                    || value == MF_MEDIA_ENGINE_EVENT_LOADEDDATA.0 as u32
                    || value == MF_MEDIA_ENGINE_EVENT_CANPLAY.0 as u32
                    || value == MF_MEDIA_ENGINE_EVENT_CANPLAYTHROUGH.0 as u32 =>
            {
                if matches!(reported.availability, LOADING | READY) {
                    reported.availability = READY;
                } else {
                    return None;
                }
            }
            value if value == MF_MEDIA_ENGINE_EVENT_ERROR.0 as u32 => {
                if !matches!(reported.availability, LOADING | READY) {
                    return None;
                }
                let kind = if param1 as i32 == MF_MEDIA_ENGINE_ERR_SRC_NOT_SUPPORTED.0 {
                    MediaErrorKind::NoBackend
                } else {
                    MediaErrorKind::Open
                };
                let detail = if param2 == 0 {
                    format!("Media Foundation error category {param1}.")
                } else {
                    let error = WindowsError::from_hresult(HRESULT(param2 as i32));
                    format!(
                        "Media Foundation could not play the source: {}",
                        error.message()
                    )
                };
                reported.availability = if kind == MediaErrorKind::NoBackend {
                    NO_BACKEND
                } else {
                    FAILED
                };
                reported.buffering = false;
                reported.ended = false;
                reported.error = Some(MediaError::new(kind, detail));
            }
            value
                if value == MF_MEDIA_ENGINE_EVENT_WAITING.0 as u32
                    || value == MF_MEDIA_ENGINE_EVENT_STALLED.0 as u32
                    || value == MF_MEDIA_ENGINE_EVENT_BUFFERINGSTARTED.0 as u32 =>
            {
                if reported.availability == READY {
                    reported.buffering = true;
                } else {
                    return None;
                }
            }
            value
                if value == MF_MEDIA_ENGINE_EVENT_PLAYING.0 as u32
                    || value == MF_MEDIA_ENGINE_EVENT_BUFFERINGENDED.0 as u32 =>
            {
                if reported.availability == READY {
                    reported.buffering = false;
                    reported.ended = false;
                } else {
                    return None;
                }
            }
            value if value == MF_MEDIA_ENGINE_EVENT_ENDED.0 as u32 => {
                if reported.availability == READY {
                    reported.buffering = false;
                    reported.ended = true;
                } else {
                    return None;
                }
            }
            value
                if value == MF_MEDIA_ENGINE_EVENT_EMPTIED.0 as u32
                    && reported.availability == READY =>
            {
                reported.availability = FAILED;
                reported.buffering = false;
                reported.ended = false;
                reported.error = Some(MediaError::new(
                    MediaErrorKind::Open,
                    "Media Foundation emptied the active media source.",
                ));
            }
            _ => return None,
        }
        Some(
            if event == MF_MEDIA_ENGINE_EVENT_ENDED.0 as u32 && reported.ended {
                MediaEvent::Ended
            } else {
                MediaEvent::Changed
            },
        )
    }
}

#[derive(Clone, Copy)]
struct PlaybackSettings {
    volume: f64,
    muted: bool,
    rate: f64,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            muted: false,
            rate: 1.0,
        }
    }
}

struct Engine {
    engine: IMFMediaEngine,
    _notify: IMFMediaEngineNotify,
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = unsafe { self.engine.Shutdown() };
    }
}

#[implement(IMFMediaEngineNotify)]
struct MediaEngineNotify {
    generation: u64,
    state: Arc<CallbackState>,
    events: Arc<EventHub>,
}

#[allow(non_snake_case)]
impl IMFMediaEngineNotify_Impl for MediaEngineNotify_Impl {
    fn EventNotify(&self, event: u32, param1: usize, param2: u32) -> windows::core::Result<()> {
        let Some(event) = self
            .state
            .transition(self.generation, event, param1, param2)
        else {
            return Ok(());
        };
        self.events.emit(event);
        Ok(())
    }
}

struct ComApartment {
    uninitialize: bool,
}

impl ComApartment {
    fn initialize() -> Result<Self, MediaError> {
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if result == S_OK || result == S_FALSE {
            Ok(Self { uninitialize: true })
        } else if result == RPC_E_CHANGED_MODE {
            // COM is already initialized in another apartment mode. Calls are
            // still legal, but we must not balance initialization we did not do.
            Ok(Self {
                uninitialize: false,
            })
        } else {
            Err(MediaError::new(
                MediaErrorKind::Open,
                format!(
                    "COM could not initialize native media playback: {}",
                    WindowsError::from_hresult(result).message()
                ),
            ))
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.uninitialize {
            unsafe { CoUninitialize() };
        }
    }
}

static MEDIA_FOUNDATION_USERS: Mutex<usize> = Mutex::new(0);

struct MediaFoundation;

impl MediaFoundation {
    fn acquire() -> Result<Self, MediaError> {
        let mut users = MEDIA_FOUNDATION_USERS
            .lock()
            .expect("Media Foundation reference mutex poisoned");
        if *users == 0 {
            unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.map_err(|error| {
                MediaError::new(
                    MediaErrorKind::Open,
                    format!("Media Foundation could not start: {}", error.message()),
                )
            })?;
        }
        *users += 1;
        Ok(Self)
    }
}

impl Drop for MediaFoundation {
    fn drop(&mut self) {
        let mut users = MEDIA_FOUNDATION_USERS
            .lock()
            .expect("Media Foundation reference mutex poisoned");
        *users -= 1;
        if *users == 0 {
            let _ = unsafe { MFShutdown() };
        }
    }
}

struct VideoWindows {
    parking: HWND,
    view: HWND,
}

impl VideoWindows {
    fn new() -> Result<Self, MediaError> {
        let instance = unsafe { GetModuleHandleW(PCWSTR::null()) }.map_err(open_error)?;
        let parking = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!(""),
                WS_POPUP | WS_CLIPCHILDREN,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(instance.into()),
                None,
            )
        }
        .map_err(open_error)?;
        let view = match unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                0,
                0,
                1,
                1,
                Some(parking),
                None,
                Some(instance.into()),
                None,
            )
        } {
            Ok(view) => view,
            Err(error) => {
                unsafe { DestroyWindow(parking) }.ok();
                return Err(open_error(error));
            }
        };
        Ok(Self { parking, view })
    }
}

impl Drop for VideoWindows {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.view).ok();
            DestroyWindow(self.parking).ok();
        }
    }
}

pub(super) struct Player {
    engine: RefCell<Option<Engine>>,
    factory: IMFMediaEngineClassFactory,
    kind: MediaKind,
    events: Arc<EventHub>,
    state: Arc<CallbackState>,
    settings: Cell<PlaybackSettings>,
    video_windows: Option<VideoWindows>,
    _media_foundation: MediaFoundation,
    _com: ComApartment,
}

impl Player {
    pub(super) fn new(kind: MediaKind, events: Arc<EventHub>) -> Result<Self, MediaError> {
        let com = ComApartment::initialize()?;
        let media_foundation = MediaFoundation::acquire()?;
        let state = Arc::new(CallbackState::default());
        let video_windows = matches!(kind, MediaKind::Video)
            .then(VideoWindows::new)
            .transpose()?;
        let factory: IMFMediaEngineClassFactory = unsafe {
            CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_INPROC_SERVER)
        }
        .map_err(open_error)?;

        Ok(Self {
            engine: RefCell::new(None),
            factory,
            kind,
            events,
            state,
            settings: Cell::new(PlaybackSettings::default()),
            video_windows,
            _media_foundation: media_foundation,
            _com: com,
        })
    }

    pub(super) fn load(&self, source: MediaSource) -> Result<(), MediaError> {
        let generation = self.state.begin_load();
        drop(self.engine.borrow_mut().take());
        let source = source_url(source).inspect_err(|error| {
            self.state.fail(generation, error.kind(), error.message());
            self.events.emit(MediaEvent::Changed);
        })?;
        let engine = self.create_engine(generation).and_then(|engine| {
            let result: windows::core::Result<()> = (|| unsafe {
                engine.engine.SetSource(&BSTR::from(source.as_str()))?;
                engine.engine.Load()
            })();
            result.map(|()| engine).map_err(open_error)
        });
        match engine {
            Ok(engine) => {
                *self.engine.borrow_mut() = Some(engine);
                Ok(())
            }
            Err(error) => {
                self.state.fail(generation, error.kind(), error.message());
                self.events.emit(MediaEvent::Changed);
                Err(error)
            }
        }
    }

    pub(super) fn command(&self, command: MediaCommand) -> MediaCommandOutcome {
        if matches!(
            command,
            MediaCommand::Play | MediaCommand::Pause | MediaCommand::Seek(_)
        ) && !self.state.is_ready()
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

        let mut settings = self.settings.get();
        let engine = self.engine.borrow();
        let Some(engine) = engine.as_ref() else {
            match command {
                MediaCommand::SetVolume(volume) => settings.volume = volume.clamp(0.0, 1.0),
                MediaCommand::SetMuted(muted) => settings.muted = muted,
                MediaCommand::SetRate(rate) => settings.rate = rate,
                MediaCommand::Play | MediaCommand::Pause | MediaCommand::Seek(_) => {
                    return MediaCommandOutcome::Refused(MediaError::new(
                        MediaErrorKind::Playback,
                        "There is no active media source.",
                    ));
                }
            }
            self.settings.set(settings);
            return MediaCommandOutcome::Applied;
        };

        if let MediaCommand::Seek(seconds) = command {
            let duration = unsafe { engine.engine.GetDuration() };
            if !duration.is_finite() || duration <= 0.0 || seconds > duration {
                return MediaCommandOutcome::Refused(MediaError::new(
                    MediaErrorKind::Playback,
                    "The active media source does not have a seekable duration.",
                ));
            }
        }
        if matches!(command, MediaCommand::Play) && unsafe { engine.engine.GetDuration() } == 0.0 {
            return MediaCommandOutcome::Refused(MediaError::new(
                MediaErrorKind::Playback,
                "The media source has no playable duration.",
            ));
        }

        let result: windows::core::Result<()> = (|| unsafe {
            match command {
                MediaCommand::Play => {
                    if self.state.reported().ended {
                        engine.engine.SetCurrentTime(0.0)?;
                        self.state.clear_ended();
                    }
                    engine.engine.SetPlaybackRate(settings.rate)?;
                    engine.engine.Play()
                }
                MediaCommand::Pause => engine.engine.Pause(),
                MediaCommand::Seek(seconds) => {
                    engine.engine.SetCurrentTime(seconds)?;
                    self.state.clear_ended();
                    Ok(())
                }
                MediaCommand::SetVolume(volume) => {
                    let volume = volume.clamp(0.0, 1.0);
                    engine.engine.SetVolume(volume)?;
                    settings.volume = volume;
                    Ok(())
                }
                MediaCommand::SetMuted(muted) => {
                    engine.engine.SetMuted(muted)?;
                    settings.muted = muted;
                    Ok(())
                }
                MediaCommand::SetRate(rate) => {
                    engine.engine.SetDefaultPlaybackRate(rate)?;
                    if let Err(error) = engine.engine.SetPlaybackRate(rate) {
                        let _ = engine.engine.SetDefaultPlaybackRate(settings.rate);
                        return Err(error);
                    }
                    settings.rate = rate;
                    Ok(())
                }
            }
        })();
        match result {
            Ok(()) => {
                self.settings.set(settings);
                MediaCommandOutcome::Applied
            }
            Err(error) => MediaCommandOutcome::Refused(playback_error(error)),
        }
    }

    pub(super) fn snapshot(&self) -> MediaSnapshot {
        let reported = self.state.reported();
        let engine = self.engine.borrow();
        let native = engine.as_ref().map(|engine| &engine.engine);
        let raw_duration = native.map(|engine| unsafe { engine.GetDuration() });
        let zero_duration = reported.availability == READY && raw_duration == Some(0.0);
        let availability = match reported.availability {
            IDLE => MediaAvailability::Idle,
            LOADING => MediaAvailability::Loading,
            READY if zero_duration => MediaAvailability::Failed(MediaError::new(
                MediaErrorKind::Open,
                "The media source has no playable duration.",
            )),
            READY => MediaAvailability::Ready,
            NO_BACKEND => MediaAvailability::NoBackend(current_error(&reported)),
            _ => MediaAvailability::Failed(current_error(&reported)),
        };
        let settings = self.settings.get();
        let state = if reported.ended {
            PlaybackState::Ended
        } else if reported.buffering {
            PlaybackState::Buffering
        } else if reported.availability == READY
            && native.is_some_and(|engine| unsafe { !engine.IsPaused().as_bool() })
        {
            PlaybackState::Playing
        } else {
            PlaybackState::Paused
        };
        let duration = raw_duration.and_then(finite_positive);
        MediaSnapshot {
            availability,
            state,
            position: native
                .and_then(|engine| finite_nonnegative(unsafe { engine.GetCurrentTime() }))
                .unwrap_or(0.0),
            duration,
            volume: native
                .map(|engine| unsafe { engine.GetVolume() }.clamp(0.0, 1.0))
                .unwrap_or(settings.volume),
            muted: native
                .map(|engine| unsafe { engine.GetMuted().as_bool() })
                .unwrap_or(settings.muted),
            rate: if state == PlaybackState::Playing {
                native
                    .and_then(|engine| finite_positive(unsafe { engine.GetPlaybackRate() }))
                    .unwrap_or(settings.rate)
            } else {
                settings.rate
            },
            buffered: native.map(buffered).unwrap_or_default(),
        }
    }

    pub(super) fn native_video_view(&self) -> Option<NativeVideoView> {
        let view = self.video_windows.as_ref()?.view;
        unsafe { NativeVideoView::from_ptr(view.0.cast()) }
    }

    fn create_engine(&self, generation: u64) -> Result<Engine, MediaError> {
        let notify: IMFMediaEngineNotify = MediaEngineNotify {
            generation,
            state: Arc::clone(&self.state),
            events: Arc::clone(&self.events),
        }
        .into();
        let mut attributes = None;
        unsafe { MFCreateAttributes(&mut attributes, 2) }.map_err(open_error)?;
        let attributes = attributes.expect("MFCreateAttributes returned no attribute store");
        unsafe { attributes.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &notify) }.map_err(open_error)?;
        if let Some(windows) = &self.video_windows {
            unsafe {
                attributes.SetUINT64(
                    &MF_MEDIA_ENGINE_PLAYBACK_HWND,
                    windows.view.0 as usize as u64,
                )
            }
            .map_err(open_error)?;
        }
        let flags = if matches!(self.kind, MediaKind::Audio) {
            MF_MEDIA_ENGINE_AUDIOONLY.0 as u32
        } else {
            0
        };
        let engine =
            unsafe { self.factory.CreateInstance(flags, &attributes) }.map_err(open_error)?;
        unsafe { engine.SetPreload(MF_MEDIA_ENGINE_PRELOAD_AUTOMATIC) }.map_err(open_error)?;
        let settings = self.settings.get();
        let configure: windows::core::Result<()> = (|| unsafe {
            engine.SetVolume(settings.volume)?;
            engine.SetMuted(settings.muted)?;
            engine.SetDefaultPlaybackRate(settings.rate)
        })();
        configure.map_err(open_error)?;
        Ok(Engine {
            engine,
            _notify: notify,
        })
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.state.invalidate();
        drop(self.engine.get_mut().take());
    }
}

fn current_error(reported: &ReportedState) -> MediaError {
    reported.error.clone().unwrap_or_else(|| {
        MediaError::new(MediaErrorKind::Open, "Media Foundation playback failed.")
    })
}

fn buffered(engine: &IMFMediaEngine) -> Vec<TimeRange> {
    let Ok(ranges) = (unsafe { engine.GetBuffered() }) else {
        return Vec::new();
    };
    let mut buffered = Vec::with_capacity(unsafe { ranges.GetLength() } as usize);
    for index in 0..unsafe { ranges.GetLength() } {
        let (Ok(start), Ok(end)) = (unsafe { ranges.GetStart(index) }, unsafe {
            ranges.GetEnd(index)
        }) else {
            continue;
        };
        if start.is_finite() && end.is_finite() {
            buffered.push(TimeRange::new(start.max(0.0), end.max(0.0)));
        }
    }
    buffered
}

fn source_url(source: MediaSource) -> Result<Url, MediaError> {
    match source {
        MediaSource::File(path) => Url::from_file_path(path).map_err(|()| {
            MediaError::new(
                MediaErrorKind::InvalidSource,
                "The local media path cannot be represented as a file URL.",
            )
        }),
        MediaSource::Url(url) => Url::parse(&url).map_err(|error| {
            MediaError::new(
                MediaErrorKind::InvalidSource,
                format!("The media URL is invalid: {error}"),
            )
        }),
    }
}

fn finite_nonnegative(value: f64) -> Option<f64> {
    (value.is_finite() && value >= 0.0).then_some(value)
}

fn finite_positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

fn open_error(error: WindowsError) -> MediaError {
    MediaError::new(
        MediaErrorKind::Open,
        format!(
            "Media Foundation could not initialize playback: {}",
            error.message()
        ),
    )
}

fn playback_error(error: WindowsError) -> MediaError {
    MediaError::new(
        MediaErrorKind::Playback,
        format!(
            "Media Foundation refused the playback command: {}",
            error.message()
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_generations_isolate_stale_callbacks() {
        let state = CallbackState::default();
        let first = state.begin_load();
        assert_eq!(
            state.transition(first, MF_MEDIA_ENGINE_EVENT_CANPLAY.0 as u32, 0, 0),
            Some(MediaEvent::Changed)
        );
        let second = state.begin_load();
        assert_eq!(
            state.transition(first, MF_MEDIA_ENGINE_EVENT_ENDED.0 as u32, 0, 0),
            None
        );
        assert_eq!(state.reported().availability, LOADING);
        assert!(!state.reported().ended);
        assert_eq!(
            state.transition(second, MF_MEDIA_ENGINE_EVENT_CANPLAY.0 as u32, 0, 0),
            Some(MediaEvent::Changed)
        );
        assert_eq!(state.reported().availability, READY);
    }

    #[test]
    fn terminal_events_cannot_erase_a_failure() {
        let state = CallbackState::default();
        let generation = state.begin_load();
        state.fail(generation, MediaErrorKind::Open, "decoder failed");
        assert_eq!(
            state.transition(generation, MF_MEDIA_ENGINE_EVENT_EMPTIED.0 as u32, 0, 0),
            None
        );
        let reported = state.reported();
        assert_eq!(reported.availability, FAILED);
        assert_eq!(
            reported.error.expect("failure remains").message(),
            "decoder failed"
        );
    }

    #[test]
    fn empty_active_source_becomes_a_failure() {
        let state = CallbackState::default();
        let generation = state.begin_load();
        state.transition(generation, MF_MEDIA_ENGINE_EVENT_CANPLAY.0 as u32, 0, 0);
        state.transition(generation, MF_MEDIA_ENGINE_EVENT_EMPTIED.0 as u32, 0, 0);
        let reported = state.reported();
        assert_eq!(reported.availability, FAILED);
        assert!(
            reported
                .error
                .expect("empty active source has an error")
                .message()
                .contains("emptied")
        );
    }

    #[test]
    fn invalid_durations_are_not_exposed_as_seekable() {
        assert_eq!(finite_positive(0.0), None);
        assert_eq!(finite_positive(f64::NAN), None);
        assert_eq!(finite_positive(f64::INFINITY), None);
        assert_eq!(finite_positive(3.5), Some(3.5));
    }
}
