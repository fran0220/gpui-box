//! A compact outline of a long surface, and a way back into it.
//!
//! A conversation, a log, a document: something you have been through and now
//! have to get back to a particular part of. A scrollbar says where you are as
//! a fraction, which is the one thing about your position that does not help —
//! "sixty percent" is not a place. A [`Outline`] says where the *places* are:
//! a mark for each one worth returning to, the one you are reading brighter
//! than the rest, what each one says under the pointer, and a click that
//! travels there rather than jumping.
//!
//! # The footprint does not grow
//!
//! The obvious minimap has one mark per place, which works until there are
//! four hundred places and the marks are a solid line. Past what fits, marks
//! become *buckets*: even ranges over the whole surface, each drawn once and
//! saying how many it stands for. So the outline spans the entire
//! conversation at any length, and the thing it costs — that a bucket's mark
//! lands you at the first place in its range rather than at an exact one — is
//! the right thing to give up, because at four hundred places you are looking
//! for a region anyway.
//!
//! Under the limit every bucket holds exactly one mark, so nothing is
//! condensed until condensing is the only alternative to a solid line.
//!
//! # It maps a surface it does not own
//!
//! The list is named, not held. The minimap reads which row is at the top of
//! that list and scrolls it by the same name, so it composes with a list the
//! caller built however they liked, and there is no second copy of the scroll
//! position to disagree with the first.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Theme};

use crate::data::viewport::{glide_to_row, viewed_rows};
use crate::foundation::Ident;
use crate::overlay::Tooltipped;
use crate::strings::{ActiveStrings, StringKey};

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// The height of one mark's hit target, and the gap between two of them.
///
/// The target is much taller than the line it draws, because a two-pixel line
/// is not something a pointer can be asked to hit.
const SLOT: f32 = 10.0;
const GAP: f32 = 3.0;

/// Room kept clear above and below the stack, so the outline reads as a group
/// rather than as something jammed against the ends of its container.
const MARGIN: f32 = 24.0;

/// The most marks an outline ever draws, however tall the window is.
///
/// A cap rather than "as many as fit" because the outline is a glance, and a
/// glance does not scale: forty marks down the side of a tall screen is a
/// texture, not a set of places. When the window is tall enough for more, the
/// marks stay put and the buckets stay even.
const MOST: usize = 12;

/// The width of a mark at rest, and under the pointer.
const MARK_WIDTH: f32 = 12.0;
const MARK_WIDTH_HOVERED: f32 = 20.0;
const MARK_HEIGHT: f32 = 2.0;

/// A place in the mapped surface worth returning to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    id: SharedString,
    row: usize,
    title: SharedString,
    detail: Option<SharedString>,
}

impl Mark {
    /// `row` is the mapped list's row index, which is what makes the mark a
    /// destination rather than only a label.
    pub fn new(id: impl Into<SharedString>, row: usize, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            row,
            title: title.into(),
            detail: None,
        }
    }

    /// A second line for the preview: what followed this place, typically, so
    /// the reader recognises the one they want without going there.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn row(&self) -> usize {
        self.row
    }
}

/// The outline of a long surface. Reports the mark that was chosen, and
/// travels there.
#[derive(IntoElement)]
pub struct Outline {
    ident: Ident,
    over: Option<Ident>,
    marks: Vec<Mark>,
    slots: Option<usize>,
    on_select: Option<SelectHandler>,
}

impl std::fmt::Debug for Outline {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Outline")
            .field("ident", &self.ident)
            .field("over", &self.over)
            .field("marks", &self.marks.len())
            .field("slots", &self.slots)
            .finish()
    }
}

impl Outline {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            over: None,
            marks: Vec::new(),
            slots: None,
            on_select: None,
        }
    }

    /// The list this is an outline of, by name.
    ///
    /// Without one the outline still draws and still reports, but it cannot
    /// know which mark you are reading and clicking one moves nothing — which
    /// is why every real use names a list.
    pub fn over(mut self, list: impl Into<Ident>) -> Self {
        self.over = Some(list.into());
        self
    }

    pub fn marks(mut self, marks: impl IntoIterator<Item = Mark>) -> Self {
        self.marks.extend(marks);
        self
    }

    pub fn mark(mut self, mark: Mark) -> Self {
        self.marks.push(mark);
        self
    }

    /// Draws exactly this many marks instead of as many as the mapped list's
    /// height allows.
    ///
    /// For a caller whose outline is not beside the list it maps, and for
    /// tests, which have no window height worth measuring.
    pub fn slots(mut self, slots: usize) -> Self {
        self.slots = Some(slots.max(1));
        self
    }

    /// Reports the mark that was chosen. The travel happens either way: the
    /// minimap owns the scroll, and this is for a host that wants to know.
    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// The preview a mark shows under the pointer.
    fn preview(&self, mark: &Mark, held: usize, cx: &App) -> SharedString {
        let mut text = mark.title.to_string();
        if let Some(detail) = &mark.detail {
            text.push('\n');
            text.push_str(detail);
        }
        // A condensed mark says so, because otherwise its preview is a claim
        // about one place that is really about several.
        if held > 1 {
            text.push('\n');
            text.push_str(
                &cx.strings()
                    .format(StringKey::OutlineMarks, &[&held.to_string()]),
            );
        }
        SharedString::from(text)
    }

    fn tick(
        &self,
        range: (usize, usize),
        current: Option<usize>,
        theme: &Theme,
        cx: &mut App,
    ) -> gpui::AnyElement {
        let (start, end) = range;
        // The bucket shows the place you are reading when that falls inside
        // it, and its first place otherwise — so hovering the bright mark
        // previews what is on screen rather than something near it.
        let shown = current
            .filter(|at| (start..end).contains(at))
            .unwrap_or(start);
        let mark = &self.marks[shown];
        let ident = self.ident.child(mark.id.as_ref());
        let reading = current.is_some_and(|at| (start..end).contains(&at));
        let held = end - start;

        let row = mark.row;
        let list = self.over.clone();
        let id = mark.id.clone();
        let handler = self.on_select.clone();

        let hover_group = ident.child("hover").semantic_id();
        div()
            .id(ident.element_id())
            .group(hover_group.clone())
            .h(px(SLOT))
            .w_full()
            .flex()
            .items_center()
            .cursor_pointer()
            .child(
                div()
                    .h(px(MARK_HEIGHT))
                    .w(px(MARK_WIDTH))
                    .rounded(px(MARK_HEIGHT / 2.0))
                    // Only the pointer widens a mark. The one you are reading
                    // reads brighter and stays the same size, so the outline's
                    // shape does not change under you as you scroll past it.
                    .group_hover(hover_group.clone(), |style| style.w(px(MARK_WIDTH_HOVERED)))
                    // The one you are reading is the only one drawn in the
                    // text colour: the outline is read at a glance, and a
                    // glance can only carry one distinction.
                    .bg(if reading {
                        theme.colors.text
                    } else {
                        theme.colors.hairline_strong
                    }),
            )
            .tip(ident.clone(), self.preview(mark, held, cx))
            .on_click(move |_, window, cx| {
                if let Some(list) = &list {
                    glide_to_row(list, row, cx);
                }
                if let Some(handler) = &handler {
                    handler(id.clone(), window, cx);
                }
            })
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Button)
                    .parent(self.ident.semantic_id())
                    .text(mark.title.clone())
                    // How many places this mark stands for, so a snapshot
                    // shows that the outline condensed rather than lost them.
                    .value(held.to_string())
                    .selected(reading),
            )
            .into_any_element()
    }
}

impl RenderOnce for Outline {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // One mark is not a map. Two is the least that can tell you which way
        // to go, so below that the outline draws nothing at all rather than
        // putting a control on screen that cannot change anything.
        if self.marks.len() < 2 {
            return div().into_any_element();
        }

        let theme = cx.theme().clone();
        let viewed = self
            .over
            .as_ref()
            .and_then(|list| viewed_rows(list, cx))
            .unwrap_or((0, 0.0));
        let current = current_mark(&self.marks, viewed.0);
        let slots = self.slots.unwrap_or_else(|| fitting(viewed.1));
        let ranges = buckets(self.marks.len(), slots);

        div()
            .flex()
            .flex_col()
            .items_start()
            .justify_center()
            .gap(px(GAP))
            .children(
                ranges
                    .into_iter()
                    .map(|range| self.tick(range, current, &theme, cx))
                    .collect::<Vec<_>>(),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::List)
                    .value(self.marks.len().to_string()),
            )
            .into_any_element()
    }
}

/// Which mark the reader is inside, for a list scrolled to `top`.
///
/// The last mark at or above the top row: the place whose section fills the
/// screen. Above the first mark the first one is still it, because a reader
/// who has scrolled to the very beginning has not left the conversation.
fn current_mark(marks: &[Mark], top: usize) -> Option<usize> {
    if marks.is_empty() {
        return None;
    }
    Some(
        marks
            .iter()
            .rposition(|mark| mark.row <= top)
            .unwrap_or_default(),
    )
}

/// How many marks a frame `height` tall has room for, never fewer than one and
/// never more than [`MOST`].
///
/// A height of zero is a frame that has not been laid out yet rather than a
/// frame with no room, so it gets the full count for that one frame instead of
/// collapsing the outline to a single mark and expanding it again.
fn fitting(height: f32) -> usize {
    if height <= 0.0 {
        return MOST;
    }
    let usable = (height - 2.0 * MARGIN).max(SLOT);
    (((usable + GAP) / (SLOT + GAP)).floor() as usize).clamp(1, MOST)
}

/// `n` marks divided into at most `slots` even ranges, each `[start, end)`.
///
/// With `n <= slots` every range holds exactly one mark, which is the
/// uncondensed outline. Ranges are contiguous and cover everything, so no mark
/// is unreachable and none is reachable twice.
fn buckets(n: usize, slots: usize) -> Vec<(usize, usize)> {
    if n == 0 {
        return Vec::new();
    }
    let slots = slots.clamp(1, n);
    (0..slots)
        .map(|slot| (slot * n / slots, (slot + 1) * n / slots))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marks(rows: &[usize]) -> Vec<Mark> {
        rows.iter()
            .enumerate()
            .map(|(ix, row)| Mark::new(format!("m{ix}"), *row, format!("Mark {ix}")))
            .collect()
    }

    #[test]
    fn under_the_cap_every_bucket_is_one_mark() {
        let ranges = buckets(5, MOST);
        assert_eq!(ranges.len(), 5);
        assert!(
            ranges
                .iter()
                .enumerate()
                .all(|(slot, &(start, end))| start == slot && end == slot + 1),
            "nothing is condensed until condensing is the only alternative"
        );
    }

    #[test]
    fn past_the_cap_the_buckets_cover_everything_evenly() {
        let ranges = buckets(100, 8);
        assert_eq!(ranges.len(), 8);
        assert_eq!(ranges[0].0, 0, "the outline starts at the beginning");
        assert_eq!(ranges[7].1, 100, "and reaches the end");
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].1, pair[1].0, "no mark falls between two buckets");
        }
        for &(start, end) in &ranges {
            assert!(
                (12..=13).contains(&(end - start)),
                "one bucket holds {} of a hundred marks",
                end - start
            );
        }
    }

    #[test]
    fn degenerate_outlines_do_not_panic_or_lose_marks() {
        assert!(buckets(0, 8).is_empty());
        // More slots than marks is the uncondensed case, not eight empty
        // buckets over three marks.
        assert_eq!(buckets(3, 8).len(), 3);
        // No slots at all still has to place every mark somewhere.
        assert_eq!(buckets(3, 0), vec![(0, 3)]);
    }

    #[test]
    fn the_current_mark_is_the_one_whose_section_you_are_in() {
        let outline = marks(&[0, 5, 9]);
        assert_eq!(current_mark(&outline, 0), Some(0));
        assert_eq!(current_mark(&outline, 4), Some(0));
        assert_eq!(current_mark(&outline, 5), Some(1));
        assert_eq!(current_mark(&outline, 8), Some(1));
        assert_eq!(
            current_mark(&outline, 400),
            Some(2),
            "past the last mark it is still the last mark"
        );
        assert_eq!(
            current_mark(&marks(&[3, 7]), 1),
            Some(0),
            "above the first mark the reader has not left the first section"
        );
        assert_eq!(current_mark(&[], 4), None);
    }

    #[test]
    fn an_unmeasured_frame_is_not_a_frame_with_no_room() {
        assert_eq!(fitting(0.0), MOST, "before layout, assume there is room");
        assert_eq!(fitting(880.0), MOST, "a tall window does not grow the map");
        assert_eq!(fitting(2000.0), MOST);
        assert!(fitting(100.0) < MOST, "a short one does shrink it");
        assert_eq!(fitting(1.0), 1, "and never below a single mark");
    }
}
