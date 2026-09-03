//! A searchable listbox that can hold more than one caller-owned option.
//!
//! `MultiSelect` deliberately keeps the same controlled boundary as
//! [`super::select::Select`]: opening, query text, and keyboard focus are
//! transient view state, while the selected ids remain the caller's state.
//! Selecting an option emits an intent; it never mutates the selected set.

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
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, TypeScale};

use crate::controls::field::{FieldState, field_shell};
use crate::controls::input::{TextInput, TextInputEvent};
use crate::controls::select::SelectOption;
use crate::display::tag::Tag;
use crate::foundation::{Disableable, Ident, Pressable, Sizable, StyledExt, text};
use crate::layout::measure;
use crate::overlay::popover::{self, MenuKey};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// What a multi-select reports. The owner decides whether the intent changes
/// its selected ids or query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiSelectEvent {
    /// Toggle the option with this business identity.
    Toggled(SharedString),
    /// Remove a selected option from the set.
    Removed(SharedString),
    /// Clear all selected options.
    Cleared,
    /// The typist changed the search query.
    QueryChanged(SharedString),
    Opened,
    Closed,
}

impl EventEmitter<MultiSelectEvent> for MultiSelect {}

/// A selectable option set with search, chips, and listbox semantics.
pub struct MultiSelect {
    ident: Ident,
    query: Entity<TextInput>,
    options: Vec<SelectOption>,
    selected: Vec<SharedString>,
    name: SharedString,
    placeholder: Option<SharedString>,
    size: ControlSize,
    disabled: bool,
    invalid: bool,
    open: bool,
    clearable: bool,
    active: Option<SharedString>,
    scroll: ScrollHandle,
    trigger_bounds: Rc<Cell<Bounds<Pixels>>>,
    reveal_active: bool,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for MultiSelect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MultiSelect")
            .field("ident", &self.ident)
            .field("options", &self.options.len())
            .field("selected", &self.selected)
            .field("open", &self.open)
            .finish()
    }
}

impl MultiSelect {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let query = cx.new(|cx| TextInput::new(ident.child("query"), window, cx).bare(true));
        let subscription = cx.subscribe(&query, |select, _query, event, cx| match event {
            TextInputEvent::Change(query) => select.on_query(query.clone(), cx),
            TextInputEvent::Submit => select.toggle_active(cx),
            TextInputEvent::Cancel => select.close(cx),
            _ => {}
        });
        Self {
            ident,
            query,
            options: Vec::new(),
            selected: Vec::new(),
            name: SharedString::default(),
            placeholder: None,
            size: ControlSize::Md,
            disabled: false,
            invalid: false,
            open: false,
            clearable: false,
            active: None,
            scroll: ScrollHandle::new(),
            trigger_bounds: Rc::default(),
            reveal_active: false,
            _subscriptions: vec![subscription],
        }
    }

    pub fn options(mut self, options: impl IntoIterator<Item = SelectOption>) -> Self {
        self.options = options.into_iter().collect();
        self
    }

    pub fn selected(mut self, selected: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.selected = selected.into_iter().map(Into::into).collect();
        self
    }

    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = name.into();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn clearable(mut self, clearable: bool) -> Self {
        self.clearable = clearable;
        self
    }

    pub fn set_selected(
        &mut self,
        selected: impl IntoIterator<Item = impl Into<SharedString>>,
        cx: &mut Context<Self>,
    ) {
        self.selected = selected.into_iter().map(Into::into).collect();
        cx.notify();
    }

    pub fn set_options(&mut self, options: Vec<SelectOption>, cx: &mut Context<Self>) {
        self.options = options;
        self.active = None;
        self.reveal_active = true;
        cx.notify();
    }

    pub fn selected_ids(&self) -> &[SharedString] {
        &self.selected
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.open {
            return;
        }
        self.open = true;
        self.active = self.first_match(cx);
        self.reveal_active = true;
        self.query.read(cx).focus_handle(cx).focus(window, cx);
        cx.emit(MultiSelectEvent::Opened);
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.active = None;
        self.query
            .update(cx, |query, cx| query.set_text_quietly("", cx));
        cx.emit(MultiSelectEvent::Closed);
        cx.notify();
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.close(cx);
        } else {
            self.open(window, cx);
        }
    }

    fn on_query(&mut self, query: SharedString, cx: &mut Context<Self>) {
        self.open = true;
        self.active = self.first_match(cx);
        self.reveal_active = true;
        cx.emit(MultiSelectEvent::QueryChanged(query));
        cx.notify();
    }

    fn matches(&self, cx: &App) -> Vec<usize> {
        let labels: Vec<&str> = self
            .options
            .iter()
            .map(|option| option.label.as_ref())
            .collect();
        popover::filter_indices_for(cx, self.query.read(cx).value().as_ref(), &labels)
    }

    fn first_match(&self, cx: &App) -> Option<SharedString> {
        self.matches(cx)
            .into_iter()
            .map(|index| &self.options[index])
            .find(|option| !option.disabled)
            .map(|option| option.id.clone())
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let matches: Vec<usize> = self
            .matches(cx)
            .into_iter()
            .filter(|index| !self.options[*index].disabled)
            .collect();
        if matches.is_empty() {
            return;
        }
        let current = self.active.as_ref().and_then(|id| {
            matches
                .iter()
                .position(|index| self.options[*index].id == *id)
        });
        if let Some(next) = popover::step(current, matches.len(), delta) {
            self.active = Some(self.options[matches[next]].id.clone());
            self.reveal_active = true;
            cx.notify();
        }
    }

    fn toggle_active(&mut self, cx: &mut Context<Self>) {
        if let Some(active) = self.active.clone() {
            self.toggle_id(active, cx);
        }
    }

    fn toggle_id(&mut self, id: SharedString, cx: &mut Context<Self>) {
        if self.disabled
            || self
                .options
                .iter()
                .find(|option| option.id == id)
                .is_some_and(|option| option.disabled)
        {
            return;
        }
        cx.emit(MultiSelectEvent::Toggled(id));
    }

    fn remove_id(&mut self, id: SharedString, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        cx.emit(MultiSelectEvent::Removed(id));
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        if !self.disabled && self.clearable && !self.selected.is_empty() {
            cx.emit(MultiSelectEvent::Cleared);
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        match popover::classify_key(
            event.keystroke.key.as_str(),
            event.keystroke.modifiers.platform,
            event.keystroke.modifiers.control,
        ) {
            MenuKey::Down => {
                self.open(window, cx);
                self.step(1, cx);
                cx.stop_propagation();
            }
            MenuKey::Up => {
                self.open(window, cx);
                self.step(-1, cx);
                cx.stop_propagation();
            }
            MenuKey::Escape => {
                self.close(cx);
                cx.stop_propagation();
            }
            MenuKey::Enter if self.open => {
                self.toggle_active(cx);
                cx.stop_propagation();
            }
            MenuKey::Backspace if self.open && self.query.read(cx).is_empty() => {
                if let Some(id) = self.selected.last().cloned() {
                    self.remove_id(id, cx);
                }
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    fn option(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let option = &self.options[index];
        let selected = self.selected.iter().any(|id| id == &option.id);
        let active = self.active.as_ref() == Some(&option.id);
        let ident = self.ident.child("option").child(option.id.as_ref());
        let option_id = option.id.clone();
        let disabled = option.disabled;
        let mut row = div()
            .id(ident.element_id())
            .w_full()
            .row()
            .items_center()
            .gap(px(theme.space(Space::Sm)))
            .px(px(theme.space(Space::Sm)))
            .py(px(theme.space(Space::Xs)))
            .radius(&theme, Radius::Control)
            .when(active, |element| element.bg(theme.colors.hover))
            .when(!disabled, |element| element.cursor_pointer().pressable(cx))
            .child(
                icon(if selected {
                    Icon::CheckboxChecked
                } else {
                    Icon::CheckboxEmpty
                })
                .size(px(theme.control.sm.icon_size))
                .text_color(if disabled {
                    theme.colors.text_disabled
                } else if selected {
                    theme.colors.accent
                } else {
                    theme.colors.text_muted
                }),
            )
            .child(
                text(&theme, TypeScale::Body, option.label.clone()).text_color(if disabled {
                    theme.colors.text_disabled
                } else {
                    theme.colors.text
                }),
            )
            .when_some(option.description.clone(), |element, description| {
                element.child(
                    text(&theme, TypeScale::Caption, description)
                        .text_color(theme.colors.text_muted),
                )
            })
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Option)
                    .parent(self.ident.child("list").semantic_id())
                    .text(option.label.clone())
                    .checked(selected)
                    .selected(selected)
                    .disabled(disabled),
            );
        if !disabled {
            row = row.on_mouse_down(MouseButton::Left, {
                let id = option_id;
                cx.listener(move |select, _, _, cx| select.toggle_id(id.clone(), cx))
            });
        }
        row.into_any_element()
    }
}

impl Disableable for MultiSelect {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for MultiSelect {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for MultiSelect {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.query.read(cx).focus_handle(cx)
    }
}

impl Render for MultiSelect {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let focused = self.query.read(cx).focus_handle(cx).is_focused(window);
        let placeholder = self
            .placeholder
            .clone()
            .unwrap_or_else(|| cx.strings().text(StringKey::SelectPlaceholder));
        self.query.update(cx, |query, cx| {
            query.set_placeholder(placeholder.clone(), cx);
            query.set_name(self.name.clone(), cx);
            query.set_disabled(self.disabled, cx);
        });

        let visible_indices = self.matches(cx);
        let geometry = self.open.then(|| {
            popover::menu_geometry(
                window,
                self.trigger_bounds.get(),
                &theme,
                theme.measures.menu_max_height,
                theme.measures.menu_min_width,
            )
        });
        if self.reveal_active {
            if let Some(active) = &self.active
                && let Some(index) = visible_indices
                    .iter()
                    .position(|index| &self.options[*index].id == active)
            {
                self.scroll.scroll_to_item(index);
            }
            self.reveal_active = false;
        }

        let chips = self.selected.iter().filter_map(|id| {
            let option = self.options.iter().find(|option| &option.id == id)?;
            let id = option.id.clone();
            let select = cx.entity();
            Some(
                Tag::new(
                    self.ident.child("tag").child(id.as_ref()),
                    option.label.clone(),
                )
                .on_remove(move |_, app| {
                    let id = id.clone();
                    select.update(app, |select, cx| select.remove_id(id, cx));
                }),
            )
        });
        let trigger_id = self.ident.clone();
        let trigger = field_shell(
            &theme,
            self.size,
            FieldState::default()
                .focused(focused)
                .invalid(self.invalid)
                .disabled(self.disabled),
        )
        .id(self.ident.child("field").element_id())
        .min_h(px(metrics.height))
        .flex_wrap()
        .gap(px(theme.space(Space::Xs)))
        .when(!self.disabled, |element| {
            element.on_mouse_down(
                MouseButton::Left,
                cx.listener(|select, _, window, cx| select.toggle(window, cx)),
            )
        })
        .children(chips)
        .child(
            div()
                .flex_1()
                .min_w(px(theme.space(Space::Xl)))
                .child(self.query.clone()),
        )
        .when(
            self.clearable && !self.selected.is_empty() && !self.disabled,
            |element| {
                element.child(
                    div()
                        .id(self.ident.child("clear").element_id())
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|select, _, _, cx| select.clear(cx)),
                        )
                        .child(
                            icon(Icon::Close)
                                .size(px(metrics.icon_size))
                                .text_color(theme.colors.text_muted),
                        )
                        .semantic_in(
                            cx,
                            NodeSpec::new(self.ident.child("clear").semantic_id(), Role::Button)
                                .parent(self.ident.semantic_id())
                                .text(cx.strings().text(StringKey::SelectClear)),
                        ),
                )
            },
        );
        let measured = self.trigger_bounds.clone();
        let trigger = div()
            .w_full()
            .on_children_prepainted(move |bounds, window, _| {
                if let Some(bounds) = bounds.first() {
                    measure::record(&measured, *bounds, window);
                }
            })
            .child(trigger)
            .into_any_element();

        let menu = geometry.map(|geometry| {
            let list = div()
                .id(self.ident.child("list").element_id())
                .max_h(px(geometry.max_height))
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .p(px(theme.space(Space::Xs)))
                .flex()
                .flex_col()
                .gap(px(theme.space(Space::Xxs)))
                .children(
                    visible_indices
                        .iter()
                        .copied()
                        .map(|index| self.option(index, cx)),
                )
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("list").semantic_id(), Role::List),
                );
            popover::menu_overlay(
                &self.ident.child("menu.anchor"),
                &theme,
                geometry.placement,
                geometry.hang,
                popover::card_flush(&theme)
                    .w(px(geometry.width))
                    .max_h(px(geometry.max_height))
                    .child(popover::menu_body(
                        &self.ident.child("list.fade"),
                        &self.scroll,
                        list,
                    ))
                    .into_any_element(),
            )
        });

        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Combobox)
            .text(self.name.clone())
            .placeholder(placeholder)
            .expanded(self.open)
            .value(cx.numbers().count(self.selected.len()));
        if !self.disabled {
            spec = spec.focus(&self.query.read(cx).focus_handle(cx));
        }

        div()
            .id(trigger_id.element_id())
            .w_full()
            .capture_key_down(cx.listener(Self::on_key_down))
            .child(popover::anchored_slot(
                geometry.map_or(crate::overlay::Placement::Below, |geometry| {
                    geometry.placement
                }),
                geometry.map_or(crate::overlay::Hang::Start, |geometry| geometry.hang),
                trigger,
                menu,
            ))
            .semantic_in(cx, spec)
    }
}
