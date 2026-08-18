//! The contracts every component in this library implements.
//!
//! Components are `RenderOnce` builders. They read the active theme from the
//! application context instead of accepting a `&Theme` argument, and they
//! derive both their GPUI element id and their semantic assertion id from a
//! single caller-supplied [`Ident`].

pub mod direction;
mod ident;
mod interaction;
pub mod slot;
pub(crate) mod stepping;
mod styled_ext;
mod theme_overlay;

pub use direction::{
    ActiveDirection, DirectionalExt, LayoutDirection, LogicalSide, PhysicalSide,
    set_layout_direction,
};
pub use ident::Ident;
pub use interaction::{HoverLift, Pressable};
pub use slot::{SlotRender, Slots, Slotted};
pub use styled_ext::{
    CardVariant, FocusRing, Hoverable, SelectedRow, StyledExt, rule, rule_vertical, selection_rail,
    text,
};
pub use theme_overlay::ThemeOverlay;

pub use gpui_kit_theme::{
    ActiveTheme, ControlSize, Density, Elevation, Layer, ThemeRegistry, activate_theme, set_density,
};

/// A control that can refuse interaction.
///
/// Implementations must not install action handlers while disabled, so a
/// disabled control cannot fire even if a host mis-routes an event.
pub trait Disableable {
    fn disabled(self, disabled: bool) -> Self;
}

/// A control that can present itself as the current choice.
pub trait Selectable {
    fn selected(self, selected: bool) -> Self;
}

/// A control that resolves its metrics from the token control scale.
pub trait Sizable: Sized {
    fn control_size(self, size: ControlSize) -> Self;

    fn xs(self) -> Self {
        self.control_size(ControlSize::Xs)
    }

    fn small(self) -> Self {
        self.control_size(ControlSize::Sm)
    }

    fn medium(self) -> Self {
        self.control_size(ControlSize::Md)
    }

    fn large(self) -> Self {
        self.control_size(ControlSize::Lg)
    }
}
