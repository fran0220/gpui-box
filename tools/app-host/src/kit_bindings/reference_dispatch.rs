//! Fixed dispatchers for borrowed native entities. Never route an opaque handle
//! through a descriptor snapshot or a second retained-component registry.
use super::*;
use crate::references::Registration;
use anyhow::{Result, bail, ensure};

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
