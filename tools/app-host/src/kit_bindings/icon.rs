//! Closed built-in glyph selection, never an asset loader. Resolving a glyph
//! does not add it to the host's selective bundle or bypass missing-asset refusal.
use anyhow::{Result, anyhow, ensure};
use gpui_kit::assets::{Icon, IconName, IconWeight};
use serde_json::Value;

pub(super) fn resolve(value: &Value) -> Result<Icon> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("expected icon descriptor"))?;
    ensure!(
        object
            .keys()
            .all(|key| matches!(key.as_str(), "key" | "weight")),
        "unknown icon field"
    );
    let key = object
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("icon key required"))?;
    let name = IconName::ALL
        .iter()
        .copied()
        .find(|name| name.source_name() == key)
        .ok_or_else(|| anyhow!("unknown built-in icon key"))?;
    let weight = match object.get("weight") {
        None => IconWeight::Regular,
        Some(Value::String(weight)) if weight == "regular" => IconWeight::Regular,
        Some(Value::String(weight)) if weight == "fill" => IconWeight::Fill,
        _ => anyhow::bail!("unknown icon weight"),
    };
    Ok(Icon::new(name).with_weight(weight))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_exact_catalog_and_both_weights() {
        for name in IconName::ALL {
            let regular = resolve(&json!({"key": name.source_name()})).unwrap();
            assert_eq!(regular.name(), *name);
            assert_eq!(regular.weight(), IconWeight::Regular);
            let fill = resolve(&json!({"key": name.source_name(), "weight": "fill"})).unwrap();
            assert_eq!(fill, regular.filled());
            assert_ne!(regular.path(), fill.path());
        }
    }

    #[test]
    fn refuses_paths_urls_unknown_fields_and_weights() {
        for value in [
            json!({}),
            json!({"key":"../arrow-left"}),
            json!({"key":"https://host/a.svg"}),
            json!({"key":"arrow-left","path":"a.svg"}),
            json!({"key":"arrow-left","weight":null}),
            json!({"key":"arrow-left","weight":"bold"}),
        ] {
            assert!(resolve(&value).is_err(), "{value}");
        }
    }
}
