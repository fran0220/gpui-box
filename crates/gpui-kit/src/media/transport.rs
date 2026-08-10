//! The seam between a player and the surfaces that show one.
//!
//! [`crate::content::TransportBar`] reports every control and applies none of
//! them, which is right for a bar the host wires to its own player. A player
//! *component* needs the other half: something it can ask what is true and
//! ask to change it, without this crate holding a decoder, a clock, or a
//! device.
//!
//! That something is [`MediaTransport`]. It is a trait rather than an
//! implementation because there is no audio or video backend in GPUI at the
//! pinned revision — no decoder, no output device, and no frame pump — so a
//! component that claimed to play something would be claiming a capability
//! nothing behind it has. The trait is the contract; an operating-system
//! backend lands behind it additively, and the components above it do not
//! change when it does.
//!
//! # What a transport promises
//!
//! - [`snapshot`](MediaTransport::snapshot) answers with what is true now. It
//!   is read once per frame and must not block, allocate a decode, or start
//!   one.
//! - [`apply`](MediaTransport::apply) is asked for a change and answers
//!   whether it took it. A refusal carries the backend's own sentence, and the
//!   next snapshot still reports the state that actually holds — which is what
//!   keeps a refused seek from moving the head on screen.
//! - [`origin`](MediaTransport::origin) says whether the facts came from a
//!   real player or from a fixture, and every component publishes it. A
//!   fixture that could be mistaken for a player is the one failure this
//!   module exists to prevent.
//!
//! # The fixture is not a player
//!
//! [`FixtureTransport`] decodes nothing, opens no device, and advances no
//! clock: its position moves only when something asks it to seek. It exists so
//! the scenes and the tests have deterministic media states, and it reports
//! [`MediaOrigin::Fixture`] so that the surface above it says so on screen.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::SharedString;

use crate::content::transport::{BufferedRange, TrackStep, TransportDuration, TransportState};

/// Where a surface's playback facts came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaOrigin {
    /// A real player: a decoder, an output device, and a clock that advances
    /// on its own.
    Platform,
    /// A deterministic stand-in. Nothing is decoded and no clock runs.
    Fixture,
}

impl MediaOrigin {
    /// The name a semantic node publishes, so a test reads where the facts
    /// came from rather than which caption was drawn for them.
    pub fn name(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::Fixture => "fixture",
        }
    }

    pub fn is_fixture(self) -> bool {
        matches!(self, Self::Fixture)
    }
}

/// Whether there is anything to play, and whether anything could play it.
///
/// This is [`crate::state::Loadable`]'s vocabulary with the one distinction a
/// player needs and a list does not: media the host holds and nothing on this
/// machine can decode is neither loading nor broken, and calling it either
/// would send the reader looking for the wrong repair.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MediaAvailability {
    /// Nothing has been handed to the transport yet.
    #[default]
    Idle,
    /// The transport is opening the media.
    Loading,
    /// There is no backend on this machine that can play it, in the
    /// backend's own words.
    NoBackend(SharedString),
    /// The media was opened and could not be read, in the backend's words.
    Failed(SharedString),
    /// The transport holds the media and can move it.
    Ready,
}

impl MediaAvailability {
    /// The name a semantic node publishes.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading => "loading",
            Self::NoBackend(_) => "no-backend",
            Self::Failed(_) => "failed",
            Self::Ready => "ready",
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// The backend's own sentence, when it gave one.
    pub fn reason(&self) -> Option<SharedString> {
        match self {
            Self::NoBackend(reason) | Self::Failed(reason) => Some(reason.clone()),
            _ => None,
        }
    }
}

/// Everything a player surface draws, as the transport reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaSnapshot {
    pub availability: MediaAvailability,
    pub state: TransportState,
    /// Where the head is, in seconds.
    pub position: f32,
    pub duration: TransportDuration,
    pub volume: f32,
    pub muted: bool,
    pub speed: f32,
    /// The spans the backend already holds. An empty list draws no buffer,
    /// rather than a full one.
    pub buffered: Vec<BufferedRange>,
}

impl Default for MediaSnapshot {
    fn default() -> Self {
        Self {
            availability: MediaAvailability::default(),
            state: TransportState::Paused,
            position: 0.0,
            duration: TransportDuration::Unknown,
            volume: 1.0,
            muted: false,
            speed: 1.0,
            buffered: Vec::new(),
        }
    }
}

/// What a surface asks a transport for. It decides none of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaCommand {
    Play,
    Pause,
    /// Go to this many seconds.
    Seek(f32),
    SetVolume(f32),
    ToggleMute,
    SetSpeed(f32),
    Step(TrackStep),
}

impl MediaCommand {
    /// The name a semantic node and an event log use.
    pub fn name(self) -> &'static str {
        match self {
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Seek(_) => "seek",
            Self::SetVolume(_) => "volume",
            Self::ToggleMute => "mute",
            Self::SetSpeed(_) => "speed",
            Self::Step(_) => "step",
        }
    }
}

/// What the transport did with a command.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaOutcome {
    /// The transport took it. The next snapshot says what that produced;
    /// nothing here promises the change is already visible.
    Applied,
    /// The transport declined, in its own words.
    Refused(SharedString),
    /// This backend does not implement this command at all. It is not a
    /// failure and not a refusal, and a surface may drop the control instead
    /// of offering one that never works.
    Unsupported,
}

impl MediaOutcome {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Refused(_) => "refused",
            Self::Unsupported => "unsupported",
        }
    }
}

/// A player, seen from a component that will never be one.
///
/// Implementations answer through `&self` and hold whatever interior
/// mutability they need, because a surface holds an [`Rc`] of one and is
/// rebuilt every frame.
pub trait MediaTransport: std::fmt::Debug {
    /// Whether these facts come from a real player or from a fixture.
    fn origin(&self) -> MediaOrigin;

    /// What is true now. Called once per frame; it must not block.
    fn snapshot(&self) -> MediaSnapshot;

    /// Asks for a change, and answers what became of the request.
    fn apply(&self, command: MediaCommand) -> MediaOutcome;
}

/// What a player surface reports. It applies none of it: the transport
/// already did or already refused, and this says which.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaEvent {
    /// The transport took the command.
    Applied(MediaCommand),
    /// The transport declined, and this is why.
    Refused(MediaCommand, SharedString),
    /// The transport does not implement the command.
    Unsupported(MediaCommand),
}

impl MediaEvent {
    /// Pairs a command with what the transport answered.
    pub fn of(command: MediaCommand, outcome: MediaOutcome) -> Self {
        match outcome {
            MediaOutcome::Applied => Self::Applied(command),
            MediaOutcome::Refused(reason) => Self::Refused(command, reason),
            MediaOutcome::Unsupported => Self::Unsupported(command),
        }
    }

    pub fn command(&self) -> MediaCommand {
        match self {
            Self::Applied(command) | Self::Refused(command, _) | Self::Unsupported(command) => {
                *command
            }
        }
    }
}

/// A transport that plays nothing, on purpose.
///
/// It holds a snapshot the caller wrote and moves it only when a command asks
/// it to: no clock runs, so the same scene renders the same pixels on every
/// machine and in any order, which is what the visual gate needs. It records
/// every command so a test can assert what a control asked for.
///
/// It reports [`MediaOrigin::Fixture`], and every surface in this module
/// publishes and draws that, so a fixture on screen is never mistaken for a
/// player.
pub struct FixtureTransport {
    snapshot: RefCell<MediaSnapshot>,
    commands: RefCell<Vec<MediaCommand>>,
    /// Set when the fixture is standing in for a backend that says no.
    refusal: Option<SharedString>,
    unsupported: Vec<&'static str>,
}

impl std::fmt::Debug for FixtureTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FixtureTransport")
            .field("snapshot", &self.snapshot.borrow())
            .field("commands", &self.commands.borrow().len())
            .field("refuses", &self.refusal.is_some())
            .finish()
    }
}

impl Default for FixtureTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl FixtureTransport {
    /// A fixture holding nothing, which is [`MediaAvailability::Idle`].
    pub fn new() -> Self {
        Self {
            snapshot: RefCell::new(MediaSnapshot::default()),
            commands: RefCell::new(Vec::new()),
            refusal: None,
            unsupported: Vec::new(),
        }
    }

    /// A fixture that holds media of a known length and is ready to move it.
    pub fn ready(duration: f32) -> Self {
        let mut fixture = Self::new();
        {
            let snapshot = fixture.snapshot.get_mut();
            snapshot.availability = MediaAvailability::Ready;
            snapshot.duration = TransportDuration::Known(duration.max(0.0));
        }
        fixture
    }

    /// A fixture standing in for a live stream: ready, and no total.
    pub fn live() -> Self {
        let mut fixture = Self::new();
        {
            let snapshot = fixture.snapshot.get_mut();
            snapshot.availability = MediaAvailability::Ready;
            snapshot.duration = TransportDuration::Unknown;
        }
        fixture
    }

    pub fn state(mut self, state: TransportState) -> Self {
        self.snapshot.get_mut().state = state;
        self
    }

    /// Where the head stands. It stays there: nothing here advances it.
    pub fn position(mut self, seconds: f32) -> Self {
        self.snapshot.get_mut().position = seconds.max(0.0);
        self
    }

    pub fn volume(mut self, volume: f32) -> Self {
        self.snapshot.get_mut().volume = volume.clamp(0.0, 1.0);
        self
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.snapshot.get_mut().muted = muted;
        self
    }

    pub fn speed(mut self, speed: f32) -> Self {
        self.snapshot.get_mut().speed = speed.max(f32::EPSILON);
        self
    }

    pub fn buffered(mut self, ranges: impl IntoIterator<Item = BufferedRange>) -> Self {
        self.snapshot.get_mut().buffered = ranges.into_iter().collect();
        self
    }

    pub fn loading(mut self) -> Self {
        self.snapshot.get_mut().availability = MediaAvailability::Loading;
        self
    }

    /// Stands in for a machine with no backend that can play this.
    pub fn no_backend(mut self, reason: impl Into<SharedString>) -> Self {
        self.snapshot.get_mut().availability = MediaAvailability::NoBackend(reason.into());
        self
    }

    /// Stands in for media that was opened and could not be read.
    pub fn failed(mut self, reason: impl Into<SharedString>) -> Self {
        self.snapshot.get_mut().availability = MediaAvailability::Failed(reason.into());
        self
    }

    /// Makes every command answer [`MediaOutcome::Refused`], which is how a
    /// test proves a refused control changes nothing on screen.
    pub fn refusing(mut self, reason: impl Into<SharedString>) -> Self {
        self.refusal = Some(reason.into());
        self
    }

    /// Names commands this fixture does not implement, by
    /// [`MediaCommand::name`].
    pub fn unsupported(mut self, commands: impl IntoIterator<Item = &'static str>) -> Self {
        self.unsupported = commands.into_iter().collect();
        self
    }

    /// Every command asked of it, in order.
    pub fn commands(&self) -> Vec<MediaCommand> {
        self.commands.borrow().clone()
    }

    /// The fixture as a surface takes it.
    pub fn shared(self) -> Rc<dyn MediaTransport> {
        Rc::new(self)
    }
}

impl MediaTransport for FixtureTransport {
    fn origin(&self) -> MediaOrigin {
        MediaOrigin::Fixture
    }

    fn snapshot(&self) -> MediaSnapshot {
        self.snapshot.borrow().clone()
    }

    fn apply(&self, command: MediaCommand) -> MediaOutcome {
        self.commands.borrow_mut().push(command);
        if self.unsupported.contains(&command.name()) {
            return MediaOutcome::Unsupported;
        }
        if let Some(reason) = &self.refusal {
            return MediaOutcome::Refused(reason.clone());
        }
        let mut snapshot = self.snapshot.borrow_mut();
        // A fixture that is not holding media cannot be moved by a command
        // either: taking one would put it in a state its availability denies.
        if !snapshot.availability.is_ready() {
            return MediaOutcome::Unsupported;
        }
        match command {
            MediaCommand::Play => snapshot.state = TransportState::Playing,
            MediaCommand::Pause => snapshot.state = TransportState::Paused,
            MediaCommand::Seek(seconds) => {
                let clamped = match snapshot.duration.seconds() {
                    Some(total) => seconds.clamp(0.0, total),
                    None => seconds.max(0.0),
                };
                snapshot.position = clamped;
            }
            MediaCommand::SetVolume(volume) => snapshot.volume = volume.clamp(0.0, 1.0),
            MediaCommand::ToggleMute => snapshot.muted = !snapshot.muted,
            MediaCommand::SetSpeed(speed) => snapshot.speed = speed.max(f32::EPSILON),
            // Which media comes next is a playlist, and a fixture holds one
            // piece of media and no playlist.
            MediaCommand::Step(_) => return MediaOutcome::Unsupported,
        }
        MediaOutcome::Applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixture_never_advances_its_own_head() {
        let fixture = FixtureTransport::ready(120.0).position(30.0);
        assert_eq!(fixture.apply(MediaCommand::Play), MediaOutcome::Applied);
        assert_eq!(fixture.snapshot().state, TransportState::Playing);
        assert_eq!(
            fixture.snapshot().position,
            30.0,
            "playing a fixture decodes nothing, so nothing moves"
        );
    }

    #[test]
    fn a_seek_stops_at_a_known_end_and_only_at_zero_without_one() {
        let bounded = FixtureTransport::ready(120.0);
        bounded.apply(MediaCommand::Seek(500.0));
        assert_eq!(bounded.snapshot().position, 120.0);

        let live = FixtureTransport::live();
        live.apply(MediaCommand::Seek(-5.0));
        assert_eq!(live.snapshot().position, 0.0);
        live.apply(MediaCommand::Seek(500.0));
        assert_eq!(live.snapshot().position, 500.0);
    }

    #[test]
    fn a_refusing_transport_changes_nothing_and_says_why() {
        let fixture = FixtureTransport::ready(120.0).refusing("The device is in use.");
        let outcome = fixture.apply(MediaCommand::Play);
        assert_eq!(
            outcome,
            MediaOutcome::Refused(SharedString::from("The device is in use."))
        );
        assert_eq!(
            fixture.snapshot().state,
            TransportState::Paused,
            "a refused command must leave the state that still holds"
        );
        assert_eq!(fixture.commands(), vec![MediaCommand::Play]);
    }

    #[test]
    fn a_transport_holding_nothing_takes_no_command() {
        let idle = FixtureTransport::new();
        assert_eq!(idle.apply(MediaCommand::Play), MediaOutcome::Unsupported);
        assert_eq!(idle.snapshot().state, TransportState::Paused);

        let absent = FixtureTransport::new().no_backend("No decoder for AV1 on this machine.");
        assert_eq!(absent.apply(MediaCommand::Play), MediaOutcome::Unsupported);
        assert_eq!(
            absent.snapshot().availability.name(),
            "no-backend",
            "a machine that cannot play it is not a machine that is loading it"
        );
    }

    #[test]
    fn an_event_carries_the_command_and_what_became_of_it() {
        assert_eq!(
            MediaEvent::of(MediaCommand::Pause, MediaOutcome::Applied),
            MediaEvent::Applied(MediaCommand::Pause)
        );
        let refused = MediaEvent::of(
            MediaCommand::Seek(4.0),
            MediaOutcome::Refused(SharedString::from("Seeking a live stream is refused.")),
        );
        assert_eq!(refused.command(), MediaCommand::Seek(4.0));
        assert!(matches!(refused, MediaEvent::Refused(_, _)));
    }

    #[test]
    fn a_fixture_says_it_is_a_fixture() {
        assert!(FixtureTransport::new().origin().is_fixture());
        assert_eq!(MediaOrigin::Platform.name(), "platform");
    }
}
