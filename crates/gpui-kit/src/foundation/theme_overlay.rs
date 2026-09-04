//! Overriding the theme for one subtree.
//!
//! Tokens are the authority and a component reads them rather than accepting
//! colours, which is what keeps a hundred and forty components consistent. It
//! also means a caller with one honest exception — a media player whose chrome
//! is dark inside a light application, a preview that has to show the theme it
//! is previewing, a brand surface that owns its own accent — had no supported
//! way to say so, and the unsupported ways all end the same: the component is
//! copied into the product and stops receiving fixes.
//!
//! [`ThemeOverlay`] is the supported way. It takes the theme in force, hands
//! it to the caller to modify, and installs the result for its child and for
//! nothing else. What comes back is still a [`Theme`], so the subtree is still
//! reading a complete token set rather than a scattering of overrides, and a
//! component inside it cannot tell the difference — which is the point, since
//! a component that could would need a policy for it.
//!
//! The pairing is the element's, not the caller's: the override is pushed
//! before the child is laid out and popped after, in every phase, so there is
//! no path on which it escapes into a sibling.

use gpui::{
    AnyElement, App, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Window,
};
use gpui_kit_theme::{ActiveTheme, Theme, pop_theme, push_theme};

type Adjust = Box<dyn FnOnce(&Theme) -> Theme>;

/// A subtree that reads a theme its caller adjusted.
pub struct ThemeOverlay {
    adjust: Option<Adjust>,
    resolved: Option<Theme>,
    child: AnyElement,
}

impl ThemeOverlay {
    /// Adjusts the theme in force and installs the result for `child`.
    ///
    /// ```ignore
    /// ThemeOverlay::new(
    ///     |theme| theme.clone().modify(|theme| theme.colors.accent = brand),
    ///     checkout_panel(),
    /// )
    /// ```
    pub fn new(adjust: impl FnOnce(&Theme) -> Theme + 'static, child: impl IntoElement) -> Self {
        Self {
            adjust: Some(Box::new(adjust)),
            resolved: None,
            child: child.into_any_element(),
        }
    }

    /// Installs a whole theme, for a caller that already has the one it wants.
    pub fn theme(theme: Theme, child: impl IntoElement) -> Self {
        Self::new(move |_| theme, child)
    }

    /// The theme this overlay installs, resolved once and reused by every
    /// phase, because a closure that ran per phase could return three
    /// different themes for one frame.
    fn resolve(&mut self, cx: &App) -> Theme {
        if let Some(theme) = self.resolved.clone() {
            return theme;
        }
        let theme = match self.adjust.take() {
            Some(adjust) => adjust(cx.theme()),
            None => cx.theme().clone(),
        };
        self.resolved = Some(theme.clone());
        theme
    }
}

impl IntoElement for ThemeOverlay {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ThemeOverlay {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let theme = self.resolve(cx);
        push_theme(cx, theme);
        let layout = self.child.request_layout(window, cx);
        pop_theme(cx);
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let theme = self.resolve(cx);
        push_theme(cx, theme);
        self.child.prepaint(window, cx);
        pop_theme(cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let theme = self.resolve(cx);
        push_theme(cx, theme);
        self.child.paint(window, cx);
        pop_theme(cx);
    }
}
