//! A fixed-row, virtualized presentation of caller-owned log entries.
//!
//! This component owns no process and reads no stream. The caller supplies
//! every entry, its stable identity, already-formatted timestamp, level and
//! source strings, and any search-hit ranges. Appending and filtering remain
//! caller operations.
//!
//! # One entry, one fixed slot
//!
//! [`LogStream`] composes the crate's [`List`], so only the
//! rows inside its bounded viewport are laid out. Every entry is one clipped,
//! non-wrapping slot. That makes a large stream viewport-cheap, at the explicit
//! cost that a long message does not grow its row. The list node publishes the
//! total while only visible entry identities appear in the semantic tree.
//!
//! # Following is view state
//!
//! Following the newest entry, and pausing that follow, change only where the
//! viewport is looking. They are transient visual state keyed by the stream's
//! identity. Selecting an entry and asking to copy one are caller-owned intents
//! and are only offered when the matching callbacks exist. Message text uses
//! GPUI's read-only selection primitive without adding caller content to the
//! Kit semantic snapshot; the explicit copy intent remains the whole-entry
//! operation.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, TypeScale};

use crate::controls::button::Button;
use crate::data::{List, ListItem, scroll_to_row};
use crate::display::badge::{Badge, Tone};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::highlight::HighlightedText;
use crate::display::loading::PulseLoader;
use crate::display::status::StatusDot;
use crate::foundation::{Disableable, Ident, Sizable, StyledExt};
use crate::motion::keyed;
use crate::strings::{ActiveStrings, StringKey};

type EntryHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// One caller-owned log entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub id: SharedString,
    pub timestamp: SharedString,
    pub level: SharedString,
    pub source: SharedString,
    pub message: SharedString,
    pub tone: Tone,
    pub search_hits: Vec<Range<usize>>,
    pub current_hit: Option<usize>,
}

impl LogEntry {
    pub fn new(id: impl Into<SharedString>, message: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            timestamp: SharedString::default(),
            level: SharedString::default(),
            source: SharedString::default(),
            message: message.into(),
            tone: Tone::Neutral,
            search_hits: Vec::new(),
            current_hit: None,
        }
    }

    /// A timestamp the caller already formatted.
    pub fn timestamp(mut self, timestamp: impl Into<SharedString>) -> Self {
        self.timestamp = timestamp.into();
        self
    }

    /// The caller's level name and the semantic tone it should carry.
    pub fn level(mut self, level: impl Into<SharedString>, tone: Tone) -> Self {
        self.level = level.into();
        self.tone = tone;
        self
    }

    /// The caller's source name. Nothing here interprets it.
    pub fn source(mut self, source: impl Into<SharedString>) -> Self {
        self.source = source.into();
        self
    }

    /// Search hits as sorted, non-overlapping UTF-8 byte ranges into
    /// [`LogEntry::message`]. Invalid ranges are skipped by `HighlightedText`.
    pub fn search_hits(mut self, hits: impl IntoIterator<Item = Range<usize>>) -> Self {
        self.search_hits = hits.into_iter().collect();
        self
    }

    /// Which supplied search hit is current.
    pub fn current_hit(mut self, hit: usize) -> Self {
        self.current_hit = Some(hit);
        self
    }
}

/// What is known about the stream as a whole.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LogStreamState {
    Loading,
    Empty,
    Unavailable(SharedString),
    Error(SharedString),
    /// Entries are the last verified value; the text says why they are stale.
    Stale(SharedString),
    #[default]
    Ready,
}

impl LogStreamState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Unavailable(_) => "unavailable",
            Self::Error(_) => "error",
            Self::Stale(_) => "stale",
            Self::Ready => "ready",
        }
    }

    fn shows_entries(&self) -> bool {
        matches!(self, Self::Ready | Self::Stale(_))
    }
}

/// A virtualized, read-only log presentation.
#[derive(IntoElement)]
pub struct LogStream {
    ident: Ident,
    entries: Vec<LogEntry>,
    state: LogStreamState,
    visible_rows: usize,
    selected: Option<SharedString>,
    on_select: Option<EntryHandler>,
    on_copy: Option<EntryHandler>,
}

impl std::fmt::Debug for LogStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LogStream")
            .field("ident", &self.ident)
            .field("entries", &self.entries.len())
            .field("state", &self.state)
            .field("visible_rows", &self.visible_rows)
            .field("selected", &self.selected)
            .finish()
    }
}

impl LogStream {
    pub fn new(ident: impl Into<Ident>, entries: impl IntoIterator<Item = LogEntry>) -> Self {
        Self {
            ident: ident.into(),
            entries: entries.into_iter().collect(),
            state: LogStreamState::Ready,
            visible_rows: 12,
            selected: None,
            on_select: None,
            on_copy: None,
        }
    }

    pub fn state(mut self, state: LogStreamState) -> Self {
        self.state = state;
        self
    }

    /// Bounds and virtualizes the stream to this many fixed-height entries.
    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = rows.max(1);
        self
    }

    /// The caller-owned selected entry.
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    /// Reports an entry selection and changes no data.
    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Reports that the selected entry should be copied. The host decides what
    /// representation belongs on the clipboard.
    pub fn on_copy(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_copy = Some(Rc::new(handler));
        self
    }
}

#[derive(Debug, Default)]
struct FollowState {
    started: bool,
    count: usize,
    newest: Option<SharedString>,
    paused: bool,
}

impl RenderOnce for LogStream {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let entries = Rc::new(self.entries);
        let count = entries.len();
        let shows_entries = self.state.shows_entries();
        let list_ident = self.ident.child("entries");
        let follow =
            keyed::slot::<FollowState>(&self.ident.child("follow-state").semantic_id(), cx);
        let (paused, should_follow) = {
            let mut state = follow.borrow_mut();
            let newest = entries.last().map(|entry| entry.id.clone());
            let should_follow = shows_entries
                && count > 0
                && !state.paused
                && (!state.started || count != state.count || newest != state.newest);
            if shows_entries {
                state.started = true;
                state.count = count;
                state.newest = newest;
            }
            (state.paused, should_follow)
        };
        if should_follow {
            scroll_to_row(&list_ident, count - 1, cx);
        }

        let body = if shows_entries {
            entries_body(
                Rc::clone(&entries),
                &list_ident,
                &theme,
                self.visible_rows,
                self.selected.clone(),
                self.on_select.clone(),
            )
        } else {
            state_body(&self.ident, &self.state, cx)
        };

        let toolbar = shows_entries.then(|| {
            let last = count.saturating_sub(1);
            let follow_label = cx.strings().text(if paused {
                StringKey::LogFollow
            } else {
                StringKey::LogPause
            });
            let follow_cell = Rc::clone(&follow);
            let scroll_ident = list_ident.clone();
            let follow_button = Button::new(self.ident.child("follow"))
                .label(follow_label)
                .ghost()
                .control_size(ControlSize::Xs)
                .semantic_parent(self.ident.semantic_id())
                .checked_state(!paused)
                .on_click(move |window, cx| {
                    let mut state = follow_cell.borrow_mut();
                    state.paused = !state.paused;
                    let resumes = !state.paused;
                    drop(state);
                    if resumes && count > 0 {
                        scroll_to_row(&scroll_ident, last, cx);
                    }
                    window.refresh();
                });

            let copy_target = self
                .selected
                .as_ref()
                .filter(|selected| entries.iter().any(|entry| &entry.id == *selected));
            let copy_button = self.on_copy.as_ref().map(|handler| {
                let mut button = Button::new(self.ident.child("copy"))
                    .label(cx.strings().text(StringKey::Copy))
                    .ghost()
                    .control_size(ControlSize::Xs)
                    .semantic_parent(self.ident.semantic_id())
                    .disabled(copy_target.is_none());
                if let Some(target) = copy_target.cloned() {
                    let handler = Rc::clone(handler);
                    button = button.on_click(move |window, cx| handler(target.clone(), window, cx));
                }
                button
            });

            let mode_label = cx.strings().text(if paused {
                StringKey::LogPaused
            } else {
                StringKey::LogFollowing
            });
            div()
                .row()
                .w_full()
                .items_center()
                .justify_between()
                .gap_token(&theme, Space::Sm)
                .child(
                    div()
                        .row()
                        .items_center()
                        .gap_token(&theme, Space::Xs)
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(if paused {
                            theme.colors.warning
                        } else {
                            theme.colors.text_muted
                        })
                        .child(StatusDot::new(if paused {
                            Tone::Warning
                        } else {
                            Tone::Success
                        }))
                        .child(mode_label.clone())
                        .semantic_in(
                            cx,
                            NodeSpec::new(self.ident.child("mode").semantic_id(), Role::Status)
                                .parent(self.ident.semantic_id())
                                .text(mode_label)
                                .value(if paused { "paused" } else { "following" }),
                        ),
                )
                .child(
                    div()
                        .row()
                        .gap_token(&theme, Space::Xs)
                        .children(copy_button)
                        .child(follow_button),
                )
        });

        let stale = match &self.state {
            LogStreamState::Stale(reason) => Some(
                div()
                    .row()
                    .items_center()
                    .gap_token(&theme, Space::Xs)
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.warning)
                    .child(StatusDot::new(Tone::Warning))
                    .child(reason.clone())
                    .semantic_in(
                        cx,
                        NodeSpec::new(self.ident.child("stale").semantic_id(), Role::Status)
                            .parent(self.ident.semantic_id())
                            .text(reason.clone())
                            .value("stale"),
                    ),
            ),
            _ => None,
        };

        let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Region)
            .value(self.state.name())
            .busy(matches!(self.state, LogStreamState::Loading))
            .invalid(matches!(self.state, LogStreamState::Error(_)));
        if let LogStreamState::Unavailable(reason) | LogStreamState::Error(reason) = &self.state {
            spec = spec.description(reason.clone());
        }

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .p_token(&theme, Space::Sm)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Raised, Elevation::Raised)
            .children(toolbar)
            .children(stale)
            .child(body)
            .semantic_in(cx, spec)
    }
}

fn entries_body(
    entries: Rc<Vec<LogEntry>>,
    list_ident: &Ident,
    theme: &gpui_kit_theme::Theme,
    visible_rows: usize,
    selected: Option<SharedString>,
    on_select: Option<EntryHandler>,
) -> AnyElement {
    let rows = Rc::clone(&entries);
    let parent = list_ident.clone();
    let row_theme = theme.clone();
    let mut list = List::new(list_ident.clone(), entries.len(), move |index, _, cx| {
        let entry = &rows[index];
        ListItem::new(entry.id.clone(), entry_row(&parent, entry, &row_theme, cx))
            // Log payloads stay out of diagnostic snapshots. The caller's
            // level is enough to name the row without publishing the message.
            .text(entry.level.clone())
    })
    .row_height(theme.control.get(ControlSize::Sm).height)
    .visible_rows(visible_rows);
    if let Some(selected) = selected {
        list = list.selected(selected);
    }
    if let Some(handler) = on_select {
        list = list.on_select(move |id, window, cx| handler(id, window, cx));
    }
    list.into_any_element()
}

fn entry_row(
    parent: &Ident,
    entry: &LogEntry,
    theme: &gpui_kit_theme::Theme,
    cx: &App,
) -> AnyElement {
    let entry_ident = parent.child(entry.id.as_ref());
    let mut message = HighlightedText::new(entry.message.clone())
        .selectable(entry_ident.child("message"))
        .hits(entry.search_hits.clone())
        .monospace(true);
    if let Some(current) = entry.current_hit {
        message = message.current(current);
    }
    let drawn = message.published_hits();
    let message = div()
        .flex_1()
        .min_w_0()
        .h_full()
        .overflow_hidden()
        .whitespace_nowrap()
        .child(message);
    let message = if drawn > 0 {
        message
            .semantic_in(
                cx,
                NodeSpec::new(entry_ident.child("hits").semantic_id(), Role::Status)
                    .parent(entry_ident.semantic_id())
                    .value(drawn.to_string()),
            )
            .into_any_element()
    } else {
        message.into_any_element()
    };

    div()
        .row()
        .items_center()
        .w_full()
        .h_full()
        .gap_token(theme, Space::Sm)
        .font_family(theme.typography.mono.clone())
        .text_size(px(theme.typography.code.size))
        .line_height(px(theme.typography.code.line_height))
        .child(
            div()
                .flex_none()
                .w(px(76.0))
                .overflow_hidden()
                .text_color(theme.colors.text_faint)
                .child(entry.timestamp.clone()),
        )
        .child(
            div()
                .flex_none()
                .w(px(68.0))
                .overflow_hidden()
                .child(Badge::new(entry.level.clone()).tone(entry.tone)),
        )
        .child(
            div()
                .flex_none()
                .w(px(104.0))
                .overflow_hidden()
                .text_color(theme.colors.text_muted)
                .child(entry.source.clone()),
        )
        .child(message)
        .into_any_element()
}

fn state_body(ident: &Ident, state: &LogStreamState, cx: &App) -> AnyElement {
    let strings = cx.strings();
    match state {
        LogStreamState::Loading => div()
            .flex()
            .items_center()
            .justify_center()
            .w_full()
            .p(px(24.0))
            .child(PulseLoader::new(ident.child("loading")).label(strings.text(StringKey::Loading)))
            .into_any_element(),
        LogStreamState::Empty => {
            EmptyState::new(ident.child("empty"), strings.text(StringKey::LogEmpty))
                .kind(EmptyKind::Empty)
                .into_any_element()
        }
        LogStreamState::Unavailable(reason) => EmptyState::new(
            ident.child("unavailable"),
            strings.text(StringKey::LogUnavailable),
        )
        .kind(EmptyKind::Unavailable)
        .detail(reason.clone())
        .into_any_element(),
        LogStreamState::Error(reason) => {
            EmptyState::new(ident.child("error"), strings.text(StringKey::LogError))
                .kind(EmptyKind::Failed)
                .detail(reason.clone())
                .into_any_element()
        }
        LogStreamState::Ready | LogStreamState::Stale(_) => div().into_any_element(),
    }
}
