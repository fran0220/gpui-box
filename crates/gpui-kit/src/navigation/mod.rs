//! Navigation and disclosure: tabs, accordion sections, and a breadcrumb
//! trail.
//!
//! None of these own where the typist is. Each reports the place that was
//! picked and renders whatever the caller says is current, so a host that
//! refuses a move keeps showing the place that still holds.

pub mod accordion;
pub mod breadcrumb;
pub mod tabs;

pub use accordion::{Accordion, AccordionSection};
pub use breadcrumb::{Breadcrumb, Crumb};
pub use tabs::{TabItem, Tabs};
