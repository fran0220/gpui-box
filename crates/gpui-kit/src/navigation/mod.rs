//! Navigation and disclosure: tabs, accordion sections, a breadcrumb trail, a
//! collapsible rail, and page controls.
//!
//! None of these own where the typist is. Each reports the place that was
//! picked and renders whatever the caller says is current, so a host that
//! refuses a move keeps showing the place that still holds.

pub mod accordion;
pub mod breadcrumb;
pub mod pagination;
pub mod sidebar;
pub mod tabs;
pub mod wizard;

pub use accordion::{Accordion, AccordionSection};
pub use breadcrumb::{Breadcrumb, Crumb};
pub use pagination::{PageTotal, Pagination};
pub use sidebar::{Sidebar, SidebarItem, SidebarSection};
pub use tabs::{TabItem, Tabs};
pub use wizard::{StepStatus, Wizard, WizardIntent, WizardLayout, WizardStep};
