//! Floating surfaces that render above the page.
//!
//! [`Overlay`] owns placement, stacking and dismissal; [`FocusTrap`] keeps the
//! keyboard inside an open overlay and gives focus back when it closes.
//! [`Dialog`] composes both into a modal that asks one question, [`Drawer`] is
//! the same surface arriving from an edge, and [`Tooltip`] is hover-delayed
//! help that is never the only way to act.
//! [`ToastLayer`] holds transient notifications, which report what happened
//! and never hide a failure on a timer.
//!
//! [`Popover`] is the anchored surface the menu family is built from:
//! [`Menu`] hangs one off a trigger, [`ContextMenu`] opens the same list at
//! the pointer, and [`CommandPalette`] filters it from the keyboard.

mod dialog;
mod drawer;
mod focus;
pub mod frost;
pub mod hover_card;
mod kbd;
mod layer;
mod menu;
pub mod menubar;
pub mod notification_center;
mod palette;
pub mod panel;
pub mod popover;
pub mod toast;
pub mod tooltip;

pub use dialog::{Dialog, DialogEvent};
pub use drawer::{Drawer, DrawerEvent};
pub use focus::FocusTrap;
pub use frost::Frost;
pub use hover_card::{HoverCard, HoverCardEvent};
pub use kbd::{Kbd, caps};
pub use layer::{Edge, Overlay, Placement, priority, surface};
pub use menu::{ContextMenu, ContextMenuEvent, Menu, MenuEvent, MenuItem};
pub use menubar::{Menubar, MenubarEvent, MenubarMenu};
pub use notification_center::{
    Notification, NotificationCenter, NotificationCenterEvent, UnreadCount,
};
pub use palette::{Command, CommandPalette, CommandPaletteEvent};
pub use popover::{Popover, PopoverEvent};
pub use toast::{Toast, ToastCorner, ToastLayer};
pub use tooltip::{Tooltip, Tooltipped};
