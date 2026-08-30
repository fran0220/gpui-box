//! One region that opens and shuts.
//!
//! [`Accordion`] is the many-region case and holds every rule worth holding:
//! the header treatment, the chevron that turns, the semantic motion role, and
//! the decision that a shut body leaves the tree rather than hiding in it. The
//! shared GPUI [`gpui::Reveal`] primitive owns measured disclosure geometry and
//! clipping. A collapsible is the same component policy with one section, so it
//! is built by handing that section to an accordion rather than writing either
//! layer again.
//!
//! Whether it is open belongs to the caller, as it does for the accordion: the
//! collapsible reports the state activating the header asks for and draws the
//! state it was handed.

use std::rc::Rc;

use gpui::{
    AnyElement, App, IntoElement, RenderOnce, SharedString, Window, prelude::FluentBuilder,
};
use gpui_kit_theme::ControlSize;

use crate::foundation::{Ident, Sizable};
use crate::navigation::accordion::{Accordion, AccordionSection};

/// The section id the one region is filed under.
///
/// It is a fixed word rather than the caller's own id so the published header
/// lands at `{ident}.header` for every collapsible in a tree, which is what a
/// test addresses.
const SECTION: &str = "header";

type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A single region behind a header that opens and shuts it.
#[derive(IntoElement)]
pub struct Collapsible {
    ident: Ident,
    title: SharedString,
    description: Option<SharedString>,
    open: bool,
    disabled: bool,
    size: ControlSize,
    body: Option<AnyElement>,
    on_toggle: Option<ToggleHandler>,
}

impl std::fmt::Debug for Collapsible {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Collapsible")
            .field("ident", &self.ident)
            .field("title", &self.title)
            .field("open", &self.open)
            .field("disabled", &self.disabled)
            .field("has_body", &self.body.is_some())
            .field("has_handler", &self.on_toggle.is_some())
            .finish()
    }
}

impl Collapsible {
    pub fn new(ident: impl Into<Ident>, title: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            title: title.into(),
            description: None,
            open: false,
            disabled: false,
            size: ControlSize::Md,
            body: None,
            on_toggle: None,
        }
    }

    /// Secondary text in the header, readable while the region is shut.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Whether the region is open. The caller's answer, not the component's.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Refuses the header. A refused collapsible installs no handler.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }

    /// Reports the state activating the header asks for.
    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// The id of the header a test clicks and a reader announces.
    pub fn header_id(ident: &Ident) -> SharedString {
        ident.child(SECTION).semantic_id()
    }
}

impl Sizable for Collapsible {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Collapsible {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut section =
            AccordionSection::new(SECTION, self.title.clone()).disabled(self.disabled);
        if let Some(description) = self.description.clone() {
            section = section.description(description);
        }
        if let Some(body) = self.body {
            section = section.body(body);
        }

        Accordion::new(self.ident.clone())
            .control_size(self.size)
            .section(section)
            .when(self.open, |accordion| accordion.expanded_ids(&[SECTION]))
            .when_some(self.on_toggle, |accordion, handler| {
                accordion.on_toggle(move |_, next, window, cx| handler(next, window, cx))
            })
    }
}
