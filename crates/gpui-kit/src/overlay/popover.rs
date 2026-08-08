//! The anchored surface every menu-shaped component is built from.
//!
//! [`Popover`] is the component: a surface that hangs off a trigger and owns
//! nothing but whether it is open. The free functions around it are the parts
//! `Select`, `Menu`, `ContextMenu` and `CommandPalette` share — row geometry,
//! key classification, cursor movement, and the match ranking a filterable
//! list orders itself by.

use std::rc::Rc;

use gpui::{
    Anchor, AnyElement, App, Context, ElementId, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, Point, Render,
    SharedString, Styled, Window, div, prelude::*, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Space, Theme};

use crate::controls::button::Button;
use crate::foundation::{Ident, StyledExt};
use crate::overlay::focus::FocusTrap;
use crate::overlay::layer::{Overlay, Placement, surface};

use crate::motion;

/// What a keystroke means to a menu-like surface, once the platform's
/// modifier conventions have been applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    /// Enters a submenu, which is the leading edge on a left-to-right layout.
    Right,
    /// Leaves a submenu.
    Left,
    Enter,
    ModifiedEnter,
    Escape,
    Backspace,
    Other,
}

/// Reads one keystroke as a menu intent, so every menu-like surface answers
/// the same keys the same way.
pub fn classify_key(key: &str, command: bool, control: bool) -> MenuKey {
    match key {
        "up" => MenuKey::Up,
        "down" => MenuKey::Down,
        "right" => MenuKey::Right,
        "left" => MenuKey::Left,
        "enter" if command || control => MenuKey::ModifiedEnter,
        "enter" => MenuKey::Enter,
        "escape" => MenuKey::Escape,
        "backspace" => MenuKey::Backspace,
        _ => MenuKey::Other,
    }
}

/// The letter a keystroke types, for a menu that jumps on it.
///
/// Only a bare letter counts: a modified keystroke is a shortcut, and treating
/// it as type-ahead would move the cursor while the typist meant to act.
pub fn typed_letter(key: &str, modifiers: gpui::Modifiers) -> Option<char> {
    if modifiers.platform || modifiers.control || modifiers.alt || modifiers.function {
        return None;
    }
    let mut characters = key.chars();
    let letter = characters.next()?;
    if characters.next().is_some() || !letter.is_alphanumeric() {
        return None;
    }
    Some(letter.to_ascii_lowercase())
}

/// The next entry whose label starts with `letter`, searching forward from the
/// cursor and wrapping. `None` in `labels` marks an entry that cannot be
/// jumped to, such as a separator or a refused row.
pub fn jump_to<S: AsRef<str>>(
    labels: &[Option<S>],
    from: Option<usize>,
    letter: char,
) -> Option<usize> {
    let count = labels.len();
    if count == 0 {
        return None;
    }
    let start = from.map_or(0, |index| index + 1);
    let letter = letter.to_lowercase().next()?;
    (0..count)
        .map(|offset| (start + offset) % count)
        .find(|index| {
            labels[*index].as_ref().is_some_and(|label| {
                label
                    .as_ref()
                    .chars()
                    .next()
                    .and_then(|first| first.to_lowercase().next())
                    == Some(letter)
            })
        })
}

/// The index `delta` steps from `active`, wrapping at the ends.
///
/// A menu wraps where a strip stops: the list is short, the whole of it is on
/// screen, and arrowing off the bottom onto the top is how a menu has always
/// behaved. A strip stops instead, which `foundation::stepping` handles.
pub fn step(active: Option<usize>, count: usize, delta: isize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let count = count as isize;
    Some(match active {
        None if delta >= 0 => 0,
        None => count - 1,
        Some(index) => (index as isize + delta).rem_euclid(count),
    } as usize)
}

/// How well `label` answers `query`, lower being better, or `None` when it
/// does not answer it at all.
///
/// A typist who knows what they are looking for types its beginning, so a
/// prefix outranks the start of a later word, which outranks a match in the
/// middle of one, which outranks letters merely occurring in order.
pub fn match_rank(query: &str, label: &str) -> Option<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(1);
    }
    let label = label.to_lowercase();
    if label.starts_with(&query) {
        Some(0)
    } else if word_starts(&label).any(|start| label[start..].starts_with(&query)) {
        Some(1)
    } else if label.contains(&query) {
        Some(2)
    } else if is_subsequence(&query, &label) {
        Some(3)
    } else {
        None
    }
}

/// Byte offsets of every character that begins a word.
fn word_starts(label: &str) -> impl Iterator<Item = usize> + '_ {
    label.char_indices().filter_map(move |(index, character)| {
        if index == 0 || !character.is_alphanumeric() {
            return None;
        }
        let previous = label[..index].chars().next_back()?;
        (!previous.is_alphanumeric()).then_some(index)
    })
}

fn is_subsequence(query: &str, label: &str) -> bool {
    let mut characters = label.chars();
    query
        .chars()
        .all(|wanted| characters.any(|character| character == wanted))
}

/// The indices of the labels `query` answers, best answer first.
pub fn filter_indices<S: AsRef<str>>(query: &str, labels: &[S]) -> Vec<usize> {
    let mut ranked: Vec<_> = labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| match_rank(query, label.as_ref()).map(|rank| (rank, index)))
        .collect();
    ranked.sort_by_key(|&(rank, index)| (rank, index));
    ranked.into_iter().map(|(_, index)| index).collect()
}

/// The bordered, elevated surface every anchored overlay draws.
pub fn card(theme: &Theme) -> gpui::Div {
    div()
        .border_1()
        .border_color(theme.colors.hairline_strong)
        .rounded(px(theme.radii.card))
        .elevation(theme, Elevation::Overlay)
        .p(px(theme.spacing.xs))
        .overflow_hidden()
        .bg(theme.colors.overlay)
        .text_size(px(theme.typography.body.size))
        .text_color(theme.colors.text)
}

/// [`card`] without the inner padding, for a surface that draws its own rows
/// edge to edge.
pub fn card_flush(theme: &Theme) -> gpui::Div {
    card(theme).p_0()
}

fn pinned(layer: AnyElement) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_0()
        .child(layer)
        .into_any_element()
}

/// Places `content` under `anchor`, flipping above when there is no room.
pub fn anchored_below(id: impl Into<ElementId>, theme: &Theme, content: AnyElement) -> AnyElement {
    pinned(
        gpui::deferred(
            gpui::anchored()
                .anchor(Anchor::TopLeft)
                .snap_to_window_with_margin(px(theme.spacing.sm))
                .child(motion::menu_in(
                    id,
                    theme,
                    div()
                        .occlude()
                        .pt(px(theme.spacing.sm - 2.0))
                        .child(content),
                )),
        )
        .priority(1)
        .into_any_element(),
    )
}

/// Places `content` over `anchor`, flipping below when there is no room.
pub fn anchored_above(id: impl Into<ElementId>, theme: &Theme, content: AnyElement) -> AnyElement {
    pinned(
        gpui::deferred(
            gpui::anchored()
                .anchor(Anchor::BottomLeft)
                .snap_to_window_with_margin(px(theme.spacing.sm))
                .child(motion::menu_in(
                    id,
                    theme,
                    div()
                        .occlude()
                        .pb(px(theme.spacing.sm - 2.0))
                        .child(content),
                )),
        )
        .priority(1)
        .into_any_element(),
    )
}

/// Places `content` at a point, which is what a context menu needs.
pub fn at(
    id: impl Into<ElementId>,
    theme: &Theme,
    position: Point<Pixels>,
    content: AnyElement,
) -> AnyElement {
    gpui::deferred(
        gpui::anchored()
            .position(position)
            .anchor(Anchor::TopLeft)
            .snap_to_window_with_margin(px(theme.spacing.sm))
            .child(motion::menu_in(id, theme, div().occlude().child(content))),
    )
    .priority(1)
    .into_any_element()
}

/// Places `content` in the middle of the window, over a scrim.
pub fn modal(
    id: impl Into<ElementId>,
    theme: &Theme,
    viewport: gpui::Size<Pixels>,
    content: AnyElement,
) -> AnyElement {
    gpui::deferred(
        gpui::anchored()
            .position(gpui::point(px(0.0), px(0.0)))
            .child(
                div()
                    .occlude()
                    .w(viewport.width)
                    .h(viewport.height)
                    .bg(gpui::black().opacity(0.6))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(motion::dialog_in(id, theme, div().child(content))),
            ),
    )
    .priority(2)
    .into_any_element()
}

/// One row of a menu-like surface, with the shared height, hover wash, and
/// refusal treatment.
pub fn menu_row(theme: &Theme, selected: bool, highlighted: bool) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .px(px(theme.spacing.sm))
        .py(px(6.0))
        .rounded(px(theme.radii.control))
        .text_size(px(theme.typography.body.size))
        .text_color(if selected || highlighted {
            theme.colors.text
        } else {
            theme.colors.text_muted
        })
        .when(selected, |element| {
            element
                .bg(theme.colors.selected)
                .shadow(theme.selected_ring())
        })
        .when(!selected && highlighted, |element| {
            element.bg(theme.colors.hover)
        })
        .when(!selected && !highlighted, |element| {
            element.hover(|style| style.bg(theme.colors.hover).text_color(theme.colors.text))
        })
}

/// A section label inside a menu-like surface.
pub fn heading(theme: &Theme, label: &str) -> gpui::Div {
    div()
        .px(px(theme.spacing.sm))
        .pb(px(theme.spacing.xs))
        .pt(px(6.0))
        .text_size(px(10.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.colors.text_muted.opacity(0.6))
        .child(SharedString::from(tracked_upper(label)))
}

/// The hairline between two groups of menu rows.
pub fn separator(theme: &Theme) -> gpui::Div {
    div()
        .h(px(1.0))
        .mx(px(-theme.spacing.xs))
        .my(px(theme.spacing.xs))
        .bg(theme.colors.hairline)
}

/// The shortcut cap a menu row carries on its trailing edge.
pub fn key_cap(theme: &Theme, label: impl Into<SharedString>) -> gpui::Div {
    div()
        .h(px(22.0))
        .px(px(5.0))
        .rounded(px(theme.radii.small))
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.colors.hover.opacity(0.38))
        .font_family(theme.typography.mono.clone())
        .text_size(px(theme.typography.caption.size))
        .text_color(theme.colors.text_muted)
        .child(label.into())
}

/// The surface a modal draws itself on.
pub fn dialog_card(theme: &Theme) -> gpui::Div {
    div()
        .w(px(360.0))
        .p(px(theme.spacing.xl - theme.spacing.xs))
        .rounded(px(theme.radii.dialog))
        .bg(theme.colors.overlay)
        .border_1()
        .border_color(theme.colors.hairline_strong)
        .elevation(theme, Elevation::Modal)
        .flex()
        .flex_col()
        .text_color(theme.colors.text)
}

/// The one question a modal is asking.
pub fn dialog_title(theme: &Theme, title: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(px(15.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.colors.text)
        .child(title.into())
}

/// The detail under a modal's question.
pub fn dialog_body(theme: &Theme, body: impl Into<SharedString>) -> gpui::Div {
    div()
        .mt(px(theme.spacing.sm))
        .text_size(px(theme.typography.body.size))
        .line_height(px(theme.typography.body.line_height))
        .text_color(theme.colors.text_muted)
        .child(body.into())
}

/// Lays a trigger out with the slot an anchored overlay hangs from.
///
/// A surface anchors to where its slot sits, so the slot goes under the
/// trigger for a surface that opens downwards and above it for one that opens
/// upwards. The slot takes no space of its own.
pub fn anchored_slot(
    placement: Placement,
    trigger: AnyElement,
    overlay: Option<AnyElement>,
) -> gpui::Div {
    let slot = div().relative().children(overlay);
    // The trigger sizes to itself: one stretched to its container would claim
    // to be a full-width control.
    let frame = div().flex().flex_col().items_start();
    match placement {
        Placement::Above => frame.child(slot).child(trigger),
        _ => frame.child(trigger).child(slot),
    }
}

/// What a popover reports. The owner decides what any of it means.
///
/// A dismissal is always followed by [`PopoverEvent::Closed`], so a subscriber
/// that only cares that the surface went away has one event to watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopoverEvent {
    Opened,
    /// The surface was waved away, by escape or by a click outside it.
    Dismissed,
    Closed,
}

impl EventEmitter<PopoverEvent> for Popover {}

/// Builds the popover body for one frame.
type Content = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// A surface anchored to a trigger, holding whatever the caller puts in it.
///
/// Open state and the element that had the keyboard before opening both
/// outlive a frame, so a popover is a view rather than a builder. The body is
/// a callback instead of a stored element because an `AnyElement` can be
/// consumed once, while an open popover re-renders for as long as it stays
/// open.
pub struct Popover {
    ident: Ident,
    focus_handle: FocusHandle,
    trigger_focus: FocusHandle,
    trigger: SharedString,
    trigger_icon: Option<Icon>,
    content: Option<Content>,
    placement: Placement,
    dismissable: bool,
    open: bool,
    /// Set by `open`, cleared by the first frame that can act on it.
    pending_focus: bool,
    trap: FocusTrap,
}

impl std::fmt::Debug for Popover {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Popover")
            .field("ident", &self.ident)
            .field("trigger", &self.trigger)
            .field("has_content", &self.content.is_some())
            .field("placement", &self.placement)
            .field("dismissable", &self.dismissable)
            .field("open", &self.open)
            .finish()
    }
}

impl Popover {
    pub fn new(ident: impl Into<Ident>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            trigger_focus: cx.focus_handle(),
            trigger: SharedString::default(),
            trigger_icon: None,
            content: None,
            placement: Placement::Below,
            dismissable: true,
            open: false,
            pending_focus: false,
            trap: FocusTrap::new(),
        }
    }

    /// The label of the control that opens the surface.
    pub fn trigger(mut self, label: impl Into<SharedString>) -> Self {
        self.trigger = label.into();
        self
    }

    pub fn trigger_icon(mut self, icon: Icon) -> Self {
        self.trigger_icon = Some(icon);
        self
    }

    /// Supplies the body, rebuilt on every frame the popover is open.
    pub fn content(
        mut self,
        content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.content = Some(Rc::new(content));
        self
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// Whether escape and a click outside close the surface. A popover that is
    /// not dismissable installs neither handler.
    pub fn dismissable(mut self, dismissable: bool) -> Self {
        self.dismissable = dismissable;
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_dismissable(&self) -> bool {
        self.dismissable
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        self.pending_focus = true;
        self.trap.engage(window, cx);
        cx.emit(PopoverEvent::Opened);
        cx.notify();
    }

    /// Closes the surface and gives the keyboard back to the trigger.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.pending_focus = false;
        self.trap.release(window, cx);
        self.trigger_focus.focus(window, cx);
        cx.emit(PopoverEvent::Closed);
        cx.notify();
    }

    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.dismiss(window, cx);
        } else {
            self.open(window, cx);
        }
    }

    /// Reports a wave-away. A popover that is not dismissable cannot be waved
    /// away even by a host calling this directly.
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open || !self.dismissable {
            return;
        }
        cx.emit(PopoverEvent::Dismissed);
        self.close(window, cx);
    }

    fn on_dismiss_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.open || event.keystroke.key.as_str() != "escape" {
            return;
        }
        self.dismiss(window, cx);
        cx.stop_propagation();
    }
}

impl Focusable for Popover {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Popover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let popover = cx.entity().downgrade();
        let trigger = Button::new(self.ident.child("trigger"))
            .label(self.trigger.clone())
            .secondary()
            .track_focus(&self.trigger_focus)
            .when_some(self.trigger_icon, |button, glyph| button.icon(glyph))
            .on_click(move |window, cx| {
                popover
                    .update(cx, |popover, cx| popover.toggle(window, cx))
                    .ok();
            })
            .into_any_element();

        let overlay = self.open.then(|| {
            if self.pending_focus {
                // The handle can only take focus once this frame has put it in
                // the dispatch tree, which is why opening records the intent.
                self.pending_focus = false;
                self.focus_handle.focus(window, cx);
            }
            let body = self.content.clone().map(|content| content(window, cx));
            let mut card = surface(&theme, Elevation::Overlay)
                .p_token(&theme, Space::Sm)
                .track_focus(&self.focus_handle);
            if self.dismissable {
                card = card
                    .on_key_down(cx.listener(Self::on_dismiss_key))
                    .on_mouse_down_out(cx.listener(|popover, _, window, cx| {
                        popover.dismiss(window, cx);
                    }));
            }
            let card = card.children(body).semantic_in(
                cx,
                NodeSpec::new(self.ident.child("surface").semantic_id(), Role::Group)
                    .parent(self.ident.semantic_id())
                    .focus(&self.focus_handle),
            );
            Overlay::new(self.ident.child("overlay"))
                .placement(self.placement)
                .child(card)
                .into_any_element()
        });

        anchored_slot(self.placement, trigger, overlay).semantic_in(
            cx,
            NodeSpec::new(self.ident.semantic_id(), Role::Group).expanded(self.open),
        )
    }
}

/// Upper-cases a section label and opens its letter spacing, which is the one
/// place in the library that shouts.
pub fn tracked_upper(label: &str) -> String {
    let mut output = String::with_capacity(label.len() * 2);
    for (index, character) in label.to_uppercase().chars().enumerate() {
        if index > 0 {
            output.push('\u{200A}');
        }
        output.push(character);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_wraps_and_handles_empty_lists() {
        assert_eq!(step(None, 0, 1), None);
        assert_eq!(step(None, 3, 1), Some(0));
        assert_eq!(step(None, 3, -1), Some(2));
        assert_eq!(step(Some(2), 3, 1), Some(0));
        assert_eq!(step(Some(0), 3, -1), Some(2));
    }

    #[test]
    fn filtering_prefers_prefixes_and_is_stable() {
        let labels = ["main", "feature/main-sync", "master", "dev"];
        assert_eq!(filter_indices("ma", &labels), vec![0, 2, 1]);
        assert_eq!(filter_indices("", &labels), vec![0, 1, 2, 3]);
    }

    #[test]
    fn key_classification_keeps_modified_enter_distinct() {
        assert_eq!(classify_key("enter", false, false), MenuKey::Enter);
        assert_eq!(classify_key("enter", true, false), MenuKey::ModifiedEnter);
        assert_eq!(classify_key("escape", false, false), MenuKey::Escape);
    }

    #[test]
    fn a_submenu_is_entered_and_left_sideways() {
        assert_eq!(classify_key("right", false, false), MenuKey::Right);
        assert_eq!(classify_key("left", false, false), MenuKey::Left);
    }

    #[test]
    fn ranking_prefers_a_prefix_then_a_word_then_a_subsequence() {
        assert_eq!(match_rank("com", "Command palette"), Some(0));
        assert_eq!(match_rank("pal", "Command palette"), Some(1));
        assert_eq!(match_rank("mmand", "Command palette"), Some(2));
        assert_eq!(match_rank("cmp", "Command palette"), Some(3));
        assert_eq!(match_rank("zz", "Command palette"), None);
    }

    #[test]
    fn filtering_orders_literal_matches_ahead_of_a_subsequence() {
        let labels = ["Set theme", "Reset zoom", "Show settings", "Save file"];
        assert_eq!(filter_indices("se", &labels), vec![0, 2, 1, 3]);
    }

    #[test]
    fn type_ahead_wraps_and_skips_entries_it_cannot_land_on() {
        let labels = [
            Some("Copy"),
            None,
            Some("Cut"),
            Some("Paste"),
            Some("Copy path"),
        ];
        assert_eq!(jump_to(&labels, None, 'c'), Some(0));
        assert_eq!(jump_to(&labels, Some(0), 'c'), Some(2));
        assert_eq!(jump_to(&labels, Some(2), 'c'), Some(4));
        assert_eq!(jump_to(&labels, Some(4), 'c'), Some(0));
        assert_eq!(jump_to(&labels, None, 'z'), None);
    }

    #[test]
    fn only_an_unmodified_letter_is_type_ahead() {
        let none = gpui::Modifiers::none();
        assert_eq!(typed_letter("s", none), Some('s'));
        assert_eq!(typed_letter("escape", none), None);
        assert_eq!(typed_letter("s", gpui::Modifiers::command()), None);
    }
}
