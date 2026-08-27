//! A caller-owned history whose entries can be revisited.
//!
//! The history does not keep an undo stack and does not decide what can be
//! restored. Entry order, the current entry, labels, already-formatted times,
//! sources, and refusals all belong to the caller. The component only renders
//! those facts and reports the identity of an entry the typist asked to jump
//! to.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TextTone, Theme, TypeScale};

use crate::foundation::{
    Disableable, FocusRing, Ident, Pressable, StyledExt, text as foundation_text,
};
use crate::strings::ActiveNumbers;

/// The local width reserved for the history rail.
const RAIL_WIDTH: f32 = 20.0;
/// The marker is local geometry: it occurs only on this rail.
const MARKER_SIZE: f32 = 10.0;
/// How far below the entry's own surface the marker centres on the first line
/// of the label. Local geometry: it depends on this rail's marker alone.
const MARKER_TOP: f32 = 5.0;

type JumpHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// One caller-owned point in an undo history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    id: SharedString,
    label: SharedString,
    description: Option<SharedString>,
    time: Option<SharedString>,
    source: Option<SharedString>,
    unavailable: Option<SharedString>,
}

impl HistoryEntry {
    /// `id` is the entry's durable business identity, never its list position.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            time: None,
            source: None,
            unavailable: None,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A time already formatted in the caller's locale and time zone.
    pub fn time(mut self, time: impl Into<SharedString>) -> Self {
        self.time = Some(time.into());
        self
    }

    /// Who or what produced the entry, in the caller's own words.
    pub fn source(mut self, source: impl Into<SharedString>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Keeps the entry visible while stating why it cannot be restored.
    pub fn unavailable(mut self, reason: impl Into<SharedString>) -> Self {
        self.unavailable = Some(reason.into());
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }
}

/// A history list that reports jump intents and changes no document state.
#[derive(IntoElement)]
pub struct UndoHistory {
    ident: Ident,
    label: SharedString,
    entries: Vec<HistoryEntry>,
    current: Option<SharedString>,
    disabled: bool,
    on_jump: Option<JumpHandler>,
}

impl std::fmt::Debug for UndoHistory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UndoHistory")
            .field("ident", &self.ident)
            .field("entries", &self.entries.len())
            .field("current", &self.current)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_jump.is_some())
            .finish()
    }
}

impl UndoHistory {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            entries: Vec::new(),
            current: None,
            disabled: false,
            on_jump: None,
        }
    }

    pub fn entry(mut self, entry: HistoryEntry) -> Self {
        self.entries.push(entry);
        self
    }

    pub fn entries(mut self, entries: impl IntoIterator<Item = HistoryEntry>) -> Self {
        self.entries.extend(entries);
        self
    }

    /// The entry the caller says the document currently holds.
    pub fn current(mut self, id: impl Into<SharedString>) -> Self {
        self.current = Some(id.into());
        self
    }

    pub fn on_jump(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_jump = Some(Rc::new(handler));
        self
    }
}

impl Disableable for UndoHistory {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for UndoHistory {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let count = self.entries.len();
        let entries = Rc::new(self.entries);
        let current = self.current;
        let handler = self.on_jump.filter(|_| !self.disabled);
        let mut list = div().column().w_full();

        for (index, entry) in entries.iter().enumerate() {
            list = list.child(entry_element(
                &self.ident,
                &theme,
                entry,
                index > 0,
                index + 1 < count,
                current.as_ref(),
                self.disabled,
                handler.as_ref(),
                cx,
            ));
        }

        if let Some(handler) = handler {
            let keyboard = Rc::clone(&handler);
            let entries = Rc::clone(&entries);
            list = list.on_key_down(move |event, window, cx| {
                let current_index = current
                    .as_ref()
                    .and_then(|current| entries.iter().position(|entry| &entry.id == current));
                let target = match event.keystroke.key.as_str() {
                    "up" => jump_target(
                        &entries,
                        current_index.and_then(|index| index.checked_sub(1)),
                        -1,
                        current.as_ref(),
                    ),
                    "down" => jump_target(
                        &entries,
                        current_index.map(|index| index + 1),
                        1,
                        current.as_ref(),
                    ),
                    "home" => jump_target(&entries, Some(0), 1, current.as_ref()),
                    "end" => {
                        jump_target(&entries, entries.len().checked_sub(1), -1, current.as_ref())
                    }
                    _ => None,
                };
                if let Some(target) = target {
                    keyboard(target, window, cx);
                    cx.stop_propagation();
                }
            });
        }

        list.semantic_in(
            cx,
            NodeSpec::new(self.ident.semantic_id(), Role::List)
                .text(self.label)
                .value(cx.numbers().count(count)),
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn entry_element(
    history: &Ident,
    theme: &Theme,
    entry: &HistoryEntry,
    follows: bool,
    continues: bool,
    current: Option<&SharedString>,
    history_disabled: bool,
    handler: Option<&JumpHandler>,
    cx: &mut App,
) -> AnyElement {
    let ident = history.child(entry.id.as_ref());
    let selected = current == Some(&entry.id);
    let disabled = history_disabled || entry.unavailable.is_some();
    let actionable = !disabled && !selected && handler.is_some();
    let marker_color = if selected {
        theme.colors.accent
    } else {
        theme.colors.hairline
    };

    // The entry's own padding lives on its surface rather than on the row, so
    // the rail spans the row edge to edge and the thread one entry leaves
    // meets the thread the next one starts. The lead is what puts the marker
    // on the first line of the label.
    let lead = theme.space(Space::Sm) + MARKER_TOP;
    let rail = div()
        .w(px(RAIL_WIDTH))
        .flex_none()
        .column()
        .items_center()
        .when(follows, |element| {
            element.child(
                div()
                    .h(px(lead))
                    .w(px(theme.borders.hairline))
                    .flex_none()
                    .bg(theme.colors.divider),
            )
        })
        .child(
            div()
                .when(!follows, |element| element.mt(px(lead)))
                .size(px(MARKER_SIZE))
                .flex_none()
                .rounded_full()
                .border(px(theme.borders.hairline))
                .border_color(marker_color)
                .when(selected, |element| element.bg(marker_color)),
        )
        .when(continues, |element| {
            element.child(
                div()
                    .w(px(theme.borders.hairline))
                    .flex_1()
                    .min_h(px(theme.space(Space::Lg)))
                    .bg(theme.colors.divider),
            )
        });

    let mut metadata = div().row().flex_wrap().gap_token(theme, Space::Sm);
    if let Some(source) = &entry.source {
        metadata = metadata.child(
            foundation_text(theme, TypeScale::Caption, source.clone())
                .text_tone(theme, TextTone::Muted)
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child("source").semantic_id(), Role::Text)
                        .parent(ident.semantic_id())
                        .text(source.clone()),
                ),
        );
    }
    if let Some(time) = &entry.time {
        metadata = metadata.child(
            foundation_text(theme, TypeScale::Caption, time.clone())
                .text_tone(theme, TextTone::Faint)
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child("time").semantic_id(), Role::Text)
                        .parent(ident.semantic_id())
                        .text(time.clone()),
                ),
        );
    }

    let mut content = div()
        .column()
        .gap(px(theme.space(Space::Xxs)))
        .child(
            foundation_text(theme, TypeScale::Label, entry.label.clone()).text_tone(
                theme,
                if disabled {
                    TextTone::Faint
                } else {
                    TextTone::Primary
                },
            ),
        )
        .children(entry.description.clone().map(|description| {
            foundation_text(theme, TypeScale::Body, description).text_tone(theme, TextTone::Muted)
        }))
        .when(entry.source.is_some() || entry.time.is_some(), |element| {
            element.child(metadata)
        });

    if let Some(reason) = &entry.unavailable {
        content = content.child(
            foundation_text(theme, TypeScale::Caption, reason.clone())
                .text_color(theme.colors.warning)
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child("reason").semantic_id(), Role::Text)
                        .parent(ident.semantic_id())
                        .text(reason.clone()),
                ),
        );
    }

    let hover_group = ident.child("hover").semantic_id();

    // The surface an entry wears starts to the right of the rail, so the
    // timeline stays continuous through the entry the document is on.
    let surface = div()
        .flex_1()
        .min_w_0()
        .column()
        .px_token(theme, Space::Sm)
        .py_token(theme, Space::Sm)
        .radius(theme, Radius::Control)
        .when(selected, |element| element.bg(theme.colors.selected))
        .when(actionable, |element| {
            element.group_hover(hover_group.clone(), |style| style.bg(theme.colors.hover))
        })
        .child(content);

    let mut row = div()
        .id(ident.element_id())
        .group(hover_group)
        .row()
        .items_stretch()
        .w_full()
        .gap_token(theme, Space::Sm)
        .px_token(theme, Space::Sm)
        .radius(theme, Radius::Control)
        .when(history_disabled, |element| {
            element.opacity(theme.opacity.disabled)
        })
        .child(rail)
        .child(surface);

    if !disabled && handler.is_some() {
        row = row.tab_index(0).pressable(cx).focus_ring(theme);
    }
    if actionable {
        row = row.cursor_pointer();
    }

    if let Some(handler) = handler {
        if actionable {
            let click = Rc::clone(handler);
            let id = entry.id.clone();
            row = row.on_click(move |_, window, cx| click(id.clone(), window, cx));
        }

        if !disabled {
            let keyboard = Rc::clone(handler);
            let id = entry.id.clone();
            row = row.on_key_down(move |event, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") && !selected {
                    keyboard(id.clone(), window, cx);
                    cx.stop_propagation();
                }
            });
        }
    }

    let mut spec = NodeSpec::new(ident.semantic_id(), Role::Row)
        .parent(history.semantic_id())
        .text(entry.label.clone())
        .selected(selected)
        .disabled(disabled);
    if let Some(description) = &entry.description {
        spec = spec.description(description.clone());
    }

    row.semantic_in(cx, spec).into_any_element()
}

fn jump_target(
    entries: &[HistoryEntry],
    start: Option<usize>,
    delta: isize,
    current: Option<&SharedString>,
) -> Option<SharedString> {
    let mut index = start? as isize;
    while index >= 0 && (index as usize) < entries.len() {
        let entry = &entries[index as usize];
        if entry.unavailable.is_none() && current != Some(&entry.id) {
            return Some(entry.id.clone());
        }
        index += delta;
    }
    None
}
