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
    Styled, StyledText, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, TypeScale};

use crate::content::ansi::{ansi_highlights, strip_ansi};
use crate::controls::button::Button;
use crate::data::{List, ListItem, scroll_to_row};
use crate::display::badge::{Badge, Tone};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::highlight::HighlightedText;
use crate::display::loading::Skeleton;
use crate::display::status::StatusDot;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Disableable, Ident, Sizable, StyledExt};
use crate::motion::keyed;
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

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
    compiled: Option<(SharedString, Vec<crate::content::ansi::AnsiRun>)>,
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
            compiled: None,
        }
    }

    /// Strips ANSI once so a virtualized row does not parse the same line
    /// again on every scroll.
    pub fn prepare_ansi(mut self) -> Self {
        self.compiled = Some(strip_ansi(self.message.as_ref()));
        self
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

impl HasPhase for LogStreamState {
    fn phase(&self) -> Phase {
        match self {
            Self::Loading => Phase::Loading,
            Self::Empty => Phase::Empty,
            Self::Unavailable(_) => Phase::Unavailable,
            Self::Error(_) | Self::Stale(_) => Phase::Error,
            Self::Ready => Phase::Ready,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(reason) | Self::Error(reason) | Self::Stale(reason) => {
                Some(reason.as_ref())
            }
            _ => None,
        }
    }

    fn is_stale(&self) -> bool {
        matches!(self, Self::Stale(_))
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
    ansi: bool,
    slots: Slots,
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
            ansi: false,
            slots: Slots::default(),
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

    /// Treats each message as ANSI-coloured text. Search hits still win.
    pub fn ansi(mut self, ansi: bool) -> Self {
        self.ansi = ansi;
        self
    }
}

impl Slotted for LogStream {
    const SLOTS: &'static [&'static str] =
        &[slot::EMPTY, slot::FAILED, slot::LOADING, slot::HEADER_EXTRA];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let entries = Rc::new(if self.ansi {
            self.entries
                .into_iter()
                .map(|entry| {
                    if entry.compiled.is_some() {
                        entry
                    } else {
                        entry.prepare_ansi()
                    }
                })
                .collect()
        } else {
            self.entries
        });
        let count = entries.len();
        let shows_entries = self.state.shows_entries();
        let list_ident = self.ident.child("entries");
        let follow = keyed::slot::<FollowState>(
            &self.ident.child("follow-state").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
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
            scroll_to_row(&list_ident, count - 1, window, cx);
        }

        let extra = self.slots.render(slot::HEADER_EXTRA, window, cx);
        let body = if shows_entries {
            entries_body(
                Rc::clone(&entries),
                &list_ident,
                &theme,
                self.visible_rows,
                self.selected.clone(),
                self.on_select.clone(),
                self.ansi,
            )
        } else {
            state_body(&self.ident, &self.state, &self.slots, window, cx)
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
                        scroll_to_row(&scroll_ident, last, window, cx);
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
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .gap_token(&theme, Space::Xs)
            .p_token(&theme, Space::Sm)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Raised, Elevation::Raised)
            .children(extra)
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
    ansi: bool,
) -> AnyElement {
    let rows = Rc::clone(&entries);
    let parent = list_ident.clone();
    let row_theme = theme.clone();
    let columns = Columns::of(&entries);
    let mut list = List::new(list_ident.clone(), entries.len(), move |index, _, cx| {
        let entry = &rows[index];
        ListItem::new(
            entry.id.clone(),
            // The row's position in the filtered stream is its reading order.
            // The list is virtualized, so a copy spanning rows that were never
            // mounted says so rather than dropping them silently.
            entry_row(
                &parent,
                entry,
                index as u64,
                true,
                columns,
                &row_theme,
                ansi,
                cx,
            ),
        )
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

/// Which of the fixed leading columns any entry in the stream actually fills.
///
/// A column no entry supplies is not drawn. Reserving its width regardless
/// leaves a gutter that reads as content the row failed to render, and pushes
/// the message — the only part of a log line a reader is looking for — away
/// from the edge it is scanned from.
#[derive(Debug, Clone, Copy)]
struct Columns {
    timestamp: bool,
    level: bool,
    source: bool,
}

impl Columns {
    fn of(entries: &[LogEntry]) -> Self {
        Self {
            timestamp: entries.iter().any(|entry| !entry.timestamp.is_empty()),
            level: entries.iter().any(|entry| !entry.level.is_empty()),
            source: entries.iter().any(|entry| !entry.source.is_empty()),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn entry_row(
    parent: &Ident,
    entry: &LogEntry,
    order: u64,
    virtualized: bool,
    columns: Columns,
    theme: &gpui_kit_theme::Theme,
    ansi: bool,
    cx: &App,
) -> AnyElement {
    let entry_ident = parent.child(entry.id.as_ref());
    let message_element = if ansi && entry.search_hits.is_empty() {
        let (plain, runs) = entry
            .compiled
            .clone()
            .unwrap_or_else(|| strip_ansi(entry.message.as_ref()));
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .whitespace_nowrap()
            .child(StyledText::new(plain).with_highlights(ansi_highlights(&runs, theme)))
            .into_any_element()
    } else {
        let mut message = HighlightedText::new(entry.message.clone())
            .selectable(entry_ident.child("message"))
            .in_document(order, virtualized)
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
        if drawn > 0 {
            message
                .semantic_in(
                    cx,
                    NodeSpec::new(entry_ident.child("hits").semantic_id(), Role::Status)
                        .parent(entry_ident.semantic_id())
                        .value(cx.numbers().count(drawn)),
                )
                .into_any_element()
        } else {
            message.into_any_element()
        }
    };

    div()
        .row()
        .items_center()
        .w_full()
        .h_full()
        .gap_token(theme, Space::Sm)
        .mono(theme)
        .text_size(px(theme.typography.code.size))
        .line_height(px(theme.typography.code.line_height))
        .when(columns.timestamp, |row| {
            row.child(
                div()
                    .flex_none()
                    .w(px(76.0))
                    .overflow_hidden()
                    .text_color(theme.colors.text_faint)
                    .child(entry.timestamp.clone()),
            )
        })
        .when(columns.level, |row| {
            // The column is a column so the levels line up; the pill inside it
            // stays the width of its own word, because a pill stretched to a
            // slot is claiming a length its label does not have.
            row.child(
                div()
                    .flex_none()
                    .row()
                    .items_center()
                    .w(px(68.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_none()
                            .child(Badge::new(entry.level.clone()).tone(entry.tone)),
                    ),
            )
        })
        .when(columns.source, |row| {
            row.child(
                div()
                    .flex_none()
                    .w(px(104.0))
                    .overflow_hidden()
                    .text_color(theme.colors.text_muted)
                    .child(entry.source.clone()),
            )
        })
        .child(message_element)
        .into_any_element()
}

fn state_body(
    ident: &Ident,
    state: &LogStreamState,
    slots: &Slots,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    match state {
        // A stream that has not arrived is drawn in the shape of the rows it
        // will arrive as, so waiting for a log and reading an empty one are
        // not the same rectangle with a different mark in the middle of it.
        LogStreamState::Loading => slots.or_else(slot::LOADING, window, cx, |_, cx| {
            let theme = cx.theme().clone();
            div()
                .column()
                .w_full()
                .gap_token(&theme, Space::Xs)
                .child(
                    Skeleton::new(ident.child("loading"))
                        .rows(4)
                        .row_height(theme.typography.code.line_height)
                        .widths([0.82, 0.64, 0.9, 0.38])
                        .label(cx.strings().text(StringKey::Loading)),
                )
                .child(
                    div()
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_faint)
                        .child(cx.strings().text(StringKey::Loading)),
                )
                .into_any_element()
        }),
        LogStreamState::Empty => slots.or_else(slot::EMPTY, window, cx, |_, cx| {
            EmptyState::new(ident.child("empty"), cx.strings().text(StringKey::LogEmpty))
                .kind(EmptyKind::Empty)
                .into_any_element()
        }),
        LogStreamState::Unavailable(reason) => slots.or_else(slot::EMPTY, window, cx, |_, cx| {
            EmptyState::new(
                ident.child("unavailable"),
                cx.strings().text(StringKey::LogUnavailable),
            )
            .kind(EmptyKind::Unavailable)
            .detail(reason.clone())
            .into_any_element()
        }),
        LogStreamState::Error(reason) => slots.or_else(slot::FAILED, window, cx, |_, cx| {
            EmptyState::new(ident.child("error"), cx.strings().text(StringKey::LogError))
                .kind(EmptyKind::Failed)
                .detail(reason.clone())
                .into_any_element()
        }),
        LogStreamState::Ready | LogStreamState::Stale(_) => div().into_any_element(),
    }
}

#[cfg(test)]
mod log_stream_phase_tests {
    use super::*;

    #[test]
    fn stale_projects_as_error_and_keeps_the_verified_entries() {
        let state = LogStreamState::Stale("offline".into());
        assert_eq!(state.phase(), Phase::Error);
        assert!(state.is_stale());
        assert_eq!(state.reason(), Some("offline"));
        assert!(state.shows_entries());
    }
}
