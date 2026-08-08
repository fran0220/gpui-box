//! The parts a modal surface and a drawer build from.
//!
//! [`Dialog`](crate::overlay::Dialog) and [`Drawer`](crate::overlay::Drawer)
//! differ in where they sit and how they arrive, not in what they are made of,
//! so the body callback and the two pieces of copy above it live here.

use std::rc::Rc;

use gpui::{AnyElement, App, ParentElement, SharedString, Styled, Window, div, px};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::Theme;

use crate::foundation::Ident;

/// Builds a surface body for one frame.
///
/// An `AnyElement` can be consumed once, while an open surface re-renders for
/// as long as it stays open, so the caller supplies a builder instead.
pub type Body = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

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
        .text_color(theme.colors.text_muted)
        .child(description.clone())
        .semantic_in(
            cx,
            NodeSpec::new(ident.child("description").semantic_id(), Role::Text)
                .parent(ident.semantic_id())
                .text(description),
        )
}
