//! A horizontal bar of grouped actions, with the rest behind an overflow menu.
//!
//! # Why overflow is declared, not measured
//!
//! A truthful overflow needs to know how wide every item is before it decides
//! which ones fit. GPUI measures during layout, which happens after the
//! element tree has been built, and a toolbar child is an `AnyElement` that
//! can be consumed exactly once — so a builder cannot measure a child and then
//! still move it into a menu. Guessing at widths would produce a bar that
//! claims to have dropped items it in fact drew, which is worse than not
//! overflowing at all.
//!
//! So the caller declares the cut with [`Toolbar::overflow_after`], and the
//! toolbar guarantees the part it can: an item past the cut is **moved**, never
//! dropped. It becomes a row in the overflow [`Menu`],
//! keeping its identity, its label, and its refusal, and the trigger publishes
//! how many items went there. With no menu to move them into, every item is
//! drawn inline, because losing an action is never the better failure.

use gpui::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, Theme};

use crate::foundation::{Ident, Sizable, StyledExt};
use crate::overlay::{Menu, MenuItem};

/// One action in the bar.
///
/// The element is what the bar draws; the id and label are what the overflow
/// menu needs to offer the same action when the item does not fit.
pub struct ToolbarItem {
    id: SharedString,
    label: SharedString,
    icon: Option<Icon>,
    shortcut: Option<SharedString>,
    disabled: bool,
    content: AnyElement,
}

impl std::fmt::Debug for ToolbarItem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToolbarItem")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl ToolbarItem {
    /// `id` is the action's business identity, and is what the overflow menu
    /// reports when the item is taken from there instead of from the bar.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            shortcut: None,
            disabled: false,
            content: content.into_any_element(),
        }
    }

    /// The glyph the overflow row carries. The inline control draws its own.
    pub fn icon(mut self, glyph: Icon) -> Self {
        self.icon = Some(glyph);
        self
    }

    pub fn shortcut(mut self, keystroke: impl Into<SharedString>) -> Self {
        self.shortcut = Some(keystroke.into());
        self
    }

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

    fn menu_row(&self) -> MenuItem {
        let mut row =
            MenuItem::command(self.id.clone(), self.label.clone()).disabled(self.disabled);
        if let Some(glyph) = self.icon {
            row = row.icon(glyph);
        }
        if let Some(shortcut) = self.shortcut.clone() {
            row = row.shortcut(shortcut);
        }
        row
    }
}

/// What sits between two groups.
enum Slot {
    Group {
        id: SharedString,
        items: Vec<ToolbarItem>,
    },
    /// Pushes everything after it to the far end of the bar.
    Spacer,
}

/// A bar of grouped actions.
#[derive(IntoElement)]
pub struct Toolbar {
    ident: Ident,
    label: Option<SharedString>,
    slots: Vec<Slot>,
    size: ControlSize,
    overflow_after: Option<usize>,
    overflow_menu: Option<Entity<Menu>>,
}

impl std::fmt::Debug for Toolbar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Toolbar")
            .field("ident", &self.ident)
            .field("items", &self.item_count())
            .field("overflow_after", &self.overflow_after)
            .field("has_overflow_menu", &self.overflow_menu.is_some())
            .finish()
    }
}

impl Toolbar {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            slots: Vec::new(),
            size: ControlSize::Md,
            overflow_after: None,
            overflow_menu: None,
        }
    }

    /// What the bar is called, for a reader that has only the tree.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A run of related actions, separated from its neighbours by a rule.
    pub fn group(
        mut self,
        id: impl Into<SharedString>,
        items: impl IntoIterator<Item = ToolbarItem>,
    ) -> Self {
        self.slots.push(Slot::Group {
            id: id.into(),
            items: items.into_iter().collect(),
        });
        self
    }

    /// Flexible space: everything after it sits at the far end of the bar.
    pub fn spacer(mut self) -> Self {
        self.slots.push(Slot::Spacer);
        self
    }

    /// Keeps the first `count` items in the bar and moves the rest into the
    /// overflow menu. See the module documentation for why the cut is declared
    /// rather than measured.
    pub fn overflow_after(mut self, count: usize) -> Self {
        self.overflow_after = Some(count);
        self
    }

    /// The menu the overflowed items are moved into.
    ///
    /// It is caller-owned because whether it is open outlives a frame.
    pub fn overflow_menu(mut self, menu: Entity<Menu>) -> Self {
        self.overflow_menu = Some(menu);
        self
    }

    /// How many actions the bar holds, drawn or overflowed.
    pub fn item_count(&self) -> usize {
        self.slots
            .iter()
            .map(|slot| match slot {
                Slot::Group { items, .. } => items.len(),
                Slot::Spacer => 0,
            })
            .sum()
    }

    /// Where the cut falls, which is nowhere when there is no menu to move
    /// anything into.
    fn cut(&self) -> usize {
        match (self.overflow_after, self.overflow_menu.is_some()) {
            (Some(cut), true) => cut,
            _ => usize::MAX,
        }
    }
}

impl Sizable for Toolbar {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Toolbar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let total = self.item_count();
        let cut = self.cut();
        let ident = self.ident.clone();

        let mut drawn: Vec<AnyElement> = Vec::new();
        let mut overflowed: Vec<MenuItem> = Vec::new();
        let mut index = 0usize;
        let mut previous_group = false;

        for slot in self.slots {
            match slot {
                Slot::Spacer => {
                    drawn.push(div().flex_1().into_any_element());
                    previous_group = false;
                }
                Slot::Group { id, items } => {
                    let group = ident.child(id.as_ref());
                    let mut inline: Vec<AnyElement> = Vec::new();
                    for item in items {
                        if index >= cut {
                            overflowed.push(item.menu_row());
                        } else {
                            inline.push(item.content);
                        }
                        index += 1;
                    }
                    if inline.is_empty() {
                        continue;
                    }
                    if previous_group {
                        drawn.push(rule(&group, &theme, &ident, cx));
                    }
                    drawn.push(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(theme.space(Space::Xs)))
                            .children(inline)
                            .semantic_in(
                                cx,
                                NodeSpec::new(group.semantic_id(), Role::Group)
                                    .parent(ident.semantic_id()),
                            )
                            .into_any_element(),
                    );
                    previous_group = true;
                }
            }
        }

        let hidden = overflowed.len();
        let overflow = self.overflow_menu.filter(|_| hidden > 0).map(|menu| {
            let rows = overflowed;
            if menu.read(cx).offered() != rows.as_slice() {
                menu.update(cx, |menu, cx| menu.set_items(rows, cx));
            }
            menu.clone()
        });
        let overflow_ident = ident.child("overflow");

        div()
            .id(ident.element_id())
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .gap(px(theme.space(Space::Sm)))
            .px(px(theme.space(Space::Sm)))
            .py(px(theme.space(Space::Xs)))
            .bg(theme.colors.panel)
            .hairline(&theme)
            .children(drawn)
            .children(overflow.map(|menu| {
                div()
                    .flex()
                    .flex_none()
                    .child(menu)
                    // The trigger says how many actions moved here, so a
                    // snapshot shows that they were relocated and not lost.
                    .semantic_in(
                        cx,
                        NodeSpec::new(overflow_ident.semantic_id(), Role::Group)
                            .parent(ident.semantic_id())
                            .text("More actions")
                            .value(hidden.to_string()),
                    )
            }))
            .semantic_in(cx, {
                let mut spec =
                    NodeSpec::new(ident.semantic_id(), Role::Toolbar).value(total.to_string());
                if let Some(label) = self.label {
                    spec = spec.text(label);
                }
                spec
            })
    }
}

fn rule(group: &Ident, theme: &Theme, toolbar: &Ident, cx: &App) -> AnyElement {
    div()
        .flex_none()
        .w(px(theme.borders.hairline))
        .h(px(theme.control.get(ControlSize::Sm).height))
        .bg(theme.colors.hairline)
        .semantic_in(
            cx,
            NodeSpec::new(group.child("rule").semantic_id(), Role::Separator)
                .parent(toolbar.semantic_id()),
        )
        .into_any_element()
}
