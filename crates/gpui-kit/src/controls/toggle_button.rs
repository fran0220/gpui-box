//! Buttons that stay in: one on its own, and a run of them.
//!
//! # Why this is not a [`Switch`](crate::controls::toggle::Switch)
//!
//! A switch is a setting. Flipping it applies something to the world — the
//! window goes dark, the daemon starts — and it is drawn as a setting, with a
//! label beside a track, so a reader can tell at a glance which of two states
//! the machine is in without acting on anything.
//!
//! A toggle is a button. Pressing it does not change a setting; it changes
//! what the *next* thing you do means, and it looks like a button because you
//! act on it. Bold in a formatting bar is a toggle: it stays in, but nothing
//! became bold when it went in. The published roles say the same thing —
//! `Switch` publishes [`Role::Switch`], a toggle publishes [`Role::Button`]
//! carrying a checked state — so a reader is never told a button is a setting.
//!
//! Both exist because a host that has only one of them has to lie with the
//! other, and the lie is not cosmetic: a switch nobody applied is a switch
//! reporting a state the machine is not in.
//!
//! # Why this is not a [`SegmentedControl`](crate::controls::segmented::SegmentedControl)
//!
//! A segmented control is a radio group in a strip: exactly one segment holds,
//! always, and there is no keystroke or click that empties it. That is the
//! right control for a required choice among a few, and [`ToggleGroup`] does
//! not reimplement it.
//!
//! What a segmented control cannot say is *none* and cannot say *several*, and
//! those are the two cases here, which is why the modes are named
//! [`ToggleSelection::AtMostOne`] and [`ToggleSelection::Any`] rather than
//! single and multiple. If the answer is required, reach for a segmented
//! control.

use std::rc::Rc;

use gpui::{
    App, FocusHandle, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::ControlSize;

use crate::controls::button::{Button, ButtonJoin, ButtonVariant};
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{Disableable, Ident, Selectable, Sizable};
use crate::reactive::Binding;

/// Reports the state the toggle should take next.
type PressHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A button that stays in.
///
/// Whether it is in belongs to the caller: the toggle draws the state it was
/// handed and reports the state pressing it asks for. A host that refuses the
/// press simply does not apply it, and the button keeps showing what is true.
#[derive(IntoElement)]
pub struct Toggle {
    ident: Ident,
    label: Option<SharedString>,
    name: Option<SharedString>,
    glyph: Option<Icon>,
    icon_only: bool,
    pressed: bool,
    disabled: bool,
    size: ControlSize,
    variant: ButtonVariant,
    join: ButtonJoin,
    semantic_parent: Option<SharedString>,
    focus_handle: Option<FocusHandle>,
    on_press: Option<PressHandler>,
}

impl std::fmt::Debug for Toggle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Toggle")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("pressed", &self.pressed)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_press.is_some())
            .finish()
    }
}

impl Toggle {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            name: None,
            glyph: None,
            icon_only: false,
            pressed: false,
            disabled: false,
            size: ControlSize::Md,
            // A toggle out still has to look like a button. Drawn as a ghost
            // it is a word on the canvas, which is indistinguishable from the
            // label beside it and from the refused toggle at the end of the
            // row; the tonal fill is what says there is something here to
            // press before anybody presses it.
            variant: ButtonVariant::Secondary,
            join: ButtonJoin::Alone,
            semantic_parent: None,
            focus_handle: None,
            on_press: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// What assistive technology and a test call this toggle, when the words
    /// on it are not what it is called.
    pub fn accessible_name(mut self, name: impl Into<SharedString>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn icon(mut self, glyph: Icon) -> Self {
        self.glyph = Some(glyph);
        self
    }

    /// Draws the toggle as a square carrying only its glyph, and names it.
    ///
    /// The name is required for the same reason it is on
    /// [`IconButton`](crate::controls::button::IconButton): a glyph nobody can
    /// name is a control nobody can announce or address.
    pub fn icon_only(mut self, glyph: Icon, name: impl Into<SharedString>) -> Self {
        self.glyph = Some(glyph);
        self.label = None;
        self.icon_only = true;
        self.name = Some(name.into());
        self
    }

    /// Whether the toggle is in. This is the caller's answer, not the
    /// toggle's.
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Secondary)
    }

    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    /// Where the toggle sits in a joined run of them.
    pub fn join(mut self, join: ButtonJoin) -> Self {
        self.join = join;
        self
    }

    pub fn semantic_parent(mut self, parent: impl Into<SharedString>) -> Self {
        self.semantic_parent = Some(parent.into());
        self
    }

    pub fn track_focus(mut self, handle: &FocusHandle) -> Self {
        self.focus_handle = Some(handle.clone());
        self
    }

    /// Reports the state pressing asks for, which is the opposite of the one
    /// the toggle was handed.
    pub fn on_press(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_press = Some(Rc::new(handler));
        self
    }

    /// Draws what the caller's [`Binding`] currently holds, and writes back
    /// through it. Sugar for [`Self::pressed`] and [`Self::on_press`].
    pub fn bind(self, binding: &Binding<bool>, cx: &App) -> Self {
        let pressed = binding.get(cx);
        let binding = binding.clone();
        self.pressed(pressed)
            .on_press(move |next, _window, cx| binding.set(cx, next))
    }

    fn actionable(&self) -> bool {
        !self.disabled && self.on_press.is_some()
    }
}

impl Disableable for Toggle {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Toggle {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let next = !self.pressed;
        let actionable = self.actionable();

        Button::new(self.ident.clone())
            .variant(self.variant)
            .control_size(self.size)
            .join(self.join)
            .disabled(self.disabled)
            // Selected is what paints the button in; checked is what says so
            // out loud, including when the answer is no.
            .selected(self.pressed)
            .checked_state(self.pressed)
            .when_some(self.label.clone(), |button, label| button.label(label))
            .when_some(self.name.clone(), |button, name| {
                button.accessible_name(name)
            })
            .when_some(self.glyph, |button, glyph| {
                match (self.icon_only, self.name.clone()) {
                    (true, Some(name)) => button.icon_only(glyph, name),
                    _ => button.icon(glyph),
                }
            })
            .when_some(self.semantic_parent.clone(), |button, parent| {
                button.semantic_parent(parent)
            })
            .when_some(self.focus_handle.as_ref(), |button, handle| {
                button.track_focus(handle)
            })
            .when_some(
                actionable.then(|| self.on_press.clone()).flatten(),
                |button, handler| button.on_click(move |window, cx| handler(next, window, cx)),
            )
    }
}

/// How many of a group's toggles may be in at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToggleSelection {
    /// One or none. Pressing the one that is in reports an empty set, which is
    /// exactly what a [`SegmentedControl`](crate::controls::segmented::SegmentedControl)
    /// cannot do.
    AtMostOne,
    /// Any number, including none and all.
    #[default]
    Any,
}

/// One toggle in a group, identified by business identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToggleItem {
    id: SharedString,
    label: SharedString,
    name: Option<SharedString>,
    icon: Option<Icon>,
    icon_only: bool,
    disabled: bool,
}

impl ToggleItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            name: None,
            icon: None,
            icon_only: false,
            disabled: false,
        }
    }

    /// A toggle carrying only a glyph. The name is what it is called.
    pub fn glyph(id: impl Into<SharedString>, glyph: Icon, name: impl Into<SharedString>) -> Self {
        let name = name.into();
        Self {
            icon: Some(glyph),
            icon_only: true,
            name: Some(name.clone()),
            ..Self::new(id, name)
        }
    }

    pub fn icon(mut self, glyph: Icon) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// Refuses this toggle. A refused toggle installs no handler.
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

/// Reports the whole set the group should hold next, and which toggle was
/// acted on to get there.
type ChangeHandler = Rc<dyn Fn(Vec<SharedString>, SharedString, &mut Window, &mut App)>;

/// A run of toggles where none, one, or several may be in.
///
/// The set is the caller's. The group reports the set pressing one toggle asks
/// for, and draws whichever set the caller handed it, so a host that applies
/// only part of the report gets exactly what it applied.
///
/// Every toggle is its own tab stop, because every one of them is separately
/// actionable — unlike a segmented control, where the whole strip is one stop
/// and the arrows move the single answer around inside it.
#[derive(IntoElement)]
pub struct ToggleGroup {
    ident: Ident,
    label: Option<SharedString>,
    items: Vec<ToggleItem>,
    pressed: Vec<SharedString>,
    selection: ToggleSelection,
    size: ControlSize,
    variant: ButtonVariant,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl std::fmt::Debug for ToggleGroup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToggleGroup")
            .field("ident", &self.ident)
            .field("items", &self.items.len())
            .field("pressed", &self.pressed)
            .field("selection", &self.selection)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_change.is_some())
            .finish()
    }
}

impl ToggleGroup {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            items: Vec::new(),
            pressed: Vec::new(),
            selection: ToggleSelection::default(),
            size: ControlSize::Md,
            variant: ButtonVariant::Secondary,
            disabled: false,
            on_change: None,
        }
    }

    /// What the whole group is asking, for a reader who has only the tree.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = ToggleItem>) -> Self {
        self.items = items.into_iter().collect();
        self
    }

    pub fn selection(mut self, selection: ToggleSelection) -> Self {
        self.selection = selection;
        self
    }

    /// The set that is in. Ids the group does not offer are ignored rather
    /// than invented into toggles.
    pub fn pressed(mut self, ids: impl IntoIterator<Item = SharedString>) -> Self {
        self.pressed = ids.into_iter().collect();
        self
    }

    pub fn pressed_ids<S: AsRef<str>>(mut self, ids: &[S]) -> Self {
        self.pressed = ids
            .iter()
            .map(|id| SharedString::from(id.as_ref().to_string()))
            .collect();
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(Vec<SharedString>, SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Draws the set the caller's [`Binding`] holds, and writes the set a
    /// press asks for. Sugar for [`Self::pressed`] and [`Self::on_change`].
    ///
    /// The set is the whole answer, in both directions, so
    /// [`ToggleSelection`] keeps deciding what a press does to it.
    pub fn bind(self, binding: &Binding<Vec<SharedString>>, cx: &App) -> Self {
        let pressed = binding.get(cx);
        let binding = binding.clone();
        self.pressed(pressed)
            .on_change(move |next, _pressed, _window, cx| binding.set(cx, next))
    }
}

impl Disableable for ToggleGroup {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for ToggleGroup {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

/// The set pressing `id` asks for.
///
/// Order follows the group's own order rather than the order things were
/// pressed in, so a host that renders the report back gets a stable answer.
fn next_set(
    items: &[ToggleItem],
    pressed: &[SharedString],
    id: &SharedString,
    selection: ToggleSelection,
) -> Vec<SharedString> {
    let is_in = pressed.contains(id);
    match (selection, is_in) {
        (ToggleSelection::AtMostOne, true) => Vec::new(),
        (ToggleSelection::AtMostOne, false) => vec![id.clone()],
        (ToggleSelection::Any, _) => items
            .iter()
            .map(|item| &item.id)
            .filter(|other| {
                if *other == id {
                    !is_in
                } else {
                    pressed.contains(other)
                }
            })
            .cloned()
            .collect(),
    }
}

impl RenderOnce for ToggleGroup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let parent = self.ident.semantic_id();
        let last = self.items.len().saturating_sub(1);
        let actionable = !self.disabled && self.on_change.is_some();

        let toggles = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let join = match (index, last) {
                    (_, 0) => ButtonJoin::Alone,
                    (0, _) => ButtonJoin::Leading,
                    (index, last) if index == last => ButtonJoin::Trailing,
                    _ => ButtonJoin::Middle,
                };
                let refused = self.disabled || item.disabled;
                let id = item.id.clone();
                let handler = actionable
                    .then(|| self.on_change.clone())
                    .flatten()
                    .filter(|_| !item.disabled);
                let next = next_set(&self.items, &self.pressed, &item.id, self.selection);

                Toggle::new(self.ident.child(item.id.as_ref()))
                    .variant(self.variant)
                    .control_size(self.size)
                    .join(join)
                    .semantic_parent(parent.clone())
                    .pressed(self.pressed.contains(&item.id))
                    .disabled(refused)
                    .when(!item.icon_only, |toggle| toggle.label(item.label.clone()))
                    .when_some(item.name.clone(), |toggle, name| {
                        toggle.accessible_name(name)
                    })
                    .when_some(item.icon, |toggle, glyph| {
                        match (item.icon_only, item.name.clone()) {
                            (true, Some(name)) => toggle.icon_only(glyph, name),
                            _ => toggle.icon(glyph),
                        }
                    })
                    .when_some(handler, |toggle, handler| {
                        toggle.on_press(move |_, window, cx| {
                            handler(next.clone(), id.clone(), window, cx)
                        })
                    })
            })
            .collect::<Vec<_>>();

        // No gap: the toggles are joined, so their shared corners are already
        // flattened. A gap between them leaves square edges facing across a
        // hole, which is a run that has been taken apart rather than one
        // frame.
        div()
            .row_reading(cx.layout_direction())
            .flex_none()
            .children(toggles)
            .semantic_in(cx, {
                let mut spec = NodeSpec::new(parent, Role::Toolbar).disabled(self.disabled);
                if let Some(label) = self.label.clone() {
                    spec = spec.text(label);
                }
                spec
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<ToggleItem> {
        vec![
            ToggleItem::new("bold", "Bold"),
            ToggleItem::new("italic", "Italic"),
            ToggleItem::new("underline", "Underline").disabled(true),
        ]
    }

    fn ids(values: &[&str]) -> Vec<SharedString> {
        values.iter().map(|id| SharedString::from(*id)).collect()
    }

    #[test]
    fn several_may_be_in_at_once() {
        let items = items();
        let pressed = ids(&["bold"]);
        assert_eq!(
            next_set(&items, &pressed, &"italic".into(), ToggleSelection::Any),
            ids(&["bold", "italic"])
        );
        assert_eq!(
            next_set(&items, &pressed, &"bold".into(), ToggleSelection::Any),
            Vec::<SharedString>::new()
        );
    }

    #[test]
    fn the_report_follows_the_groups_own_order() {
        let items = items();
        let pressed = ids(&["italic"]);
        assert_eq!(
            next_set(&items, &pressed, &"bold".into(), ToggleSelection::Any),
            ids(&["bold", "italic"]),
            "the order is the group's, not the order they went in"
        );
    }

    #[test]
    fn at_most_one_can_be_emptied_which_is_the_whole_difference() {
        let items = items();
        let pressed = ids(&["bold"]);
        assert_eq!(
            next_set(
                &items,
                &pressed,
                &"italic".into(),
                ToggleSelection::AtMostOne
            ),
            ids(&["italic"])
        );
        assert_eq!(
            next_set(&items, &pressed, &"bold".into(), ToggleSelection::AtMostOne),
            Vec::<SharedString>::new(),
            "a radio group has no move that gets here"
        );
    }
}
