//! Bounded native method dispatch. Identity and authority come from the retained
//! host frame, never from focus or an object pointer supplied by JavaScript.
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Invocation {
    kind: String,
    id: u64,
    instance: u64,
    revision: u64,
    worker_revision: u64,
    target: String,
    component: String,
    #[serde(default)]
    reference: Option<Value>,
    mode: String,
    method: String,
    args: Value,
    deadline: u64,
}

impl Invocation {
    pub(super) fn validate(&self) -> Result<()> {
        ensure!(
            self.kind == "invoke" && self.id > 0 && self.instance > 0,
            "Invalid native request identity"
        );
        ensure!(
            self.target.len() <= 512 && self.component.len() <= 64 && self.method.len() <= 64,
            "Native request name limit exceeded"
        );
        ensure!(
            matches!(self.mode.as_str(), "invoke" | "query"),
            "Unknown native invocation mode"
        );
        ensure!(
            self.args.is_object(),
            "Native method arguments must be a JSON object"
        );
        if let Some(reference) = &self.reference {
            ensure!(
                self.target.is_empty() && self.component.is_empty(),
                "Reference requests cannot override source identity"
            );
            payload(reference)?;
        }
        payload(&self.args)
    }
}

fn payload(value: &Value) -> Result<()> {
    fn visit(value: &Value, depth: usize, count: &mut usize) -> Result<()> {
        *count += 1;
        ensure!(
            depth <= 8 && *count <= 1024,
            "Native payload limit exceeded"
        );
        match value {
            Value::Array(values) => {
                for value in values {
                    visit(value, depth + 1, count)?;
                }
            }
            Value::Object(values) => {
                for (key, value) in values {
                    ensure!(
                        !matches!(key.as_str(), "__proto__" | "prototype" | "constructor"),
                        "Invalid native payload key"
                    );
                    visit(value, depth + 1, count)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    visit(value, 0, &mut 0)?;
    ensure!(
        serde_json::to_vec(value)?.len() <= 16384,
        "Native payload byte limit exceeded"
    );
    Ok(())
}

impl Host {
    pub(super) fn invoke_request(
        &mut self,
        request: Invocation,
        window: &mut Window,
        cx: &mut App,
    ) {
        let result = (|| -> Result<Value> {
            request.validate()?;
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
            ensure!(
                u128::from(request.deadline) >= now && u128::from(request.deadline) <= now + 3000,
                "Native request expired or invalid deadline"
            );
            let frame = self.frame.as_ref().context("No native frame")?;
            ensure!(
                frame.revision == request.revision
                    && self.rendered_revision.get() == request.revision,
                "Stale or unrendered native request revision"
            );
            fn find<'a>(node: &'a Node, id: &str) -> Option<&'a Node> {
                if node.id == id {
                    return Some(node);
                }
                node.children
                    .iter()
                    .chain(node.slots.values().flatten())
                    .find_map(|child| find(child, id))
            }
            self.references.reconcile(
                &frame.tree,
                |node| self.clipboard.owner_of(node),
                |id| self.kit.native_entity_id(id),
                cx,
            );
            let source = request
                .reference
                .as_ref()
                .map(|reference| self.references.source(reference, request.instance))
                .transpose()?;
            let (target, component) = source
                .as_ref()
                .map(|(id, component)| (id.as_str(), component.as_deref().unwrap_or_default()))
                .unwrap_or((&request.target, &request.component));
            let node = find(&frame.tree, target).context("Native target not mounted")?;
            ensure!(
                node.kind == Kind::Kit
                    && node.instance == request.instance
                    && node.component.as_deref() == Some(component),
                "Native target identity mismatch"
            );
            ensure!(
                request.mode == "query"
                    || request.method == "$release"
                    || node.props.get("disabled") != Some(&Value::Bool(true)),
                "Native command refused: target disabled"
            );
            let owner = self
                .clipboard
                .owner_of(node)
                .context("Native owner unavailable")?;
            let result = cx.with_effect_owner(Some(owner), |cx| {
                if let Some(reference) = &request.reference {
                    if request.method == "$release" {
                        ensure!(
                            request.mode == "invoke"
                                && request.args.as_object().is_some_and(|args| args.is_empty()),
                            "Invalid reference release"
                        );
                        self.references.release(reference, owner)?;
                        return Ok(Value::Null);
                    }
                    return self.references.invoke(
                        owner,
                        reference,
                        &request.method,
                        &request.args,
                        request.mode == "query",
                        window,
                        cx,
                    );
                }
                let refs = self.references.registration(node, owner);
                self.kit.invoke_registered(
                    node,
                    &request.method,
                    &request.args,
                    request.mode == "query",
                    window,
                    cx,
                    &refs,
                )
            })?;
            payload(&result)?;
            Ok(result)
        })();
        let mut response = json!({"kind":"native-response", "id":request.id, "instance":request.instance, "workerRevision":request.worker_revision});
        match result {
            Ok(value) => response["value"] = value,
            Err(error) => {
                response["error"] = Value::String(error.to_string().chars().take(2048).collect())
            }
        }
        let _ = self.bridge.outgoing.try_send(response);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_payload_rejects_deep_wide_and_prototype_data() {
        assert!(payload(&json!({"text":"hello", "selection":[2,7]})).is_ok());
        assert!(payload(&json!({"constructor":null})).is_err());
        assert!(payload(&json!(vec![0; 1024])).is_err());
        let mut value = Value::Null;
        for _ in 0..9 {
            value = json!([value]);
        }
        assert!(payload(&value).is_err());
        assert!(payload(&json!("x".repeat(16385))).is_err());
    }
}
