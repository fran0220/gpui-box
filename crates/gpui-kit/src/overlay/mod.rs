//! Floating surfaces that render above the page.
//!
//! [`Overlay`] owns placement, stacking and dismissal; [`FocusTrap`] keeps the
//! keyboard inside an open overlay and gives focus back when it closes.

mod focus;
mod kbd;
mod layer;
pub mod popover;

pub use focus::FocusTrap;
pub use kbd::{Kbd, caps};
pub use layer::{Overlay, Placement, priority, surface};
