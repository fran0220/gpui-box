use super::*;
use gpui::TestAppContext;
use gpui_kit_testkit::harness::Harness;

fn node(component: &str, props: Value) -> Node {
    serde_json::from_value(
        json!({"kind":"kit","component":component,"id":"structured","props":props,"events":{}}),
    )
    .expect("structured fixture descriptor")
}

#[gpui::test]
fn all_form_methods_preserve_native_state_and_teardown(cx: &mut TestAppContext) {
    let state = Rc::new(State::default());
    let build = state.clone();
    let n = Rc::new(node(
        "SchemaForm",
        json!({"fields":[{"name":"title","kind":"text","required":true},{"name":"items","kind":"list","maxItems":3,"item":{"name":"entry","kind":"text"}},{"name":"file","kind":"files"},{"name":"unsupported","kind":"unrenderable","reason":"No host adapter","required":true}]}),
    ));
    let descriptor = n.clone();
    let mut h = Harness::new(cx, gpui_kit::install, move |w, cx| {
        build.render(&descriptor, KitSlots::new(), w, cx, Rc::new(|_, _| {}))
    });
    let id = state
        .forms
        .borrow()
        .values()
        .next()
        .expect("mounted form")
        .entity
        .entity_id();
    let call = |h: &mut Harness, m: &str, a: Value, q: bool| {
        h.update(|w, cx| {
            let schema = super::super::validation::invocation("SchemaForm", m, &a, q)
                .expect("declared method and valid arguments");
            let result = state
                .invoke(&n, m, &a, q, w, cx)
                .expect("native form method");
            super::super::validation::validate(&result, schema).expect("bounded typed result");
            result
        })
    };
    assert_eq!(
        call(&mut h, "add_list_item", json!({"path":"items"}), false),
        json!(true)
    );
    assert_eq!(
        call(&mut h, "add_list_item", json!({"path":"items"}), false),
        json!(true)
    );
    assert_eq!(
        call(
            &mut h,
            "move_list_item",
            json!({"path":"items","from":0,"to":1}),
            false
        ),
        json!(true)
    );
    assert_eq!(
        call(
            &mut h,
            "remove_list_item",
            json!({"path":"items","index":1}),
            false
        ),
        json!(true)
    );
    assert_eq!(
        call(
            &mut h,
            "set_files",
            json!({"path":"missing","files":[]}),
            false
        ),
        json!(false)
    );
    assert_eq!(
        call(
            &mut h,
            "set_field_validation",
            json!({"path":"title","validation":{"state":"invalid","reason":"Host refusal"}}),
            false
        ),
        json!(true)
    );
    assert_eq!(
        call(&mut h, "field_validation", json!({"path":"title"}), true),
        json!({"state":"invalid","reason":"Host refusal"})
    );
    assert_eq!(
        call(
            &mut h,
            "clear_field_validation",
            json!({"path":"title"}),
            false
        ),
        json!(true)
    );
    call(
        &mut h,
        "set_validation",
        json!({"validation":{"state":"validating"}}),
        false,
    );
    assert_eq!(
        call(&mut h, "validation", json!({}), true),
        json!({"state":"validating","reason":null})
    );
    call(&mut h, "clear_validation", json!({}), false);
    assert_eq!(call(&mut h, "validation", json!({}), true), Value::Null);
    call(
        &mut h,
        "set_error",
        json!({"path":"title","message":"Host error"}),
        false,
    );
    call(&mut h, "clear_host_errors", json!({}), false);
    assert_eq!(call(&mut h, "validate", json!({}), false), json!(false));
    assert_eq!(
        call(
            &mut h,
            "set_field_visibility",
            json!({"path":"items","visibility":"hiddenOmit"}),
            false
        ),
        json!(true)
    );
    assert_eq!(
        call(&mut h, "field_visibility", json!({"path":"items"}), true),
        json!("hiddenOmit")
    );
    let all = call(&mut h, "values", json!({}), true);
    let submission = call(&mut h, "submission_values", json!({}), true);
    assert!(
        all.as_array()
            .expect("value records")
            .iter()
            .any(|v| v["path"] == "items")
    );
    assert!(
        !submission
            .as_array()
            .expect("submission records")
            .iter()
            .any(|v| v["path"].as_str().expect("field path").starts_with("items"))
    );
    assert_eq!(
        call(&mut h, "has_unrenderable_required", json!({}), true),
        json!(true)
    );
    assert!(
        call(&mut h, "unrenderable", json!({}), true)
            .as_array()
            .expect("unrenderable records")
            .iter()
            .any(|v| v["path"] == "unsupported" && v["reason"] == "No host adapter")
    );
    call(&mut h, "set_disabled", json!({"disabled":true}), false);
    assert!(
        h.update(|w, cx| state.invoke(&n, "validate", &json!({}), false, w, cx))
            .is_err()
    );
    assert!(call(&mut h, "values", json!({}), true).is_array());
    h.frame();
    assert_eq!(
        state
            .forms
            .borrow()
            .values()
            .next()
            .expect("retained form")
            .entity
            .entity_id(),
        id
    );
    h.update(|_, cx| state.reconcile(&node("JsonView", json!({"value":{"kind":"null"}})), cx));
    assert!(state.forms.borrow().is_empty());
}

#[gpui::test]
fn json_view_preserves_number_spelling_duplicates_and_disclosure(cx: &mut TestAppContext) {
    let v = json!({"kind":"object","members":[{"key":"same","value":{"kind":"number","text":"1.10"}},{"key":"same","value":{"kind":"redacted","text":"withheld"}},{"key":"nested","value":{"kind":"array","items":[{"kind":"boolean","boolean":false}]}}]});
    let JsonValue::Object(members) = json_value(&v) else {
        panic!("object")
    };
    assert_eq!(members.len(), 3);
    assert_eq!(members[0].1, JsonValue::number("1.10"));
    let v = json!({"kind":"identifiedObject","members":[{"id":"first","key":"same","value":{"kind":"number","text":"1.10"}},{"id":"second","key":"same","value":{"kind":"redacted","text":"withheld"}},{"id":"nested","key":"nested","value":{"kind":"array","items":[{"kind":"boolean","boolean":false}]}}]});
    let n = Rc::new(node(
        "JsonView",
        json!({"value":v,"expanded":["~2nested"],"visibleRows":4}),
    ));
    let descriptor = n.clone();
    let state = Rc::new(State::default());
    let build = state.clone();
    let mut h = Harness::new(cx, gpui_kit::install, move |w, cx| {
        build.render(&descriptor, KitSlots::new(), w, cx, Rc::new(|_, _| {}))
    });
    let result = h.update(|w, cx| {
        state
            .invoke(&n, "disclosed_paths", &json!({}), true, w, cx)
            .expect("disclosed path query")
    });
    assert!(
        result
            .as_array()
            .expect("disclosed paths")
            .contains(&json!("~2nested/0"))
    );
}
