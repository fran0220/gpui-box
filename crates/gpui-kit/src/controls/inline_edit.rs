//! Text that becomes a field when it is asked to.
//!
//! Whether the field is open, and what it holds when it opens, are the
//! caller's. `InlineEdit` reports that an edit was requested, committed, or
//! abandoned, and writes nothing.
//!
//! # A failed save keeps what was typed
//!
//! Losing someone's typing because the host refused the save is the worst
//! thing this component could do. So the field is built once when an editing
//! session opens and is left alone for as long as the session stays open: a
//! caller that answers a commit with [`InlineEdit::failure`] and keeps
//! `editing` true gets the typed text still there, under the reason it did not
//! save.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    App, AppContext, Entity, Focusable, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, TypeScale};

use crate::controls::input::{self, TextInput};
use crate::controls::textarea::{self, TextArea};
use crate::foundation::window_state;
use crate::foundation::{
    Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt, text as foundation_text,
};
use crate::strings::{ActiveStrings, StringKey};

type EditHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type CommitHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// The field an open editing session holds.
#[derive(Clone)]
enum Editor {
    Line(Entity<TextInput>),
    Block(Entity<TextArea>),
}

impl Editor {
    fn value(&self, cx: &App) -> SharedString {
        match self {
            Self::Line(field) => field.read(cx).value().clone(),
            Self::Block(field) => field.read(cx).value().clone(),
        }
    }

    fn focus(&self, window: &mut Window, cx: &mut App) {
        let handle = match self {
            Self::Line(field) => field.read(cx).focus_handle(cx),
            Self::Block(field) => field.read(cx).focus_handle(cx),
        };
        window.focus(&handle, cx);
    }

    fn is_block(&self) -> bool {
        matches!(self, Self::Block(_))
    }
}

/// What one identity remembers between two frames.
///
/// A `RenderOnce` builder cannot carry anything across frames, so the open
/// field lives in an application global keyed by the component's identity —
/// the same arrangement [`crate::layout::measure`] and [`crate::data::DataGrid`]
/// use.
#[derive(Default)]
struct Memory {
    editor: RefCell<Option<Editor>>,
}

type Memories = HashMap<SharedString, Rc<Memory>>;

fn memory(id: &SharedString, window: &Window, cx: &mut App) -> Rc<Memory> {
    window_state::with(
        window.window_handle().window_id(),
        cx,
        |memories: &mut Memories| Rc::clone(memories.entry(id.clone()).or_default()),
    )
}

/// Text that a click or enter turns into a field.
#[derive(IntoElement)]
pub struct InlineEdit {
    ident: Ident,
    value: SharedString,
    placeholder: Option<SharedString>,
    editing: bool,
    multiline: bool,
    rows: usize,
    failure: Option<SharedString>,
    size: ControlSize,
    disabled: bool,
    on_edit: Option<EditHandler>,
    on_commit: Option<CommitHandler>,
    on_cancel: Option<EditHandler>,
}

impl std::fmt::Debug for InlineEdit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InlineEdit")
            .field("ident", &self.ident)
            .field("editing", &self.editing)
            .field("multiline", &self.multiline)
            .field("failed", &self.failure.is_some())
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl InlineEdit {
    pub fn new(ident: impl Into<Ident>, value: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            value: value.into(),
            placeholder: None,
            editing: false,
            multiline: false,
            rows: 3,
            failure: None,
            size: ControlSize::Md,
            disabled: false,
            on_edit: None,
            on_commit: None,
            on_cancel: None,
        }
    }

    /// What to show where an empty value would be, so a blank line is still
    /// something to aim at.
    /// The placeholder the host gave, or the built-in default word for a
    /// value that is not there.
    fn resolved_placeholder(&self, cx: &App) -> SharedString {
        self.placeholder
            .clone()
            .unwrap_or_else(|| cx.strings().text(StringKey::InlineEditPlaceholder))
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Whether the caller has opened the field. The component never opens it
    /// on its own; a click reports the request.
    pub fn editing(mut self, editing: bool) -> Self {
        self.editing = editing;
        self
    }

    /// Edits over a [`TextArea`] rather than a [`TextInput`], so enter inserts
    /// a line and the platform modifier plus enter commits.
    pub fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        self
    }

    /// How many rows a multi-line field opens at.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// Why the last save did not take, in the host's own words. The typed text
    /// stays exactly as it was.
    pub fn failure(mut self, failure: impl Into<SharedString>) -> Self {
        self.failure = Some(failure.into());
        self
    }

    /// Reports that the typist asked to edit.
    pub fn on_edit(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_edit = Some(Rc::new(handler));
        self
    }

    /// Reports the text the field held when the edit ended.
    pub fn on_commit(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_commit = Some(Rc::new(handler));
        self
    }

    /// Reports that the edit was abandoned. Nothing was typed as far as the
    /// host is concerned.
    pub fn on_cancel(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(handler));
        self
    }

    /// The field for the open session, built once when the session opens.
    fn editor(&self, state: &Rc<Memory>, window: &mut Window, cx: &mut App) -> Editor {
        let existing = state
            .editor
            .borrow()
            .clone()
            .filter(|editor| editor.is_block() == self.multiline);
        if let Some(editor) = existing {
            return editor;
        }

        let ident = self.ident.child("field");
        let editor = if self.multiline {
            let rows = self.rows;
            let value = self.value.clone();
            let placeholder = self.resolved_placeholder(cx);
            Editor::Block(cx.new(|cx| {
                TextArea::new(ident, window, cx)
                    .text(value)
                    .placeholder(placeholder)
                    .rows(rows)
            }))
        } else {
            let value = self.value.clone();
            let placeholder = self.resolved_placeholder(cx);
            Editor::Line(cx.new(|cx| {
                TextInput::new(ident, window, cx)
                    .text(value)
                    .placeholder(placeholder)
                    .bare(true)
            }))
        };
        *state.editor.borrow_mut() = Some(editor.clone());
        editor.focus(window, cx);
        editor
    }
}

impl Disableable for InlineEdit {
    /// Refuses editing entirely: no click, no key, and no field, whatever the
    /// caller says about `editing`.
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for InlineEdit {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for InlineEdit {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let state = memory(&self.ident.semantic_id(), window, cx);
        let editing = self.editing && !self.disabled;

        if !editing {
            // The session is over; the next one starts from the caller's value
            // rather than from whatever the last one left behind.
            state.editor.borrow_mut().take();
        }

        let failure = self.failure.clone().map(|reason| {
            foundation_text(&theme, TypeScale::Body, reason.clone())
                .text_color(theme.colors.danger)
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("failure").semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .invalid(true)
                        .text(reason),
                )
        });

        if !editing {
            let actionable = !self.disabled && self.on_edit.is_some();
            let empty = self.value.is_empty();
            let mut reading = div()
                .id(self.ident.element_id())
                .row()
                .min_h(px(metrics.height))
                .px(px(theme.space(Space::Xs)))
                .radius(&theme, Radius::Control)
                .child(
                    foundation_text(
                        &theme,
                        TypeScale::Label,
                        if empty {
                            self.resolved_placeholder(cx)
                        } else {
                            self.value.clone()
                        },
                    )
                    .text_size(px(metrics.font_size))
                    .text_color(if self.disabled || empty {
                        theme.colors.text_faint
                    } else {
                        theme.colors.text
                    }),
                )
                .when(actionable, |element| {
                    element
                        .cursor_pointer()
                        .tab_index(0)
                        .pressable(cx)
                        .hover(|style| style.bg(theme.colors.hover))
                        .focus_ring(&theme)
                });

            if let (true, Some(handler)) = (actionable, self.on_edit.clone()) {
                let key = Rc::clone(&handler);
                reading = reading
                    .on_click(move |_, window, cx| handler(window, cx))
                    .on_key_down(move |event, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            key(window, cx);
                            cx.stop_propagation();
                        }
                    });
            }

            let mut spec = NodeSpec::new(
                self.ident.semantic_id(),
                if actionable { Role::Button } else { Role::Text },
            )
            .disabled(!actionable)
            .invalid(self.failure.is_some())
            .value(if empty { "empty" } else { "reading" });
            if empty {
                spec = spec.placeholder(self.resolved_placeholder(cx));
            } else {
                spec = spec.text(self.value.clone());
            }

            return div()
                .column()
                .w_full()
                .gap(px(theme.space(Space::Xs)))
                .child(reading)
                .children(failure)
                .semantic_in(cx, spec)
                .into_any_element();
        }

        let editor = self.editor(&state, window, cx);
        let reading = editor.clone();
        let commit = self.on_commit.clone().map(|handler| {
            let editor = reading.clone();
            move |window: &mut Window, cx: &mut App| {
                handler(editor.value(cx), window, cx);
            }
        });
        let cancel = self.on_cancel.clone();

        let mut frame = div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap(px(theme.space(Space::Xs)));

        if let Some(commit) = commit.clone() {
            // A press anywhere else ends the edit the way leaving a field
            // does, which is the other half of "enter or blur commits".
            frame = frame.on_mouse_down_out(move |_, window, cx| commit(window, cx));
        }

        // The field's own bindings dispatch before an ancestor's key listener,
        // so the two chords are taken as actions in the capture phase rather
        // than as keystrokes that never arrive.
        if let Some(commit) = commit {
            let line = commit.clone();
            frame = frame
                .capture_action::<input::Submit>(move |_, window, cx| {
                    line(window, cx);
                    cx.stop_propagation();
                })
                .capture_action::<textarea::Submit>(move |_, window, cx| {
                    commit(window, cx);
                    cx.stop_propagation();
                });
        }

        if let Some(cancel) = cancel {
            let line = Rc::clone(&cancel);
            frame = frame
                .capture_action::<input::Cancel>(move |_, window, cx| {
                    line(window, cx);
                    cx.stop_propagation();
                })
                .capture_action::<textarea::Cancel>(move |_, window, cx| {
                    cancel(window, cx);
                    cx.stop_propagation();
                });
        }

        let field = match editor {
            Editor::Line(field) => div()
                .w_full()
                .min_h(px(metrics.height))
                .px(px(theme.space(Space::Xs)))
                .radius(&theme, Radius::Control)
                .well(&theme)
                .when(self.failure.is_some(), |element| {
                    element.border_color(theme.colors.danger)
                })
                .shadow(theme.focus_ring())
                .text_size(px(metrics.font_size))
                .child(field),
            Editor::Block(field) => div().w_full().child(field),
        };

        frame
            .child(field)
            .children(failure)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .text(self.value.clone())
                    .invalid(self.failure.is_some())
                    .value(if self.failure.is_some() {
                        "failed"
                    } else {
                        "editing"
                    }),
            )
            .into_any_element()
    }
}
