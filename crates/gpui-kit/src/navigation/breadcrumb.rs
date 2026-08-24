//! The trail of places that leads to where the typist is now.
//!
//! The last crumb is the current place. It is not a way to go anywhere, so it
//! is published as text rather than as a link, and no handler is installed on
//! it. Every earlier crumb reports its own id and lets the caller decide
//! whether the move happens.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};

use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{
    FocusRing, Hoverable, Ident, Pressable, StyledExt, text as foundation_text,
};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type RevealHandler = Rc<dyn Fn(Vec<SharedString>, &mut Window, &mut App)>;

/// One place on the trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crumb {
    pub id: SharedString,
    pub label: SharedString,
}

impl Crumb {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// A trail of crumbs, collapsed in the middle when it grows too long.
#[derive(IntoElement)]
pub struct Breadcrumb {
    ident: Ident,
    crumbs: Vec<Crumb>,
    max_visible: Option<usize>,
    on_select: Option<SelectHandler>,
    on_reveal: Option<RevealHandler>,
}

impl std::fmt::Debug for Breadcrumb {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Breadcrumb")
            .field("ident", &self.ident)
            .field("crumbs", &self.crumbs.len())
            .field("max_visible", &self.max_visible)
            .field("has_handler", &self.on_select.is_some())
            .finish()
    }
}

impl Breadcrumb {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            crumbs: Vec::new(),
            max_visible: None,
            on_select: None,
            on_reveal: None,
        }
    }

    pub fn crumb(mut self, crumb: Crumb) -> Self {
        self.crumbs.push(crumb);
        self
    }

    pub fn crumbs(mut self, crumbs: impl IntoIterator<Item = Crumb>) -> Self {
        self.crumbs.extend(crumbs);
        self
    }

    /// How many real crumbs may be shown before the trail collapses.
    ///
    /// A collapsed trail keeps the first crumb and the most recent ones, and
    /// puts everything between them behind one ellipsis crumb. Fewer than two
    /// visible crumbs would hide the current place, so the count is clamped.
    pub fn max_visible(mut self, max_visible: usize) -> Self {
        self.max_visible = Some(max_visible.max(2));
        self
    }

    /// Reports the crumb that was picked. The last crumb never reports.
    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Reports the ids the ellipsis crumb hides, in trail order.
    ///
    /// Without this handler the ellipsis is not actionable, because there
    /// would be no way to act on it.
    pub fn on_reveal(
        mut self,
        handler: impl Fn(Vec<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reveal = Some(Rc::new(handler));
        self
    }

    /// Splits the trail into what is shown before the ellipsis, what it hides,
    /// and what is shown after it.
    fn split(&self) -> (&[Crumb], &[Crumb], &[Crumb]) {
        let count = self.crumbs.len();
        let Some(max) = self.max_visible.filter(|max| count > *max) else {
            return (&self.crumbs, &[], &[]);
        };
        let tail = max - 1;
        (
            &self.crumbs[..1],
            &self.crumbs[1..count - tail],
            &self.crumbs[count - tail..],
        )
    }
}

impl RenderOnce for Breadcrumb {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (head, hidden, tail) = self.split();
        let shown = head.len() + tail.len();
        // The trail runs from the root to here, and "from" is where reading
        // starts, so the strip follows the reading direction rather than the
        // left edge.
        let mut trail = div()
            .row_reading(cx.layout_direction())
            .flex_wrap()
            .gap(px(theme.space(Space::Xs)));
        let mut placed = 0;

        for crumb in head.iter().chain(tail.iter()) {
            if placed > 0 {
                trail = trail.child(separator(&theme));
            }
            placed += 1;
            let current = placed == shown;
            trail = trail.child(self.crumb_element(crumb, current, cx));

            if !hidden.is_empty() && placed == head.len() {
                trail = trail
                    .child(separator(&theme))
                    .child(self.ellipsis_element(hidden, cx));
            }
        }

        trail.semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::List))
    }
}

impl Breadcrumb {
    fn crumb_element(&self, crumb: &Crumb, current: bool, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.child(crumb.id.as_ref());
        let hover_group = ident.child("hover").semantic_id();
        let actionable = !current && self.on_select.is_some();

        let mut element = div()
            .id(ident.element_id())
            .group(hover_group.clone())
            .flex_none()
            // A crumb that can be gone back to wears a target: without one the
            // trail is a sentence, and nothing in it says which words answer
            // to a pointer.
            .px(px(theme.space(Space::Xs)))
            .py(px(theme.space(Space::Xs) / 2.0))
            .radius(&theme, Radius::Control)
            .when(actionable, |element| element.hover_row(&theme))
            .child(
                foundation_text(&theme, TypeScale::Label, crumb.label.clone())
                    .text_color(if current {
                        theme.colors.text
                    } else {
                        theme.colors.text_muted
                    })
                    .when(actionable, |element| {
                        element
                            .group_hover(hover_group, |style| style.text_color(theme.colors.text))
                    }),
            )
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .focus_ring(&theme)
            });

        if let (true, Some(handler)) = (actionable, self.on_select.clone()) {
            let id = crumb.id.clone();
            let clicked = id.clone();
            let click = Rc::clone(&handler);
            element = element
                .on_click(move |_, window, cx| click(clicked.clone(), window, cx))
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        handler(id.clone(), window, cx);
                        cx.stop_propagation();
                    }
                });
        }

        element.semantic_in(
            cx,
            NodeSpec::new(
                ident.semantic_id(),
                if current { Role::Text } else { Role::Link },
            )
            .parent(self.ident.semantic_id())
            .selected(current)
            .text(crumb.label.clone()),
        )
    }

    /// The one crumb without a business identity of its own, so its id is
    /// derived from the trail that owns it.
    fn ellipsis_element(&self, hidden: &[Crumb], cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.child("collapsed");
        let ids: Vec<SharedString> = hidden.iter().map(|crumb| crumb.id.clone()).collect();
        let count = ids.len();
        let digits = cx.numbers().count(count);
        let label = cx.strings().format_plural(
            StringKey::BreadcrumbHiddenOne,
            StringKey::BreadcrumbHiddenMany,
            cx.numbers().plural(count),
            &[digits.as_ref()],
        );
        let actionable = self.on_reveal.is_some();
        let hover_group = ident.child("hover").semantic_id();

        let mut element = div()
            .id(ident.element_id())
            .group(hover_group.clone())
            .flex_none()
            // Three dots set in a run of text is punctuation. The chip is what
            // makes it a control that hides a count of crumbs.
            .px(px(theme.space(Space::Xs)))
            .py(px(theme.space(Space::Xs) / 2.0))
            .radius(&theme, Radius::Control)
            .when(actionable, |element| {
                element.bg(theme.colors.hover).hover_row(&theme)
            })
            .child(
                foundation_text(&theme, TypeScale::Label, SharedString::from("…"))
                    .text_tone(&theme, gpui_kit_theme::TextTone::Muted)
                    .when(actionable, |element| {
                        element
                            .group_hover(hover_group, |style| style.text_color(theme.colors.text))
                    }),
            )
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .focus_ring(&theme)
            });

        if let Some(handler) = self.on_reveal.clone() {
            let reported = ids.clone();
            let click = Rc::clone(&handler);
            element = element
                .on_click(move |_, window, cx| click(reported.clone(), window, cx))
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        handler(ids.clone(), window, cx);
                        cx.stop_propagation();
                    }
                });
        }

        element.semantic_in(
            cx,
            NodeSpec::new(
                ident.semantic_id(),
                if actionable { Role::Button } else { Role::Text },
            )
            .parent(self.ident.semantic_id())
            .value(digits)
            .text(label),
        )
    }
}

fn separator(theme: &gpui_kit_theme::Theme) -> impl IntoElement {
    div().flex_none().child(
        foundation_text(theme, TypeScale::Label, SharedString::from("/"))
            .text_tone(theme, gpui_kit_theme::TextTone::Faint),
    )
}
