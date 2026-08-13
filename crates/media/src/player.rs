//! Native audio and video playback on the desktop platforms that provide it.
//!
//! [`MediaPlayer`] is the platform boundary: it owns the decoder, output
//! device, clock, and (for video) native view. It deliberately knows nothing
//! about GPUI or any product model. A host loads a [`MediaSource`], applies
//! commands, reads a non-blocking [`MediaSnapshot`], and subscribes to
//! [`MediaEvent`] only to learn when another snapshot should be read.
//!
//! AVFoundation backs macOS and Media Foundation backs Windows. Other targets
//! construct the same type in an explicit [`MediaAvailability::NoBackend`]
//! state, so shared code never has to pretend that idle media and unavailable
//! playback are the same condition.
//!
//! # Threading
//!
//! Create, use, and drop a player on the UI thread. Its native view belongs to
//! that thread. Event subscribers may be invoked by a platform media callback
//! thread; handlers must be short, must not call back into the player, and
//! must marshal UI work to their application's UI executor.

use std::{
    collections::BTreeMap,
    ffi::c_void,
    fmt,
    marker::PhantomData,
    path::PathBuf,
    ptr::NonNull,
    rc::Rc,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

#[cfg(all(feature = "native-playback", target_os = "macos"))]
mod macos;
mod unsupported;
#[cfg(all(feature = "native-playback", target_os = "windows"))]
mod windows;

/// Whether a player should open audio only or expose a native video view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Audio,
    Video,
}

/// A local file or an operating-system-supported media URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaSource {
    File(PathBuf),
    Url(String),
}

impl MediaSource {
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::File(path.into())
    }

    pub fn url(url: impl Into<String>) -> Self {
        Self::Url(url.into())
    }
}

impl From<PathBuf> for MediaSource {
    fn from(path: PathBuf) -> Self {
        Self::File(path)
    }
}

impl From<&std::path::Path> for MediaSource {
    fn from(path: &std::path::Path) -> Self {
        Self::File(path.to_path_buf())
    }
}

/// The broad repair implied by a native media error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaErrorKind {
    /// This target or its installed codecs cannot play the source.
    NoBackend,
    /// The source cannot be represented as a local file or media URL.
    InvalidSource,
    /// The native service could not open or initialize the source.
    Open,
    /// Playback was already initialized and an operation failed.
    Playback,
}

/// A stable category and the native service's diagnostic sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaError {
    kind: MediaErrorKind,
    message: String,
}

impl MediaError {
    pub fn new(kind: MediaErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> MediaErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for MediaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for MediaError {}

/// Whether a source is present and usable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaAvailability {
    Idle,
    Loading,
    Ready,
    NoBackend(MediaError),
    Failed(MediaError),
}

impl MediaAvailability {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn error(&self) -> Option<&MediaError> {
        match self {
            Self::NoBackend(error) | Self::Failed(error) => Some(error),
            Self::Idle | Self::Loading | Self::Ready => None,
        }
    }
}

/// What the native clock is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    Playing,
    Buffering,
    Ended,
    #[default]
    Paused,
}

/// One buffered interval in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRange {
    pub start: f64,
    pub end: f64,
}

impl TimeRange {
    pub fn new(start: f64, end: f64) -> Self {
        if start <= end {
            Self { start, end }
        } else {
            Self {
                start: end,
                end: start,
            }
        }
    }
}

/// A cheap, current view of native playback state.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaSnapshot {
    pub availability: MediaAvailability,
    pub state: PlaybackState,
    pub position: f64,
    /// `None` is a live stream or media whose duration is not known yet.
    pub duration: Option<f64>,
    pub volume: f64,
    pub muted: bool,
    pub rate: f64,
    pub buffered: Vec<TimeRange>,
}

impl MediaSnapshot {
    fn unavailable(availability: MediaAvailability) -> Self {
        Self {
            availability,
            state: PlaybackState::Paused,
            position: 0.0,
            duration: None,
            volume: 1.0,
            muted: false,
            rate: 1.0,
            buffered: Vec::new(),
        }
    }
}

impl Default for MediaSnapshot {
    fn default() -> Self {
        Self::unavailable(MediaAvailability::Idle)
    }
}

/// A change requested of an already-created player.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaCommand {
    Play,
    Pause,
    Seek(f64),
    SetVolume(f64),
    SetMuted(bool),
    SetRate(f64),
}

/// The synchronous part of applying a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaCommandOutcome {
    Applied,
    Refused(MediaError),
    Unsupported,
}

/// A signal that a host should read another snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaEvent {
    Changed,
    Ended,
}

type EventHandler = Arc<dyn Fn(MediaEvent) + Send + Sync + 'static>;

#[derive(Default)]
struct EventHub {
    next_id: AtomicU64,
    handlers: Mutex<BTreeMap<u64, EventHandler>>,
}

impl EventHub {
    fn subscribe(self: &Arc<Self>, handler: EventHandler) -> MediaSubscription {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.handlers
            .lock()
            .expect("media event subscriber mutex poisoned")
            .insert(id, handler);
        MediaSubscription {
            hub: Arc::downgrade(self),
            id,
        }
    }

    #[cfg(any(
        test,
        all(
            feature = "native-playback",
            any(target_os = "macos", target_os = "windows")
        )
    ))]
    fn emit(&self, event: MediaEvent) {
        let handlers = self
            .handlers
            .lock()
            .expect("media event subscriber mutex poisoned")
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for handler in handlers {
            handler(event);
        }
    }
}

/// Removes one media event handler when dropped.
pub struct MediaSubscription {
    hub: Weak<EventHub>,
    id: u64,
}

impl fmt::Debug for MediaSubscription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MediaSubscription")
            .field("id", &self.id)
            .finish()
    }
}

impl Drop for MediaSubscription {
    fn drop(&mut self) {
        if let Some(hub) = self.hub.upgrade() {
            hub.handlers
                .lock()
                .expect("media event subscriber mutex poisoned")
                .remove(&self.id);
        }
    }
}

/// An opaque native video view borrowed from a [`MediaPlayer`].
///
/// On macOS this is an `NSView`; on Windows it is a child `HWND`. The player
/// remains its owner. A UI integration must retain the player until its host
/// has detached the view.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NativeVideoView {
    raw: NonNull<c_void>,
    _main_thread: PhantomData<Rc<()>>,
}

impl NativeVideoView {
    #[cfg(all(
        feature = "native-playback",
        any(target_os = "macos", target_os = "windows")
    ))]
    unsafe fn from_ptr(raw: *mut c_void) -> Option<Self> {
        NonNull::new(raw).map(|raw| Self {
            raw,
            _main_thread: PhantomData,
        })
    }

    pub fn as_ptr(self) -> *mut c_void {
        self.raw.as_ptr()
    }
}

impl fmt::Debug for NativeVideoView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("NativeVideoView")
            .field(&self.raw)
            .finish()
    }
}

enum Backend {
    #[cfg(all(
        feature = "native-playback",
        any(target_os = "macos", target_os = "windows")
    ))]
    Native(platform::Player),
    Unavailable(unsupported::Player),
}

#[cfg(all(feature = "native-playback", target_os = "macos"))]
use macos as platform;
#[cfg(all(feature = "native-playback", target_os = "windows"))]
use windows as platform;

/// One native audio or video player.
///
/// The service is intentionally `!Send`: the native view and the playback
/// controller must remain on their creating UI thread. Its event callbacks are
/// separately thread-safe and may be delivered from native media threads.
pub struct MediaPlayer {
    kind: MediaKind,
    backend: Backend,
    events: Arc<EventHub>,
    _main_thread: PhantomData<Rc<()>>,
}

impl fmt::Debug for MediaPlayer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MediaPlayer")
            .field("kind", &self.kind)
            .field("snapshot", &self.snapshot())
            .finish()
    }
}

impl MediaPlayer {
    /// Creates the platform player. Initialization failures remain observable
    /// through [`snapshot`](Self::snapshot), just like asynchronous open
    /// failures, so a UI always has a truthful state to render.
    pub fn new(kind: MediaKind) -> Self {
        let events = Arc::new(EventHub::default());
        #[cfg(all(
            feature = "native-playback",
            any(target_os = "macos", target_os = "windows")
        ))]
        let backend = match platform::Player::new(kind, Arc::clone(&events)) {
            Ok(player) => Backend::Native(player),
            Err(error) => Backend::Unavailable(unsupported::Player::failed(error)),
        };
        #[cfg(not(all(
            feature = "native-playback",
            any(target_os = "macos", target_os = "windows")
        )))]
        let backend = Backend::Unavailable(unsupported::Player::no_backend());

        Self {
            kind,
            backend,
            events,
            _main_thread: PhantomData,
        }
    }

    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    /// Replaces the current source and begins opening it asynchronously.
    pub fn load(&self, source: MediaSource) -> Result<(), MediaError> {
        match &self.backend {
            #[cfg(all(
                feature = "native-playback",
                any(target_os = "macos", target_os = "windows")
            ))]
            Backend::Native(player) => player.load(source),
            Backend::Unavailable(player) => {
                let _ = source;
                Err(player.error().clone())
            }
        }
    }

    /// Applies a playback command without waiting for decoding or rendering.
    pub fn command(&self, command: MediaCommand) -> MediaCommandOutcome {
        match &self.backend {
            #[cfg(all(
                feature = "native-playback",
                any(target_os = "macos", target_os = "windows")
            ))]
            Backend::Native(player) => player.command(command),
            Backend::Unavailable(_) => {
                let _ = command;
                MediaCommandOutcome::Unsupported
            }
        }
    }

    /// Reads native state without opening, decoding, or blocking for media.
    pub fn snapshot(&self) -> MediaSnapshot {
        match &self.backend {
            #[cfg(all(
                feature = "native-playback",
                any(target_os = "macos", target_os = "windows")
            ))]
            Backend::Native(player) => player.snapshot(),
            Backend::Unavailable(player) => player.snapshot(),
        }
    }

    /// Returns the view used by a video player. Audio and unsupported players
    /// return `None`.
    pub fn native_video_view(&self) -> Option<NativeVideoView> {
        match &self.backend {
            #[cfg(all(
                feature = "native-playback",
                any(target_os = "macos", target_os = "windows")
            ))]
            Backend::Native(player) => player.native_video_view(),
            Backend::Unavailable(_) => None,
        }
    }

    /// Registers a non-blocking signal handler. Read [`snapshot`](Self::snapshot)
    /// after receiving a signal; events intentionally do not duplicate state.
    pub fn subscribe(
        &self,
        handler: impl Fn(MediaEvent) + Send + Sync + 'static,
    ) -> MediaSubscription {
        self.events.subscribe(Arc::new(handler))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn unsupported_backend_is_distinct_from_idle_media() {
        let player = unsupported::Player::no_backend();
        let snapshot = player.snapshot();
        assert!(matches!(
            snapshot.availability,
            MediaAvailability::NoBackend(_)
        ));
        assert_eq!(snapshot.state, PlaybackState::Paused);
    }

    #[test]
    fn subscriptions_stop_at_drop() {
        let hub = Arc::new(EventHub::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let subscription = hub.subscribe({
            let calls = Arc::clone(&calls);
            Arc::new(move |_| {
                calls.fetch_add(1, Ordering::Relaxed);
            })
        });
        hub.emit(MediaEvent::Changed);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        drop(subscription);
        hub.emit(MediaEvent::Changed);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn subscriptions_accept_native_callback_threads() {
        let hub = Arc::new(EventHub::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let subscription = hub.subscribe({
            let calls = Arc::clone(&calls);
            Arc::new(move |_| {
                calls.fetch_add(1, Ordering::Relaxed);
            })
        });
        let callback_hub = Arc::clone(&hub);
        std::thread::spawn(move || callback_hub.emit(MediaEvent::Changed))
            .join()
            .expect("native callback thread completes");
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        drop(subscription);
        std::thread::spawn(move || hub.emit(MediaEvent::Changed))
            .join()
            .expect("callback thread completes after unsubscribe");
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn ranges_are_ordered_for_native_backends() {
        assert_eq!(TimeRange::new(8.0, 3.0), TimeRange::new(3.0, 8.0));
    }
}
