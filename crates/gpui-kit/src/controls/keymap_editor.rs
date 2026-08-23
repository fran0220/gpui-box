//! Caller-owned keymap facts with transient, shared shortcut recording.

use gpui::{
    AppContext, Context, Entity, EventEmitter, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, Styled, Subscription, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TypeScale};

use crate::controls::button::Button;
use crate::controls::keybinding_recorder::{KeybindingRecorder, KeybindingRecorderEvent};
use crate::foundation::{Disableable, Ident, StyledExt, text as foundation_text};
use crate::overlay::Kbd;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// One effective binding, identified independently of its position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapBinding {
    id: SharedString,
    keystroke: SharedString,
    conflict: Option<SharedString>,
    provenance: Option<SharedString>,
}

impl KeymapBinding {
    pub fn new(id: impl Into<SharedString>, keystroke: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            keystroke: keystroke.into(),
            conflict: None,
            provenance: None,
        }
    }
    pub fn conflict(mut self, value: impl Into<SharedString>) -> Self {
        self.conflict = Some(value.into());
        self
    }
    pub fn provenance(mut self, value: impl Into<SharedString>) -> Self {
        self.provenance = Some(value.into());
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn keystroke(&self) -> &SharedString {
        &self.keystroke
    }

    pub fn conflict_reason(&self) -> Option<&SharedString> {
        self.conflict.as_ref()
    }

    pub fn provenance_label(&self) -> Option<&SharedString> {
        self.provenance.as_ref()
    }
}

/// Caller-owned command metadata and binding facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapCommand {
    id: SharedString,
    label: SharedString,
    context: Option<SharedString>,
    default_bindings: Vec<SharedString>,
    effective_bindings: Vec<KeymapBinding>,
    search_text: SharedString,
    keywords: Vec<SharedString>,
    refusal: Option<SharedString>,
}

impl KeymapCommand {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            context: None,
            default_bindings: vec![],
            effective_bindings: vec![],
            search_text: "".into(),
            keywords: vec![],
            refusal: None,
        }
    }
    pub fn context(mut self, value: impl Into<SharedString>) -> Self {
        self.context = Some(value.into());
        self
    }
    pub fn defaults(mut self, values: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.default_bindings = values.into_iter().map(Into::into).collect();
        self
    }
    pub fn bindings(mut self, values: impl IntoIterator<Item = KeymapBinding>) -> Self {
        self.effective_bindings = values.into_iter().collect();
        self
    }
    pub fn searchable(
        mut self,
        text: impl Into<SharedString>,
        keywords: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.search_text = text.into();
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }
    pub fn refused(mut self, reason: impl Into<SharedString>) -> Self {
        self.refusal = Some(reason.into());
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn label_text(&self) -> &SharedString {
        &self.label
    }

    pub fn context_label(&self) -> Option<&SharedString> {
        self.context.as_ref()
    }

    pub fn default_bindings(&self) -> &[SharedString] {
        &self.default_bindings
    }

    pub fn effective_bindings(&self) -> &[KeymapBinding] {
        &self.effective_bindings
    }

    pub fn search_text(&self) -> &SharedString {
        &self.search_text
    }

    pub fn keywords(&self) -> &[SharedString] {
        &self.keywords
    }

    pub fn refusal_reason(&self) -> Option<&SharedString> {
        self.refusal.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeymapEditorEvent {
    AddCaptured {
        command_id: SharedString,
        keystroke: SharedString,
    },
    Remove {
        command_id: SharedString,
        binding_id: SharedString,
    },
    Reset {
        command_id: SharedString,
    },
    RecordingCancelled {
        command_id: SharedString,
    },
}

impl EventEmitter<KeymapEditorEvent> for KeymapEditor {}

/// Coordinates one recorder over caller-owned keymap rows; it never applies a binding.
pub struct KeymapEditor {
    ident: Ident,
    commands: Vec<KeymapCommand>,
    query: SharedString,
    disabled: bool,
    active_command: Option<SharedString>,
    suppress_next_recorder_cancel: bool,
    recorder: Entity<KeybindingRecorder>,
    _recorder_subscription: Subscription,
}

impl std::fmt::Debug for KeymapEditor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KeymapEditor")
            .field("ident", &self.ident)
            .field("commands", &self.commands)
            .field("query", &self.query)
            .field("disabled", &self.disabled)
            .field("active_command", &self.active_command)
            .finish_non_exhaustive()
    }
}

impl KeymapEditor {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let recorder = cx.new(|cx| {
            KeybindingRecorder::new(ident.child("recorder"), window, cx)
                .label(cx.strings().text(StringKey::KeymapAdd))
        });
        let subscription = cx.subscribe(&recorder, |this, _, event, cx| {
            match event {
                KeybindingRecorderEvent::Captured(keystroke) => {
                    let Some(command_id) = this.active_command.take() else {
                        return;
                    };
                    cx.emit(KeymapEditorEvent::AddCaptured {
                        command_id,
                        keystroke: keystroke.clone(),
                    })
                }
                KeybindingRecorderEvent::Cancelled => {
                    if this.suppress_next_recorder_cancel {
                        this.suppress_next_recorder_cancel = false;
                        return;
                    }
                    let Some(command_id) = this.active_command.take() else {
                        return;
                    };
                    cx.emit(KeymapEditorEvent::RecordingCancelled { command_id })
                }
                KeybindingRecorderEvent::Started => return,
            }
            cx.notify();
        });
        Self {
            ident,
            commands: vec![],
            query: "".into(),
            disabled: false,
            active_command: None,
            suppress_next_recorder_cancel: false,
            recorder,
            _recorder_subscription: subscription,
        }
    }

    pub fn commands(mut self, commands: impl IntoIterator<Item = KeymapCommand>) -> Self {
        self.commands = commands.into_iter().collect();
        self
    }
    pub fn query(mut self, query: impl Into<SharedString>) -> Self {
        self.query = query.into();
        self
    }
    pub fn set_commands(&mut self, commands: Vec<KeymapCommand>, cx: &mut Context<Self>) {
        self.commands = commands;
        self.cancel_if_active_is_hidden(cx);
        cx.notify();
    }
    pub fn set_query(&mut self, query: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.query = query.into();
        self.cancel_if_active_is_hidden(cx);
        cx.notify();
    }

    /// Refuses every editor action without removing caller-owned bindings.
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.recorder.update(cx, |recorder, cx| recorder.cancel(cx));
        }
        cx.notify();
    }

    pub fn active_command(&self) -> Option<&SharedString> {
        self.active_command.as_ref()
    }

    pub fn current_commands(&self) -> &[KeymapCommand] {
        &self.commands
    }

    fn matches(&self, command: &KeymapCommand) -> bool {
        let query = self.query.to_lowercase();
        query.is_empty()
            || [&command.id, &command.label, &command.search_text]
                .into_iter()
                .chain(command.context.iter())
                .chain(command.keywords.iter())
                .any(|value| value.to_lowercase().contains(&query))
    }

    fn cancel_if_active_is_hidden(&mut self, cx: &mut Context<Self>) {
        let visible = self.active_command.as_ref().is_none_or(|active| {
            self.commands.iter().any(|command| {
                &command.id == active && command.refusal.is_none() && self.matches(command)
            })
        });
        if !visible {
            self.recorder.update(cx, |recorder, cx| recorder.cancel(cx));
        }
    }
}

impl Disableable for KeymapEditor {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Render for KeymapEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let visible: Vec<_> = self
            .commands
            .iter()
            .filter(|command| self.matches(command))
            .cloned()
            .collect();
        let count = visible.len();
        let root_id = self.ident.semantic_id();
        let disabled = self.disabled;
        let entity = cx.entity().clone();
        div()
            .id(self.ident.element_id())
            .column()
            .gap_token(&theme, Space::Md)
            .child(
                foundation_text(
                    &theme,
                    TypeScale::Caption,
                    cx.strings().format_plural(
                        StringKey::KeymapResultOne,
                        StringKey::KeymapResultCount,
                        cx.numbers().plural(count),
                        &[cx.numbers().count(count).as_ref()],
                    ),
                )
                .text_tone(&theme, gpui_kit_theme::TextTone::Muted)
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("status").semantic_id(), Role::Status)
                        .parent(root_id.clone())
                        .value(cx.numbers().count(count)),
                ),
            )
            .children(visible.into_iter().map(|command| {
                let row = self.ident.child(command.id.as_ref());
                let refused = command.refusal.is_some();
                let actionable = !disabled && !refused;
                let active = self.active_command.as_ref() == Some(&command.id);
                let effective: Vec<_> = command
                    .effective_bindings
                    .iter()
                    .map(|binding| binding.keystroke.clone())
                    .collect();
                let changed = effective != command.default_bindings;
                let mut actions = div().row().gap_token(&theme, Space::Xs);
                if actionable {
                    let target = command.id.clone();
                    let editor = entity.clone();
                    actions = actions.child(
                        Button::new(row.child("add"))
                            .label(cx.strings().text(StringKey::KeymapAdd))
                            .ghost()
                            .semantic_parent(row.semantic_id())
                            .on_click(move |window, cx| {
                                editor.update(cx, |this, cx| {
                                    if let Some(command_id) = this.active_command.take() {
                                        this.suppress_next_recorder_cancel = true;
                                        this.recorder.update(cx, |field, cx| field.cancel(cx));
                                        cx.emit(KeymapEditorEvent::RecordingCancelled {
                                            command_id,
                                        });
                                    }
                                    this.active_command = Some(target.clone());
                                    this.recorder
                                        .update(cx, |field, cx| field.start(window, cx));
                                    cx.notify();
                                });
                            }),
                    );
                    if changed {
                        let target = command.id.clone();
                        let editor = entity.clone();
                        actions = actions.child(
                            Button::new(row.child("reset"))
                                .label(cx.strings().text(StringKey::KeymapReset))
                                .ghost()
                                .semantic_parent(row.semantic_id())
                                .on_click(move |_, cx| {
                                    editor.update(cx, |_, cx| {
                                        cx.emit(KeymapEditorEvent::Reset {
                                            command_id: target.clone(),
                                        })
                                    })
                                }),
                        );
                    }
                }
                let bindings = command.effective_bindings.iter().map(|binding| {
                    let binding_suffix = format!("binding.{}", binding.id);
                    let binding_id = row.child(binding_suffix);
                    let mut spec = NodeSpec::new(binding_id.semantic_id(), Role::Group)
                        .parent(row.semantic_id())
                        .value(binding.keystroke.clone());
                    let description = [binding.conflict.clone(), binding.provenance.clone()]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join("; ");
                    if !description.is_empty() {
                        spec = spec.description(description);
                    }
                    let mut line = div()
                        .row()
                        .w_full()
                        .items_center()
                        .gap_token(&theme, Space::Sm)
                        .child(Kbd::new(binding.keystroke.clone()).id(binding_id.child("keys")))
                        .child(
                            div()
                                .row()
                                .flex_1()
                                .min_w_0()
                                .gap_token(&theme, Space::Sm)
                                .children(binding.conflict.clone().map(|reason| {
                                    foundation_text(&theme, TypeScale::Body, reason.clone())
                                        .text_color(theme.colors.danger)
                                        .semantic_in(
                                            cx,
                                            NodeSpec::new(
                                                binding_id.child("conflict").semantic_id(),
                                                Role::Status,
                                            )
                                            .parent(binding_id.semantic_id())
                                            .invalid(true)
                                            .text(reason),
                                        )
                                }))
                                .children(binding.provenance.clone().map(|provenance| {
                                    foundation_text(&theme, TypeScale::Body, provenance.clone())
                                        .text_tone(&theme, gpui_kit_theme::TextTone::Muted)
                                        .semantic_in(
                                            cx,
                                            NodeSpec::new(
                                                binding_id.child("provenance").semantic_id(),
                                                Role::Status,
                                            )
                                            .parent(binding_id.semantic_id())
                                            .text(provenance),
                                        )
                                })),
                        );
                    if actionable {
                        let editor = entity.clone();
                        let command_id = command.id.clone();
                        let id = binding.id.clone();
                        line = line.child(
                            Button::new(binding_id.child("remove"))
                                .label(cx.strings().text(StringKey::KeymapRemove))
                                .ghost()
                                .semantic_parent(binding_id.semantic_id())
                                .on_click(move |_, cx| {
                                    editor.update(cx, |_, cx| {
                                        cx.emit(KeymapEditorEvent::Remove {
                                            command_id: command_id.clone(),
                                            binding_id: id.clone(),
                                        })
                                    })
                                }),
                        );
                    }
                    line.semantic_in(cx, spec)
                });
                let effective_value = if command.effective_bindings.is_empty() {
                    cx.strings().text(StringKey::KeybindingUnbound)
                } else {
                    SharedString::from(
                        command
                            .effective_bindings
                            .iter()
                            .map(|binding| binding.keystroke.as_ref())
                            .collect::<Vec<_>>()
                            .join(", "),
                    )
                };
                let defaults_value = if command.default_bindings.is_empty() {
                    cx.strings().text(StringKey::KeybindingUnbound)
                } else {
                    SharedString::from(
                        command
                            .default_bindings
                            .iter()
                            .map(SharedString::as_ref)
                            .collect::<Vec<_>>()
                            .join(", "),
                    )
                };
                let defaults = command.default_bindings.iter().cloned().map(Kbd::new);
                div()
                    .column()
                    .gap_token(&theme, Space::Sm)
                    .p(px(theme.space(Space::Sm)))
                    .child(
                        div()
                            .row()
                            .items_start()
                            .justify_between()
                            .child(
                                div()
                                    .column()
                                    .child(foundation_text(
                                        &theme,
                                        TypeScale::Subtitle,
                                        command.label.clone(),
                                    ))
                                    .children(command.context.clone().map(|context| {
                                        foundation_text(&theme, TypeScale::Body, context)
                                            .text_tone(&theme, gpui_kit_theme::TextTone::Muted)
                                    })),
                            )
                            .child(actions),
                    )
                    .child(
                        div()
                            .id(row.child("effective").element_id())
                            .column()
                            .gap_token(&theme, Space::Xs)
                            .child(foundation_text(
                                &theme,
                                TypeScale::Label,
                                cx.strings().text(StringKey::KeymapEffective),
                            ))
                            .children((command.effective_bindings.is_empty()).then(|| {
                                foundation_text(&theme, TypeScale::Body, effective_value.clone())
                            }))
                            .children(bindings)
                            .semantic_in(
                                cx,
                                NodeSpec::new(row.child("effective").semantic_id(), Role::Group)
                                    .parent(row.semantic_id())
                                    .value(effective_value),
                            ),
                    )
                    .child(
                        div()
                            .id(row.child("defaults").element_id())
                            .row()
                            .items_center()
                            .gap_token(&theme, Space::Xs)
                            .child(foundation_text(
                                &theme,
                                TypeScale::Label,
                                cx.strings().text(StringKey::KeymapDefaults),
                            ))
                            .children(defaults)
                            .semantic_in(
                                cx,
                                NodeSpec::new(row.child("defaults").semantic_id(), Role::Group)
                                    .parent(row.semantic_id())
                                    .value(defaults_value),
                            ),
                    )
                    .children(command.refusal.clone().map(|reason| {
                        foundation_text(&theme, TypeScale::Body, reason.clone()).semantic_in(
                            cx,
                            NodeSpec::new(row.child("refusal").semantic_id(), Role::Status)
                                .parent(row.semantic_id())
                                .text(reason),
                        )
                    }))
                    .children(active.then(|| self.recorder.clone()))
                    .semantic_in(
                        cx,
                        command
                            .context
                            .clone()
                            .map_or_else(
                                || NodeSpec::new(row.semantic_id(), Role::Row),
                                |context| {
                                    NodeSpec::new(row.semantic_id(), Role::Row).description(context)
                                },
                            )
                            .parent(root_id.clone())
                            .text(command.label)
                            .disabled(disabled || refused),
                    )
            }))
            .semantic_in(cx, NodeSpec::new(root_id, Role::Group).disabled(disabled))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command() -> KeymapCommand {
        KeymapCommand::new("workbench.open", "Open item")
            .context("Workspace")
            .defaults(["cmd-o", "ctrl-o"])
            .bindings([
                KeymapBinding::new("primary", "cmd-shift-o")
                    .conflict("Already assigned")
                    .provenance("User keymap"),
                KeymapBinding::new("alternate", "ctrl-o"),
            ])
            .searchable("open a workspace item", ["file", "picker"])
    }

    #[test]
    fn model_preserves_all_caller_owned_binding_facts() {
        let command = command();
        assert_eq!(command.default_bindings(), ["cmd-o", "ctrl-o"]);
        assert_eq!(command.effective_bindings().len(), 2);
        assert_eq!(
            command.effective_bindings()[0]
                .conflict_reason()
                .map(SharedString::as_ref),
            Some("Already assigned")
        );
        assert_eq!(
            command.effective_bindings()[0]
                .provenance_label()
                .map(SharedString::as_ref),
            Some("User keymap")
        );
        assert_ne!(
            command
                .effective_bindings()
                .iter()
                .map(KeymapBinding::keystroke)
                .collect::<Vec<_>>(),
            command.default_bindings().iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn filtering_uses_deliberately_supplied_metadata_case_insensitively() {
        // Construct only enough editor state to test its product-neutral predicate.
        let matches = |query: &str| {
            let query = query.to_lowercase();
            let command = command();
            query.is_empty()
                || [command.id(), command.label_text(), command.search_text()]
                    .into_iter()
                    .chain(command.context_label())
                    .chain(command.keywords())
                    .any(|value| value.to_lowercase().contains(&query))
        };
        for query in ["WORKBENCH", "item", "workspace", "file", "picker"] {
            assert!(matches(query), "{query}");
        }
        assert!(!matches("terminal"));
    }
}
