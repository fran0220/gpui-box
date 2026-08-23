//! A virtualized surface whose rows are exactly what the caller drew.
//!
//! [`List`](crate::data::List) answers "show me these rows" and decides what a
//! row looks like: it pads it, gives it a hover and a selected state, and
//! publishes a row node for it. A conversation, a log and a diff answer a
//! different question. Their rows are not entries in a list — they are turns,
//! lines and hunks, each with its own shape, its own spacing rule and its own
//! identity — and a container that wraps them in list chrome is describing
//! something they are not.
//!
//! What those surfaces still need is everything virtualization is: only the
//! screenful is laid out, a row is measured once and that measurement survives
//! the next frame, and the surface can be scrolled, revealed, glided and
//! followed by name. That is this. A `Flow` draws the element the caller
//! returns and nothing else around it, and registers the measured state under
//! the caller's identity, so [`follow_end`](crate::motion::follow_end),
//! [`scroll_to_row`](crate::data::scroll_to_row),
//! [`reveal_row`](crate::data::reveal_row) and
//! [`glide_to_row`](crate::data::glide_to_row) all reach it the same way they
//! reach a list.
//!
//! `List`'s own variable-height mode is a `Flow` whose rows happen to be list
//! rows, which is the honest relationship between the two: one is a surface,
//! the other is what a surface is made of.
//!
//! # A flow publishes nothing
//!
//! Every other surface here publishes a container node carrying its total and
//! a node per row. A flow cannot: it did not build the rows and has no idea
//! what they are, and a node it invented would name a row that the caller
//! already named something else. So the caller publishes both, exactly as it
//! would if it were laying the rows out itself — which is the only version
//! where the ids in a snapshot are the ids the product uses.
//!
//! # Name the rows
//!
//! A flow only learns a row's height by laying it out, and what it has learned
//! is addressed by index. [`Flow::keys`] says which indices still mean what
//! they meant, so a turn arriving at the end re-measures that turn instead of
//! discarding every height above it. A flow that only counts its rows keeps
//! the blunter rule and loses its measurements whenever the count changes in
//! any way but growing.

use std::rc::Rc;

use gpui::{
    AnyElement, App, IntoElement, ListAlignment, ListSizingBehavior, RenderOnce, SharedString,
    Styled, Window, list, prelude::FluentBuilder, px,
};
use gpui_kit_theme::{ActiveTheme, ControlSize};

use crate::data::viewport::{Rows, list_state};
use crate::foundation::Ident;

type RenderRow = Rc<dyn Fn(usize, &mut Window, &mut App) -> AnyElement>;

/// How much room the flow takes in whatever holds it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Extent {
    /// As tall as its rows, which lays every one of them out. Right for a
    /// short set; wrong for anything virtualization was wanted for.
    Content,
    /// Exactly this many rows tall, at the row estimate.
    Rows(usize),
    /// Whatever its parent gives it, which is what a surface filling a pane
    /// wants.
    Fill,
}

/// Rows drawn by their caller, laid out only where the viewport reaches.
#[derive(IntoElement)]
pub struct Flow {
    ident: Ident,
    count: usize,
    render_row: RenderRow,
    keys: Option<Vec<SharedString>>,
    estimate: Option<f32>,
    alignment: ListAlignment,
    extent: Extent,
}

impl std::fmt::Debug for Flow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Flow")
            .field("ident", &self.ident)
            .field("count", &self.count)
            .field("named", &self.keys.is_some())
            .field("extent", &self.extent)
            .finish()
    }
}

impl Flow {
    /// A flow of `count` rows, each built on demand by `render_row`.
    ///
    /// The closure is called for the rows the viewport reaches, in any order,
    /// and may be called again for a row that scrolled away and came back — so
    /// it has to be able to build a row rather than hand over one it built
    /// earlier.
    pub fn new(
        ident: impl Into<Ident>,
        count: usize,
        render_row: impl Fn(usize, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self {
            ident: ident.into(),
            count,
            render_row: Rc::new(render_row),
            keys: None,
            estimate: None,
            alignment: ListAlignment::Top,
            extent: Extent::Content,
        }
    }

    /// Names the rows, in the order they are drawn, so the measurements either
    /// side of a change survive it.
    ///
    /// The names are the rows' own identities. A name that covers a row's
    /// *content* as well as its identity — a revision, a content hash — is
    /// what a surface whose rows are rewritten in place wants, because a row
    /// that kept its name keeps its measured height, and a row whose bytes
    /// changed is a row whose height is no longer known.
    pub fn keys(mut self, keys: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.keys = Some(keys.into_iter().map(Into::into).collect());
        self
    }

    /// What a row nobody has laid out yet is assumed to be worth.
    ///
    /// It decides how nearly right the scrollbar is on the first frame and how
    /// far a scroll into unmeasured territory lands; it never constrains a row,
    /// which is as tall as what it holds.
    pub fn estimate(mut self, height: f32) -> Self {
        self.estimate = Some(height);
        self
    }

    /// Rests the rows against the end rather than the start, for a
    /// conversation or a log whose newest row is the one worth seeing.
    pub fn anchored_to_end(mut self) -> Self {
        self.alignment = ListAlignment::Bottom;
        self
    }

    /// Bounds the viewport to `rows` rows at the row estimate.
    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.extent = Extent::Rows(rows);
        self
    }

    /// Takes the height of whatever holds it, which is what a surface filling
    /// a pane does — and the only way a flow in a pane is bounded at all,
    /// since its rows are not a number of anything.
    pub fn fills(mut self) -> Self {
        self.extent = Extent::Fill;
        self
    }
}

impl RenderOnce for Flow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let estimate = self
            .estimate
            .unwrap_or_else(|| theme.control.get(ControlSize::Md).height);
        let described = match &self.keys {
            Some(keys) if keys.len() == self.count => Rows::Keyed(keys),
            // A caller whose names do not cover the rows it says it has has
            // described something other than this flow, so the count is
            // believed and the names are not.
            _ => Rows::Counted(self.count),
        };
        let state = list_state(
            &self.ident,
            described,
            self.alignment,
            px(estimate),
            window,
            cx,
        );
        let render_row = Rc::clone(&self.render_row);
        let extent = self.extent;

        list(state, move |index, window, cx| {
            render_row(index, window, cx)
        })
        .with_sizing_behavior(match extent {
            // Sizing itself to its content is what lays every row out, so it
            // is also what a bounded flow must not do.
            Extent::Content => ListSizingBehavior::Infer,
            Extent::Rows(_) | Extent::Fill => ListSizingBehavior::Auto,
        })
        .w_full()
        .when(extent == Extent::Fill, |element| element.h_full())
        .when_some(
            match extent {
                Extent::Rows(rows) => Some(rows),
                _ => None,
            },
            |element, rows| element.h(px(estimate * rows as f32)),
        )
    }
}
