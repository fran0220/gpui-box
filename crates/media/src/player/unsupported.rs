use super::{MediaAvailability, MediaError, MediaSnapshot};

pub(super) struct Player {
    error: MediaError,
    no_backend: bool,
}

impl Player {
    #[cfg(any(test, not(any(target_os = "macos", target_os = "windows"))))]
    pub(super) fn no_backend() -> Self {
        Self {
            error: MediaError::new(
                super::MediaErrorKind::NoBackend,
                "Native media playback is available only on macOS and Windows.",
            ),
            no_backend: true,
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub(super) fn failed(error: MediaError) -> Self {
        Self {
            error,
            no_backend: false,
        }
    }

    pub(super) fn error(&self) -> &MediaError {
        &self.error
    }

    pub(super) fn snapshot(&self) -> MediaSnapshot {
        MediaSnapshot::unavailable(if self.no_backend {
            MediaAvailability::NoBackend(self.error.clone())
        } else {
            MediaAvailability::Failed(self.error.clone())
        })
    }
}
