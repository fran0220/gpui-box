//! Fixed dispatchers for borrowed native entities. Never route an opaque handle
//! through a descriptor snapshot or a second retained-component registry.
use super::*;
use crate::references::Registration;
use anyhow::{Result, bail, ensure};

/// Dispatch a borrowed native Menu through the same implementation as the
/// descriptor family. Ancestor lifetime and disabled checks belong to Registry.
pub(crate) fn menu(
    target: &Entity<gpui_kit::prelude::Menu>,
    method: &str,
    args: &Value,
    query: bool,
    window: &mut Window,
    cx: &mut App,
    refs: &Registration<'_>,
) -> Result<Value> {
    if query && method == "focus_handle" {
        let schema = validation::invocation("Menu", method, args, true)?;
        let focus = gpui::Focusable::focus_handle(target.read(cx), cx);
        let value = refs.focus(
            target,
            &focus,
            |menu, focus, cx| gpui::Focusable::focus_handle(menu, cx) == *focus,
            |_, _| true,
        )?;
        validation::validate(&value, schema)?;
        return Ok(value);
    }
    super::overlay_extra::menu_value(target, method, args, query, window, cx)
}

pub(crate) fn text_input(
    target: &Entity<TextInput>,
    method: &str,
    args: &Value,
    query: bool,
    _window: &mut Window,
    cx: &mut App,
    refs: &Registration<'_>,
) -> Result<Value> {
    if query && method == "focus_handle" {
        let schema = validation::invocation("TextInput", method, args, true)?;
        let focus = gpui::Focusable::focus_handle(target.read(cx), cx);
        let value = refs.focus(
            target,
            &focus,
            |input, focus, cx| gpui::Focusable::focus_handle(input, cx) == *focus,
            |input, _| !input.is_disabled(),
        )?;
        validation::validate(&value, schema)?;
        return Ok(value);
    }
    text_input_value(target, method, args, query, cx)
}

impl KitState {
    pub(crate) fn reference_query(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        window: &mut Window,
        cx: &mut App,
        refs: &Registration<'_>,
    ) -> Option<Result<Value>> {
        if let Some(result) = self
            .controls_extra
            .reference_query(node, method, args, cx, refs)
        {
            return Some(result);
        }
        let component = node.component.as_deref().unwrap_or_default();
        if overlay_extra::COMPONENTS.contains(&component)
            && (method == "focus_handle"
                || (component == "CommandPalette" && method == "query_input"))
        {
            return Some((|| {
                let schema = validation::invocation(component, method, args, true)?;
                let value = if method == "query_input" {
                    self.overlay_extra.query_input(node, cx, refs, text_input)?
                } else {
                    self.overlay_extra
                        .invoke_reference(node, method, args, true, cx, refs)?
                };
                validation::validate(&value, schema)?;
                Ok(value)
            })());
        }
        if node.component.as_deref() != Some("TextInput") || method != "focus_handle" {
            return None;
        }
        let retained = self
            .retained
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .cloned();
        Some((|| {
            let retained =
                retained.ok_or_else(|| anyhow::anyhow!("native input is not mounted"))?;
            let Control::Input(input) = &retained.control else {
                bail!("wrong native input identity")
            };
            text_input(input, method, args, true, window, cx, refs)
        })())
    }
}

pub(super) fn text_input_value(
    target: &Entity<TextInput>,
    method: &str,
    args: &Value,
    query: bool,
    cx: &mut App,
) -> Result<Value> {
    let schema = validation::invocation("TextInput", method, args, query)?;
    let result = if query {
        let input = target.read(cx);
        match method {
            "value" => json!(input.value().as_ref()),
            "is_empty" => json!(input.is_empty()),
            "is_disabled" => json!(input.is_disabled()),
            "is_secret" => json!(input.is_secret()),
            "cursor_offset" => json!(input.cursor_offset()),
            "selected_range" => {
                let range = input.selected_range();
                json!({"start":range.start,"end":range.end})
            }
            _ => bail!("unsupported TextInput query"),
        }
    } else {
        ensure!(
            !target.read(cx).is_disabled(),
            "disabled native input refuses invocation"
        );
        target.update(cx, |input, cx| -> Result<()> {
            let string = |key: &str| args[key].as_str().unwrap_or_default().to_owned();
            let boolean = |key: &str| args[key].as_bool().unwrap_or(false);
            match method {
                "set_name" => input.set_name(string("name"), cx),
                "set_placeholder" => input.set_placeholder(string("placeholder"), cx),
                "set_value" => input.set_value(string("value"), cx),
                "set_text_quietly" => input.set_text_quietly(string("value"), cx),
                "set_secret" => input.set_secret(boolean("secret"), cx),
                "set_bare" => input.set_bare(boolean("bare"), cx),
                "set_max_length" => {
                    input.set_max_length(args["max_length"].as_u64().map(|n| n as usize), cx)
                }
                "set_disabled" => input.set_disabled(boolean("disabled"), cx),
                "set_read_only" => input.set_read_only(boolean("read_only"), cx),
                "set_required" => input.set_required(boolean("required"), cx),
                "set_invalid" => input.set_invalid(boolean("invalid"), cx),
                "set_control_size" => input.set_control_size(
                    match args["size"].as_str() {
                        Some("xs") => ControlSize::Xs,
                        Some("sm") => ControlSize::Sm,
                        Some("lg") => ControlSize::Lg,
                        _ => ControlSize::Md,
                    },
                    cx,
                ),
                _ => bail!("unsupported TextInput command"),
            }
            Ok(())
        })?;
        Value::Null
    };
    validation::validate(&result, schema)?;
    Ok(result)
}

#[cfg(all(test, feature = "capture"))]
mod tests {
    use super::*;
    use gpui::{AppContext, EffectOwner, TestAppContext, div};
    use gpui_kit::prelude::{Menu, MenuEvent};
    use gpui_kit_testkit::harness::Harness;

    #[gpui::test]
    fn borrowed_menu_uses_shared_methods_and_inherited_guards(cx: &mut TestAppContext) {
        struct Parent {
            menu: Option<Entity<Menu>>,
            disabled: bool,
        }
        let mut harness = Harness::new(cx, gpui_kit::install, |_, _| div().into_any_element());
        let (events, _subscription, _target) = harness.update(|window, cx| {
            let target = cx.new(|cx| Menu::new("borrowed-menu", window, cx));
            let events = Rc::new(RefCell::new(Vec::new()));
            let observe = events.clone();
            let subscription = cx.subscribe(&target, move |_, event: &MenuEvent, _| {
                observe.borrow_mut().push(event.clone());
            });
            let parent = cx.new(|_| Parent { menu: Some(target.clone()), disabled: false });
            let registry = crate::references::Registry::new();
            let owner = EffectOwner::new();
            let node: Node = serde_json::from_value(json!({"kind":"kit","id":"source","instance":1,"component":"SplitButton"})).expect("source");
            let reference = registry.registration(&node, owner).entity(
                "Menu", &parent, &target,
                |parent, _| parent.menu.clone(),
                |parent, _| !parent.disabled,
                menu,
            ).expect("native menu reference");
            let items = json!({"items":[{"kind":"submenu","id":"more","label":"More","items":[{"kind":"command","id":"pin","label":"Pin"}]}]});
            assert_eq!(registry.invoke(owner,&reference,"set_items",&items,false,window,cx).expect("shared setter"),Value::Null);
            assert_eq!(target.read(cx).offered()[0].id().as_ref(), "more");
            let duplicate = json!({"items":[{"kind":"submenu","id":"same","label":"More","items":[{"kind":"command","id":"same","label":"Duplicate"}]}]});
            assert!(registry.invoke(owner,&reference,"set_items",&duplicate,false,window,cx).is_err());
            assert_eq!(target.read(cx).offered()[0].id().as_ref(), "more");
            assert_eq!(registry.invoke(owner,&reference,"open_submenu",&json!({"id":"missing"}),false,window,cx).expect("missing submenu"),json!(false));
            assert_eq!(registry.invoke(owner,&reference,"open_submenu",&json!({"id":"more"}),false,window,cx).expect("submenu"),json!(true));
            let focus = registry.invoke(owner,&reference,"focus_handle",&json!({}),true,window,cx).expect("focus ref");
            assert!(registry.invoke(owner,&reference,"focus_handle",&json!({"extra":true}),true,window,cx).is_err());
            assert!(registry.invoke(owner,&reference,"set_items",&items,true,window,cx).is_err());
            parent.update(cx, |parent,_| parent.disabled = true);
            assert!(registry.invoke(owner,&reference,"close",&json!({}),false,window,cx).is_err());
            assert!(registry.invoke(owner,&focus,"focus",&json!({}),false,window,cx).is_err());
            assert_eq!(registry.invoke(owner,&reference,"is_open",&json!({}),true,window,cx).expect("disabled query"),json!(true));
            parent.update(cx, |parent,_| { parent.disabled = false; parent.menu = None; });
            assert!(registry.invoke(owner,&reference,"is_open",&json!({}),true,window,cx).is_err());
            assert!(registry.invoke(owner,&focus,"is_focused",&json!({}),true,window,cx).is_err());
            assert!(target.read(cx).is_open());
            (events, subscription, target)
        });
        assert_eq!(&*events.borrow(), &[MenuEvent::Opened]);
    }
}
