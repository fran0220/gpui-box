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

const DATA_DEPTH: usize = 32;
const WORK_LIMIT: usize = 100_000;
const SCHEMA_NODES: usize = 4096;
const SCHEMA_DEPTH: usize = 128;
const VALIDATION_STACK: usize = 256;

fn definition_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
        })
}

fn schema_document(root: &Value) -> Result<()> {
    ensure!(root.is_object(), "expected schema object");
    if let Some(defs) = root.get("$defs") {
        let defs = defs
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("expected definitions object"))?;
        ensure!(
            defs.keys().all(|key| definition_name(key)),
            "invalid definition name"
        );
    }
    fn visit<'a>(
        root: &'a Value,
        schema: &'a Value,
        depth: usize,
        nodes: &mut Vec<&'a Value>,
    ) -> Result<()> {
        ensure!(depth <= SCHEMA_DEPTH, "schema depth exceeded");
        ensure!(nodes.len() < SCHEMA_NODES, "schema size exceeded");
        let object = schema
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("expected schema object"))?;
        nodes.push(schema);
        ensure!(
            std::ptr::eq(root, schema) || !object.contains_key("$defs"),
            "definitions must belong to document root"
        );
        if let Some(reference) = schema.get("$ref") {
            let name = reference
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("invalid local ref"))?;
            ensure!(
                definition_name(name) && root["$defs"].get(name).is_some(),
                "unknown local schema ref"
            );
            ensure!(
                object.keys().all(|key| key == "$ref"
                    || key == "nullable"
                    || (std::ptr::eq(root, schema) && key == "$defs")),
                "ref cannot have sibling constraints except nullable"
            );
        } else if let Some(branches) = schema.get("oneOf") {
            let branches = branches
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("invalid oneOf schema"))?;
            ensure!(!branches.is_empty(), "invalid oneOf schema");
            for branch in branches {
                visit(root, branch, depth + 1, nodes)?;
            }
        } else if let Some(choices) = schema.get("enum") {
            let choices = choices
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("invalid schema enum"))?;
            ensure!(
                choices.len() <= SCHEMA_NODES
                    && choices
                        .iter()
                        .all(|value| !value.is_array() && !value.is_object()),
                "expected primitive enum choices"
            );
        } else if schema["type"] == "object" {
            let fields = schema["fields"]
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("invalid schema fields"))?;
            let required = schema["required"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("invalid required fields"))?;
            ensure!(
                required.len() <= SCHEMA_NODES
                    && required
                        .iter()
                        .all(|key| key.as_str().is_some_and(|key| fields.contains_key(key))),
                "invalid required fields"
            );
            for field in fields.values() {
                visit(root, field, depth + 1, nodes)?;
            }
        } else if schema["type"] == "array" {
            ensure!(
                schema["max"]
                    .as_u64()
                    .is_some_and(|max| max <= 9_007_199_254_740_991),
                "invalid array max"
            );
            visit(root, &schema["items"], depth + 1, nodes)?;
        }
        Ok(())
    }
    fn progress(
        root: &Value,
        schema: &Value,
        depth: usize,
        active: &mut HashSet<*const Value>,
        finished: &mut HashSet<*const Value>,
    ) -> Result<()> {
        let pointer = schema as *const Value;
        ensure!(
            !active.contains(&pointer),
            "non-progressing schema ref cycle"
        );
        if finished.contains(&pointer) {
            return Ok(());
        }
        ensure!(depth <= SCHEMA_DEPTH, "schema ref depth exceeded");
        active.insert(pointer);
        if let Some(name) = schema["$ref"].as_str() {
            progress(root, &root["$defs"][name], depth + 1, active, finished)?;
        } else if let Some(branches) = schema["oneOf"].as_array() {
            for branch in branches {
                progress(root, branch, depth + 1, active, finished)?;
            }
        }
        active.remove(&pointer);
        finished.insert(pointer);
        Ok(())
    }
    let mut nodes = Vec::new();
    visit(root, root, 0, &mut nodes)?;
    if let Some(defs) = root["$defs"].as_object() {
        for schema in defs.values() {
            visit(root, schema, 0, &mut nodes)?;
        }
    }
    let mut active = HashSet::new();
    let mut finished = HashSet::new();
    for schema in nodes {
        progress(root, schema, 0, &mut active, &mut finished)?;
    }
    Ok(())
}

struct Validation<'a> {
    document: &'a Value,
    work: usize,
    stack: usize,
    exhausted: bool,
}

pub(super) fn validate(value: &Value, schema: &Value) -> Result<()> {
    schema_document(schema)?;
    validate_data(
        value,
        schema,
        &mut Validation {
            document: schema,
            work: 0,
            stack: 0,
            exhausted: false,
        },
        0,
    )
}

fn validate_data(
    value: &Value,
    schema: &Value,
    context: &mut Validation<'_>,
    depth: usize,
) -> Result<()> {
    context.work += 1;
    context.stack += 1;
    if depth > DATA_DEPTH || context.work > WORK_LIMIT || context.stack > VALIDATION_STACK {
        context.exhausted = true;
        bail!("schema validation budget exceeded");
    }
    let result = validate_data_inner(value, schema, context, depth);
    context.stack -= 1;
    result
}

fn validate_data_inner(
    value: &Value,
    schema: &Value,
    context: &mut Validation<'_>,
    depth: usize,
) -> Result<()> {
    if value.is_null() && schema["nullable"] == true {
        return Ok(());
    }
    if let Some(name) = schema["$ref"].as_str() {
        return validate_data(value, &context.document["$defs"][name], context, depth);
    }
    if let Some(branches) = schema.get("oneOf") {
        let branches = branches
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("invalid oneOf schema"))?;
        ensure!(!branches.is_empty(), "invalid oneOf schema");
        let mut matches = 0;
        for branch in branches {
            matches += usize::from(validate_data(value, branch, context, depth).is_ok());
            ensure!(!context.exhausted, "schema validation budget exceeded");
        }
        ensure!(matches == 1, "expected exactly one matching branch");
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
                validate_data(
                    value,
                    fields
                        .get(key)
                        .ok_or_else(|| anyhow::anyhow!("unknown property: {key}"))?,
                    context,
                    depth + 1,
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
            let mut item_schema = &schema["items"];
            while let Some(name) = item_schema["$ref"].as_str() {
                item_schema = &context.document["$defs"][name];
            }
            for item in items {
                validate_data(item, &schema["items"], context, depth + 1)?;
                if item_schema["fields"].get("id").is_some() {
                    let id = item["id"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("invalid item identity"))?;
                    ensure!(ids.insert(id), "duplicate item identity");
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
    let mut paths = schema["slotIds"].as_str().into_iter().collect::<Vec<_>>();
    if let Some(additional) = schema.get("slotPaths") {
        for path in additional
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("invalid slot paths"))?
        {
            paths.push(
                path.as_str()
                    .ok_or_else(|| anyhow::anyhow!("invalid slot path"))?,
            );
        }
    }
    for path in paths {
        let fields = path.split('.').collect::<Vec<_>>();
        ensure!(
            fields.len() <= 32 && fields.iter().all(|field| definition_name(field)),
            "invalid slot path"
        );
        let mut values = node
            .props
            .get(fields[0])
            .into_iter()
            .flat_map(|value| {
                value
                    .as_array()
                    .map_or(std::slice::from_ref(value), Vec::as_slice)
            })
            .collect::<Vec<_>>();
        for field in fields.iter().skip(1) {
            values = values
                .into_iter()
                .filter_map(|value| value.get(*field))
                .flat_map(|value| {
                    value
                        .as_array()
                        .map_or(std::slice::from_ref(value), Vec::as_slice)
                })
                .collect();
        }
        for id in values.into_iter().filter_map(|item| item["id"].as_str()) {
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
    if super::controls_extra::COMPONENTS.contains(&component) {
        super::controls_extra::validate(node)?;
    }
    if super::navigation_extra::COMPONENTS.contains(&component) {
        super::navigation_extra::validate(node)?;
    }
    if super::layout_extra::COMPONENTS.contains(&component) {
        super::layout_extra::validate(node)?;
    }
    if super::datetime::COMPONENTS.contains(&component) {
        super::datetime::validate(node)?;
    }
    if super::agent::COMPONENTS.contains(&component) {
        super::agent::validate(node)?;
    }
    if super::game_effects::COMPONENTS.contains(&component) {
        super::game_effects::validate(node)?;
    }
    if super::display::COMPONENTS.contains(&component) {
        super::display::validate(node)?;
    }
    if super::charts::COMPONENTS.contains(&component) {
        super::charts::validate(node)?;
    }
    if super::canvas::COMPONENTS.contains(&component) {
        super::canvas::validate_props(node)?;
    }
    if super::overlay_extra::COMPONENTS.contains(&component) {
        super::overlay_extra::validate_props(node)?;
    }
    if super::content::COMPONENTS.contains(&component) {
        super::content::validate_descriptor(node)?;
    }
    if super::media::COMPONENTS.contains(&component) {
        super::media::validate_descriptor(node)?;
    }
    if super::data_extra::COMPONENTS.contains(&component) {
        super::data_extra::validate_props(node)?;
    }
    if super::structured::COMPONENTS.contains(&component) {
        super::structured::validate_props(node)?;
    }
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
    fn local_refs_and_budgets_match_js_cases() {
        let fixtures: Value = serde_json::from_str(include_str!(
            "../../../js-runtime/tests/schema-fixtures.json"
        ))
        .expect("schema parity fixtures");
        for fixture in fixtures["references"]
            .as_array()
            .expect("reference fixtures")
        {
            for case in fixture["cases"].as_array().expect("reference cases") {
                assert_eq!(
                    validate(&case[0], &fixture["schema"]).is_ok(),
                    case[1].as_bool().expect("verdict"),
                    "{}: {}",
                    fixture["name"],
                    case[0]
                );
            }
        }
        for schema in fixtures["invalidReferences"]
            .as_array()
            .expect("invalid references")
        {
            assert!(validate(&Value::Null, schema).is_err(), "{schema}");
        }
        let mut value = Value::Null;
        for _ in 0..32 {
            value = json!({"next": value});
        }
        assert!(validate(&value, &fixtures["depthSchema"]).is_ok());
        assert!(validate(&json!({"next": value}), &fixtures["depthSchema"]).is_err());
        // Root = 1; each item = union + ref + boolean + failed null branch.
        assert!(validate(&json!(vec![true; 24_999]), &fixtures["workSchema"]).is_ok());
        let error = validate(&json!(vec![true; 25_000]), &fixtures["workSchema"])
            .expect_err("aggregate budget");
        assert!(error.to_string().contains("budget"));
    }

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
