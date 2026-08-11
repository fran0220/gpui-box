//! A product-neutral diagnostic surface over caller-owned data.
//!
//! This component reports selection, filter, action, and retry intents. It
//! never changes the supplied diagnostics, opens a location, or runs a fix.

use std::rc::Rc;

use gpui::{App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, px};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TextTone, TypeScale};

use crate::controls::button::Button;
use crate::controls::filter_bar::{FilterBar, FilterCondition, ResultCount};
use crate::controls::toggle_button::Toggle;
use crate::data::list::{List, ListItem};
use crate::display::badge::{Badge, Tone};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::loading::PulseLoader;
use crate::foundation::{Disableable, Ident, Sizable, StyledExt, text};
use crate::state::Loadable;
use crate::strings::{ActiveStrings, StringKey};

type FilterHandler = Rc<dyn Fn(DiagnosticFilter, &mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(SharedString, SharedString, &mut Window, &mut App)>;
type RetryHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Severity assigned by the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticSeverity {
    pub const ALL: [Self; 4] = [Self::Error, Self::Warning, Self::Information, Self::Hint];

    pub fn name(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Information => "information",
            Self::Hint => "hint",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            Self::Error => Tone::Danger,
            Self::Warning => Tone::Warning,
            Self::Information => Tone::Info,
            Self::Hint => Tone::Neutral,
        }
    }

    fn label(self, cx: &App) -> SharedString {
        cx.strings().text(match self {
            Self::Error => StringKey::DiagnosticsSeverityError,
            Self::Warning => StringKey::DiagnosticsSeverityWarning,
            Self::Information => StringKey::DiagnosticsSeverityInformation,
            Self::Hint => StringKey::DiagnosticsSeverityHint,
        })
    }

    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// Caller-formatted location text. No path interpretation is performed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticLocation(SharedString);

impl DiagnosticLocation {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self(label.into())
    }
    pub fn label(&self) -> &SharedString {
        &self.0
    }
}

/// A caller-owned action that can be reported for a diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticAction {
    id: SharedString,
    label: SharedString,
    disabled: bool,
}

impl DiagnosticAction {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
        }
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn id(&self) -> &SharedString {
        &self.id
    }
    pub fn label(&self) -> &SharedString {
        &self.label
    }
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
}

/// One caller-owned diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    id: SharedString,
    severity: DiagnosticSeverity,
    location: DiagnosticLocation,
    message: SharedString,
    actions: Vec<DiagnosticAction>,
    disabled: bool,
}

impl Diagnostic {
    pub fn new(
        id: impl Into<SharedString>,
        severity: DiagnosticSeverity,
        location: DiagnosticLocation,
        message: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            severity,
            location,
            message: message.into(),
            actions: Vec::new(),
            disabled: false,
        }
    }
    pub fn action(mut self, action: DiagnosticAction) -> Self {
        self.actions.push(action);
        self
    }
    pub fn actions(mut self, actions: impl IntoIterator<Item = DiagnosticAction>) -> Self {
        self.actions.extend(actions);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn id(&self) -> &SharedString {
        &self.id
    }
    pub fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }
    pub fn location(&self) -> &DiagnosticLocation {
        &self.location
    }
    pub fn message(&self) -> &SharedString {
        &self.message
    }
    pub fn diagnostic_actions(&self) -> &[DiagnosticAction] {
        &self.actions
    }
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
}

/// Selected severities, stored as a compact bit set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticFilter(u8);

impl Default for DiagnosticFilter {
    fn default() -> Self {
        Self::all()
    }
}

impl DiagnosticFilter {
    const ALL_BITS: u8 = 0b1111;
    pub const fn all() -> Self {
        Self(Self::ALL_BITS)
    }
    pub const fn none() -> Self {
        Self(0)
    }
    pub fn from_severities(values: impl IntoIterator<Item = DiagnosticSeverity>) -> Self {
        Self(
            values
                .into_iter()
                .fold(0, |bits, severity| bits | severity.bit()),
        )
    }
    pub const fn contains(self, severity: DiagnosticSeverity) -> bool {
        self.0 & severity.bit() != 0
    }
    pub fn includes(self, diagnostic: &Diagnostic) -> bool {
        self.contains(diagnostic.severity)
    }
    pub const fn removing(self, severity: DiagnosticSeverity) -> Self {
        Self(self.0 & !severity.bit())
    }
    pub const fn clearing(self) -> Self {
        Self::none()
    }
    const fn setting(self, severity: DiagnosticSeverity, included: bool) -> Self {
        if included {
            Self(self.0 | severity.bit())
        } else {
            Self(self.0 & !severity.bit())
        }
    }
    pub fn severities(self) -> impl Iterator<Item = DiagnosticSeverity> {
        DiagnosticSeverity::ALL
            .into_iter()
            .filter(move |severity| self.contains(*severity))
    }
}

/// A diagnostic list whose data and state remain owned by its caller.
#[derive(IntoElement)]
pub struct DiagnosticsList {
    ident: Ident,
    diagnostics: Loadable<Vec<Diagnostic>, SharedString>,
    filter: DiagnosticFilter,
    selected: Option<SharedString>,
    visible_rows: Option<usize>,
    size: ControlSize,
    disabled: bool,
    on_filter: Option<FilterHandler>,
    on_select: Option<SelectHandler>,
    on_action: Option<ActionHandler>,
    on_retry: Option<RetryHandler>,
}

impl std::fmt::Debug for DiagnosticsList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DiagnosticsList")
            .field("ident", &self.ident)
            .field("diagnostics", &self.diagnostics)
            .field("filter", &self.filter)
            .field("selected", &self.selected)
            .field("visible_rows", &self.visible_rows)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl DiagnosticsList {
    pub fn new(
        ident: impl Into<Ident>,
        diagnostics: Loadable<Vec<Diagnostic>, SharedString>,
    ) -> Self {
        Self {
            ident: ident.into(),
            diagnostics,
            filter: DiagnosticFilter::default(),
            selected: None,
            visible_rows: None,
            size: ControlSize::Md,
            disabled: false,
            on_filter: None,
            on_select: None,
            on_action: None,
            on_retry: None,
        }
    }
    pub fn filter(mut self, filter: DiagnosticFilter) -> Self {
        self.filter = filter;
        self
    }
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }
    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = Some(rows);
        self
    }
    pub fn on_filter(
        mut self,
        handler: impl Fn(DiagnosticFilter, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_filter = Some(Rc::new(handler));
        self
    }
    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
    pub fn on_action(
        mut self,
        handler: impl Fn(SharedString, SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
    pub fn on_retry(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(Rc::new(handler));
        self
    }
}

impl Disableable for DiagnosticsList {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}
impl Sizable for DiagnosticsList {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for DiagnosticsList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ident = self.ident.clone();
        let state = match self.diagnostics {
            Loadable::Idle => EmptyState::new(
                ident.child("idle"),
                cx.strings().text(StringKey::DiagnosticsUnstarted),
            )
            .kind(EmptyKind::Unstarted)
            .into_any_element(),
            Loadable::Loading => PulseLoader::new(ident.child("loading"))
                .label(cx.strings().text(StringKey::Loading))
                .into_any_element(),
            Loadable::Empty => EmptyState::new(
                ident.child("empty"),
                cx.strings().text(StringKey::DiagnosticsEmpty),
            )
            .kind(EmptyKind::Empty)
            .into_any_element(),
            Loadable::Unavailable(reason) => EmptyState::new(
                ident.child("unavailable"),
                cx.strings().text(StringKey::DiagnosticsUnavailable),
            )
            .kind(EmptyKind::Unavailable)
            .detail(reason)
            .into_any_element(),
            Loadable::Error(reason) => {
                let mut empty = EmptyState::new(
                    ident.child("error"),
                    cx.strings().text(StringKey::DiagnosticsError),
                )
                .kind(EmptyKind::Failed)
                .detail(reason);
                if let Some(handler) = self.on_retry.clone().filter(|_| !self.disabled) {
                    empty = empty.action(
                        Button::new(ident.child("retry"))
                            .label(cx.strings().text(StringKey::TryAgain))
                            .semantic_parent(ident.child("error").semantic_id())
                            .control_size(self.size)
                            .on_click(move |window, cx| handler(window, cx)),
                    );
                }
                empty.into_any_element()
            }
            Loadable::Ready(diagnostics) if diagnostics.is_empty() => EmptyState::new(
                ident.child("empty"),
                cx.strings().text(StringKey::DiagnosticsEmpty),
            )
            .kind(EmptyKind::Empty)
            .into_any_element(),
            Loadable::Ready(diagnostics) => {
                let rows: Vec<_> = diagnostics
                    .into_iter()
                    .filter(|diagnostic| self.filter.includes(diagnostic))
                    .collect();
                let filter_handler = self.on_filter.clone().filter(|_| !self.disabled);
                let conditions = self.filter.severities().map(|severity| {
                    FilterCondition::new(
                        severity.name(),
                        cx.strings().text(StringKey::DiagnosticsFilterField),
                        cx.strings().text(StringKey::DiagnosticsFilterOperator),
                        severity.label(cx),
                    )
                    .tone(severity.tone())
                });
                let filter_control = div()
                    .flex()
                    .flex_wrap()
                    .gap(px(cx.theme().space(Space::Xs)))
                    .children(DiagnosticSeverity::ALL.map(|severity| {
                        let mut toggle =
                            Toggle::new(ident.child("filters.severities").child(severity.name()))
                                .label(severity.label(cx))
                                .pressed(self.filter.contains(severity))
                                .control_size(self.size)
                                .disabled(self.disabled)
                                .semantic_parent(ident.child("filters").semantic_id());
                        if let Some(handler) = filter_handler.clone() {
                            let filter = self.filter;
                            toggle = toggle.on_press(move |included, window, cx| {
                                handler(filter.setting(severity, included), window, cx);
                            });
                        }
                        toggle
                    }));
                let mut bar = FilterBar::new(ident.child("filters"))
                    .conditions(conditions)
                    .add_control(filter_control)
                    .count(ResultCount::Known(rows.len()))
                    .control_size(self.size)
                    .disabled(self.disabled);
                if let Some(handler) = filter_handler.clone() {
                    let filter = self.filter;
                    bar = bar.on_remove(move |id, window, cx| {
                        if let Some(severity) = DiagnosticSeverity::ALL
                            .into_iter()
                            .find(|severity| severity.name() == id.as_ref())
                        {
                            handler(filter.removing(severity), window, cx);
                        }
                    });
                }
                if let Some(handler) = filter_handler {
                    let filter = self.filter;
                    bar = bar.on_clear(move |window, cx| handler(filter.clearing(), window, cx));
                }
                if rows.is_empty() {
                    div()
                        .flex()
                        .flex_col()
                        .child(bar)
                        .child(
                            EmptyState::new(
                                ident.child("no-match"),
                                cx.strings().text(StringKey::DiagnosticsNoMatch),
                            )
                            .kind(EmptyKind::Empty),
                        )
                        .into_any_element()
                } else {
                    let rows = Rc::new(rows);
                    let action_handler = self.on_action.clone().filter(|_| !self.disabled);
                    let row_ident = ident.child("list");
                    let size = self.size;
                    let disabled = self.disabled;
                    let rows_for_render = Rc::clone(&rows);
                    let mut list = List::new(row_ident.clone(), rows.len(), move |index, _, cx| {
                        let diagnostic = &rows_for_render[index];
                        let diagnostic_id = diagnostic.id.clone();
                        let item_ident = row_ident.child(diagnostic.id.as_ref());
                        let actions = diagnostic.actions.iter().map(|action| {
                            let mut button =
                                Button::new(item_ident.child("action").child(action.id.as_ref()))
                                    .label(action.label.clone())
                                    .semantic_parent(item_ident.semantic_id())
                                    .ghost()
                                    .control_size(size)
                                    .disabled(disabled || diagnostic.disabled || action.disabled);
                            if let Some(handler) = action_handler
                                .clone()
                                .filter(|_| !diagnostic.disabled && !action.disabled)
                            {
                                let diagnostic_id = diagnostic_id.clone();
                                let action_id = action.id.clone();
                                button = button.on_click(move |window, cx| {
                                    cx.stop_propagation();
                                    handler(diagnostic_id.clone(), action_id.clone(), window, cx);
                                });
                            }
                            button
                        });
                        let content = div()
                            .flex()
                            .items_center()
                            .gap(px(cx.theme().space(Space::Sm)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .child(
                                        text(
                                            cx.theme(),
                                            TypeScale::Caption,
                                            diagnostic.location.0.clone(),
                                        )
                                        .text_tone(cx.theme(), TextTone::Muted),
                                    )
                                    .child(text(
                                        cx.theme(),
                                        TypeScale::Body,
                                        diagnostic.message.clone(),
                                    )),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .child(
                                        Badge::new(diagnostic.severity.label(cx))
                                            .tone(diagnostic.severity.tone()),
                                    )
                                    .semantic_in(
                                        cx,
                                        NodeSpec::new(
                                            item_ident.child("severity").semantic_id(),
                                            Role::Status,
                                        )
                                        .parent(item_ident.semantic_id())
                                        .text(diagnostic.severity.label(cx)),
                                    ),
                            )
                            .children(actions);
                        ListItem::new(diagnostic.id.clone(), content)
                            .text(diagnostic.message.clone())
                            .disabled(disabled || diagnostic.disabled)
                    })
                    .row_height(cx.theme().control.get(self.size).height * 2.0)
                    .control_size(self.size)
                    .disabled(self.disabled);
                    if let Some(selected) = self.selected {
                        list = list.selected(selected);
                    }
                    if let Some(rows) = self.visible_rows {
                        list = list.visible_rows(rows);
                    }
                    if let Some(handler) = self.on_select.filter(|_| !self.disabled) {
                        list = list.on_select(move |id, window, cx| handler(id, window, cx));
                    }
                    div()
                        .flex()
                        .flex_col()
                        .child(bar)
                        .child(list)
                        .into_any_element()
                }
            }
        };
        div()
            .w_full()
            .child(state)
            .semantic_in(cx, NodeSpec::new(ident.semantic_id(), Role::Group))
    }
}
