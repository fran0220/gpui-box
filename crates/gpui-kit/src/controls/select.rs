//! A control for choosing one of a known set of options.
//!
//! The open menu is transient view state, so `Select` is a view rather than a
//! builder. The chosen value is not: the select reports what was picked and
//! renders whatever the owner decides is current, so a host that rejects a
//! choice keeps showing the one that still holds.

use gpui::{
    AnyElement, App, Context, ElementId, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, TypeScale};

use crate::foundation::{
    Disableable, Ident, Pressable, Sizable, StyledExt, text as foundation_text,
};
use crate::motion;
use crate::overlay::popover::{self, MenuKey};
use crate::strings::{ActiveStrings, StringKey};

/// One choice, identified by business identity rather than by position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption {
    pub id: SharedString,
    pub label: SharedString,
    pub description: Option<SharedString>,
    pub disabled: bool,
}

impl SelectOption {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            disabled: false,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// What a [`Select`] reports. The owner decides what any of it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectEvent {
    /// The typist picked this option. The owner decides whether it holds.
    Selected(SharedString),
    Opened,
    Closed,
}

impl EventEmitter<SelectEvent> for Select {}

/// A closed list of options with one answer.
///
/// The select owns only whether its menu is open. It reports the option that
/// was picked and draws whatever the caller says is current, so a refused
/// choice is visible as the checkmark not moving.
pub struct Select {
    ident: Ident,
    focus_handle: FocusHandle,
    options: Vec<SelectOption>,
    selected: Option<SharedString>,
    name: SharedString,
    placeholder: Option<SharedString>,
    size: ControlSize,
    disabled: bool,
    invalid: bool,
    open: bool,
    /// Which row the keyboard is on, which is not a choice until it is taken.
    active: Option<usize>,
}

impl std::fmt::Debug for Select {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Select")
            .field("ident", &self.ident)
            .field("options", &self.options.len())
            .field("selected", &self.selected)
            .field("open", &self.open)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl Select {
    pub fn new(ident: impl Into<Ident>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            options: Vec::new(),
            selected: None,
            name: SharedString::default(),
            placeholder: None,
            size: ControlSize::Md,
            disabled: false,
            invalid: false,
            open: false,
            active: None,
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

    /// Names the control independently of its current answer or placeholder.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = name.into();
        self
    }

    pub fn set_name(&mut self, name: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.name = name.into();
        cx.notify();
    }

    /// The placeholder the host gave, or the built-in default.
    fn resolved_placeholder(&self, cx: &App) -> SharedString {
        self.placeholder
            .clone()
            .unwrap_or_else(|| cx.strings().text(StringKey::SelectPlaceholder))
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Replaces the options from the host side, keeping a selection that is
    /// still offered and dropping one that is not.
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
        cx.notify();
    }

    pub fn set_selected(&mut self, id: Option<SharedString>, cx: &mut Context<Self>) {
        self.selected = id;
        cx.notify();
    }

    pub fn selected_id(&self) -> Option<&SharedString> {
        self.selected.as_ref()
    }

    pub fn selected_option(&self) -> Option<&SelectOption> {
        let id = self.selected.as_ref()?;
        self.options.iter().find(|option| &option.id == id)
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.open = false;
        }
        cx.notify();
    }

    fn open_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.open {
            return;
        }
        self.open = true;
        // The keyboard starts on what is already chosen, so the first arrow
        // key moves from the current answer rather than from the top.
        self.active = self
            .selected
            .as_ref()
            .and_then(|id| self.options.iter().position(|option| &option.id == id))
            .or_else(|| self.first_selectable(0, 1));
        window.focus(&self.focus_handle, cx);
        cx.emit(SelectEvent::Opened);
        cx.notify();
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.active = None;
        cx.emit(SelectEvent::Closed);
        cx.notify();
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.close_menu(cx);
        } else {
            self.open_menu(window, cx);
        }
    }

    /// The next option that can actually be chosen, skipping refusals.
    fn first_selectable(&self, from: usize, delta: isize) -> Option<usize> {
        let count = self.options.len();
        if count == 0 {
            return None;
        }
        let mut index = from.min(count - 1);
        for _ in 0..count {
            if !self.options[index].disabled {
                return Some(index);
            }
            index = ((index as isize + delta).rem_euclid(count as isize)) as usize;
        }
        None
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(next) = popover::step(self.active, self.options.len(), delta) else {
            return;
        };
        self.active = self.first_selectable(next, delta.signum());
        cx.notify();
    }

    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(option) = self.options.get(index) else {
            return;
        };
        if option.disabled {
            return;
        }
        let id = option.id.clone();
        self.open = false;
        self.active = None;
        cx.emit(SelectEvent::Selected(id));
        cx.emit(SelectEvent::Closed);
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let key = popover::classify_key(
            event.keystroke.key.as_str(),
            event.keystroke.modifiers.platform,
            event.keystroke.modifiers.control,
        );
        match (self.open, key) {
            (false, MenuKey::Down | MenuKey::Up | MenuKey::Enter) => {
                self.open_menu(window, cx);
                cx.stop_propagation();
            }
            (true, MenuKey::Down) => {
                self.step(1, cx);
                cx.stop_propagation();
            }
            (true, MenuKey::Up) => {
                self.step(-1, cx);
                cx.stop_propagation();
            }
            (true, MenuKey::Enter) => {
                if let Some(active) = self.active {
                    self.choose(active, cx);
                }
                cx.stop_propagation();
            }
            (true, MenuKey::Escape) => {
                self.close_menu(cx);
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    fn menu(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let rows = self
            .options
            .iter()
            .enumerate()
            .map(|(index, option)| self.row(index, option, self.options.len(), cx))
            .collect::<Vec<_>>();

        let list = popover::card_flush(&theme)
            .p(px(theme.space(Space::Xs)))
            .min_w(px(180.0))
            .flex()
            .flex_col()
            .max_h(px(320.0))
            .id(self.ident.child("menu").element_id())
            .overflow_y_scroll()
            .children(rows)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.child("menu").semantic_id(), Role::Menu),
            )
            .into_any_element();

        let _ = window;
        popover::anchored_below(
            ElementId::from(self.ident.child("menu.anchor").semantic_id()),
            &theme,
            list,
        )
    }

    fn row(
        &self,
        index: usize,
        option: &SelectOption,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let selected = self.selected.as_ref() == Some(&option.id);
        let active = self.active == Some(index);
        let ident = self.ident.child(option.id.as_ref());
        let hover_group = ident.child("hover").semantic_id();

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Option)
            .parent(self.ident.child("menu").semantic_id())
            .checked(selected)
            .disabled(option.disabled)
            .text(option.label.clone());
        if active {
            spec = spec.hovered(true);
        }

        let row = popover::menu_row(&theme, selected, active)
            .id(ident.element_id())
            .group(hover_group.clone())
            .when(!option.disabled, |element| {
                element.cursor_pointer().pressable(cx)
            })
            .when(option.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(popover::menu_label(
                        &theme,
                        option.label.clone(),
                        selected,
                        active,
                        hover_group,
                    ))
                    .when_some(option.description.clone(), |element, description| {
                        element.child(
                            foundation_text(&theme, TypeScale::Caption, description)
                                .text_tone(&theme, gpui_kit_theme::TextTone::Muted),
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
                    cx.listener(move |select, _, _, cx| {
                        select.choose(index, cx);
                    }),
                )
            })
            .semantic_in(cx, spec);

        motion::row_in(ident.child("in").element_id(), &theme, index, count, row).into_any_element()
    }
}

impl Disableable for Select {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Select {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for Select {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Select {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let focused = self.focus_handle.is_focused(window);
        let label = self
            .selected_option()
            .map(|option| option.label.clone())
            .unwrap_or_else(|| self.resolved_placeholder(cx));
        let has_choice = self.selected_option().is_some();

        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Combobox)
            .disabled(self.disabled)
            .invalid(self.invalid)
            .expanded(self.open)
            .text(self.name.clone())
            .placeholder(self.resolved_placeholder(cx));
        if !self.disabled {
            spec = spec.focus(&self.focus_handle);
        }
        if let Some(option) = self.selected_option() {
            spec = spec.value(option.label.clone());
        }

        let menu = self.open.then(|| self.menu(window, cx));

        div()
            .id(self.ident.element_id())
            .when(!self.disabled, |element| {
                element
                    .track_focus(&self.focus_handle)
                    .on_key_down(cx.listener(Self::on_key_down))
            })
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(theme.space(Space::Sm)))
            .h(px(metrics.height))
            .px(px(metrics.padding_x))
            .radius(&theme, Radius::Control)
            .well(&theme)
            .when(self.invalid, |element| {
                element.border_color(theme.colors.danger)
            })
            .when(focused, |element| element.shadow(theme.focus_ring()))
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .when(!self.disabled, |element| {
                element.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|select, _, window, cx| select.toggle(window, cx)),
                )
            })
            .child(
                foundation_text(&theme, TypeScale::Label, label)
                    .text_size(px(metrics.font_size))
                    .text_color(if self.disabled || !has_choice {
                        theme.colors.text_faint
                    } else {
                        theme.colors.text
                    }),
            )
            .child(
                // One glyph in both states: the menu itself shows whether the
                // control is open, and a flipped arrow would say it twice.
                icon(Icon::AltArrowDown)
                    .size(px(metrics.icon_size * 0.9))
                    .text_color(theme.colors.text_muted),
            )
            .children(menu)
            .semantic_in(cx, spec)
    }
}
