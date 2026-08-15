//! A hierarchical choice whose data and accepted value remain caller-owned.

use gpui::{
    AnyElement, App, Context, ElementId, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, TypeScale};

use crate::controls::button::{Button, ButtonVariant};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::loading::PulseLoader;
use crate::foundation::{
    ActiveDirection, DirectionalExt, Disableable, Ident, LayoutDirection, Pressable, Sizable,
    StyledExt, text as foundation_text,
};
use crate::overlay::{
    Placement,
    popover::{self, MenuKey},
};
use crate::state::Loadable;
use crate::strings::{ActiveStrings, StringKey};

/// One caller-owned node in a cascader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CascaderOption {
    pub id: SharedString,
    pub label: SharedString,
    pub disabled: bool,
    /// `None` is a leaf; `Some` is a branch, including before it is loaded.
    pub children: Option<Loadable<Vec<CascaderOption>, SharedString>>,
}

impl CascaderOption {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
            children: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn idle_children(mut self) -> Self {
        self.children = Some(Loadable::Idle);
        self
    }
    pub fn loading_children(mut self) -> Self {
        self.children = Some(Loadable::Loading);
        self
    }
    pub fn empty_children(mut self) -> Self {
        self.children = Some(Loadable::Empty);
        self
    }
    pub fn unavailable_children(mut self, reason: impl Into<SharedString>) -> Self {
        self.children = Some(Loadable::Unavailable(reason.into().to_string()));
        self
    }
    pub fn error_children(mut self, reason: impl Into<SharedString>) -> Self {
        self.children = Some(Loadable::Error(reason.into()));
        self
    }
    pub fn children(mut self, children: impl IntoIterator<Item = CascaderOption>) -> Self {
        self.children = Some(Loadable::Ready(children.into_iter().collect()));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CascaderEvent {
    Selected(SharedString),
    Expanded(SharedString),
    Retry(SharedString),
    Opened,
    Closed,
}

impl EventEmitter<CascaderEvent> for Cascader {}

/// A menu of progressively revealed caller-owned option columns.
pub struct Cascader {
    ident: Ident,
    focus_handle: FocusHandle,
    options: Vec<CascaderOption>,
    selected: Option<SharedString>,
    name: SharedString,
    placeholder: Option<SharedString>,
    size: ControlSize,
    disabled: bool,
    open: bool,
    open_path: Vec<SharedString>,
    active: Option<SharedString>,
}

impl std::fmt::Debug for Cascader {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Cascader")
            .field("ident", &self.ident)
            .field("options", &self.options.len())
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .field("open", &self.open)
            .field("open_path", &self.open_path)
            .finish()
    }
}

impl Cascader {
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
            open: false,
            open_path: Vec::new(),
            active: None,
        }
    }

    pub fn options(mut self, options: impl IntoIterator<Item = CascaderOption>) -> Self {
        self.options = options.into_iter().collect();
        self
    }
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
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
    pub fn set_options(&mut self, options: Vec<CascaderOption>, cx: &mut Context<Self>) {
        let open_path = Self::valid_open_path(&options, &self.open_path);
        let active = self.active.take();
        self.options = options;
        self.open_path = open_path;
        self.active = if self.open {
            let current = self.current_options();
            active
                .filter(|id| {
                    current
                        .iter()
                        .any(|option| &option.id == id && !option.disabled)
                })
                .or_else(|| Self::first_enabled(current, false))
        } else {
            None
        };
        cx.notify();
    }
    pub fn set_selected(&mut self, selected: Option<SharedString>, cx: &mut Context<Self>) {
        self.selected = selected;
        cx.notify();
    }
    pub fn set_name(&mut self, name: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.name = name.into();
        cx.notify();
    }
    pub fn set_placeholder(&mut self, placeholder: Option<SharedString>, cx: &mut Context<Self>) {
        self.placeholder = placeholder;
        cx.notify();
    }
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.close(cx);
        }
        cx.notify();
    }
    pub fn selected_id(&self) -> Option<&SharedString> {
        self.selected.as_ref()
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn open_path(&self) -> &[SharedString] {
        &self.open_path
    }

    fn find<'a>(options: &'a [CascaderOption], id: &SharedString) -> Option<&'a CascaderOption> {
        for option in options {
            if &option.id == id {
                return Some(option);
            }
            if let Some(Loadable::Ready(children)) = &option.children
                && let Some(found) = Self::find(children, id)
            {
                return Some(found);
            }
        }
        None
    }

    fn path_to(
        options: &[CascaderOption],
        id: &SharedString,
        path: &mut Vec<SharedString>,
    ) -> bool {
        for option in options {
            if &option.id == id {
                return true;
            }
            if let Some(Loadable::Ready(children)) = &option.children {
                path.push(option.id.clone());
                if Self::path_to(children, id, path) {
                    return true;
                }
                path.pop();
            }
        }
        false
    }

    fn valid_open_path(options: &[CascaderOption], path: &[SharedString]) -> Vec<SharedString> {
        let mut current = options;
        let mut valid = Vec::new();
        for id in path {
            let Some(option) = current
                .iter()
                .find(|option| &option.id == id && !option.disabled)
            else {
                break;
            };
            let Some(children) = &option.children else {
                break;
            };
            valid.push(id.clone());
            match children {
                Loadable::Ready(children) => current = children,
                _ => break,
            }
        }
        valid
    }

    fn current_options(&self) -> &[CascaderOption] {
        let mut options = self.options.as_slice();
        for id in &self.open_path {
            let Some(option) = options.iter().find(|option| &option.id == id) else {
                return &[];
            };
            let Some(Loadable::Ready(children)) = &option.children else {
                return &[];
            };
            options = children;
        }
        options
    }

    fn first_enabled(options: &[CascaderOption], reverse: bool) -> Option<SharedString> {
        if reverse {
            options
                .iter()
                .rev()
                .find(|o| !o.disabled)
                .map(|o| o.id.clone())
        } else {
            options.iter().find(|o| !o.disabled).map(|o| o.id.clone())
        }
    }

    /// Opens the transient option surface without changing any caller-owned value.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.open {
            return;
        }
        self.open = true;
        self.active = Self::first_enabled(&self.options, false);
        window.focus(&self.focus_handle, cx);
        cx.emit(CascaderEvent::Opened);
        cx.notify();
    }

    /// Closes the transient option surface and clears its open path.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.open_path.clear();
        self.active = None;
        cx.emit(CascaderEvent::Closed);
        cx.notify();
    }

    fn activate(&mut self, id: SharedString, cx: &mut Context<Self>) {
        let Some(option) = Self::find(&self.options, &id) else {
            return;
        };
        if option.disabled {
            return;
        }
        let branch = option.children.is_some();
        let child_active = match &option.children {
            Some(Loadable::Ready(children)) => Self::first_enabled(children, false),
            _ => None,
        };
        if branch {
            let mut path = Vec::new();
            Self::path_to(&self.options, &id, &mut path);
            path.push(id.clone());
            self.open_path = path;
            self.active = child_active;
            cx.emit(CascaderEvent::Expanded(id));
            cx.notify();
        } else {
            self.open = false;
            self.open_path.clear();
            self.active = None;
            cx.emit(CascaderEvent::Selected(id));
            cx.emit(CascaderEvent::Closed);
            cx.notify();
        }
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.open_path.pop() {
            self.active = Some(parent);
            cx.notify();
        }
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let options = self.current_options();
        let start = self
            .active
            .as_ref()
            .and_then(|id| options.iter().position(|o| &o.id == id));
        let mut next = popover::step(start, options.len(), delta);
        for _ in 0..options.len() {
            let Some(index) = next else { return };
            if !options[index].disabled {
                self.active = Some(options[index].id.clone());
                cx.notify();
                return;
            }
            next = popover::step(next, options.len(), delta);
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let raw = event.keystroke.key.as_str();
        let key = popover::classify_key(
            raw,
            event.keystroke.modifiers.platform,
            event.keystroke.modifiers.control,
        );
        if !self.open && matches!(key, MenuKey::Enter | MenuKey::Up | MenuKey::Down) {
            self.open(window, cx);
            cx.stop_propagation();
            return;
        }
        if !self.open {
            return;
        }
        let direction = cx.layout_direction();
        let toward_children = matches!(
            (direction, key),
            (LayoutDirection::LeftToRight, MenuKey::Right)
                | (LayoutDirection::RightToLeft, MenuKey::Left)
        );
        let toward_parent = matches!(
            (direction, key),
            (LayoutDirection::LeftToRight, MenuKey::Left)
                | (LayoutDirection::RightToLeft, MenuKey::Right)
        );
        match key {
            MenuKey::Down => self.step(1, cx),
            MenuKey::Up => self.step(-1, cx),
            MenuKey::Enter => {
                let Some(active) = self.active.clone() else {
                    return;
                };
                self.activate(active, cx)
            }
            MenuKey::Escape => self.close(cx),
            _ if raw == "home" => {
                self.active = Self::first_enabled(self.current_options(), false);
                cx.notify();
            }
            _ if raw == "end" => {
                self.active = Self::first_enabled(self.current_options(), true);
                cx.notify();
            }
            _ if toward_children => {
                let Some(active) = self.active.clone() else {
                    return;
                };
                self.activate(active, cx)
            }
            _ if toward_parent => self.back(cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn row(&self, option: &CascaderOption, column: &Ident, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let ident = self.ident.child(option.id.as_ref());
        let hover_group = ident.child("hover").semantic_id();
        let active = self.active.as_ref() == Some(&option.id);
        let expanded = self.open_path.contains(&option.id);
        let id = option.id.clone();
        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Option)
            .parent(column.semantic_id())
            .text(option.label.clone())
            .hovered(active)
            .disabled(option.disabled);
        if option.children.is_some() {
            spec = spec.expanded(expanded);
        }
        popover::menu_row(&theme, false, active)
            .id(ident.element_id())
            .group(hover_group.clone())
            .row_reading(direction)
            .when(!option.disabled, |row| row.cursor_pointer().pressable(cx))
            .child(popover::menu_label_state(
                &theme,
                option.label.clone(),
                false,
                active,
                option.disabled,
                hover_group,
            ))
            .when(option.children.is_some(), |row| {
                row.child(div().flex_1()).child(
                    icon(if direction.is_rtl() {
                        Icon::AltArrowLeft
                    } else {
                        Icon::AltArrowRight
                    })
                    .size(px(14.0))
                    .text_color(theme.colors.text_muted),
                )
            })
            .when(!option.disabled, |row| {
                row.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| this.activate(id.clone(), cx)),
                )
            })
            .semantic_in(cx, spec)
            .into_any_element()
    }

    fn state_column(&self, parent: &CascaderOption, cx: &mut Context<Self>) -> AnyElement {
        let ident = self.ident.child(parent.id.as_ref()).child("state");
        let strings = cx.strings();
        let state = parent.children.as_ref().expect("branch");
        let content: AnyElement = match state {
            Loadable::Ready(children) if children.is_empty() => {
                EmptyState::new(ident, strings.text(StringKey::CascaderEmpty))
                    .kind(EmptyKind::Empty)
                    .into_any_element()
            }
            Loadable::Ready(children) => self.option_column(children, &ident, cx),
            Loadable::Loading => div()
                .p(px(24.0))
                .child(PulseLoader::new(ident.clone()).label(strings.text(StringKey::Loading)))
                .into_any_element(),
            Loadable::Idle => EmptyState::new(ident, strings.text(StringKey::CascaderUnstarted))
                .kind(EmptyKind::Unstarted)
                .into_any_element(),
            Loadable::Empty => EmptyState::new(ident, strings.text(StringKey::CascaderEmpty))
                .kind(EmptyKind::Empty)
                .into_any_element(),
            Loadable::Unavailable(reason) => {
                let weak = cx.entity().downgrade();
                let parent_id = parent.id.clone();
                let action_id = ident.child("retry");
                EmptyState::new(ident.clone(), strings.text(StringKey::CascaderUnavailable))
                    .kind(EmptyKind::Unavailable)
                    .detail(reason.as_str())
                    .action(
                        Button::new(action_id)
                            .semantic_parent(ident.semantic_id())
                            .variant(ButtonVariant::Secondary)
                            .label(strings.text(StringKey::TryAgain))
                            .on_click(move |_, cx| {
                                let _ = weak.update(cx, |_, cx| {
                                    cx.emit(CascaderEvent::Retry(parent_id.clone()))
                                });
                            }),
                    )
                    .into_any_element()
            }
            Loadable::Error(reason) => {
                let weak = cx.entity().downgrade();
                let parent_id = parent.id.clone();
                let action_id = ident.child("retry");
                EmptyState::new(ident.clone(), strings.text(StringKey::CascaderError))
                    .kind(EmptyKind::Failed)
                    .detail(reason.clone())
                    .action(
                        Button::new(action_id)
                            .semantic_parent(ident.semantic_id())
                            .variant(ButtonVariant::Secondary)
                            .label(strings.text(StringKey::TryAgain))
                            .on_click(move |_, cx| {
                                let _ = weak.update(cx, |_, cx| {
                                    cx.emit(CascaderEvent::Retry(parent_id.clone()))
                                });
                            }),
                    )
                    .into_any_element()
            }
        };
        content
    }

    fn option_column(
        &self,
        options: &[CascaderOption],
        ident: &Ident,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(ident.element_id())
            .min_w(px(180.0))
            .max_h(px(320.0))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .children(options.iter().map(|option| self.row(option, ident, cx)))
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Menu)
                    .parent(self.ident.child("menu").semantic_id()),
            )
            .into_any_element()
    }

    fn menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let root = self.ident.child("menu.root");
        let mut columns = vec![self.option_column(&self.options, &root, cx)];
        let mut options = self.options.as_slice();
        for id in &self.open_path {
            let Some(parent) = options.iter().find(|option| &option.id == id) else {
                break;
            };
            columns.push(self.state_column(parent, cx));
            match &parent.children {
                Some(Loadable::Ready(children)) => options = children,
                _ => break,
            }
        }
        let card = popover::card_flush(&theme)
            .p(px(theme.space(Space::Xs)))
            .flex()
            .row_reading(cx.layout_direction())
            .children(columns)
            .id(self.ident.child("menu").element_id())
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.child("menu").semantic_id(), Role::Menu)
                    .parent(self.ident.semantic_id()),
            );
        popover::anchored_below(
            ElementId::from(self.ident.child("menu.anchor").semantic_id()),
            &theme,
            card.into_any_element(),
        )
    }
}

impl Disableable for Cascader {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}
impl Sizable for Cascader {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}
impl Focusable for Cascader {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Cascader {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let focused = self.focus_handle.is_focused(window);
        let placeholder = self
            .placeholder
            .clone()
            .unwrap_or_else(|| cx.strings().text(StringKey::CascaderPlaceholder));
        let selected = self
            .selected
            .as_ref()
            .and_then(|id| Self::find(&self.options, id));
        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Combobox)
            .disabled(self.disabled)
            .expanded(self.open)
            .text(self.name.clone())
            .placeholder(placeholder.clone());
        if !self.disabled {
            spec = spec.focus(&self.focus_handle);
        }
        if let Some(option) = selected {
            spec = spec.value(option.label.clone());
        }
        let label = selected.map(|o| o.label.clone()).unwrap_or(placeholder);
        let menu = self.open.then(|| self.menu(cx));
        let trigger = div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .h(px(metrics.height))
            .px(px(metrics.padding_x))
            .radius(&theme, Radius::Control)
            .well(&theme)
            .when(focused, |element| element.shadow(theme.focus_ring()))
            .when(self.disabled, |el| el.opacity(theme.opacity.disabled))
            .when(!self.disabled, |el| {
                el.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        if this.open {
                            this.close(cx)
                        } else {
                            this.open(window, cx)
                        }
                    }),
                )
            })
            .child(
                foundation_text(&theme, TypeScale::Label, label)
                    .text_size(px(metrics.font_size))
                    .text_color(if self.disabled || selected.is_none() {
                        theme.colors.text_faint
                    } else {
                        theme.colors.text
                    }),
            )
            .child(
                icon(Icon::AltArrowDown)
                    .size(px(metrics.icon_size * 0.9))
                    .text_color(theme.colors.text_muted),
            )
            .into_any_element();
        popover::anchored_slot(Placement::Below, trigger, menu)
            .id(self.ident.element_id())
            .when(!self.disabled, |el| {
                el.track_focus(&self.focus_handle)
                    .on_key_down(cx.listener(Self::on_key_down))
            })
            .w_full()
            .semantic_in(cx, spec)
    }
}
