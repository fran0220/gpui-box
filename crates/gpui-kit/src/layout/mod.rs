//! Frames that decide where other components sit.
//!
//! None of these owns the arrangement. A split reports the ratio a drag asked
//! for, a scroll area reports how far it is scrolled and how much more there
//! is, and a toolbar reports which of its actions did not fit — each renders
//! exactly what the caller says is true, so a host that refuses a change keeps
//! showing the layout that still holds.

mod measure;
pub mod scroll;
pub mod split;
pub mod toolbar;

pub use scroll::{ScrollArea, ScrollAxis};
pub use split::{SplitAxis, SplitPane, SplitSide};
pub use toolbar::{Toolbar, ToolbarItem};
