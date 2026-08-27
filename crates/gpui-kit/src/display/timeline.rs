//! A chronological feed of what happened, and who did it.
//!
//! # The timeline does not know what time it is
//!
//! Every time here is a string the caller already formatted, and so is every
//! day heading. Turning an instant into words is calendar, time-zone and
//! locale work: whoever owns the clock owns the wording, and a component that
//! formatted a timestamp itself would be inventing a locale for its host. The
//! adapter that does the formatting sits above this component and hands it
//! finished strings, which is the whole seam.
//!
//! An entry whose time nobody knows says so. It is not quietly floated to the
//! top or dropped to the bottom, because either would be a claim about when it
//! happened.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, Theme, TypeScale};

use crate::display::badge::Tone;
use crate::display::status::StatusDot;
use crate::foundation::{Ident, StyledExt};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// When an entry happened, as far as anyone knows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum EntryTime {
    /// A time the caller has already put into words.
    At(SharedString),
    /// Nobody knows when this happened, and the entry says so.
    #[default]
    Unknown,
}

impl EntryTime {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::At(_) => "known",
            Self::Unknown => "unknown",
        }
    }

    /// The words a reader sees, including the ones for a time nobody knows.
    pub(crate) fn shown(&self, cx: &App) -> SharedString {
        match self {
            Self::At(time) => time.clone(),
            Self::Unknown => cx.strings().text(StringKey::TimeUnknown),
        }
    }
}

impl From<SharedString> for EntryTime {
    fn from(value: SharedString) -> Self {
        Self::At(value)
    }
}

impl From<&'static str> for EntryTime {
    fn from(value: &'static str) -> Self {
        Self::At(SharedString::new_static(value))
    }
}

impl From<String> for EntryTime {
    fn from(value: String) -> Self {
        Self::At(SharedString::from(value))
    }
}

/// One thing that happened.
pub struct TimelineEntry {
    id: SharedString,
    time: EntryTime,
    actor: Option<SharedString>,
    description: SharedString,
    tone: Tone,
    detail: Option<AnyElement>,
}

impl std::fmt::Debug for TimelineEntry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TimelineEntry")
            .field("id", &self.id)
            .field("time", &self.time)
            .field("actor", &self.actor)
            .field("tone", &self.tone)
            .field("has_detail", &self.detail.is_some())
            .finish()
    }
}

impl TimelineEntry {
    pub fn new(id: impl Into<SharedString>, description: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            time: EntryTime::Unknown,
            actor: None,
            description: description.into(),
            tone: Tone::Neutral,
            detail: None,
        }
    }

    /// The time, already in the words the host chose.
    pub fn time(mut self, time: impl Into<EntryTime>) -> Self {
        self.time = time.into();
        self
    }

    pub fn time_unknown(mut self) -> Self {
        self.time = EntryTime::Unknown;
        self
    }

    pub fn actor(mut self, actor: impl Into<SharedString>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Anything the entry carries beyond one line, such as a diff or a reason.
    pub fn detail(mut self, detail: impl IntoElement) -> Self {
        self.detail = Some(detail.into_any_element());
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// A run of entries under one heading, usually a day.
pub struct TimelineGroup {
    id: SharedString,
    label: SharedString,
    entries: Vec<TimelineEntry>,
}

impl std::fmt::Debug for TimelineGroup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TimelineGroup")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl TimelineGroup {
    /// `label` is the heading in the caller's words — "Today", "12 March" —
    /// because the timeline does not name days either.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            entries: Vec::new(),
        }
    }

    pub fn entry(mut self, entry: TimelineEntry) -> Self {
        self.entries.push(entry);
        self
    }

    pub fn entries(mut self, entries: impl IntoIterator<Item = TimelineEntry>) -> Self {
        self.entries.extend(entries);
        self
    }
}

/// A chronological feed, optionally divided into days.
#[derive(IntoElement)]
pub struct Timeline {
    ident: Ident,
    groups: Vec<TimelineGroup>,
    loose: Vec<TimelineEntry>,
}

impl std::fmt::Debug for Timeline {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Timeline")
            .field("ident", &self.ident)
            .field("groups", &self.groups.len())
            .field("entries", &self.loose.len())
            .finish()
    }
}

impl Timeline {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            groups: Vec::new(),
            loose: Vec::new(),
        }
    }

    pub fn group(mut self, group: TimelineGroup) -> Self {
        self.groups.push(group);
        self
    }

    pub fn groups(mut self, groups: impl IntoIterator<Item = TimelineGroup>) -> Self {
        self.groups.extend(groups);
        self
    }

    /// Entries with no heading over them, kept in the order they arrived.
    pub fn entries(mut self, entries: impl IntoIterator<Item = TimelineEntry>) -> Self {
        self.loose.extend(entries);
        self
    }

    fn count(&self) -> usize {
        self.loose.len()
            + self
                .groups
                .iter()
                .map(|group| group.entries.len())
                .sum::<usize>()
    }
}

impl RenderOnce for Timeline {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();
        let count = self.count();

        let mut feed = div().column().w_full().gap_token(&theme, Space::Md);

        for (index, entry) in self.loose.into_iter().enumerate() {
            feed = feed.child(entry_element(&ident, &theme, entry, index + 1 < count, cx));
        }

        for group in self.groups {
            let heading = div()
                .row()
                .w_full()
                .gap_token(&theme, Space::Sm)
                .type_scale(&theme, TypeScale::Caption)
                .text_color(theme.colors.text_faint)
                .child(div().w(px(theme.measures.timeline_rail_width)).flex_none())
                .child(group.label.clone())
                .child(
                    div()
                        .flex_1()
                        .h(px(theme.borders.hairline))
                        .bg(theme.colors.divider),
                )
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child(group.id.as_ref()).semantic_id(), Role::Heading)
                        .parent(ident.semantic_id())
                        .text(group.label.clone())
                        .value(cx.numbers().count(group.entries.len())),
                );

            let last = group.entries.len().saturating_sub(1);
            let mut section = div().column().w_full().gap_token(&theme, Space::Md);
            for (index, entry) in group.entries.into_iter().enumerate() {
                section = section.child(entry_element(&ident, &theme, entry, index < last, cx));
            }

            feed = feed.child(
                div()
                    .column()
                    .w_full()
                    .gap_token(&theme, Space::Md)
                    .child(heading)
                    .child(section),
            );
        }

        feed.semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::List).value(cx.numbers().count(count)),
        )
    }
}

/// One entry, with the rail running past its dot when something follows it.
fn entry_element(
    timeline: &Ident,
    theme: &Theme,
    entry: TimelineEntry,
    continues: bool,
    cx: &mut App,
) -> AnyElement {
    let ident = timeline.child(entry.id.as_ref());
    let unknown = entry.time == EntryTime::Unknown;

    let rail = div()
        .w(px(theme.measures.timeline_rail_width))
        .flex_none()
        .column()
        .items_center()
        .child(
            div()
                .mt(px(theme.space(Space::Xs)))
                .child(StatusDot::new(entry.tone)),
        )
        .when(continues, |element| {
            element.child(
                div()
                    .mt(px(theme.space(Space::Xs)))
                    .w(px(theme.borders.hairline))
                    .flex_1()
                    .min_h(px(theme.space(Space::Md)))
                    .bg(theme.colors.divider),
            )
        });

    let heading = div()
        .row()
        .flex_wrap()
        .gap_token(theme, Space::Sm)
        .type_scale(theme, TypeScale::Caption)
        .child(
            div()
                .text_color(if unknown {
                    theme.colors.warning
                } else {
                    theme.colors.text_muted
                })
                .child(entry.time.shown(cx)),
        )
        .children(entry.actor.clone().map(|actor| {
            div()
                .text_color(theme.colors.text_faint)
                .child(actor.clone())
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child("actor").semantic_id(), Role::Text)
                        .parent(ident.semantic_id())
                        .text(actor),
                )
        }));

    div()
        .row()
        .items_start()
        .w_full()
        .gap_token(theme, Space::Sm)
        .child(rail)
        .child(
            div()
                .column()
                .flex_1()
                .min_w_0()
                .gap(px(theme.space(Space::Xxs)))
                .child(heading)
                .child(
                    div()
                        .type_scale(theme, TypeScale::Label)
                        .text_color(theme.colors.text)
                        .child(entry.description.clone()),
                )
                .children(entry.detail.map(|detail| {
                    div()
                        .mt_token(theme, Space::Xs)
                        .type_scale(theme, TypeScale::Caption)
                        .text_color(theme.colors.text_muted)
                        .child(detail)
                })),
        )
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Row)
                .parent(timeline.semantic_id())
                .text(entry.description.clone())
                // The time is the one fact a reader would otherwise have to
                // take off the pixels, and an unknown one says so by name.
                .value(match &entry.time {
                    EntryTime::At(time) => time.clone(),
                    EntryTime::Unknown => SharedString::new_static("time unknown"),
                }),
        )
        .into_any_element()
}
