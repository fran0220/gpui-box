//! Floating surfaces that render above the page.
//!
//! [`Overlay`] owns placement, stacking and dismissal; [`FocusTrap`] keeps the
//! keyboard inside an open overlay and gives focus back when it closes.
//! [`Dialog`] composes both into a modal that asks one question, and
//! [`Tooltip`] is hover-delayed help that is never the only way to act.

mod dialog;
mod focus;
mod kbd;
mod layer;
pub mod popover;
pub mod tooltip;

pub use dialog::{Dialog, DialogEvent};
pub use focus::FocusTrap;
pub use kbd::{Kbd, caps};
pub use layer::{Overlay, Placement, priority, surface};
pub use tooltip::{Tooltip, Tooltipped};
