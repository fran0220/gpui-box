//! Bounded synchronous registration authenticated by the host's generation map.
//! The transport bounds the whole request to 256 KiB; the store alone validates
//! and decodes resource bytes. Focus and ambient effect owners are not authority.
use super::{Host, resources};
use anyhow::{Result, ensure};
use gpui::{App, EffectOwner};
use serde::Deserialize;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ResourceRequest {
    pub(super) kind: String,
    pub(super) id: u64,
    pub(super) instance: u64,
    pub(super) revision: u64,
    pub(super) deadline: u64,
    pub(super) registration: resources::Registration,
}

impl ResourceRequest {
    pub(super) fn validate(&self) -> Result<()> {
        ensure!(
            self.kind == "register-resource",
            "invalid resource request kind"
        );
        ensure!(
            self.id > 0 && self.instance > 0 && self.revision > 0,
            "invalid resource request identity or revision"
        );
        Ok(())
    }

    /// Inputs are taken from the current host frame and policy, never the wire.
    fn authorize(
        &self,
        now: u128,
        frame_revision: Option<u64>,
        rendered_revision: u64,
        consent: Option<bool>,
        principal: Option<EffectOwner>,
    ) -> Result<EffectOwner> {
        self.validate()?;
        let deadline = u128::from(self.deadline);
        ensure!(
            deadline > now && deadline - now <= 3000,
            "invalid or expired resource deadline"
        );
        ensure!(
            frame_revision == Some(self.revision) && rendered_revision == self.revision,
            "stale or unrendered resource revision"
        );
        ensure!(consent == Some(true), "resource permission refused");
        principal.ok_or_else(|| anyhow::anyhow!("resource principal unavailable"))
    }
}

impl Host {
    pub(super) fn register_resource(&mut self, request: ResourceRequest, _cx: &mut App) {
        let (id, instance) = (request.id, request.instance);
        let result = (|| {
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
            let principal = request.authorize(
                now,
                self.frame.as_ref().map(|frame| frame.revision),
                self.rendered_revision.get(),
                self.frame
                    .as_ref()
                    .and_then(|frame| frame.resources.get(&request.instance).copied()),
                self.clipboard.principal(request.instance),
            )?;
            self.resource_store
                .register(principal, request.registration)
        })();
        let response = match result {
            Ok(value) => json!({
                "kind": "resource-response", "id": id,
                "instance": instance, "value": value,
            }),
            Err(error) => json!({
                "kind": "resource-response", "id": id,
                "instance": instance, "error": bounded_error(&error.to_string()),
            }),
        };
        // Never block the UI on a stalled bridge. A dropped response times out
        // at the caller; no success is emitted before the store accepts bytes.
        let _ = self.bridge.outgoing.try_send(response);
    }
}

fn bounded_error(message: &str) -> &str {
    let mut end = message.len().min(2048);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    &message[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire() -> serde_json::Value {
        json!({"kind":"register-resource", "id":1, "instance":7,
            "revision":9, "deadline":4000,
            "registration":{"key":"asset", "mime":"application/octet-stream", "data":"AQ=="}})
    }

    fn request() -> ResourceRequest {
        serde_json::from_value(wire()).expect("valid resource request fixture")
    }

    #[test]
    fn closed_request_and_positive_identities() {
        assert!(request().validate().is_ok());
        for field in ["id", "instance", "revision"] {
            let mut value = wire();
            value[field] = json!(0);
            assert!(
                serde_json::from_value::<ResourceRequest>(value)
                    .expect("request with invalid identity")
                    .validate()
                    .is_err()
            );
        }
        let mut value = wire();
        value["kind"] = json!("resource");
        assert!(
            serde_json::from_value::<ResourceRequest>(value)
                .expect("request with invalid kind")
                .validate()
                .is_err()
        );
        for field in ["owner", "url", "path", "process"] {
            let mut value = wire();
            value[field] = json!("forbidden");
            assert!(serde_json::from_value::<ResourceRequest>(value).is_err());
        }
        let mut value = wire();
        value["registration"]["url"] = json!("forbidden");
        assert!(serde_json::from_value::<ResourceRequest>(value).is_err());
    }

    #[test]
    fn authorization_requires_rendered_frame_consent_and_mapped_principal() {
        let request = request();
        let principal = EffectOwner::default();
        assert_eq!(
            request
                .authorize(1000, Some(9), 9, Some(true), Some(principal))
                .expect("current rendered and authorized principal"),
            principal
        );
        for (frame, rendered, consent, owner) in [
            (None, 9, Some(true), Some(principal)),
            (Some(8), 9, Some(true), Some(principal)),
            (Some(9), 8, Some(true), Some(principal)),
            (Some(9), 0, Some(true), Some(principal)),
            (Some(9), 9, None, Some(principal)),
            (Some(9), 9, Some(false), Some(principal)),
            (Some(9), 9, Some(true), None),
        ] {
            assert!(
                request
                    .authorize(1000, frame, rendered, consent, owner)
                    .is_err()
            );
        }
    }

    #[test]
    fn deadline_window_is_exclusive_now_inclusive_three_seconds() {
        let mut request = request();
        for (deadline, accepted) in [
            (0, false),
            (999, false),
            (1000, false),
            (1001, true),
            (4000, true),
            (4001, false),
            (u64::MAX, false),
        ] {
            request.deadline = deadline;
            assert_eq!(
                request
                    .authorize(1000, Some(9), 9, Some(true), Some(EffectOwner::default()))
                    .is_ok(),
                accepted
            );
        }
        assert!(
            request
                .authorize(
                    u128::MAX,
                    Some(9),
                    9,
                    Some(true),
                    Some(EffectOwner::default())
                )
                .is_err()
        );
    }

    #[test]
    fn errors_are_bounded_without_splitting_utf8() {
        assert_eq!(bounded_error("refused"), "refused");
        let message = format!("{}é", "x".repeat(2047));
        assert_eq!(bounded_error(&message).len(), 2047);
        assert_eq!(bounded_error(&"x".repeat(3000)).len(), 2048);
    }
}
