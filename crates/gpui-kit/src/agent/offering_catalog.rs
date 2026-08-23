//! Searchable offerings aggregated across caller-owned server sources.
//!
//! This component displays what callers already know. It does not discover,
//! install, invoke, trust, permit, or connect to anything. A result is always
//! identified by both its server and offering because names and offering ids
//! are only unique within one server.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, TextTone, TypeScale};

use crate::agent::server_list::{Offering, OfferingKind};
use crate::display::badge::{Badge, Tone};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::status::StatusDot;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{
    CardVariant, Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt, text,
};
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

type ActivateHandler = Rc<dyn Fn(OfferingIdentity, &mut Window, &mut App)>;

/// Stable identity for one server-owned offering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferingIdentity {
    pub server_id: SharedString,
    pub offering_id: SharedString,
}

impl OfferingIdentity {
    pub fn new(server_id: impl Into<SharedString>, offering_id: impl Into<SharedString>) -> Self {
        Self {
            server_id: server_id.into(),
            offering_id: offering_id.into(),
        }
    }
}

/// An offering and the caller-authored text used to search it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchableOffering {
    offering: Offering,
    searchable_text: SharedString,
}

impl SearchableOffering {
    pub fn new(offering: Offering, searchable_text: impl Into<SharedString>) -> Self {
        Self {
            offering,
            searchable_text: searchable_text.into(),
        }
    }

    pub fn offering(&self) -> &Offering {
        &self.offering
    }
}

/// What is truthfully known about one source's offerings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfferingSourceState {
    Loading,
    Empty,
    Unavailable(SharedString),
    Error(SharedString),
    Ready(Vec<SearchableOffering>),
    /// The source's last verified results remain visible beside the failure.
    Stale {
        offerings: Vec<SearchableOffering>,
        reason: SharedString,
    },
}

impl OfferingSourceState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Unavailable(_) => "unavailable",
            Self::Error(_) => "error",
            Self::Ready(offerings) if offerings.is_empty() => "empty",
            Self::Ready(_) => "ready",
            Self::Stale { .. } => "stale",
        }
    }

    fn offerings(&self) -> &[SearchableOffering] {
        match self {
            Self::Ready(offerings) | Self::Stale { offerings, .. } => offerings,
            _ => &[],
        }
    }
}

impl HasPhase for OfferingSourceState {
    fn phase(&self) -> Phase {
        match self {
            Self::Loading => Phase::Loading,
            Self::Empty => Phase::Empty,
            Self::Unavailable(_) => Phase::Unavailable,
            Self::Error(_) | Self::Stale { .. } => Phase::Error,
            Self::Ready(_) => Phase::Ready,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(reason) | Self::Error(reason) | Self::Stale { reason, .. } => {
                Some(reason.as_ref())
            }
            _ => None,
        }
    }

    fn is_stale(&self) -> bool {
        matches!(self, Self::Stale { .. })
    }
}

/// One attributed source and its independently truthful state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferingSource {
    id: SharedString,
    name: SharedString,
    state: OfferingSourceState,
}

impl OfferingSource {
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        state: OfferingSourceState,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            state,
        }
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn state(&self) -> &OfferingSourceState {
        &self.state
    }
}

/// A searchable, kind-filterable catalog of offerings from multiple servers.
#[derive(IntoElement)]
pub struct OfferingCatalog {
    ident: Ident,
    sources: Vec<OfferingSource>,
    query: SharedString,
    kinds: Vec<OfferingKind>,
    selected: Option<OfferingIdentity>,
    size: ControlSize,
    disabled: bool,
    on_activate: Option<ActivateHandler>,
    slots: Slots,
}

impl std::fmt::Debug for OfferingCatalog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OfferingCatalog")
            .field("ident", &self.ident)
            .field("sources", &self.sources)
            .field("query", &self.query)
            .field("kinds", &self.kinds)
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl OfferingCatalog {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            sources: Vec::new(),
            query: SharedString::default(),
            kinds: Vec::new(),
            selected: None,
            size: ControlSize::Md,
            disabled: false,
            on_activate: None,
            slots: Slots::default(),
        }
    }

    pub fn source(mut self, source: OfferingSource) -> Self {
        self.sources.push(source);
        self
    }

    pub fn sources(mut self, sources: impl IntoIterator<Item = OfferingSource>) -> Self {
        self.sources.extend(sources);
        self
    }

    /// The caller-owned query matched against each offering's searchable text.
    pub fn query(mut self, query: impl Into<SharedString>) -> Self {
        self.query = query.into();
        self
    }

    /// Included kinds. An empty collection includes all kinds.
    pub fn kinds(mut self, kinds: impl IntoIterator<Item = OfferingKind>) -> Self {
        self.kinds = kinds.into_iter().collect();
        self
    }

    pub fn selected(mut self, identity: OfferingIdentity) -> Self {
        self.selected = Some(identity);
        self
    }

    /// Reports activation with both identities. The component performs no action.
    pub fn on_activate(
        mut self,
        handler: impl Fn(OfferingIdentity, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }
}

impl Disableable for OfferingCatalog {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for OfferingCatalog {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Slotted for OfferingCatalog {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for OfferingCatalog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state_name = self.aggregate_state();
        let busy = self
            .sources
            .iter()
            .any(|source| matches!(source.state, OfferingSourceState::Loading));
        let invalid = self
            .sources
            .iter()
            .any(|source| matches!(source.state, OfferingSourceState::Error(_)));
        let statuses: Vec<AnyElement> = self
            .sources
            .iter()
            .filter(|source| source.state.name() != "ready")
            .map(|source| self.source_status(source, &theme, cx))
            .collect();
        let body = self.results(&theme, window, cx);

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .p_token(&theme, Space::Sm)
            .card_surface(&theme, CardVariant::Elevated)
            .children(statuses)
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .value(state_name)
                    .busy(busy)
                    .invalid(invalid),
            )
    }
}

impl OfferingCatalog {
    fn aggregate_state(&self) -> &'static str {
        let Some(first) = self.sources.first().map(|source| source.state.name()) else {
            return "empty";
        };
        if self
            .sources
            .iter()
            .all(|source| source.state.name() == first)
        {
            first
        } else {
            "mixed"
        }
    }

    fn source_status(
        &self,
        source: &OfferingSource,
        theme: &gpui_kit_theme::Theme,
        cx: &mut App,
    ) -> AnyElement {
        let ident = self
            .ident
            .child("source")
            .child(encoded_segment(source.id.as_ref()));
        let (label_key, reason, tone) = match &source.state {
            OfferingSourceState::Loading => (StringKey::OfferingSourceLoading, None, Tone::Info),
            OfferingSourceState::Empty | OfferingSourceState::Ready(_) => {
                (StringKey::OfferingSourceEmpty, None, Tone::Neutral)
            }
            OfferingSourceState::Unavailable(reason) => (
                StringKey::OfferingSourceUnavailable,
                Some(reason.clone()),
                Tone::Neutral,
            ),
            OfferingSourceState::Error(reason) => (
                StringKey::OfferingSourceError,
                Some(reason.clone()),
                Tone::Danger,
            ),
            OfferingSourceState::Stale { reason, .. } => (
                StringKey::OfferingSourceStale,
                Some(reason.clone()),
                Tone::Warning,
            ),
        };
        let label = cx.strings().format(label_key, &[source.name.as_ref()]);
        div()
            .row()
            .w_full()
            .items_start()
            .gap_token(theme, Space::Sm)
            .p_token(theme, Space::Sm)
            .radius(theme, Radius::Control)
            .bg(tone.color(theme).opacity(0.12))
            .child(div().mt(px(4.0)).child(StatusDot::new(tone)))
            .child(
                div()
                    .column()
                    .min_w_0()
                    .child(
                        text(theme, TypeScale::Caption, label.clone())
                            .text_color(tone.color(theme)),
                    )
                    .children(reason.clone().map(|reason| {
                        text(theme, TypeScale::Body, reason).text_color(tone.color(theme))
                    })),
            )
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Status)
                    .parent(self.ident.semantic_id())
                    .text(label)
                    .value(source.state.name())
                    .description(reason.unwrap_or_default())
                    .busy(matches!(source.state, OfferingSourceState::Loading))
                    .invalid(matches!(source.state, OfferingSourceState::Error(_))),
            )
            .into_any_element()
    }

    fn results(
        &self,
        theme: &gpui_kit_theme::Theme,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let aggregate = self.aggregate_state();
        let replacement = match aggregate {
            "loading" => self.slots.render(slot::LOADING, window, cx),
            "error" => self.slots.render(slot::FAILED, window, cx),
            _ => None,
        };
        if let Some(replacement) = replacement {
            return replacement;
        }

        let query = self.query.to_lowercase();
        let filtered: Vec<(&OfferingSource, &SearchableOffering)> = self
            .sources
            .iter()
            .flat_map(|source| {
                source
                    .state
                    .offerings()
                    .iter()
                    .map(move |offering| (source, offering))
            })
            .filter(|(_, result)| {
                self.kinds.is_empty() || self.kinds.contains(&result.offering.kind())
            })
            .filter(|(_, result)| {
                query.is_empty() || result.searchable_text.to_lowercase().contains(&query)
            })
            .collect();
        if filtered.is_empty() {
            let key = if self
                .sources
                .iter()
                .any(|source| !source.state.offerings().is_empty())
            {
                StringKey::OfferingCatalogNoMatch
            } else {
                StringKey::OfferingCatalogEmpty
            };
            return self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(self.ident.child("empty"), cx.strings().text(key))
                    .kind(EmptyKind::Empty)
                    .into_any_element()
            });
        }

        let list_ident = self.ident.child("results");
        div()
            .column()
            .w_full()
            .gap_token(theme, Space::Xs)
            .children(
                filtered
                    .iter()
                    .map(|(source, result)| self.result(source, result, &list_ident, theme, cx)),
            )
            .semantic_in(
                cx,
                NodeSpec::new(list_ident.semantic_id(), Role::List)
                    .parent(self.ident.semantic_id())
                    .value(filtered.len().to_string()),
            )
            .into_any_element()
    }

    fn result(
        &self,
        source: &OfferingSource,
        result: &SearchableOffering,
        list_ident: &Ident,
        theme: &gpui_kit_theme::Theme,
        cx: &mut App,
    ) -> AnyElement {
        let identity = OfferingIdentity::new(source.id.clone(), result.offering.id().clone());
        let ident = list_ident
            .child(encoded_segment(identity.server_id.as_ref()))
            .child(encoded_segment(identity.offering_id.as_ref()));
        let selected = self.selected.as_ref() == Some(&identity);
        let actionable = !self.disabled && self.on_activate.is_some();
        let metrics = theme.control.get(self.size);
        let kind = result.offering.kind();
        let glyph = match kind {
            OfferingKind::Tool => Icon::Tuning,
            OfferingKind::Skill => Icon::Command,
            OfferingKind::Resource => Icon::Document,
        };
        let mut row = div()
            .id(ident.element_id())
            .row()
            .w_full()
            .items_start()
            .gap_token(theme, Space::Sm)
            .p_token(theme, Space::Sm)
            .radius(theme, Radius::Control)
            .when(selected, |element| element.bg(theme.colors.selected))
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .when(!selected, |element| {
                        element.hover(|style| style.bg(theme.colors.hover))
                    })
                    .focus_ring(theme)
            })
            .child(
                icon(glyph)
                    .size(px(metrics.icon_size))
                    .text_color(theme.colors.text_faint),
            )
            .child(
                div()
                    .column()
                    .flex_1()
                    .min_w_0()
                    .child(text(
                        theme,
                        TypeScale::Label,
                        result.offering.name().clone(),
                    ))
                    .when_some(
                        result.offering.summary_text().cloned(),
                        |element, summary| {
                            element.child(
                                text(theme, TypeScale::Body, summary)
                                    .text_tone(theme, TextTone::Muted),
                            )
                        },
                    )
                    .when_some(
                        result.offering.qualifier_text().cloned(),
                        |element, qualifier| {
                            element.child(
                                text(theme, TypeScale::Code, qualifier)
                                    .text_tone(theme, TextTone::Faint)
                                    .font_family(theme.typography.mono.clone()),
                            )
                        },
                    ),
            )
            .child(
                div()
                    .column()
                    .items_end()
                    .gap_token(theme, Space::Xs)
                    .child(Badge::new(kind.name()).tone(Tone::Neutral))
                    .child(
                        text(theme, TypeScale::Caption, source.name.clone())
                            .text_tone(theme, TextTone::Muted)
                            .semantic_in(
                                cx,
                                NodeSpec::new(ident.child("server").semantic_id(), Role::Status)
                                    .parent(ident.semantic_id())
                                    .text(source.name.clone())
                                    .value(source.id.clone()),
                            ),
                    ),
            );
        if let (true, Some(handler)) = (actionable, self.on_activate.clone()) {
            row = row.on_click(move |_, window, cx| handler(identity.clone(), window, cx));
        }
        row.semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Row)
                .parent(list_ident.semantic_id())
                .text(result.offering.name().clone())
                .value(kind.name())
                .selected(selected)
                .disabled(self.disabled),
        )
        .into_any_element()
    }
}

fn encoded_segment(value: &str) -> String {
    value.replace('%', "%25").replace('.', "%2E")
}

#[cfg(test)]
mod offering_phase_tests {
    use super::*;

    #[test]
    fn stale_projects_as_error_and_keeps_the_verified_offerings() {
        let state = OfferingSourceState::Stale {
            offerings: Vec::new(),
            reason: "offline".into(),
        };
        assert_eq!(state.phase(), Phase::Error);
        assert!(state.is_stale());
        assert_eq!(state.reason(), Some("offline"));
    }
}
