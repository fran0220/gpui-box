//! Component-specific data methods. The host owns owner scope, target lookup,
//! current generation/revision checks, request cancellation, and reply budgets.
use super::*;
use anyhow::{Result, bail, ensure};

impl KitState {
    pub(crate) fn invoke(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Value> {
        let component = node.component.as_deref().unwrap_or_default();
        let result_schema = validation::invocation(component, method, args, query)?;
        let entry = self
            .retained
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        ensure!(
            query || (!flag(node, "disabled") && !entry.route.borrow().disabled),
            "disabled target refuses invocation"
        );
        let string = |key: &str| args[key].as_str().unwrap_or_default().to_owned();
        let boolean = |key: &str| args[key].as_bool().unwrap_or(false);
        let control_size = || match args["size"].as_str() {
            Some("xs") => ControlSize::Xs,
            Some("sm") => ControlSize::Sm,
            Some("lg") => ControlSize::Lg,
            _ => ControlSize::Md,
        };
        let result = match (&entry.control, component, query) {
            (Control::Input(input), "TextInput", true) => {
                let input = input.read(cx);
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
            }
            (Control::Input(input), "TextInput", false) => {
                ensure!(
                    !input.read(cx).is_disabled(),
                    "disabled native input refuses invocation"
                );
                input.update(cx, |input, cx| -> Result<()> {
                    match method {
                        "set_name" => input.set_name(string("name"), cx),
                        "set_placeholder" => input.set_placeholder(string("placeholder"), cx),
                        "set_value" => input.set_value(string("value"), cx),
                        "set_text_quietly" => input.set_text_quietly(string("value"), cx),
                        "set_secret" => input.set_secret(boolean("secret"), cx),
                        "set_bare" => input.set_bare(boolean("bare"), cx),
                        "set_max_length" => input
                            .set_max_length(args["max_length"].as_u64().map(|n| n as usize), cx),
                        "set_disabled" => {
                            input.set_disabled(boolean("disabled"), cx);
                            entry.route.borrow_mut().disabled = boolean("disabled");
                        }
                        "set_read_only" => input.set_read_only(boolean("read_only"), cx),
                        "set_required" => input.set_required(boolean("required"), cx),
                        "set_invalid" => input.set_invalid(boolean("invalid"), cx),
                        "set_control_size" => input.set_control_size(control_size(), cx),
                        _ => bail!("unsupported TextInput command"),
                    }
                    Ok(())
                })?;
                Value::Null
            }
            (Control::Select(select), "Select", true) => {
                let select = select.read(cx);
                match method {
                    "selected_id" => json!(select.selected_id().map(|id| id.as_ref())),
                    "is_open" => json!(select.is_open()),
                    "is_disabled" => json!(select.is_disabled()),
                    "selected_option" => select.selected_option().map_or(Value::Null, |option| json!({
                        "id":option.id.as_ref(), "label":option.label.as_ref(), "disabled":option.disabled,
                        "description":option.description.as_ref().map(|value| value.as_ref()),
                        "group":option.group.as_ref().map(|value| value.as_ref()),
                    })),
                    _ => bail!("unsupported Select query"),
                }
            }
            (Control::Select(select), "Select", false) => {
                ensure!(
                    !select.read(cx).is_disabled(),
                    "disabled native select refuses invocation"
                );
                select.update(cx, |select, cx| -> Result<()> {
                    match method {
                        "set_name" => select.set_name(string("name"), cx),
                        "set_placeholder" => select.set_placeholder(
                            args["placeholder"]
                                .as_str()
                                .map(|value| value.to_owned().into()),
                            cx,
                        ),
                        "set_selected" => select.set_selected(
                            args["id"].as_str().map(|value| value.to_owned().into()),
                            cx,
                        ),
                        "set_options" => select.set_options(options(args.get("options")), cx),
                        "set_disabled" => {
                            select.set_disabled(boolean("disabled"), cx);
                            entry.route.borrow_mut().disabled = boolean("disabled");
                        }
                        "set_invalid" => select.set_invalid(boolean("invalid"), cx),
                        "set_clearable" => select.set_clearable(boolean("clearable"), cx),
                        "set_control_size" => select.set_control_size(control_size(), cx),
                        _ => bail!("unsupported Select command"),
                    }
                    Ok(())
                })?;
                Value::Null
            }
            (Control::Popover(popover, _), "Popover", true) => match method {
                "is_open" => json!(popover.read(cx).is_open()),
                "is_dismissable" => json!(popover.read(cx).is_dismissable()),
                _ => bail!("unsupported Popover query"),
            },
            (Control::Popover(popover, _), "Popover", false) => {
                popover.update(cx, |popover, cx| -> Result<()> {
                    match method {
                        "open" => popover.open(window, cx),
                        "close" => popover.close(window, cx),
                        "toggle" => popover.toggle(window, cx),
                        "dismiss" => {
                            ensure!(popover.is_dismissable(), "popover refuses dismissal");
                            popover.dismiss(window, cx);
                        }
                        "set_trigger" => popover.set_trigger(string("label"), cx),
                        "set_dismissable" => popover.set_dismissable(boolean("dismissable"), cx),
                        "set_placement" => popover.set_placement(
                            if string("placement") == "above" {
                                Placement::Above
                            } else {
                                Placement::Below
                            },
                            cx,
                        ),
                        "set_hang" => popover.set_hang(
                            if string("hang") == "end" {
                                Hang::End
                            } else {
                                Hang::Start
                            },
                            cx,
                        ),
                        _ => bail!("unsupported Popover command"),
                    }
                    Ok(())
                })?;
                Value::Null
            }
            (Control::Dialog(dialog, _), "Dialog", true) => match method {
                "is_open" => json!(dialog.read(cx).is_open()),
                "is_dismissable" => json!(dialog.read(cx).is_dismissable()),
                _ => bail!("unsupported Dialog query"),
            },
            (Control::Dialog(dialog, _), "Dialog", false) => {
                dialog.update(cx, |dialog, cx| -> Result<()> {
                    match method {
                        "open" => dialog.open(window, cx),
                        "close" => dialog.close(window, cx),
                        "confirm" => dialog.confirm(window, cx),
                        "cancel" => dialog.cancel(window, cx),
                        "dismiss" => {
                            ensure!(dialog.is_dismissable(), "dialog refuses dismissal");
                            dialog.dismiss(window, cx);
                        }
                        "set_title" => dialog.set_title(string("title"), cx),
                        "set_description" => dialog.set_description(
                            args["description"]
                                .as_str()
                                .map(|value| value.to_owned().into()),
                            cx,
                        ),
                        "set_confirm_label" => dialog.set_confirm_label(
                            args["label"].as_str().map(|value| value.to_owned().into()),
                            cx,
                        ),
                        "set_cancel_label" => dialog.set_cancel_label(
                            args["label"].as_str().map(|value| value.to_owned().into()),
                            cx,
                        ),
                        "set_dismissable" => dialog.set_dismissable(boolean("dismissable"), cx),
                        "set_destructive" => dialog.set_destructive(boolean("destructive"), cx),
                        _ => bail!("unsupported Dialog command"),
                    }
                    Ok(())
                })?;
                Value::Null
            }
            _ => bail!("native target component mismatch"),
        };
        validation::validate(&result, result_schema)?;
        Ok(result)
    }
}
