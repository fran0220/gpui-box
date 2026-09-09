//! Same declarative contracts as JS, embedded so native validation never loads
//! executable code or follows paths supplied by a descriptor.
use super::super::Node;
use anyhow::{Result, bail, ensure};
use serde_json::Value;
use std::{collections::HashSet, sync::LazyLock};

static SCHEMAS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("schemas.json")).expect("generated Kit schemas")
});
static METHODS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("methods.json")).expect("generated Kit method schemas")
});

pub(super) fn validate(value: &Value, schema: &Value) -> Result<()> {
    if let Some(branches) = schema.get("oneOf") {
        let branches = branches
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("invalid oneOf schema"))?;
        ensure!(!branches.is_empty(), "invalid oneOf schema");
        let matches = branches
            .iter()
            .filter(|branch| validate(value, branch).is_ok())
            .count();
        ensure!(matches == 1, "expected exactly one matching branch");
        return Ok(());
    }
    if value.is_null() && schema["nullable"] == true {
        return Ok(());
    }
    if let Some(choices) = schema["enum"].as_array() {
        ensure!(choices.contains(value), "invalid enum value");
        return Ok(());
    }
    match schema["type"].as_str().unwrap_or_default() {
        "string" => {
            let text = value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("expected string"))?;
            let length = text.encode_utf16().count() as u64;
            ensure!(
                length >= schema["min"].as_u64().unwrap_or(0)
                    && length <= schema["max"].as_u64().unwrap_or(u64::MAX),
                "string exceeds limits"
            );
        }
        "boolean" => ensure!(value.is_boolean(), "expected boolean"),
        "number" => {
            let number = value
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("expected number"))?;
            ensure!(
                number.is_finite()
                    && number >= schema["min"].as_f64().unwrap_or(f64::MIN)
                    && number <= schema["max"].as_f64().unwrap_or(f64::MAX),
                "number exceeds limits"
            );
            ensure!(
                schema["integer"] != true || number.fract() == 0.,
                "expected integer"
            );
        }
        "object" => {
            let object = value
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("expected object"))?;
            let fields = schema["fields"].as_object().expect("schema fields");
            for (key, value) in object {
                validate(
                    value,
                    fields
                        .get(key)
                        .ok_or_else(|| anyhow::anyhow!("unknown property: {key}"))?,
                )?;
            }
            for key in schema["required"].as_array().expect("required fields") {
                ensure!(
                    object.contains_key(key.as_str().expect("required key")),
                    "missing required property"
                );
            }
        }
        "array" => {
            let items = value
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("expected array"))?;
            ensure!(
                items.len() as u64 <= schema["max"].as_u64().expect("array max"),
                "array exceeds limits"
            );
            let mut ids = HashSet::new();
            for item in items {
                validate(item, &schema["items"])?;
                if schema["items"]["fields"].get("id").is_some() {
                    ensure!(
                        ids.insert(item["id"].as_str().expect("validated identity")),
                        "duplicate item identity"
                    );
                }
            }
        }
        _ => bail!("unsupported schema"),
    }
    Ok(())
}

pub(super) fn invocation(
    component: &str,
    name: &str,
    args: &Value,
    query: bool,
) -> Result<&'static Value> {
    let mode = if query { "query" } else { "invoke" };
    let method = METHODS
        .get(component)
        .and_then(|component| component.get(mode))
        .and_then(|methods| methods.get(name))
        .ok_or_else(|| anyhow::anyhow!("unsupported Kit method"))?;
    validate(args, &method["args"])?;
    Ok(&method["result"])
}

fn validate_slots(schema: &Value, node: &Node) -> Result<()> {
    let mut slots: HashSet<String> = schema["slots"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    if let Some(property) = schema["slotIds"].as_str() {
        for id in node
            .props
            .get(property)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item["id"].as_str())
        {
            if let Some(suffixes) = schema["slotSuffixes"].as_array() {
                for suffix in suffixes.iter().filter_map(Value::as_str) {
                    slots.insert(format!("{id}:{suffix}"));
                }
            } else {
                slots.insert(id.to_owned());
            }
        }
    }
    for (name, children) in &node.slots {
        ensure!(
            slots.contains(name.as_str()) && children.len() <= 1024,
            "unknown slot or slot limit exceeded"
        );
    }
    Ok(())
}

pub(crate) fn validate_descriptor(node: &Node) -> Result<()> {
    let component = node.component.as_deref().unwrap_or_default();
    let schema = SCHEMAS
        .get(component)
        .ok_or_else(|| anyhow::anyhow!("unsupported Kit component"))?;
    validate(&Value::Object(node.props.clone()), &schema["props"])?;
    validate_slots(schema, node)?;
    for (event, action) in &node.events {
        ensure!(schema["events"].get(event).is_some(), "unknown Kit event");
        ensure!(
            !action.is_empty() && action.len() <= 512,
            "invalid Kit action"
        );
    }
    ensure!(
        node.props.get("disabled") != Some(&Value::Bool(true)) || node.events.is_empty(),
        "disabled control has actions"
    );
    if component == "Slider" {
        let min = node.props.get("min").and_then(Value::as_f64).unwrap_or(0.);
        let max = node.props.get("max").and_then(Value::as_f64).unwrap_or(1.);
        let value = node
            .props
            .get("value")
            .and_then(Value::as_f64)
            .unwrap_or(min);
        ensure!(
            min < max && value >= min && value <= max,
            "invalid slider range"
        );
        if let Some(high) = node.props.get("high").and_then(Value::as_f64) {
            ensure!(high >= value && high <= max, "invalid upper slider value");
        }
    }
    if component == "List" {
        let parents = node
            .props
            .get("rows")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|row| {
                (
                    row["id"].as_str().expect("validated id"),
                    row["within"].as_str(),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        for id in parents.keys() {
            let mut seen = HashSet::from([*id]);
            let mut next = parents[id];
            while let Some(parent) = next {
                ensure!(
                    parents.contains_key(parent) && seen.insert(parent),
                    "invalid row parent"
                );
                next = parents[parent];
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn schema_primitives_match_shared_js_cases() {
        let fixtures: Value = serde_json::from_str(include_str!(
            "../../../js-runtime/tests/schema-fixtures.json"
        ))
        .expect("schema parity fixtures");
        for fixture in fixtures["unions"].as_array().expect("union fixtures") {
            for case in fixture["cases"].as_array().expect("union cases") {
                assert_eq!(
                    validate(&case[0], &fixture["schema"]).is_ok(),
                    case[1].as_bool().expect("verdict"),
                    "{}: {}",
                    fixture["name"],
                    case[0]
                );
            }
        }
        assert!(validate(&Value::Null, &serde_json::json!({"oneOf":[]})).is_err());
        assert!(validate(&Value::Null, &serde_json::json!({"oneOf":{}})).is_err());
        for fixture in fixtures["slots"].as_array().expect("slot fixtures") {
            for case in fixture["cases"].as_array().expect("slot cases") {
                let mut node: Node = serde_json::from_value(
                    serde_json::json!({"kind":"column","id":"fixture","props":fixture["props"]}),
                )
                .expect("slot fixture node");
                node.slots
                    .insert(case[0].as_str().expect("slot name").into(), vec![]);
                assert_eq!(
                    validate_slots(&fixture["schema"], &node).is_ok(),
                    case[1].as_bool().expect("verdict"),
                    "slot {}",
                    case[0]
                );
            }
        }
    }

    #[test]
    fn native_contract_rejects_unknown_properties_and_asymmetric_ranges() {
        let node = |props| {
            serde_json::from_value::<Node>(
                json!({"kind":"kit","id":"range","component":"Slider","props":props}),
            )
            .expect("fixture node")
        };
        assert!(
            validate_descriptor(&node(json!({"min":-10,"max":20,"value":-3,"high":17}))).is_ok()
        );
        assert!(
            validate_descriptor(&node(json!({"min":-10,"max":20,"value":-3,"high":-4}))).is_err()
        );
        assert!(validate_descriptor(&node(json!({"source":"/etc/passwd"}))).is_err());
        assert!(validate_descriptor(&node(json!({"disabled":"false"}))).is_err());
    }

    #[test]
    fn native_list_rejects_missing_and_cyclic_row_parents() {
        let list = |rows| {
            serde_json::from_value::<Node>(
                json!({"kind":"kit","id":"list","component":"List","props":{"rows":rows}}),
            )
            .expect("list fixture")
        };
        assert!(
            validate_descriptor(&list(
                json!([{"id":"a","label":"A"},{"id":"b","label":"B","within":"a"}])
            ))
            .is_ok()
        );
        assert!(
            validate_descriptor(&list(json!([{"id":"a","label":"A","within":"missing"}]))).is_err()
        );
        assert!(
            validate_descriptor(&list(
                json!([{"id":"a","label":"A","within":"b"},{"id":"b","label":"B","within":"a"}])
            ))
            .is_err()
        );
    }

    #[test]
    fn embedded_schemas_exactly_match_native_registration() {
        let names: HashSet<_> = SCHEMAS
            .as_object()
            .expect("schema map")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(names, super::super::COMPONENTS.iter().copied().collect());
    }
}
