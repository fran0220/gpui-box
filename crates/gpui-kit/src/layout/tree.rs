//! A tree of splits, and the frame that draws one.
//!
//! [`SplitPane`] is two panes and a divider. [`SplitTree`] is however many of
//! those the caller nests, arranged as a [`SplitLayout`] the caller owns. The
//! tree is data, not view state: the library draws whatever tree it is handed
//! and reports every change the typist asked for as a [`SplitChange`], so a
//! host that refuses a resize keeps the arrangement that still holds.
//!
//! # Persisting a layout
//!
//! This crate takes no serialization dependency, so [`SplitLayout`] carries no
//! derived `Serialize`. Instead it converts losslessly to and from a flat
//! [`Vec<SplitRecord>`] of plain fields, which a host serializes with whatever
//! format it already uses:
//!
//! ```
//! # use gpui_kit::layout::{SplitLayout, SplitPaneSpec};
//! let layout = SplitLayout::horizontal(
//!     "workspace",
//!     0.3,
//!     SplitLayout::leaf(SplitPaneSpec::new("files").min(180.0)),
//!     SplitLayout::pane("editor"),
//! );
//! let records = layout.to_records();
//! // ... the host writes `records` out field by field, and reads them back ...
//! assert_eq!(
//!     SplitLayout::from_records(&records).expect("records were produced by this layout"),
//!     layout
//! );
//! ```
//!
//! # Minimums propagate
//!
//! A leaf states the smallest it may be drawn at. A branch's minimum along its
//! own axis is the sum of its children's, plus the divider between them, and
//! across the other axis it is the larger of the two. A divider high in the
//! tree therefore stops where a leaf far below it would run out of room,
//! rather than reporting a ratio that starves it.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};

use crate::foundation::{Disableable, Ident};
use crate::layout::split::{HANDLE, SplitAxis, SplitPane, SplitSide};

type ChangeHandler = Rc<dyn Fn(SplitChange, &mut Window, &mut App)>;

/// One leaf of a [`SplitLayout`]: a named place the caller puts content.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitPaneSpec {
    id: SharedString,
    min_width: f32,
    min_height: f32,
    rail: f32,
    collapsed: bool,
}

impl SplitPaneSpec {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            min_width: 0.0,
            min_height: 0.0,
            rail: 0.0,
            collapsed: false,
        }
    }

    /// The smallest this pane may be drawn at on either axis, in pixels.
    ///
    /// A pane that only cares about one axis states that axis instead: a file
    /// tree that needs 180px of width does not thereby need 180px of height,
    /// and saying so would stop a divider it has nothing to do with.
    pub fn min(self, min: f32) -> Self {
        self.min_width(min).min_height(min)
    }

    pub fn min_width(mut self, min: f32) -> Self {
        self.min_width = min.max(0.0);
        self
    }

    pub fn min_height(mut self, min: f32) -> Self {
        self.min_height = min.max(0.0);
        self
    }

    /// How wide the pane is while collapsed. A rail of zero removes the pane
    /// from the drawing entirely.
    pub fn rail(mut self, rail: f32) -> Self {
        self.rail = rail.max(0.0);
        self
    }

    /// Whether the caller says this pane is collapsed. A collapsed pane is
    /// drawn at its rail extent and its divider is not offered, because there
    /// is nothing to drag it between.
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// The smallest this pane may be drawn at along `axis`, in pixels.
    pub fn min_size(&self, axis: SplitAxis) -> f32 {
        match axis {
            SplitAxis::Horizontal => self.min_width,
            SplitAxis::Vertical => self.min_height,
        }
    }

    pub fn rail_size(&self) -> f32 {
        self.rail
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }
}

/// A tree of splits the caller owns.
#[derive(Debug, Clone, PartialEq)]
pub enum SplitLayout {
    Pane(SplitPaneSpec),
    Branch {
        id: SharedString,
        axis: SplitAxis,
        /// How much of the branch the first child takes, from 0 to 1.
        ratio: f32,
        start: Box<SplitLayout>,
        end: Box<SplitLayout>,
    },
}

impl SplitLayout {
    pub fn pane(id: impl Into<SharedString>) -> Self {
        Self::Pane(SplitPaneSpec::new(id))
    }

    pub fn leaf(spec: SplitPaneSpec) -> Self {
        Self::Pane(spec)
    }

    pub fn split(
        id: impl Into<SharedString>,
        axis: SplitAxis,
        ratio: f32,
        start: SplitLayout,
        end: SplitLayout,
    ) -> Self {
        Self::Branch {
            id: id.into(),
            axis,
            ratio: ratio.clamp(0.0, 1.0),
            start: Box::new(start),
            end: Box::new(end),
        }
    }

    pub fn horizontal(
        id: impl Into<SharedString>,
        ratio: f32,
        start: SplitLayout,
        end: SplitLayout,
    ) -> Self {
        Self::split(id, SplitAxis::Horizontal, ratio, start, end)
    }

    pub fn vertical(
        id: impl Into<SharedString>,
        ratio: f32,
        start: SplitLayout,
        end: SplitLayout,
    ) -> Self {
        Self::split(id, SplitAxis::Vertical, ratio, start, end)
    }

    pub fn id(&self) -> &SharedString {
        match self {
            Self::Pane(spec) => &spec.id,
            Self::Branch { id, .. } => id,
        }
    }

    pub fn is_pane(&self) -> bool {
        matches!(self, Self::Pane(_))
    }

    /// Every leaf, in the order it is drawn.
    pub fn panes(&self) -> Vec<&SplitPaneSpec> {
        let mut found = Vec::new();
        self.walk(&mut |node| {
            if let Self::Pane(spec) = node {
                found.push(spec);
            }
        });
        found
    }

    /// The subtree named `id`, wherever it sits.
    pub fn find(&self, id: &str) -> Option<&SplitLayout> {
        if self.id().as_ref() == id {
            return Some(self);
        }
        match self {
            Self::Pane(_) => None,
            Self::Branch { start, end, .. } => start.find(id).or_else(|| end.find(id)),
        }
    }

    fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a SplitLayout)) {
        visit(self);
        if let Self::Branch { start, end, .. } = self {
            start.walk(visit);
            end.walk(visit);
        }
    }

    /// The extent this subtree is collapsed to, when it is a collapsed leaf.
    fn rail(&self) -> Option<f32> {
        match self {
            Self::Pane(spec) if spec.collapsed => Some(spec.rail),
            _ => None,
        }
    }

    /// Whether a branch offers a divider at all.
    ///
    /// A branch with a collapsed side has nothing to trade: the rail is a
    /// fixed extent, so there is no ratio to move.
    fn divides(&self) -> bool {
        match self {
            Self::Pane(_) => false,
            Self::Branch { start, end, .. } => start.rail().is_none() && end.rail().is_none(),
        }
    }

    /// The smallest this subtree may be drawn at along `axis`, in pixels.
    pub fn min_extent(&self, axis: SplitAxis) -> f32 {
        match self {
            Self::Pane(spec) => {
                if spec.collapsed {
                    spec.rail
                } else {
                    spec.min_size(axis)
                }
            }
            Self::Branch {
                axis: branch_axis,
                start,
                end,
                ..
            } => {
                let (first, second) = (start.min_extent(axis), end.min_extent(axis));
                if *branch_axis == axis {
                    first + second + if self.divides() { HANDLE } else { 0.0 }
                } else {
                    first.max(second)
                }
            }
        }
    }

    /// The same tree with one branch's ratio replaced.
    pub fn with_ratio(&self, split: &str, ratio: f32) -> Self {
        self.mapped(&mut |node| match node {
            Self::Branch {
                id,
                axis,
                start,
                end,
                ..
            } if id.as_ref() == split => Self::Branch {
                id: id.clone(),
                axis: *axis,
                ratio: ratio.clamp(0.0, 1.0),
                start: start.clone(),
                end: end.clone(),
            },
            other => other.clone(),
        })
    }

    /// The same tree with one leaf's collapsed flag replaced.
    pub fn with_collapsed(&self, pane: &str, collapsed: bool) -> Self {
        self.mapped(&mut |node| match node {
            Self::Pane(spec) if spec.id.as_ref() == pane => {
                Self::Pane(spec.clone().collapsed(collapsed))
            }
            other => other.clone(),
        })
    }

    /// The same tree with a reported change applied.
    ///
    /// This is offered so a host that simply accepts every change has one call
    /// to make. A host that judges them applies the ones it accepts itself.
    pub fn applied(&self, change: &SplitChange) -> Self {
        match change {
            SplitChange::Ratio { split, ratio } => self.with_ratio(split, *ratio),
            SplitChange::Collapsed { pane, .. } => self.with_collapsed(pane, true),
        }
    }

    fn mapped(&self, map: &mut impl FnMut(&SplitLayout) -> SplitLayout) -> Self {
        let replaced = map(self);
        match replaced {
            Self::Pane(spec) => Self::Pane(spec),
            Self::Branch {
                id,
                axis,
                ratio,
                start,
                end,
            } => Self::Branch {
                id,
                axis,
                ratio,
                start: Box::new(start.mapped(map)),
                end: Box::new(end.mapped(map)),
            },
        }
    }

    /// The tree flattened into plain records, parents before children and the
    /// first child before the second.
    pub fn to_records(&self) -> Vec<SplitRecord> {
        let mut records = Vec::new();
        self.record_into(None, &mut records);
        records
    }

    fn record_into(&self, parent: Option<SharedString>, records: &mut Vec<SplitRecord>) {
        match self {
            Self::Pane(spec) => records.push(SplitRecord {
                id: spec.id.clone(),
                parent,
                kind: SplitKind::Pane,
                ratio: 0.0,
                min_width: spec.min_width,
                min_height: spec.min_height,
                rail: spec.rail,
                collapsed: spec.collapsed,
            }),
            Self::Branch {
                id,
                axis,
                ratio,
                start,
                end,
            } => {
                records.push(SplitRecord {
                    id: id.clone(),
                    parent,
                    kind: match axis {
                        SplitAxis::Horizontal => SplitKind::Horizontal,
                        SplitAxis::Vertical => SplitKind::Vertical,
                    },
                    ratio: *ratio,
                    min_width: 0.0,
                    min_height: 0.0,
                    rail: 0.0,
                    collapsed: false,
                });
                start.record_into(Some(id.clone()), records);
                end.record_into(Some(id.clone()), records);
            }
        }
    }

    /// Rebuilds a tree the host wrote out with [`SplitLayout::to_records`].
    pub fn from_records(records: &[SplitRecord]) -> Result<Self, SplitRecordError> {
        let mut children: HashMap<&str, Vec<&SplitRecord>> = HashMap::new();
        let mut by_id: HashMap<&str, &SplitRecord> = HashMap::new();
        let mut roots: Vec<&SplitRecord> = Vec::new();

        for record in records {
            if by_id.insert(record.id.as_ref(), record).is_some() {
                return Err(SplitRecordError::DuplicateId(record.id.clone()));
            }
            match &record.parent {
                Some(parent) => children.entry(parent.as_ref()).or_default().push(record),
                None => roots.push(record),
            }
        }

        for record in records {
            if let Some(parent) = &record.parent
                && !by_id.contains_key(parent.as_ref())
            {
                return Err(SplitRecordError::MissingParent {
                    id: record.id.clone(),
                    parent: parent.clone(),
                });
            }
        }

        let root = match roots.as_slice() {
            [] => return Err(SplitRecordError::NoRoot),
            [root] => *root,
            _ => {
                return Err(SplitRecordError::ManyRoots(
                    roots.iter().map(|record| record.id.clone()).collect(),
                ));
            }
        };

        // A cycle leaves its members out of the root's reachable set, so
        // counting what was built is enough to find one without walking twice.
        let mut built = 0;
        let layout = build(root, &children, &mut built)?;
        if built != records.len() {
            return Err(SplitRecordError::Unreachable);
        }
        Ok(layout)
    }
}

fn build(
    record: &SplitRecord,
    children: &HashMap<&str, Vec<&SplitRecord>>,
    built: &mut usize,
) -> Result<SplitLayout, SplitRecordError> {
    *built += 1;
    let own = children
        .get(record.id.as_ref())
        .map(Vec::as_slice)
        .unwrap_or_default();
    match record.kind {
        SplitKind::Pane => {
            if !own.is_empty() {
                return Err(SplitRecordError::PaneWithChildren(record.id.clone()));
            }
            Ok(SplitLayout::Pane(
                SplitPaneSpec::new(record.id.clone())
                    .min_width(record.min_width)
                    .min_height(record.min_height)
                    .rail(record.rail)
                    .collapsed(record.collapsed),
            ))
        }
        SplitKind::Horizontal | SplitKind::Vertical => {
            let [start, end] = own else {
                return Err(SplitRecordError::WrongChildCount {
                    id: record.id.clone(),
                    found: own.len(),
                });
            };
            Ok(SplitLayout::split(
                record.id.clone(),
                match record.kind {
                    SplitKind::Horizontal => SplitAxis::Horizontal,
                    _ => SplitAxis::Vertical,
                },
                record.ratio,
                build(start, children, built)?,
                build(end, children, built)?,
            ))
        }
    }
}

/// What one [`SplitRecord`] describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitKind {
    Pane,
    Horizontal,
    Vertical,
}

impl SplitKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pane => "pane",
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// One node of a [`SplitLayout`], as plain fields a host can write anywhere.
///
/// `ratio` is meaningful for a branch, and `min_width`, `min_height`, `rail`,
/// and `collapsed` for a pane; the other fields are zero and ignored, so a host
/// that stores every field back gets the same tree.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitRecord {
    pub id: SharedString,
    pub parent: Option<SharedString>,
    pub kind: SplitKind,
    pub ratio: f32,
    pub min_width: f32,
    pub min_height: f32,
    pub rail: f32,
    pub collapsed: bool,
}

/// Why a set of records is not a tree.
#[derive(Debug, Clone, PartialEq)]
pub enum SplitRecordError {
    NoRoot,
    ManyRoots(Vec<SharedString>),
    DuplicateId(SharedString),
    MissingParent {
        id: SharedString,
        parent: SharedString,
    },
    /// A split needs exactly two children.
    WrongChildCount {
        id: SharedString,
        found: usize,
    },
    PaneWithChildren(SharedString),
    /// Records that no path from the root reaches, which means a cycle.
    Unreachable,
}

impl std::fmt::Display for SplitRecordError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRoot => write!(formatter, "no record without a parent"),
            Self::ManyRoots(ids) => {
                let names: Vec<&str> = ids.iter().map(SharedString::as_ref).collect();
                write!(formatter, "more than one root: {}", names.join(", "))
            }
            Self::DuplicateId(id) => write!(formatter, "`{id}` appears more than once"),
            Self::MissingParent { id, parent } => {
                write!(formatter, "`{id}` names a parent `{parent}` that is absent")
            }
            Self::WrongChildCount { id, found } => {
                write!(formatter, "split `{id}` has {found} children, not 2")
            }
            Self::PaneWithChildren(id) => write!(formatter, "pane `{id}` has children"),
            Self::Unreachable => write!(formatter, "records the root does not reach"),
        }
    }
}

impl std::error::Error for SplitRecordError {}

/// A change the typist asked the layout for. Nothing has been applied.
#[derive(Debug, Clone, PartialEq)]
pub enum SplitChange {
    Ratio {
        split: SharedString,
        ratio: f32,
    },
    Collapsed {
        split: SharedString,
        side: SplitSide,
        pane: SharedString,
    },
}

/// Draws a [`SplitLayout`] and reports what the typist asked to change.
#[derive(IntoElement)]
pub struct SplitTree {
    ident: Ident,
    layout: SplitLayout,
    panes: HashMap<SharedString, AnyElement>,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl std::fmt::Debug for SplitTree {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SplitTree")
            .field("ident", &self.ident)
            .field("panes", &self.panes.len())
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_change.is_some())
            .finish()
    }
}

impl SplitTree {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            layout: SplitLayout::pane("pane"),
            panes: HashMap::new(),
            disabled: false,
            on_change: None,
        }
    }

    pub fn layout(mut self, layout: SplitLayout) -> Self {
        self.layout = layout;
        self
    }

    /// What goes in the leaf named `id`. A leaf nothing is given draws empty.
    pub fn pane(mut self, id: impl Into<SharedString>, content: impl IntoElement) -> Self {
        self.panes.insert(id.into(), content.into_any_element());
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(SplitChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    fn node(
        &self,
        layout: &SplitLayout,
        panes: &mut HashMap<SharedString, AnyElement>,
        cx: &mut App,
    ) -> AnyElement {
        match layout {
            SplitLayout::Pane(spec) => {
                let ident = self.ident.child(spec.id.as_ref());
                div()
                    .flex()
                    .flex_col()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .size_full()
                    .overflow_hidden()
                    .children(panes.remove(&spec.id))
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.semantic_id(), Role::Group)
                            .parent(self.ident.semantic_id())
                            .expanded(!spec.collapsed),
                    )
                    .into_any_element()
            }
            SplitLayout::Branch {
                id,
                axis,
                ratio,
                start,
                end,
            } => {
                let horizontal = *axis == SplitAxis::Horizontal;
                let first = self.node(start, panes, cx);
                let second = self.node(end, panes, cx);

                // A branch with a collapsed side has no ratio to move, so it
                // is a fixed rail beside a pane rather than a split.
                if let Some(rail) = start.rail().or_else(|| end.rail()) {
                    let start_rail = start.rail().is_some();
                    let fixed = |element: AnyElement| {
                        div()
                            .flex_none()
                            .when(horizontal, |frame| frame.w(px(rail)).h_full())
                            .when(!horizontal, |frame| frame.h(px(rail)).w_full())
                            .overflow_hidden()
                            .child(element)
                    };
                    let flexible = |element: AnyElement| {
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .min_h(px(0.0))
                            .overflow_hidden()
                            .child(element)
                    };
                    let (first, second) = if start_rail {
                        (fixed(first), flexible(second))
                    } else {
                        (flexible(first), fixed(second))
                    };
                    return div()
                        .flex()
                        .when(horizontal, |frame| frame.flex_row())
                        .when(!horizontal, |frame| frame.flex_col())
                        .items_stretch()
                        .size_full()
                        .overflow_hidden()
                        .child(first)
                        .child(second)
                        .into_any_element();
                }

                let split_id = id.clone();
                let collapsible = start.is_pane() || end.is_pane();
                let start_pane = start.id().clone();
                let end_pane = end.id().clone();
                let start_is_pane = start.is_pane();
                let end_is_pane = end.is_pane();

                let mut split = SplitPane::new(self.ident.child(id.as_ref()))
                    .axis(*axis)
                    .ratio(*ratio)
                    .min_sizes(start.min_extent(*axis), end.min_extent(*axis))
                    .collapsible(collapsible)
                    .disabled(self.disabled)
                    .start(first)
                    .end(second);

                if let Some(handler) = self.on_change.clone() {
                    let resized = split_id.clone();
                    let reported = Rc::clone(&handler);
                    split = split.on_resize(move |ratio, window, cx| {
                        reported(
                            SplitChange::Ratio {
                                split: resized.clone(),
                                ratio,
                            },
                            window,
                            cx,
                        );
                    });
                    split = split.on_collapse(move |side, window, cx| {
                        // Only a leaf can be collapsed; a whole subtree has no
                        // single identity the host could hide.
                        let pane = match side {
                            SplitSide::Start if start_is_pane => start_pane.clone(),
                            SplitSide::End if end_is_pane => end_pane.clone(),
                            _ => return,
                        };
                        handler(
                            SplitChange::Collapsed {
                                split: split_id.clone(),
                                side,
                                pane,
                            },
                            window,
                            cx,
                        );
                    });
                }

                split.into_any_element()
            }
        }
    }
}

impl Disableable for SplitTree {
    /// Freezes every divider in the tree. A frozen tree installs no handler.
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for SplitTree {
    fn render(mut self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let layout = self.layout.clone();
        let mut panes = std::mem::take(&mut self.panes);
        let body = self.node(&layout, &mut panes, cx);
        div()
            .id(self.ident.element_id())
            .size_full()
            .overflow_hidden()
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .value(layout.panes().len().to_string()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> SplitLayout {
        SplitLayout::vertical(
            "root",
            0.75,
            SplitLayout::horizontal(
                "body",
                0.25,
                SplitLayout::leaf(SplitPaneSpec::new("files").min_width(180.0).rail(40.0)),
                SplitLayout::leaf(SplitPaneSpec::new("editor").min_width(320.0)),
            ),
            SplitLayout::leaf(SplitPaneSpec::new("terminal").min_height(120.0)),
        )
    }

    #[test]
    fn a_tree_round_trips_through_plain_records() {
        let layout = workspace();
        let records = layout.to_records();
        assert_eq!(records.len(), 5);
        assert_eq!(records[0].parent, None);
        assert_eq!(SplitLayout::from_records(&records), Ok(layout.clone()));
        assert_eq!(
            SplitLayout::from_records(&records)
                .expect("the records are a tree")
                .to_records(),
            records,
            "the conversion loses nothing in either direction"
        );
    }

    #[test]
    fn records_that_are_not_a_tree_say_why() {
        assert_eq!(
            SplitLayout::from_records(&[]),
            Err(SplitRecordError::NoRoot)
        );

        let mut records = workspace().to_records();
        records.retain(|record| record.id.as_ref() != "terminal");
        assert_eq!(
            SplitLayout::from_records(&records),
            Err(SplitRecordError::WrongChildCount {
                id: "root".into(),
                found: 1
            })
        );

        let mut orphan = workspace().to_records();
        orphan[4].parent = Some("nowhere".into());
        assert!(matches!(
            SplitLayout::from_records(&orphan),
            Err(SplitRecordError::MissingParent { .. })
        ));
    }

    #[test]
    fn a_minimum_is_the_sum_along_the_axis_and_the_larger_across_it() {
        let layout = workspace();
        let body = layout.find("body").expect("body is in the tree");
        // Side by side, the two widths add, and so does the divider.
        assert_eq!(
            body.min_extent(SplitAxis::Horizontal),
            180.0 + 320.0 + HANDLE
        );
        // Across the split, the taller of the two children decides, and
        // neither of them states a height.
        assert_eq!(body.min_extent(SplitAxis::Vertical), 0.0);
        // The root stacks the body over the terminal, so their heights add.
        assert_eq!(layout.min_extent(SplitAxis::Vertical), 120.0 + HANDLE);
        assert_eq!(
            layout.min_extent(SplitAxis::Horizontal),
            180.0 + 320.0 + HANDLE,
            "the widest row decides the whole tree's width"
        );
    }

    #[test]
    fn a_collapsed_leaf_is_worth_its_rail_and_removes_the_divider() {
        let collapsed = workspace().with_collapsed("files", true);
        let body = collapsed.find("body").expect("body is in the tree");
        assert_eq!(body.min_extent(SplitAxis::Horizontal), 40.0 + 320.0);
    }

    #[test]
    fn applying_a_reported_change_produces_the_tree_the_host_would_store() {
        let layout = workspace();
        let moved = layout.applied(&SplitChange::Ratio {
            split: "body".into(),
            ratio: 0.4,
        });
        let SplitLayout::Branch { ratio, .. } = moved.find("body").expect("body is in the tree")
        else {
            panic!("body is a split");
        };
        assert_eq!(*ratio, 0.4);
        assert_ne!(moved, layout, "the caller's tree is untouched");

        let hidden = layout.applied(&SplitChange::Collapsed {
            split: "body".into(),
            side: SplitSide::Start,
            pane: "files".into(),
        });
        assert!(hidden.panes()[0].is_collapsed());
    }

    #[test]
    fn every_leaf_is_reachable_by_name() {
        let layout = workspace();
        let names: Vec<&str> = layout
            .panes()
            .iter()
            .map(|spec| spec.id().as_ref())
            .collect();
        assert_eq!(names, vec!["files", "editor", "terminal"]);
        assert!(layout.find("editor").is_some());
        assert!(layout.find("nothing").is_none());
    }
}
