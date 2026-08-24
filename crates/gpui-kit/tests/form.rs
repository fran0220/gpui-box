//! The form family: a field says what is wrong, a number field refuses to
//! lie about its range, a strip reports a choice, a combobox filters without
//! deciding, and a tag field refuses out loud.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext as _, Entity, InteractiveElement, IntoElement, ParentElement, SharedString,
    TestAppContext, div, prelude::*, px, size,
};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

/// Collects the events a view reports, the way an owning host would.
fn record<T: 'static + Clone, V: 'static + gpui::EventEmitter<T>>(
    harness: &mut Harness,
    entity: &Entity<V>,
) -> Rc<RefCell<Vec<T>>> {
    let events: Rc<RefCell<Vec<T>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let entity = entity.clone();
    harness.update(move |_, cx| {
        cx.subscribe(&entity, move |_, event: &T, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    events
}

// -- FormField ------------------------------------------------------------

fn field(
    cx: &mut TestAppContext,
    configure: impl Fn(FormField) -> FormField + 'static,
) -> (Harness, Rc<RefCell<Option<Entity<TextInput>>>>) {
    let slot: Rc<RefCell<Option<Entity<TextInput>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let input = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| TextInput::new("workspace.name", window, cx).text("Runs 2024"))
            })
            .clone();
        configure(
            FormField::new("workspace.name.field", "Workspace name").control("workspace.name"),
        )
        .child(input)
        .into_any_element()
    });
    (harness, slot)
}

#[gpui::test]
fn a_label_names_the_control_it_belongs_to(cx: &mut TestAppContext) {
    let (mut harness, _slot) = field(cx, |field| field);
    let label = harness
        .node("workspace.name.field.label")
        .expect("the label is published");

    assert_eq!(label.text.as_deref(), Some("Workspace name"));
    assert_eq!(
        label.labels.as_deref(),
        Some("workspace.name"),
        "a test that knows only the label must be able to reach the control"
    );
    assert!(harness.node("workspace.name").is_some());

    let tree = harness.accessibility_tree();
    let nodes = tree["nodes"].as_object().expect("native nodes");
    let label_id = nodes
        .iter()
        .find(|(_, node)| node["element_id"] == "Name(\"workspace.name.field.label\")")
        .map(|(id, _)| id)
        .expect("native label");
    let control = nodes
        .values()
        .find(|node| {
            node["element_id"] == "Name(\"workspace.name\")" && node["aria"]["role"] == "TextInput"
        })
        .expect("native text input");
    assert_eq!(
        control["aria"]["labelled_by"],
        serde_json::json!([label_id])
    );
}

#[gpui::test]
fn required_is_published_rather_than_only_marked(cx: &mut TestAppContext) {
    let (mut harness, _slot) = field(cx, |field| field.required(true));
    let node = harness.node("workspace.name.field").expect("published");
    assert!(node.required);
    assert!(!node.invalid);
}

#[gpui::test]
fn an_error_is_added_to_the_description_rather_than_replacing_it(cx: &mut TestAppContext) {
    let (mut harness, _slot) = field(cx, |field| {
        field
            .description("Shown wherever this workspace appears.")
            .error("A workspace with this name already exists.")
    });

    assert_eq!(
        harness
            .node("workspace.name.field.description")
            .expect("the description survives an error")
            .text
            .as_deref(),
        Some("Shown wherever this workspace appears.")
    );
    let error = harness
        .node("workspace.name.field.error")
        .expect("the error is published");
    assert_eq!(
        error.text.as_deref(),
        Some("A workspace with this name already exists.")
    );
    assert!(error.invalid);
    assert!(
        harness
            .node("workspace.name.field")
            .expect("published")
            .invalid
    );

    let tree = harness.accessibility_tree();
    let nodes = tree["nodes"].as_object().expect("native nodes");
    let related = nodes
        .iter()
        .filter(|(_, node)| {
            matches!(
                node["element_id"].as_str(),
                Some("Name(\"workspace.name.field.description\")")
                    | Some("Name(\"workspace.name.field.error\")")
            )
        })
        .map(|(id, _)| id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let control = nodes
        .values()
        .find(|node| {
            node["element_id"] == "Name(\"workspace.name\")" && node["aria"]["role"] == "TextInput"
        })
        .expect("native text input");
    let described_by = control["aria"]["described_by"]
        .as_array()
        .expect("native described-by")
        .iter()
        .map(|id| id.as_str().expect("ephemeral node id"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(described_by, related);
}

#[gpui::test]
fn an_error_worded_like_the_description_is_shown_once(cx: &mut TestAppContext) {
    let (mut harness, _slot) = field(cx, |field| {
        field
            .description("A name is required.")
            .error("A name is required.")
    });

    assert!(
        harness.node("workspace.name.field.description").is_none(),
        "the same sentence must not be printed twice"
    );
    assert!(harness.node("workspace.name.field.error").is_some());
}

#[gpui::test]
fn a_field_without_an_error_invents_no_validity(cx: &mut TestAppContext) {
    let (mut harness, _slot) = field(cx, |field| field.description("Shown everywhere."));
    let node = harness.node("workspace.name.field").expect("published");
    assert!(!node.invalid);
    assert!(harness.node("workspace.name.field.error").is_none());
}

// -- NumberInput ----------------------------------------------------------

fn number(
    cx: &mut TestAppContext,
    configure: impl Fn(NumberInput) -> NumberInput + 'static,
) -> (Harness, Entity<NumberInput>) {
    let slot: Rc<RefCell<Option<Entity<NumberInput>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| configure(NumberInput::new("workspace.retention", window, cx)))
            })
            .clone()
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("number input was built");
    (harness, entity)
}

#[gpui::test]
fn a_value_outside_the_range_is_shown_and_published_as_invalid(cx: &mut TestAppContext) {
    let (mut harness, _entity) = number(cx, |input| {
        input.value(90.0).range(1.0, 60.0).step(5.0).unit("days")
    });
    let node = harness.node("workspace.retention").expect("published");

    assert_eq!(
        node.value.as_deref(),
        Some("90 days"),
        "the number that is actually set stays visible"
    );
    assert!(node.invalid, "and it is reported as outside the range");
    assert_eq!(node.value_max, Some(60.0));
}

#[gpui::test]
fn a_step_reports_the_next_number_and_never_applies_it(cx: &mut TestAppContext) {
    let (mut harness, entity) = number(cx, |input| input.value(3.0).range(0.0, 10.0).step(1.0));
    let events = record::<NumberInputEvent, _>(&mut harness, &entity);

    harness.click("workspace.retention.increment");
    assert_eq!(*events.borrow(), vec![NumberInputEvent::Changed(4.0)]);

    let held = harness.update({
        let entity = entity.clone();
        move |_, cx| entity.read(cx).current()
    });
    assert_eq!(
        held,
        Some(3.0),
        "the host still holds the value it started with"
    );
}

#[gpui::test]
fn a_step_at_the_boundary_reports_nothing_and_the_button_is_refused(cx: &mut TestAppContext) {
    let (mut harness, entity) = number(cx, |input| input.value(10.0).range(0.0, 10.0).step(1.0));
    let events = record::<NumberInputEvent, _>(&mut harness, &entity);

    assert!(
        harness
            .node("workspace.retention.increment")
            .expect("published")
            .disabled
    );
    assert!(
        !harness
            .node("workspace.retention.decrement")
            .expect("published")
            .disabled
    );

    harness.click("workspace.retention.increment");
    assert!(events.borrow().is_empty());
}

#[gpui::test]
fn arrow_keys_step_and_page_keys_step_further(cx: &mut TestAppContext) {
    let (mut harness, entity) = number(cx, |input| input.value(50.0).range(0.0, 100.0).step(2.0));
    let events = record::<NumberInputEvent, _>(&mut harness, &entity);

    harness.click("workspace.retention.field");
    harness.keystrokes("up");
    harness.keystrokes("down");
    harness.keystrokes("pageup");

    assert_eq!(
        *events.borrow(),
        vec![
            NumberInputEvent::Changed(52.0),
            NumberInputEvent::Changed(50.0),
            NumberInputEvent::Changed(70.0),
        ]
    );
}

#[gpui::test]
fn typing_reports_the_number_that_was_typed(cx: &mut TestAppContext) {
    let (mut harness, entity) = number(cx, |input| input.value(4.0).range(0.0, 100.0));
    let events = record::<NumberInputEvent, _>(&mut harness, &entity);

    harness.click("workspace.retention.field");
    let select_all = if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    };
    harness.keystrokes(select_all);
    harness.keystrokes("7");

    assert_eq!(
        events.borrow().last(),
        Some(&NumberInputEvent::Changed(7.0))
    );
}

#[derive(Debug)]
struct DecimalCommaNumbers;

impl NumberAdapter for DecimalCommaNumbers {
    fn count(&self, value: usize) -> SharedString {
        EnglishNumbers.count(value)
    }

    fn plural(&self, value: usize) -> Plural {
        EnglishNumbers.plural(value)
    }

    fn count_of_total(&self, done: usize, total: usize) -> SharedString {
        EnglishNumbers.count_of_total(done, total)
    }

    fn percent(&self, value: f32) -> SharedString {
        EnglishNumbers.percent(value)
    }

    fn decimal(&self, value: f64, precision: usize) -> SharedString {
        EnglishNumbers
            .decimal(value, precision)
            .replace('.', ",")
            .into()
    }

    fn parse_decimal(&self, text: &str) -> Option<f64> {
        text.trim().replace(',', ".").parse().ok()
    }
}

#[gpui::test]
fn a_number_field_accepts_the_same_digits_its_adapter_writes(cx: &mut TestAppContext) {
    let (mut harness, entity) = number(cx, |input| input.precision(2).unit("kg"));
    let events = record::<NumberInputEvent, _>(&mut harness, &entity);
    harness.update({
        let entity = entity.clone();
        move |_, cx| {
            set_numbers(DecimalCommaNumbers, cx);
            entity.update(cx, |number, cx| number.set_value(12.5, cx));
        }
    });
    assert_eq!(
        harness
            .node("workspace.retention")
            .expect("published")
            .value
            .as_deref(),
        Some("12,50 kg")
    );

    harness.click("workspace.retention.field");
    let select_all = if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    };
    harness.keystrokes(select_all);
    harness.keystrokes("12,75");

    assert_eq!(
        events.borrow().last(),
        Some(&NumberInputEvent::Changed(12.75)),
        "localized output must remain editable input"
    );
}

#[gpui::test]
fn text_that_is_not_a_number_is_reported_and_published_as_invalid(cx: &mut TestAppContext) {
    let (mut harness, entity) = number(cx, |input| input.value(4.0).range(0.0, 100.0));
    let events = record::<NumberInputEvent, _>(&mut harness, &entity);

    harness.click("workspace.retention.field");
    harness.keystrokes("x");

    assert!(matches!(
        events.borrow().last(),
        Some(NumberInputEvent::Unparsable(_))
    ));
    assert!(
        harness
            .node("workspace.retention")
            .expect("published")
            .invalid
    );
}

#[gpui::test]
fn a_number_nobody_supplied_starts_empty_rather_than_at_zero(cx: &mut TestAppContext) {
    let (mut harness, entity) = number(cx, |input| input.min(1.0));

    harness.update(|_, cx| {
        let input = entity.read(cx);
        assert_eq!(input.current(), None, "nobody has given it a number");
        assert!(
            !input.is_invalid(cx),
            "a control opens holding nothing, not a zero its own range rejects"
        );
    });

    let node = harness.node("workspace.retention").expect("published");
    assert_eq!(node.value.as_deref(), Some(""));
}

#[gpui::test]
fn stepping_an_empty_field_lands_on_the_bound_rather_than_past_a_zero(cx: &mut TestAppContext) {
    let (mut harness, entity) = number(cx, |input| input.min(1.0).max(10.0).step(1.0));
    let events = record::<NumberInputEvent, _>(&mut harness, &entity);

    harness.click("workspace.retention.increment");

    assert_eq!(
        events.borrow().last(),
        Some(&NumberInputEvent::Changed(1.0)),
        "the first step offers the smallest accepted number"
    );
}

#[gpui::test]
fn a_number_drawn_as_wrong_can_say_what_is_wrong_with_it(cx: &mut TestAppContext) {
    let (mut harness, entity) = number(cx, |input| input.value(4.0).range(1.0, 100.0));

    harness.update(|_, cx| {
        let input = entity.read(cx);
        assert!(!input.is_invalid(cx));
        assert_eq!(input.invalid_reason(cx), None, "nothing is wrong yet");
    });

    harness.click("workspace.retention.field");
    let select_all = if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    };
    harness.keystrokes(select_all);
    harness.keystrokes("0");
    harness.frame();

    harness.update(|_, cx| {
        let input = entity.read(cx);
        assert!(input.is_invalid(cx));
        assert_eq!(
            input.invalid_reason(cx).as_deref(),
            Some("The smallest accepted value is 1."),
            "the border and the sentence come from the same range"
        );
    });
}

// -- SegmentedControl -----------------------------------------------------

fn segmented(cx: &mut TestAppContext) -> (Harness, Rc<RefCell<Vec<String>>>) {
    let picked: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = picked.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |_window, _cx| {
        let sink = sink.clone();
        SegmentedControl::new("workspace.visibility")
            .label("Visibility")
            .segments([
                Segment::new("private", "Private"),
                Segment::new("team", "Team"),
                Segment::new("public", "Public").disabled(true),
            ])
            .selected("team")
            .on_select(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });
    (harness, picked)
}

#[gpui::test]
fn a_strip_is_a_radio_group_that_looks_like_a_strip(cx: &mut TestAppContext) {
    let (mut harness, _picked) = segmented(cx);
    let chosen = harness
        .node("workspace.visibility.team")
        .expect("published");

    assert_eq!(chosen.role, gpui_kit::semantics::Role::Radio);
    assert_eq!(chosen.checked, Some(true));
    assert_eq!(chosen.parent.as_deref(), Some("workspace.visibility"));
    assert_eq!(
        harness
            .node("workspace.visibility.private")
            .expect("published")
            .checked,
        Some(false)
    );
}

#[gpui::test]
fn a_segment_reports_the_choice_without_taking_it(cx: &mut TestAppContext) {
    let (mut harness, picked) = segmented(cx);
    harness.click("workspace.visibility.private");

    assert_eq!(*picked.borrow(), vec!["private".to_string()]);
    assert_eq!(
        harness
            .node("workspace.visibility.team")
            .expect("published")
            .checked,
        Some(true),
        "the caller has not applied the choice, so the strip still shows the old one"
    );
}

#[gpui::test]
fn a_refused_segment_ignores_a_click(cx: &mut TestAppContext) {
    let (mut harness, picked) = segmented(cx);
    harness.click("workspace.visibility.public");

    assert!(picked.borrow().is_empty());
    assert!(
        harness
            .node("workspace.visibility.public")
            .expect("published")
            .disabled
    );
}

// -- Combobox -------------------------------------------------------------

fn combobox(
    cx: &mut TestAppContext,
    configure: impl Fn(Combobox) -> Combobox + 'static,
) -> (Harness, Entity<Combobox>) {
    let slot: Rc<RefCell<Option<Entity<Combobox>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    configure(
                        Combobox::new("workspace.region", window, cx)
                            .name("Region")
                            .options([
                                SelectOption::new("eu-west", "Europe (Ireland)"),
                                SelectOption::new("eu-north", "Europe (Stockholm)"),
                                SelectOption::new("us-east", "United States (Virginia)"),
                                SelectOption::new("ap-south", "Asia Pacific (Mumbai)")
                                    .disabled(true),
                            ])
                            .selected("eu-west")
                            .placeholder("Choose a region"),
                    )
                })
            })
            .clone()
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("combobox was built");
    (harness, entity)
}

#[gpui::test]
fn a_closed_combobox_shows_the_current_answer(cx: &mut TestAppContext) {
    let (mut harness, _entity) = combobox(cx, |combobox| combobox);
    let node = harness.node("workspace.region").expect("published");

    assert_eq!(node.value.as_deref(), Some("Europe (Ireland)"));
    assert_eq!(node.expanded, Some(false));
    assert!(harness.node("workspace.region.us-east").is_none());
}

#[gpui::test]
fn combobox_native_nodes_share_a_name_and_keep_identity(cx: &mut TestAppContext) {
    let (mut harness, entity) = combobox(cx, |combobox| combobox);
    let tree = harness.accessibility_tree();
    let nodes = tree["nodes"].as_object().expect("nodes");
    let field = nodes
        .values()
        .find(|node| {
            node["element_id"] == "Name(\"workspace.region\")" && node["aria"]["role"] == "ComboBox"
        })
        .expect("native combobox");
    assert_eq!(field["aria"]["label"], "Region");
    assert_eq!(field["aria"]["value"], "Europe (Ireland)");
    let native_id = field["accesskit_id"].clone();
    let query = nodes
        .values()
        .find(|node| {
            node["element_id"] == "Name(\"workspace.region.query\")"
                && node["aria"]["role"] == "TextInput"
        })
        .expect("editable query target");
    assert_eq!(query["aria"]["label"], "Region");

    harness.update(|_, cx| {
        entity.update(cx, |combobox, cx| {
            combobox.set_selected(Some("us-east".into()), cx)
        });
    });
    let tree = harness.accessibility_tree();
    let field = tree["nodes"]
        .as_object()
        .and_then(|nodes| {
            nodes.values().find(|node| {
                node["element_id"] == "Name(\"workspace.region\")"
                    && node["aria"]["role"] == "ComboBox"
            })
        })
        .expect("updated native combobox");
    assert_eq!(field["accesskit_id"], native_id);
    assert_eq!(field["aria"]["value"], "United States (Virginia)");
}

#[gpui::test]
fn a_disabled_combobox_has_no_unnamed_or_focusable_native_target(cx: &mut TestAppContext) {
    let (mut harness, _entity) = combobox(cx, |combobox| combobox.disabled(true));
    let tree = harness.accessibility_tree();
    let nodes = tree["nodes"].as_object().expect("nodes");
    for (element_id, role) in [
        ("Name(\"workspace.region\")", "ComboBox"),
        ("Name(\"workspace.region.query\")", "TextInput"),
    ] {
        let node = nodes
            .values()
            .find(|node| node["element_id"] == element_id && node["aria"]["role"] == role)
            .unwrap_or_else(|| panic!("missing native {role}"));
        assert_eq!(node["aria"]["label"], "Region");
        assert_eq!(node["aria"]["disabled"], true);
        let actions = node["aria"]["on_action"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for action in ["Focus", "SetValue", "SetTextSelection"] {
            assert!(
                !actions.iter().any(|candidate| candidate == action),
                "disabled {role} advertised {action}: {actions:?}"
            );
        }
    }
}

#[gpui::test]
fn typing_opens_the_list_and_shows_only_what_matched(cx: &mut TestAppContext) {
    let (mut harness, _entity) = combobox(cx, |combobox| combobox);
    harness.click("workspace.region.query");
    let select_all = if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    };
    harness.keystrokes(select_all);
    harness.keystrokes("u n i");

    assert_eq!(
        harness
            .node("workspace.region")
            .expect("published")
            .expanded,
        Some(true)
    );
    assert!(harness.node("workspace.region.us-east").is_some());
    assert!(
        harness.node("workspace.region.eu-north").is_none(),
        "a row that does not answer the query is not listed"
    );
}

#[gpui::test]
fn enter_takes_the_highlighted_option_without_moving_the_answer(cx: &mut TestAppContext) {
    let (mut harness, entity) = combobox(cx, |combobox| combobox);
    let events = record::<ComboboxEvent, _>(&mut harness, &entity);

    harness.click("workspace.region.query");
    let select_all = if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    };
    harness.keystrokes(select_all);
    harness.keystrokes("u n i");
    harness.keystrokes("enter");

    assert!(
        events
            .borrow()
            .contains(&ComboboxEvent::Selected("us-east".into()))
    );
    let node = harness.node("workspace.region").expect("published");
    assert_eq!(
        node.value.as_deref(),
        Some("Europe (Ireland)"),
        "the owner has not applied it, so the old answer still holds"
    );
    assert_eq!(node.expanded, Some(false));
}

#[gpui::test]
fn escape_reverts_the_query_to_the_current_answer_and_reports_nothing(cx: &mut TestAppContext) {
    let (mut harness, entity) = combobox(cx, |combobox| combobox);
    let events = record::<ComboboxEvent, _>(&mut harness, &entity);

    harness.click("workspace.region.query");
    let select_all = if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    };
    harness.keystrokes(select_all);
    harness.keystrokes("u n i");
    harness.keystrokes("escape");

    let typed = harness.update({
        let entity = entity.clone();
        move |_, cx| entity.read(cx).query_text(cx).to_string()
    });
    assert_eq!(typed, "Europe (Ireland)");
    assert_eq!(
        harness
            .node("workspace.region")
            .expect("published")
            .expanded,
        Some(false)
    );
    assert!(
        !events
            .borrow()
            .iter()
            .any(|event| matches!(event, ComboboxEvent::Selected(_) | ComboboxEvent::Custom(_))),
        "abandoning an edit is not a choice"
    );
}

#[gpui::test]
fn a_query_nothing_answers_reports_nothing_when_the_set_is_closed(cx: &mut TestAppContext) {
    let (mut harness, entity) = combobox(cx, |combobox| combobox);
    let events = record::<ComboboxEvent, _>(&mut harness, &entity);

    harness.click("workspace.region.query");
    let select_all = if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    };
    harness.keystrokes(select_all);
    harness.keystrokes("z z z");
    harness.keystrokes("enter");

    assert!(
        harness.node("workspace.region.empty").is_some(),
        "an empty list says which query answered nothing"
    );
    assert!(
        !events
            .borrow()
            .iter()
            .any(|event| matches!(event, ComboboxEvent::Selected(_) | ComboboxEvent::Custom(_)))
    );
}

#[gpui::test]
fn a_custom_value_is_reported_when_the_caller_allows_one(cx: &mut TestAppContext) {
    let (mut harness, entity) = combobox(cx, |combobox| combobox.allow_custom(true));
    let events = record::<ComboboxEvent, _>(&mut harness, &entity);

    harness.click("workspace.region.query");
    let select_all = if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    };
    harness.keystrokes(select_all);
    harness.keystrokes("z z z");
    harness.keystrokes("enter");

    assert!(
        events
            .borrow()
            .contains(&ComboboxEvent::Custom("zzz".into()))
    );
}

#[gpui::test]
fn a_refused_option_cannot_be_taken(cx: &mut TestAppContext) {
    let (mut harness, entity) = combobox(cx, |combobox| combobox);
    let events = record::<ComboboxEvent, _>(&mut harness, &entity);

    harness.click("workspace.region");
    harness.click("workspace.region.ap-south");

    assert!(
        harness
            .node("workspace.region.ap-south")
            .expect("published")
            .disabled
    );
    assert!(
        !events
            .borrow()
            .iter()
            .any(|event| matches!(event, ComboboxEvent::Selected(_)))
    );
}

fn popup_combobox_options() -> Vec<SelectOption> {
    let mut options = Vec::new();
    for index in 0..14 {
        options.push(SelectOption::new(
            format!("archive-{index:02}"),
            format!("Archived runtime {index:02}"),
        ));
        options.push(SelectOption::new(
            format!("model-{index:02}"),
            format!("Agent model {index:02}"),
        ));
    }
    options.push(SelectOption::new("unknown", "Unknown model").description(
        "This model may not support chat in direct mode.\nChoose a chat-capable model.",
    ));
    options.push(SelectOption::new("managed", "Managed model").disabled(true));
    options
}

fn popup_combobox(cx: &mut TestAppContext, near_bottom: bool) -> (Harness, Entity<Combobox>) {
    let slot: Rc<RefCell<Option<Entity<Combobox>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let combobox = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    Combobox::new("models.search", window, cx)
                        .name("All Agent models")
                        .options(popup_combobox_options())
                        .selected("unknown")
                })
            })
            .clone();
        let trigger = if near_bottom {
            div()
                .h_full()
                .flex()
                .flex_col()
                .justify_end()
                .pb(px(44.0))
                .child(combobox)
                .into_any_element()
        } else {
            div().pt(px(12.0)).child(combobox).into_any_element()
        };
        div()
            .size_full()
            .child(
                div()
                    .id("models.surface")
                    .w(px(300.0))
                    .h_full()
                    .overflow_y_scroll()
                    .child(trigger),
            )
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("combobox was built");
    (harness, entity)
}

fn resize_popup(harness: &mut Harness, width: f32, height: f32) {
    harness
        .context()
        .simulate_resize(size(px(width), px(height)));
    harness.frame();
    harness.frame();
}

fn assert_popup_row_inside(inner: gpui::Bounds<gpui::Pixels>, outer: gpui::Bounds<gpui::Pixels>) {
    let epsilon = px(0.5);
    assert!(
        inner.top() + epsilon >= outer.top(),
        "{inner:?} above {outer:?}"
    );
    assert!(
        inner.bottom() <= outer.bottom() + epsilon,
        "{inner:?} below {outer:?}"
    );
}

#[gpui::test]
fn filtered_popup_flips_above_and_reveals_the_full_last_match(cx: &mut TestAppContext) {
    let (mut harness, entity) = popup_combobox(cx, true);
    resize_popup(&mut harness, 520.0, 480.0);
    harness.update(move |_, cx| entity.update(cx, |combobox, cx| combobox.set_query("model", cx)));
    harness.click("models.search.query");
    harness.keystrokes("end");

    let margin = harness.update(|_, cx| cx.theme().spacing.sm);
    let trigger = harness.bounds("models.search").expect("trigger bounds");
    let menu = harness.bounds("models.search.menu").expect("menu bounds");
    let active = harness
        .bounds("models.search.unknown")
        .expect("last filtered match");
    assert!(menu.bottom() <= trigger.top(), "the menu must flip above");
    assert!(f32::from(menu.top()) >= margin - 0.5);
    assert!(f32::from(menu.bottom()) <= 480.0 - margin + 0.5);
    assert!(active.size.height > px(40.0), "the warning is multi-line");
    assert_popup_row_inside(active, menu);
    assert!(
        harness
            .node("models.search.unknown")
            .expect("active match")
            .hovered
    );
    assert!(
        !harness
            .node("models.search.managed")
            .expect("disabled final match")
            .hovered,
        "End skips the disabled match"
    );
    assert!(
        harness.node("models.search.archive-13").is_none(),
        "the filtered positions omit non-matches"
    );
}

#[gpui::test]
fn narrow_filtered_popup_stays_below_and_home_end_update_scroll(cx: &mut TestAppContext) {
    let (mut harness, entity) = popup_combobox(cx, false);
    resize_popup(&mut harness, 360.0, 240.0);
    let query_entity = entity.clone();
    harness.update(move |_, cx| {
        query_entity.update(cx, |combobox, cx| combobox.set_query("model", cx))
    });
    harness.click("models.search.query");

    let margin = harness.update(|_, cx| cx.theme().spacing.sm);
    let trigger = harness.bounds("models.search").expect("trigger bounds");
    let menu = harness.bounds("models.search.menu").expect("menu bounds");
    assert!(menu.top() >= trigger.bottom(), "the menu must remain below");
    assert!(f32::from(menu.top()) >= margin - 0.5);
    assert!(f32::from(menu.bottom()) <= 240.0 - margin + 0.5);

    harness.keystrokes("end");
    assert_popup_row_inside(
        harness
            .bounds("models.search.unknown")
            .expect("End match bounds"),
        harness.bounds("models.search.menu").expect("menu bounds"),
    );
    harness.keystrokes("home");
    assert!(
        harness
            .node("models.search.model-00")
            .expect("first filtered match")
            .hovered
    );
    assert_popup_row_inside(
        harness
            .bounds("models.search.model-00")
            .expect("Home match bounds"),
        harness.bounds("models.search.menu").expect("menu bounds"),
    );

    harness.update(move |_, cx| {
        entity.update(cx, |combobox, cx| {
            combobox.set_selected(Some("unknown".into()), cx)
        })
    });
    assert!(
        harness
            .node("models.search.unknown")
            .expect("new selected match")
            .hovered
    );
    assert_popup_row_inside(
        harness
            .bounds("models.search.unknown")
            .expect("selected change bounds"),
        harness.bounds("models.search.menu").expect("menu bounds"),
    );
}

// -- TagInput -------------------------------------------------------------

fn tag_input(
    cx: &mut TestAppContext,
    configure: impl Fn(TagInput) -> TagInput + 'static,
) -> (Harness, Entity<TagInput>) {
    let slot: Rc<RefCell<Option<Entity<TagInput>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    configure(
                        TagInput::new("workspace.labels", window, cx)
                            .tags(["indexing", "nightly"])
                            .placeholder("Add a label"),
                    )
                })
            })
            .clone()
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("tag input was built");
    (harness, entity)
}

#[gpui::test]
fn every_tag_is_addressed_by_what_it_says(cx: &mut TestAppContext) {
    let (mut harness, _entity) = tag_input(cx, |tags| tags);
    assert_eq!(
        harness
            .node("workspace.labels.indexing")
            .expect("published")
            .text
            .as_deref(),
        Some("indexing")
    );
    assert_eq!(
        harness
            .node("workspace.labels")
            .expect("published")
            .value
            .as_deref(),
        Some("2")
    );
}

#[gpui::test]
fn the_first_backspace_targets_the_last_tag_and_the_second_removes_it(cx: &mut TestAppContext) {
    let (mut harness, entity) = tag_input(cx, |tags| tags);
    let events = record::<TagInputEvent, _>(&mut harness, &entity);

    harness.click("workspace.labels.field");
    harness.keystrokes("backspace");

    assert!(
        events.borrow().is_empty(),
        "the first press removes nothing at all"
    );
    assert!(
        harness
            .node("workspace.labels.nightly")
            .expect("published")
            .selected,
        "it singles the tag out so the typist can see what is at risk"
    );

    harness.keystrokes("backspace");
    assert_eq!(
        *events.borrow(),
        vec![TagInputEvent::Removed("nightly".into())]
    );
    assert!(
        harness.node("workspace.labels.nightly").is_some(),
        "the set is the caller's: nothing goes until the caller says so"
    );
}

#[gpui::test]
fn enter_commits_what_was_typed(cx: &mut TestAppContext) {
    let (mut harness, entity) = tag_input(cx, |tags| tags);
    let events = record::<TagInputEvent, _>(&mut harness, &entity);

    harness.click("workspace.labels.field");
    harness.keystrokes("f l a k y enter");

    assert_eq!(*events.borrow(), vec![TagInputEvent::Added("flaky".into())]);
}

#[gpui::test]
fn a_comma_commits_what_came_before_it(cx: &mut TestAppContext) {
    let (mut harness, entity) = tag_input(cx, |tags| tags);
    let events = record::<TagInputEvent, _>(&mut harness, &entity);

    harness.click("workspace.labels.field");
    harness.keystrokes("f l a k y ,");

    assert_eq!(*events.borrow(), vec![TagInputEvent::Added("flaky".into())]);
    let typed = harness.update({
        let entity = entity.clone();
        move |_, cx| entity.read(cx).field().read(cx).value().to_string()
    });
    assert_eq!(typed, "", "the separator does not end up inside a tag");
}

#[gpui::test]
fn a_duplicate_is_reported_as_a_duplicate(cx: &mut TestAppContext) {
    let (mut harness, entity) = tag_input(cx, |tags| tags);
    let events = record::<TagInputEvent, _>(&mut harness, &entity);

    harness.click("workspace.labels.field");
    harness.keystrokes("n i g h t l y enter");

    assert_eq!(
        *events.borrow(),
        vec![TagInputEvent::Duplicate("nightly".into())]
    );
    let refusal = harness
        .node("workspace.labels.refusal")
        .expect("the refusal is shown where the typist is looking");
    assert!(refusal.text.is_some());
    assert!(harness.node("workspace.labels").expect("published").invalid);
}

#[gpui::test]
fn a_full_field_refuses_out_loud_rather_than_swallowing_the_keystroke(cx: &mut TestAppContext) {
    let (mut harness, entity) = tag_input(cx, |tags| tags.max(2));
    let events = record::<TagInputEvent, _>(&mut harness, &entity);

    harness.click("workspace.labels.field");
    harness.keystrokes("f l a k y enter");

    assert_eq!(
        *events.borrow(),
        vec![TagInputEvent::Refused("flaky".into())]
    );
    assert!(harness.node("workspace.labels.refusal").is_some());
    let typed = harness.update({
        let entity = entity.clone();
        move |_, cx| entity.read(cx).field().read(cx).value().to_string()
    });
    assert_eq!(
        typed, "flaky",
        "what was typed stays put, because it was refused rather than lost"
    );
}

#[gpui::test]
fn removing_a_tag_reports_it_and_changes_nothing(cx: &mut TestAppContext) {
    let (mut harness, entity) = tag_input(cx, |tags| tags);
    let events = record::<TagInputEvent, _>(&mut harness, &entity);

    harness.click("workspace.labels.indexing.remove");

    assert_eq!(
        *events.borrow(),
        vec![TagInputEvent::Removed("indexing".into())]
    );
    assert!(harness.node("workspace.labels.indexing").is_some());
}

#[gpui::test]
fn the_host_owns_the_set(cx: &mut TestAppContext) {
    let (mut harness, entity) = tag_input(cx, |tags| tags);
    harness.update({
        let entity = entity.clone();
        move |_, cx| {
            entity.update(cx, |tags, cx| {
                tags.set_tags(vec!["indexing".into()], cx);
            });
        }
    });

    assert!(harness.node("workspace.labels.nightly").is_none());
    assert_eq!(
        harness
            .node("workspace.labels")
            .expect("published")
            .value
            .as_deref(),
        Some("1")
    );
}
