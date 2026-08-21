//! Read-only presentation of a diff the caller already computed.
//!
//! Files, hunks, lines, line identities, old and new line numbers and marks
//! all come from the caller. This module computes no diff, applies no patch
//! and reads no filesystem.
//!
//! # One renderer, two arrangements
//!
//! Unified and split presentation use the same caller-supplied logical rows
//! and the same line-side renderer. For a replacement, the caller supplies
//! the aligned old and new sides: split mode places them opposite one another,
//! while unified mode places them on consecutive rows. The component never
//! guesses which removal belongs with which addition.
//!
//! # Colour
//!
//! Spans are the caller's to supply and their spans always win. Two things
//! fill in what the caller left empty, in this order. On a replacement,
//! [`word_spans`] marks the tokens with no counterpart on the other side,
//! because on a row that exists because two versions differ, where they differ
//! is the point. Everything else, when the caller named a language with
//! [`DiffView::language`], is coloured by [`crate::content::highlight`] — a
//! scanner, not a parser, on a language this crate has a table for, changing
//! colour and nothing else. Each side carries its own scan, since the old and
//! new texts are two versions of one file and not one document.
//!
//! # Size, and what it costs
//!
//! The hierarchy is flattened once per render, which walks all caller-owned
//! files, hunks and lines. The resulting rows are handed to the virtualized
//! [`List`], so only the viewport is laid out or published.
//!
//! Rows are fixed-height by default, which is what makes a hundred-thousand
//! row diff open instantly: no row has to be laid out for the list to know
//! where any other one is. The price is that a long line is clipped, and
//! [`DiffView::wrapping`] is the other side of that trade for a diff whose
//! changes live at the ends of long lines.
//!
//! Neither helps with a review of forty files, which is unreadable at any row
//! height. [`DiffFile::folded`] puts a file away behind a header that says how
//! many lines it adds and removes, so the top of a large review is a list of
//! files again.

use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, SyntaxColor, Theme, TypeScale,
};

use crate::content::code_view::styled_code;
use crate::content::highlight::{Carry, Language, line_spans};
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
    /// When true the hunk header stays and the lines do not, so a host can
    /// offer more context without this view inventing it.
    pub collapsed: bool,
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
            collapsed: false,
        }
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

/// One caller-supplied file and its hunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFile {
    pub id: SharedString,
    pub label: SharedString,
    pub hunks: Vec<DiffHunk>,
    /// When true the file's header stays and its hunks do not.
    pub folded: bool,
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
            folded: false,
        }
    }

    /// Puts the whole file away behind its header.
    ///
    /// A review of forty files is unreadable as forty thousand rows, and the
    /// hunk-level collapse does not help: it still leaves every file's header
    /// and every hunk's header on screen. Folding is what makes the top of a
    /// large review a list of files again.
    ///
    /// The header of a folded file says what it is hiding — how many lines
    /// were added and removed — because a fold that says only "there is
    /// something here" makes the reader open every file to find out which ones
    /// matter, which is the thing folding was for.
    pub fn folded(mut self, folded: bool) -> Self {
        self.folded = folded;
        self
    }

    /// How many lines this file adds and removes, over all its hunks,
    /// collapsed ones included: a collapsed hunk still changed what it
    /// changed.
    fn changed(&self) -> (usize, usize) {
        self.hunks.iter().flat_map(|hunk| hunk.lines.iter()).fold(
            (0, 0),
            |(added, removed), line| {
                let adds = line
                    .new
                    .as_ref()
                    .is_some_and(|side| side.mark == DiffLineMark::Added);
                let takes = line
                    .old
                    .as_ref()
                    .is_some_and(|side| side.mark == DiffLineMark::Removed);
                (added + usize::from(adds), removed + usize::from(takes))
            },
        )
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
    /// The host should disclose more context around this hunk.
    ExpandHunk {
        file_id: SharedString,
        hunk_id: SharedString,
    },
    /// The host should put this file back on screen. Reported instead of
    /// [`DiffViewEvent::FileActivated`] while a file is folded, so a host that
    /// treats a header click as "open this file elsewhere" does not have to
    /// guess which of the two the reader meant.
    UnfoldFile {
        file_id: SharedString,
    },
}

/// A virtualized, read-only diff presentation.
#[derive(IntoElement)]
pub struct DiffView {
    ident: Ident,
    files: Vec<DiffFile>,
    presentation: DiffPresentation,
    visible_rows: usize,
    wrapping: bool,
    language: Option<SharedString>,
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
            .field("wrapping", &self.wrapping)
            .field("language", &self.language)
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
            wrapping: false,
            language: None,
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

    /// Wraps a line that is too long instead of clipping it.
    ///
    /// Fixed rows are what make a large diff cheap: every row's height is
    /// known without laying it out, so the list can jump anywhere in a hundred
    /// thousand rows immediately. The price is that a long line is cut off at
    /// the right edge, and in a diff that price is sometimes wrong — a
    /// minified bundle, a long string literal, a one-line JSON change is a
    /// line whose *end* is the change, and a diff that hides it is showing
    /// nothing.
    ///
    /// So this is a choice rather than a default. Rows are then measured as
    /// they are laid out, which costs layout for every row the reader passes
    /// and makes the scrollbar settle rather than being right immediately.
    pub fn wrapping(mut self, wrapping: bool) -> Self {
        self.wrapping = wrapping;
        self
    }

    /// Colours code that arrived without colours, by the language the caller
    /// says it is.
    ///
    /// A diff of code is code, and reading it uncoloured next to a coloured
    /// editor is harder than it needs to be. The rule is the same one
    /// [`CodeView::language`](crate::content::CodeView::language) keeps: a
    /// line that arrived with spans keeps them, because the caller's grammar
    /// outranks this scanner, and a language this crate does not know colours
    /// nothing rather than guessing.
    ///
    /// A replacement keeps its word-level spans rather than gaining syntax
    /// colour. Saying *which words changed* is the more specific claim on a
    /// row that exists because two versions differ, and two claims in one
    /// place is one too many.
    pub fn language(mut self, language: impl Into<SharedString>) -> Self {
        self.language = Some(language.into());
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
    File {
        label: SharedString,
        /// Added and removed line counts, so a folded header says what it
        /// holds instead of only that it holds something.
        changed: (usize, usize),
        folded: bool,
    },
    Hunk(SharedString),
    Expand,
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
            Self::File { .. } => StringKey::DiffFile,
            Self::Hunk(_) => StringKey::DiffHunk,
            Self::Expand => StringKey::DiffExpandHunk,
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
    fn render(mut self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        if let Some(known) = self.language.as_deref().and_then(Language::named) {
            colour(&mut self.files, known);
        }
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
            let expand_label = cx.strings().text(StringKey::DiffExpandHunk);
            let fit = match self.wrapping {
                true => Fit::Wraps,
                false => Fit::Clips,
            };
            let mut list = List::new(list_ident.clone(), count, move |index, _, cx| {
                let row = &rendered[index];
                let label = cx.strings().text(row.kind.label_key());
                ListItem::new(
                    row.id.clone(),
                    // The row's position in the flattened diff is its reading
                    // order. The list is virtualized, so a copy spanning rows
                    // that were never mounted reports itself incomplete.
                    diff_row(
                        &row.id,
                        &row.kind,
                        index as u64,
                        &row_theme,
                        &expand_label,
                        fit,
                    ),
                )
                // Source and paths stay out of diagnostic snapshots. Stable
                // business ids and the row kind remain addressable.
                .text(label)
            })
            .visible_rows(self.visible_rows);
            list = match self.wrapping {
                true => list.flowing(),
                false => list.row_height(theme.control.get(ControlSize::Sm).height),
            };

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
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
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

/// Colours every line that arrived without colours, one file at a time.
///
/// The scan carries state from line to line, because a block comment or a
/// string opened on one line is still open on the next, and a scanner that
/// forgot that would colour the inside of a comment as code.
///
/// Each side is carried separately. The old and new texts are two different
/// versions of one file, and threading one scan through both would let a
/// bracket that only exists in the new version change how the old version is
/// read. A removed line is scanned in the old file's sequence and an added
/// line in the new file's, which is what each of them actually is.
fn colour(files: &mut [DiffFile], language: Language) {
    for file in files {
        let (mut old, mut new) = (Carry::None, Carry::None);
        for line in file.hunks.iter_mut().flat_map(|hunk| hunk.lines.iter_mut()) {
            // Which words changed is settled before the syntax scan, so a
            // replacement keeps that answer. It is the more specific claim: on
            // a row that exists because two versions differ, showing where
            // they differ beats showing that both are code.
            let (was, is) = fill_word_spans(line.old.take(), line.new.take());
            (line.old, line.new) = (was, is);
            for (side, carry) in [(line.old.as_mut(), &mut old), (line.new.as_mut(), &mut new)] {
                let Some(side) = side else { continue };
                let (found, next) = line_spans(language, side.text.as_ref(), *carry);
                *carry = next;
                // A line that already has spans keeps them, but the scan still
                // has to cross it: skipping it would lose the comment or
                // string it opened and mis-colour everything after.
                if side.spans.is_empty() {
                    side.spans = found;
                }
            }
        }
    }
}

fn flatten(files: Vec<DiffFile>, presentation: DiffPresentation) -> Vec<FlatRow> {
    let mut rows = Vec::new();
    for file in files {
        let file_path = Ident::new("file").child(file.id.as_ref());
        let changed = file.changed();
        rows.push(FlatRow {
            id: file_path.semantic_id(),
            event: match file.folded {
                true => DiffViewEvent::UnfoldFile {
                    file_id: file.id.clone(),
                },
                false => DiffViewEvent::FileActivated {
                    file_id: file.id.clone(),
                },
            },
            kind: FlatKind::File {
                label: file.label,
                changed,
                folded: file.folded,
            },
        });
        if file.folded {
            continue;
        }
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
            if hunk.collapsed {
                rows.push(FlatRow {
                    id: hunk_path.child("expand").semantic_id(),
                    event: DiffViewEvent::ExpandHunk {
                        file_id: file.id.clone(),
                        hunk_id: hunk.id.clone(),
                    },
                    kind: FlatKind::Expand,
                });
                continue;
            }
            for line in hunk.lines {
                let line_path = hunk_path.child("line").child(line.id.as_ref());
                let event = DiffViewEvent::LineActivated {
                    file_id: file.id.clone(),
                    hunk_id: hunk.id.clone(),
                    line_id: line.id.clone(),
                };
                match presentation {
                    DiffPresentation::Split => {
                        let (old, new) = fill_word_spans(line.old, line.new);
                        rows.push(FlatRow {
                            id: line_path.semantic_id(),
                            event,
                            kind: FlatKind::Split { old, new },
                        });
                    }
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
                            let (old, new) = fill_word_spans(Some(old), Some(new));
                            let old = old.expect("paired old");
                            let new = new.expect("paired new");
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

/// The `+n −m` a file header carries.
///
/// Nothing at all when the file changes nothing, because a pair of zeroes is
/// noise on every row of a large review, and the two counts are separate
/// elements because they are separate colours: the eye reads how much was
/// taken out against how much came in without reading either number.
fn counts(changed: (usize, usize), theme: &Theme) -> Vec<AnyElement> {
    let (added, removed) = changed;
    let mut marks = Vec::new();
    if added > 0 {
        marks.push(
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.syntax.get(SyntaxColor::Added))
                .child(SharedString::from(format!("+{added}")))
                .into_any_element(),
        );
    }
    if removed > 0 {
        marks.push(
            div()
                .type_scale(theme, TypeScale::Caption)
                .text_color(theme.colors.syntax.get(SyntaxColor::Removed))
                .child(SharedString::from(format!("\u{2212}{removed}")))
                .into_any_element(),
        );
    }
    marks
}

/// Whether the list hands a row its height or asks the row what it is.
///
/// The difference reaches every element in the row. A slotted row can say
/// `h_full`, because the slot is a height; a measured row's height *is* its
/// content, so the same call resolves against nothing and a one-line header
/// would collapse onto its text. And only a measured row can afford to let a
/// long line wrap, since a wrapped line is taller than the slot a fixed-height
/// list reserved for it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fit {
    Clips,
    Wraps,
}

/// The frame every row shares, holding whichever height its list can give.
fn row_frame(theme: &Theme, fit: Fit) -> gpui::Div {
    match fit {
        Fit::Clips => div().row().w_full().h_full(),
        Fit::Wraps => div()
            .row()
            .w_full()
            .min_h(px(theme.control.get(ControlSize::Sm).height)),
    }
}

/// The cell a line of code is laid out in, which is where wrapping is decided:
/// `min_w_0` lets the cell be narrower than the line so the text has a width
/// to wrap against, and only a clipping row refuses the breaks.
fn code_cell(fit: Fit) -> gpui::Div {
    match fit {
        Fit::Clips => div()
            .row()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap(),
        // Not a row: a flex child is measured at its own max content width, so
        // a line laid out as one would be handed all the width it asked for
        // and would have nothing to wrap against. As a block it inherits the
        // cell's width instead, and `min_w_0` is what lets that width be
        // narrower than the line.
        Fit::Wraps => div().flex_1().min_w_0(),
    }
}

fn diff_row(
    id: &SharedString,
    kind: &FlatKind,
    order: u64,
    theme: &Theme,
    expand: &SharedString,
    fit: Fit,
) -> AnyElement {
    match kind {
        FlatKind::File {
            label,
            changed,
            folded,
        } => row_frame(theme, fit)
            .items_center()
            .gap_token(theme, Space::Sm)
            .px_token(theme, Space::Sm)
            .type_scale(theme, TypeScale::Label)
            .text_color(theme.colors.text)
            .bg(theme.colors.raised)
            // A folded file is marked as one, so a header that hides
            // everything under it is distinguishable from a file that happens
            // to change nothing.
            .when(*folded, |element| {
                element.child(
                    gpui_kit_assets::icon(gpui_kit_assets::Icon::AltArrowRight)
                        .size(px(theme.typography.label.size))
                        .text_color(theme.colors.text_muted),
                )
            })
            .child(label.clone())
            .children(counts(*changed, theme))
            .into_any_element(),
        FlatKind::Expand => row_frame(theme, fit)
            .items_center()
            .px_token(theme, Space::Sm)
            .type_scale(theme, TypeScale::Caption)
            .text_color(theme.colors.accent)
            .child(expand.clone())
            .into_any_element(),
        FlatKind::Hunk(header) => row_frame(theme, fit)
            .items_center()
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
        } => unified_line(id, side, *old_number, *new_number, order * 2, theme, fit),
        FlatKind::Split { old, new } => {
            split_line(id, old.as_ref(), new.as_ref(), order, theme, fit)
        }
    }
}

fn unified_line(
    id: &SharedString,
    side: &DiffSide,
    old_number: Option<usize>,
    new_number: Option<usize>,
    order: u64,
    theme: &Theme,
    fit: Fit,
) -> AnyElement {
    let color = side.mark.tone().color(theme);
    // A wrapped line grows downwards, so its number and mark belong beside the
    // line's first row rather than beside the middle of everything it became.
    row_frame(theme, fit)
        .map(|frame| match fit {
            Fit::Clips => frame.items_center(),
            Fit::Wraps => frame.items_start(),
        })
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
            code_cell(fit).child(
                styled_code(theme, side.text.clone(), &side.spans)
                    .selectable_in_document(
                        SharedString::from(format!("{id}.text")),
                        SharedString::from(format!("{id}.text")),
                        order,
                    )
                    .virtualized_participant(true),
            ),
        )
        .into_any_element()
}

fn split_line(
    id: &SharedString,
    old: Option<&DiffSide>,
    new: Option<&DiffSide>,
    order: u64,
    theme: &Theme,
    fit: Fit,
) -> AnyElement {
    row_frame(theme, fit)
        .items_stretch()
        .font_family(theme.typography.mono.clone())
        .text_size(px(theme.typography.code.size))
        .line_height(px(theme.typography.code.line_height))
        // Within one split row the left column reads before the right, so the
        // two columns take consecutive orders under the row's own.
        .child(code_side(id, "old", old, order * 2, theme, fit))
        .child(
            div()
                .flex_none()
                .w(px(theme.borders.hairline))
                .bg(theme.colors.divider),
        )
        .child(code_side(id, "new", new, order * 2 + 1, theme, fit))
        .into_any_element()
}

fn code_side(
    id: &SharedString,
    slot: &str,
    side: Option<&DiffSide>,
    order: u64,
    theme: &Theme,
    fit: Fit,
) -> AnyElement {
    let mark = side.map_or(DiffLineMark::Context, |side| side.mark);
    let color = mark.tone().color(theme);
    div()
        .row()
        .map(|column| match fit {
            Fit::Clips => column.items_center(),
            Fit::Wraps => column.items_start(),
        })
        .flex_1()
        .min_w_0()
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
        .child(code_cell(fit).children(side.map(|side| {
            let key = SharedString::from(format!("{id}.{slot}.text"));
            styled_code(theme, side.text.clone(), &side.spans)
                .selectable_in_document(key.clone(), key, order)
                .virtualized_participant(true)
        })))
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

fn fill_word_spans(
    old: Option<DiffSide>,
    new: Option<DiffSide>,
) -> (Option<DiffSide>, Option<DiffSide>) {
    match (old, new) {
        (Some(mut old), Some(mut new))
            if old.spans.is_empty() && new.spans.is_empty() && old.text != new.text =>
        {
            let (left, right) = word_spans(old.text.as_ref(), new.text.as_ref());
            old.spans = left;
            new.spans = right;
            (Some(old), Some(new))
        }
        (old, new) => (old, new),
    }
}

/// Highlights tokens that do not appear in the other side.
///
/// This is not a diff: both strings are complete, and the spans name the
/// tokens that have no counterpart. A host that already computed a real
/// intra-line diff should pass its own [`CodeSpan`]s instead.
pub fn word_spans(old: &str, new: &str) -> (Vec<CodeSpan>, Vec<CodeSpan>) {
    let left = tokens(old);
    let right = tokens(new);
    let keep_left = lcs_keep(&left, &right);
    let keep_right = lcs_keep(&right, &left);
    (
        unmatched_spans(&left, &keep_left, SyntaxColor::Removed),
        unmatched_spans(&right, &keep_right, SyntaxColor::Added),
    )
}

fn tokens(text: &str) -> Vec<(Range<usize>, String)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        let word = ch.is_alphanumeric() || ch == '_';
        let end = chars.peek().map(|(next, _)| *next).unwrap_or(text.len());
        let next_word = chars
            .peek()
            .map(|(_, next)| next.is_alphanumeric() || *next == '_')
            .unwrap_or(!word);
        if word != next_word || chars.peek().is_none() {
            let slice = &text[start..end];
            if !slice.chars().all(char::is_whitespace) {
                out.push((start..end, slice.to_string()));
            }
            start = end;
        }
        let _ = index;
    }
    out
}

fn lcs_keep(side: &[(Range<usize>, String)], other: &[(Range<usize>, String)]) -> Vec<bool> {
    let mut keep = vec![false; side.len()];
    let mut cursor = 0usize;
    for (index, (_, token)) in side.iter().enumerate() {
        if let Some(found) = other[cursor..].iter().position(|(_, held)| held == token) {
            keep[index] = true;
            cursor += found + 1;
        }
    }
    keep
}

fn unmatched_spans(
    tokens: &[(Range<usize>, String)],
    keep: &[bool],
    role: SyntaxColor,
) -> Vec<CodeSpan> {
    tokens
        .iter()
        .zip(keep)
        .filter(|(_, keep)| !**keep)
        .map(|((range, _), _)| CodeSpan {
            range: range.clone(),
            role,
        })
        .collect()
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
                        role: SyntaxColor::Keyword,
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

    #[test]
    fn word_spans_mark_tokens_that_have_no_counterpart() {
        let (old, new) = word_spans("old_cache.read()", "verified_cache.read()");
        assert!(old.iter().any(|span| span.role == SyntaxColor::Removed));
        assert!(new.iter().any(|span| span.role == SyntaxColor::Added));
        assert!(
            old.iter()
                .all(|span| span.range.end <= "old_cache.read()".len())
        );
    }

    fn line_event(id: &str) -> DiffViewEvent {
        DiffViewEvent::LineActivated {
            file_id: "source".into(),
            hunk_id: "change".into(),
            line_id: id.to_string().into(),
        }
    }
}
