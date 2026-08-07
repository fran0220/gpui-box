//! A frame that keeps a ratio.
//!
//! # Which dimension decides
//!
//! A ratio relates two numbers, so exactly one of them has to be the one that
//! is given, and the component says which rather than guessing. [`AspectFit`]
//! is that answer:
//!
//! - [`AspectFit::Width`] takes the width from the parent and computes the
//!   height. This is the common case — a thumbnail in a column, a video in an
//!   article — and it is the default.
//! - [`AspectFit::Height`] takes the height from the parent and computes the
//!   width, which is what a strip of previews along a fixed-height row needs.
//!
//! **When the parent constrains both**, the frame still does what `fit` says:
//! it pins the one dimension `fit` names and lets the ratio decide the other,
//! even where that overflows the parent along the axis it did not take. The
//! alternative — shrinking to fit inside both — would be a *contain* box,
//! which is a different component: it leaves empty space on one axis, and
//! whoever laid the parent out is the only one who can say whether that space
//! is acceptable. A frame that silently switched between the two would hold
//! its ratio while quietly disagreeing with the size the caller asked for, so
//! this one keeps the promise it was given and lets the overflow be visible.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, prelude::*,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};

use crate::foundation::Ident;

/// Which dimension the parent decides, leaving the other to the ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AspectFit {
    #[default]
    Width,
    Height,
}

/// A container whose two dimensions stay in a fixed ratio.
#[derive(IntoElement)]
pub struct AspectRatio {
    ident: Ident,
    /// Width divided by height. `16.0 / 9.0` is wider than tall.
    ratio: f32,
    fit: AspectFit,
    child: Option<AnyElement>,
}

impl std::fmt::Debug for AspectRatio {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AspectRatio")
            .field("ident", &self.ident)
            .field("ratio", &self.ratio)
            .field("fit", &self.fit)
            .field("has_child", &self.child.is_some())
            .finish()
    }
}

impl AspectRatio {
    /// `ratio` is width divided by height.
    pub fn new(ident: impl Into<Ident>, ratio: f32) -> Self {
        Self {
            ident: ident.into(),
            ratio,
            fit: AspectFit::default(),
            child: None,
        }
    }

    /// The ratio written as the two numbers a caller already has, so
    /// `sixteen by nine` does not have to be divided at the call site.
    pub fn of(ident: impl Into<Ident>, width: f32, height: f32) -> Self {
        Self::new(ident, if height == 0.0 { 1.0 } else { width / height })
    }

    pub fn fit(mut self, fit: AspectFit) -> Self {
        self.fit = fit;
        self
    }

    /// Takes the width from the parent and computes the height.
    pub fn width_driven(self) -> Self {
        self.fit(AspectFit::Width)
    }

    /// Takes the height from the parent and computes the width.
    pub fn height_driven(self) -> Self {
        self.fit(AspectFit::Height)
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }

    pub fn ratio(&self) -> f32 {
        self.ratio
    }
}

impl RenderOnce for AspectRatio {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // A ratio of zero or less is not a ratio; a square is the one answer
        // that cannot be wrong in a direction.
        let ratio = if self.ratio.is_finite() && self.ratio > 0.0 {
            self.ratio
        } else {
            1.0
        };

        div()
            .flex_none()
            .overflow_hidden()
            .aspect_ratio(ratio)
            .map(|frame| match self.fit {
                // The dimension `fit` does not name is left auto, which is
                // what leaves it for the ratio to decide. `self_start` keeps a
                // flex parent from stretching that free dimension back out
                // and overriding the ratio with its own cross size.
                AspectFit::Width => frame.w_full().self_start(),
                AspectFit::Height => frame.h_full().self_start(),
            })
            .children(self.child)
            .semantic_in(cx, NodeSpec::new(self.ident.semantic_id(), Role::Region))
    }
}
