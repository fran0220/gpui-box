use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU8, Ordering},
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

#[derive(Default)]
struct CallbackState {
    availability: AtomicU8,
    buffering: AtomicBool,
    ended: AtomicBool,
    error: Mutex<Option<MediaError>>,
}

impl CallbackState {
    fn set_error(&self, kind: MediaErrorKind, message: impl Into<String>) {
        self.availability.store(
            if kind == MediaErrorKind::NoBackend {
                NO_BACKEND
            } else {
                FAILED
            },
            Ordering::Release,
        );
        *self.error.lock().expect("media error mutex poisoned") =
            Some(MediaError::new(kind, message));
    }
}

#[implement(IMFMediaEngineNotify)]
struct MediaEngineNotify {
    state: Arc<CallbackState>,
    events: Arc<EventHub>,
}

#[allow(non_snake_case)]
impl IMFMediaEngineNotify_Impl for MediaEngineNotify_Impl {
    fn EventNotify(&self, event: u32, param1: usize, param2: u32) -> windows::core::Result<()> {
        match event {
            value if value == MF_MEDIA_ENGINE_EVENT_LOADSTART.0 as u32 => {
                self.state.availability.store(LOADING, Ordering::Release);
                self.state.ended.store(false, Ordering::Release);
            }
            value
                if value == MF_MEDIA_ENGINE_EVENT_LOADEDMETADATA.0 as u32
                    || value == MF_MEDIA_ENGINE_EVENT_LOADEDDATA.0 as u32
                    || value == MF_MEDIA_ENGINE_EVENT_CANPLAY.0 as u32
                    || value == MF_MEDIA_ENGINE_EVENT_CANPLAYTHROUGH.0 as u32 =>
            {
                self.state.availability.store(READY, Ordering::Release);
            }
            value if value == MF_MEDIA_ENGINE_EVENT_ERROR.0 as u32 => {
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
                self.state.set_error(kind, detail);
            }
            value
                if value == MF_MEDIA_ENGINE_EVENT_WAITING.0 as u32
                    || value == MF_MEDIA_ENGINE_EVENT_STALLED.0 as u32
                    || value == MF_MEDIA_ENGINE_EVENT_BUFFERINGSTARTED.0 as u32 =>
            {
                self.state.buffering.store(true, Ordering::Release);
            }
            value
                if value == MF_MEDIA_ENGINE_EVENT_PLAYING.0 as u32
                    || value == MF_MEDIA_ENGINE_EVENT_BUFFERINGENDED.0 as u32 =>
            {
                self.state.buffering.store(false, Ordering::Release);
                self.state.ended.store(false, Ordering::Release);
            }
            value if value == MF_MEDIA_ENGINE_EVENT_ENDED.0 as u32 => {
                self.state.ended.store(true, Ordering::Release);
                self.events.emit(MediaEvent::Ended);
                return Ok(());
            }
            value if value == MF_MEDIA_ENGINE_EVENT_EMPTIED.0 as u32 => {
                self.state.availability.store(IDLE, Ordering::Release);
                self.state.buffering.store(false, Ordering::Release);
                self.state.ended.store(false, Ordering::Release);
            }
            _ => {}
        }
        self.events.emit(MediaEvent::Changed);
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
    engine: IMFMediaEngine,
    _notify: IMFMediaEngineNotify,
    state: Arc<CallbackState>,
    video_windows: Option<VideoWindows>,
    _media_foundation: MediaFoundation,
    _com: ComApartment,
}

impl Player {
    pub(super) fn new(kind: MediaKind, events: Arc<EventHub>) -> Result<Self, MediaError> {
        let com = ComApartment::initialize()?;
        let media_foundation = MediaFoundation::acquire()?;
        let state = Arc::new(CallbackState::default());
        let notify: IMFMediaEngineNotify = MediaEngineNotify {
            state: Arc::clone(&state),
            events,
        }
        .into();
        let video_windows = matches!(kind, MediaKind::Video)
            .then(VideoWindows::new)
            .transpose()?;

        let mut attributes = None;
        unsafe { MFCreateAttributes(&mut attributes, 2) }.map_err(open_error)?;
        let attributes = attributes.expect("MFCreateAttributes returned no attribute store");
        unsafe { attributes.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &notify) }.map_err(open_error)?;
        if let Some(windows) = &video_windows {
            unsafe {
                attributes.SetUINT64(
                    &MF_MEDIA_ENGINE_PLAYBACK_HWND,
                    windows.view.0 as usize as u64,
                )
            }
            .map_err(open_error)?;
        }

        let factory: IMFMediaEngineClassFactory = unsafe {
            CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_INPROC_SERVER)
        }
        .map_err(open_error)?;
        let flags = if matches!(kind, MediaKind::Audio) {
            MF_MEDIA_ENGINE_AUDIOONLY.0 as u32
        } else {
            0
        };
        let engine = unsafe { factory.CreateInstance(flags, &attributes) }.map_err(open_error)?;
        unsafe { engine.SetPreload(MF_MEDIA_ENGINE_PRELOAD_AUTOMATIC) }.map_err(open_error)?;

        Ok(Self {
            engine,
            _notify: notify,
            state,
            video_windows,
            _media_foundation: media_foundation,
            _com: com,
        })
    }

    pub(super) fn load(&self, source: MediaSource) -> Result<(), MediaError> {
        let source = source_url(source)?;
        self.state.availability.store(LOADING, Ordering::Release);
        self.state.buffering.store(false, Ordering::Release);
        self.state.ended.store(false, Ordering::Release);
        *self.state.error.lock().expect("media error mutex poisoned") = None;
        let result: windows::core::Result<()> = (|| unsafe {
            self.engine.SetSource(&BSTR::from(source.as_str()))?;
            self.engine.Load()
        })();
        if let Err(error) = result {
            let error = open_error(error);
            self.state.set_error(error.kind(), error.message());
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn command(&self, command: MediaCommand) -> MediaCommandOutcome {
        if matches!(
            command,
            MediaCommand::Play | MediaCommand::Pause | MediaCommand::Seek(_)
        ) && self.state.availability.load(Ordering::Acquire) != READY
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
        let result: windows::core::Result<()> = (|| unsafe {
            match command {
                MediaCommand::Play => {
                    if self.state.ended.swap(false, Ordering::AcqRel) {
                        self.engine.SetCurrentTime(0.0)?;
                    }
                    self.engine.Play()
                }
                MediaCommand::Pause => self.engine.Pause(),
                MediaCommand::Seek(seconds) => {
                    self.state.ended.store(false, Ordering::Release);
                    self.engine.SetCurrentTime(seconds)
                }
                MediaCommand::SetVolume(volume) => self.engine.SetVolume(volume.clamp(0.0, 1.0)),
                MediaCommand::SetMuted(muted) => self.engine.SetMuted(muted),
                MediaCommand::SetRate(rate) => {
                    self.engine.SetDefaultPlaybackRate(rate)?;
                    self.engine.SetPlaybackRate(rate)
                }
            }
        })();
        match result {
            Ok(()) => MediaCommandOutcome::Applied,
            Err(error) => MediaCommandOutcome::Refused(playback_error(error)),
        }
    }

    pub(super) fn snapshot(&self) -> MediaSnapshot {
        let availability = match self.state.availability.load(Ordering::Acquire) {
            IDLE => MediaAvailability::Idle,
            LOADING => MediaAvailability::Loading,
            READY => MediaAvailability::Ready,
            NO_BACKEND => MediaAvailability::NoBackend(self.current_error()),
            _ => MediaAvailability::Failed(self.current_error()),
        };
        let state = if self.state.ended.load(Ordering::Acquire) {
            PlaybackState::Ended
        } else if self.state.buffering.load(Ordering::Acquire) {
            PlaybackState::Buffering
        } else if unsafe { self.engine.IsPaused().as_bool() } {
            PlaybackState::Paused
        } else {
            PlaybackState::Playing
        };
        let duration = unsafe { self.engine.GetDuration() };
        MediaSnapshot {
            availability,
            state,
            position: finite_nonnegative(unsafe { self.engine.GetCurrentTime() }).unwrap_or(0.0),
            duration: finite_nonnegative(duration).filter(|duration| *duration > 0.0),
            volume: unsafe { self.engine.GetVolume() }.clamp(0.0, 1.0),
            muted: unsafe { self.engine.GetMuted().as_bool() },
            rate: finite_nonnegative(unsafe { self.engine.GetPlaybackRate() })
                .filter(|rate| *rate > 0.0)
                .unwrap_or(1.0),
            buffered: self.buffered(),
        }
    }

    pub(super) fn native_video_view(&self) -> Option<NativeVideoView> {
        let view = self.video_windows.as_ref()?.view;
        unsafe { NativeVideoView::from_ptr(view.0.cast()) }
    }

    fn current_error(&self) -> MediaError {
        self.state
            .error
            .lock()
            .expect("media error mutex poisoned")
            .clone()
            .unwrap_or_else(|| {
                MediaError::new(MediaErrorKind::Open, "Media Foundation playback failed.")
            })
    }

    fn buffered(&self) -> Vec<TimeRange> {
        let Ok(ranges) = (unsafe { self.engine.GetBuffered() }) else {
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
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = unsafe { self.engine.Shutdown() };
    }
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
