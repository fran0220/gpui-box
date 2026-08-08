//! The horizontal row of menus along the top of a window's content.
//!
//! # What this adds, and what it reuses
//!
//! Everything inside an open menu — rows, checkable items, separators, section
//! labels, submenus, type-ahead, up and down, escape folding one submenu away
//! at a time — is [`Menu`], unchanged. A menubar holds one [`Menu`] view per
//! title and coordinates them; it does not reimplement a single row.
//!
//! What it adds is the three behaviours that only exist because the menus sit
//! in a row:
//!
//! 1. **At most one is open.** Opening a menu closes whichever was open.
//! 2. **Hover switches, but only once one is open.** Before that, hovering a
//!    title does nothing, because a menubar that opened on hover from cold
//!    would ambush anybody whose pointer crossed it on the way somewhere else.
//!    After that, the row behaves as one surface and moving along it moves the
//!    open menu, with no second click.
//! 3. **The reading-order arrows step between titles.** Left and right are
//!    read through [`LayoutDirection::arrow_step`](crate::foundation::direction::LayoutDirection::arrow_step), so in a right-to-left
//!    layout the arrow that points at the next title is the one that opens it.
//!
//! Escape needs nothing added: [`Menu`] already closes and hands the keyboard
//! back to its own trigger, and the menubar learns the menu closed from the
//! event rather than by being told twice.
//!
//! # Why the arrows do not fight the submenus
//!
//! Left and right already mean "enter this submenu" and "leave it" inside an
//! open menu. So the menubar takes them on the way back up rather than on the
//! way down: [`Menu`] stops a sideways key that actually moved through a
//! submenu and declines one that found no submenu to move through, and only a
//! declined key reaches the row. The deeper surface gets first refusal, which
//! is the only ordering under which "right opens the submenu" and "right opens
//! the next menu" can both be true of the same key.
//!
//! The row has ends rather than wrapping, which is the rule every strip in
//! this library keeps. Arrowing off the last title stops there instead of
//! silently reappearing at the other end of the window.

use gpui::{
    AppContext as _, Context, Entity, EventEmitter, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, div,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space};

use crate::controls::button::{Button, ButtonVariant};
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::stepping::bounded_step;
use crate::foundation::{Disableable, Ident, Sizable, StyledExt};
use crate::overlay::menu::{Menu, MenuEvent, MenuItem};

/// One title in the bar and the commands under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenubarMenu {
    id: SharedString,
    label: SharedString,
    items: Vec<MenuItem>,
    disabled: bool,
}

impl MenubarMenu {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        items: impl IntoIterator<Item = MenuItem>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            items: items.into_iter().collect(),
            disabled: false,
        }
    }

    /// Refuses the whole title. A refused title installs no handler, opens on
    /// no hover, and the arrows step over it.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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
}

/// What a menubar reports. The owner decides what any of it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenubarEvent {
    Opened(SharedString),
    /// A row was taken, named together with the menu it came from, because two
    /// menus may offer the same command and the host may care which one asked.
    Invoked {
        menu: SharedString,
        item: SharedString,
    },
    Closed(SharedString),
}

impl EventEmitter<MenubarEvent> for Menubar {}

/// A row of menus, at most one of them open.
pub struct Menubar {
    ident: Ident,
    menus: Vec<MenubarMenu>,
    /// One view per title, absent for a title the host refused: a refused
    /// title is a plain disabled button with no menu behind it at all.
    views: Vec<Option<Entity<Menu>>>,
    open: Option<SharedString>,
    size: ControlSize,
}

impl std::fmt::Debug for Menubar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Menubar")
            .field("ident", &self.ident)
            .field("menus", &self.menus.len())
            .field("open", &self.open)
            .finish()
    }
}

impl Menubar {
    pub fn new(
        ident: impl Into<Ident>,
        menus: impl IntoIterator<Item = MenubarMenu>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ident = ident.into();
        let menus: Vec<MenubarMenu> = menus.into_iter().collect();
        let mut bar = Self {
            ident,
            menus,
            views: Vec::new(),
            open: None,
            size: ControlSize::Sm,
        };
        bar.build_views(window, cx);
        bar
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// The menu standing open, if any.
    pub fn open_menu(&self) -> Option<&SharedString> {
        self.open.as_ref()
    }

    pub fn menus(&self) -> &[MenubarMenu] {
        &self.menus
    }

    /// Replaces the titles, closing anything that was open.
    pub fn set_menus(
        &mut self,
        menus: Vec<MenubarMenu>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close(window, cx);
        self.menus = menus;
        self.build_views(window, cx);
        cx.notify();
    }

    fn build_views(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ident = self.ident.clone();
        let size = self.size;
        let menus = self.menus.clone();
        let mut views = Vec::with_capacity(menus.len());
        for menu in &menus {
            if menu.disabled {
                views.push(None);
                continue;
            }
            let id = menu.id.clone();
            let view = cx.new(|cx| {
                Menu::new(ident.child(id.as_ref()), window, cx)
                    .trigger(menu.label.clone())
                    .trigger_variant(ButtonVariant::Ghost)
                    .control_size(size)
                    .items(menu.items.clone())
            });
            cx.subscribe(&view, {
                let id = id.clone();
                move |bar, _, event: &MenuEvent, cx| bar.on_menu_event(&id, event, cx)
            })
            .detach();
            views.push(Some(view));
        }
        self.views = views;
    }

    fn on_menu_event(&mut self, id: &SharedString, event: &MenuEvent, cx: &mut Context<Self>) {
        match event {
            MenuEvent::Opened => {
                self.open = Some(id.clone());
                cx.emit(MenubarEvent::Opened(id.clone()));
                cx.notify();
            }
            MenuEvent::Closed => {
                // Only the menu the bar still believes is open clears it: a
                // sibling closing on its way out of the way must not erase the
                // record of the one that just replaced it.
                if self.open.as_ref() == Some(id) {
                    self.open = None;
                }
                cx.emit(MenubarEvent::Closed(id.clone()));
                cx.notify();
            }
            MenuEvent::Invoked(item) => cx.emit(MenubarEvent::Invoked {
                menu: id.clone(),
                item: item.clone(),
            }),
            MenuEvent::Dismissed => {}
        }
    }

    fn index_of(&self, id: &SharedString) -> Option<usize> {
        self.menus.iter().position(|menu| &menu.id == id)
    }

    fn view_for(&self, id: &SharedString) -> Option<&Entity<Menu>> {
        self.index_of(id)
            .and_then(|index| self.views[index].as_ref())
    }

    /// Opens one title, closing whatever stood open. A title the host refused
    /// has no menu and opens nothing.
    ///
    /// The old menu is closed before the new one opens rather than in reaction
    /// to it, because closing hands the keyboard back to its own trigger, and
    /// doing that after the new menu had taken focus would pull the keyboard
    /// back out of the menu the typist just opened.
    pub fn open(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let wanted = SharedString::from(id.to_string());
        let Some(view) = self.view_for(&wanted).cloned() else {
            return;
        };
        if let Some(open) = self.open.clone().filter(|open| open != &wanted) {
            self.close_menu(&open, window, cx);
        }
        view.update(cx, |menu, cx| menu.open(window, cx));
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = self.open.clone() else {
            return;
        };
        self.close_menu(&open, window, cx);
    }

    fn close_menu(&mut self, id: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.view_for(id).cloned() else {
            return;
        };
        view.update(cx, |menu, cx| menu.close(window, cx));
    }

    /// Hovering a title once one is open moves the open menu onto it, which is
    /// what a row of menus does everywhere and what a row of buttons does
    /// nowhere.
    fn on_hover_title(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = self.open.clone() else {
            return;
        };
        let Some(menu) = self.menus.get(index) else {
            return;
        };
        if menu.disabled || menu.id == open {
            return;
        }
        let id = menu.id.clone();
        self.open(id.as_ref(), window, cx);
    }

    /// Steps to the next title, but only with a key the open menu declined.
    ///
    /// This runs on the way back up, after the open menu has had the key, so a
    /// right that entered a submenu never reaches here and a right that found
    /// no submenu to enter does.
    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = self.open.clone() else {
            return;
        };
        let Some(step) = cx
            .layout_direction()
            .arrow_step(event.keystroke.key.as_str())
        else {
            return;
        };
        let from = self.index_of(&open);
        let refused = |index: usize| self.menus[index].disabled;
        let Some(next) = bounded_step(self.menus.len(), from, step as isize, refused) else {
            return;
        };
        let id = self.menus[next].id.clone();
        self.open(id.as_ref(), window, cx);
        cx.stop_propagation();
    }
}

impl Sizable for Menubar {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Render for Menubar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let bar_id = self.ident.semantic_id();

        let titles = self
            .menus
            .iter()
            .enumerate()
            .map(|(index, menu)| {
                let ident = self.ident.child(menu.id.as_ref());
                match &self.views[index] {
                    Some(view) => div()
                        .id(ident.child("title").element_id())
                        .flex()
                        .flex_none()
                        .on_hover(cx.listener(move |bar, hovered: &bool, window, cx| {
                            if *hovered {
                                bar.on_hover_title(index, window, cx);
                            }
                        }))
                        .child(view.clone())
                        .into_any_element(),
                    // A refused title installs nothing: no menu, no hover, no
                    // click handler, so there is no path by which it can open.
                    None => Button::new(ident)
                        .label(menu.label.clone())
                        .ghost()
                        .control_size(self.size)
                        .disabled(true)
                        .semantic_parent(bar_id.clone())
                        .into_any_element(),
                }
            })
            .collect::<Vec<_>>();

        div()
            .id(self.ident.child("bar").element_id())
            .row_reading(direction)
            .flex_none()
            .gap_token(&theme, Space::Xs)
            .on_key_down(cx.listener(Self::on_key))
            .children(titles)
            .semantic_in(
                cx,
                NodeSpec::new(bar_id, Role::Toolbar).expanded(self.open.is_some()),
            )
    }
}
