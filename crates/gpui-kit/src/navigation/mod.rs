//! Navigation and disclosure: tabs, accordion sections, a breadcrumb trail, a
//! collapsible rail, and page controls.
//!
//! None of these own where the typist is. Each reports the place that was
//! picked and renders whatever the caller says is current, so a host that
//! refuses a move keeps showing the place that still holds.

pub mod accordion;
pub mod anchor_list;
pub mod breadcrumb;
pub mod collapsible;
pub mod pagination;
pub mod sidebar;
pub mod tabs;
pub mod undo_history;
pub mod wizard;

pub use accordion::{Accordion, AccordionSection};
pub use anchor_list::{Anchor, AnchorList};
pub use breadcrumb::{Breadcrumb, Crumb};
pub use collapsible::Collapsible;
pub use pagination::{PageTotal, Pagination};
pub use sidebar::{Sidebar, SidebarItem, SidebarSection};
pub use tabs::{SaveState, TabItem, Tabs};
pub use undo_history::{HistoryEntry, UndoHistory};
pub use wizard::{StepStatus, Wizard, WizardIntent, WizardLayout, WizardStep};
