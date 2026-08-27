//! Lists of commands: one opened from a trigger, one opened at the pointer.
//!
//! A menu presents what can be done and reports what was chosen. It never
//! does any of it, which is why a checkable item reports the intent to change
//! and keeps drawing the state the host still holds.
//!
//! [`Menu`] and [`ContextMenu`] differ only in what opens them, so the item
//! model, the cursor, and the panels are shared: [`MenuState`] holds where the
//! keyboard is and which submenus are open, and both views render it through
//! the same panels.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Pixels, Point, Render,
    SharedString, StatefulInteractiveElement, Styled, Transformation, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, Theme, TypeScale};

use crate::controls::button::{Button, ButtonJoin, ButtonVariant};
use crate::display::icon::flips;
use crate::foundation::direction::{ActiveDirection, LayoutDirection};
use crate::foundation::{Ident, Pressable, Selectable, Sizable, StyledExt, text};
use crate::motion;
use crate::overlay::focus::FocusTrap;
use crate::overlay::kbd::Kbd;
use crate::overlay::layer::{Hang, Overlay, OverlaySurface, Placement, surface};
use crate::overlay::popover::{self, MenuKey};

/// The leading slot every row reserves, so labels line up whether or not the
/// row carries a check or an icon.
const GLYPH_SLOT: f32 = 16.0;

/// What one entry in a menu is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum MenuItemKind {
    Command,
    /// Carries a checked state the host owns.
    Check(bool),
    Separator,
    /// A caption naming the group that follows.
    Section,
    Submenu(Vec<MenuItem>),
}

/// One entry in a menu, identified by business identity rather than position.
///
/// An id is unique within its menu, including across submenus, because it is
/// what the menu reports and what a test addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    id: SharedString,
    label: SharedString,
    kind: MenuItemKind,
    shortcut: Option<SharedString>,
    icon: Option<Icon>,
    disabled: bool,
    /// A command that destroys something. Painted in danger so a delete row
    /// is not just another line of body text.
    destructive: bool,
}

impl MenuItem {
    /// Something to do, reported as [`MenuEvent::Invoked`] when taken.
    pub fn command(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: MenuItemKind::Command,
            shortcut: None,
            icon: None,
            disabled: false,
            destructive: false,
        }
    }

    /// Something that is on or off. The menu draws `checked` and reports the
    /// intent to change it; it never changes it itself.
    pub fn check(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        checked: bool,
    ) -> Self {
        Self {
            kind: MenuItemKind::Check(checked),
            ..Self::command(id, label)
        }
    }

    /// A rule between groups. It carries an id because it is published, and
    /// the keyboard steps over it.
    pub fn separator(id: impl Into<SharedString>) -> Self {
        Self {
            kind: MenuItemKind::Separator,
            ..Self::command(id, "")
        }
    }

    /// A caption naming the group that follows. Not something to act on.
    pub fn section(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            kind: MenuItemKind::Section,
            ..Self::command(id, label)
        }
    }

    /// A nested menu, opening to the side.
    pub fn submenu(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        items: impl IntoIterator<Item = MenuItem>,
    ) -> Self {
        Self {
            kind: MenuItemKind::Submenu(items.into_iter().collect()),
            ..Self::command(id, label)
        }
    }

    /// The keystroke that does the same thing without the menu.
    pub fn shortcut(mut self, keystroke: impl Into<SharedString>) -> Self {
        self.shortcut = Some(keystroke.into());
        self
    }

    pub fn icon(mut self, glyph: Icon) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// Refuses the item. A refused item installs no handler at all.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the item as one that destroys something. The row still reports
    /// [`MenuEvent::Invoked`]; the host decides what that means.
    pub fn destructive(mut self, destructive: bool) -> Self {
        self.destructive = destructive;
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn is_destructive(&self) -> bool {
        self.destructive
    }

    /// The foreground a row uses.
    ///
    /// A refused row keeps the colour it would have had and is dimmed as a
    /// whole instead, the way every other refused row in the library is. Flat
    /// grey text at caption weight is what a section label looks like, so a
    /// recoloured row stopped reading as a command at all.
    fn foreground(&self, theme: &Theme) -> gpui::Hsla {
        if self.destructive {
            theme.colors.danger
        } else {
            theme.colors.text
        }
    }

    /// Whether the keyboard can land on this row.
    fn is_selectable(&self) -> bool {
        !self.disabled
            && matches!(
                self.kind,
                MenuItemKind::Command | MenuItemKind::Check(_) | MenuItemKind::Submenu(_)
            )
    }

    fn children(&self) -> Option<&[MenuItem]> {
        match &self.kind {
            MenuItemKind::Submenu(children) => Some(children),
            _ => None,
        }
    }
}

/// What activating a row did.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Activation {
    Invoked(SharedString),
    OpenedSubmenu,
    /// A separator, a section label, or a refused row: nothing happened.
    Ignored,
}

/// Where the keyboard is, and which submenus stand open.
///
/// Held separately from the items so both menu views share one set of
/// movement rules, and so those rules can be tested without a window.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MenuState {
    /// The index path of the open submenus, outermost first.
    path: Vec<usize>,
    /// The row the keyboard is on in the deepest open panel, which is not a
    /// choice until it is taken.
    active: Option<usize>,
}

/// The items of the panel `path` addresses.
fn level<'a>(items: &'a [MenuItem], path: &[usize]) -> &'a [MenuItem] {
    let mut level = items;
    for index in path {
        match level.get(*index).and_then(MenuItem::children) {
            Some(children) => level = children,
            None => break,
        }
    }
    level
}

fn item_at<'a>(items: &'a [MenuItem], path: &[usize]) -> Option<&'a MenuItem> {
    let (last, parents) = path.split_last()?;
    level(items, parents).get(*last)
}

fn first_selectable(items: &[MenuItem]) -> Option<usize> {
    items.iter().position(MenuItem::is_selectable)
}

impl MenuState {
    fn current<'a>(&self, items: &'a [MenuItem]) -> &'a [MenuItem] {
        level(items, &self.path)
    }

    fn reset(&mut self) {
        self.path.clear();
        self.active = None;
    }

    /// Moves the cursor, stepping over separators, section labels, and rows
    /// the host has refused.
    fn step(&mut self, items: &[MenuItem], delta: isize) {
        let level = self.current(items);
        let count = level.len();
        let Some(start) = popover::step(self.active, count, delta) else {
            return;
        };
        let mut index = start;
        for _ in 0..count {
            if level[index].is_selectable() {
                self.active = Some(index);
                return;
            }
            index = (index as isize + delta.signum()).rem_euclid(count as isize) as usize;
        }
    }

    /// Jumps to the next row whose label starts with `letter`.
    fn jump(&mut self, items: &[MenuItem], letter: char) -> bool {
        let labels: Vec<Option<&str>> = self
            .current(items)
            .iter()
            .map(|item| item.is_selectable().then(|| item.label.as_ref()))
            .collect();
        match popover::jump_to(&labels, self.active, letter) {
            Some(index) => {
                self.active = Some(index);
                true
            }
            None => false,
        }
    }

    /// Opens the submenu the cursor is on and moves into it.
    fn enter(&mut self, items: &[MenuItem]) -> bool {
        let Some(active) = self.active else {
            return false;
        };
        let Some(item) = self.current(items).get(active) else {
            return false;
        };
        if !item.is_selectable() || item.children().is_none() {
            return false;
        }
        self.path.push(active);
        self.active = first_selectable(self.current(items));
        true
    }

    /// Closes the deepest submenu and puts the cursor back on the row that
    /// opened it. Reports whether there was one to close.
    fn leave(&mut self) -> bool {
        match self.path.pop() {
            Some(index) => {
                self.active = Some(index);
                true
            }
            None => false,
        }
    }

    fn activate(&mut self, items: &[MenuItem], path: &[usize]) -> Activation {
        let Some((last, parents)) = path.split_last() else {
            return Activation::Ignored;
        };
        let Some(item) = level(items, parents).get(*last) else {
            return Activation::Ignored;
        };
        if !item.is_selectable() {
            return Activation::Ignored;
        }
        // Acting in a shallower panel closes whatever stood open past it.
        self.path = parents.to_vec();
        self.active = Some(*last);
        if item.children().is_some() {
            self.path = path.to_vec();
            self.active = first_selectable(self.current(items));
            return Activation::OpenedSubmenu;
        }
        Activation::Invoked(item.id.clone())
    }

    /// The index path of the item carrying `id`, at any depth.
    fn path_to(items: &[MenuItem], id: &str) -> Option<Vec<usize>> {
        for (index, item) in items.iter().enumerate() {
            if item.id == id {
                return Some(vec![index]);
            }
            if let Some(children) = item.children()
                && let Some(mut rest) = Self::path_to(children, id)
            {
                rest.insert(0, index);
                return Some(rest);
            }
        }
        None
    }
}

/// Reports the row a pointer or the keyboard took.
type Activate<V> = Rc<dyn Fn(&mut V, Vec<usize>, &mut Window, &mut Context<V>)>;

/// Renders the open panels side by side, outermost first.
fn panels<V: 'static>(
    ident: &Ident,
    items: &[MenuItem],
    state: &MenuState,
    root_parent: SharedString,
    theme: &Theme,
    cx: &mut Context<V>,
    activate: Activate<V>,
) -> gpui::Div {
    let depth = state.path.len();
    let mut rendered: Vec<AnyElement> = Vec::with_capacity(depth + 1);
    for level_depth in 0..=depth {
        let base = state.path[..level_depth].to_vec();
        let parent = if level_depth == 0 {
            root_parent.clone()
        } else {
            match item_at(items, &base) {
                Some(item) => ident.child(item.id.as_ref()).semantic_id(),
                None => root_parent.clone(),
            }
        };
        let active = if level_depth == depth {
            state.active
        } else {
            None
        };
        let opened = (level_depth < depth).then(|| state.path[level_depth]);
        // A submenu belongs to one row, not to the panel that row is in, so
        // it starts level with that row. Anchoring it to the top of the panel
        // instead is what made an expanded "Share" appear to have come out of
        // the heading four rows above it.
        let offset = if level_depth > 0 {
            let owner = state.path[level_depth - 1];
            let siblings = level(items, &state.path[..level_depth - 1]);
            rows_above(&siblings[..owner.min(siblings.len())], theme)
        } else {
            0.0
        };
        let panel = panel(
            ident,
            level(items, &base),
            &base,
            parent,
            active,
            opened,
            theme,
            cx,
            activate.clone(),
        );
        rendered.push(if offset > 0.0 {
            div().mt(px(offset)).child(panel).into_any_element()
        } else {
            panel
        });
    }

    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(theme.space(Space::Xs)))
        .children(rendered)
}

/// How far down a panel the row after `items` starts.
fn rows_above(items: &[MenuItem], theme: &Theme) -> f32 {
    items
        .iter()
        .map(|item| match item.kind {
            MenuItemKind::Separator => popover::menu_separator_height(theme),
            MenuItemKind::Section => popover::menu_heading_height(theme),
            _ => popover::menu_row_height(theme),
        })
        .sum()
}

#[allow(clippy::too_many_arguments)]
fn panel<V: 'static>(
    ident: &Ident,
    items: &[MenuItem],
    base: &[usize],
    parent: SharedString,
    active: Option<usize>,
    opened: Option<usize>,
    theme: &Theme,
    cx: &mut Context<V>,
    activate: Activate<V>,
) -> AnyElement {
    let rows = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let mut path = base.to_vec();
            path.push(index);
            row(
                ident,
                item,
                path,
                parent.clone(),
                active == Some(index),
                opened == Some(index),
                index,
                items.len(),
                theme,
                cx,
                activate.clone(),
            )
        })
        .collect::<Vec<_>>();

    surface(theme, OverlaySurface::FLOATING)
        .min_w(px(theme.measures.menu_min_width))
        .p_token(theme, Space::Xs)
        .children(rows)
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn row<V: 'static>(
    ident: &Ident,
    item: &MenuItem,
    path: Vec<usize>,
    parent: SharedString,
    active: bool,
    opened: bool,
    index: usize,
    count: usize,
    theme: &Theme,
    cx: &mut Context<V>,
    activate: Activate<V>,
) -> AnyElement {
    let row_ident = ident.child(item.id.as_ref());
    match &item.kind {
        MenuItemKind::Separator => popover::separator(theme)
            .semantic_in(
                cx,
                NodeSpec::new(row_ident.semantic_id(), Role::Separator).parent(parent),
            )
            .into_any_element(),
        MenuItemKind::Section => popover::heading(theme, item.label.as_ref())
            .semantic_in(
                cx,
                NodeSpec::new(row_ident.semantic_id(), Role::Heading)
                    .parent(parent)
                    .level(2)
                    .text(item.label.clone()),
            )
            .into_any_element(),
        kind => {
            let checked = match kind {
                MenuItemKind::Check(checked) => Some(*checked),
                _ => None,
            };
            let submenu = item.children().is_some();
            let mut spec = NodeSpec::new(row_ident.semantic_id(), Role::MenuItem)
                .parent(parent)
                .text(item.label.clone())
                .disabled(item.disabled)
                .hovered(active);
            if let Some(checked) = checked {
                spec = spec.checked(checked);
            }
            if submenu {
                spec = spec.expanded(opened);
            }

            let glyph = match (checked, item.icon) {
                (Some(true), _) => Some(Icon::Check),
                (Some(false), _) => None,
                (None, glyph) => glyph,
            };
            let color = item.foreground(theme);

            let row = popover::menu_row(theme, false, active || opened)
                .id(row_ident.element_id())
                .when(active, |element| element.aria_active_descendant())
                .when(item.disabled, |element| {
                    element.opacity(theme.opacity.disabled)
                })
                .when(!item.disabled, |element| {
                    element.cursor_pointer().pressable(cx)
                })
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .w(px(GLYPH_SLOT))
                        .justify_center()
                        .children(glyph.map(|glyph| {
                            icon(glyph)
                                .size(px(theme.control.sm.icon_size))
                                .text_color(color)
                        })),
                )
                .child(
                    text(theme, TypeScale::Label, item.label.clone())
                        .flex_1()
                        .text_color(color),
                )
                .children(
                    item.shortcut
                        .clone()
                        .map(|keystroke| Kbd::new(keystroke).into_any_element()),
                )
                .when(submenu, |element| {
                    // The chevron says "there is more this way", and this
                    // way is the way the menu reads.
                    let flipped = flips(Icon::AltArrowRight, cx.layout_direction());
                    element.child(
                        icon(Icon::AltArrowRight)
                            .size(px(theme.control.xs.icon_size))
                            .text_color(if item.destructive {
                                theme.colors.danger
                            } else {
                                theme.colors.text_muted
                            })
                            .when(flipped, |glyph| {
                                glyph.with_transformation(Transformation::scale(gpui::size(
                                    -1.0, 1.0,
                                )))
                            }),
                    )
                })
                .when(!item.disabled, |element| {
                    element.on_click(cx.listener(move |view, _, window, cx| {
                        activate(view, path.clone(), window, cx);
                    }))
                })
                .semantic_in(cx, spec);

            motion::row_in(row_ident.child("in").element_id(), theme, index, count, row)
                .into_any_element()
        }
    }
}

/// What a menu reports. The owner decides what any of it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEvent {
    Opened,
    /// A command or a checkable row was taken. For a checkable row this is the
    /// intent to change it, which the menu does not act on itself.
    Invoked(SharedString),
    /// The menu was waved away, by escape or by a click outside it.
    Dismissed,
    Closed,
}

impl EventEmitter<MenuEvent> for Menu {}

/// A list of commands, opened from a trigger.
pub struct Menu {
    ident: Ident,
    focus_handle: FocusHandle,
    trigger_focus: FocusHandle,
    trigger: SharedString,
    trigger_icon: Option<Icon>,
    /// What the trigger is called when it carries a glyph instead of a label.
    trigger_name: Option<SharedString>,
    trigger_variant: ButtonVariant,
    trigger_join: ButtonJoin,
    trigger_size: ControlSize,
    items: Vec<MenuItem>,
    placement: Placement,
    hang: Hang,
    open: bool,
    /// Set when opening, cleared by the first frame that can act on it.
    pending_focus: bool,
    state: MenuState,
    trap: FocusTrap,
}

impl std::fmt::Debug for Menu {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Menu")
            .field("ident", &self.ident)
            .field("trigger", &self.trigger)
            .field("items", &self.items.len())
            .field("open", &self.open)
            .field("open_submenus", &self.state.path.len())
            .finish()
    }
}

impl Menu {
    pub fn new(ident: impl Into<Ident>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            trigger_focus: cx.focus_handle(),
            trigger: SharedString::default(),
            trigger_icon: None,
            trigger_name: None,
            trigger_variant: ButtonVariant::Secondary,
            trigger_join: ButtonJoin::Alone,
            trigger_size: ControlSize::Md,
            items: Vec::new(),
            placement: Placement::Below,
            hang: Hang::Start,
            open: false,
            pending_focus: false,
            state: MenuState::default(),
            trap: FocusTrap::new(),
        }
    }

    /// The label of the control that opens the menu.
    pub fn trigger(mut self, label: impl Into<SharedString>) -> Self {
        self.trigger = label.into();
        self
    }

    pub fn trigger_icon(mut self, glyph: Icon) -> Self {
        self.trigger_icon = Some(glyph);
        self
    }

    /// What the trigger is called when it has no label of its own, for a menu
    /// opened from a glyph such as the arrow of a split button.
    pub fn trigger_name(mut self, name: impl Into<SharedString>) -> Self {
        self.trigger_name = Some(name.into());
        self
    }

    pub fn set_trigger_name(&mut self, name: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.trigger_name = Some(name.into());
        cx.notify();
    }

    /// How the trigger is painted, for a menu that sits inside another
    /// control and has to look like part of it.
    pub fn trigger_variant(mut self, variant: ButtonVariant) -> Self {
        self.trigger_variant = variant;
        self
    }

    /// Where the trigger sits in a joined run of buttons.
    pub fn trigger_join(mut self, join: ButtonJoin) -> Self {
        self.trigger_join = join;
        self
    }

    /// Repaints the trigger for an owner whose own paint is decided after the
    /// menu is built.
    pub fn set_trigger_style(
        &mut self,
        variant: ButtonVariant,
        size: ControlSize,
        cx: &mut Context<Self>,
    ) {
        self.trigger_variant = variant;
        self.trigger_size = size;
        cx.notify();
    }

    pub fn items(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
        self.items = items.into_iter().collect();
        self
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Which of the trigger's edges the surface hangs from.
    ///
    /// A trigger near the trailing edge of the window wants [`Hang::End`]: the
    /// surface then grows back across the page instead of being slid sideways
    /// off its trigger to stay inside the window.
    pub fn hang(mut self, hang: Hang) -> Self {
        self.hang = hang;
        self
    }

    /// The rows the menu currently offers, so an owner that maintains them can
    /// tell whether a replacement would change anything.
    pub fn offered(&self) -> &[MenuItem] {
        &self.items
    }

    /// Replaces the items from the host side, dropping a cursor that pointed
    /// into what is no longer offered.
    pub fn set_items(&mut self, items: Vec<MenuItem>, cx: &mut Context<Self>) {
        self.items = items;
        self.state.reset();
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        self.pending_focus = true;
        self.state.reset();
        self.state.active = first_selectable(&self.items);
        self.trap.engage(window, cx);
        cx.emit(MenuEvent::Opened);
        cx.notify();
    }

    /// Opens the menu with the submenu carrying `id` already expanded, and
    /// reports whether such a submenu exists.
    pub fn open_submenu(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(path) = MenuState::path_to(&self.items, id) else {
            return false;
        };
        if item_at(&self.items, &path)
            .and_then(MenuItem::children)
            .is_none()
        {
            return false;
        }
        self.open(window, cx);
        self.state.path = path;
        self.state.active = first_selectable(self.state.current(&self.items));
        cx.notify();
        true
    }

    /// Closes the menu and every submenu under it, and gives the keyboard back
    /// to the trigger.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.pending_focus = false;
        self.state.reset();
        self.trap.release(window, cx);
        self.trigger_focus.focus(window, cx);
        cx.emit(MenuEvent::Closed);
        cx.notify();
    }

    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        cx.emit(MenuEvent::Dismissed);
        self.close(window, cx);
    }

    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.dismiss(window, cx);
        } else {
            self.open(window, cx);
        }
    }

    fn take(&mut self, path: Vec<usize>, window: &mut Window, cx: &mut Context<Self>) {
        match self.state.activate(&self.items, &path) {
            Activation::Invoked(id) => {
                cx.emit(MenuEvent::Invoked(id));
                self.close(window, cx);
            }
            Activation::OpenedSubmenu => cx.notify(),
            Activation::Ignored => {}
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        let handled = handle_key(&mut self.state, &self.items, event, cx.layout_direction());
        match handled {
            Handled::Moved => {
                cx.notify();
                cx.stop_propagation();
            }
            Handled::Activate(path) => {
                self.take(path, window, cx);
                cx.stop_propagation();
            }
            Handled::Close => {
                self.dismiss(window, cx);
                cx.stop_propagation();
            }
            Handled::None => {}
        }
    }
}

/// What a keystroke asked a menu to do.
enum Handled {
    Moved,
    Activate(Vec<usize>),
    /// Close the whole menu: nothing was left to fold away.
    Close,
    None,
}

/// Resolves a physical sideways key into logical submenu motion. `true`
/// enters a child and `false` returns to its parent.
fn submenu_intent(key: MenuKey, direction: LayoutDirection) -> Option<bool> {
    match (key, direction) {
        (MenuKey::Right, LayoutDirection::LeftToRight)
        | (MenuKey::Left, LayoutDirection::RightToLeft) => Some(true),
        (MenuKey::Left, LayoutDirection::LeftToRight)
        | (MenuKey::Right, LayoutDirection::RightToLeft) => Some(false),
        _ => None,
    }
}

/// Applies one keystroke to the cursor, shared by both menu views.
fn handle_key(
    state: &mut MenuState,
    items: &[MenuItem],
    event: &KeyDownEvent,
    direction: LayoutDirection,
) -> Handled {
    let key = popover::classify_key(
        event.keystroke.key.as_str(),
        event.keystroke.modifiers.platform,
        event.keystroke.modifiers.control,
    );
    // A sideways key that entered or left a submenu is this menu's; one that
    // found no submenu to move through did nothing, which lets a menubar take
    // the same key and step to its next top-level menu instead.
    if let Some(enter) = submenu_intent(key, direction) {
        let moved = if enter {
            state.enter(items)
        } else {
            state.leave()
        };
        return if moved { Handled::Moved } else { Handled::None };
    }
    match key {
        MenuKey::Down => {
            state.step(items, 1);
            Handled::Moved
        }
        MenuKey::Up => {
            state.step(items, -1);
            Handled::Moved
        }
        MenuKey::Enter => match state.active {
            Some(active) => {
                let mut path = state.path.clone();
                path.push(active);
                Handled::Activate(path)
            }
            None => Handled::Moved,
        },
        // Escape folds one submenu away at a time, so a typist who opened one
        // by mistake does not lose the whole menu.
        MenuKey::Escape => {
            if state.leave() {
                Handled::Moved
            } else {
                Handled::Close
            }
        }
        _ => match popover::typed_letter(event.keystroke.key.as_str(), event.keystroke.modifiers) {
            Some(letter) if state.jump(items, letter) => Handled::Moved,
            _ => Handled::None,
        },
    }
}

impl Sizable for Menu {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.trigger_size = size;
        self
    }
}

impl Focusable for Menu {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Menu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let menu = cx.entity().downgrade();
        let glyph_only = self.trigger.is_empty();
        let trigger = Button::new(self.ident.child("trigger"))
            .label(self.trigger.clone())
            .variant(self.trigger_variant)
            .join(self.trigger_join)
            .control_size(self.trigger_size)
            // An open menu marks the control it came out of. A ghost trigger
            // that keeps its resting appearance while a panel hangs off it
            // leaves the panel looking like it belongs to the whole row.
            .selected(self.open)
            .track_focus(&self.trigger_focus)
            .when_some(self.trigger_icon, |button, glyph| {
                match (glyph_only, self.trigger_name.clone()) {
                    (true, Some(name)) => button.icon_only(glyph, name),
                    _ => button.icon(glyph),
                }
            })
            .when_some(self.trigger_name.clone(), |button, name| {
                button.accessible_name(name)
            })
            .on_click(move |window, cx| {
                menu.update(cx, |menu, cx| menu.toggle(window, cx)).ok();
            })
            .into_any_element();

        let overlay = self.open.then(|| {
            if self.pending_focus {
                // The handle can only take focus once this frame has put it in
                // the dispatch tree, which is why opening records the intent.
                self.pending_focus = false;
                self.focus_handle.focus(window, cx);
            }
            let activate: Activate<Self> =
                Rc::new(|menu: &mut Self, path, window, cx| menu.take(path, window, cx));
            let menu_id = self.ident.child("menu").semantic_id();
            let menu_name = self
                .trigger_name
                .clone()
                .unwrap_or_else(|| self.trigger.clone());
            let content = panels(
                &self.ident,
                &self.items,
                &self.state,
                menu_id.clone(),
                &theme,
                cx,
                activate,
            )
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down_out(cx.listener(|menu, _, window, cx| menu.dismiss(window, cx)))
            .semantic_in(
                cx,
                NodeSpec::new(menu_id, Role::Menu)
                    .parent(self.ident.semantic_id())
                    .text(menu_name)
                    .expanded(true)
                    .focus(&self.focus_handle),
            );

            Overlay::new(self.ident.child("overlay"))
                .placement(self.placement)
                .hang(self.hang)
                .child(content)
                .into_any_element()
        });

        popover::anchored_slot(self.placement, self.hang, trigger, overlay).semantic_in(
            cx,
            NodeSpec::new(self.ident.semantic_id(), Role::Group)
                .expanded(self.open)
                .focus(&self.focus_handle),
        )
    }
}

/// What a context menu reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextMenuEvent {
    /// The region was right-clicked. The payload names what was pointed at;
    /// the host decides what that means for its selection, because opening a
    /// menu is not choosing anything.
    Opened(SharedString),
    Invoked(SharedString),
    Dismissed,
    Closed,
}

impl EventEmitter<ContextMenuEvent> for ContextMenu {}

/// Builds the wrapped region for one frame.
type Content = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// The same menu, opened at the pointer over a region rather than from a
/// trigger.
pub struct ContextMenu {
    ident: Ident,
    focus_handle: FocusHandle,
    name: SharedString,
    target: Option<SharedString>,
    items: Vec<MenuItem>,
    content: Option<Content>,
    open: bool,
    position: Point<Pixels>,
    pending_focus: bool,
    state: MenuState,
    trap: FocusTrap,
}

impl std::fmt::Debug for ContextMenu {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContextMenu")
            .field("ident", &self.ident)
            .field("target", &self.target)
            .field("items", &self.items.len())
            .field("open", &self.open)
            .finish()
    }
}

impl ContextMenu {
    pub fn new(ident: impl Into<Ident>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            name: SharedString::default(),
            target: None,
            items: Vec::new(),
            content: None,
            open: false,
            position: gpui::point(px(0.0), px(0.0)),
            pending_focus: false,
            state: MenuState::default(),
            trap: FocusTrap::new(),
        }
    }

    pub fn menu(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
        self.items = items.into_iter().collect();
        self
    }

    /// Names the command surface independently of the region under it.
    pub fn name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = name.into();
        self
    }

    pub fn set_name(&mut self, name: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.name = name.into();
        cx.notify();
    }

    /// What the wrapped region stands for. Reported when the menu opens, so a
    /// host can decide whether to select it. Defaults to the menu's own id.
    pub fn target(mut self, target: impl Into<SharedString>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Supplies the wrapped region, rebuilt on every frame.
    pub fn content(
        mut self,
        content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.content = Some(Rc::new(content));
        self
    }

    pub fn set_items(&mut self, items: Vec<MenuItem>, cx: &mut Context<Self>) {
        self.items = items;
        self.state.reset();
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn position(&self) -> Point<Pixels> {
        self.position
    }

    /// Opens at a window position. Reports the target and nothing else: what
    /// is selected stays the host's answer.
    pub fn open_at(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.position = position;
        self.state.reset();
        self.state.active = first_selectable(&self.items);
        if !self.open {
            self.open = true;
            self.pending_focus = true;
            self.trap.engage(window, cx);
        }
        let target = self
            .target
            .clone()
            .unwrap_or_else(|| self.ident.semantic_id());
        cx.emit(ContextMenuEvent::Opened(target));
        cx.notify();
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.pending_focus = false;
        self.state.reset();
        self.trap.release(window, cx);
        cx.emit(ContextMenuEvent::Closed);
        cx.notify();
    }

    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        cx.emit(ContextMenuEvent::Dismissed);
        self.close(window, cx);
    }

    fn take(&mut self, path: Vec<usize>, window: &mut Window, cx: &mut Context<Self>) {
        match self.state.activate(&self.items, &path) {
            Activation::Invoked(id) => {
                cx.emit(ContextMenuEvent::Invoked(id));
                self.close(window, cx);
            }
            Activation::OpenedSubmenu => cx.notify(),
            Activation::Ignored => {}
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        match handle_key(&mut self.state, &self.items, event, cx.layout_direction()) {
            Handled::Moved => {
                cx.notify();
                cx.stop_propagation();
            }
            Handled::Activate(path) => {
                self.take(path, window, cx);
                cx.stop_propagation();
            }
            Handled::Close => {
                self.dismiss(window, cx);
                cx.stop_propagation();
            }
            Handled::None => {}
        }
    }
}

impl Focusable for ContextMenu {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ContextMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let region = self.content.clone().map(|content| content(window, cx));
        let menu_id = self.ident.child("menu").semantic_id();

        let overlay = self.open.then(|| {
            if self.pending_focus {
                self.pending_focus = false;
                self.focus_handle.focus(window, cx);
            }
            let activate: Activate<Self> =
                Rc::new(|menu: &mut Self, path, window, cx| menu.take(path, window, cx));
            let content = panels(
                &self.ident,
                &self.items,
                &self.state,
                menu_id.clone(),
                &theme,
                cx,
                activate,
            )
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down_out(cx.listener(|menu, _, window, cx| menu.dismiss(window, cx)))
            .semantic_in(
                cx,
                NodeSpec::new(menu_id.clone(), Role::Menu)
                    .parent(self.ident.semantic_id())
                    .text(self.name.clone())
                    .expanded(true)
                    .focus(&self.focus_handle),
            );

            Overlay::new(self.ident.child("overlay"))
                .placement(Placement::At(self.position))
                .child(content)
                .into_any_element()
        });

        div()
            .id(self.ident.element_id())
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|menu, event: &MouseDownEvent, window, cx| {
                    menu.open_at(event.position, window, cx);
                    cx.stop_propagation();
                }),
            )
            .children(region)
            .children(overlay)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Region).expanded(self.open),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<MenuItem> {
        vec![
            MenuItem::section("edit.section", "Edit"),
            MenuItem::command("edit.undo", "Undo").shortcut("cmd-z"),
            MenuItem::separator("edit.rule"),
            MenuItem::command("edit.paste", "Paste").disabled(true),
            MenuItem::check("edit.wrap", "Wrap lines", true),
            MenuItem::submenu(
                "edit.share",
                "Share",
                [
                    MenuItem::command("edit.share.link", "Copy link"),
                    MenuItem::command("edit.share.mail", "Send by mail"),
                ],
            ),
        ]
    }

    fn cursor() -> MenuState {
        MenuState {
            path: Vec::new(),
            active: first_selectable(&items()),
        }
    }

    #[test]
    fn the_cursor_starts_on_something_that_can_be_taken() {
        assert_eq!(cursor().active, Some(1));
    }

    #[test]
    fn moving_steps_over_labels_rules_and_refusals() {
        let items = items();
        let mut state = cursor();
        state.step(&items, 1);
        assert_eq!(
            state.active,
            Some(4),
            "the rule and the refusal are skipped"
        );
        state.step(&items, 1);
        assert_eq!(state.active, Some(5));
        state.step(&items, 1);
        assert_eq!(state.active, Some(1), "the cursor wraps past the section");
        state.step(&items, -1);
        assert_eq!(state.active, Some(5));
    }

    #[test]
    fn typing_a_letter_jumps_to_the_next_row_that_starts_with_it() {
        let items = items();
        let mut state = cursor();
        assert!(state.jump(&items, 'w'));
        assert_eq!(state.active, Some(4));
        assert!(!state.jump(&items, 'p'), "a refused row is not jumped to");
    }

    #[test]
    fn a_submenu_is_entered_and_left_without_invoking_anything() {
        let items = items();
        let mut state = cursor();
        state.active = Some(5);
        assert!(state.enter(&items));
        assert_eq!(state.path, vec![5]);
        assert_eq!(state.active, Some(0));

        state.step(&items, 1);
        assert_eq!(state.active, Some(1));
        assert!(state.leave());
        assert_eq!(state.path, Vec::<usize>::new());
        assert_eq!(state.active, Some(5), "the cursor returns to the submenu");
        assert!(!state.leave(), "there is nothing left to fold away");
    }

    #[test]
    fn submenu_keys_follow_the_reading_direction() {
        assert_eq!(
            submenu_intent(MenuKey::Right, LayoutDirection::LeftToRight),
            Some(true)
        );
        assert_eq!(
            submenu_intent(MenuKey::Left, LayoutDirection::LeftToRight),
            Some(false)
        );
        assert_eq!(
            submenu_intent(MenuKey::Left, LayoutDirection::RightToLeft),
            Some(true)
        );
        assert_eq!(
            submenu_intent(MenuKey::Right, LayoutDirection::RightToLeft),
            Some(false)
        );
    }

    #[test]
    fn a_row_that_cannot_be_taken_reports_nothing() {
        let items = items();
        let mut state = cursor();
        assert_eq!(state.activate(&items, &[0]), Activation::Ignored);
        assert_eq!(state.activate(&items, &[2]), Activation::Ignored);
        assert_eq!(state.activate(&items, &[3]), Activation::Ignored);
    }

    #[test]
    fn taking_a_checkable_row_reports_it_and_changes_nothing() {
        let items = items();
        let mut state = cursor();
        assert_eq!(
            state.activate(&items, &[4]),
            Activation::Invoked("edit.wrap".into())
        );
        assert_eq!(
            items[4].kind,
            MenuItemKind::Check(true),
            "the menu does not toggle its own item"
        );
    }

    #[test]
    fn acting_in_an_outer_panel_folds_the_open_submenu_away() {
        let items = items();
        let mut state = cursor();
        state.activate(&items, &[5]);
        assert_eq!(state.path, vec![5]);
        assert_eq!(
            state.activate(&items, &[1]),
            Activation::Invoked("edit.undo".into())
        );
        assert_eq!(state.path, Vec::<usize>::new());
    }

    #[test]
    fn an_item_is_addressed_by_identity_at_any_depth() {
        let items = items();
        assert_eq!(MenuState::path_to(&items, "edit.share"), Some(vec![5]));
        assert_eq!(
            MenuState::path_to(&items, "edit.share.mail"),
            Some(vec![5, 1])
        );
        assert_eq!(MenuState::path_to(&items, "edit.nothing"), None);
    }

    #[test]
    fn a_destructive_row_is_still_something_that_can_be_taken() {
        let item = MenuItem::command("delete", "Delete").destructive(true);
        assert!(item.is_destructive());
        assert!(item.is_selectable());
        let items = vec![item];
        let mut state = MenuState {
            path: Vec::new(),
            active: Some(0),
        };
        assert_eq!(
            state.activate(&items, &[0]),
            Activation::Invoked("delete".into())
        );
    }

    #[test]
    fn a_destructive_refusal_still_installs_nothing() {
        let item = MenuItem::command("delete", "Delete")
            .destructive(true)
            .disabled(true);
        assert!(item.is_destructive());
        assert!(item.is_disabled());
        assert!(!item.is_selectable());
        let items = vec![item];
        let mut state = MenuState::default();
        assert_eq!(state.activate(&items, &[0]), Activation::Ignored);
    }

    #[test]
    fn a_refused_row_keeps_the_colour_of_the_command_it_is() {
        // Flat grey at caption weight is what a section label looks like, so
        // a refused row that was recoloured stopped reading as a command.
        // Refusal is carried by dimming the whole row instead.
        let theme = Theme::studio_dark();
        let refused = MenuItem::command("publish", "Publish").disabled(true);
        assert_eq!(refused.foreground(&theme), theme.colors.text);
        let refused_destructive = MenuItem::command("delete", "Delete")
            .destructive(true)
            .disabled(true);
        assert_eq!(refused_destructive.foreground(&theme), theme.colors.danger);
        assert!(theme.opacity.disabled < 1.0);
    }
}
