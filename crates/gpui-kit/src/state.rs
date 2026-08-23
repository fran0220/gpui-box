//! Explicit async states for truthful user interfaces.
//!
//! Per-surface enums keep the typed payload they already carry. [`Phase`] is
//! the shared projection: what a surface knows about its own content, not
//! what that surface is doing. [`HasPhase`] is how those enums, and the
//! library-level [`Loadable`] / [`AsyncValue`] types, answer the same
//! question.

/// What a surface knows about its own content.
///
/// This is not a description of activity. A transport that is playing and one
/// that is paused are both [`Phase::Ready`]: the difference belongs to the
/// transport, not to the ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Phase {
    /// Never requested.
    #[default]
    Idle,
    /// Accepted, and not yet started.
    Queued,
    /// Waiting for an answer (approval, credentials).
    Blocked,
    /// In flight, with no verified value.
    Loading,
    /// In flight, keeping a verified value on screen.
    Refreshing,
    /// A verified value.
    Ready,
    /// A successful result with nothing in it.
    Empty,
    /// A refusal, or a capability that does not exist.
    Unavailable,
    /// The attempt failed.
    Error,
    /// Withdrawn.
    Cancelled,
}

impl Phase {
    /// The name a semantic node publishes for the phase itself.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Queued => "queued",
            Self::Blocked => "blocked",
            Self::Loading => "loading",
            Self::Refreshing => "refreshing",
            Self::Ready => "ready",
            Self::Empty => "empty",
            Self::Unavailable => "unavailable",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
        }
    }

    /// Whether this phase is still waiting on something.
    pub const fn is_busy(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Blocked | Self::Loading | Self::Refreshing
        )
    }
}

/// A surface that can say which [`Phase`] it is in.
pub trait HasPhase {
    fn phase(&self) -> Phase;

    /// The host's own words. Never authored here.
    fn reason(&self) -> Option<&str> {
        None
    }

    /// A verified value remains on screen after a failed refresh.
    fn is_stale(&self) -> bool {
        false
    }
}

impl HasPhase for Phase {
    fn phase(&self) -> Phase {
        *self
    }
}

impl<T: HasPhase + ?Sized> HasPhase for &T {
    fn phase(&self) -> Phase {
        (*self).phase()
    }

    fn reason(&self) -> Option<&str> {
        (*self).reason()
    }

    fn is_stale(&self) -> bool {
        (*self).is_stale()
    }
}

/// The states a value a host is fetching can be in.
///
/// Empty, unavailable, and failed are separate variants on purpose: a refusal
/// rendered as an absence of data is a lie about the host.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Loadable<T, E = String> {
    #[default]
    Idle,
    Loading,
    Ready(T),
    Empty,
    Unavailable(String),
    Error(E),
}

impl<T, E> Loadable<T, E> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Ready(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Loadable<U, E> {
        match self {
            Self::Idle => Loadable::Idle,
            Self::Loading => Loadable::Loading,
            Self::Ready(value) => Loadable::Ready(map(value)),
            Self::Empty => Loadable::Empty,
            Self::Unavailable(reason) => Loadable::Unavailable(reason),
            Self::Error(error) => Loadable::Error(error),
        }
    }

    /// The same facts, split so a later refresh can keep the verified value.
    pub fn into_async(self) -> AsyncValue<T, E> {
        match self {
            Self::Idle => AsyncValue::idle(),
            Self::Loading => AsyncValue::loading(),
            Self::Ready(value) => AsyncValue::ready(value),
            Self::Empty => AsyncValue::empty(),
            Self::Unavailable(reason) => AsyncValue::refused(reason),
            Self::Error(error) => AsyncValue::error(error),
        }
    }
}

impl<T, E> HasPhase for Loadable<T, E> {
    fn phase(&self) -> Phase {
        match self {
            Self::Idle => Phase::Idle,
            Self::Loading => Phase::Loading,
            Self::Ready(_) => Phase::Ready,
            Self::Empty => Phase::Empty,
            Self::Unavailable(_) => Phase::Unavailable,
            Self::Error(_) => Phase::Error,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(reason) => Some(reason.as_str()),
            _ => None,
        }
    }
}

/// A value and, separately, what is currently happening to it.
///
/// Splitting the two is what lets a failed refresh keep the last verified
/// value on screen instead of replacing it with an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncValue<T, E = String> {
    pub value: Option<T>,
    pub status: AsyncStatus<E>,
    /// How many times this has been asked for. A first load and a third retry
    /// are different facts.
    pub attempts: usize,
}

/// What is happening to an [`AsyncValue`] right now.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AsyncStatus<E = String> {
    #[default]
    Idle,
    Loading,
    Refreshing,
    Ready,
    Empty,
    Unavailable(String),
    Error(E),
}

impl<E: AsRef<str>> HasPhase for AsyncStatus<E> {
    fn phase(&self) -> Phase {
        match self {
            Self::Idle => Phase::Idle,
            Self::Loading => Phase::Loading,
            Self::Refreshing => Phase::Refreshing,
            Self::Ready => Phase::Ready,
            Self::Empty => Phase::Empty,
            Self::Unavailable(_) => Phase::Unavailable,
            Self::Error(_) => Phase::Error,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(reason) => Some(reason.as_str()),
            Self::Error(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl<T, E> AsyncValue<T, E> {
    pub fn idle() -> Self {
        Self {
            value: None,
            status: AsyncStatus::Idle,
            attempts: 0,
        }
    }

    pub fn loading() -> Self {
        Self {
            value: None,
            status: AsyncStatus::Loading,
            attempts: 0,
        }
    }

    pub fn ready(value: T) -> Self {
        Self {
            value: Some(value),
            status: AsyncStatus::Ready,
            attempts: 1,
        }
    }

    pub fn empty() -> Self {
        Self {
            value: None,
            status: AsyncStatus::Empty,
            attempts: 1,
        }
    }

    /// The host refused, in its own words.
    pub fn refused(reason: impl Into<String>) -> Self {
        Self {
            value: None,
            status: AsyncStatus::Unavailable(reason.into()),
            attempts: 1,
        }
    }

    pub fn error(error: E) -> Self {
        Self {
            value: None,
            status: AsyncStatus::Error(error),
            attempts: 1,
        }
    }

    pub fn refresh(&mut self) {
        self.status = AsyncStatus::Refreshing;
        self.attempts = self.attempts.saturating_add(1);
    }

    pub fn fail_refresh(&mut self, error: E) {
        self.status = AsyncStatus::Error(error);
    }

    /// Records one more attempt without changing the status.
    pub fn record_attempt(&mut self) {
        self.attempts = self.attempts.saturating_add(1);
    }

    pub fn is_stale(&self) -> bool {
        self.value.is_some() && matches!(self.status, AsyncStatus::Error(_))
    }
}

impl<T, E: AsRef<str>> HasPhase for AsyncValue<T, E> {
    fn phase(&self) -> Phase {
        self.status.phase()
    }

    fn reason(&self) -> Option<&str> {
        self.status.reason()
    }

    fn is_stale(&self) -> bool {
        AsyncValue::is_stale(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refresh_failure_does_not_erase_real_data() {
        let mut value = AsyncValue::<_, &str>::ready(vec!["real"]);
        value.refresh();
        assert!(!value.is_stale());
        value.fail_refresh("offline");
        assert_eq!(value.value.as_deref(), Some(["real"].as_slice()));
        assert!(value.is_stale());
        assert_eq!(value.attempts, 2);
        assert_eq!(value.phase(), Phase::Error);
    }

    #[test]
    fn unavailable_is_not_empty() {
        let unavailable: Loadable<Vec<u8>> = Loadable::Unavailable("unsupported".into());
        assert_ne!(unavailable, Loadable::Empty);
        assert_eq!(unavailable.phase(), Phase::Unavailable);
        assert_eq!(unavailable.reason(), Some("unsupported"));
    }

    #[test]
    fn a_refusal_constructs_as_unavailable() {
        let value = AsyncValue::<(), String>::refused("permission denied");
        assert_eq!(value.phase(), Phase::Unavailable);
        assert_eq!(value.reason(), Some("permission denied"));
        assert!(value.value.is_none());
    }

    #[test]
    fn loadable_projects_into_async_value() {
        let ready = Loadable::<_, String>::Ready(7).into_async();
        assert_eq!(ready.phase(), Phase::Ready);
        assert_eq!(ready.value, Some(7));
        assert_eq!(ready.attempts, 1);

        let refused = Loadable::<u8, String>::Unavailable("offline".into()).into_async();
        assert_eq!(refused.phase(), Phase::Unavailable);
        assert_eq!(refused.reason(), Some("offline"));
    }

    #[test]
    fn every_phase_has_a_distinct_name() {
        let names: Vec<_> = [
            Phase::Idle,
            Phase::Queued,
            Phase::Blocked,
            Phase::Loading,
            Phase::Refreshing,
            Phase::Ready,
            Phase::Empty,
            Phase::Unavailable,
            Phase::Error,
            Phase::Cancelled,
        ]
        .into_iter()
        .map(Phase::name)
        .collect();
        for (index, name) in names.iter().enumerate() {
            assert!(names[index + 1..].iter().all(|other| other != name));
        }
    }
}
