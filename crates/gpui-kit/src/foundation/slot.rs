//! Replacing a node a component authored, rather than configuring it.
//!
//! A builder lets a caller change what a component was told. It does not let
//! a caller change what the component decided, and some of those decisions
//! are nodes: the empty state a list shows when it has nothing, the failure
//! it shows when a load failed, the control a toolbar puts its overflow
//! behind. Those are complete little components in their own right, and a
//! host that needs one of them to be its own — with its own wording, its own
//! action, its own illustration — has no way to say so through a setter.
//!
//! Before this existed, components that felt the pressure grew a private
//! escape hatch each: `DataGrid` took a `vacancy`, others took nothing and
//! the caller worked around the component. One named mechanism replaces all
//! of them, so a reader who has learned it once has learned it everywhere.
//!
//! A slot name is a `&'static str` a component publishes as an associated
//! constant, and [`Slotted::slot`] rejects a name the component does not
//! declare. That rejection is a panic rather than a silent no-op on purpose:
//! a filled slot that renders nothing looks exactly like a component that
//! ignored you, and the caller has no way to tell the two apart.

use std::rc::Rc;

use gpui::{AnyElement, App, Window};

/// What a caller supplies for one slot.
pub type SlotRender = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// The nodes a caller has replaced inside one component.
///
/// Components hold one of these and reach it through [`Slotted`]. The list is
/// short by construction — a component names its slots and there are a few of
/// them — so this is a vector rather than a map.
#[derive(Clone, Default)]
pub struct Slots {
    filled: Vec<(&'static str, SlotRender)>,
}

impl std::fmt::Debug for Slots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slots")
            .field(
                "filled",
                &self.filled.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Slots {
    /// Records a caller's node for one position, replacing any earlier one.
    pub fn set(
        &mut self,
        name: &'static str,
        render: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) {
        let render: SlotRender = Rc::new(render);
        match self.filled.iter_mut().find(|(held, _)| *held == name) {
            Some(entry) => entry.1 = render,
            None => self.filled.push((name, render)),
        }
    }

    /// Whether the caller replaced this position.
    ///
    /// A component asks this when the caller's node makes some of its own
    /// work unnecessary, rather than merely different.
    pub fn holds(&self, name: &'static str) -> bool {
        self.filled.iter().any(|(held, _)| *held == name)
    }

    /// The caller's node for this position, if there is one.
    pub fn render(
        &self,
        name: &'static str,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let render = self
            .filled
            .iter()
            .find(|(held, _)| *held == name)
            .map(|(_, render)| Rc::clone(render))?;
        Some(render(window, cx))
    }

    /// The caller's node for this position, or the component's own.
    ///
    /// The component's node is built only when it is the one being used, so a
    /// replaced empty state costs nothing to have been replaced.
    pub fn or_else(
        &self,
        name: &'static str,
        window: &mut Window,
        cx: &mut App,
        own: impl FnOnce(&mut Window, &mut App) -> AnyElement,
    ) -> AnyElement {
        match self.render(name, window, cx) {
            Some(element) => element,
            None => own(window, cx),
        }
    }
}

/// A component with named positions a caller may replace.
///
/// The names live in [`Slotted::SLOTS`], which is what makes a typo a failure
/// instead of a silence, and what lets the generated API index tell a reader
/// which positions a given component actually offers.
pub trait Slotted: Sized {
    /// Every position this component names.
    const SLOTS: &'static [&'static str];

    fn slots_mut(&mut self) -> &mut Slots;

    /// Replaces the node this component would have authored at `name`.
    ///
    /// # Panics
    ///
    /// If `name` is not one of [`Slotted::SLOTS`]. The set is fixed at compile
    /// time and a slot nobody reads is indistinguishable from a component that
    /// ignored the caller, so this reports rather than absorbs it.
    #[track_caller]
    fn slot(
        mut self,
        name: &'static str,
        render: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        assert!(
            Self::SLOTS.contains(&name),
            "{name} is not a slot this component names; it offers {:?}",
            Self::SLOTS
        );
        self.slots_mut().set(name, render);
        self
    }
}

/// The slot every component that can have nothing to show names.
///
/// A list, a grid, a tree, a catalogue and a palette all reach the same state
/// and all authored their own node for it. Sharing the name means a host that
/// wants its own empty state writes it once and applies it everywhere.
pub const EMPTY: &str = "empty";

/// The slot for the node a component shows when a load failed.
///
/// Kept apart from [`EMPTY`] because a failure and an absence are two facts,
/// and a component that lets a caller replace one of them with the other has
/// handed over the means to lie.
pub const FAILED: &str = "failed";

/// The slot for the node a component shows while it is loading.
pub const LOADING: &str = "loading";
