//! What a structured surface is allowed to claim: `JsonView` and `SchemaForm`.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext, IntoElement, SharedString, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit_semantics::{Node, Role, Snapshot};
use gpui_kit_testkit::harness::Harness;

// ------------------------------------------------------------------ documents

/// A document holding the three facts a viewer usually confuses, plus a
/// subtree the caller withheld.
fn document() -> JsonValue {
    JsonValue::object([
        ("id", JsonValue::string("run-4812")),
        ("attempts", JsonValue::number("3")),
        // Recorded as nothing, which is not the same as never recorded: there
        // is no `resumed_from` key at all.
        ("cursor", JsonValue::Null),
        ("labels", JsonValue::object(Vec::<(&str, JsonValue)>::new())),
        (
            "credentials",
            JsonValue::object([("token", JsonValue::redacted("51 characters"))]),
        ),
        (
            "steps",
            JsonValue::array([JsonValue::string("plan"), JsonValue::string("apply")]),
        ),
    ])
}

fn view(expanded: &'static [&'static str]) -> JsonView {
    JsonView::new("json", document()).expanded_paths(expanded)
}

#[gpui::test]
fn null_an_empty_object_and_an_absent_key_are_three_presentations(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| view(&[]).into_any_element());

    let null = harness.node("json.cursor").expect("a null key is a row");
    assert_eq!(null.role, Role::TreeItem);
    assert_eq!(null.value.as_deref(), Some("null"));
    assert_eq!(
        null.expanded, None,
        "a scalar has nothing to disclose, so it claims no disclosure state"
    );

    let empty = harness
        .node("json.labels")
        .expect("an empty object is a row");
    assert_eq!(empty.value.as_deref(), Some("empty object"));
    assert_eq!(
        empty.expanded, None,
        "an empty object must not look like one that is merely shut"
    );

    assert!(
        harness.node("json.resumed_from").is_none(),
        "a key the document does not hold produces no row at all"
    );

    let populated = harness.node("json.steps").expect("published");
    assert_eq!(populated.value.as_deref(), Some("array"));
    assert_eq!(
        populated.expanded,
        Some(false),
        "an array with items in it is shut, which is a different fact from empty"
    );
}

#[gpui::test]
fn a_withheld_subtree_reads_as_withheld_and_leaks_nothing(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        view(&["credentials"]).into_any_element()
    });

    let secret = harness
        .node("json.credentials/token")
        .expect("a withheld value keeps its row");
    assert_eq!(
        secret.value.as_deref(),
        Some("withheld"),
        "a hidden value must read as withheld, not as absent or empty"
    );
    assert_eq!(secret.text.as_deref(), Some("token"));

    let snapshot: Snapshot = harness.snapshot();
    let leaked: Vec<&Node> = snapshot
        .nodes
        .iter()
        .filter(|node| {
            let carries = |text: &Option<String>| {
                text.as_deref()
                    .is_some_and(|text| text.contains("51 characters"))
            };
            carries(&node.text) || carries(&node.value)
        })
        .collect();
    assert!(
        leaked.is_empty(),
        "not even the shape of a withheld value belongs in a snapshot: {leaked:?}"
    );
}

#[gpui::test]
fn a_shut_value_publishes_none_of_what_is_under_it(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| view(&[]).into_any_element());

    assert!(harness.node("json.steps").is_some());
    assert!(
        harness.node("json.steps/0").is_none(),
        "a shut array must not lay out or publish its items"
    );

    let container = harness.node("json").expect("published");
    assert_eq!(container.role, Role::Tree);
    assert_eq!(
        container.value.as_deref(),
        Some("6"),
        "the container states how many rows it disclosed"
    );
}

#[gpui::test]
fn opening_a_value_reports_the_path_and_applies_nothing(cx: &mut TestAppContext) {
    let calls: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        view(&[])
            .on_toggle(move |path, open, _, _| sink.borrow_mut().push(format!("{path}:{open}")))
            .into_any_element()
    });

    harness.click("json.steps.toggle");
    assert_eq!(calls.borrow().as_slice(), ["steps:true"]);
    assert!(
        harness.node("json.steps/0").is_none(),
        "the view opens nothing itself: the caller owns the disclosure"
    );
}

#[gpui::test]
fn a_refused_view_installs_no_handler(cx: &mut TestAppContext) {
    let calls: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        view(&[])
            .disabled(true)
            .on_select(move |path, _, _| sink.borrow_mut().push(path.to_string()))
            .into_any_element()
    });

    harness.click("json.cursor");
    assert!(calls.borrow().is_empty());
}

#[gpui::test]
fn a_document_that_is_one_scalar_is_still_one_row(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        JsonView::new("json", JsonValue::Null)
            .root_label("Answer")
            .into_any_element()
    });

    let row = harness.node("json.value").expect("published");
    assert_eq!(row.text.as_deref(), Some("Answer"));
    assert_eq!(row.value.as_deref(), Some("null"));
}

// ---------------------------------------------------------------------- forms

fn schema() -> Schema {
    Schema::new()
        .field(
            SchemaField::new(
                "path",
                SchemaKind::Text {
                    placeholder: None,
                    secret: false,
                },
            )
            .label("File")
            .required(true),
        )
        .field(
            SchemaField::new(
                "max_bytes",
                SchemaKind::Integer(NumberBounds::new().min(1.0)),
            )
            .label("Maximum bytes"),
        )
        .field(SchemaField::new("ratio", SchemaKind::Number(NumberBounds::new())).label("Ratio"))
        .field(SchemaField::new("follow", SchemaKind::Boolean).label("Follow links"))
        .field(
            SchemaField::new(
                "encoding",
                SchemaKind::Enum(vec![
                    SchemaChoice::new("utf-8", "UTF-8"),
                    SchemaChoice::new("latin-1", "Latin-1"),
                ]),
            )
            .label("Encoding"),
        )
        .field(
            SchemaField::new(
                "profile",
                SchemaKind::OpenEnum(vec![SchemaChoice::new("fast", "Fast")]),
            )
            .label("Profile"),
        )
        .field(SchemaField::new("tags", SchemaKind::TextList { max: None }).label("Tags"))
        .field(
            SchemaField::new(
                "limits",
                SchemaKind::Object(vec![
                    SchemaField::new("timeout_ms", SchemaKind::Integer(NumberBounds::new()))
                        .label("Timeout"),
                ]),
            )
            .label("Limits"),
        )
}

fn form_harness(cx: &mut TestAppContext, schema: Schema) -> (Harness, gpui::Entity<SchemaForm>) {
    let held: Rc<RefCell<Option<gpui::Entity<SchemaForm>>>> = Rc::new(RefCell::new(None));
    let sink = Rc::clone(&held);
    let harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let mut slot = sink.borrow_mut();
        let form = slot
            .get_or_insert_with(|| cx.new(|cx| SchemaForm::new("form", schema.clone(), window, cx)))
            .clone();
        form.into_any_element()
    });
    let form = held.borrow().clone().expect("built");
    (harness, form)
}

#[gpui::test]
fn each_schema_type_produces_the_control_it_should(cx: &mut TestAppContext) {
    let (mut harness, _form) = form_harness(cx, schema());

    assert_eq!(
        harness.node("form.path.control").map(|node| node.role),
        Some(Role::Input),
        "a string is a text field"
    );
    assert_eq!(
        harness.node("form.max_bytes.control").map(|node| node.role),
        Some(Role::Input),
        "an integer is a number field"
    );
    assert_eq!(
        harness.node("form.ratio.control").map(|node| node.role),
        Some(Role::Input),
        "a number is a number field"
    );
    assert_eq!(
        harness.node("form.follow.control").map(|node| node.role),
        Some(Role::Switch),
        "a boolean is a switch"
    );
    assert_eq!(
        harness.node("form.encoding.control").map(|node| node.role),
        Some(Role::Combobox),
        "a closed choice is a select"
    );
    assert!(
        harness.node("form.encoding.control.query").is_none(),
        "a closed choice cannot be typed into"
    );
    assert_eq!(
        harness.node("form.profile.control").map(|node| node.role),
        Some(Role::Combobox),
    );
    assert!(
        harness.node("form.profile.control.query").is_some(),
        "an open choice can be typed into"
    );
    assert_eq!(
        harness.node("form.tags.control").map(|node| node.role),
        Some(Role::Group),
        "a list of strings is a tag field"
    );
    assert_eq!(
        harness.node("form.limits").map(|node| node.role),
        Some(Role::Group),
        "a nested object is a heading over its own fields"
    );
    assert!(
        harness.node("form.limits/timeout_ms.control").is_some(),
        "a nested field keeps its place under the object that holds it"
    );
    assert_eq!(
        harness
            .node("form.path")
            .and_then(|node| node.required.then_some(true)),
        Some(true),
        "a required field says so"
    );
}

#[gpui::test]
fn a_field_the_form_cannot_draw_is_reported_rather_than_dropped(cx: &mut TestAppContext) {
    let schema = Schema::new()
        .field(
            SchemaField::new(
                "name",
                SchemaKind::Text {
                    placeholder: None,
                    secret: false,
                },
            )
            .label("Name"),
        )
        .field(
            SchemaField::new(
                "matcher",
                SchemaKind::Unrenderable("This argument is three shapes at once.".into()),
            )
            .label("Matcher"),
        )
        // The form refuses this one on its own: a choice among nothing is not
        // a control, and an empty menu would read as a list that had not
        // loaded.
        .field(SchemaField::new("mode", SchemaKind::Enum(Vec::new())).label("Mode"));

    let (mut harness, form) = form_harness(cx, schema);

    let field = harness
        .node("form.matcher")
        .expect("the field keeps its place");
    assert_eq!(field.text.as_deref(), Some("Matcher"));
    let refusal = harness
        .node("form.matcher.unrenderable")
        .expect("published");
    assert_eq!(refusal.role, Role::Status);
    assert!(refusal.invalid);
    assert!(!refusal.required);
    assert_eq!(refusal.value.as_deref(), Some("unrenderable"));
    assert_eq!(
        refusal.text.as_deref(),
        Some("This argument is three shapes at once."),
        "the host's reason is shown word for word"
    );

    assert!(
        harness.node("form.mode.unrenderable").is_some(),
        "a choice among no choices is refused by the form itself"
    );

    let summary = harness.node("form.unrenderable").expect("published");
    assert_eq!(summary.value.as_deref(), Some("unrenderable"));

    harness.update(|_, cx| {
        let form = form.read(cx);
        let reported: Vec<&str> = form
            .unrenderable()
            .iter()
            .map(|field| field.path.as_ref())
            .collect();
        assert_eq!(reported, vec!["matcher", "mode"]);
        assert!(!form.has_unrenderable_required());
        let values: Vec<(SharedString, FieldValue)> = form.values(cx);
        assert!(
            values
                .iter()
                .any(|(path, value)| path == "matcher" && *value == FieldValue::Unrenderable),
            "a field the form could not draw is still reported as one of the answers"
        );
    });
}

#[gpui::test]
fn a_required_field_the_form_cannot_draw_is_reported_loudly(cx: &mut TestAppContext) {
    let schema = Schema::new().field(
        SchemaField::new(
            "matcher",
            SchemaKind::Unrenderable("No single control stands for this.".into()),
        )
        .label("Matcher")
        .required(true),
    );
    let (mut harness, form) = form_harness(cx, schema);

    let refusal = harness
        .node("form.matcher.unrenderable")
        .expect("published");
    assert!(refusal.required);
    assert_eq!(refusal.value.as_deref(), Some("unrenderable, required"));

    let summary = harness.node("form.unrenderable").expect("published");
    assert!(summary.required);
    assert_eq!(summary.value.as_deref(), Some("unrenderable, required"));

    harness.update(|_, cx| {
        assert!(form.read(cx).has_unrenderable_required());
        form.update(cx, |form, cx| {
            assert!(
                !form.validate(cx),
                "a form holding a required field it cannot draw is never answerable"
            );
        });
    });
}

#[gpui::test]
fn a_host_error_appears_next_to_its_field(cx: &mut TestAppContext) {
    let (mut harness, form) = form_harness(cx, schema());

    assert!(harness.node("form.path.error").is_none());

    harness.update(|_, cx| {
        form.update(cx, |form, cx| {
            form.set_error("path", "That path is outside the workspace.", cx);
        });
    });
    harness.frame();

    let error = harness.node("form.path.error").expect("published");
    assert_eq!(
        error.text.as_deref(),
        Some("That path is outside the workspace."),
        "the host's words are shown, not rewritten"
    );
    assert!(harness.node("form.path").expect("published").invalid);
    assert!(
        harness.node("form.max_bytes.error").is_none(),
        "an error belongs to one field and stays there"
    );
}

#[gpui::test]
fn the_form_can_also_find_a_missing_required_field_itself(cx: &mut TestAppContext) {
    let (mut harness, form) = form_harness(cx, schema());

    harness.update(|_, cx| {
        form.update(cx, |form, cx| {
            assert!(!form.validate(cx), "nothing has been filled in");
        });
    });
    harness.frame();

    let error = harness.node("form.path.error").expect("published");
    assert_eq!(error.text.as_deref(), Some("This field is required."));
}

#[gpui::test]
fn a_number_outside_the_schema_range_is_explained_and_blocks_the_form(cx: &mut TestAppContext) {
    let (mut harness, form) = form_harness(cx, schema());

    harness.click("form.max_bytes.control.field");
    harness.keystrokes("0");
    harness.frame();

    let error = harness
        .node("form.max_bytes.error")
        .expect("a field drawn as wrong says why");
    assert_eq!(
        error.text.as_deref(),
        Some("The smallest accepted value is 1."),
    );

    harness.update(|_, cx| {
        form.update(cx, |form, cx| {
            assert!(
                !form.validate(cx),
                "a form cannot call itself answerable while a field on it is red"
            );
        });
    });
}

#[gpui::test]
fn an_untouched_field_holds_nothing_rather_than_an_empty_string(cx: &mut TestAppContext) {
    let (mut harness, form) = form_harness(cx, schema());

    harness.update(|_, cx| {
        let values = form.read(cx).values(cx);
        let path = values
            .iter()
            .find(|(path, _)| path == "path")
            .map(|(_, value)| value.clone());
        assert_eq!(path, Some(FieldValue::Absent));
        let follow = values
            .iter()
            .find(|(path, _)| path == "follow")
            .map(|(_, value)| value.clone());
        assert_eq!(
            follow,
            Some(FieldValue::Boolean(false)),
            "a switch always holds one of two answers"
        );
    });
}
