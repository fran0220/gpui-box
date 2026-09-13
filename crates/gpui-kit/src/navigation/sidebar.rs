//! A navigation rail of sectioned places, expanded or collapsed to icons.
//!
//! Where the typist is, is caller-owned. The sidebar reports the place that was
//! picked and marks whatever the caller says is current, so a host that refuses
//! a move keeps the place that still holds highlighted.
//!
//! Expanded branches draw nested links; a collapsed rail shows top-level
//! glyphs and opens their destinations in an anchored, keyboard-accessible
//! flyout. Tooltip names and status dots survive the narrow drawing.
//!
//! The host supplies a bounded height. Header and footer stay fixed while the
//! navigation body scrolls. Tab enters one remembered row; arrows move focus,
//! not selection. Logical left/right disclose branches, Enter/Space navigate,
//! and Escape closes the flyout and restores its trigger. Item ids must be
//! unique across every section and descendant.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    AnyElement, App, FocusHandle, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, TextTone, Theme, TypeScale};

use crate::display::badge::Badge;
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{
    Disableable, FocusRing, Hoverable, Ident, Pressable, SelectedFill, Sizable, StyledExt, rule,
    text as foundation_text, window_state,
};
use crate::layout::ScrollArea;
use crate::layout::scroll::scroll_handle;
use crate::motion::{Flipping, flip};
use crate::overlay::{FocusTrap, Hang, Overlay, OverlaySurface, Placement, Tooltipped, surface};
use crate::strings::{ActiveStrings, StringKey};

/// How wide the rail is expanded, and how wide it is collapsed to glyphs.
/// Neither value repeats anywhere else.
const EXPANDED_WIDTH: f32 = 240.0;
const COLLAPSED_WIDTH: f32 = 52.0;

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(SharedString, bool, &mut Window, &mut App)>;
type CollapseHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// Visual state is window- and mount-scoped, never a second routing authority.
#[derive(Default)]
struct SidebarState {
    closed: HashSet<SharedString>,
    focus: HashMap<SharedString, FocusHandle>,
    current: Option<SharedString>,
    flyout: Option<SharedString>,
    pending_focus: bool,
    trap: FocusTrap,
}

type State = Rc<RefCell<SidebarState>>;

#[derive(Clone)]
struct Entry {
    item: SidebarItem,
    ident: Ident,
    parent: Option<SharedString>,
    level: u32,
    open: bool,
}

/// One visible navigation surface. The rail and its flyout have separate
/// keyboard orders and scroll owners, but use the same destination model.
#[derive(Clone)]
struct Frame {
    entries: Rc<Vec<Entry>>,
    state: State,
    scroll: ScrollHandle,
    compact: bool,
    flyout: bool,
    controlled: bool,
    on_select: Option<SelectHandler>,
    on_toggle: Option<ToggleHandler>,
}

impl Frame {
    fn actionable(&self, entry: &Entry) -> bool {
        !entry.item.disabled
            && (self.on_select.is_some()
                || (!entry.item.children.is_empty()
                    && (self.compact || !self.controlled || self.on_toggle.is_some())))
    }

    fn focus(&self, index: usize, window: &mut Window, cx: &mut App) {
        let id = self.entries[index].ident.semantic_id();
        let handle = {
            let mut state = self.state.borrow_mut();
            if !self.flyout {
                state.current = Some(id.clone());
            }
            state.focus[&id].clone()
        };
        handle.focus(window, cx);
        window.refresh();
    }

    fn toggle(&self, index: usize, window: &mut Window, cx: &mut App) {
        let entry = &self.entries[index];
        if entry.item.disabled || entry.item.children.is_empty() {
            return;
        }
        if !self.controlled {
            let mut state = self.state.borrow_mut();
            if entry.open {
                state.closed.insert(entry.item.id.clone());
            } else {
                state.closed.remove(&entry.item.id);
            }
        }
        if let Some(handler) = &self.on_toggle {
            handler(entry.item.id.clone(), !entry.open, window, cx);
        }
        window.refresh();
    }

    fn activate(&self, index: usize, window: &mut Window, cx: &mut App) {
        let entry = &self.entries[index];
        if !self.actionable(entry) {
            return;
        }
        self.focus(index, window, cx);
        if self.compact && !entry.item.children.is_empty() {
            let mut state = self.state.borrow_mut();
            state.trap.engage(window, cx);
            state.flyout = Some(entry.item.id.clone());
            state.pending_focus = true;
        } else if let Some(handler) = &self.on_select {
            if self.flyout {
                close_flyout(&self.state, window, cx);
            }
            handler(entry.item.id.clone(), window, cx);
        } else {
            self.toggle(index, window, cx);
        }
        window.refresh();
    }

    fn key(&self, index: usize, key: &str, window: &mut Window, cx: &mut App) -> bool {
        let entry = &self.entries[index];
        let enabled = |index: &usize| self.actionable(&self.entries[*index]);
        let next = match key {
            "up" => (0..index).rev().find(enabled),
            "down" => (index + 1..self.entries.len()).find(enabled),
            "home" => (0..self.entries.len()).find(enabled),
            "end" => (0..self.entries.len()).rev().find(enabled),
            "enter" | "space" => {
                self.activate(index, window, cx);
                return true;
            }
            _ => match cx.layout_direction().arrow_step(key) {
                Some(1) if !entry.item.children.is_empty() => {
                    if self.compact {
                        self.activate(index, window, cx);
                        return true;
                    }
                    if !entry.open {
                        self.toggle(index, window, cx);
                        return true;
                    }
                    (index + 1..self.entries.len()).find(|i| {
                        self.entries[*i].parent.as_ref() == Some(&entry.ident.semantic_id())
                            && enabled(i)
                    })
                }
                Some(-1) => {
                    if !self.compact && entry.open && !entry.item.children.is_empty() {
                        self.toggle(index, window, cx);
                        return true;
                    }
                    self.entries.iter().position(|candidate| {
                        Some(&candidate.ident.semantic_id()) == entry.parent.as_ref()
                            && self.actionable(candidate)
                    })
                }
                _ => return false,
            },
        };
        if let Some(next) = next {
            self.focus(next, window, cx);
        }
        true
    }
}

fn close_flyout(state: &State, window: &mut Window, cx: &mut App) {
    let mut state = state.borrow_mut();
    state.flyout = None;
    state.pending_focus = false;
    state.trap.release(window, cx);
    window.refresh();
}

fn contains(item: &SidebarItem, active: &Option<SharedString>) -> bool {
    active.as_ref() == Some(&item.id) || item.children.iter().any(|child| contains(child, active))
}

/// What sits in the glyph slot: a catalog mark, or a caller-owned image.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SidebarLeading {
    Glyph(Icon),
    Image(SharedString),
}

/// One place in the rail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarItem {
    id: SharedString,
    label: SharedString,
    leading: Option<SidebarLeading>,
    badge: Option<SharedString>,
    disabled: bool,
    children: Vec<SidebarItem>,
}

impl SidebarItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            leading: None,
            badge: None,
            disabled: false,
            children: Vec::new(),
        }
    }

    pub fn icon(mut self, glyph: Icon) -> Self {
        self.leading = Some(SidebarLeading::Glyph(glyph));
        self
    }

    /// A resource path or URI the asset source can resolve, same as [`Avatar::image`](crate::display::avatar::Avatar::image).
    pub fn image(mut self, path: impl Into<SharedString>) -> Self {
        self.leading = Some(SidebarLeading::Image(path.into()));
        self
    }

    /// A count or a state shown next to the label, such as how many runs a
    /// place holds.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Nested destinations. Every descendant is retained; use `Tree` instead
    /// for resource browsing rather than page navigation.
    pub fn children(mut self, children: impl IntoIterator<Item = SidebarItem>) -> Self {
        self.children = children.into_iter().collect();
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }
}

/// A titled run of places.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarSection {
    id: SharedString,
    title: Option<SharedString>,
    items: Vec<SidebarItem>,
}

impl SidebarSection {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: None,
            items: Vec::new(),
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn item(mut self, item: SidebarItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = SidebarItem>) -> Self {
        self.items.extend(items);
        self
    }
}

/// A collapsible navigation rail.
#[derive(IntoElement)]
pub struct Sidebar {
    ident: Ident,
    sections: Vec<SidebarSection>,
    active: Option<SharedString>,
    collapsed: bool,
    width: Option<f32>,
    fit_height: bool,
    expanded: Option<HashSet<SharedString>>,
    header: Option<AnyElement>,
    footer: Option<AnyElement>,
    disabled: bool,
    size: ControlSize,
    on_select: Option<SelectHandler>,
    on_toggle: Option<ToggleHandler>,
    on_collapse: Option<CollapseHandler>,
}

impl std::fmt::Debug for Sidebar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Sidebar")
            .field("ident", &self.ident)
            .field("sections", &self.sections.len())
            .field("active", &self.active)
            .field("collapsed", &self.collapsed)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_select.is_some())
            .finish()
    }
}

impl Sidebar {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            sections: Vec::new(),
            active: None,
            collapsed: false,
            width: Some(EXPANDED_WIDTH),
            fit_height: false,
            expanded: None,
            header: None,
            footer: None,
            disabled: false,
            size: ControlSize::Md,
            on_select: None,
            on_toggle: None,
            on_collapse: None,
        }
    }

    pub fn section(mut self, section: SidebarSection) -> Self {
        self.sections.push(section);
        self
    }

    pub fn sections(mut self, sections: impl IntoIterator<Item = SidebarSection>) -> Self {
        self.sections.extend(sections);
        self
    }

    /// The place the caller says is current. The sidebar marks it and never
    /// moves it.
    pub fn active(mut self, id: impl Into<SharedString>) -> Self {
        self.active = Some(id.into());
        self
    }

    /// Draws glyphs only, with each label reachable as hover help.
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// Expanded width in logical pixels. Collapsed width remains 52px.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Uses the parent's allocation while expanded, for example a `SplitPane`.
    /// The host owns resizing and persistence; the sidebar does not store them.
    pub fn fill_width(mut self) -> Self {
        self.width = None;
        self
    }

    /// Sizes to its destinations instead of filling a bounded pane. Use this
    /// in content-sized layouts, such as a settings page's sidebar slot.
    /// In this mode the enclosing page, not the rail, bounds vertical space.
    pub fn fit_height(mut self) -> Self {
        self.fit_height = true;
        self
    }

    /// Controls branch expansion. Without this option branches start open and
    /// their expansion is transient local state. With it, toggles only report
    /// requests and a refused request leaves the branch unchanged.
    pub fn expanded_ids(mut self, ids: &[&str]) -> Self {
        self.expanded = Some(ids.iter().map(|id| SharedString::from(*id)).collect());
        self
    }

    /// Reports a branch expansion request, independently of page selection.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Adds a collapse/expand control. This only requests a new `collapsed`
    /// value; the host decides whether to accept it.
    pub fn on_collapse(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_collapse = Some(Rc::new(handler));
        self
    }

    pub fn header(mut self, header: impl IntoElement) -> Self {
        self.header = Some(header.into_any_element());
        self
    }

    /// The slot at the bottom of the rail, kept in both widths.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    fn item_element(
        &self,
        index: usize,
        frame: &Frame,
        theme: &Theme,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let entry = &frame.entries[index];
        let item = &entry.item;
        let metrics = theme.control.get(self.size);
        let direction = cx.layout_direction();
        let ident = &entry.ident;
        let active = self.active.as_ref() == Some(&item.id);
        let marked = active || ((frame.compact || !entry.open) && contains(item, &self.active));
        let disabled = item.disabled;
        let actionable = frame.actionable(entry);
        // Explicit focus handles own their tab policy in GPUI; div.tab_stop
        // only configures automatically created handles.
        let focus = frame.state.borrow().focus[&ident.semantic_id()]
            .clone()
            .tab_index(0)
            .tab_stop(
                actionable
                    && (frame.flyout
                        || frame.state.borrow().current.as_ref() == Some(&ident.semantic_id())),
            );
        // The wash says the row is the one being read; the accent says it is
        // the one that is current. A collapsed rail has only the glyph left
        // to say it with, which is why the colour and not the fill carries
        // the statement.
        let color = if disabled {
            theme.colors.text_faint
        } else if marked {
            theme.colors.accent
        } else if frame.compact {
            // Collapsed, the glyph is the whole row. Secondary text tone is
            // sized for something a label is standing next to, and a rail of
            // glyphs at that weight is a rail nobody can read.
            theme.colors.text
        } else {
            theme.colors.text_muted
        };
        let indent = if frame.compact {
            0.0
        } else {
            theme.space(Space::Lg) * entry.level.saturating_sub(1) as f32
        };
        let glyph_slot = flip(ident.child("glyph").semantic_id(), window, cx);

        let mut row = div()
            .id(ident.element_id())
            .row_reading(direction)
            .items_center()
            .h(px(metrics.height))
            .flex_none()
            .w_full()
            .min_w(px(0.0))
            .gap(px(theme.space(Space::Sm)))
            .ps(
                direction,
                px(if frame.compact {
                    0.0
                } else {
                    theme.space(Space::Sm) + indent
                }),
            )
            .pe(
                direction,
                px(if frame.compact {
                    0.0
                } else {
                    theme.space(Space::Sm)
                }),
            )
            .radius(theme, Radius::Control)
            .when(frame.compact, |element| element.justify_center())
            .selected_fill(theme, marked)
            .when(disabled, |element| element.opacity(theme.opacity.disabled))
            .child(
                // The glyph is the one thing that survives collapsing, so it
                // travels to its narrow position rather than being redrawn
                // there.
                div()
                    .flex()
                    .flex_none()
                    .w(px(metrics.icon_size))
                    .justify_center()
                    .child({
                        let fallback = SidebarLeading::Glyph(Icon::Document);
                        let leading = item.leading.as_ref().unwrap_or(&fallback);
                        match leading {
                            SidebarLeading::Glyph(glyph) => icon(*glyph)
                                .size(px(metrics.icon_size))
                                .text_color(color)
                                .into_any_element(),
                            SidebarLeading::Image(source) => gpui::img(source.clone())
                                .size(px(metrics.icon_size))
                                .into_any_element(),
                        }
                    })
                    .flip(&glyph_slot, window, cx),
            )
            .when(!frame.compact, |element| {
                element
                    .child(
                        div().flex_1().min_w(px(0.0)).overflow_hidden().child(
                            foundation_text(theme, TypeScale::Label, item.label.clone())
                                .truncate()
                                .text_size(px(metrics.font_size))
                                .text_color(color),
                        ),
                    )
                    .children(item.badge.clone().map(|badge| Badge::new(badge).neutral()))
            })
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .track_focus(&focus)
                    // Include the content padding: revealing only the row
                    // leaves a trailing sliver scrollable and its edge fade
                    // over the last label (and clips the focus ring).
                    .reveal_on_focus(&frame.scroll, px(theme.space(Space::Xs)))
                    .pressable(cx)
                    .when(!marked, |element| element.hover_row(theme))
                    .focus_ring(theme)
            });

        // Full labels remain available for both icon-only and truncated rows.
        row = row.tip(
            ident.clone(),
            item.badge.as_ref().map_or_else(
                || item.label.clone(),
                |badge| {
                    cx.strings().format(
                        StringKey::SidebarItemWithBadge,
                        &[item.label.as_ref(), badge.as_ref()],
                    )
                },
            ),
        );

        if actionable {
            let click = frame.clone();
            let keys = frame.clone();
            row = row
                .on_click(move |_, window, cx| click.activate(index, window, cx))
                .on_key_down(move |event, window, cx| {
                    if keys.key(index, event.keystroke.key.as_str(), window, cx) {
                        cx.stop_propagation();
                    }
                });
        }

        let branch = !item.children.is_empty();
        if !frame.compact && branch {
            let toggle_id = ident.child("toggle");
            let label = cx.strings().text(if entry.open {
                StringKey::Collapse
            } else {
                StringKey::Expand
            });
            let toggleable = !disabled && (!frame.controlled || frame.on_toggle.is_some());
            let action = frame.clone();
            row = row.child(
                div()
                    .id(toggle_id.element_id())
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(metrics.height - theme.space(Space::Sm)))
                    .radius(theme, Radius::Control)
                    .child(
                        icon(if entry.open {
                            Icon::AltArrowDown
                        } else if direction.is_rtl() {
                            Icon::AltArrowLeft
                        } else {
                            Icon::AltArrowRight
                        })
                        .size(px(metrics.icon_size))
                        .text_color(color),
                    )
                    .when(toggleable, |button| {
                        button
                            .cursor_pointer()
                            .hover_row(theme)
                            .on_click(move |_, window, cx| {
                                action.focus(index, window, cx);
                                action.toggle(index, window, cx);
                                cx.stop_propagation();
                            })
                    })
                    .tip(toggle_id.clone(), label.clone())
                    .semantic_in(
                        cx,
                        NodeSpec::new(toggle_id.semantic_id(), Role::Button)
                            .parent(ident.semantic_id())
                            .text(label)
                            .expanded(entry.open)
                            .disabled(!toggleable),
                    ),
            );
        }
        if frame.compact && item.badge.is_some() {
            // A dot preserves the presence of a status without squeezing an
            // arbitrary count into a glyph. The semantic value keeps the count.
            row = row.relative().child(
                div()
                    .absolute()
                    .top(px(theme.space(Space::Xs)))
                    .right(px(theme.space(Space::Xs)))
                    .size(px(theme.space(Space::Xs)))
                    .rounded_full()
                    .bg(color),
            );
        }

        let mut spec = NodeSpec::new(
            ident.semantic_id(),
            if frame.compact && branch {
                Role::Button
            } else {
                Role::Link
            },
        )
        .parent(
            entry
                .parent
                .clone()
                .unwrap_or_else(|| self.ident.semantic_id()),
        )
        .selected(active)
        .disabled(disabled)
        .level(entry.level)
        // A collapsed rail draws a glyph and still says what it is.
        .text(item.label.clone());
        if let Some(badge) = item.badge.clone() {
            spec = spec.value(badge);
        }
        if branch {
            spec = spec.expanded(if frame.compact {
                frame.state.borrow().flyout.as_ref() == Some(&item.id)
            } else {
                entry.open
            });
        }
        if actionable {
            spec = spec.focus(&focus);
        }

        row.semantic_in(cx, spec).into_any_element()
    }

    fn append(
        &self,
        item: &SidebarItem,
        parent: Option<&Entry>,
        state: &SidebarState,
        compact: bool,
        entries: &mut Vec<Entry>,
    ) {
        let mut item = item.clone();
        item.disabled |= self.disabled || parent.is_some_and(|parent| parent.item.disabled);
        let open = self.expanded.as_ref().map_or_else(
            || !state.closed.contains(&item.id),
            |expanded| expanded.contains(&item.id),
        );
        let entry = Entry {
            ident: self.ident.child(item.id.as_ref()),
            item,
            parent: parent.map(|parent| parent.ident.semantic_id()),
            level: parent.map_or(1, |parent| parent.level + 1),
            open,
        };
        entries.push(entry.clone());
        if open && !compact {
            for child in &entry.item.children {
                self.append(child, Some(&entry), state, compact, entries);
            }
        }
    }

    fn prepare(&self, frame: &Frame, window: &mut Window, cx: &mut App) {
        let mut state = frame.state.borrow_mut();
        for entry in frame.entries.iter() {
            state
                .focus
                .entry(entry.ident.semantic_id())
                .or_insert_with(|| cx.focus_handle());
        }
        if frame.flyout {
            state.trap.begin_frame();
            for entry in frame.entries.iter().filter(|entry| frame.actionable(entry)) {
                let handle = state.focus[&entry.ident.semantic_id()].clone();
                state.trap.register(handle);
            }
            if state.pending_focus {
                state.pending_focus = false;
                let active = frame.entries.iter().find(|entry| {
                    frame.actionable(entry) && self.active.as_ref() == Some(&entry.item.id)
                });
                if let Some(entry) = active {
                    state.focus[&entry.ident.semantic_id()].focus(window, cx);
                } else {
                    state.trap.focus_first(window, cx);
                }
            }
        } else {
            let current_valid = frame.entries.iter().any(|entry| {
                frame.actionable(entry)
                    && state.current.as_ref() == Some(&entry.ident.semantic_id())
            });
            if !current_valid {
                let restore = state
                    .current
                    .as_ref()
                    .and_then(|id| state.focus.get(id))
                    .is_some_and(|handle| handle.is_focused(window));
                let target = frame
                    .entries
                    .iter()
                    .filter(|entry| frame.actionable(entry))
                    .find(|entry| self.active.as_ref() == Some(&entry.item.id))
                    .or_else(|| {
                        frame.entries.iter().rev().find(|entry| {
                            frame.actionable(entry) && contains(&entry.item, &self.active)
                        })
                    })
                    .or_else(|| frame.entries.iter().find(|entry| frame.actionable(entry)));
                state.current = target.map(|entry| entry.ident.semantic_id());
                if restore && let Some(entry) = target {
                    state.focus[&entry.ident.semantic_id()].focus(window, cx);
                }
            }
        }
    }

    fn flyout(
        &self,
        entry: &Entry,
        rail: &Frame,
        theme: &Theme,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let ident = entry.ident.child("flyout");
        // The parent is also a destination. Keep that action separate from the
        // icon trigger, which opens the flyout rather than navigating.
        let mut destination = entry.clone();
        destination.ident = entry.ident.child("destination");
        destination.parent = Some(ident.semantic_id());
        destination.item.children.clear();
        let mut entries = vec![destination.clone()];
        for child in &entry.item.children {
            self.append(
                child,
                Some(&destination),
                &rail.state.borrow(),
                false,
                &mut entries,
            );
        }
        let frame = Frame {
            entries: Rc::new(entries),
            scroll: scroll_handle(&ident.child("scroll"), window, cx),
            compact: false,
            flyout: true,
            ..rail.clone()
        };
        self.prepare(&frame, window, cx);
        let rows = (0..frame.entries.len())
            .map(|index| self.item_element(index, &frame, theme, window, cx))
            .collect::<Vec<_>>();
        let outside = rail.state.clone();
        let keys = rail.state.clone();
        let fallback_focus = {
            let mut state = rail.state.borrow_mut();
            let handle = state
                .focus
                .entry(ident.semantic_id())
                .or_insert_with(|| cx.focus_handle())
                .clone();
            if state.trap.stops().is_empty() {
                state.trap.register(handle.clone());
                handle.focus(window, cx);
            }
            handle
        };
        let content = div()
            .id(ident.element_id())
            .tab_group()
            .track_focus(&fallback_focus)
            .tab_stop(false)
            .on_mouse_down_out(move |_, window, cx| close_flyout(&outside, window, cx))
            .on_key_down(move |event, window, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => close_flyout(&keys, window, cx),
                    "tab" => {
                        let state = keys.borrow();
                        if event.keystroke.modifiers.shift {
                            state.trap.focus_prev(window, cx);
                        } else {
                            state.trap.focus_next(window, cx);
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
            })
            .child(
                surface(ident.child("surface"), theme, OverlaySurface::FLOATING)
                    .w(px(self.width.unwrap_or(EXPANDED_WIDTH)))
                    .p(px(theme.space(Space::Sm)))
                    .child(
                        ScrollArea::new(ident.child("scroll"))
                            .height(
                                (frame.entries.len() as f32
                                    * (theme.control.get(self.size).height
                                        + theme.space(Space::Xs))
                                    + theme.space(Space::Xs))
                                .min(f32::from(window.viewport_size().height) * 0.6),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .p(px(theme.space(Space::Xs)))
                                    .gap(px(theme.space(Space::Xs)))
                                    .children(rows),
                            ),
                    ),
            )
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::List)
                    .parent(entry.ident.semantic_id())
                    .text(entry.item.label.clone())
                    .expanded(true),
            );
        Overlay::new(ident.child("overlay"))
            .placement(Placement::Below)
            .hang(if cx.layout_direction().is_rtl() {
                Hang::End
            } else {
                Hang::Start
            })
            .child(content)
            .into_any_element()
    }
}

impl Disableable for Sidebar {
    /// Freezes the whole rail. A frozen rail installs no handler at all.
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Sidebar {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state = window_state::with_key(
            &self.ident.semantic_id(),
            window.window_handle().window_id(),
            cx,
            |state: &mut State| state.clone(),
        );
        let mut entries = Vec::new();
        let mut ranges = Vec::new();
        for section in &self.sections {
            let start = entries.len();
            for item in &section.items {
                self.append(item, None, &state.borrow(), self.collapsed, &mut entries);
            }
            ranges.push(start..entries.len());
        }
        let open = state.borrow().flyout.clone();
        if open.is_some()
            && (!self.collapsed
                || !entries.iter().any(|entry| {
                    !entry.item.disabled
                        && !entry.item.children.is_empty()
                        && Some(&entry.item.id) == open.as_ref()
                }))
        {
            close_flyout(&state, window, cx);
        }
        let frame = Frame {
            entries: Rc::new(entries),
            state,
            scroll: scroll_handle(&self.ident.child("scroll"), window, cx),
            compact: self.collapsed,
            flyout: false,
            controlled: self.expanded.is_some(),
            on_select: self.on_select.clone(),
            on_toggle: self.on_toggle.clone(),
        };
        self.prepare(&frame, window, cx);
        let mut body = div()
            .flex()
            .flex_col()
            .when(!self.collapsed, |body| body.p(px(theme.space(Space::Xs))))
            .gap(px(theme.space(Space::Md)));

        for (index, section) in self.sections.iter().enumerate() {
            let section_ident = self.ident.child(section.id.as_ref());
            let mut rows: Vec<AnyElement> = Vec::new();
            for row_index in ranges[index].clone() {
                let entry = &frame.entries[row_index];
                let row = self.item_element(row_index, &frame, &theme, window, cx);
                if self.collapsed && frame.state.borrow().flyout.as_ref() == Some(&entry.item.id) {
                    let overlay = self.flyout(entry, &frame, &theme, window, cx);
                    // The zero-size anchor sits at the row's logical trailing
                    // edge. GPUI's overlay owns clipping escape and viewport fit.
                    rows.push(
                        div()
                            .row_reading(cx.layout_direction())
                            .w_full()
                            .child(row)
                            .child(div().flex_none().size(px(0.0)).child(overlay))
                            .into_any_element(),
                    );
                } else {
                    rows.push(row);
                }
            }

            // A collapsed rail has no room for a caption, so the run is
            // separated by a rule instead of titled.
            let heading = section
                .title
                .clone()
                .filter(|_| !self.collapsed)
                .map(|title| {
                    foundation_text(&theme, TypeScale::Caption, title.clone())
                        .px(px(theme.space(Space::Sm)))
                        .text_tone(&theme, TextTone::Faint)
                        .semantic_in(
                            cx,
                            NodeSpec::new(section_ident.semantic_id(), Role::Heading)
                                .parent(self.ident.semantic_id())
                                .level(1)
                                .text(title),
                        )
                });

            // Collapsed, the rule is the only thing left that says one run of
            // glyphs ended and another began. A gap on its own is a gap.
            if self.collapsed && index > 0 {
                body = body.child(rule(&theme));
            }
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap(px(theme.space(Space::Xs)))
                    .children(heading)
                    .children(rows),
            );
        }

        // A mounted rail may receive entirely new caller-owned destinations.
        // Keep hidden descendants' focus/expansion, but not removed identities.
        let mut pending = self
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .collect::<Vec<_>>();
        let mut item_ids = HashSet::new();
        let mut focus_ids = HashSet::new();
        while let Some(item) = pending.pop() {
            item_ids.insert(item.id.clone());
            let ident = self.ident.child(item.id.as_ref());
            focus_ids.insert(ident.semantic_id());
            focus_ids.insert(ident.child("destination").semantic_id());
            focus_ids.insert(ident.child("flyout").semantic_id());
            pending.extend(&item.children);
        }
        {
            let mut state = frame.state.borrow_mut();
            state.closed.retain(|id| item_ids.contains(id));
            state.focus.retain(|id, _| focus_ids.contains(id));
        }

        let collapse = self.on_collapse.clone().map(|handler| {
            let collapsed = self.collapsed;
            crate::controls::button::Button::new(self.ident.child("collapse"))
                .icon_only(
                    if collapsed != cx.layout_direction().is_rtl() {
                        Icon::AltArrowRight
                    } else {
                        Icon::AltArrowLeft
                    },
                    cx.strings().text(if collapsed {
                        StringKey::Expand
                    } else {
                        StringKey::Collapse
                    }),
                )
                .ghost()
                .disabled(self.disabled)
                .on_click(move |window, cx| handler(!collapsed, window, cx))
        });
        let header = (self.header.is_some() || collapse.is_some()).then(|| {
            div()
                .row_reading(cx.layout_direction())
                .items_center()
                .flex_none()
                .gap(px(theme.space(Space::Sm)))
                .min_w(px(0.0))
                .when(self.collapsed, |header| header.flex_col())
                .children(self.header.map(|header| {
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .child(header)
                }))
                .children(collapse)
        });

        div()
            .id(self.ident.element_id())
            .flex()
            .flex_col()
            .flex_none()
            .when(!self.fit_height, |element| element.h_full())
            .min_h(px(0.0))
            .min_w(px(0.0))
            .when(self.collapsed, |element| element.w(px(COLLAPSED_WIDTH)))
            .when(!self.collapsed, |element| match self.width {
                Some(width) => element.w(px(width)),
                None => element.w_full(),
            })
            .gap(px(theme.space(Space::Md)))
            .p(px(theme.space(Space::Sm)))
            .bg(theme.colors.panel)
            .children(header)
            .child(
                div()
                    .when(!self.fit_height, |body| body.flex_1())
                    .min_h(px(0.0))
                    .child(
                        ScrollArea::new(self.ident.child("scroll"))
                            .when(self.fit_height, |scroll| scroll.fit_height())
                            .child(body),
                    ),
            )
            .children(
                self.footer
                    .map(|footer| div().flex_none().overflow_hidden().child(footer)),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::List).expanded(!self.collapsed),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_and_icon_replace_the_same_leading_slot() {
        let with_glyph = SidebarItem::new("claude", "Claude Code").icon(Icon::Terminal);
        let with_image = SidebarItem::new("claude", "Claude Code").image("agents/claude.svg");
        let replaced = with_glyph.clone().image("agents/claude.svg");

        assert_ne!(with_glyph, with_image);
        assert_eq!(replaced, with_image);
        assert_eq!(
            with_image.leading,
            Some(SidebarLeading::Image(SharedString::from(
                "agents/claude.svg"
            )))
        );
    }
}
