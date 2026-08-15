//! Page controls over a total the host may or may not know.
//!
//! Which page is current is caller-owned: the control reports the page that was
//! asked for and draws whichever one the caller says is showing.
//!
//! A host that can only say "there is another page" says exactly that. With an
//! unknown total there is no last-page control, no numbered range, and no
//! total in the copy, because a page count nobody counted is a number nobody
//! can trust.

use std::rc::Rc;

use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TextTone, TypeScale};

use crate::controls::button::{Button, ButtonVariant, IconButton};
use crate::controls::select::Select;
use crate::foundation::{
    Disableable, Ident, Selectable, Sizable, StyledExt, text as foundation_text,
};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// How wide the page-size control is. The value occurs once.
const PAGE_SIZE_WIDTH: f32 = 160.0;

/// How many pages the host knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTotal {
    /// The host counted the pages.
    Known(usize),
    /// The host knows only whether one more page exists.
    Unknown { has_next: bool },
}

impl PageTotal {
    pub fn is_known(self) -> bool {
        matches!(self, Self::Known(_))
    }

    pub fn count(self) -> Option<usize> {
        match self {
            Self::Known(total) => Some(total),
            Self::Unknown { .. } => None,
        }
    }
}

/// First, previous, next, and last, plus a numbered range when there is one.
#[derive(IntoElement)]
pub struct Pagination {
    ident: Ident,
    page: usize,
    total: PageTotal,
    siblings: usize,
    size: ControlSize,
    page_size: Option<Entity<Select>>,
    disabled: bool,
    on_select: Option<SelectHandler>,
}

impl std::fmt::Debug for Pagination {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Pagination")
            .field("ident", &self.ident)
            .field("page", &self.page)
            .field("total", &self.total)
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_select.is_some())
            .finish()
    }
}

impl Pagination {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            page: 1,
            total: PageTotal::Unknown { has_next: false },
            siblings: 1,
            size: ControlSize::Sm,
            page_size: None,
            disabled: false,
            on_select: None,
        }
    }

    /// The page the caller says is showing, counted from one.
    pub fn page(mut self, page: usize) -> Self {
        self.page = page.max(1);
        self
    }

    pub fn total_pages(mut self, total: usize) -> Self {
        self.total = PageTotal::Known(total.max(1));
        self
    }

    /// Says only whether another page exists, for a host that cannot count.
    pub fn unknown_total(mut self, has_next: bool) -> Self {
        self.total = PageTotal::Unknown { has_next };
        self
    }

    pub fn total(mut self, total: PageTotal) -> Self {
        self.total = total;
        self
    }

    /// How many numbered pages sit either side of the current one before the
    /// range is elided.
    pub fn siblings(mut self, siblings: usize) -> Self {
        self.siblings = siblings;
        self
    }

    /// The caller-owned control for how many rows a page holds.
    pub fn page_size(mut self, select: Entity<Select>) -> Self {
        self.page_size = Some(select);
        self
    }

    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    fn has_previous(&self) -> bool {
        self.page > 1
    }

    fn has_next(&self) -> bool {
        match self.total {
            PageTotal::Known(total) => self.page < total,
            PageTotal::Unknown { has_next } => has_next,
        }
    }
}

impl Disableable for Pagination {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Pagination {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

/// One entry in the numbered range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageSlot {
    Page(usize),
    /// The pages between the two neighbours it sits among.
    Gap(usize, usize),
}

/// The numbered range for a known total.
///
/// The first and last page are always offered, because they are the two the
/// typist is most likely to want and the only ones an ellipsis must not eat.
pub(crate) fn slots(page: usize, total: usize, siblings: usize) -> Vec<PageSlot> {
    let page = page.clamp(1, total);
    let mut shown: Vec<usize> = Vec::new();
    for candidate in 1..=total {
        let near = candidate.abs_diff(page) <= siblings;
        if candidate == 1 || candidate == total || near {
            shown.push(candidate);
        }
    }

    let mut slots = Vec::with_capacity(shown.len());
    for (index, candidate) in shown.iter().copied().enumerate() {
        if index > 0 {
            let previous = shown[index - 1];
            // An ellipsis standing for exactly one page costs the same room as
            // the page and hides it for nothing, so the page is drawn instead.
            if candidate == previous + 2 {
                slots.push(PageSlot::Page(previous + 1));
            } else if candidate > previous + 1 {
                slots.push(PageSlot::Gap(previous, candidate));
            }
        }
        slots.push(PageSlot::Page(candidate));
    }
    slots
}

impl RenderOnce for Pagination {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let actionable = !self.disabled && self.on_select.is_some();
        let handler = self.on_select.clone().filter(|_| actionable);
        let ident = self.ident.clone();
        let size = self.size;

        let strings = cx.strings().clone();
        let step = |name: &'static str, glyph: Icon, key: StringKey, target: Option<usize>| {
            let label = strings.text(key);
            let enabled = target.is_some();
            let mut control = IconButton::new(ident.child(name), glyph, label)
                .ghost()
                .control_size(size)
                .semantic_parent(ident.semantic_id())
                .disabled(!enabled);
            // A control with nowhere to go installs no handler at all, so it
            // cannot fire even if a host mis-routes an event.
            if let (Some(target), Some(handler)) = (target, handler.clone()) {
                control = control.on_click(move |window, cx| handler(target, window, cx));
            }
            control
        };

        let first = step(
            "first",
            Icon::AltArrowLeft,
            StringKey::PaginationFirst,
            self.has_previous().then_some(1),
        );
        let previous = step(
            "previous",
            Icon::ArrowLeft,
            StringKey::PaginationPrevious,
            self.has_previous().then(|| self.page - 1),
        );
        let next = step(
            "next",
            Icon::ArrowRight,
            StringKey::PaginationNext,
            self.has_next().then(|| self.page + 1),
        );
        // An unknown total has no last page to go to, so no control claims one.
        let last = self.total.count().map(|total| {
            step(
                "last",
                Icon::AltArrowRight,
                StringKey::PaginationLast,
                (self.page < total).then_some(total),
            )
        });

        let numbers = self.total.count().map(|total| {
            let mut range = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(theme.space(Space::Xs)));
            for slot in slots(self.page, total, self.siblings) {
                range = match slot {
                    PageSlot::Page(number) => {
                        let current = number == self.page;
                        let page_id = format!("page-{number}");
                        let mut button = Button::new(ident.child(page_id))
                            .label(cx.numbers().count(number))
                            .variant(if current {
                                ButtonVariant::Secondary
                            } else {
                                ButtonVariant::Ghost
                            })
                            .control_size(size)
                            .semantic_parent(ident.semantic_id())
                            .selected(current);
                        if let (false, Some(handler)) = (current, handler.clone()) {
                            button = button.on_click(move |window, cx| handler(number, window, cx));
                        }
                        range.child(button)
                    }
                    PageSlot::Gap(from, to) => {
                        let gap_id = format!("gap-{from}-{to}");
                        let gap = ident.child(gap_id);
                        let hidden = to - from - 1;
                        range.child(
                            div()
                                .px(px(theme.space(Space::Xs)))
                                .child(
                                    foundation_text(
                                        &theme,
                                        TypeScale::Label,
                                        SharedString::new_static("…"),
                                    )
                                    .text_tone(&theme, gpui_kit_theme::TextTone::Faint),
                                )
                                .semantic_in(
                                    cx,
                                    NodeSpec::new(gap.semantic_id(), Role::Text)
                                        .parent(ident.semantic_id())
                                        .value(cx.numbers().count(hidden))
                                        .text(strings.format(
                                            StringKey::PaginationMorePages,
                                            &[cx.numbers().count(hidden).as_ref()],
                                        )),
                                ),
                        )
                    }
                };
            }
            range
        });

        // With no total to state, the copy says where the typist is and stops
        // there rather than inventing an end.
        let status_text = match self.total {
            PageTotal::Known(total) => strings.format(
                StringKey::PaginationPageOfTotal,
                &[
                    cx.numbers().count(self.page).as_ref(),
                    cx.numbers().count(total).as_ref(),
                ],
            ),
            PageTotal::Unknown { .. } => {
                strings.format(StringKey::PaginationPage, &[cx.numbers().count(self.page).as_ref()])
            }
        };
        let status = foundation_text(&theme, TypeScale::Caption, status_text.clone())
            .text_tone(&theme, TextTone::Muted)
            .semantic_in(
                cx,
                NodeSpec::new(ident.child("status").semantic_id(), Role::Text)
                    .parent(ident.semantic_id())
                    .text(status_text),
            );

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Group).disabled(self.disabled);
        if let Some(total) = self.total.count() {
            spec = spec.value(cx.numbers().count(total));
        }

        div()
            .id(ident.element_id())
            .flex()
            .flex_row()
            .items_center()
            .flex_wrap()
            .gap(px(theme.space(Space::Sm)))
            .child(first)
            .child(previous)
            .children(numbers)
            .child(next)
            .children(last)
            .child(status)
            // A select fills the width it is given, and a page-size control
            // has no business being as wide as the bar it sits in.
            .children(
                self.page_size
                    .map(|select| div().flex_none().w(px(PAGE_SIZE_WIDTH)).child(select)),
            )
            .semantic_in(cx, spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_range_shows_every_page() {
        assert_eq!(
            slots(2, 4, 1),
            vec![
                PageSlot::Page(1),
                PageSlot::Page(2),
                PageSlot::Page(3),
                PageSlot::Page(4)
            ]
        );
    }

    #[test]
    fn a_long_range_elides_the_middle_and_keeps_both_ends() {
        assert_eq!(
            slots(9, 20, 1),
            vec![
                PageSlot::Page(1),
                PageSlot::Gap(1, 8),
                PageSlot::Page(8),
                PageSlot::Page(9),
                PageSlot::Page(10),
                PageSlot::Gap(10, 20),
                PageSlot::Page(20),
            ]
        );
        assert_eq!(
            slots(9, 20, 1)
                .iter()
                .filter_map(|slot| match slot {
                    PageSlot::Gap(from, to) => Some(to - from - 1),
                    PageSlot::Page(_) => None,
                })
                .collect::<Vec<_>>(),
            vec![6, 9],
            "an ellipsis says how many pages it stands for"
        );
    }

    #[test]
    fn an_ellipsis_never_stands_for_a_single_page() {
        assert_eq!(
            slots(2, 5, 1),
            vec![
                PageSlot::Page(1),
                PageSlot::Page(2),
                PageSlot::Page(3),
                PageSlot::Page(4),
                PageSlot::Page(5),
            ]
        );
        assert_eq!(
            slots(2, 6, 1),
            vec![
                PageSlot::Page(1),
                PageSlot::Page(2),
                PageSlot::Page(3),
                PageSlot::Gap(3, 6),
                PageSlot::Page(6),
            ]
        );
    }

    #[test]
    fn an_unknown_total_counts_nothing_and_still_knows_about_one_more() {
        let total = PageTotal::Unknown { has_next: true };
        assert!(!total.is_known());
        assert_eq!(total.count(), None);
    }
}
