//! Marking ranges of text that somebody else owns.
//!
//! # The boundary, and what the caller owes
//!
//! This crate does not search text. Deciding that a run of characters answers
//! a query is the same kind of judgement as deciding that a word is a keyword:
//! it needs to know about case folding, word boundaries, normalisation forms,
//! whether the query is a regular expression, and what the host considers one
//! document — none of which a product-neutral component can answer, and all of
//! which the application that owns the text already has.
//!
//! So the component takes **ranges**, not a query. The caller owes:
//!
//! 1. the exact [`SharedString`] it wants marked, handed to this component;
//! 2. byte offsets into *that* string, on character boundaries;
//! 3. ranges sorted ascending and not overlapping one another;
//! 4. which of them, by index, is the current one — or none.
//!
//! A range that breaks any of those is skipped rather than drawn wrongly and
//! rather than panicking, because a highlight is decoration over text that is
//! still readable without it: losing a mark is recoverable, losing the line is
//! not. [`HighlightedText::published_hits`] reports how many were actually
//! drawn, so a caller that got the offsets wrong can see it.
//!
//! The same boundary serves the read-only code view, which takes
//! pre-classified [`CodeSpan`](crate::content::CodeSpan)s for exactly the same
//! reason.

use std::ops::Range;

use gpui::{
    App, HighlightStyle, IntoElement, ParentElement, RenderOnce, SharedString, Styled, StyledText,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius};

use crate::foundation::Ident;

/// A run of text with some of it marked.
///
/// The current mark is a different treatment from the rest, not a stronger
/// one: "which hit am I on" and "where are the other hits" are two questions,
/// and one shade of the same colour answers neither clearly.
#[derive(Debug, IntoElement)]
pub struct HighlightedText {
    ident: Option<Ident>,
    selection_ident: Option<Ident>,
    text: SharedString,
    hits: Vec<Range<usize>>,
    current: Option<usize>,
    monospace: bool,
}

impl HighlightedText {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            ident: None,
            selection_ident: None,
            text: text.into(),
            hits: Vec::new(),
            current: None,
            monospace: false,
        }
    }

    /// Publishes a node under this id. Without one nothing is published, the
    /// way a decorative `Badge` publishes nothing.
    pub fn id(mut self, ident: impl Into<Ident>) -> Self {
        self.ident = Some(ident.into());
        self
    }

    /// Enables native text selection without publishing the text in a
    /// semantic snapshot. This is appropriate for redacted logs and other
    /// caller-owned content whose bounds still need stable transient state.
    pub fn selectable(mut self, ident: impl Into<Ident>) -> Self {
        self.selection_ident = Some(ident.into());
        self
    }

    /// The ranges to mark, in byte offsets into the text this was given.
    pub fn hits(mut self, hits: impl IntoIterator<Item = Range<usize>>) -> Self {
        self.hits = hits.into_iter().collect();
        self
    }

    /// Which hit, by index into [`HighlightedText::hits`], is the current one.
    pub fn current(mut self, index: usize) -> Self {
        self.current = Some(index);
        self
    }

    pub fn monospace(mut self, monospace: bool) -> Self {
        self.monospace = monospace;
        self
    }

    /// How many of the given ranges name text this string actually holds.
    ///
    /// A caller whose offsets are wrong gets a smaller number than it handed
    /// in, which is how a broken boundary is noticed rather than guessed at.
    pub fn published_hits(&self) -> usize {
        segments(self.text.as_ref(), &self.hits)
            .iter()
            .filter(|segment| segment.hit.is_some())
            .count()
    }
}

/// One run of the text: either plain, or the `hit`th mark.
struct Segment {
    text: String,
    hit: Option<usize>,
}

/// Cuts the text at the range boundaries, skipping anything that does not name
/// a real slice or that runs backwards over what came before.
fn segments(text: &str, hits: &[Range<usize>]) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    let mut cut = 0usize;
    for (index, range) in hits.iter().enumerate() {
        if range.start < cut || range.start >= range.end {
            continue;
        }
        let (Some(before), Some(inside)) =
            (text.get(cut..range.start), text.get(range.start..range.end))
        else {
            continue;
        };
        if !before.is_empty() {
            out.push(Segment {
                text: before.to_string(),
                hit: None,
            });
        }
        out.push(Segment {
            text: inside.to_string(),
            hit: Some(index),
        });
        cut = range.end;
    }
    if let Some(rest) = text.get(cut..)
        && !rest.is_empty()
    {
        out.push(Segment {
            text: rest.to_string(),
            hit: None,
        });
    }
    out
}

impl RenderOnce for HighlightedText {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let current = self.current;
        // What was drawn, not what was asked for: a range naming no slice of
        // this string produced no mark, and publishing the request would make
        // the tree claim a highlight nobody can see.
        let drawn = self.published_hits();
        let mut offset = 0;
        let highlights = segments(self.text.as_ref(), &self.hits)
            .into_iter()
            .filter_map(|segment| {
                let start = offset;
                offset += segment.text.len();
                segment.hit.map(|hit| {
                    let is_current = Some(hit) == current;
                    (
                        start..offset,
                        HighlightStyle {
                            color: is_current.then_some(theme.colors.text_on_accent),
                            background_color: Some(if is_current {
                                theme.colors.accent
                            } else {
                                theme
                                    .colors
                                    .accent
                                    .opacity(theme.effects.selected_ring_alpha)
                            }),
                            background_radius: Some(px(theme.radius(Radius::Small))),
                            ..Default::default()
                        },
                    )
                })
            })
            .collect::<Vec<_>>();
        let text = StyledText::new(self.text.clone()).with_highlights(highlights);
        let selectable_id = self
            .selection_ident
            .as_ref()
            .map(|ident| ident.child("text").element_id());

        let mut element = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .when(self.monospace, |element| {
                element
                    .font_family(theme.typography.mono.clone())
                    .text_size(px(theme.typography.code.size))
                    .line_height(px(theme.typography.code.line_height))
            });
        element = match selectable_id {
            Some(id) => element.child(text.selectable(id)),
            None => element.child(text),
        };
        match self.ident {
            Some(ident) => {
                // The text itself is the caller's content and is published as
                // it is, the way a Markdown paragraph is; the count of marks
                // is the fact this component adds.
                element
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.semantic_id(), Role::Text)
                            .text(self.text.clone())
                            .value(drawn.to_string()),
                    )
                    .into_any_element()
            }
            None => element.into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_outside_the_text_is_skipped_rather_than_drawn() {
        let text = HighlightedText::new("abcdef").hits([0..3, 10..20]);
        assert_eq!(text.published_hits(), 1);
    }

    #[test]
    fn a_range_that_runs_back_over_an_earlier_one_is_skipped() {
        let text = HighlightedText::new("abcdef").hits([2..4, 1..3]);
        assert_eq!(text.published_hits(), 1);
    }

    #[test]
    fn a_range_cutting_a_character_in_half_is_skipped() {
        // "é" is two bytes wide, so 0..1 cuts it in half and names no slice
        // this string holds; 0..2 names the whole character and does.
        let text = HighlightedText::new("éx").hits([0..1, 0..2]);
        assert_eq!(text.published_hits(), 1);
    }

    #[test]
    fn the_whole_text_survives_being_cut_up() {
        let joined: String = segments("hello world", &[0..5, 6..11])
            .into_iter()
            .map(|segment| segment.text)
            .collect();
        assert_eq!(joined, "hello world");
    }
}
