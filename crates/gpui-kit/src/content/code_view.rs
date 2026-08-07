//! Read-only code, with line numbers and marked lines.
//!
//! # No grammar, and no new dependency
//!
//! Deciding that a word is a keyword needs a grammar, which is the same kind
//! of fact the calendar is: answered correctly only by the library the
//! application already depends on. `docs/coverage.md` records syntax
//! highlighting as out of scope, and this view keeps that line — it takes
//! **pre-classified spans** from the caller, exactly as `Markdown` colours a
//! fenced block.
//!
//! What the caller owes, per line:
//!
//! 1. the line's own text, with no newline in it;
//! 2. [`CodeSpan`] ranges in byte offsets into *that line's* text, not into
//!    the whole document, on character boundaries;
//! 3. ranges sorted ascending and not overlapping.
//!
//! A span that breaks any of those is skipped rather than drawn wrongly: the
//! line stays readable without its colour, and colour that landed in the wrong
//! place would be a lie about the code. This is the same boundary
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
//! [`ScrollArea`](crate::layout::ScrollArea) that scrolls both ways, which is
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
//! GPUI offers no text-selection primitive, which `docs/coverage.md` records
//! as a library-wide gap; nothing rendered here can be selected with a
//! pointer. So the view carries a control that copies the whole text, the way
//! a `Markdown` code block does, and does not pretend a caret exists.

use gpui::{
    AnyElement, App, ClipboardItem, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Theme, TypeScale};

use crate::content::markdown::CodeSpan;
use crate::controls::button::Button;
use crate::data::{List, ListItem};
use crate::display::empty::{EmptyKind, EmptyState};
use crate::foundation::{Disableable, Ident, Sizable, StyledExt};
use crate::layout::{ScrollArea, ScrollAxis};
use crate::strings::{ActiveStrings, StringKey};

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

    /// The rail colour and the wash behind the row.
    fn colors(self, theme: &Theme) -> (gpui::Hsla, gpui::Hsla) {
        let tint = match self {
            Self::Added => theme.colors.success,
            Self::Removed => theme.colors.danger,
            Self::Changed => theme.colors.warning,
            Self::Highlighted => theme.colors.accent,
            Self::Error => theme.colors.danger,
        };
        (tint, tint.opacity(theme.effects.selected_ring_alpha))
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

    /// The language's name, shown as written. Nothing here reads it.
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

/// One rendered line: the gutter number, the rail, and the coloured runs.
fn line_element(
    ident: &Ident,
    line: &CodeLine,
    gutter: f32,
    line_numbers: bool,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let (rail, wash) = line
        .mark
        .map(|mark| mark.colors(theme))
        .unzip_or(theme.colors.hairline, gpui::transparent_black());
    let struck = line.mark.is_some_and(LineMark::struck);

    let row = div()
        .row()
        .items_start()
        .w_full()
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
                    .child(SharedString::from(line.number.to_string())),
            )
        })
        // The rail is what a reader in monochrome sees, and it is drawn even
        // when the gutter is off, because the mark is the fact.
        .child(
            div()
                .flex_none()
                .w(px(theme.borders.thick))
                .h_full()
                .when(line.mark.is_some(), |element| element.bg(rail)),
        )
        .child(
            // The runs sit in a row. Without one they are laid out as blocks
            // and every run paints from the same left edge, one word over the
            // next, which is what a line with more than one span looked like.
            div()
                .row()
                .items_baseline()
                .flex_1()
                .min_w_0()
                .whitespace_nowrap()
                .pl(px(GUTTER_GAP / 2.0))
                .when(struck, |element| element.line_through())
                .children(runs(theme, line)),
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

/// The coloured runs of one line, skipping any span that does not name a slice
/// this line actually holds.
///
/// Each run holds its own width. Inside a row that lays its children out in
/// one direction, a run that is allowed to shrink is shrunk to nothing, and
/// every run then paints from the same left edge with one word on top of the
/// next.
fn runs(theme: &Theme, line: &CodeLine) -> Vec<AnyElement> {
    let text = line.text.as_ref();
    let mut out: Vec<AnyElement> = Vec::new();
    let mut cut = 0usize;
    for span in &line.spans {
        if span.range.start < cut || span.range.start >= span.range.end {
            continue;
        }
        let (Some(before), Some(inside)) = (
            text.get(cut..span.range.start),
            text.get(span.range.start..span.range.end),
        ) else {
            continue;
        };
        if !before.is_empty() {
            out.push(
                div()
                    .flex_none()
                    .child(SharedString::from(before.to_string()))
                    .into_any_element(),
            );
        }
        out.push(
            div()
                .flex_none()
                .text_color(span.tone.color(theme))
                .child(SharedString::from(inside.to_string()))
                .into_any_element(),
        );
        cut = span.range.end;
    }
    if let Some(rest) = text.get(cut..)
        && !rest.is_empty()
    {
        out.push(
            div()
                .flex_none()
                .child(SharedString::from(rest.to_string()))
                .into_any_element(),
        );
    }
    out
}

/// Splits an optional pair into two values with defaults, so a marked and an
/// unmarked line take the same code path.
trait UnzipOr<A, B> {
    fn unzip_or(self, first: A, second: B) -> (A, B);
}

impl<A, B> UnzipOr<A, B> for Option<(A, B)> {
    fn unzip_or(self, first: A, second: B) -> (A, B) {
        self.unwrap_or((first, second))
    }
}

impl RenderOnce for CodeView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let gutter = self.gutter_width();
        let line_numbers = self.line_numbers;
        let total = self.lines.len();
        let body_ident = self.ident.child("lines");

        let copy = self.copyable.then(|| {
            let clipboard = self.text();
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
            EmptyState::new(
                self.ident.child("empty"),
                cx.strings().text(StringKey::CodeEmpty),
            )
            .kind(EmptyKind::Empty)
            .into_any_element()
        } else if let Some(visible) = self.visible_lines {
            let lines = std::rc::Rc::new(self.lines);
            let list_ident = body_ident.clone();
            let theme_for_rows = theme.clone();
            List::new(body_ident.clone(), total, move |index, _window, cx| {
                let line = &lines[index];
                ListItem::new(
                    line_id(&list_ident, line.number),
                    line_element(&list_ident, line, gutter, line_numbers, &theme_for_rows, cx),
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
                                line_element(&body_ident, line, gutter, line_numbers, &theme, cx)
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
            .p_token(&theme, Space::Sm)
            .radius(&theme, Radius::Card)
            .hairline(&theme)
            .bg(theme.colors.raised)
            .when(self.language.is_some() || copy.is_some(), |element| {
                element.child(
                    div()
                        .row()
                        .w_full()
                        .justify_between()
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_faint)
                        .child(div().child(self.language.clone().unwrap_or_default()))
                        .children(copy),
                )
            })
            .child(
                div()
                    .w_full()
                    .font_family(theme.typography.mono.clone())
                    .text_size(px(theme.typography.code.size))
                    .line_height(px(theme.typography.code.line_height))
                    .text_color(theme.colors.text)
                    .child(body),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Region)
                    .when_language(self.language)
                    // A container publishes how much it holds, which here is
                    // the number of lines it was handed — not the number drawn.
                    .value(total.to_string()),
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
    use super::*;
    use crate::display::badge::Tone;

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
            tone: Tone::Accent,
        }]);
        // One run, the whole line, because the span named nothing real.
        assert_eq!(runs(&theme, &line).len(), 1);
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
