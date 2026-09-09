//! Caller-owned native browser hosts. See docs/webview.md for native constraints.

use url::Url;

/// Synchronous navigation policy. This governs navigation, not subresource
/// networking; it is not a network sandbox or an IPC authorization policy.
#[derive(Clone, Debug, Default)]
pub struct NavigationPolicy {
    /// Empty allows all HTTP(S) origins. Entries are parsed and normalized.
    pub origins: Vec<Url>,
}

impl NavigationPolicy {
    /// Rejects local files, executable schemes, credentials, and opaque origins.
    pub fn allows(&self, address: &str) -> bool {
        let Ok(url) = Url::parse(address) else {
            return false;
        };
        matches!(url.scheme(), "http" | "https")
            && url.username().is_empty()
            && url.password().is_none()
            && (self.origins.is_empty()
                || self
                    .origins
                    .iter()
                    .any(|allowed| allowed.origin() == url.origin()))
    }
}

/// Engine notifications. PageFinished is deliberately not a success assertion:
/// native failure notifications may follow it. Do not log URLs or messages
/// without the application's privacy/redaction policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserEvent {
    NavigationStarted(String),
    NavigationRefused(String),
    PageFinished(String),
    LoadFailed {
        url: String,
        description: String,
    },
    ProcessTerminated(String),
    PopupRefused(String),
    DownloadRefused(String),
    PermissionRefused(String),
    /// Untrusted page data, NEVER a privileged runtime command. In particular
    /// Linux Wry reports the main URL for iframe IPC, not the sender's origin.
    Message {
        reported_url: String,
        body: String,
    },
    MessageRefused,
    /// The caller did not drain the bounded event queue quickly enough. State
    /// is incomplete; do not infer success from a stream with missing events.
    EventsDropped(usize),
}

#[derive(Clone)]
struct EventSender {
    queue: std::sync::mpsc::SyncSender<BrowserEvent>,
    dropped: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

struct EventInbox {
    queue: std::sync::mpsc::Receiver<BrowserEvent>,
    dropped: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl EventSender {
    fn channel() -> (Self, EventInbox) {
        let (sender, receiver) = std::sync::mpsc::sync_channel(256);
        let dropped = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        (
            Self {
                queue: sender,
                dropped: dropped.clone(),
            },
            EventInbox {
                queue: receiver,
                dropped,
            },
        )
    }

    // Native callbacks must never block the UI thread, even under page spam.
    fn send(&self, event: BrowserEvent) -> Result<(), std::sync::mpsc::TrySendError<BrowserEvent>> {
        let result = self.queue.try_send(event);
        if matches!(result, Err(std::sync::mpsc::TrySendError::Full(_))) {
            self.dropped
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        result
    }
}

impl EventInbox {
    fn drain(&self) -> impl Iterator<Item = BrowserEvent> + '_ {
        std::iter::from_fn(|| {
            if let Ok(event) = self.queue.try_recv() {
                return Some(event);
            }
            let dropped = self.dropped.swap(0, std::sync::atomic::Ordering::Relaxed);
            (dropped > 0).then_some(BrowserEvent::EventsDropped(dropped))
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod native;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub use native::{BrowserHost, BrowserOptions};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_event_flood_is_bounded_nonblocking_and_reported() {
        let (sender, inbox) = EventSender::channel();
        for index in 0..256 {
            assert!(
                sender
                    .send(BrowserEvent::PageFinished(index.to_string()))
                    .is_ok()
            );
        }
        assert!(sender.send(BrowserEvent::MessageRefused).is_err());
        assert!(sender.send(BrowserEvent::MessageRefused).is_err());
        // A caller consuming only part of one iterator must not lose overflow.
        assert_eq!(
            inbox.drain().next(),
            Some(BrowserEvent::PageFinished("0".into()))
        );
        let events: Vec<_> = inbox.drain().collect();
        assert_eq!(events.len(), 256);
        assert_eq!(events[0], BrowserEvent::PageFinished("1".into()));
        assert_eq!(events[254], BrowserEvent::PageFinished("255".into()));
        assert_eq!(events[255], BrowserEvent::EventsDropped(2));
        assert_eq!(inbox.drain().count(), 0);
        assert!(sender.send(BrowserEvent::MessageRefused).is_ok());
        assert_eq!(
            inbox.drain().collect::<Vec<_>>(),
            vec![BrowserEvent::MessageRefused]
        );
    }

    #[test]
    fn navigation_compares_origins_not_prefixes() {
        let policy = NavigationPolicy {
            origins: vec![Url::parse("https://example.test/path").expect("valid fixture URL")],
        };
        assert!(policy.allows("https://EXAMPLE.test:443/elsewhere"));
        for denied in [
            "https://example.test.evil.test/",
            "http://example.test/",
            "https://example.test:444/",
            "https://user@example.test/",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,hi",
            "about:blank",
        ] {
            assert!(!policy.allows(denied), "{denied}");
        }
    }
}
