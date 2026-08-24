//! A schema, rendered as data or as a form.

use super::support::*;

/// A document with the three facts a viewer usually confuses: a key holding
/// `null`, a key holding an empty object, and a key that is simply not here.
pub(super) fn scene_document() -> JsonValue {
    JsonValue::object([
        ("id", JsonValue::string("run-4812")),
        ("attempts", JsonValue::number("3")),
        ("streaming", JsonValue::Bool(true)),
        // Nothing has been recorded here yet, which is not the same as the
        // key being missing: `resumed_from` is missing, and says nothing.
        ("cursor", JsonValue::Null),
        ("labels", JsonValue::object(Vec::<(&str, JsonValue)>::new())),
        (
            "credentials",
            JsonValue::object([("token", JsonValue::redacted("51 characters"))]),
        ),
        (
            "request",
            JsonValue::object([
                ("method", JsonValue::string("POST")),
                (
                    "headers",
                    JsonValue::object([
                        ("content-type", JsonValue::string("application/json")),
                        ("authorization", JsonValue::redacted("a value")),
                    ]),
                ),
            ]),
        ),
        (
            "steps",
            JsonValue::array([
                JsonValue::string("plan"),
                JsonValue::string("apply"),
                JsonValue::object([("retries", JsonValue::number("0"))]),
            ]),
        ),
    ])
}

pub(super) fn json_view(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(
            JsonView::new("scene.json", scene_document())
                .expanded_paths(&["credentials", "request", "request/headers", "steps"])
                .selected("request/method")
                .on_toggle(|_, _, _, _| {})
                .on_select(|_, _, _| {}),
        )
        .into_any_element()
}

/// The form kept across frames.
///
/// Its fields own carets, open menus and a selection, so rebuilding it every
/// frame would throw away whatever had been typed into it.
pub(super) struct SceneSchemaForm {
    form: Entity<SchemaForm>,
}

impl Global for SceneSchemaForm {}

pub(super) fn scene_schema() -> Schema {
    Schema::new()
        .field(
            SchemaField::new(
                "path",
                SchemaKind::Text {
                    placeholder: Some("relative to the workspace".into()),
                    secret: false,
                },
            )
            .label("File")
            .description("Which file the call reads")
            .required(true),
        )
        .field(
            SchemaField::new(
                "max_bytes",
                SchemaKind::Integer(NumberBounds::new().min(1.0).max(65_536.0).step(1024.0)),
            )
            .label("Maximum bytes"),
        )
        .field(
            SchemaField::new("follow_symlinks", SchemaKind::Boolean).label("Follow symbolic links"),
        )
        .field(
            SchemaField::new(
                "encoding",
                SchemaKind::Enum(vec![
                    SchemaChoice::new("utf-8", "UTF-8"),
                    SchemaChoice::new("latin-1", "Latin-1"),
                ]),
            )
            .label("Encoding")
            .required(true),
        )
        .field(
            SchemaField::new("attachments", SchemaKind::Files { max: Some(2) })
                .label("Attachments")
                .description("Paths are acquired and accepted by the host"),
        )
        .field(
            SchemaField::new(
                "headers",
                SchemaKind::List {
                    item: Box::new(SchemaField::new(
                        "header",
                        SchemaKind::Object(vec![
                            SchemaField::new(
                                "name",
                                SchemaKind::Text {
                                    placeholder: Some("Header name".into()),
                                    secret: false,
                                },
                            )
                            .label("Name")
                            .required(true),
                            SchemaField::new("enabled", SchemaKind::Boolean).label("Enabled"),
                        ]),
                    )),
                    max: Some(3),
                },
            )
            .label("Headers"),
        )
        .field(
            SchemaField::new(
                "limits",
                SchemaKind::Object(vec![
                    SchemaField::new("timeout_ms", SchemaKind::Integer(NumberBounds::new()))
                        .label("Timeout in milliseconds"),
                ]),
            )
            .label("Limits"),
        )
        .field(
            SchemaField::new(
                "matcher",
                SchemaKind::Unrenderable(
                    "This argument is one of three shapes at once, and no single control \
                     stands for that."
                        .into(),
                ),
            )
            .label("Matcher")
            .required(true),
        )
}

pub(super) fn schema_form(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneSchemaForm>() {
        set_schema_file_policy(DefaultSchemaFilePolicy, cx);
        let form = cx.new(|cx| SchemaForm::new("scene.schema", scene_schema(), window, cx));
        form.update(cx, |form, cx| {
            // An error the host returned rather than one the form derived:
            // only the host knows this path is outside the workspace.
            form.set_error("path", "That path is outside the workspace.", cx);
            let _ = form.set_field_validation("encoding", ValidationState::Validating, cx);
            form.set_files("attachments", vec!["/workspace/brief.md".into()], cx)
                .expect("the scene's permissive file policy accepts its fixture");
            form.add_list_item("headers", window, cx);
        });
        cx.set_global(SceneSchemaForm { form });
    }
    let form = cx.global::<SceneSchemaForm>().form.clone();
    let theme = cx.theme().clone();
    stack(&theme).w(px(520.0)).child(form).into_any_element()
}
