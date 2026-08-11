//! Read-only presentation of a diff the caller already computed.
//!
//! Files, hunks, lines, line identities, old and new line numbers, marks and
//! pre-classified code spans all come from the caller. This module computes no
//! diff, parses no syntax, applies no patch and reads no filesystem.
//!
//! # One renderer, two arrangements
//!
//! Unified and split presentation use the same caller-supplied logical rows
//! and the same line-side renderer. For a replacement, the caller supplies
//! the aligned old and new sides: split mode places them opposite one another,
//! while unified mode places them on consecutive fixed rows. The component
//! never guesses which removal belongs with which addition.
//!
//! # Large data
//!
//! The hierarchy is flattened once per render, which walks all caller-owned
//! files, hunks and lines. The resulting fixed-height rows are handed to the
//! virtualized [`List`], so only viewport rows are laid out
//! or published. The explicit price is the same one paid by virtualized
//! `CodeView`: long lines are clipped and do not horizontally scroll. This is
//! suitable for a large already-materialized diff, not for lazy diff loading.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, Theme, TypeScale,
};

use crate::content::code_view::code_runs;
use crate::content::markdown::CodeSpan;
use crate::data::{List, ListItem};
use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::{Ident, StyledExt};
use crate::strings::{ActiveStrings, StringKey};

type EventHandler = Rc<dyn Fn(DiffViewEvent, &mut Window, &mut App)>;

/// How the same caller-supplied lines are arranged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffPresentation {
    #[default]
    Unified,
    Split,
}

impl DiffPresentation {
    pub fn name(self) -> &'static str {
        match self {
            Self::Unified => "unified",
            Self::Split => "split",
        }
    }
}

/// The caller's claim about one diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum DiffLineMark {
    #[default]
    Context,
    Added,
    Removed,
}

impl DiffLineMark {
    fn tone(self) -> Tone {
        match self {
            Self::Context => Tone::Neutral,
            Self::Added => Tone::Success,
            Self::Removed => Tone::Danger,
        }
    }

    fn key(self) -> StringKey {
        match self {
            Self::Context => StringKey::DiffContextLine,
            Self::Added => StringKey::CodeLineAdded,
            Self::Removed => StringKey::CodeLineRemoved,
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Context => " ",
            Self::Added => "+",
            Self::Removed => "-",
        }
    }
}

/// One side of a caller-supplied diff line.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DiffSide {
    number: Option<usize>,
    text: SharedString,
    spans: Vec<CodeSpan>,
    mark: DiffLineMark,
}

impl DiffSide {
    fn new(text: impl Into<SharedString>, mark: DiffLineMark) -> Self {
        Self {
            number: None,
            text: text.into(),
            spans: Vec::new(),
            mark,
        }
    }
}

/// One stable, caller-supplied logical diff row.
///
/// A context row carries the same text on both sides. A pure addition or
/// removal carries one side. A replacement carries both different sides, so
/// split presentation can align them without this component deciding which
/// removal belongs with which addition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub id: SharedString,
    old: Option<DiffSide>,
    new: Option<DiffSide>,
}

impl DiffLine {
    /// A context line. The text begins on both sides; old and new numbers are
    /// supplied independently.
    pub fn new(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        let side = DiffSide::new(text, DiffLineMark::Context);
        Self {
            id: id.into(),
            old: Some(side.clone()),
            new: Some(side),
        }
    }

    /// A pure addition supplied by the caller.
    pub fn added(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            old: None,
            new: Some(DiffSide::new(text, DiffLineMark::Added)),
        }
    }

    /// A pure removal supplied by the caller.
    pub fn removed(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            old: Some(DiffSide::new(text, DiffLineMark::Removed)),
            new: None,
        }
    }

    /// An aligned replacement supplied by the caller. Unified presentation
    /// draws its removed and added sides as consecutive rows; split
    /// presentation draws them opposite one another.
    pub fn paired(
        id: impl Into<SharedString>,
        old: impl Into<SharedString>,
        new: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            old: Some(DiffSide::new(old, DiffLineMark::Removed)),
            new: Some(DiffSide::new(new, DiffLineMark::Added)),
        }
    }

    pub fn old_number(mut self, number: usize) -> Self {
        if let Some(old) = &mut self.old {
            old.number = Some(number);
        }
        self
    }

    pub fn new_number(mut self, number: usize) -> Self {
        if let Some(new) = &mut self.new {
            new.number = Some(number);
        }
        self
    }

    /// Pre-classified spans for both sides. Use `old_spans` and `new_spans`
    /// when an aligned replacement has different classifications.
    pub fn spans(mut self, spans: impl IntoIterator<Item = CodeSpan>) -> Self {
        let spans: Vec<CodeSpan> = spans.into_iter().collect();
        if let Some(old) = &mut self.old {
            old.spans = spans.clone();
        }
        if let Some(new) = &mut self.new {
            new.spans = spans;
        }
        self
    }

    pub fn old_spans(mut self, spans: impl IntoIterator<Item = CodeSpan>) -> Self {
        let spans: Vec<CodeSpan> = spans.into_iter().collect();
        if self.is_context() {
            if let Some(old) = &mut self.old {
                old.spans = spans.clone();
            }
            if let Some(new) = &mut self.new {
                new.spans = spans;
            }
        } else if let Some(old) = &mut self.old {
            old.spans = spans;
        }
        self
    }

    pub fn new_spans(mut self, spans: impl IntoIterator<Item = CodeSpan>) -> Self {
        let spans: Vec<CodeSpan> = spans.into_iter().collect();
        if self.is_context() {
            if let Some(old) = &mut self.old {
                old.spans = spans.clone();
            }
            if let Some(new) = &mut self.new {
                new.spans = spans;
            }
        } else if let Some(new) = &mut self.new {
            new.spans = spans;
        }
        self
    }

    fn is_context(&self) -> bool {
        matches!(
            (&self.old, &self.new),
            (Some(old), Some(new))
                if old.mark == DiffLineMark::Context && new.mark == DiffLineMark::Context
        )
    }
}

/// One caller-supplied hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub id: SharedString,
    pub header: SharedString,
    pub lines: Vec<DiffLine>,
}

impl DiffHunk {
    pub fn new(
        id: impl Into<SharedString>,
        header: impl Into<SharedString>,
        lines: impl IntoIterator<Item = DiffLine>,
    ) -> Self {
        Self {
            id: id.into(),
            header: header.into(),
            lines: lines.into_iter().collect(),
        }
    }
}

/// One caller-supplied file and its hunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFile {
    pub id: SharedString,
    pub label: SharedString,
    pub hunks: Vec<DiffHunk>,
}

impl DiffFile {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        hunks: impl IntoIterator<Item = DiffHunk>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            hunks: hunks.into_iter().collect(),
        }
    }
}

/// An action on caller-owned diff identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffViewEvent {
    FileActivated {
        file_id: SharedString,
    },
    HunkActivated {
        file_id: SharedString,
        hunk_id: SharedString,
    },
    LineActivated {
        file_id: SharedString,
        hunk_id: SharedString,
        line_id: SharedString,
    },
}

/// A virtualized, read-only diff presentation.
#[derive(IntoElement)]
pub struct DiffView {
    ident: Ident,
    files: Vec<DiffFile>,
    presentation: DiffPresentation,
    visible_rows: usize,
    on_event: Option<EventHandler>,
}

impl std::fmt::Debug for DiffView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DiffView")
            .field("ident", &self.ident)
            .field("files", &self.files.len())
            .field("presentation", &self.presentation)
            .field("visible_rows", &self.visible_rows)
            .field("has_handler", &self.on_event.is_some())
            .finish()
    }
}

impl DiffView {
    pub fn new(ident: impl Into<Ident>, files: impl IntoIterator<Item = DiffFile>) -> Self {
        Self {
            ident: ident.into(),
            files: files.into_iter().collect(),
            presentation: DiffPresentation::Unified,
            visible_rows: 18,
            on_event: None,
        }
    }

    pub fn presentation(mut self, presentation: DiffPresentation) -> Self {
        self.presentation = presentation;
        self
    }

    /// Bounds and virtualizes the view to this many fixed-height rows. File
    /// and hunk headers each occupy one row too.
    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = rows.max(1);
        self
    }

    /// Reports file, hunk and line actions without changing or applying the
    /// diff.
    pub fn on_event(
        mut self,
        handler: impl Fn(DiffViewEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

#[derive(Debug, Clone)]
struct FlatRow {
    id: SharedString,
    event: DiffViewEvent,
    kind: FlatKind,
}

#[derive(Debug, Clone)]
enum FlatKind {
    File(SharedString),
    Hunk(SharedString),
    Unified {
        side: DiffSide,
        old_number: Option<usize>,
        new_number: Option<usize>,
    },
    Split {
        old: Option<DiffSide>,
        new: Option<DiffSide>,
    },
}

impl FlatKind {
    fn label_key(&self) -> StringKey {
        match self {
            Self::File(_) => StringKey::DiffFile,
            Self::Hunk(_) => StringKey::DiffHunk,
            Self::Unified { side, .. } => side.mark.key(),
            Self::Split { old, new } => match (old, new) {
                (Some(old), Some(new))
                    if old.mark == DiffLineMark::Context && new.mark == DiffLineMark::Context =>
                {
                    StringKey::DiffContextLine
                }
                (Some(_), Some(_)) => StringKey::DiffChangedLine,
                (Some(old), None) => old.mark.key(),
                (None, Some(new)) => new.mark.key(),
                (None, None) => StringKey::DiffContextLine,
            },
        }
    }
}

impl RenderOnce for DiffView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rows = Rc::new(flatten(self.files, self.presentation));
        let count = rows.len();
        let list_ident = self.ident.child("rows");

        let body: AnyElement = if rows.is_empty() {
            EmptyState::new(
                self.ident.child("empty"),
                cx.strings().text(StringKey::DiffEmpty),
            )
            .kind(EmptyKind::Empty)
            .into_any_element()
        } else {
            let rendered = Rc::clone(&rows);
            let row_theme = theme.clone();
            let mut list = List::new(list_ident.clone(), count, move |index, _, cx| {
                let row = &rendered[index];
                let label = cx.strings().text(row.kind.label_key());
                ListItem::new(row.id.clone(), diff_row(&row.kind, &row_theme))
                    // Source and paths stay out of diagnostic snapshots. Stable
                    // business ids and the row kind remain addressable.
                    .text(label)
            })
            .row_height(theme.control.get(ControlSize::Sm).height)
            .visible_rows(self.visible_rows);

            if let Some(handler) = self.on_event.clone() {
                let indices: Rc<HashMap<SharedString, usize>> = Rc::new(
                    rows.iter()
                        .enumerate()
                        .map(|(index, row)| (row.id.clone(), index))
                        .collect(),
                );
                let event_rows = Rc::clone(&rows);
                list = list.on_select(move |id, window, cx| {
                    if let Some(index) = indices.get(&id) {
                        handler(event_rows[*index].event.clone(), window, cx);
                    }
                });
            }
            list.into_any_element()
        };

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .p_token(&theme, Space::Sm)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Raised, Elevation::Raised)
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .value(self.presentation.name())
                    .read_only(true),
            )
    }
}

fn flatten(files: Vec<DiffFile>, presentation: DiffPresentation) -> Vec<FlatRow> {
    let mut rows = Vec::new();
    for file in files {
        let file_path = Ident::new("file").child(file.id.as_ref());
        rows.push(FlatRow {
            id: file_path.semantic_id(),
            event: DiffViewEvent::FileActivated {
                file_id: file.id.clone(),
            },
            kind: FlatKind::File(file.label),
        });
        for hunk in file.hunks {
            let hunk_path = file_path.child("hunk").child(hunk.id.as_ref());
            rows.push(FlatRow {
                id: hunk_path.semantic_id(),
                event: DiffViewEvent::HunkActivated {
                    file_id: file.id.clone(),
                    hunk_id: hunk.id.clone(),
                },
                kind: FlatKind::Hunk(hunk.header),
            });
            for line in hunk.lines {
                let line_path = hunk_path.child("line").child(line.id.as_ref());
                let event = DiffViewEvent::LineActivated {
                    file_id: file.id.clone(),
                    hunk_id: hunk.id.clone(),
                    line_id: line.id.clone(),
                };
                match presentation {
                    DiffPresentation::Split => rows.push(FlatRow {
                        id: line_path.semantic_id(),
                        event,
                        kind: FlatKind::Split {
                            old: line.old,
                            new: line.new,
                        },
                    }),
                    DiffPresentation::Unified => match (line.old, line.new) {
                        (Some(old), Some(new))
                            if old.text == new.text
                                && old.spans == new.spans
                                && old.mark == DiffLineMark::Context
                                && new.mark == DiffLineMark::Context =>
                        {
                            rows.push(FlatRow {
                                id: line_path.semantic_id(),
                                event,
                                kind: FlatKind::Unified {
                                    old_number: old.number,
                                    new_number: new.number,
                                    side: old,
                                },
                            });
                        }
                        (Some(old), Some(new)) => {
                            rows.push(FlatRow {
                                id: line_path.child("old").semantic_id(),
                                event: event.clone(),
                                kind: FlatKind::Unified {
                                    old_number: old.number,
                                    new_number: None,
                                    side: old,
                                },
                            });
                            rows.push(FlatRow {
                                id: line_path.child("new").semantic_id(),
                                event,
                                kind: FlatKind::Unified {
                                    old_number: None,
                                    new_number: new.number,
                                    side: new,
                                },
                            });
                        }
                        (Some(old), None) => rows.push(FlatRow {
                            id: line_path.semantic_id(),
                            event,
                            kind: FlatKind::Unified {
                                old_number: old.number,
                                new_number: None,
                                side: old,
                            },
                        }),
                        (None, Some(new)) => rows.push(FlatRow {
                            id: line_path.semantic_id(),
                            event,
                            kind: FlatKind::Unified {
                                old_number: None,
                                new_number: new.number,
                                side: new,
                            },
                        }),
                        (None, None) => {}
                    },
                }
            }
        }
    }
    rows
}

fn diff_row(kind: &FlatKind, theme: &Theme) -> AnyElement {
    match kind {
        FlatKind::File(label) => div()
            .row()
            .items_center()
            .w_full()
            .h_full()
            .px_token(theme, Space::Sm)
            .type_scale(theme, TypeScale::Label)
            .text_color(theme.colors.text)
            .bg(theme.colors.raised)
            .child(label.clone())
            .into_any_element(),
        FlatKind::Hunk(header) => div()
            .row()
            .items_center()
            .w_full()
            .h_full()
            .px_token(theme, Space::Sm)
            .font_family(theme.typography.mono.clone())
            .text_size(px(theme.typography.code.size))
            .text_color(theme.colors.accent)
            .bg(theme
                .colors
                .accent
                .opacity(theme.effects.selected_ring_alpha))
            .child(header.clone())
            .into_any_element(),
        FlatKind::Unified {
            side,
            old_number,
            new_number,
        } => unified_line(side, *old_number, *new_number, theme),
        FlatKind::Split { old, new } => split_line(old.as_ref(), new.as_ref(), theme),
    }
}

fn unified_line(
    side: &DiffSide,
    old_number: Option<usize>,
    new_number: Option<usize>,
    theme: &Theme,
) -> AnyElement {
    let color = side.mark.tone().color(theme);
    div()
        .row()
        .items_center()
        .w_full()
        .h_full()
        .font_family(theme.typography.mono.clone())
        .text_size(px(theme.typography.code.size))
        .line_height(px(theme.typography.code.line_height))
        .when(side.mark != DiffLineMark::Context, |element| {
            element.bg(color.opacity(theme.effects.selected_ring_alpha))
        })
        .child(number(old_number, theme))
        .child(number(new_number, theme))
        .child(
            div()
                .flex_none()
                .w(px(18.0))
                .text_align(gpui::TextAlign::Center)
                .text_color(color)
                .child(side.mark.prefix()),
        )
        .child(
            div()
                .row()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .children(code_runs(theme, side.text.as_ref(), &side.spans)),
        )
        .into_any_element()
}

fn split_line(old: Option<&DiffSide>, new: Option<&DiffSide>, theme: &Theme) -> AnyElement {
    div()
        .row()
        .items_center()
        .w_full()
        .h_full()
        .font_family(theme.typography.mono.clone())
        .text_size(px(theme.typography.code.size))
        .line_height(px(theme.typography.code.line_height))
        .child(code_side(old, theme))
        .child(
            div()
                .flex_none()
                .w(px(theme.borders.hairline))
                .h_full()
                .bg(theme.colors.hairline_strong),
        )
        .child(code_side(new, theme))
        .into_any_element()
}

fn code_side(side: Option<&DiffSide>, theme: &Theme) -> AnyElement {
    let mark = side.map_or(DiffLineMark::Context, |side| side.mark);
    let color = mark.tone().color(theme);
    div()
        .row()
        .items_center()
        .flex_1()
        .min_w_0()
        .h_full()
        .when(side.is_some() && mark != DiffLineMark::Context, |element| {
            element.bg(color.opacity(theme.effects.selected_ring_alpha))
        })
        .child(number(side.and_then(|side| side.number), theme))
        .child(
            div()
                .flex_none()
                .w(px(18.0))
                .text_align(gpui::TextAlign::Center)
                .text_color(color)
                .child(side.map_or("", |side| side.mark.prefix())),
        )
        .child(
            div()
                .row()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .children(
                    side.into_iter()
                        .flat_map(|side| code_runs(theme, side.text.as_ref(), &side.spans)),
                ),
        )
        .into_any_element()
}

fn number(number: Option<usize>, theme: &Theme) -> AnyElement {
    div()
        .flex_none()
        .w(px(44.0))
        .pr(px(theme.space(Space::Xs)))
        .overflow_hidden()
        .text_align(gpui::TextAlign::Right)
        .text_color(theme.colors.text_faint)
        .child(number.map_or_else(SharedString::default, |number| {
            SharedString::from(number.to_string())
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattening_uses_only_caller_identity() {
        let files = vec![DiffFile::new(
            "readme",
            "README.md",
            [DiffHunk::new(
                "intro",
                "@@ introduction @@",
                [DiffLine::new("line-title", "# Title")],
            )],
        )];
        let rows = flatten(files, DiffPresentation::Unified);
        assert_eq!(rows[0].id, "file.readme");
        assert_eq!(rows[1].id, "file.readme.hunk.intro");
        assert_eq!(rows[2].id, "file.readme.hunk.intro.line.line-title");
    }

    #[test]
    fn caller_aligned_replacement_is_one_split_row_and_two_unified_rows() {
        let files = || {
            vec![DiffFile::new(
                "source",
                "src/lib.rs",
                [DiffHunk::new(
                    "change",
                    "@@ fixture @@",
                    [DiffLine::paired("cache", "old_cache", "verified_cache")],
                )],
            )]
        };

        let split = flatten(files(), DiffPresentation::Split);
        assert_eq!(split.len(), 3);
        assert_eq!(split[2].id, "file.source.hunk.change.line.cache");
        match &split[2].kind {
            FlatKind::Split { old, new } => {
                assert_eq!(
                    old.as_ref().map(|side| side.text.as_ref()),
                    Some("old_cache")
                );
                assert_eq!(
                    new.as_ref().map(|side| side.text.as_ref()),
                    Some("verified_cache")
                );
            }
            _ => panic!("replacement must remain aligned in split presentation"),
        }

        let unified = flatten(files(), DiffPresentation::Unified);
        assert_eq!(unified.len(), 4);
        assert_eq!(unified[2].id, "file.source.hunk.change.line.cache.old");
        assert_eq!(unified[3].id, "file.source.hunk.change.line.cache.new");
    }

    #[test]
    fn every_public_line_shape_has_the_same_presence_in_both_presentations() {
        let lines = [
            DiffLine::new("context", "same"),
            DiffLine::added("added", "new"),
            DiffLine::removed("removed", "old"),
            DiffLine::paired("paired", "before", "after"),
        ];
        let files = || {
            vec![DiffFile::new(
                "source",
                "src/lib.rs",
                [DiffHunk::new("change", "@@ fixture @@", lines.clone())],
            )]
        };

        let split = flatten(files(), DiffPresentation::Split);
        let unified = flatten(files(), DiffPresentation::Unified);

        assert_eq!(split.len(), 6);
        assert_eq!(unified.len(), 7);
        for id in ["context", "added", "removed", "paired"] {
            assert!(split.iter().any(|row| row.event == line_event(id)));
            assert!(unified.iter().any(|row| row.event == line_event(id)));
        }
    }

    #[test]
    fn side_specific_spans_keep_a_context_line_one_logical_row() {
        let files = || {
            vec![DiffFile::new(
                "source",
                "src/lib.rs",
                [DiffHunk::new(
                    "change",
                    "@@ fixture @@",
                    [DiffLine::new("context", "same").old_spans([CodeSpan {
                        range: 0..4,
                        tone: Tone::Accent,
                    }])],
                )],
            )]
        };

        let split = flatten(files(), DiffPresentation::Split);
        let unified = flatten(files(), DiffPresentation::Unified);

        assert_eq!(split.len(), 3);
        assert_eq!(unified.len(), 3);
        assert_eq!(split[2].event, line_event("context"));
        assert_eq!(unified[2].event, line_event("context"));
    }

    fn line_event(id: &str) -> DiffViewEvent {
        DiffViewEvent::LineActivated {
            file_id: "source".into(),
            hunk_id: "change".into(),
            line_id: id.to_string().into(),
        }
    }
}
