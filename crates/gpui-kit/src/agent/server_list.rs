//! What is connected, and what each connection offers.
//!
//! # Wording
//!
//! Nothing here names a protocol, a vendor, or a product. A connected thing is
//! a *server*, what it offers are *tools*, *skills* and *resources*, and the
//! act of asking is *asking*. Those words describe the shape of the surface
//! rather than the wire format underneath it, so a host speaking any protocol
//! renders the same component — which is the whole reason this crate refuses
//! product vocabulary.
//!
//! # Five states, and none of them is a shade of another
//!
//! Connected, connecting, disconnected, failed, and turned off by the reader
//! are five different sentences. Collapsing the last two loses the difference
//! between something that broke and something nobody wanted; collapsing the
//! first two claims a connection that has not been made. A failed server keeps
//! its reason on screen, in the host's own words, and offers exactly one
//! control, which reports a retry and retries nothing.
//!
//! # Offering nothing is not the same as not having been asked
//!
//! [`Catalog::Offers`] with an empty list means the server answered and the
//! answer was empty. [`Catalog::Unasked`] means nobody asked. Rendering the
//! second as the first would tell the reader a server is useless when the
//! truth is that the application has not got round to it yet.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px, radians,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TextTone, Theme, TypeScale};

use crate::controls::button::Button;
use crate::display::badge::{Badge, Tone};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::icon::flips;
use crate::display::loading::PulseLoader;
use crate::display::status::{Callout, StatusDot};
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{
    CardVariant, Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt, text,
};
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

use std::f32::consts::FRAC_PI_2;

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type RetryHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(SharedString, bool, &mut Window, &mut App)>;

/// Where a connection stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerState {
    Connected,
    Connecting,
    /// Not connected, and nothing went wrong.
    Disconnected,
    /// The attempt failed. The reason is the host's and is shown word for
    /// word; this crate never authors one.
    Failed {
        reason: SharedString,
    },
    /// The reader turned this one off. A refusal, not a failure, which is why
    /// it is a state of its own rather than a disconnection with a note.
    Disabled {
        reason: Option<SharedString>,
    },
}

impl ServerState {
    /// The name the node publishes. It is the state, never its colour.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Connecting => "connecting",
            Self::Disconnected => "disconnected",
            Self::Failed { .. } => "failed",
            Self::Disabled { .. } => "disabled",
        }
    }

    fn tone(&self) -> Tone {
        match self {
            Self::Connected => Tone::Success,
            Self::Connecting => Tone::Info,
            Self::Disconnected => Tone::Neutral,
            Self::Failed { .. } => Tone::Danger,
            Self::Disabled { .. } => Tone::Neutral,
        }
    }

    fn label(&self, cx: &App) -> SharedString {
        cx.strings().text(match self {
            Self::Connected => StringKey::ServerConnected,
            Self::Connecting => StringKey::ServerConnecting,
            Self::Disconnected => StringKey::ServerDisconnected,
            Self::Failed { .. } => StringKey::ServerFailed,
            Self::Disabled { .. } => StringKey::ServerDisabled,
        })
    }

    /// What the host said about this state, if anything. A failure always has
    /// one; a state the reader chose may not.
    fn reason(&self) -> Option<&SharedString> {
        match self {
            Self::Failed { reason } => Some(reason),
            Self::Disabled { reason } => reason.as_ref(),
            _ => None,
        }
    }
}

impl HasPhase for ServerState {
    fn phase(&self) -> Phase {
        match self {
            Self::Connected => Phase::Ready,
            Self::Connecting => Phase::Loading,
            Self::Disconnected => Phase::Idle,
            Self::Failed { .. } => Phase::Error,
            Self::Disabled { .. } => Phase::Unavailable,
        }
    }

    fn reason(&self) -> Option<&str> {
        ServerState::reason(self).map(|reason| reason.as_ref())
    }
}

/// Which of the three kinds of thing a server offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OfferingKind {
    /// Something the application can call.
    Tool,
    /// A procedure the application can follow.
    Skill,
    /// Something the application can read.
    Resource,
}

impl OfferingKind {
    /// The published name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Tool => "tool",
            Self::Skill => "skill",
            Self::Resource => "resource",
        }
    }

    fn heading(self, cx: &App) -> SharedString {
        cx.strings().text(match self {
            Self::Tool => StringKey::ServerTools,
            Self::Skill => StringKey::ServerSkills,
            Self::Resource => StringKey::ServerResources,
        })
    }

    fn glyph(self) -> Icon {
        match self {
            Self::Tool => Icon::Tuning,
            Self::Skill => Icon::Command,
            Self::Resource => Icon::Document,
        }
    }
}

/// One thing a server offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offering {
    id: SharedString,
    kind: OfferingKind,
    name: SharedString,
    summary: Option<SharedString>,
    /// The one extra fact that tells two similarly named things apart — a
    /// resource's locator, a tool's signature — written by the host.
    qualifier: Option<SharedString>,
}

impl Offering {
    pub fn new(
        id: impl Into<SharedString>,
        kind: OfferingKind,
        name: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            name: name.into(),
            summary: None,
            qualifier: None,
        }
    }

    pub fn tool(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self::new(id, OfferingKind::Tool, name)
    }

    pub fn skill(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self::new(id, OfferingKind::Skill, name)
    }

    pub fn resource(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self::new(id, OfferingKind::Resource, name)
    }

    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    pub fn qualifier(mut self, qualifier: impl Into<SharedString>) -> Self {
        self.qualifier = Some(qualifier.into());
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn kind(&self) -> OfferingKind {
        self.kind
    }

    pub fn name(&self) -> &SharedString {
        &self.name
    }

    pub fn summary_text(&self) -> Option<&SharedString> {
        self.summary.as_ref()
    }

    pub fn qualifier_text(&self) -> Option<&SharedString> {
        self.qualifier.as_ref()
    }
}

/// What is known about what a server offers.
///
/// [`Catalog::Offers`] holding an empty list is an answer; [`Catalog::Unasked`]
/// is the absence of a question. The two are separate variants rather than one
/// emptiness because a surface that cannot tell them apart will show the wrong
/// one to somebody who is trying to work out whether their server is broken.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Catalog {
    /// Nobody has asked this server what it offers.
    #[default]
    Unasked,
    /// The question is in flight.
    Asking,
    /// The server answered. An empty list is an answer.
    Offers(Vec<Offering>),
    /// The question could not be answered, for the host's stated reason.
    Unavailable(SharedString),
}

/// One connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerEntry {
    id: SharedString,
    name: SharedString,
    detail: Option<SharedString>,
    state: ServerState,
    catalog: Catalog,
}

impl ServerEntry {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            detail: None,
            state: ServerState::Disconnected,
            catalog: Catalog::Unasked,
        }
    }

    /// A second line naming the connection: where it runs, which account it
    /// uses. The host writes it; this crate never derives one.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn state(mut self, state: ServerState) -> Self {
        self.state = state;
        self
    }

    pub fn catalog(mut self, catalog: Catalog) -> Self {
        self.catalog = catalog;
        self
    }

    pub fn offers(self, offerings: impl IntoIterator<Item = Offering>) -> Self {
        self.catalog(Catalog::Offers(offerings.into_iter().collect()))
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// The connections an application holds, and what each one offers.
#[derive(IntoElement)]
pub struct ServerList {
    ident: Ident,
    servers: Vec<ServerEntry>,
    expanded: Vec<SharedString>,
    selected: Option<SharedString>,
    size: ControlSize,
    disabled: bool,
    on_select: Option<SelectHandler>,
    on_retry: Option<RetryHandler>,
    on_toggle: Option<ToggleHandler>,
    slots: Slots,
}

impl std::fmt::Debug for ServerList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ServerList")
            .field("ident", &self.ident)
            .field("servers", &self.servers.len())
            .field("expanded", &self.expanded)
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl ServerList {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            servers: Vec::new(),
            expanded: Vec::new(),
            selected: None,
            size: ControlSize::Md,
            disabled: false,
            on_select: None,
            on_retry: None,
            on_toggle: None,
            slots: Slots::default(),
        }
    }

    pub fn server(mut self, server: ServerEntry) -> Self {
        self.servers.push(server);
        self
    }

    pub fn servers(mut self, servers: impl IntoIterator<Item = ServerEntry>) -> Self {
        self.servers.extend(servers);
        self
    }

    /// The servers whose offerings are shown. Everything else is folded away,
    /// and a folded server publishes none of what it offers.
    pub fn expanded(mut self, ids: impl IntoIterator<Item = SharedString>) -> Self {
        self.expanded = ids.into_iter().collect();
        self
    }

    pub fn expanded_ids<S: AsRef<str>>(mut self, ids: &[S]) -> Self {
        self.expanded = ids
            .iter()
            .map(|id| SharedString::from(id.as_ref().to_string()))
            .collect();
        self
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Reports that a failed connection should be attempted again. This crate
    /// connects to nothing.
    pub fn on_retry(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_retry = Some(Rc::new(handler));
        self
    }

    /// Reports that a server's offerings should be shown or folded away.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

impl Disableable for ServerList {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for ServerList {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Slotted for ServerList {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for ServerList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let count = self.servers.len();

        let body: Vec<AnyElement> = if self.servers.is_empty() {
            vec![self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(
                    self.ident.child("empty"),
                    cx.strings().text(StringKey::ServerEmpty),
                )
                .kind(EmptyKind::Empty)
                .detail(cx.strings().text(StringKey::ServerEmptyDetail))
                .into_any_element()
            })]
        } else {
            self.servers
                .iter()
                .map(|server| self.server_element(server, &theme, window, cx))
                .collect()
        };

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .children(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::List).value(count.to_string()),
            )
    }
}

impl ServerList {
    fn server_element(
        &self,
        server: &ServerEntry,
        theme: &Theme,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let direction = cx.layout_direction();
        let ident = self.ident.child(server.id.as_ref());
        let metrics = theme.control.get(self.size);
        let open = self.expanded.contains(&server.id);
        let turned_off = matches!(server.state, ServerState::Disabled { .. });
        // A connection the reader turned off is refused, not dimmed: nothing
        // on its row installs a handler, so it cannot be operated by mistake.
        let refused = self.disabled || turned_off;
        let selected = self.selected.as_ref() == Some(&server.id);
        let selectable = !refused && self.on_select.is_some();
        let toggleable = !refused && self.on_toggle.is_some();

        let chevron = {
            let toggle = ident.child("toggle");
            let mut glyph = div()
                .id(toggle.element_id())
                .row()
                .flex_none()
                .size(px(metrics.icon_size))
                .child(
                    icon(Icon::AltArrowRight)
                        .size(px(metrics.icon_size))
                        .text_color(theme.colors.text_muted)
                        // Open, it points down, which is not directional.
                        .when(!open && flips(Icon::AltArrowRight, direction), |glyph| {
                            glyph.with_transformation(gpui::Transformation::scale(gpui::size(
                                -1.0, 1.0,
                            )))
                        })
                        .when(open, |glyph| {
                            glyph.with_transformation(gpui::Transformation::rotate(radians(
                                FRAC_PI_2,
                            )))
                        }),
                )
                .when(toggleable, |element| {
                    element
                        .cursor_pointer()
                        .tab_index(0)
                        .pressable(cx)
                        .focus_ring(theme)
                });
            if let (true, Some(handler)) = (toggleable, self.on_toggle.clone()) {
                let id = server.id.clone();
                glyph = glyph.on_click(move |_, window, cx| {
                    handler(id.clone(), !open, window, cx);
                    cx.stop_propagation();
                });
            }
            glyph.semantic_in(
                cx,
                NodeSpec::new(toggle.semantic_id(), Role::Button)
                    .parent(ident.semantic_id())
                    .text(server.name.clone())
                    .expanded(open)
                    .disabled(!toggleable),
            )
        };

        let mut header = div()
            .id(ident.element_id())
            .row_reading(direction)
            .w_full()
            .gap_token(theme, Space::Sm)
            .p_token(theme, Space::Sm)
            .when(selected, |element| element.bg(theme.colors.selected))
            .when(turned_off, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .when(selectable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .when(!selected, |element| {
                        element.hover(|style| style.bg(theme.colors.hover.opacity(0.3)))
                    })
                    .focus_ring(theme)
            })
            .child(chevron)
            .child(StatusDot::new(server.state.tone()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .column()
                    .child(text(theme, TypeScale::Label, server.name.clone()))
                    .when_some(server.detail.clone(), |element, detail| {
                        element.child(
                            text(theme, TypeScale::Caption, detail)
                                .text_tone(theme, TextTone::Muted),
                        )
                    }),
            )
            .child(
                Badge::new(server.state.label(cx))
                    .tone(server.state.tone())
                    .id(ident.child("state")),
            );

        if let (true, Some(handler)) = (selectable, self.on_select.clone()) {
            let id = server.id.clone();
            header = header.on_click(move |_, window, cx| handler(id.clone(), window, cx));
        }

        let header = header.semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Row)
                .parent(self.ident.semantic_id())
                .text(server.name.clone())
                .value(server.state.name())
                .selected(selected)
                .disabled(refused)
                .expanded(open),
        );

        // A failure keeps its reason on screen next to the thing that failed.
        // So does a refusal the reader chose, when the host said why.
        let reason = server.state.reason().map(|reason| {
            let node = ident.child("reason");
            div()
                .px_token(theme, Space::Sm)
                .child(
                    Callout::new(
                        reason.clone(),
                        match server.state {
                            ServerState::Failed { .. } => Tone::Danger,
                            _ => Tone::Neutral,
                        },
                    )
                    .id(node.child("callout")),
                )
                .semantic_in(
                    cx,
                    NodeSpec::new(node.semantic_id(), Role::Status)
                        .parent(ident.semantic_id())
                        .text(reason.clone())
                        .value(server.state.name()),
                )
        });

        let retry = self
            .on_retry
            .clone()
            .filter(|_| matches!(server.state, ServerState::Failed { .. }))
            .filter(|_| !refused)
            .map(|handler| {
                let id = server.id.clone();
                div().px_token(theme, Space::Sm).child(
                    Button::new(ident.child("retry"))
                        .label(cx.strings().text(StringKey::TryAgain))
                        .secondary()
                        .control_size(ControlSize::Sm)
                        .on_click(move |window, cx| handler(id.clone(), window, cx)),
                )
            });

        let offerings = open.then(|| self.offerings_element(server, &ident, theme, window, cx));

        div()
            .column()
            .w_full()
            .gap_token(theme, Space::Xs)
            .card_surface(theme, CardVariant::Elevated)
            .child(header)
            .children(reason)
            .children(retry)
            .children(offerings)
            .into_any_element()
    }

    fn offerings_element(
        &self,
        server: &ServerEntry,
        server_ident: &Ident,
        theme: &Theme,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let ident = server_ident.child("offerings");
        match &server.catalog {
            Catalog::Unasked => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(ident, cx.strings().text(StringKey::ServerOfferingsUnasked))
                    .kind(EmptyKind::Unstarted)
                    .detail(cx.strings().text(StringKey::ServerOfferingsUnaskedDetail))
                    .into_any_element()
            }),
            Catalog::Asking => self.slots.or_else(slot::LOADING, window, cx, |_, cx| {
                let label = cx.strings().text(StringKey::ServerOfferingsAsking);
                div()
                    .p_token(theme, Space::Sm)
                    .child(PulseLoader::new(ident.child("loader")).label(label.clone()))
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.semantic_id(), Role::Status)
                            .parent(server_ident.semantic_id())
                            .text(label)
                            .busy(true)
                            .value("asking"),
                    )
                    .into_any_element()
            }),
            Catalog::Unavailable(reason) => self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(
                    ident,
                    cx.strings().text(StringKey::ServerOfferingsUnavailable),
                )
                .kind(EmptyKind::Unavailable)
                .detail(reason.clone())
                .into_any_element()
            }),
            // The answer was empty, which is an answer. It is drawn as one:
            // the reader is told the server offers nothing, not that nobody
            // has looked.
            Catalog::Offers(offerings) if offerings.is_empty() => {
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(ident, cx.strings().text(StringKey::ServerOfferingsNone))
                        .kind(EmptyKind::Empty)
                        .detail(cx.strings().text(StringKey::ServerOfferingsNoneDetail))
                        .into_any_element()
                })
            }
            Catalog::Offers(offerings) => {
                let mut groups: Vec<AnyElement> = Vec::new();
                for kind in [
                    OfferingKind::Tool,
                    OfferingKind::Skill,
                    OfferingKind::Resource,
                ] {
                    let members: Vec<&Offering> = offerings
                        .iter()
                        .filter(|offering| offering.kind == kind)
                        .collect();
                    if members.is_empty() {
                        continue;
                    }
                    let heading_ident = ident.child(kind.name());
                    let heading = kind.heading(cx);
                    groups.push(
                        div()
                            .column()
                            .w_full()
                            .child(
                                text(theme, TypeScale::Subtitle, heading.clone())
                                    .px_token(theme, Space::Sm)
                                    .py_token(theme, Space::Xs)
                                    .text_tone(theme, TextTone::Faint)
                                    .semantic_in(
                                        cx,
                                        NodeSpec::new(heading_ident.semantic_id(), Role::Heading)
                                            .parent(ident.semantic_id())
                                            .text(heading)
                                            .value(members.len().to_string()),
                                    ),
                            )
                            .children(members.into_iter().map(|offering| {
                                self.offering_element(server, offering, server_ident, theme, cx)
                            }))
                            .into_any_element(),
                    );
                }
                div()
                    .column()
                    .w_full()
                    .children(groups)
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.semantic_id(), Role::List)
                            .parent(server_ident.semantic_id())
                            .value(offerings.len().to_string()),
                    )
                    .into_any_element()
            }
        }
    }

    /// One offering, named under the server that offers it.
    ///
    /// Two servers may offer the same name, so the id carries the attribution
    /// and a test never has to guess which one it reached.
    fn offering_element(
        &self,
        server: &ServerEntry,
        offering: &Offering,
        server_ident: &Ident,
        theme: &Theme,
        cx: &mut App,
    ) -> AnyElement {
        let direction = cx.layout_direction();
        let ident = server_ident.child("offering").child(offering.id.as_ref());
        let metrics = theme.control.get(self.size);
        let _ = server;

        div()
            .row_reading(direction)
            .w_full()
            .items_start()
            .gap_token(theme, Space::Sm)
            .px_token(theme, Space::Sm)
            .py_token(theme, Space::Xs)
            .child(
                icon(offering.kind.glyph())
                    .size(px(metrics.icon_size))
                    .text_color(theme.colors.text_faint),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .column()
                    .child(text(theme, TypeScale::Label, offering.name.clone()))
                    .when_some(offering.summary.clone(), |element, summary| {
                        element.child(
                            text(theme, TypeScale::Body, summary).text_tone(theme, TextTone::Muted),
                        )
                    })
                    .when_some(offering.qualifier.clone(), |element, qualifier| {
                        element.child(
                            text(theme, TypeScale::Code, qualifier)
                                .text_tone(theme, TextTone::Faint)
                                .font_family(theme.typography.mono.clone()),
                        )
                    }),
            )
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Row)
                    .parent(server_ident.child("offerings").semantic_id())
                    .text(offering.name.clone())
                    .value(offering.kind.name()),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_publishes_its_own_name() {
        let names: Vec<&str> = [
            ServerState::Connected,
            ServerState::Connecting,
            ServerState::Disconnected,
            ServerState::Failed {
                reason: SharedString::new_static("x"),
            },
            ServerState::Disabled { reason: None },
        ]
        .iter()
        .map(ServerState::name)
        .collect();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len(), "two states share a name");
    }

    #[test]
    fn an_empty_answer_is_not_an_unasked_question() {
        assert_ne!(Catalog::Offers(Vec::new()), Catalog::Unasked);
        assert_eq!(Catalog::default(), Catalog::Unasked);
    }

    #[test]
    fn only_a_failure_and_a_stated_refusal_carry_a_reason() {
        assert!(ServerState::Connected.reason().is_none());
        assert!(ServerState::Disconnected.reason().is_none());
        assert!(ServerState::Disabled { reason: None }.reason().is_none());
        assert!(
            ServerState::Failed {
                reason: SharedString::new_static("no route")
            }
            .reason()
            .is_some()
        );
    }
}

#[cfg(test)]
mod server_phase_tests {
    use super::*;

    #[test]
    fn connected_is_ready_and_disabled_is_unavailable() {
        assert_eq!(ServerState::Connected.phase(), Phase::Ready);
        assert_eq!(ServerState::Connecting.phase(), Phase::Loading);
        assert_eq!(ServerState::Disconnected.phase(), Phase::Idle);
        assert_eq!(
            ServerState::Failed {
                reason: "timeout".into()
            }
            .phase(),
            Phase::Error
        );
        let disabled = ServerState::Disabled {
            reason: Some("reader turned it off".into()),
        };
        assert_eq!(disabled.phase(), Phase::Unavailable);
        assert_eq!(HasPhase::reason(&disabled), Some("reader turned it off"));
    }
}
