//! Read-only code, with line numbers and marked lines.
//!
//! # Colour comes from the name, or from the caller
//!
//! Naming the language with [`CodeView::language`] is the caller's own claim
//! about what the code is, so it is read as well as shown: a name
//! [`crate::content::highlight`] has a table for colours the lines that did
//! not arrive with spans of their own. A name it does not know is displayed
//! and nothing more — nothing here reads the code to work out what it probably
//! is.
//!
//! Past the four classes a scanner can find, deciding that a word is a type or
//! a call needs a grammar, which is the same kind of fact the calendar is:
//! answered correctly only by the library the application already depends on.
//! So a caller with one supplies **pre-classified spans** per line and they
//! win outright, exactly as `Markdown` takes them for a fenced block.
//!
//! What such a caller owes, per line:
//!
//! 1. the line's own text, with no newline in it;
//! 2. [`CodeSpan`] ranges in byte offsets into *that line's* text, not into
//!    the whole document, on character boundaries;
//! 3. ranges sorted ascending and not overlapping.
//!
//! A span that breaks any of those is skipped rather than drawn wrongly: the
//! line stays readable without its colour, and colour that landed in the wrong
//! place would be a lie about the code. The same rule catches the scanner if
//! it is ever wrong. This is the same boundary
//! [`crate::display::highlight`] states for search hits, and it is deliberately
//! the same one.
//!
//! # Long lines scroll, they do not wrap
//!
//! In prose a wrap is invisible; in code a column carries meaning. Indentation
//! is structure, an aligned comment is a column, and a diagnostic that says
//! "column 34" is pointing at a place a wrapped line no longer has. Wrapping
//! also breaks the one thing the gutter claims — that line 41 is one row — so
//! a marked line would stop lining up with its mark. So a long line runs off
//! the edge and the view scrolls to it.
//!
//! # Size, and what virtualization costs here
//!
//! With no [`CodeView::visible_lines`] every line is laid out inside a
//! [`ScrollArea`] that scrolls both ways, which is
//! the mode that keeps the horizontal scroll above.
//!
//! With `visible_lines` the body becomes the virtualized [`List`], which draws
//! only the rows the viewport holds — and then the horizontal scroll goes,
//! for the reason `DataGrid` already states: `uniform_list` owns its own
//! scroll offset and lays every row out at the width it is given. A long line
//! is clipped at the frame in that mode. The two are named rather than
//! blended, because a view that silently dropped the right-hand half of a line
//! would be worse than one that says which mode it is in.
//!
//! # Copying
//!
//! Each shaped line uses GPUI's read-only selection primitive, including
//! syntax-coloured spans, RTL hit testing, pointer capture, and keyboard copy.
//! The optional copy control remains the document-wide operation, including
//! virtualized lines that are not currently mounted.

use gpui::{
    AnyElement, App, ClipboardItem, HighlightStyle, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, Styled, StyledText, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, Theme, TypeScale,
};

use crate::content::highlight::{Cache, Language};
use crate::content::markdown::CodeSpan;
use crate::controls::button::Button;
use crate::data::{List, ListItem};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Disableable, Ident, Sizable, StyledExt};
use crate::layout::{ScrollArea, ScrollAxis};
use crate::motion::keyed;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// How wide the gutter is per digit, and how far a line's text sits from it.
/// Both occur once, so they stay next to the component.
const DIGIT_WIDTH: f32 = 8.0;
const GUTTER_GAP: f32 = 12.0;

/// What a line is being called out for.
///
/// These are the host's claims about the code, not judgements this view makes:
/// nothing here diffs anything or finds an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineMark {
    Added,
    Removed,
    Changed,
    /// Called out for attention without saying anything about correctness.
    Highlighted,
    Error,
}

impl LineMark {
    /// The name the semantic tree publishes, so a test tells the five apart
    /// without reading a colour.
    pub fn name(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Changed => "changed",
            Self::Highlighted => "highlighted",
            Self::Error => "error",
        }
    }

    fn key(self) -> StringKey {
        match self {
            Self::Added => StringKey::CodeLineAdded,
            Self::Removed => StringKey::CodeLineRemoved,
            Self::Changed => StringKey::CodeLineChanged,
            Self::Highlighted => StringKey::CodeLineHighlighted,
            Self::Error => StringKey::CodeLineError,
        }
    }

    /// The wash behind the row.
    ///
    /// The mark is a wash across the whole line and no edge stripe beside it:
    /// a called-out line is already a rectangle the width of the listing, and
    /// a rule drawn down one side of it is a second geometry saying what the
    /// fill has said.
    ///
    /// A line somebody deleted and a line the host says is wrong are two
    /// different claims, and drawing both in the danger colour at one strength
    /// left them the same row twice. The diff classes take the syntax table's
    /// own added and removed colours, which is where the rest of the library
    /// says "this line went" already, and a failure keeps danger to itself.
    fn wash(self, theme: &Theme) -> gpui::Hsla {
        match self {
            Self::Added => theme.colors.syntax.added_wash,
            Self::Removed => theme.colors.syntax.removed_wash,
            Self::Changed => theme
                .colors
                .warning
                .opacity(theme.effects.semantic_wash_faint_alpha),
            Self::Highlighted => theme
                .colors
                .accent
                .opacity(theme.effects.semantic_wash_faint_alpha),
            Self::Error => theme
                .colors
                .danger
                .opacity(theme.effects.semantic_wash_faint_alpha),
        }
    }

    /// A removed line is struck through as well as tinted, because a colour
    /// alone is not a difference anyone reading in monochrome can see.
    fn struck(self) -> bool {
        matches!(self, Self::Removed)
    }
}

/// One line of code, identified by its number.
///
/// A line number is a line's business identity in a file: it survives the view
/// scrolling, and it is what a diagnostic and a review comment already point
/// at. It is not the row's position in whatever slice the caller passed, which
/// is why a view of lines 400 to 450 numbers them 400 to 450.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLine {
    pub number: usize,
    pub text: SharedString,
    pub spans: Vec<CodeSpan>,
    pub mark: Option<LineMark>,
}

impl CodeLine {
    pub fn new(number: usize, text: impl Into<SharedString>) -> Self {
        Self {
            number,
            text: text.into(),
            spans: Vec::new(),
            mark: None,
        }
    }

    /// Pre-classified runs, in byte offsets into this line's own text.
    pub fn spans(mut self, spans: impl IntoIterator<Item = CodeSpan>) -> Self {
        self.spans = spans.into_iter().collect();
        self
    }

    pub fn mark(mut self, mark: LineMark) -> Self {
        self.mark = Some(mark);
        self
    }
}

/// Read-only code with a gutter.
#[derive(IntoElement)]
pub struct CodeView {
    ident: Ident,
    lines: Vec<CodeLine>,
    /// The fence's info string equivalent, shown exactly as written. Nothing
    /// here parses it.
    language: Option<SharedString>,
    line_numbers: bool,
    visible_lines: Option<usize>,
    copyable: bool,
    slots: Slots,
}

impl std::fmt::Debug for CodeView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodeView")
            .field("ident", &self.ident)
            .field("lines", &self.lines.len())
            .field("language", &self.language)
            .field("visible_lines", &self.visible_lines)
            .finish()
    }
}

impl CodeView {
    pub fn new(ident: impl Into<Ident>, lines: impl IntoIterator<Item = CodeLine>) -> Self {
        Self {
            ident: ident.into(),
            lines: lines.into_iter().collect(),
            language: None,
            line_numbers: true,
            visible_lines: None,
            copyable: true,
            slots: Slots::default(),
        }
    }

    /// Splits plain text into numbered lines starting at 1.
    ///
    /// A convenience over [`CodeView::new`] and nothing more: it classifies
    /// nothing and marks nothing.
    pub fn from_text(ident: impl Into<Ident>, text: &str) -> Self {
        Self::new(
            ident,
            text.lines()
                .enumerate()
                .map(|(index, line)| CodeLine::new(index + 1, line.to_string())),
        )
    }

    /// The language's name, shown as written above the code.
    ///
    /// It is also the caller's claim about what the code is, so a name
    /// [`crate::content::highlight`] recognises colours the lines that did not
    /// arrive with spans of their own. A name it does not recognise is shown
    /// and nothing more.
    pub fn language(mut self, language: impl Into<SharedString>) -> Self {
        self.language = Some(language.into());
        self
    }

    pub fn line_numbers(mut self, line_numbers: bool) -> Self {
        self.line_numbers = line_numbers;
        self
    }

    /// Bounds the viewport to `lines` rows and virtualizes the body.
    ///
    /// Horizontal scrolling goes with it; see the module documentation.
    pub fn visible_lines(mut self, lines: usize) -> Self {
        self.visible_lines = Some(lines);
        self
    }

    /// Whether the view carries a control that copies the whole text.
    pub fn copyable(mut self, copyable: bool) -> Self {
        self.copyable = copyable;
        self
    }

    /// The text a copy would put on the clipboard: every line, in order.
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.text.as_ref())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn gutter_width(&self) -> f32 {
        let widest = self
            .lines
            .iter()
            .map(|line| line.number)
            .max()
            .unwrap_or(1)
            .max(1)
            .to_string()
            .len();
        widest as f32 * DIGIT_WIDTH + GUTTER_GAP
    }
}

impl Sizable for CodeView {
    /// Accepted for uniformity with every other component; the code itself is
    /// set from the `code` typographic step whatever this says, because a code
    /// listing that changed size with a button would stop being comparable
    /// with the one beside it.
    fn control_size(self, _size: ControlSize) -> Self {
        self
    }
}

/// One rendered line: the gutter number, the state wash, and the coloured runs.
#[allow(clippy::too_many_arguments)]
fn line_element(
    ident: &Ident,
    line: &CodeLine,
    gutter: f32,
    line_numbers: bool,
    virtualized: bool,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let wash = line
        .mark
        .map_or(gpui::transparent_black(), |mark| mark.wash(theme));
    let struck = line.mark.is_some_and(LineMark::struck);

    let text_ident = ident.child(format!("line-{}-text", line.number));

    let row = div()
        .row()
        .items_start()
        .w_full()
        .pl(px(theme.space(Space::Sm)))
        .h(px(theme.typography.code.line_height))
        .when(line.mark.is_some(), |element| element.bg(wash))
        .when(line_numbers, |element| {
            element.child(
                div()
                    .flex_none()
                    .w(px(gutter))
                    .pr(px(GUTTER_GAP / 2.0))
                    .text_align(gpui::TextAlign::Right)
                    .text_color(theme.colors.text_faint)
                    .child(cx.numbers().count(line.number)),
            )
        })
        // A removed line is struck through as well as washed, so the mark
        // survives a reader who cannot separate the wash from the surface.
        .child(
            div()
                .flex_1()
                .min_w_0()
                .whitespace_nowrap()
                .pl(px(GUTTER_GAP / 2.0))
                .when(struck, |element| element.line_through())
                .child(
                    styled_code(theme, line.text.clone(), &line.spans)
                        .selectable_in_document(
                            text_ident.element_id(),
                            // The line number is this file's reading order, and
                            // it stays that whether or not the line is on
                            // screen, so a selection spanning a scroll still
                            // resolves.
                            text_ident.semantic_id(),
                            line.number as u64,
                        )
                        // In the virtualized mode the rows between two ends of
                        // a selection may never have been laid out, so a copy
                        // that crosses them reports itself incomplete rather
                        // than inventing them. The copy control remains the
                        // whole-document operation.
                        .virtualized_participant(virtualized),
                ),
        );

    match line.mark {
        // Only a marked line is an assertion target. A thousand unmarked lines
        // would bury every other node under rows that repeat their own text.
        Some(mark) => row
            .semantic_in(
                cx,
                NodeSpec::new(line_id(ident, line.number), Role::Row)
                    .parent(ident.semantic_id())
                    // The mark's wording, not the line's source: what the view
                    // claims about the line is the fact it adds, and the code
                    // itself is content nobody here wrote.
                    .text(cx.strings().text(mark.key()))
                    .value(mark.name())
                    .invalid(matches!(mark, LineMark::Error)),
            )
            .into_any_element(),
        None => row.into_any_element(),
    }
}

/// One line's stable id.
///
/// The number is prefixed rather than trailing on its own, because an id whose
/// last segment is a bare number reads as a list position, and the audit that
/// catches that mistake elsewhere is worth more than the two characters.
fn line_id(ident: &Ident, number: usize) -> SharedString {
    ident.child(format!("line-{number}")).semantic_id()
}

/// One shaped code value with its syntax ranges, whoever classified them.
///
/// Keeping the line as one text layout makes selection, bidirectional hit
/// testing, and copying work across syntax-coloured boundaries.
pub(crate) fn styled_code(
    theme: &Theme,
    text: impl Into<SharedString>,
    spans: &[CodeSpan],
) -> StyledText {
    let text = text.into();
    let highlights = code_highlights(theme, &text, spans);
    StyledText::new(text).with_highlights(highlights)
}

fn code_highlights(
    theme: &Theme,
    text: &str,
    spans: &[CodeSpan],
) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    let mut end = 0;
    spans
        .iter()
        .filter_map(|span| {
            let valid = span.range.start >= end
                && span.range.start < span.range.end
                && span.range.end <= text.len()
                && text.is_char_boundary(span.range.start)
                && text.is_char_boundary(span.range.end);
            valid.then(|| {
                end = span.range.end;
                (
                    span.range.clone(),
                    HighlightStyle::color(theme.colors.syntax.get(span.role)),
                )
            })
        })
        .collect()
}

impl Slotted for CodeView {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for CodeView {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let gutter = self.gutter_width();
        let line_numbers = self.line_numbers;
        let total = self.lines.len();
        let body_ident = self.ident.child("lines");

        // Naming the language is the caller's own claim about what the code
        // is, so it is read as well as shown. A line that arrived with spans
        // keeps them: the caller's grammar outranks this scanner. A line
        // holding a newline is left alone entirely, because the joined source
        // would no longer line up with the rows and colour would land on the
        // wrong code.
        let source = SharedString::from(self.text());
        if let Some(known) = self.language.as_deref().and_then(Language::named)
            && self
                .lines
                .iter()
                .all(|line| line.spans.is_empty() && !line.text.contains('\n'))
        {
            let cache = keyed::slot::<Cache>(
                &self.ident.child("colour").semantic_id(),
                window.window_handle().window_id(),
                cx,
            );
            let coloured = cache.borrow_mut().lines(known, &source);
            for (line, spans) in self.lines.iter_mut().zip(coloured.iter()) {
                line.spans = spans.clone();
            }
        }

        let copy = self.copyable.then(|| {
            let clipboard = source.to_string();
            Button::new(self.ident.child("copy"))
                .label(cx.strings().text(StringKey::Copy))
                .ghost()
                .control_size(ControlSize::Xs)
                .semantic_parent(self.ident.semantic_id())
                .disabled(clipboard.is_empty())
                .on_click(move |_, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(clipboard.clone()));
                })
        });

        let body: AnyElement = if total == 0 {
            self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                EmptyState::new(
                    self.ident.child("empty"),
                    cx.strings().text(StringKey::CodeEmpty),
                )
                .kind(EmptyKind::Empty)
                .into_any_element()
            })
        } else if let Some(visible) = self.visible_lines {
            let lines = std::rc::Rc::new(self.lines);
            let list_ident = body_ident.clone();
            let theme_for_rows = theme.clone();
            List::new(body_ident.clone(), total, move |index, _window, cx| {
                let line = &lines[index];
                ListItem::new(
                    line_id(&list_ident, line.number),
                    line_element(
                        &list_ident,
                        line,
                        gutter,
                        line_numbers,
                        true,
                        &theme_for_rows,
                        cx,
                    ),
                )
            })
            .row_height(theme.typography.code.line_height)
            .visible_lines(visible)
            .into_any_element()
        } else {
            // Every line is laid out, so the block is as tall as its code. A
            // scroll area that filled the height it was offered was offered
            // none by a column that states no height, and the body collapsed
            // to an empty strip. Only the horizontal scroll does work here.
            ScrollArea::new(body_ident.clone())
                .axis(ScrollAxis::Both)
                .fit_height()
                .child(
                    div().column().children(
                        self.lines
                            .iter()
                            .map(|line| {
                                line_element(
                                    &body_ident,
                                    line,
                                    gutter,
                                    line_numbers,
                                    false,
                                    &theme,
                                    cx,
                                )
                            })
                            .collect::<Vec<_>>(),
                    ),
                )
                .into_any_element()
        };

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            // The block's rows run edge to edge and pad themselves: a row
            // inset by the card's own padding leaves its mark's wash stopping
            // short of the card, which reads as a band that failed to finish
            // rather than as a line that is marked.
            .py_token(&theme, Space::Sm)
            .radius(&theme, Radius::Card)
            .overflow_hidden()
            .frame(&theme, Surface::Raised, Elevation::Raised)
            .when(self.language.is_some() || copy.is_some(), |element| {
                element.child(
                    div()
                        .row()
                        .w_full()
                        .px_token(&theme, Space::Sm)
                        .justify_between()
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_faint)
                        .child(div().child(self.language.clone().unwrap_or_default()))
                        .children(copy),
                )
            })
            .child(
                div()
                    .relative()
                    .w_full()
                    .overflow_hidden()
                    .mono(&theme)
                    .text_size(px(theme.typography.code.size))
                    .line_height(px(theme.typography.code.line_height))
                    .text_color(theme.colors.text)
                    .child(body)
                    // A column carries meaning in code, so a long line runs off
                    // the edge rather than wrapping — and the edge it runs off
                    // is a fade, not a cut through the middle of a word that
                    // reads as a rendering failure.
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right_0()
                            .w(px(theme.effects.edge_fade_band))
                            .bg(gpui::linear_gradient(
                                90.0,
                                gpui::linear_color_stop(
                                    theme.surface(Surface::Raised).opacity(0.0),
                                    0.0,
                                ),
                                gpui::linear_color_stop(theme.surface(Surface::Raised), 1.0),
                            )),
                    ),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .when_language(self.language)
                    // A container publishes how much it holds, which here is
                    // the number of lines it was handed — not the number drawn.
                    .value(cx.numbers().count(total)),
            )
    }
}

/// Adds the language to a spec only when the caller supplied one.
trait LanguageSpec {
    fn when_language(self, language: Option<SharedString>) -> Self;
}

impl LanguageSpec for NodeSpec {
    fn when_language(self, language: Option<SharedString>) -> Self {
        match language {
            Some(language) => self.text(language),
            None => self,
        }
    }
}

/// Adds the `visible_lines` bound to a `List`, named for this view.
trait VisibleLines {
    fn visible_lines(self, lines: usize) -> Self;
}

impl VisibleLines for List {
    fn visible_lines(self, lines: usize) -> Self {
        self.visible_rows(lines)
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit_theme::SyntaxColor;

    use super::*;

    #[test]
    fn every_mark_publishes_a_name_of_its_own() {
        let names = [
            LineMark::Added,
            LineMark::Removed,
            LineMark::Changed,
            LineMark::Highlighted,
            LineMark::Error,
        ]
        .map(LineMark::name);
        let mut sorted = names.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
    }

    #[test]
    fn a_span_naming_no_slice_of_the_line_is_skipped() {
        let theme = Theme::studio_dark();
        let line = CodeLine::new(1, "let x = 1;").spans([CodeSpan {
            range: 40..50,
            role: SyntaxColor::Keyword,
        }]);
        assert!(code_highlights(&theme, line.text.as_ref(), &line.spans).is_empty());
    }

    #[test]
    fn a_view_keeps_the_numbers_it_was_given() {
        let view = CodeView::new(
            "review.hunk",
            [CodeLine::new(400, "a"), CodeLine::new(401, "b")],
        );
        assert_eq!(view.lines[0].number, 400);
        assert_eq!(view.text(), "a\nb");
    }

    #[test]
    fn splitting_text_numbers_from_one() {
        let view = CodeView::from_text("file", "first\nsecond\nthird");
        assert_eq!(view.lines.len(), 3);
        assert_eq!(view.lines[2].number, 3);
    }
}
