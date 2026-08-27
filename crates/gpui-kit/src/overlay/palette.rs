//! A filterable list of commands: the keyboard surface of an application.
//!
//! The palette is the one place a typist expects to find everything the
//! application can do, so it never hides a command it was given. A command the
//! host has marked unavailable is shown as unavailable, with the host's own
//! reason, and a query that matches nothing says so about that query instead
//! of drawing an empty list that looks like an application with no commands.

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TypeScale};

use crate::controls::input::{TextInput, TextInputEvent};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Ident, Pressable, StyledExt};
use crate::motion;
use crate::overlay::kbd::Kbd;
use crate::overlay::layer::{OverlaySurface, surface};
use crate::overlay::popover::{self, MenuKey};
use crate::strings::{ActiveSearch, ActiveStrings, SearchMatcher, StringKey};

/// How wide the palette is. The value occurs once, so it stays here rather
/// than in the token document.
const PALETTE_WIDTH: f32 = 480.0;
/// How tall the result list grows before it scrolls.
const RESULTS_MAX_HEIGHT: f32 = 320.0;

/// One thing the application can do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    id: SharedString,
    label: SharedString,
    section: Option<SharedString>,
    shortcut: Option<SharedString>,
    /// Why the host will not run this now. `None` means it will.
    unavailable: Option<SharedString>,
}

impl Command {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            section: None,
            shortcut: None,
            unavailable: None,
        }
    }

    /// The group this command belongs to. Sections stay contiguous, ordered by
    /// the best match inside them.
    pub fn section(mut self, section: impl Into<SharedString>) -> Self {
        self.section = Some(section.into());
        self
    }

    /// The keystroke that runs it without the palette.
    pub fn shortcut(mut self, keystroke: impl Into<SharedString>) -> Self {
        self.shortcut = Some(keystroke.into());
        self
    }

    /// Marks the command as one the host will not run now, in the host's own
    /// words. It is still listed: hiding a command a typist knows exists is a
    /// lie about the application.
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

    pub fn reason(&self) -> Option<&SharedString> {
        self.unavailable.as_ref()
    }

    pub fn is_available(&self) -> bool {
        self.unavailable.is_none()
    }
}

/// What the palette reports. The owner decides what any of it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandPaletteEvent {
    QueryChanged(SharedString),
    /// The highlighted command was taken.
    Invoked(SharedString),
    /// The palette was waved away with escape. The host owns whether it stays
    /// on screen, so the palette only reports the intent.
    Dismissed,
}

impl EventEmitter<CommandPaletteEvent> for CommandPalette {}

/// Orders the commands that answer `query`, keeping each section contiguous.
///
/// A section sits where its best match does, so the closest answer is still
/// the first row while the grouping stays readable.
#[allow(dead_code)]
fn order_matches(commands: &[Command], query: &str) -> Vec<usize> {
    order_matches_with(commands, query, &crate::strings::EnglishSearch)
}

fn order_matches_with(
    commands: &[Command],
    query: &str,
    matcher: &dyn SearchMatcher,
) -> Vec<usize> {
    let ranked: Vec<(usize, usize)> = commands
        .iter()
        .enumerate()
        .filter_map(|(index, command)| {
            matcher
                .rank(query, command.label.as_ref())
                .map(|rank| (rank, index))
        })
        .collect();

    let section_of = |index: usize| commands[index].section.clone().unwrap_or_default();
    let mut sections: Vec<(SharedString, usize, usize)> = Vec::new();
    for &(rank, index) in &ranked {
        let name = section_of(index);
        match sections.iter_mut().find(|(known, _, _)| *known == name) {
            Some(section) => section.1 = section.1.min(rank),
            None => sections.push((name, rank, index)),
        }
    }
    sections.sort_by_key(|(_, rank, first)| (*rank, *first));

    let mut ordered = Vec::with_capacity(ranked.len());
    for (name, _, _) in &sections {
        let mut group: Vec<(usize, usize)> = ranked
            .iter()
            .copied()
            .filter(|(_, index)| section_of(*index) == *name)
            .collect();
        group.sort_by_key(|&(rank, index)| (rank, index));
        ordered.extend(group.into_iter().map(|(_, index)| index));
    }
    ordered
}

/// A query field over a list of commands.
///
/// The palette owns the query and where the keyboard is; the command list and
/// whether the palette is on screen at all belong to the host.
pub struct CommandPalette {
    ident: Ident,
    focus_handle: FocusHandle,
    query: Entity<TextInput>,
    commands: Vec<Command>,
    /// The highlighted command, by identity, so filtering does not move the
    /// highlight onto whatever happens to sit at the same position.
    active: Option<SharedString>,
    slots: Slots,
    /// Held so the query subscription lives as long as the palette does.
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for CommandPalette {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommandPalette")
            .field("ident", &self.ident)
            .field("commands", &self.commands.len())
            .field("active", &self.active)
            .finish()
    }
}

impl CommandPalette {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let query = cx.new(|cx| {
            TextInput::new(ident.child("query"), window, cx)
                .placeholder(cx.strings().text(StringKey::PalettePlaceholder))
        });
        let subscription = cx.subscribe(&query, |palette, _query, event, cx| match event {
            TextInputEvent::Change(text) => {
                // A new query is a new list, so the highlight goes back to the
                // best answer rather than staying on a row that may be gone.
                palette.active = None;
                cx.emit(CommandPaletteEvent::QueryChanged(text.clone()));
                cx.notify();
            }
            TextInputEvent::Submit => palette.invoke(cx),
            TextInputEvent::Cancel => cx.emit(CommandPaletteEvent::Dismissed),
            _ => {}
        });

        Self {
            ident,
            focus_handle: cx.focus_handle(),
            query,
            commands: Vec::new(),
            active: None,
            slots: Slots::default(),
            _subscriptions: vec![subscription],
        }
    }

    pub fn commands(mut self, commands: impl IntoIterator<Item = Command>) -> Self {
        self.commands = commands.into_iter().collect();
        self
    }

    pub fn set_commands(&mut self, commands: Vec<Command>, cx: &mut Context<Self>) {
        self.commands = commands;
        self.active = None;
        cx.notify();
    }

    pub fn query(&self, cx: &App) -> SharedString {
        self.query.read(cx).value().clone()
    }

    /// Replaces the query from the host side, for example when the palette is
    /// opened with something already typed.
    pub fn set_query(&mut self, query: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.query
            .update(cx, |input, cx| input.set_value(query, cx));
    }

    pub fn query_input(&self) -> &Entity<TextInput> {
        &self.query
    }

    /// Moves the keyboard into the query field.
    pub fn focus_query(&self, window: &mut Window, cx: &mut App) {
        self.query.read(cx).focus_handle(cx).focus(window, cx);
    }

    /// The command the keyboard is on, or `None` when nothing can be taken.
    pub fn active_id(&self, cx: &App) -> Option<SharedString> {
        let ordered = self.ordered(cx);
        self.resolved(&ordered)
            .map(|index| self.commands[index].id.clone())
    }

    fn ordered(&self, cx: &App) -> Vec<usize> {
        order_matches_with(
            &self.commands,
            self.query.read(cx).value().as_ref(),
            cx.search().as_ref(),
        )
    }

    /// The command the highlight sits on: the one the typist put it on while
    /// it still answers the query, or the best answer that can be taken.
    fn resolved(&self, ordered: &[usize]) -> Option<usize> {
        if let Some(active) = &self.active
            && let Some(index) = ordered
                .iter()
                .copied()
                .find(|index| &self.commands[*index].id == active)
            && self.commands[index].is_available()
        {
            return Some(index);
        }
        ordered
            .iter()
            .copied()
            .find(|index| self.commands[*index].is_available())
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let ordered = self.ordered(cx);
        let choosable: Vec<usize> = ordered
            .iter()
            .copied()
            .filter(|index| self.commands[*index].is_available())
            .collect();
        if choosable.is_empty() {
            return;
        }
        let current = self
            .resolved(&ordered)
            .and_then(|index| choosable.iter().position(|choice| *choice == index));
        let Some(next) = popover::step(current, choosable.len(), delta) else {
            return;
        };
        self.active = Some(self.commands[choosable[next]].id.clone());
        cx.notify();
    }

    /// Reports the highlighted command. An unavailable command is never
    /// invoked, and no reachable row installs a handler for one.
    fn invoke(&mut self, cx: &mut Context<Self>) {
        let ordered = self.ordered(cx);
        let Some(index) = self.resolved(&ordered) else {
            return;
        };
        cx.emit(CommandPaletteEvent::Invoked(
            self.commands[index].id.clone(),
        ));
    }

    fn choose(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.active = Some(id.clone());
        cx.emit(CommandPaletteEvent::Invoked(id));
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let key = popover::classify_key(
            event.keystroke.key.as_str(),
            event.keystroke.modifiers.platform,
            event.keystroke.modifiers.control,
        );
        match key {
            MenuKey::Down => {
                self.step(1, cx);
                cx.stop_propagation();
            }
            MenuKey::Up => {
                self.step(-1, cx);
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    fn results(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let theme = cx.theme().clone();
        let ordered = self.ordered(cx);
        let highlighted = self.resolved(&ordered);
        let results_id = self.ident.child("results").semantic_id();
        let mut section: Option<SharedString> = None;
        let mut rows: Vec<gpui::AnyElement> = Vec::with_capacity(ordered.len());
        let count = ordered.len();

        for (position, index) in ordered.into_iter().enumerate() {
            let command = &self.commands[index];
            if command.section != section {
                section = command.section.clone();
                if let Some(name) = section.clone() {
                    rows.push(
                        popover::heading(&theme, name.as_ref())
                            .semantic_in(
                                cx,
                                NodeSpec::new(
                                    self.ident
                                        .child("section")
                                        .child(name.as_ref())
                                        .semantic_id(),
                                    Role::Heading,
                                )
                                .parent(results_id.clone())
                                .level(2)
                                .text(name),
                            )
                            .into_any_element(),
                    );
                }
            }

            let row_ident = self.ident.child(command.id.as_ref());
            let active = highlighted == Some(index);
            let available = command.is_available();
            let id = command.id.clone();
            let mut spec = NodeSpec::new(row_ident.semantic_id(), Role::MenuItem)
                .parent(results_id.clone())
                .text(command.label.clone())
                .disabled(!available)
                .hovered(active);
            if let Some(reason) = command.reason() {
                spec = spec.value(reason.clone());
            }

            let row = popover::menu_row(&theme, false, active)
                .id(row_ident.element_id())
                .when(available, |element| element.cursor_pointer().pressable(cx))
                .when(!available, |element| {
                    element.opacity(theme.opacity.disabled)
                })
                .child(div().flex_1().child(command.label.clone()))
                .children(command.reason().map(|reason| {
                    div()
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.warning)
                        .child(reason.clone())
                }))
                .children(
                    command
                        .shortcut
                        .clone()
                        .map(|keystroke| Kbd::new(keystroke).into_any_element()),
                )
                .when(available, |element| {
                    element.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |palette, _, _, cx| {
                            palette.choose(id.clone(), cx);
                        }),
                    )
                })
                .semantic_in(cx, spec);

            rows.push(
                motion::row_in(
                    row_ident.child("in").element_id(),
                    &theme,
                    position,
                    count,
                    row,
                )
                .into_any_element(),
            );
        }

        rows
    }
}

impl Focusable for CommandPalette {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Slotted for CommandPalette {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let query = self.query.read(cx).value().clone();
        let rows = self.results(cx);
        let results_id = self.ident.child("results").semantic_id();

        let body = if rows.is_empty() {
            // A query that answered nothing is a fact about the query, not an
            // application without commands, so it says which query it was.
            self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(
                    self.ident.child("empty"),
                    cx.strings().format(StringKey::PaletteNoMatch, &[&query]),
                )
                .kind(EmptyKind::Empty)
                .detail(cx.strings().text(StringKey::PaletteEmptyDetail))
                .into_any_element()
            })
        } else {
            div()
                .id(self.ident.child("results").element_id())
                .flex()
                .flex_col()
                .max_h(px(RESULTS_MAX_HEIGHT))
                .overflow_y_scroll()
                .children(rows)
                .semantic_in(cx, NodeSpec::new(results_id, Role::Menu))
                .into_any_element()
        };

        surface(&theme, OverlaySurface::MODAL)
            .w(px(PALETTE_WIDTH))
            .p_token(&theme, Space::Xs)
            .gap_token(&theme, Space::Xs)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .child(div().p_token(&theme, Space::Xs).child(self.query.clone()))
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group).value(query),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commands() -> Vec<Command> {
        vec![
            Command::new("editor.split", "Split editor").section("Editor"),
            Command::new("workspace.save", "Save workspace").section("Workspace"),
            Command::new("editor.save", "Save file")
                .section("Editor")
                .shortcut("cmd-s"),
            Command::new("workspace.publish", "Publish workspace")
                .section("Workspace")
                .unavailable("Approval is required"),
        ]
    }

    #[test]
    fn an_empty_query_lists_everything_grouped_by_section() {
        assert_eq!(order_matches(&commands(), ""), vec![0, 2, 1, 3]);
    }

    #[test]
    fn a_section_sits_where_its_best_match_does() {
        // "Save workspace" is a prefix match, so its section leads even though
        // the editor section was declared first.
        assert_eq!(order_matches(&commands(), "save"), vec![1, 2]);
    }

    #[test]
    fn a_command_the_host_refused_is_still_listed() {
        let ordered = order_matches(&commands(), "publish");
        assert_eq!(ordered, vec![3]);
        assert!(!commands()[3].is_available());
    }

    #[test]
    fn a_query_nothing_answers_orders_nothing() {
        assert!(order_matches(&commands(), "zzz").is_empty());
    }
}
