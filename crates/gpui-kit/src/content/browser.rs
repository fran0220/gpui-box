//! The chrome around an embedded web view.
//!
//! This crate draws no web pages. It cannot: rendering one means an engine —
//! `wry`, CEF, WebKit — and a component library that pulled a browser engine
//! into every binary that wanted a button would be charging every host for a
//! feature almost none of them use. So this is the shell only. The host owns
//! the engine, the navigation and the surface the page is painted on, and
//! hands the panel a [`ViewportState`] saying what is actually there.
//!
//! That split is the point rather than a limitation. The states a web view
//! spends its life in — loading, empty, unavailable, failed, ready — are the
//! part a design system should get right and the part every host otherwise
//! reimplements differently. What is left is a rectangle.
//!
//! # What an empty panel means
//!
//! A shell with no engine behind it draws [`ViewportState::Unavailable`], not
//! a blank page. The two look similar and mean opposite things: one is a site
//! that served nothing, the other is a build that cannot ask. A reader who
//! cannot tell them apart will retry the wrong one.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, TypeScale};

use crate::controls::button::IconButton;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::loading::{BarLoader, Skeleton};
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Disableable, Ident, Sizable, StyledExt};
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

/// What is behind the panel right now.
///
/// Five separate answers, kept separate. `Ready` is the only one that shows a
/// page, and the four that do not each say a different thing about why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewportState {
    /// A page is on its way.
    Loading,
    /// Navigation completed successfully and returned no page content.
    Empty,
    /// The host cannot open this address. The reason remains the host's own;
    /// a refusal is not rewritten as an empty page or a failed navigation.
    Unavailable(SharedString),
    /// Navigation failed, in the host's own words.
    Error(SharedString),
    /// A page is there and the host is painting it.
    Ready,
}

impl Default for ViewportState {
    fn default() -> Self {
        // An empty reason selects the localized no-engine explanation. The
        // panel cannot assume an engine exists until its host says one does.
        Self::Unavailable(SharedString::default())
    }
}

impl ViewportState {
    /// What the panel publishes as its value.
    pub fn value(&self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Unavailable(_) => "unavailable",
            Self::Error(_) => "error",
            Self::Ready => "ready",
        }
    }

    pub fn name(&self) -> &'static str {
        self.value()
    }

    fn shows_page(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

impl HasPhase for ViewportState {
    fn phase(&self) -> Phase {
        match self {
            Self::Loading => Phase::Loading,
            Self::Empty => Phase::Empty,
            Self::Unavailable(_) => Phase::Unavailable,
            Self::Error(_) => Phase::Error,
            Self::Ready => Phase::Ready,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(reason) | Self::Error(reason) => Some(reason.as_ref()),
            _ => None,
        }
    }
}

type Action = Rc<dyn Fn(&mut Window, &mut App)>;

/// A framed web view: address, controls, and the rectangle a page goes in.
#[derive(IntoElement)]
pub struct BrowserPanel {
    ident: Ident,
    /// The address as the host reports it. The panel never edits or completes
    /// it, because what a typed string resolves to is the engine's decision.
    url: SharedString,
    /// Whether the host supplied an address at all. An empty string and "no
    /// address yet" are different, and only one of them is worth drawing an
    /// empty bar for.
    url_set: bool,
    state: ViewportState,
    /// Whether history has anywhere to go. A control with nowhere to go is
    /// disabled, and a disabled control installs no handler.
    can_go_back: bool,
    can_go_forward: bool,
    on_back: Option<Action>,
    on_forward: Option<Action>,
    on_reload: Option<Action>,
    /// The host's own painted surface for the page.
    viewport: Option<AnyElement>,
    slots: Slots,
}

impl std::fmt::Debug for BrowserPanel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BrowserPanel")
            .field("ident", &self.ident)
            .field("url", &self.url)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl BrowserPanel {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            url: SharedString::default(),
            // A panel nobody has told about an engine has not got one, so the
            // honest default is the one that says so rather than the one that
            // draws an empty page.
            state: ViewportState::default(),
            url_set: false,
            can_go_back: false,
            can_go_forward: false,
            on_back: None,
            on_forward: None,
            on_reload: None,
            viewport: None,
            slots: Slots::default(),
        }
    }

    pub fn url(mut self, url: impl Into<SharedString>) -> Self {
        self.url = url.into();
        self.url_set = true;
        self
    }

    pub fn state(mut self, state: ViewportState) -> Self {
        self.state = state;
        self
    }

    /// The surface the host paints the page onto.
    pub fn viewport(mut self, viewport: impl IntoElement) -> Self {
        self.viewport = Some(viewport.into_any_element());
        self
    }

    /// Enables the back control and gives it somewhere to go.
    pub fn on_back(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.can_go_back = true;
        self.on_back = Some(Rc::new(handler));
        self
    }

    pub fn on_forward(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.can_go_forward = true;
        self.on_forward = Some(Rc::new(handler));
        self
    }

    pub fn on_reload(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_reload = Some(Rc::new(handler));
        self
    }
}

impl Slotted for BrowserPanel {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for BrowserPanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let strings = cx.strings().clone();
        let panel_id = self.ident.semantic_id();
        let viewport_ident = self.ident.child("viewport");
        let has_page = self.state.shows_page() && self.viewport.is_some();
        let state_value = if self.state.shows_page() && self.viewport.is_none() {
            "error"
        } else {
            self.state.value()
        };
        let busy = self.state == ViewportState::Loading;
        let viewport_description = matches!(self.state, ViewportState::Empty)
            .then(|| strings.text(StringKey::BrowserEmptyDetail));

        let control = |ident: Ident, glyph: Icon, name: SharedString, action: Option<Action>| {
            let mut button = IconButton::new(ident, glyph, name)
                .ghost()
                .small()
                .semantic_parent(panel_id.clone());
            // A control with nowhere to go is disabled and installs nothing,
            // rather than being enabled and quietly doing nothing.
            match action {
                Some(action) => button = button.on_click(move |window, cx| action(window, cx)),
                None => button = button.disabled(true),
            }
            button
        };

        let bar = div()
            .row_reading(direction)
            .w_full()
            .items_center()
            .gap_token(&theme, Space::Xs)
            .px_token(&theme, Space::Sm)
            .py(px(theme.spacing.xs))
            .surface(&theme, Surface::Raised)
            .child(control(
                self.ident.child("back"),
                Icon::ArrowLeft,
                strings.text(StringKey::BrowserBack),
                self.can_go_back.then_some(self.on_back).flatten(),
            ))
            .child(control(
                self.ident.child("forward"),
                Icon::ArrowRight,
                strings.text(StringKey::BrowserForward),
                self.can_go_forward.then_some(self.on_forward).flatten(),
            ))
            .child(control(
                self.ident.child("reload"),
                Icon::Refresh,
                strings.text(StringKey::BrowserReload),
                self.on_reload,
            ))
            .child(
                // The address is a well, not a field: this panel does not
                // accept typing, and a box that looks editable and is not is
                // worse than one that does not look editable.
                div()
                    .flex_1()
                    .min_w_0()
                    .px_token(&theme, Space::Sm)
                    .py(px(theme.spacing.xs / 2.0))
                    .radius(&theme, Radius::Control)
                    .well(&theme)
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(if self.url_set {
                        theme.colors.text_muted
                    } else {
                        theme.colors.text_faint
                    })
                    .truncate()
                    .child(if self.url_set {
                        self.url.clone()
                    } else {
                        strings.text(StringKey::BrowserNoAddress)
                    })
                    .semantic_in(
                        cx,
                        NodeSpec::new(self.ident.child("address").semantic_id(), Role::Text)
                            .parent(panel_id.clone())
                            .text(if self.url_set {
                                self.url.clone()
                            } else {
                                strings.text(StringKey::BrowserNoAddress)
                            }),
                    ),
            );

        let body: AnyElement = match &self.state {
            ViewportState::Ready => self.viewport.unwrap_or_else(|| {
                // Ready with nothing to draw is a host that said the page was
                // there and did not hand one over. That is a mistake worth
                // seeing rather than an empty rectangle worth ignoring.
                self.slots.or_else(slot::FAILED, window, cx, |_, _| {
                    EmptyState::new(
                        viewport_ident.child("status"),
                        strings.text(StringKey::BrowserNoViewport),
                    )
                    .kind(EmptyKind::Failed)
                    .detail(strings.text(StringKey::BrowserNoViewportDetail))
                    .into_any_element()
                })
            }),
            // A page on its way is drawn in the shape of a page — a working
            // strip across the top the way an engine reports progress, then
            // the blocks the content will land in. A word centred in an empty
            // rectangle is indistinguishable from a navigation that stopped.
            ViewportState::Loading => self.slots.or_else(slot::LOADING, window, cx, |_, cx| {
                div()
                    .size_full()
                    .column()
                    .child(BarLoader::new(viewport_ident.child("bar")))
                    .child(
                        div()
                            .column()
                            .w_full()
                            .gap_token(&theme, Space::Sm)
                            .p_token(&theme, Space::Md)
                            .child(
                                Skeleton::new(viewport_ident.child("blocks"))
                                    .rows(4)
                                    .row_height(theme.typography.body.line_height)
                                    .widths([0.52, 1.0, 0.94, 0.66])
                                    .label(strings.text(StringKey::Loading)),
                            ),
                    )
                    .semantic_in(
                        cx,
                        NodeSpec::new(viewport_ident.child("status").semantic_id(), Role::Status)
                            .parent(viewport_ident.semantic_id())
                            .text(strings.text(StringKey::Loading))
                            .value("loading")
                            .busy(true),
                    )
                    .into_any_element()
            }),
            ViewportState::Empty => self.slots.or_else(slot::EMPTY, window, cx, |_, _| {
                EmptyState::new(
                    viewport_ident.child("status"),
                    strings.text(StringKey::BrowserEmpty),
                )
                .kind(EmptyKind::Empty)
                .into_any_element()
            }),
            ViewportState::Unavailable(reason) => {
                self.slots.or_else(slot::EMPTY, window, cx, |_, _| {
                    EmptyState::new(
                        viewport_ident.child("status"),
                        strings.text(StringKey::BrowserUnavailable),
                    )
                    .kind(EmptyKind::Unavailable)
                    .detail(if reason.is_empty() {
                        strings.text(StringKey::BrowserNoEngineDetail)
                    } else {
                        reason.clone()
                    })
                    .into_any_element()
                })
            }
            ViewportState::Error(reason) => self.slots.or_else(slot::FAILED, window, cx, |_, _| {
                EmptyState::new(
                    viewport_ident.child("status"),
                    strings.text(StringKey::BrowserError),
                )
                .kind(EmptyKind::Failed)
                .detail(reason.clone())
                .into_any_element()
            }),
        };

        let mut viewport_spec = NodeSpec::new(viewport_ident.semantic_id(), Role::Region)
            .parent(panel_id.clone())
            .value(state_value)
            .busy(busy);
        if let Some(description) = viewport_description {
            viewport_spec = viewport_spec.description(description);
        }

        div()
            .id(self.ident.element_id())
            .column()
            .size_full()
            .overflow_hidden()
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .child(bar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .surface(&theme, Surface::Canvas)
                    // A loading body fills the viewport the way a page does,
                    // so only the states that are one centred sentence get
                    // centred.
                    .when(!has_page && !busy, |element| {
                        element.flex().items_center().justify_center()
                    })
                    .child(body)
                    .semantic_in(cx, viewport_spec),
            )
            .semantic_in(
                cx,
                NodeSpec::new(panel_id, Role::Group)
                    .text(strings.text(StringKey::BrowserPanel))
                    .value(state_value)
                    .busy(busy),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The panel is the shell for an engine it does not have, so the state it
    /// starts in has to be the one that says so. Defaulting to `Ready` would
    /// draw an empty page for every host that forgot to set it.
    #[test]
    fn a_panel_nobody_has_configured_reports_no_engine() {
        let panel = BrowserPanel::new("browser");
        assert_eq!(panel.state, ViewportState::default());
        assert!(!panel.url_set);
    }

    #[test]
    fn only_a_ready_panel_shows_a_page() {
        assert!(ViewportState::Ready.shows_page());
        for state in [
            ViewportState::Loading,
            ViewportState::Empty,
            ViewportState::Unavailable("blocked".into()),
            ViewportState::Error("dns".into()),
        ] {
            assert!(!state.shows_page(), "{state:?}");
        }
    }

    /// Nothing returned is not the same as something being unavailable or
    /// failing, and the panel may not report one as another.
    #[test]
    fn every_non_ready_state_reports_differently() {
        let values = [
            ViewportState::Loading.value(),
            ViewportState::Empty.value(),
            ViewportState::Unavailable("no".into()).value(),
            ViewportState::Error("no".into()).value(),
        ];
        assert_eq!(values, ["loading", "empty", "unavailable", "error"]);
    }

    /// A history control with nowhere to go must not install an action, so
    /// setting a handler is what makes the direction available.
    #[test]
    fn history_is_only_available_once_a_handler_exists() {
        let panel = BrowserPanel::new("browser");
        assert!(!panel.can_go_back);
        assert!(!panel.can_go_forward);

        let panel = BrowserPanel::new("browser").on_back(|_, _| {});
        assert!(panel.can_go_back);
        assert!(!panel.can_go_forward);
    }

    #[test]
    fn an_address_is_reported_only_once_the_host_supplies_one() {
        let panel = BrowserPanel::new("browser").url("https://example.com");
        assert!(panel.url_set);
        assert_eq!(panel.url, "https://example.com");
    }
}

#[cfg(test)]
mod viewport_phase_tests {
    use super::*;

    #[test]
    fn a_refusal_is_not_an_empty_page() {
        let state = ViewportState::Unavailable("no engine".into());
        assert_eq!(state.phase(), Phase::Unavailable);
        assert_eq!(state.name(), "unavailable");
        assert_eq!(state.reason(), Some("no engine"));
        assert_ne!(ViewportState::Empty.phase(), state.phase());
    }
}
