//! Explicit async states for truthful user interfaces.

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncValue<T, E = String> {
    pub value: Option<T>,
    pub status: AsyncStatus<E>,
}

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

impl<T, E> AsyncValue<T, E> {
    pub fn loading() -> Self {
        Self {
            value: None,
            status: AsyncStatus::Loading,
        }
    }

    pub fn ready(value: T) -> Self {
        Self {
            value: Some(value),
            status: AsyncStatus::Ready,
        }
    }

    pub fn refresh(&mut self) {
        self.status = AsyncStatus::Refreshing;
    }

    pub fn fail_refresh(&mut self, error: E) {
        self.status = AsyncStatus::Error(error);
    }

    pub fn is_stale(&self) -> bool {
        self.value.is_some()
            && matches!(self.status, AsyncStatus::Refreshing | AsyncStatus::Error(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refresh_failure_does_not_erase_real_data() {
        let mut value = AsyncValue::<_, &str>::ready(vec!["real"]);
        value.refresh();
        value.fail_refresh("offline");
        assert_eq!(value.value.as_deref(), Some(["real"].as_slice()));
        assert!(value.is_stale());
    }

    #[test]
    fn unavailable_is_not_empty() {
        let unavailable: Loadable<Vec<u8>> = Loadable::Unavailable("unsupported".into());
        assert_ne!(unavailable, Loadable::Empty);
    }
}
