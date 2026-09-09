use super::*;
use gpui::{Focusable, ParentElement, Styled, TestAppContext};
use gpui_kit_testkit::harness::Harness;

#[test]
#[ignore = "offscreen native review artifact; run explicitly with --ignored"]
fn capture_native_family_review() {
    use gpui::{Context, HeadlessAppContext, Render, size};
    use std::sync::Arc;
    struct Review {
        state: State,
        drawer: Node,
    }
    impl Render for Review {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let graph:Node=serde_json::from_value(json!({"kind":"kit","component":"NodeGraph","id":"review.graph","props":{"viewport":{"offset":{"x":17,"y":31},"zoom":0.8},"nodes":[{"id":"review.source","x":40,"y":50,"props":{"title":"Native source","state":"succeeded","status":"Verified","width":200,"ports":[{"id":"out","label":"Records","direction":"output"}]}},{"id":"review.sink","x":330,"y":90,"props":{"title":"Native sink","state":"failed","status":"Connection refused","width":210,"ports":[{"id":"in","label":"Records","direction":"input"}]}}],"edges":[{"id":"review.wire","from":"review.source","to":"review.sink","ports":{"from":"out","to":"in"},"marker":"arrow","state":"failed"}],"routing":"curves"}})).expect("graph review descriptor");
            let toolbar:Node=serde_json::from_value(json!({"kind":"kit","component":"CanvasToolbar","id":"review.toolbar","props":{"zoom":"80%","disabled":true}})).expect("toolbar review descriptor");
            let mut slots = KitSlots::new();
            slots.insert(
                "content".into(),
                Rc::new(|_, _| {
                    div()
                        .p(px(16.))
                        .child("Fresh slot body; native drawer stays open across options updates.")
                        .into_any_element()
                }),
            );
            div()
                .size_full()
                .bg(gpui::rgb(0xf7f8fa))
                .p(px(24.))
                .child(
                    div()
                        .h(px(400.))
                        .w(px(580.))
                        .child(super::super::canvas::render(
                            &graph,
                            KitSlots::new(),
                            window,
                            cx,
                            Rc::new(|_, _| {}),
                        )),
                )
                .child(super::super::canvas::render(
                    &toolbar,
                    KitSlots::new(),
                    window,
                    cx,
                    Rc::new(|_, _| {}),
                ))
                .child(
                    self.state
                        .render(&self.drawer, slots, window, cx, Rc::new(|_, _| {})),
                )
        }
    }
    let mut cx = HeadlessAppContext::with_platform(
        gpui_platform::test_text_system("Geist"),
        Arc::new(gpui_kit::assets::Assets),
        gpui_platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::install(cx);
        cx.set_reduce_motion(true);
    });
    let handle=cx.open_window(size(px(980.),px(620.)),|_,cx|cx.new(|_|Review{state:State::default(),drawer:node("Drawer",json!({"title":"Retained native overlay","description":"State and focus preserved","size":300,"dismissable":false}))})).expect("headless review window");
    let window = handle.into();
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update_window(window, |_, w, c| w.draw(c).clear(c))
            .expect("initial review frame");
    }
    cx.update(|cx| {
        handle.update(cx, |view, w, c| {
            view.state
                .invoke(&view.drawer, "open", &json!({}), false, w, c)
                .expect("open review drawer")
        })
    })
    .expect("update review window");
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update_window(window, |_, w, c| w.draw(c).clear(c))
            .expect("open review frame");
    }
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.amp/in/artifacts");
    std::fs::create_dir_all(&directory).expect("review artifact directory");
    cx.capture_screenshot(window)
        .expect("capture native review")
        .save(directory.join("native-canvas-overlay.png"))
        .expect("save native review");
}

fn node(component: &str, props: Value) -> Node {
    serde_json::from_value(json!({"kind":"kit","component":component,"id":"overlay","props":props,"events":{"open":"open","close":"close","invoked":"invoked","dismiss":"dismiss"}})).expect("overlay fixture descriptor")
}

#[gpui::test]
fn slot_setter_rejects_removed_keys_and_never_replays_the_old_factory(cx: &mut TestAppContext) {
    let state = Rc::new(State::default());
    let owner = state.clone();
    let descriptor = node("Drawer", json!({}));
    let source = descriptor.clone();
    let mounted = Rc::new(RefCell::new(true));
    let present = mounted.clone();
    let calls = Rc::new(RefCell::new(0));
    let rendered_calls = calls.clone();
    let mut h = Harness::new(cx, gpui_kit::install, move |w, c| {
        let mut slots = KitSlots::new();
        if *present.borrow() {
            let calls = rendered_calls.clone();
            slots.insert(
                "content".into(),
                Rc::new(move |_, _| {
                    *calls.borrow_mut() += 1;
                    div().child("Current content").into_any_element()
                }),
            );
        }
        owner.render(&source, slots, w, c, Rc::new(|_, _| {}))
    });
    h.update(|w, c| {
        state
            .invoke(&descriptor, "open", &json!({}), false, w, c)
            .expect("open drawer");
        state
            .invoke(
                &descriptor,
                "set_content",
                &json!({"slot":"content"}),
                false,
                w,
                c,
            )
            .expect("mounted content slot");
        assert!(
            state
                .invoke(
                    &descriptor,
                    "set_content",
                    &json!({"slot":"footer"}),
                    false,
                    w,
                    c
                )
                .is_err()
        );
    });
    assert!(*calls.borrow() > 0);
    *mounted.borrow_mut() = false;
    h.frame();
    let before = *calls.borrow();
    h.update(|w, c| {
        assert!(
            state
                .invoke(
                    &descriptor,
                    "set_content",
                    &json!({"slot":"content"}),
                    false,
                    w,
                    c
                )
                .is_err()
        );
        assert_eq!(
            state
                .invoke(&descriptor, "is_open", &json!({}), true, w, c)
                .expect("slot removal preserves drawer"),
            true
        );
    });
    h.frame();
    assert_eq!(*calls.borrow(), before);
}

#[gpui::test]
fn capacity_updates_preserve_notification_reads_and_toast_timers(cx: &mut TestAppContext) {
    for component in ["NotificationCenter", "ToastLayer"] {
        let state = Rc::new(State::default());
        let owner = state.clone();
        let descriptor = Rc::new(RefCell::new(node(component, json!({"capacity":3}))));
        let source = descriptor.clone();
        let mut h = Harness::new(
            cx,
            |cx| {
                gpui_kit::install(cx);
                cx.set_reduce_motion(true);
            },
            move |w, c| owner.render(&source.borrow(), KitSlots::new(), w, c, Rc::new(|_, _| {})),
        );
        h.update(|w, c| {
            for (id, read) in [("old", false), ("retained", true), ("new", false)] {
                let (method, args) = if component == "ToastLayer" {
                    (
                        "push",
                        json!({"toast":{"id":id,"message":id,"timeout":1000}}),
                    )
                } else {
                    (
                        "record",
                        json!({"notification":{"id":id,"message":id,"read":read}}),
                    )
                };
                state
                    .invoke(&descriptor.borrow(), method, &args, false, w, c)
                    .expect("populate native history");
            }
        });
        h.advance(Duration::from_millis(600));
        descriptor
            .borrow_mut()
            .props
            .insert("capacity".into(), json!(2));
        h.frame();
        if component == "NotificationCenter" {
            h.update(|w, c| {
                for (method, args, expected) in [
                    ("len", json!({}), json!(2)),
                    ("holds", json!({"id":"old"}), json!(false)),
                    ("is_read", json!({"id":"retained"}), json!(true)),
                    ("unread", json!({}), json!({"kind":"at-least","value":1})),
                ] {
                    assert_eq!(
                        state
                            .invoke(&descriptor.borrow(), method, &args, true, w, c)
                            .expect("retention query"),
                        expected
                    );
                }
            });
        } else {
            h.advance(Duration::from_millis(450));
            h.frame();
            h.update(|w, c| {
                assert_eq!(
                    state
                        .invoke(&descriptor.borrow(), "is_empty", &json!({}), true, w, c)
                        .expect("original toast timers expire"),
                    true
                )
            });
        }
    }
}

#[gpui::test]
fn palette_query_input_registers_the_actual_child_not_a_snapshot(cx: &mut TestAppContext) {
    fn dispatch(
        input: &Entity<TextInput>,
        method: &str,
        args: &Value,
        query: bool,
        _: &mut Window,
        cx: &mut App,
        _: &crate::references::Registration<'_>,
    ) -> Result<Value> {
        match (method, query) {
            ("value", true) => Ok(json!(input.read(cx).value().as_ref())),
            ("set_value", false) => {
                ensure!(!input.read(cx).is_disabled(), "disabled input");
                input.update(cx, |input, cx| input.set_value(s(args, "value"), cx));
                Ok(Value::Null)
            }
            _ => bail!("test dispatcher only exercises value"),
        }
    }
    let descriptor = node("CommandPalette", json!({"query":"First"}));
    let source = descriptor.clone();
    let state = Rc::new(State::default());
    let render_state = state.clone();
    let mut h = Harness::new(cx, gpui_kit::install, move |w, c| {
        render_state.render(&source, KitSlots::new(), w, c, Rc::new(|_, _| {}))
    });
    let registry = crate::references::Registry::new();
    let owner = gpui::EffectOwner::new();
    h.update(|w, c| {
        let refs = registry.registration(&descriptor, owner);
        let reference = state
            .query_input(&descriptor, c, &refs, dispatch)
            .expect("query real palette input");
        assert_eq!(
            registry
                .invoke(owner, &reference, "value", &json!({}), true, w, c)
                .expect("query input value"),
            "First"
        );
        registry
            .invoke(
                owner,
                &reference,
                "set_value",
                &json!({"value":"Changed child"}),
                false,
                w,
                c,
            )
            .expect("edit actual child");
        assert_eq!(
            state
                .invoke(&descriptor, "query", &json!({}), true, w, c)
                .expect("palette reads child"),
            "Changed child"
        );
        let palette = {
            let entries = state.entries.borrow();
            let Control::Palette(palette) = &entries
                .get(&(0, "overlay".into()))
                .expect("retained palette")
                .control
            else {
                panic!()
            };
            palette.clone()
        };
        palette
            .read(c)
            .query_input()
            .clone()
            .update(c, |input, c| input.set_disabled(true, c));
        assert!(
            registry
                .invoke(
                    owner,
                    &reference,
                    "set_value",
                    &json!({"value":"Forbidden"}),
                    false,
                    w,
                    c
                )
                .is_err()
        );
        assert_eq!(
            registry
                .invoke(owner, &reference, "value", &json!({}), true, w, c)
                .expect("disabled query remains allowed"),
            "Changed child"
        );
    });
}

#[gpui::test]
fn focus_queries_use_shared_weak_registry_and_revoke_on_native_removal(cx: &mut TestAppContext) {
    for component in [
        "Drawer",
        "HoverCard",
        "Menu",
        "ContextMenu",
        "CommandPalette",
        "NotificationCenter",
    ] {
        let descriptor = node(component, json!({}));
        let source = descriptor.clone();
        let state = Rc::new(State::default());
        let render_state = state.clone();
        let registry = crate::references::Registry::new();
        let owner = gpui::EffectOwner::new();
        let mut h = Harness::new(cx, gpui_kit::install, move |w, c| {
            render_state.render(&source, KitSlots::new(), w, c, Rc::new(|_, _| {}))
        });
        let reference = h.update(|w, c| {
            let refs = registry.registration(&descriptor, owner);
            let reference = state
                .invoke_reference(&descriptor, "focus_handle", &json!({}), true, c, &refs)
                .expect("register actual focus");
            assert_eq!(reference["type"], "FocusHandle");
            assert_eq!(
                state
                    .invoke_reference(&descriptor, "focus_handle", &json!({}), true, c, &refs)
                    .expect("repeat registration"),
                reference
            );
            registry
                .invoke(owner, &reference, "focus", &json!({}), false, w, c)
                .expect("focus native handle");
            assert_eq!(
                registry
                    .invoke(owner, &reference, "is_focused", &json!({}), true, w, c)
                    .expect("native focus query"),
                true
            );
            if component == "Drawer" {
                let mut with_stops = descriptor.clone();
                with_stops
                    .props
                    .insert("focus_stops".into(), json!([reference.clone()]));
                state
                    .apply_reference_props(&with_stops, c, &refs)
                    .expect("builder focus stops");
                state
                    .invoke_reference(
                        &descriptor,
                        "set_focus_stops",
                        &json!({"stops":[reference.clone()]}),
                        false,
                        c,
                        &refs,
                    )
                    .expect("native focus stops setter");
            }
            assert!(
                registry
                    .invoke(
                        gpui::EffectOwner::new(),
                        &reference,
                        "focus",
                        &json!({}),
                        false,
                        w,
                        c
                    )
                    .is_err()
            );
            reference
        });
        h.remount(|_, _| div().into_any_element());
        h.update(|w, c| {
            let empty = node("Kbd", json!({"keystroke":"k"}));
            state.reconcile(&empty, c);
            registry.reconcile(
                &empty,
                |_| Some(owner),
                |_| state.native_entity_id(&descriptor),
                c,
            );
            assert!(
                registry
                    .invoke(owner, &reference, "focus", &json!({}), false, w, c)
                    .is_err()
            );
        });
    }
}

#[gpui::test]
fn every_declared_data_method_executes_against_native_state(cx: &mut TestAppContext) {
    let cases: Value =
        serde_json::from_str(include_str!("fixture/methods.json")).expect("method fixtures");
    let contracts: Value =
        serde_json::from_str(include_str!("../methods.json")).expect("integrated method contracts");
    for case in cases.as_array().expect("fixture cases") {
        let component = case["component"].as_str().expect("component");
        let descriptor = node(component, case["props"].clone());
        let rendered = descriptor.clone();
        let state = Rc::new(State::default());
        let owner = state.clone();
        let mut h = Harness::new(cx, gpui_kit::install, move |w, c| {
            let mut slots = KitSlots::new();
            for name in ["content", "footer", "trigger", "empty"] {
                slots.insert(
                    name.into(),
                    Rc::new(|_, _| div().child("Current slot").into_any_element()),
                );
            }
            owner.render(&rendered, slots, w, c, Rc::new(|_, _| {}))
        });
        for step in case["steps"].as_array().expect("method steps") {
            let method = step[0].as_str().expect("method");
            let query = contracts[component]["query"].get(method).is_some();
            let result = h
                .update(|w, c| state.invoke(&descriptor, method, &step[1], query, w, c))
                .unwrap_or_else(|error| panic!("{component}.{method}: {error}"));
            #[cfg(target_os = "macos")]
            if component == "Kbd" {
                assert_eq!(result, json!(["⌃⇧K"]));
                continue;
            }
            assert_eq!(result, step[2], "{component}.{method}");
        }
    }
}

#[gpui::test]
fn native_context_replacement_cancellation_refusal_and_removal(cx: &mut TestAppContext) {
    let state = Rc::new(State::default());
    let owner = state.clone();
    let descriptor = node(
        "ContextMenu",
        json!({"target":"record-a","items":[{"kind":"command","id":"old","label":"Old"}]}),
    );
    let source = descriptor.clone();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    let mut h = Harness::new(cx, gpui_kit::install, move |w, c| {
        let output = output.clone();
        owner.render(
            &source,
            KitSlots::new(),
            w,
            c,
            Rc::new(move |action, value| output.borrow_mut().push((action.to_owned(), value))),
        )
    });
    let window = h.window();
    h.context().set_native_context_menus_supported(window, true);
    h.update(|w, c| {
        state
            .invoke(
                &descriptor,
                "open_at",
                &json!({"position":{"x":23,"y":47}}),
                false,
                w,
                c,
            )
            .expect("native open")
    });
    assert_eq!(*seen.borrow(), vec![("open".into(), json!("record-a"))]);
    h.context()
        .set_native_context_menu_cancel_fails(window, true);
    h.update(|w, c| {
        assert!(
            state
                .invoke(
                    &descriptor,
                    "set_items",
                    &json!({"items":[{"kind":"command","id":"new","label":"New"}]}),
                    false,
                    w,
                    c
                )
                .is_err()
        )
    });
    h.update(|w, c| {
        assert_eq!(
            state
                .invoke(&descriptor, "is_open", &json!({}), true, w, c)
                .expect("still native open"),
            true
        )
    });
    assert!(
        h.node("overlay.menu").is_none(),
        "cancellation failure cannot install fallback"
    );
    h.context().select_context_menu_item(window, &[0]);
    h.context().run_until_parked();
    assert!(
        !seen.borrow().iter().any(|(name, _)| name == "invoked"),
        "refused cancellation still invalidates old commands"
    );
    h.context()
        .set_native_context_menu_cancel_fails(window, false);
    h.update(|w, c| {
        state
            .invoke(
                &descriptor,
                "open_at",
                &json!({"position":{"x":23,"y":47}}),
                false,
                w,
                c,
            )
            .expect("reopen")
    });
    h.update(|w, c| {
        state
            .invoke(
                &descriptor,
                "set_target",
                &json!({"target":"record-b"}),
                false,
                w,
                c,
            )
            .expect("replace target")
    });
    h.context().run_until_parked();
    assert_eq!(
        seen.borrow().last(),
        Some(&("open".into(), json!("record-b")))
    );
    h.update(|w, c| {
        state
            .invoke(
                &descriptor,
                "set_items",
                &json!({"items":[{"kind":"command","id":"new","label":"New"}]}),
                false,
                w,
                c,
            )
            .expect("replace items")
    });
    h.context().run_until_parked();
    h.context().select_context_menu_item(window, &[0]);
    h.context().run_until_parked();
    assert!(seen.borrow().contains(&("invoked".into(), json!("new"))));
    h.update(|w, c| {
        state
            .invoke(
                &descriptor,
                "open_at",
                &json!({"position":{"x":71,"y":19}}),
                false,
                w,
                c,
            )
            .expect("open before removal")
    });
    h.remount(|_, _| div().into_any_element());
    h.update(|_, c| state.reconcile(&node("Kbd", json!({"keystroke":"k"})), c));
    h.context().run_until_parked();
    assert!(h.context().pending_context_menu_position(window).is_none());
}

#[gpui::test]
fn menu_replacement_keeps_native_focus_open_submenu_and_keyboard_identity(cx: &mut TestAppContext) {
    let state = Rc::new(State::default());
    let owner = state.clone();
    let desc = Rc::new(RefCell::new(node(
        "Menu",
        json!({"trigger":"Commands","items":[{"kind":"submenu","id":"more","label":"More","items":[{"kind":"command","id":"alpha","label":"Alpha"},{"kind":"command","id":"beta","label":"Beta"}]}]}),
    )));
    let source = desc.clone();
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let mut h = Harness::new(cx, gpui_kit::install, move |w, c| {
        let output = output.clone();
        owner.render(
            &source.borrow(),
            KitSlots::new(),
            w,
            c,
            Rc::new(move |name, value| output.borrow_mut().push((name.to_owned(), value))),
        )
    });
    h.update(|w, c| {
        state
            .invoke(&desc.borrow(), "open", &json!({}), false, w, c)
            .expect("open native menu")
    });
    h.update(|w, c| {
        state
            .invoke(
                &desc.borrow(),
                "open_submenu",
                &json!({"id":"more"}),
                false,
                w,
                c,
            )
            .expect("open native submenu")
    });
    h.keystrokes("down");
    let entity = h.update(|_, _| {
        let entries = state.entries.borrow();
        let Control::Menu(e) = &entries
            .get(&(0, "overlay".into()))
            .expect("retained menu")
            .control
        else {
            panic!()
        };
        e.clone()
    });
    let focus = h.update(|_, c| entity.read(c).focus_handle(c));
    desc.borrow_mut().props=serde_json::from_value(json!({"trigger":"Changed","items":[{"kind":"command","id":"outside","label":"Outside"},{"kind":"submenu","id":"more","label":"Renamed","items":[{"kind":"command","id":"beta","label":"Beta updated"},{"kind":"command","id":"alpha","label":"Alpha"}]}]})).expect("replacement menu props");
    h.frame();
    h.update(|w, c| {
        assert!(entity.read(c).is_open());
        assert_eq!(focus, entity.read(c).focus_handle(c));
        assert!(focus.is_focused(w));
    });
    h.keystrokes("enter");
    assert!(
        events.borrow().contains(&("invoked".into(), json!("beta"))),
        "{:?}",
        events.borrow()
    );
    h.update(|w, c| {
        assert_eq!(
            state
                .invoke(&desc.borrow(), "is_open", &json!({}), true, w, c)
                .expect("query menu after invocation"),
            false
        );
        state
            .invoke(&desc.borrow(), "open", &json!({}), false, w, c)
            .expect("reopen menu");
    });
    assert!(h.node("overlay.menu").is_some());
}

#[gpui::test]
fn drawer_live_slots_refusal_close_reopen_and_weak_removal(cx: &mut TestAppContext) {
    let state = Rc::new(State::default());
    let owner = state.clone();
    let desc = Rc::new(RefCell::new(node(
        "Drawer",
        json!({"title":"Original","dismissable":false}),
    )));
    let source = desc.clone();
    let label = Rc::new(RefCell::new("first".to_owned()));
    let body = label.clone();
    let mut h = Harness::new(cx, gpui_kit::install, move |w, c| {
        let label = body.borrow().clone();
        let mut slots = KitSlots::new();
        slots.insert(
            "content".into(),
            Rc::new(move |_, _| {
                div()
                    .w(px(220.))
                    .h(px(40.))
                    .child(label.clone())
                    .into_any_element()
            }),
        );
        owner.render(&source.borrow(), slots, w, c, Rc::new(|_, _| {}))
    });
    h.update(|w, c| {
        state
            .invoke(&desc.borrow(), "open", &json!({}), false, w, c)
            .expect("open drawer")
    });
    let weak = h.update(|_, _| {
        let entries = state.entries.borrow();
        let Control::Drawer(e) = &entries
            .get(&(0, "overlay".into()))
            .expect("retained drawer")
            .control
        else {
            panic!()
        };
        e.downgrade()
    });
    *label.borrow_mut() = "replacement".into();
    desc.borrow_mut()
        .props
        .insert("title".into(), json!("Updated"));
    h.frame();
    h.update(|w, c| {
        assert_eq!(
            state
                .invoke(&desc.borrow(), "is_open", &json!({}), true, w, c)
                .expect("query updated drawer"),
            true
        );
        assert!(
            state
                .invoke(&desc.borrow(), "dismiss", &json!({}), false, w, c)
                .is_err()
        );
        state
            .invoke(
                &desc.borrow(),
                "set_dismissable",
                &json!({"dismissable":true}),
                false,
                w,
                c,
            )
            .expect("allow dismissal");
        state
            .invoke(&desc.borrow(), "dismiss", &json!({}), false, w, c)
            .expect("dismiss drawer");
        state
            .invoke(&desc.borrow(), "settle", &json!({}), false, w, c)
            .expect("settle drawer");
        assert_eq!(
            state
                .invoke(&desc.borrow(), "is_open", &json!({}), true, w, c)
                .expect("query dismissed drawer"),
            false
        );
        state
            .invoke(&desc.borrow(), "open", &json!({}), false, w, c)
            .expect("reopen drawer");
    });
    h.remount(|_, _| div().into_any_element());
    h.update(|_, c| {
        let root: Node =
            serde_json::from_value(json!({"kind":"column","id":"empty"})).expect("empty root");
        state.reconcile(&root, c);
    });
    h.frame();
    h.update(|w, c| {
        assert!(weak.upgrade().is_none());
        assert!(
            state
                .invoke(&desc.borrow(), "open", &json!({}), false, w, c)
                .is_err()
        );
    });
}
