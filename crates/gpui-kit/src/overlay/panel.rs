//! The parts a modal surface and a drawer build from.
//!
//! [`Dialog`](crate::overlay::Dialog) and [`Drawer`](crate::overlay::Drawer)
//! differ in where they sit and how they arrive, not in what they are made of,
//! so the body callback and the two pieces of copy above it live here.

use std::rc::Rc;

use gpui::{AnyElement, App, Div, ParentElement, SharedString, Styled, Window, div, px};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{Space, Theme};

use crate::foundation::{Ident, StyledExt, rule};

/// Builds a surface body for one frame.
///
/// An `AnyElement` can be consumed once, while an open surface re-renders for
/// as long as it stays open, so the caller supplies a builder instead.
pub type Body = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// The band a modal surface's title, body, or actions sit in.
///
/// One inset for all three, so the title, the body and the buttons of a
/// dialog and of a drawer line up on the same left edge and the rules between
/// them run the full width of the surface rather than being inset by whatever
/// each section chose.
pub fn band(theme: &Theme) -> Div {
    div()
        .column()
        .px_token(theme, Space::Lg)
        .py_token(theme, Space::Md)
        .gap_token(theme, Space::Xs)
}

/// The full-bleed line between two bands of a modal surface.
///
/// A dialog and a drawer separate their header, their body and their actions
/// the same way, which is what stops a footer's buttons from reading as
/// floating over the content above them.
pub fn seam(theme: &Theme) -> Div {
    div().flex_none().child(rule(theme))
}

/// The title of a surface, published as its first-level heading.
pub fn heading(
    ident: &Ident,
    theme: &Theme,
    title: SharedString,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    div()
        .text_size(px(theme.typography.title.size))
        .line_height(px(theme.typography.title.line_height))
        .font_weight(gpui::FontWeight(theme.typography.title.weight))
        .font_fallbacks(gpui_kit_assets::text_fallbacks())
        .child(title.clone())
        .semantic_in(
            cx,
            NodeSpec::new(ident.child("title").semantic_id(), Role::Heading)
                .parent(ident.semantic_id())
                .level(1)
                .text(title),
        )
}

/// Secondary copy under the title.
pub fn description(
    ident: &Ident,
    theme: &Theme,
    description: SharedString,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    div()
        .text_size(px(theme.typography.body.size))
        .line_height(px(theme.typography.body.line_height))
        .font_fallbacks(gpui_kit_assets::text_fallbacks())
        .text_color(theme.colors.text_muted)
        .child(description.clone())
        .semantic_in(
            cx,
            NodeSpec::new(ident.child("description").semantic_id(), Role::Text)
                .parent(ident.semantic_id())
                .text(description),
        )
}
