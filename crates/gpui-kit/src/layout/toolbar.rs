//! A horizontal bar of grouped actions, with the rest behind an overflow menu.
//!
//! # Where the cut comes from
//!
//! A truthful overflow needs to know how wide every item is before it decides
//! which ones fit, and GPUI produces that during layout, after the element
//! tree has been built. A builder cannot measure a child and then still move
//! it into a menu, because a child is an `AnyElement` and is consumed once.
//!
//! So the bar measures the frame it drew and cuts the next one. Each item
//! records its own width under its business id, the bar records its own, and
//! the widths survive the rebuild because they are keyed by identity rather
//! than held in the builder. The first frame draws every item — that is what
//! produces the measurements — and the measurement asks for the frame that
//! uses them, so the settled state is the next frame rather than the next
//! interaction. The arithmetic runs over remembered widths rather than
//! current ones, so an item that moved into the menu cannot make room for
//! itself and start oscillating.
//!
//! [`Toolbar::overflow_after`] still exists and still wins, for the caller who
//! knows the answer and does not want to spend a frame arriving at it.
//!
//! Either way the bar guarantees the part that matters: an item past the cut
//! is **moved**, never dropped. It becomes a row in the overflow [`Menu`],
//! keeping its identity, its label, and its refusal, and the trigger publishes
//! how many items went there. With no menu to move them into, every item is
//! drawn inline, because losing an action is never the better failure.

use gpui::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Elevation, Space, Surface, Theme};

use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{Ident, Sizable, StyledExt};
use crate::layout::measure;
use crate::overlay::{Menu, MenuItem};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

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
    /// overflow menu, instead of the measured cut.
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

    /// Every item id in bar order, which is the order the cut walks.
    fn item_ids(&self) -> Vec<SharedString> {
        self.slots
            .iter()
            .filter_map(|slot| match slot {
                Slot::Group { items, .. } => Some(items.iter().map(|item| item.id.clone())),
                Slot::Spacer => None,
            })
            .flatten()
            .collect()
    }

    /// Where the cut falls, which is nowhere when there is no menu to move
    /// anything into.
    fn cut(&self, theme: &Theme, window: &Window, cx: &mut App) -> usize {
        if self.overflow_menu.is_none() {
            return usize::MAX;
        }
        if let Some(declared) = self.overflow_after {
            return declared;
        }
        self.measured_cut(theme, window, cx).unwrap_or(usize::MAX)
    }

    /// The largest number of items whose remembered widths fit the remembered
    /// width of the bar, or `None` while anything it needs is still unknown.
    fn measured_cut(&self, theme: &Theme, window: &Window, cx: &mut App) -> Option<usize> {
        let bar = f32::from(
            measure::cell(&self.ident.semantic_id(), window, cx)
                .get()
                .size
                .width,
        );
        if bar <= 0.0 {
            return None;
        }

        let ids = self.item_ids();
        let mut widths = Vec::with_capacity(ids.len());
        for id in &ids {
            let width = f32::from(
                measure::cell(&self.ident.child(id.as_ref()).semantic_id(), window, cx)
                    .get()
                    .size
                    .width,
            );
            // An item nobody has drawn yet has no width, and a cut computed
            // without it would hide the item that would have supplied it.
            if width <= 0.0 {
                return None;
            }
            widths.push(width);
        }

        let trigger = f32::from(
            measure::cell(&self.ident.child("overflow").semantic_id(), window, cx)
                .get()
                .size
                .width,
        );
        // Before the trigger has ever been drawn, reserve a control's worth of
        // room for it. Overestimating the reservation cuts one item early for
        // one frame; underestimating it draws a bar that does not fit.
        let trigger = if trigger > 0.0 {
            trigger
        } else {
            theme.control.get(self.size).height
        };

        let gap = theme.space(Space::Sm);
        let inner_gap = theme.space(Space::Xs);
        // The measured row already sits inside the bar's padding.
        let available = bar;

        let group_sizes = self.group_sizes();
        let mut taken = 0.0;
        let mut drawn = 0usize;
        let mut index = 0usize;
        for (group, count) in group_sizes.iter().copied().enumerate() {
            for position in 0..count {
                let width = widths[index];
                let leading = if index == 0 {
                    0.0
                } else if position == 0 {
                    // A new group brings the rule before it and the gap on
                    // either side of it.
                    2.0 * gap + theme.borders.hairline
                } else {
                    inner_gap
                };
                let rest = index + 1 < widths.len();
                let reserved = if rest { gap + trigger } else { 0.0 };
                if taken + leading + width + reserved > available {
                    return Some(drawn);
                }
                taken += leading + width;
                drawn += 1;
                index += 1;
            }
            let _ = group;
        }
        Some(drawn)
    }

    /// How many items each group holds, in bar order.
    fn group_sizes(&self) -> Vec<usize> {
        self.slots
            .iter()
            .filter_map(|slot| match slot {
                Slot::Group { items, .. } => Some(items.len()),
                Slot::Spacer => None,
            })
            .collect()
    }
}

impl Sizable for Toolbar {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Toolbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let total = self.item_count();
        let cut = self.cut(&theme, window, cx);
        let ident = self.ident.clone();
        let bar_extent = measure::cell(&ident.semantic_id(), window, cx);
        let trigger_extent = measure::cell(&ident.child("overflow").semantic_id(), window, cx);

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
                            // Each item records its own width under its
                            // business id, which is what the next frame's cut
                            // is computed from.
                            let extent = measure::cell(
                                &ident.child(item.id.as_ref()).semantic_id(),
                                window,
                                cx,
                            );
                            inline.push(
                                div()
                                    .flex()
                                    .flex_none()
                                    .on_children_prepainted(move |bounds, window, _| {
                                        if let Some(first) = bounds.first() {
                                            measure::record(&extent, *first, window);
                                        }
                                    })
                                    .child(item.content)
                                    .into_any_element(),
                            );
                        }
                        index += 1;
                    }
                    if inline.is_empty() {
                        continue;
                    }
                    if previous_group {
                        drawn.push(group_gap(&group, &theme, &ident, cx));
                    }
                    drawn.push(
                        div()
                            .row_reading(direction)
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

        // The row is a full-width child of the bar rather than the bar itself,
        // so what gets measured is the room the items were given. Reading the
        // items' own union instead would report what they took, which is the
        // number that is already too large when they do not fit.
        let row = div()
            .row_reading(direction)
            .items_center()
            .w_full()
            .gap(px(theme.space(Space::Sm)))
            .children(drawn)
            .children(overflow.map(|menu| {
                div()
                    .flex()
                    .flex_none()
                    .on_children_prepainted(move |bounds, window, _| {
                        if let Some(first) = bounds.first() {
                            measure::record(&trigger_extent, *first, window);
                        }
                    })
                    .child(menu)
                    // The trigger says how many actions moved here, so a
                    // snapshot shows that they were relocated and not lost.
                    .semantic_in(
                        cx,
                        NodeSpec::new(overflow_ident.semantic_id(), Role::Group)
                            .parent(ident.semantic_id())
                            .text(cx.strings().text(StringKey::MoreActions))
                            .value(cx.numbers().count(hidden)),
                    )
            }));

        div()
            .on_children_prepainted(move |bounds, window, _| {
                if let Some(first) = bounds.first() {
                    measure::record(&bar_extent, *first, window);
                }
            })
            .id(ident.element_id())
            .w_full()
            .px(px(theme.space(Space::Sm)))
            .py(px(theme.space(Space::Xs)))
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .child(row)
            .semantic_in(cx, {
                let mut spec = NodeSpec::new(ident.semantic_id(), Role::Toolbar)
                    .value(cx.numbers().count(total));
                if let Some(label) = self.label {
                    spec = spec.text(label);
                }
                spec
            })
    }
}

fn group_gap(group: &Ident, theme: &Theme, toolbar: &Ident, cx: &App) -> AnyElement {
    div()
        .flex_none()
        .w(px(theme.space(Space::Xs)))
        .semantic_in(
            cx,
            NodeSpec::new(group.child("gap").semantic_id(), Role::Separator)
                .parent(toolbar.semantic_id()),
        )
        .into_any_element()
}
