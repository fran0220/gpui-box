//! What is currently permitted, across a set of subjects and actions.
//!
//! # A cell has four states, not two
//!
//! [`PermissionState`] keeps `NotApplicable` apart from `Denied`. "This tool
//! has no network to reach" and "this tool is refused the network" are
//! different sentences: the first says the question does not arise, the
//! second says somebody answered it no. Drawing them the same way would
//! invent a refusal nobody made, so they are different states, different
//! wording, and different published values.
//!
//! # Where a permission came from
//!
//! A permission inherited from a broader rule is not the same as one set in
//! this matrix, and a reader who cannot tell them apart cannot tell what
//! changing a cell would actually do. The component therefore *carries and
//! shows* provenance through [`PermissionSource`], and *derives* none of it:
//! working out which rule won is policy evaluation over a rule set the host
//! owns, which is exactly the kind of fact this library takes as an input
//! rather than computing. Every cell says either where it was inherited from,
//! in the host's own words, or that it was set here.
//!
//! # Read-only and editable
//!
//! A matrix with no `on_change` handler is read-only: its cells publish
//! [`Role::Cell`] and install nothing. A cell whose state is `NotApplicable`
//! installs nothing either, in an editable matrix as much as a read-only one,
//! because there is no state for it to cycle to.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Theme, TypeScale};

use crate::foundation::{FocusRing, Ident, StyledExt};
use crate::strings::{ActiveStrings, StringKey};

type ChangeHandler = Rc<dyn Fn(PermissionChange, &mut Window, &mut App)>;

/// What a cell says about one subject and one action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionState {
    /// Goes ahead without asking.
    Allowed,
    /// Refused. Somebody decided this.
    Denied,
    /// Permitted only after somebody says so, every time.
    #[default]
    Ask,
    /// The question does not arise here. Never a refusal.
    NotApplicable,
}

impl PermissionState {
    /// The stable name published in the semantic tree.
    pub fn name(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Denied => "denied",
            Self::Ask => "ask",
            Self::NotApplicable => "not-applicable",
        }
    }

    /// The wording a reader reads.
    pub fn label(self, cx: &App) -> SharedString {
        cx.strings().text(match self {
            Self::Allowed => StringKey::PermissionAllowed,
            Self::Denied => StringKey::PermissionDenied,
            Self::Ask => StringKey::PermissionAsk,
            Self::NotApplicable => StringKey::PermissionNotApplicable,
        })
    }

    /// What operating the cell would ask for next.
    ///
    /// `NotApplicable` has no next state: a question that does not arise
    /// cannot be answered by clicking.
    pub fn next(self) -> Option<Self> {
        match self {
            Self::Allowed => Some(Self::Ask),
            Self::Ask => Some(Self::Denied),
            Self::Denied => Some(Self::Allowed),
            Self::NotApplicable => None,
        }
    }

    fn color(self, theme: &Theme) -> gpui::Hsla {
        match self {
            Self::Allowed => theme.colors.success,
            Self::Denied => theme.colors.danger,
            Self::Ask => theme.colors.warning,
            Self::NotApplicable => theme.colors.text_faint,
        }
    }
}

/// Where a cell's state was decided.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PermissionSource {
    /// Set on this subject, for this action.
    #[default]
    Here,
    /// Inherited from a broader rule, named in the host's own words.
    Inherited(SharedString),
}

impl PermissionSource {
    pub fn inherited(from: impl Into<SharedString>) -> Self {
        Self::Inherited(from.into())
    }

    /// The stable name published in the semantic tree.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Here => "here",
            Self::Inherited(_) => "inherited",
        }
    }

    pub fn label(&self, cx: &App) -> SharedString {
        match self {
            Self::Here => cx.strings().text(StringKey::PermissionSetHere),
            Self::Inherited(from) => cx.strings().format(StringKey::PermissionInherited, &[from]),
        }
    }
}

/// One cell: a state, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionEntry {
    state: PermissionState,
    source: PermissionSource,
}

impl PermissionEntry {
    /// A state decided in this matrix.
    pub fn new(state: PermissionState) -> Self {
        Self {
            state,
            source: PermissionSource::Here,
        }
    }

    /// A state a broader rule decided, naming that rule.
    pub fn inherited(state: PermissionState, from: impl Into<SharedString>) -> Self {
        Self {
            state,
            source: PermissionSource::inherited(from),
        }
    }

    pub fn state(&self) -> PermissionState {
        self.state
    }

    pub fn source(&self) -> &PermissionSource {
        &self.source
    }
}

/// One column: an action, addressed by a stable key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionAction {
    key: SharedString,
    label: SharedString,
}

impl PermissionAction {
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
        }
    }

    pub fn key(&self) -> &SharedString {
        &self.key
    }
}

/// One row: whatever the permissions are about, and its cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionSubject {
    id: SharedString,
    label: SharedString,
    cells: Vec<(SharedString, PermissionEntry)>,
}

impl PermissionSubject {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            cells: Vec::new(),
        }
    }

    /// States one cell. An action this subject never states is rendered
    /// `NotApplicable`, which is the honest reading of a row that does not
    /// mention it — and is still not a refusal.
    pub fn cell(mut self, action: impl Into<SharedString>, entry: PermissionEntry) -> Self {
        self.cells.push((action.into(), entry));
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    fn entry(&self, action: &SharedString) -> PermissionEntry {
        self.cells
            .iter()
            .find(|(key, _)| key == action)
            .map(|(_, entry)| entry.clone())
            .unwrap_or_else(|| PermissionEntry::new(PermissionState::NotApplicable))
    }
}

/// What operating a cell asks for. Nothing is applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionChange {
    pub subject: SharedString,
    pub action: SharedString,
    pub next: PermissionState,
}

/// A grid of subjects against actions.
#[derive(IntoElement)]
pub struct PermissionMatrix {
    ident: Ident,
    actions: Vec<PermissionAction>,
    subjects: Vec<PermissionSubject>,
    on_change: Option<ChangeHandler>,
}

impl std::fmt::Debug for PermissionMatrix {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PermissionMatrix")
            .field("ident", &self.ident)
            .field("actions", &self.actions.len())
            .field("subjects", &self.subjects.len())
            .field("editable", &self.on_change.is_some())
            .finish()
    }
}

impl PermissionMatrix {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            actions: Vec::new(),
            subjects: Vec::new(),
            on_change: None,
        }
    }

    pub fn action(mut self, action: PermissionAction) -> Self {
        self.actions.push(action);
        self
    }

    pub fn actions(mut self, actions: impl IntoIterator<Item = PermissionAction>) -> Self {
        self.actions.extend(actions);
        self
    }

    pub fn subject(mut self, subject: PermissionSubject) -> Self {
        self.subjects.push(subject);
        self
    }

    pub fn subjects(mut self, subjects: impl IntoIterator<Item = PermissionSubject>) -> Self {
        self.subjects.extend(subjects);
        self
    }

    /// Makes the matrix editable. Without a handler it is read-only, and a
    /// read-only cell installs nothing rather than installing a handler that
    /// does nothing.
    pub fn on_change(
        mut self,
        handler: impl Fn(PermissionChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PermissionMatrix {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let heading = div()
            .row()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .px_token(&theme, Space::Sm)
            .py_token(&theme, Space::Xs)
            .type_scale(&theme, TypeScale::Caption)
            .text_color(theme.colors.text_faint)
            .child(
                div()
                    .w(px(160.0))
                    .flex_none()
                    .child(cx.strings().text(StringKey::PermissionSubjectHeading)),
            )
            .children(
                self.actions
                    .iter()
                    .map(|action| div().flex_1().min_w_0().child(action.label.clone())),
            );

        let rows: Vec<_> = self
            .subjects
            .iter()
            .map(|subject| {
                let row_ident = self.ident.child(subject.id.as_ref());
                div()
                    .row()
                    .items_start()
                    .w_full()
                    .gap_token(&theme, Space::Sm)
                    .px_token(&theme, Space::Sm)
                    .py_token(&theme, Space::Xs)
                    .child(
                        div()
                            .w(px(160.0))
                            .flex_none()
                            .type_scale(&theme, TypeScale::Label)
                            .text_color(theme.colors.text)
                            .child(subject.label.clone())
                            .semantic_in(
                                cx,
                                NodeSpec::new(row_ident.semantic_id(), Role::Row)
                                    .text(subject.label.clone())
                                    .parent(self.ident.semantic_id()),
                            ),
                    )
                    .children(self.actions.iter().map(|action| {
                        cell(
                            &theme,
                            &row_ident,
                            subject,
                            action,
                            self.on_change.clone(),
                            cx,
                        )
                    }))
            })
            .collect();

        div()
            .column()
            .w_full()
            .radius(&theme, Radius::Card)
            .hairline(&theme)
            .child(heading)
            .children(rows)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Table)
                    .value(SharedString::from(self.subjects.len().to_string())),
            )
    }
}

fn cell(
    theme: &Theme,
    row_ident: &Ident,
    subject: &PermissionSubject,
    action: &PermissionAction,
    on_change: Option<ChangeHandler>,
    cx: &App,
) -> gpui::AnyElement {
    let entry = subject.entry(&action.key);
    let state = entry.state();
    let ident = row_ident.child(action.key.as_ref());
    let name = cx.strings().format(
        StringKey::PermissionCellName,
        &[&action.label, &state.label(cx)],
    );
    let next = state.next();
    let handler = on_change.zip(next);

    let mark = div()
        .row()
        .gap_token(theme, Space::Xs)
        .child(
            div()
                .flex_none()
                .size(px(7.0))
                .rounded_full()
                .bg(state.color(theme)),
        )
        .child(state.label(cx));

    // Provenance is shown on every cell that has a state, so "set here" is a
    // statement rather than the absence of one.
    let source = (state != PermissionState::NotApplicable).then(|| {
        div()
            .type_scale(theme, TypeScale::Caption)
            .text_color(theme.colors.text_faint)
            .child(entry.source().label(cx))
            .semantic_in(
                cx,
                NodeSpec::new(ident.child("source").semantic_id(), Role::Text)
                    .text(entry.source().label(cx))
                    .value(SharedString::new_static(entry.source().name()))
                    .parent(ident.semantic_id()),
            )
    });

    let body = div()
        .column()
        .gap_token(theme, Space::Xs)
        .type_scale(theme, TypeScale::Label)
        .text_color(theme.colors.text)
        .child(mark)
        .children(source);

    let spec = NodeSpec::new(
        ident.semantic_id(),
        if handler.is_some() {
            Role::Button
        } else {
            Role::Cell
        },
    )
    .text(name)
    .value(SharedString::new_static(state.name()))
    .parent(row_ident.semantic_id());

    // Both arms carry a border so the two matrices line up; only the editable
    // one is drawn. Hover and the focus ring arrive too late to answer "can I
    // change this?", and for a permission that question has to be answerable
    // at rest.
    let frame = div()
        .flex_1()
        .min_w_0()
        .px_token(theme, Space::Sm)
        .py_token(theme, Space::Xs)
        .border_1()
        .radius(theme, Radius::Control);

    match handler {
        Some((handler, next)) => {
            let subject_id = subject.id.clone();
            let action_key = action.key.clone();
            frame
                .id(ident.element_id())
                .tab_index(0)
                .cursor_pointer()
                .border_color(theme.colors.hairline)
                .hover(|style| style.bg(theme.colors.hover))
                .focus_ring(theme)
                .on_click(move |_event, window, cx| {
                    handler(
                        PermissionChange {
                            subject: subject_id.clone(),
                            action: action_key.clone(),
                            next,
                        },
                        window,
                        cx,
                    );
                })
                .child(body)
                .semantic_in(cx, spec)
                .into_any_element()
        }
        None => frame
            .border_color(gpui::transparent_black())
            .child(body)
            .semantic_in(cx, spec)
            .into_any_element(),
    }
}
