//! A select you can type into.
//!
//! The list, the choice, and whether a typed value is acceptable all belong
//! to the caller. The combobox owns the query, whether the list is open, and
//! where the keyboard is — and reports what was picked without moving its own
//! answer, exactly as `Select` does.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Entity, EventEmitter, FocusHandle,
    Focusable, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement, Pixels,
    Render, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Subscription, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TypeScale};

use crate::controls::field::{FieldState, field_shell};
use crate::controls::input::{LineEnd, LineStart, TextInput, TextInputEvent};
use crate::controls::select::SelectOption;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::{
    Disableable, Ident, Pressable, Sizable, StyledExt, text as foundation_text,
};
use crate::layout::measure;
use crate::motion;
use crate::overlay::Placement;
use crate::overlay::popover::{self, MenuKey};
use crate::strings::{ActiveStrings, StringKey};

/// How wide the list gets before it stops growing, and how tall before it
/// scrolls. Both occur once, so they stay next to the component.
const MENU_MIN_WIDTH: f32 = 200.0;
const MENU_MAX_HEIGHT: f32 = 320.0;

/// What a combobox reports. The owner decides what any of it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComboboxEvent {
    QueryChanged(SharedString),
    /// One of the offered options was taken.
    Selected(SharedString),
    /// A value nothing offered answers, reported only when the caller allows
    /// custom values.
    Custom(SharedString),
    Opened,
    Closed,
}

impl EventEmitter<ComboboxEvent> for Combobox {}

/// A [`Select`](crate::controls::select::Select) you can type into.
///
/// It owns the query and whether the menu is open. Which option holds is the
/// caller's answer, so escape puts the query back to it and reports nothing.
pub struct Combobox {
    ident: Ident,
    query: Entity<TextInput>,
    options: Vec<SelectOption>,
    selected: Option<SharedString>,
    name: SharedString,
    placeholder: Option<SharedString>,
    size: ControlSize,
    disabled: bool,
    invalid: bool,
    open: bool,
    /// The highlighted option by identity, so filtering does not move the
    /// highlight onto whatever happens to sit at the same position.
    active: Option<SharedString>,
    allow_custom: bool,
    /// Whether the current answer has been put in the field. The text is the
    /// typist's afterwards, so it is written once.
    seeded: bool,
    scroll: ScrollHandle,
    trigger_bounds: Rc<Cell<Bounds<Pixels>>>,
    reveal_active: bool,
    menu_geometry: Option<popover::MenuGeometry>,
    /// Held so the query subscription lives as long as the combobox does.
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for Combobox {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Combobox")
            .field("ident", &self.ident)
            .field("options", &self.options.len())
            .field("selected", &self.selected)
            .field("open", &self.open)
            .field("allow_custom", &self.allow_custom)
            .finish()
    }
}

impl Combobox {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let query = cx.new(|cx| TextInput::new(ident.child("query"), window, cx).bare(true));
        let subscription = cx.subscribe(&query, |combobox, _query, event, cx| match event {
            TextInputEvent::Change(text) => combobox.on_query(text.clone(), cx),
            TextInputEvent::Submit => combobox.commit(cx),
            TextInputEvent::Cancel => combobox.revert(cx),
            _ => {}
        });

        Self {
            ident,
            query,
            options: Vec::new(),
            selected: None,
            name: SharedString::default(),
            placeholder: None,
            size: ControlSize::Md,
            disabled: false,
            invalid: false,
            open: false,
            active: None,
            allow_custom: false,
            seeded: false,
            scroll: ScrollHandle::new(),
            trigger_bounds: Rc::default(),
            reveal_active: false,
            menu_geometry: None,
            _subscriptions: vec![subscription],
        }
    }

    pub fn options(mut self, options: impl IntoIterator<Item = SelectOption>) -> Self {
        self.options = options.into_iter().collect();
        self
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    /// Names both the combobox and its editable query target.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = name.into();
        self
    }

    pub fn set_name(&mut self, name: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.name = name.into();
        self.query
            .update(cx, |query, cx| query.set_name(self.name.clone(), cx));
        cx.notify();
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Whether a query nothing answers may be reported as a new value.
    ///
    /// With it off the combobox reports nothing at all for such a query, and
    /// the list says that this is a closed set rather than drawing an empty
    /// list that looks like a list with nothing in it.
    pub fn allow_custom(mut self, allow_custom: bool) -> Self {
        self.allow_custom = allow_custom;
        self
    }

    pub fn set_options(&mut self, options: Vec<SelectOption>, cx: &mut Context<Self>) {
        let still_offered = self
            .selected
            .as_ref()
            .is_some_and(|id| options.iter().any(|option| &option.id == id));
        if !still_offered {
            self.selected = None;
        }
        self.options = options;
        self.active = None;
        self.reveal_active = true;
        cx.notify();
    }

    /// Replaces the choice from the host side, and puts its label back in the
    /// field so the query and the answer agree again.
    pub fn set_selected(&mut self, id: Option<SharedString>, cx: &mut Context<Self>) {
        self.selected = id;
        self.seeded = true;
        if self.open {
            self.active = self.selected.as_ref().and_then(|id| {
                self.options
                    .iter()
                    .find(|option| &option.id == id && !option.disabled)
                    .map(|option| option.id.clone())
            });
        }
        self.reveal_active = true;
        let label = self.selected_label().unwrap_or_default();
        self.query
            .update(cx, |query, cx| query.set_text_quietly(label, cx));
        cx.notify();
    }

    pub fn set_query(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        // Once something is being asked, the field is no longer waiting for
        // the answer's label to be written into it.
        self.seeded = true;
        self.query.update(cx, |query, cx| query.set_value(text, cx));
    }

    pub fn selected_id(&self) -> Option<&SharedString> {
        self.selected.as_ref()
    }

    pub fn selected_option(&self) -> Option<&SelectOption> {
        let id = self.selected.as_ref()?;
        self.options.iter().find(|option| &option.id == id)
    }

    fn selected_label(&self) -> Option<SharedString> {
        self.selected_option().map(|option| option.label.clone())
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn query_text(&self, cx: &App) -> SharedString {
        self.query.read(cx).value().clone()
    }

    pub fn query_input(&self) -> &Entity<TextInput> {
        &self.query
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.query
            .update(cx, |query, cx| query.set_disabled(disabled, cx));
        if disabled {
            self.open = false;
        }
        cx.notify();
    }

    /// The query the list is filtered by.
    ///
    /// A query that is exactly the current answer is not a filter: it is what
    /// the field says while nothing has been typed, and it must not hide the
    /// rest of the list the moment the control opens.
    fn filter(&self, cx: &App) -> SharedString {
        let text = self.query.read(cx).value().clone();
        match self.selected_label() {
            Some(label) if label == text => SharedString::default(),
            _ => text,
        }
    }

    /// The options answering the query, best answer first.
    fn matches(&self, cx: &App) -> Vec<usize> {
        let labels: Vec<&str> = self
            .options
            .iter()
            .map(|option| option.label.as_ref())
            .collect();
        popover::filter_indices_for(cx, self.filter(cx).as_ref(), &labels)
    }

    /// The option the highlight sits on: the one the typist put it on while
    /// it still answers the query, or the best answer that can be taken.
    fn resolved(&self, matches: &[usize]) -> Option<usize> {
        if let Some(active) = &self.active
            && let Some(index) = matches
                .iter()
                .copied()
                .find(|index| &self.options[*index].id == active)
            && !self.options[index].disabled
        {
            return Some(index);
        }
        matches
            .iter()
            .copied()
            .find(|index| !self.options[*index].disabled)
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        if self.disabled || self.open {
            return;
        }
        if self.filter(cx).is_empty() {
            self.active = self.selected.as_ref().and_then(|id| {
                self.options
                    .iter()
                    .find(|option| &option.id == id && !option.disabled)
                    .map(|option| option.id.clone())
            });
        }
        self.open = true;
        self.reveal_active = true;
        cx.emit(ComboboxEvent::Opened);
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.active = None;
        cx.emit(ComboboxEvent::Closed);
        cx.notify();
    }

    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.close(cx);
        } else {
            self.open(cx);
            self.query.read(cx).focus_handle(cx).focus(window, cx);
        }
    }

    fn on_query(&mut self, text: SharedString, cx: &mut Context<Self>) {
        // A new query is a new list, so the highlight goes back to the best
        // answer rather than staying on a row that may be gone.
        self.active = None;
        self.reveal_active = true;
        self.open(cx);
        cx.emit(ComboboxEvent::QueryChanged(text));
        cx.notify();
    }

    /// Puts the query back to the answer that still holds, and closes without
    /// reporting anything: abandoning an edit is not a choice.
    fn revert(&mut self, cx: &mut Context<Self>) {
        let label = self.selected_label().unwrap_or_default();
        self.query
            .update(cx, |query, cx| query.set_text_quietly(label, cx));
        self.close(cx);
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        let matches = self.matches(cx);
        if let Some(index) = self.resolved(&matches) {
            self.take(index, cx);
            return;
        }
        let typed = self.query.read(cx).value().clone();
        if self.allow_custom && !typed.trim().is_empty() {
            cx.emit(ComboboxEvent::Custom(typed));
            self.close(cx);
        }
    }

    /// Reports the option and closes. The field goes back to the answer the
    /// host still holds, because a report is not an application.
    fn take(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(option) = self.options.get(index) else {
            return;
        };
        if option.disabled {
            return;
        }
        let id = option.id.clone();
        let label = self.selected_label().unwrap_or_default();
        self.query
            .update(cx, |query, cx| query.set_text_quietly(label, cx));
        cx.emit(ComboboxEvent::Selected(id));
        self.close(cx);
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let matches = self.matches(cx);
        let choosable: Vec<usize> = matches
            .iter()
            .copied()
            .filter(|index| !self.options[*index].disabled)
            .collect();
        if choosable.is_empty() {
            return;
        }
        let current = self
            .resolved(&matches)
            .and_then(|index| choosable.iter().position(|choice| *choice == index));
        let Some(next) = popover::step(current, choosable.len(), delta) else {
            return;
        };
        self.active = Some(self.options[choosable[next]].id.clone());
        self.reveal_active = true;
        cx.notify();
    }

    fn edge(&mut self, from_end: bool, cx: &mut Context<Self>) {
        let matches = self.matches(cx);
        let index = if from_end {
            matches
                .iter()
                .rev()
                .copied()
                .find(|index| !self.options[*index].disabled)
        } else {
            matches
                .iter()
                .copied()
                .find(|index| !self.options[*index].disabled)
        };
        let next = index.map(|index| self.options[index].id.clone());
        if next == self.active {
            return;
        }
        self.active = next;
        self.reveal_active = true;
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let raw = event.keystroke.key.as_str();
        let key = popover::classify_key(
            raw,
            event.keystroke.modifiers.platform,
            event.keystroke.modifiers.control,
        );
        match key {
            MenuKey::Down => {
                self.open(cx);
                self.step(1, cx);
                cx.stop_propagation();
            }
            MenuKey::Up => {
                self.open(cx);
                self.step(-1, cx);
                cx.stop_propagation();
            }
            _ if self.open && raw == "home" => {
                self.edge(false, cx);
                cx.stop_propagation();
            }
            _ if self.open && raw == "end" => {
                self.edge(true, cx);
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    fn on_line_start(&mut self, _: &LineStart, _: &mut Window, cx: &mut Context<Self>) {
        if self.open && !self.disabled {
            self.edge(false, cx);
            cx.stop_propagation();
        }
    }

    fn on_line_end(&mut self, _: &LineEnd, _: &mut Window, cx: &mut Context<Self>) {
        if self.open && !self.disabled {
            self.edge(true, cx);
            cx.stop_propagation();
        }
    }

    /// The placeholder the host gave, or the shared default word for a field
    /// that has not been answered yet.
    fn resolved_placeholder(&self, cx: &App) -> SharedString {
        self.placeholder
            .clone()
            .unwrap_or_else(|| cx.strings().text(StringKey::SelectPlaceholder))
    }

    fn menu(&mut self, geometry: popover::MenuGeometry, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let matches = self.matches(cx);
        let highlighted = self.resolved(&matches);
        let highlighted_position = highlighted
            .and_then(|highlighted| matches.iter().position(|index| *index == highlighted));
        let menu_ident = self.ident.child("menu");

        let rows = if matches.is_empty() {
            let query = self.filter(cx);
            vec![
                EmptyState::new(
                    self.ident.child("empty"),
                    cx.strings()
                        .format(StringKey::ComboboxNoMatch, &[query.as_ref()]),
                )
                .kind(EmptyKind::Empty)
                .detail(cx.strings().text(if self.allow_custom {
                    StringKey::ComboboxCreateHint
                } else {
                    StringKey::ComboboxClosedHint
                }))
                .into_any_element(),
            ]
        } else {
            let mut rows = Vec::new();
            let mut last_group: Option<SharedString> = None;
            for (position, index) in matches.iter().enumerate() {
                let option = &self.options[*index];
                if option.group != last_group {
                    if let Some(group) = option.group.clone() {
                        rows.push(self.group_heading(&group, cx));
                    }
                    last_group = option.group.clone();
                }
                rows.push(self.row(*index, highlighted, position, matches.len(), cx));
            }
            rows
        };

        if self.menu_geometry != Some(geometry) {
            self.menu_geometry = Some(geometry);
            self.reveal_active = true;
        }
        if self.reveal_active {
            if let Some(position) = highlighted_position {
                self.scroll.scroll_to_item(position);
            }
            self.reveal_active = false;
        }

        let viewport = div()
            .p(px(theme.space(Space::Xs)))
            .column()
            .max_h(px(geometry.max_height))
            .id(self.ident.child("menu.scroll").element_id())
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .children(rows);
        let list = popover::card_flush(&theme)
            .w(px(geometry.width))
            .max_h(px(geometry.max_height))
            .id(menu_ident.element_id())
            .child(viewport)
            .semantic_in(
                cx,
                NodeSpec::new(menu_ident.semantic_id(), Role::Menu)
                    .parent(self.ident.semantic_id()),
            )
            .into_any_element();

        popover::menu_overlay(
            &self.ident.child("menu.anchor"),
            &theme,
            geometry.placement,
            list,
        )
    }

    fn group_heading(&self, label: &SharedString, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let ident = self.ident.child("group").child(label.as_ref());
        foundation_text(&theme, TypeScale::Caption, label.clone())
            .px(px(theme.space(Space::Sm)))
            .py(px(theme.space(Space::Xs)))
            .text_color(theme.colors.text_faint)
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Text)
                    .parent(self.ident.child("menu").semantic_id())
                    .text(label.clone()),
            )
            .into_any_element()
    }

    fn row(
        &self,
        index: usize,
        highlighted: Option<usize>,
        position: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let option = &self.options[index];
        let selected = self.selected.as_ref() == Some(&option.id);
        let active = highlighted == Some(index);
        let ident = self.ident.child(option.id.as_ref());
        let hover_group = ident.child("hover").semantic_id();

        let row = popover::menu_row(&theme, selected, active)
            .id(ident.element_id())
            .group(hover_group.clone())
            .when(!option.disabled, |element| {
                element.cursor_pointer().pressable(cx)
            })
            .child(
                div()
                    .column()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(popover::menu_label_state(
                        &theme,
                        option.label.clone(),
                        selected,
                        active,
                        option.disabled,
                        hover_group,
                    ))
                    .when_some(option.description.clone(), |element, description| {
                        element.child(
                            foundation_text(&theme, TypeScale::Caption, description).text_color(
                                if option.disabled {
                                    theme.colors.text_disabled
                                } else {
                                    theme.colors.text_muted
                                },
                            ),
                        )
                    }),
            )
            .when(selected, |element| {
                element.child(
                    div().ml_auto().child(
                        icon(Icon::Check)
                            .size(px(14.0))
                            .text_color(theme.colors.text),
                    ),
                )
            })
            .when(!option.disabled, |element| {
                element.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |combobox, _, _, cx| combobox.take(index, cx)),
                )
            })
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Option)
                    .parent(self.ident.child("menu").semantic_id())
                    .checked(selected)
                    .disabled(option.disabled)
                    .hovered(active)
                    .text(option.label.clone()),
            );

        motion::row_in(ident.child("in").element_id(), &theme, position, count, row)
            .into_any_element()
    }
}

impl Disableable for Combobox {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Combobox {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for Combobox {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.query.read(cx).focus_handle(cx)
    }
}

impl Render for Combobox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        if !self.seeded {
            self.seeded = true;
            if let Some(label) = self.selected_label() {
                self.query
                    .update(cx, |query, cx| query.set_text_quietly(label, cx));
            }
        }
        if self.query.read(cx).placeholder_text().is_empty() {
            let placeholder = self.resolved_placeholder(cx);
            self.query
                .update(cx, |query, cx| query.set_placeholder(placeholder, cx));
        }
        if self.query.read(cx).accessible_name() != &self.name {
            let name = self.name.clone();
            self.query.update(cx, |query, cx| query.set_name(name, cx));
        }
        if self.disabled != self.query.read(cx).is_disabled() {
            let disabled = self.disabled;
            self.query
                .update(cx, |query, cx| query.set_disabled(disabled, cx));
        }

        let focused = self.query.read(cx).focus_handle(cx).is_focused(window);
        let geometry = self.open.then(|| {
            popover::menu_geometry(
                window,
                self.trigger_bounds.get(),
                &theme,
                MENU_MAX_HEIGHT,
                MENU_MIN_WIDTH,
            )
        });
        let placement = geometry.map_or(Placement::Below, |geometry| geometry.placement);
        let menu = geometry.map(|geometry| self.menu(geometry, cx));
        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Combobox)
            .disabled(self.disabled)
            .invalid(self.invalid)
            .expanded(self.open)
            .text(self.name.clone())
            .placeholder(self.resolved_placeholder(cx));
        if !self.disabled {
            spec = spec.focus(&self.query.read(cx).focus_handle(cx));
        }
        if let Some(label) = self.selected_label() {
            spec = spec.value(label);
        }

        let shell = field_shell(
            &theme,
            self.size,
            FieldState::default()
                .focused(focused)
                .invalid(self.invalid)
                .disabled(self.disabled),
        )
        .id(self.ident.child("shell").element_id())
        .when(!self.disabled, |element| {
            element.on_mouse_down(
                MouseButton::Left,
                cx.listener(|combobox, _, window, cx| {
                    if !combobox.open {
                        combobox.toggle(window, cx);
                    }
                }),
            )
        })
        .child(div().flex_1().child(self.query.clone()))
        .child(
            icon(Icon::AltArrowDown)
                .size(px(theme.control.get(self.size).icon_size * 0.9))
                .text_color(theme.colors.text_muted),
        );
        let measured = Rc::clone(&self.trigger_bounds);
        let trigger = div()
            .w_full()
            .on_children_prepainted(move |bounds, window, _| {
                if let Some(trigger) = bounds.first() {
                    measure::record(&measured, *trigger, window);
                }
            })
            .child(shell)
            .into_any_element();

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            // Home and End belong to the query while closed, but to the open
            // list while it is visible. Capture lets the combobox make that
            // distinction before TextInput consumes its caret action.
            .capture_action(cx.listener(Self::on_line_start))
            .capture_action(cx.listener(Self::on_line_end))
            .capture_key_down(cx.listener(Self::on_key_down))
            .child(popover::anchored_slot(placement, trigger, menu))
            .semantic_in(cx, spec)
    }
}
