//! Retained native structured forms and caller-owned JSON documents.
use super::*;
use anyhow::{Result, bail, ensure};
use gpui_kit::state::ValidationState;

pub(super) const COMPONENTS: &[&str] = &["JsonView", "SchemaForm"];

/// Semantic checks after the shared closed-data grammar, before native conversion.
pub(super) fn validate_props(node: &Node) -> Result<()> {
    fn number(text: &str) -> bool {
        let mut chars = text.bytes().peekable();
        if chars.peek() == Some(&b'-') {
            chars.next();
        }
        match chars.next() {
            Some(b'0') => {}
            Some(b'1'..=b'9') => {
                while chars.peek().is_some_and(u8::is_ascii_digit) {
                    chars.next();
                }
            }
            _ => return false,
        }
        if chars.peek() == Some(&b'.') {
            chars.next();
            let mut count = 0;
            while chars.peek().is_some_and(u8::is_ascii_digit) {
                chars.next();
                count += 1;
            }
            if count == 0 {
                return false;
            }
        }
        if chars.peek().is_some_and(|c| *c == b'e' || *c == b'E') {
            chars.next();
            if chars.peek().is_some_and(|c| *c == b'+' || *c == b'-') {
                chars.next();
            }
            let mut count = 0;
            while chars.peek().is_some_and(u8::is_ascii_digit) {
                chars.next();
                count += 1;
            }
            if count == 0 {
                return false;
            }
        }
        chars.next().is_none()
    }
    fn json(v: &Value) -> Result<()> {
        if v["kind"] == "number" {
            ensure!(number(&string(v, "text")), "invalid JSON number spelling");
        }
        for child in array(v, "items") {
            json(child)?;
        }
        for member in array(v, "members") {
            json(&member["value"])?;
        }
        Ok(())
    }
    fn fields(values: &[Value]) -> Result<()> {
        let mut names = std::collections::HashSet::new();
        for v in values {
            let name = string(v, "name");
            ensure!(
                names.insert(name.clone()) && !name.contains(['.', '[', ']', '/']),
                "duplicate or ambiguous schema field name"
            );
            if let (Some(min), Some(max)) = (v["min"].as_f64(), v["max"].as_f64()) {
                ensure!(min <= max, "reversed number bounds");
            }
            if let Some(children) = v["fields"].as_array() {
                fields(children)?;
            }
            if v.get("item").is_some() {
                fields(std::slice::from_ref(&v["item"]))?;
            }
        }
        Ok(())
    }
    match node.component.as_deref() {
        Some("JsonView") => json(node.props.get("value").unwrap_or(&Value::Null)),
        Some("SchemaForm") => fields(
            node.props
                .get("fields")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default(),
        ),
        _ => Ok(()),
    }
}

fn string(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_owned()
}
fn array<'a>(v: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    v[key].as_array().into_iter().flatten()
}
fn json_value(v: &Value) -> JsonValue {
    match v["kind"].as_str() {
        Some("boolean") => JsonValue::Bool(v["boolean"].as_bool().unwrap_or(false)),
        Some("number") => JsonValue::number(string(v, "text")),
        Some("string") => JsonValue::string(string(v, "text")),
        Some("redacted") => JsonValue::redacted(string(v, "text")),
        Some("array") => JsonValue::array(array(v, "items").map(json_value)),
        Some("object") => JsonValue::object(
            array(v, "members").map(|m| (string(m, "key"), json_value(&m["value"]))),
        ),
        Some("identifiedObject") => {
            JsonValue::identified_object(array(v, "members").map(|m| {
                JsonMember::new(string(m, "id"), string(m, "key"), json_value(&m["value"]))
            }))
        }
        _ => JsonValue::Null,
    }
}
fn json_view(node: &Node, emit: Emit) -> JsonView {
    let mut c = JsonView::new(
        node.id.clone(),
        json_value(node.props.get("value").unwrap_or(&Value::Null)),
    )
    .disabled(flag(node, "disabled"))
    .control_size(size(node));
    if node.props.contains_key("rootLabel") {
        c = c.root_label(text(node, "rootLabel"));
    }
    if node.props.contains_key("selected") {
        c = c.selected(text(node, "selected"));
    }
    if let Some(v) = node.props.get("expanded").and_then(Value::as_array) {
        c = c.expanded(
            v.iter()
                .filter_map(Value::as_str)
                .map(|s| SharedString::from(s.to_owned())),
        );
    }
    if node.props.contains_key("rowHeight") {
        c = c.row_height(number(node, "rowHeight", 32.));
    }
    if node.props.contains_key("visibleRows") {
        c = c.visible_rows(number(node, "visibleRows", 10.) as usize);
    }
    if !flag(node, "disabled") {
        if let Some(a) = node.events.get("toggle").cloned() {
            let e = emit.clone();
            c = c.on_toggle(move |path, expanded, _, _| {
                e(&a, json!({"path":path.as_ref(),"expanded":expanded}))
            });
        }
        if let Some(a) = node.events.get("select").cloned() {
            c = c.on_select(move |path, _, _| emit(&a, json!(path.as_ref())));
        }
    }
    c
}
fn field(v: &Value) -> SchemaField {
    let max = v["maxItems"].as_u64().map(|n| n as usize);
    let choices = || {
        array(v, "choices")
            .map(|v| {
                let mut c = SchemaChoice::new(string(v, "id"), string(v, "label"));
                if v["description"].is_string() {
                    c = c.description(string(v, "description"));
                }
                c
            })
            .collect()
    };
    let kind = match v["kind"].as_str() {
        Some("text") => SchemaKind::Text {
            placeholder: v["placeholder"].as_str().map(|s| s.to_owned().into()),
            secret: v["secret"].as_bool().unwrap_or(false),
        },
        Some("number" | "integer") => {
            let mut b = NumberBounds::new();
            if let Some(n) = v["min"].as_f64() {
                b = b.min(n);
            }
            if let Some(n) = v["max"].as_f64() {
                b = b.max(n);
            }
            if let Some(n) = v["step"].as_f64() {
                b = b.step(n);
            }
            if v["kind"] == "integer" {
                SchemaKind::Integer(b)
            } else {
                SchemaKind::Number(b)
            }
        }
        Some("boolean") => SchemaKind::Boolean,
        Some("enum") => SchemaKind::Enum(choices()),
        Some("openEnum") => SchemaKind::OpenEnum(choices()),
        Some("textList") => SchemaKind::TextList { max },
        Some("object") => SchemaKind::Object(array(v, "fields").map(field).collect()),
        Some("list") => SchemaKind::List {
            item: Box::new(field(&v["item"])),
            max,
        },
        Some("date") => SchemaKind::Date,
        Some("time") => SchemaKind::Time,
        Some("dateRange") => SchemaKind::DateRange,
        Some("files") => SchemaKind::Files { max },
        _ => SchemaKind::Unrenderable(string(v, "reason").into()),
    };
    let mut f = SchemaField::new(string(v, "name"), kind)
        .required(v["required"].as_bool().unwrap_or(false));
    if v["label"].is_string() {
        f = f.label(string(v, "label"));
    }
    if v["description"].is_string() {
        f = f.description(string(v, "description"));
    }
    f
}
fn validation(v: &Value) -> ValidationState {
    match v["state"].as_str() {
        Some("valid") => ValidationState::Valid,
        Some("validating") => ValidationState::Validating,
        Some("invalid") => ValidationState::invalid(string(v, "reason")),
        _ => ValidationState::Pending,
    }
}
fn validation_value(v: &ValidationState) -> Value {
    json!({"state":v.name(),"reason":v.reason().map(|s|s.as_ref())})
}
fn visibility(v: &Value) -> FieldVisibility {
    match v.as_str() {
        Some("hiddenInclude") => FieldVisibility::Hidden {
            submission: HiddenSubmission::Include,
        },
        Some("hiddenOmit") => FieldVisibility::Hidden {
            submission: HiddenSubmission::Omit,
        },
        _ => FieldVisibility::Visible,
    }
}
fn visibility_value(v: FieldVisibility) -> Value {
    json!(match v {
        FieldVisibility::Visible => "visible",
        FieldVisibility::Hidden {
            submission: HiddenSubmission::Include,
        } => "hiddenInclude",
        FieldVisibility::Hidden {
            submission: HiddenSubmission::Omit,
        } => "hiddenOmit",
    })
}
fn field_value(v: FieldValue) -> Value {
    match v {
        FieldValue::Text(v) => json!({"kind":"text","text":v.as_ref()}),
        FieldValue::Number(v) => json!({"kind":"number","number":v}),
        FieldValue::Boolean(v) => json!({"kind":"boolean","boolean":v}),
        FieldValue::Choice(v) => json!({"kind":"choice","text":v.as_ref()}),
        FieldValue::List(v) => {
            json!({"kind":"list","items":v.iter().map(|s|s.as_ref()).collect::<Vec<_>>()})
        }
        FieldValue::Files(v) => {
            json!({"kind":"files","items":v.iter().map(|p|p.to_string_lossy()).collect::<Vec<_>>()})
        }
        FieldValue::ItemCount(v) => json!({"kind":"itemCount","number":v}),
        FieldValue::Day(v) => json!({"kind":"day","number":v}),
        FieldValue::Time {
            hour,
            minute,
            second,
        } => json!({"kind":"time","hour":hour,"minute":minute,"second":second}),
        FieldValue::Range { start, end } => json!({"kind":"range","start":start,"end":end}),
        FieldValue::Absent => json!({"kind":"absent"}),
        FieldValue::Unrenderable => json!({"kind":"unrenderable"}),
    }
}
struct Form {
    entity: Entity<SchemaForm>,
    schema: Value,
    props: RefCell<serde_json::Map<String, Value>>,
    route: Rc<RefCell<Route>>,
    _subscription: Subscription,
}
#[derive(Default)]
pub(super) struct State {
    forms: RefCell<HashMap<Key, Rc<Form>>>,
}
impl State {
    pub(super) fn reconcile(&self, root: &Node, _cx: &mut App) {
        fn visit(n: &Node, live: &mut HashMap<Key, bool>) {
            if n.component.as_deref() == Some("SchemaForm") {
                live.insert((n.instance, n.id.clone()), true);
            }
            for n in n.children.iter().chain(n.slots.values().flatten()) {
                visit(n, live);
            }
        }
        let mut live = HashMap::new();
        visit(root, &mut live);
        self.forms.borrow_mut().retain(|k, _| live.contains_key(k));
    }
    pub(super) fn render(
        &self,
        node: &Node,
        _slots: KitSlots,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        if node.component.as_deref() == Some("JsonView") {
            return json_view(node, emit).into_any_element();
        }
        let key = (node.instance, node.id.clone());
        let schema = node.props.get("fields").cloned().unwrap_or(json!([]));
        let existing = self
            .forms
            .borrow()
            .get(&key)
            .filter(|f| f.schema == schema)
            .cloned();
        let entry = existing.unwrap_or_else(|| {
            let fields = schema
                .as_array()
                .into_iter()
                .flatten()
                .map(field)
                .collect::<Vec<_>>();
            let entity = cx.new(|cx| {
                SchemaForm::new(node.id.clone(), Schema::new().fields(fields), window, cx)
            });
            let route = Rc::new(RefCell::new(Route {
                events: node.events.clone(),
                emit: emit.clone(),
                disabled: flag(node, "disabled"),
            }));
            let callback = route.clone();
            let subscription = cx.subscribe(&entity, move |_, event: &SchemaFormEvent, _| {
                let (name, value) = match event {
                    SchemaFormEvent::Changed(path) => ("change", json!(path.as_ref())),
                    SchemaFormEvent::Submitted => ("submit", Value::Null),
                    SchemaFormEvent::FilesRequested(r) => (
                        "filesRequested",
                        json!({"path":r.path.as_ref(),"label":r.label.as_ref(),"max":r.max}),
                    ),
                };
                // Clone route before calling code which can synchronously rerender.
                let route = callback.borrow();
                let action = route.events.get(name).cloned().filter(|_| !route.disabled);
                let emit = route.emit.clone();
                drop(route);
                if let Some(action) = action {
                    emit(&action, value);
                }
            });
            let entry = Rc::new(Form {
                entity,
                schema,
                props: RefCell::new(Default::default()),
                route,
                _subscription: subscription,
            });
            self.forms.borrow_mut().insert(key, entry.clone());
            entry
        });
        {
            let mut route = entry.route.borrow_mut();
            route.events = node.events.clone();
            route.emit = emit;
            if *entry.props.borrow() != node.props {
                route.disabled = flag(node, "disabled");
            }
        }
        if *entry.props.borrow() != node.props {
            entry
                .entity
                .update(cx, |form, cx| form.set_disabled(flag(node, "disabled"), cx));
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }
    pub(super) fn invoke(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Value> {
        if node.component.as_deref() == Some("JsonView") {
            ensure!(
                query && method == "disclosed_paths",
                "unsupported JsonView method"
            );
            return Ok(json!(
                json_view(node, Rc::new(|_, _| {}))
                    .disclosed_paths(cx)
                    .iter()
                    .map(|s| s.as_ref())
                    .collect::<Vec<_>>()
            ));
        }
        let entry = self
            .forms
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("SchemaForm is not mounted"))?;
        ensure!(
            query || !entry.route.borrow().disabled,
            "disabled SchemaForm refuses commands"
        );
        let path = string(args, "path");
        if query {
            let form = entry.entity.read(cx);
            return Ok(match method {
                "field_validation"=>form.field_validation(&path,cx).as_ref().map(validation_value).unwrap_or(Value::Null),
                "field_visibility"=>form.field_visibility(&path,cx).map(visibility_value).unwrap_or(Value::Null),
                "validation"=>form.validation().map(validation_value).unwrap_or(Value::Null),
                "values"|"submission_values"=>json!(if method=="values" {form.values(cx)} else {form.submission_values(cx)}.into_iter().map(|(p,v)|json!({"path":p.as_ref(),"value":field_value(v)})).collect::<Vec<_>>()),
                "unrenderable"=>json!(form.unrenderable().iter().map(|v|json!({"path":v.path.as_ref(),"label":v.label.as_ref(),"required":v.required,"reason":v.reason.as_ref()})).collect::<Vec<_>>()),
                "has_unrenderable_required"=>json!(form.has_unrenderable_required()),
                _=>bail!("unsupported SchemaForm query"),
            });
        }
        entry.entity.update(cx, |form, cx| {
            Ok(match method {
                "set_files" => json!(
                    form.set_files(
                        path,
                        array(args, "files")
                            .filter_map(Value::as_str)
                            .map(std::path::PathBuf::from)
                            .collect(),
                        cx
                    )
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?
                ),
                "add_list_item" => json!(form.add_list_item(path, window, cx)),
                "remove_list_item" => json!(form.remove_list_item(
                    path,
                    args["index"].as_u64().unwrap_or(0) as usize,
                    cx
                )),
                "move_list_item" => json!(form.move_list_item(
                    path,
                    args["from"].as_u64().unwrap_or(0) as usize,
                    args["to"].as_u64().unwrap_or(0) as usize,
                    cx
                )),
                "set_field_validation" => {
                    json!(form.set_field_validation(path, validation(&args["validation"]), cx))
                }
                "clear_field_validation" => json!(form.clear_field_validation(path, cx)),
                "set_field_visibility" => {
                    json!(form.set_field_visibility(path, visibility(&args["visibility"]), cx))
                }
                "set_validation" => {
                    form.set_validation(validation(&args["validation"]), cx);
                    Value::Null
                }
                "clear_validation" => {
                    form.clear_validation(cx);
                    Value::Null
                }
                "set_error" => {
                    form.set_error(path, string(args, "message"), cx);
                    Value::Null
                }
                "clear_host_errors" => {
                    form.clear_host_errors(cx);
                    Value::Null
                }
                "validate" => json!(form.validate(cx)),
                "set_disabled" => {
                    let disabled = args["disabled"].as_bool().unwrap_or(false);
                    form.set_disabled(disabled, cx);
                    entry.route.borrow_mut().disabled = disabled;
                    Value::Null
                }
                _ => bail!("unsupported SchemaForm command"),
            })
        })
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests;
