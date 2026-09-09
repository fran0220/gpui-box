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
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod native;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub use native::{BrowserHost, BrowserOptions};

#[cfg(test)]
mod tests {
    use super::*;

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
