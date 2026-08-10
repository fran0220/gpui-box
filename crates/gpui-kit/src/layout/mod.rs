//! Frames that decide where other components sit.
//!
//! None of these owns the arrangement. A split reports the ratio a drag asked
//! for, a scroll area reports how far it is scrolled and how much more there
//! is, and a toolbar reports which of its actions did not fit — each renders
//! exactly what the caller says is true, so a host that refuses a change keeps
//! showing the layout that still holds.
//!
//! The application shell is built from the same rule at a larger scale:
//! [`SplitTree`] draws a whole [`SplitLayout`] the caller owns, [`Dock`]
//! arranges panels in regions over that tree, and [`StatusBar`] states what the
//! host says is true along the bottom.

pub mod aspect_ratio;
pub mod dock;
pub(crate) mod measure;
pub mod scroll;
pub mod scroll_fade;
pub mod split;
pub mod status_bar;
pub mod toolbar;
pub mod tree;

pub use aspect_ratio::{AspectFit, AspectRatio};
pub use dock::{Dock, DockEvent, DockPanel, DockRegion};
pub use scroll::{ScrollArea, ScrollAxis, scroll_offset, scroll_to};
pub use scroll_fade::{FadeEdges, ScrollFade};
pub use split::{SplitAxis, SplitPane, SplitSide};
pub use status_bar::{StatusBar, StatusGroup, StatusItem};
pub use toolbar::{Toolbar, ToolbarItem};
pub use tree::{
    SplitChange, SplitKind, SplitLayout, SplitPaneSpec, SplitRecord, SplitRecordError, SplitTree,
};
