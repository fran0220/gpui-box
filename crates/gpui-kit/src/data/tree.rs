//! A hierarchy whose open branches are caller-owned.
//!
//! The tree reports the node that was activated and the state its disclosure
//! should take next. It renders exactly the set the caller passed to
//! [`Tree::expanded`], so a host that refuses to open a branch leaves it shut.
//!
//! A collapsed node renders none of its children, and publishes none of them
//! either, so asserting that a child is absent means something.
//!
//! # What a large hierarchy costs
//!
//! What is on screen depends on what is open, so the tree first flattens the
//! hierarchy to the rows a reader could see and then draws from that. With
//! [`Tree::visible_rows`] it draws only the ones that fit, so a hierarchy with
//! ten thousand disclosed rows lays out a viewport's worth. Without it the
//! tree sizes itself to its content and every disclosed row is laid out.
//!
//! Flattening walks the hierarchy when the nodes or the open set change, and
//! reuses the last walk otherwise. That is still data rather than elements:
//! a [`TreeNode`] holds two strings, an element holds a layout.
//!
//! The tree's semantic node carries the number of disclosed rows in `value`,
//! which is what keeps three different absences apart: a node under a shut
//! branch is not disclosed, a disclosed node outside the viewport is counted
//! but not published, and a node that is not in the data at all is neither.
//!
//! A bounded tree can draw a node whose parent has scrolled off the top. The
//! node still reports the parent it has, because that is what is true of it,
//! so a walk down from the tree's own node will not reach it and a test that
//! wants it should name it. Its `level` says how deep it sits either way.

use std::f32::consts::FRAC_PI_2;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ListSizingBehavior, ParentElement,
    RenderOnce, ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, Transformation,
    Window, div, point, prelude::FluentBuilder, px, radians, uniform_list,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Theme, TypeScale};

use crate::data::viewport::scroll_handle;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::icon::flips;
use crate::display::loading::PulseLoader;
use crate::foundation::direction::{ActiveDirection, DirectionalExt, LayoutDirection};
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{
    Disableable, FocusRing, Hoverable, Ident, Pressable, SelectedFill, Sizable, StyledExt, text,
};
use crate::interaction::dnd::{
    self, DragItem, DropAxis, DropIntent, DropPosition, MakingWay, RowTarget, SurfaceDrag,
};
use crate::motion::{self, keyed};
use crate::overlay::Tooltipped;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type ToggleHandler = Rc<dyn Fn(SharedString, bool, &mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type MoveHandler = Rc<dyn Fn(&DropIntent, &mut Window, &mut App)>;
type Accepts = Rc<dyn Fn(&DragItem, &DropPosition) -> bool>;

/// One node, identified by business identity rather than by its place in the
/// hierarchy, so moving a branch does not rename what hangs under it.
#[derive(Debug, Clone)]
pub struct TreeNode {
    id: SharedString,
    label: SharedString,
    icon: Option<Icon>,
    disabled: bool,
    children: Vec<TreeNode>,
    branch: BranchState,
}

impl TreeNode {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            disabled: false,
            children: Vec::new(),
            branch: BranchState::Ready,
        }
    }

    pub fn child(mut self, child: TreeNode) -> Self {
        self.children.push(child);
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = TreeNode>) -> Self {
        self.children.extend(children);
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// What is known about this node's children.
    ///
    /// [`BranchState::Ready`] with no children is a leaf. Loading, unavailable,
    /// or failed still offers a disclosure, because the host has said there is
    /// a branch even when it cannot list it yet.
    pub fn branch(mut self, branch: BranchState) -> Self {
        self.branch = branch;
        self
    }
}

/// What is known about the children under a node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum BranchState {
    /// The children that were handed over are the complete set.
    #[default]
    Ready,
    /// The host is still listing this branch.
    Loading,
    /// The host could not list this branch, in its own words.
    Unavailable(SharedString),
    /// An attempt to list this branch failed, in its own words.
    Failed(SharedString),
}

impl BranchState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Unavailable(_) => "unavailable",
            Self::Failed(_) => "failed",
        }
    }
}

/// A disclosure hierarchy.
#[derive(IntoElement)]
pub struct Tree {
    ident: Ident,
    nodes: Vec<TreeNode>,
    expanded: Vec<SharedString>,
    selected: Option<SharedString>,
    visible_rows: Option<usize>,
    size: ControlSize,
    disabled: bool,
    loading: bool,
    failure: Option<SharedString>,
    empty: Option<EmptyState>,
    slots: Slots,
    on_toggle: Option<ToggleHandler>,
    on_select: Option<SelectHandler>,
    reorderable: bool,
    accepts: Option<Accepts>,
    on_move: Option<MoveHandler>,
}

impl std::fmt::Debug for Tree {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Tree")
            .field("ident", &self.ident)
            .field("nodes", &self.nodes.len())
            .field("expanded", &self.expanded)
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl Tree {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            nodes: Vec::new(),
            expanded: Vec::new(),
            selected: None,
            visible_rows: None,
            size: ControlSize::Md,
            disabled: false,
            loading: false,
            failure: None,
            empty: None,
            slots: Slots::default(),
            on_toggle: None,
            on_select: None,
            reorderable: false,
            accepts: None,
            on_move: None,
        }
    }

    pub fn node(mut self, node: TreeNode) -> Self {
        self.nodes.push(node);
        self
    }

    pub fn nodes(mut self, nodes: impl IntoIterator<Item = TreeNode>) -> Self {
        self.nodes.extend(nodes);
        self
    }

    pub fn expanded(mut self, ids: impl IntoIterator<Item = SharedString>) -> Self {
        self.expanded = ids.into_iter().collect();
        self
    }

    pub fn expanded_ids<S: AsRef<str>>(mut self, ids: &[S]) -> Self {
        self.expanded = ids
            .iter()
            .map(|id| SharedString::from(id.as_ref().to_string()))
            .collect();
        self
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    /// Bounds the viewport to `rows` rows, which is what lets the tree skip
    /// the disclosed rows it does not show.
    ///
    /// Without it the tree sizes itself to its content, which is the right
    /// answer for a hierarchy a reader takes in whole and the wrong one for a
    /// workspace with ten thousand files open.
    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = Some(rows);
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Lets a node be picked up and put somewhere else in the hierarchy.
    pub fn reorderable(mut self, reorderable: bool) -> Self {
        self.reorderable = reorderable;
        self
    }

    /// Whether this tree takes a payload, and where.
    ///
    /// Structural impossibilities are refused before this is consulted: a node
    /// cannot be moved inside itself or below one of its own descendants,
    /// because the descendant travels with it. Everything else — which kinds
    /// of node may hold which — is policy, and policy is the caller's.
    pub fn accepts(
        mut self,
        predicate: impl Fn(&DragItem, &DropPosition) -> bool + 'static,
    ) -> Self {
        self.accepts = Some(Rc::new(predicate));
        self
    }

    /// Reports where a dropped node should go. The tree does not move it.
    pub fn on_move(
        mut self,
        handler: impl Fn(&DropIntent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_move = Some(Rc::new(handler));
        self
    }

    /// A first load with nothing to show yet.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// A refresh that failed. Nodes that are still true stay on screen.
    pub fn failure(mut self, failure: impl Into<SharedString>) -> Self {
        self.failure = Some(failure.into());
        self
    }

    /// What to show when the query succeeded and returned nothing.
    pub fn empty(mut self, empty: EmptyState) -> Self {
        self.empty = Some(empty);
        self
    }
}

impl Slotted for Tree {
    const SLOTS: &'static [&'static str] =
        &[slot::EMPTY, slot::FAILED, slot::LOADING, slot::HEADER_EXTRA];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl Disableable for Tree {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for Tree {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

/// One node as it is actually shown: what the keyboard can reach.
#[derive(Debug, Clone)]
struct Visible {
    id: SharedString,
    label: SharedString,
    icon: Option<Icon>,
    disabled: bool,
    /// Root nodes are level 1, matching how assistive technology counts.
    level: u32,
    open: bool,
    has_children: bool,
    parent: Option<SharedString>,
    first_child: Option<SharedString>,
    kind: VisibleKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisibleKind {
    Node,
    Loading,
    Unavailable,
    Failed,
}

/// The nodes a frame shows, in the order the keyboard walks them.
///
/// A collapsed branch contributes only itself, which is why a move never lands
/// on something the typist cannot see.
#[derive(Default)]
struct FlattenCache {
    fingerprint: u64,
    rows: Vec<Visible>,
}

fn fingerprint_tree(nodes: &[TreeNode], expanded: &[SharedString]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    fn walk(nodes: &[TreeNode], hasher: &mut std::collections::hash_map::DefaultHasher) {
        for node in nodes {
            node.id.hash(hasher);
            node.label.hash(hasher);
            node.disabled.hash(hasher);
            node.branch.name().hash(hasher);
            if let BranchState::Unavailable(reason) | BranchState::Failed(reason) = &node.branch {
                reason.hash(hasher);
            }
            walk(&node.children, hasher);
        }
    }
    walk(nodes, &mut hasher);
    for id in expanded {
        id.hash(&mut hasher);
    }
    hasher.finish()
}

fn flatten(
    nodes: &[TreeNode],
    expanded: &[SharedString],
    level: u32,
    parent: Option<&SharedString>,
    out: &mut Vec<Visible>,
) {
    for node in nodes {
        let open = expanded.contains(&node.id);
        let has_children = !node.children.is_empty() || !matches!(node.branch, BranchState::Ready);
        out.push(Visible {
            id: node.id.clone(),
            label: node.label.clone(),
            icon: node.icon,
            disabled: node.disabled,
            level,
            open: open && has_children,
            has_children,
            parent: parent.cloned(),
            first_child: node.children.first().map(|child| child.id.clone()),
            kind: VisibleKind::Node,
        });
        if open && has_children {
            match &node.branch {
                BranchState::Loading if node.children.is_empty() => {
                    out.push(Visible {
                        id: SharedString::from(format!("{}.loading", node.id)),
                        label: SharedString::default(),
                        icon: None,
                        disabled: true,
                        level: level + 1,
                        open: false,
                        has_children: false,
                        parent: Some(node.id.clone()),
                        first_child: None,
                        kind: VisibleKind::Loading,
                    });
                }
                BranchState::Unavailable(reason) if node.children.is_empty() => {
                    out.push(Visible {
                        id: SharedString::from(format!("{}.unavailable", node.id)),
                        label: reason.clone(),
                        icon: None,
                        disabled: true,
                        level: level + 1,
                        open: false,
                        has_children: false,
                        parent: Some(node.id.clone()),
                        first_child: None,
                        kind: VisibleKind::Unavailable,
                    });
                }
                BranchState::Failed(reason) if node.children.is_empty() => {
                    out.push(Visible {
                        id: SharedString::from(format!("{}.failed", node.id)),
                        label: reason.clone(),
                        icon: None,
                        disabled: true,
                        level: level + 1,
                        open: false,
                        has_children: false,
                        parent: Some(node.id.clone()),
                        first_child: None,
                        kind: VisibleKind::Failed,
                    });
                }
                _ => flatten(&node.children, expanded, level + 1, Some(&node.id), out),
            }
        }
    }
}

/// One cell of indent per ancestor, each carrying the line that leads back to
/// it.
///
/// The indent is drawn rather than left as padding, so depth is a thing on
/// screen a reader can follow rather than an amount of nothing they have to
/// measure by eye. The line sits at the reading-start edge of its cell, which
/// puts it under the ancestor's disclosure whichever way the interface reads.
pub(crate) fn indent_guides(
    theme: &Theme,
    direction: LayoutDirection,
    level: u32,
    height: f32,
) -> Vec<gpui::Div> {
    (1..level)
        .map(|_| {
            div()
                .w(px(theme.space(Space::Md)))
                .h(px(height))
                .flex_none()
                .row_reading(direction)
                .border_s(direction, px(theme.borders.hairline))
                .border_color(theme.colors.hairline.opacity(theme.opacity.muted))
        })
        .collect()
}

/// What a keystroke reports: a selection, a disclosure change, or nothing.
enum Move {
    Select(SharedString),
    Toggle(SharedString, bool),
}

/// A horizontal arrow in a tree means "toward the children" or "toward the
/// parent", not "toward an edge of the screen": a branch opens in the
/// direction the indent grows, and the indent grows the way the tree reads. So
/// the two arrows swap once the interface reads right to left, while up, down,
/// home and end mean the same thing either way.
fn keystroke_move(
    key: &str,
    direction: LayoutDirection,
    visible: &[Visible],
    selected: Option<&SharedString>,
) -> Option<Move> {
    let at = visible
        .iter()
        .position(|node| Some(&node.id) == selected)
        .filter(|_| selected.is_some());
    let key = match direction.arrow_step(key) {
        Some(1) => "toward-children",
        Some(_) => "toward-parent",
        None => key,
    };
    match key {
        "up" | "down" => {
            let delta: isize = if key == "down" { 1 } else { -1 };
            let from = match at {
                Some(at) => at as isize + delta,
                // Entering from outside, a move lands on the end it travels
                // away from.
                None if delta > 0 => 0,
                None => visible.len() as isize - 1,
            };
            step(visible, from, delta).map(Move::Select)
        }
        "home" => step(visible, 0, 1).map(Move::Select),
        "end" => step(visible, visible.len() as isize - 1, -1).map(Move::Select),
        "toward-children" => {
            let node = visible.get(at?)?;
            if node.has_children && !node.open {
                Some(Move::Toggle(node.id.clone(), true))
            } else {
                node.first_child
                    .clone()
                    .filter(|_| node.open)
                    .map(Move::Select)
            }
        }
        "toward-parent" => {
            let node = visible.get(at?)?;
            if node.has_children && node.open {
                Some(Move::Toggle(node.id.clone(), false))
            } else {
                node.parent.clone().map(Move::Select)
            }
        }
        _ => None,
    }
}

/// The first node from `from` in `delta`'s direction that accepts selection.
fn step(visible: &[Visible], from: isize, delta: isize) -> Option<SharedString> {
    let mut index = from;
    while index >= 0 && (index as usize) < visible.len() {
        let node = &visible[index as usize];
        if !node.disabled {
            return Some(node.id.clone());
        }
        index += delta;
    }
    None
}

/// The node named `id`, wherever it sits.
fn find<'a>(nodes: &'a [TreeNode], id: &SharedString) -> Option<&'a TreeNode> {
    for node in nodes {
        if &node.id == id {
            return Some(node);
        }
        if let Some(found) = find(&node.children, id) {
            return Some(found);
        }
    }
    None
}

fn collect(node: &TreeNode, out: &mut Vec<SharedString>) {
    out.push(node.id.clone());
    for child in &node.children {
        collect(child, out);
    }
}

/// A node and everything hanging under it.
///
/// A node cannot be moved into, before, or after anything in here: its
/// descendants travel with it, so the destination would end up inside the
/// thing being moved. That is a structural impossibility rather than a policy,
/// which is why the tree judges it instead of asking the caller.
fn subtree(nodes: &[TreeNode], id: &SharedString) -> Vec<SharedString> {
    let mut ids = Vec::new();
    if let Some(node) = find(nodes, id) {
        collect(node, &mut ids);
    }
    ids
}

/// What a node needs to take part in a move.
#[derive(Clone)]
struct Reorder {
    surface: SharedString,
    drag: Option<SurfaceDrag>,
    accepts: Accepts,
    on_drop: MoveHandler,
}

impl Tree {
    fn reorder(&self, window: &mut Window, cx: &mut App) -> Option<Reorder> {
        if self.disabled || !self.reorderable {
            return None;
        }
        let on_drop = self.on_move.clone()?;
        let surface = self.ident.semantic_id();
        let nodes = self.nodes.clone();
        let caller = self.accepts.clone();
        let own = surface.clone();
        let accepts: Accepts = Rc::new(move |item: &DragItem, position: &DropPosition| {
            if item.source == own && subtree(&nodes, &item.id).contains(position.anchor()) {
                return false;
            }
            match &caller {
                Some(caller) => caller(item, position),
                None => item.source == own,
            }
        });
        Some(Reorder {
            drag: dnd::surface_drag(&surface, window, cx),
            surface,
            accepts,
            on_drop,
        })
    }

    fn vacant(&mut self, window: &mut Window, cx: &mut App) -> AnyElement {
        if let Some(replacement) = self
            .slots
            .render(slot::LOADING, window, cx)
            .filter(|_| self.loading)
        {
            return replacement;
        }
        if self.loading {
            return PulseLoader::new(self.ident.child("loading"))
                .label(cx.strings().text(StringKey::TreeLoadingChildren))
                .into_any_element();
        }
        if let Some(failure) = self.failure.clone() {
            if let Some(replacement) = self.slots.render(slot::FAILED, window, cx) {
                return replacement;
            }
            return EmptyState::new(self.ident.child("empty"), SharedString::default())
                .kind(EmptyKind::Failed)
                .detail(failure)
                .into_any_element();
        }
        if let Some(replacement) = self.slots.render(slot::EMPTY, window, cx) {
            return replacement;
        }
        match self.empty.take() {
            Some(empty) => empty.into_any_element(),
            None => EmptyState::new(
                self.ident.child("empty"),
                cx.strings().text(StringKey::TreeEmpty),
            )
            .kind(EmptyKind::Empty)
            .into_any_element(),
        }
    }
}

impl RenderOnce for Tree {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let extra = self.slots.render(slot::HEADER_EXTRA, window, cx);
        if self.nodes.is_empty() {
            let vacant = self.vacant(window, cx);
            let mut spec = NodeSpec::new(self.ident.semantic_id(), Role::Tree)
                .value("0")
                .busy(self.loading);
            if self.failure.is_some() {
                spec = spec.description(cx.strings().text(StringKey::TreeChildrenUnavailable));
            }
            return div()
                .id(self.ident.element_id())
                .column()
                .w_full()
                .children(extra)
                .child(vacant)
                .semantic_in(cx, spec)
                .into_any_element();
        }
        let reorder = self.reorder(window, cx);
        let cache = keyed::slot::<FlattenCache>(
            &self.ident.child("flatten").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let fingerprint = fingerprint_tree(&self.nodes, &self.expanded);
        let visible = {
            let mut cache = cache.borrow_mut();
            if cache.fingerprint == fingerprint && !cache.rows.is_empty() {
                cache.rows.clone()
            } else {
                let mut visible = Vec::new();
                flatten(&self.nodes, &self.expanded, 1, None, &mut visible);
                cache.fingerprint = fingerprint;
                cache.rows = visible.clone();
                visible
            }
        };

        let mut stack = div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .children(extra);

        // A tree that draws only its viewport can still be walked end to end,
        // because the keyboard moves over the flattened rows rather than over
        // the ones that happen to be built. A move that lands off screen
        // brings the row it named into view.
        let rows_ident = self.ident.child("rows");
        let scroll = self
            .visible_rows
            .map(|_| scroll_handle(&rows_ident, window, cx));

        if !self.disabled && (self.on_select.is_some() || self.on_toggle.is_some()) {
            let nodes = visible.clone();
            let selected = self.selected.clone();
            let select = self.on_select.clone();
            let toggle = self.on_toggle.clone();
            let direction = cx.layout_direction();
            let scroll = scroll.clone();
            stack = stack.on_key_down(move |event, window, cx| {
                let Some(next) = keystroke_move(
                    event.keystroke.key.as_str(),
                    direction,
                    &nodes,
                    selected.as_ref(),
                ) else {
                    return;
                };
                match next {
                    Move::Select(id) => {
                        if let (Some(scroll), Some(at)) =
                            (scroll.as_ref(), nodes.iter().position(|node| node.id == id))
                        {
                            scroll.scroll_to_item(at, ScrollStrategy::Nearest);
                            window.refresh();
                        }
                        if Some(&id) == selected.as_ref() {
                            return;
                        }
                        let Some(handler) = select.as_ref() else {
                            return;
                        };
                        handler(id, window, cx);
                    }
                    Move::Toggle(id, open) => {
                        let Some(handler) = toggle.as_ref() else {
                            return;
                        };
                        handler(id, open, window, cx);
                    }
                }
                cx.stop_propagation();
            });
        }

        let rows = Rows {
            ident: self.ident.clone(),
            selected: self.selected.clone(),
            disabled: self.disabled,
            size: self.size,
            on_select: self.on_select.clone(),
            on_toggle: self.on_toggle.clone(),
        };
        let count = visible.len();

        match (self.visible_rows, scroll) {
            (Some(bound), Some(scroll)) => {
                let theme = theme.clone();
                let icon_size = metrics.icon_size;
                let height = theme.control.get(self.size).height;
                stack = stack.child(
                    uniform_list(
                        rows_ident.element_id(),
                        count,
                        move |range: Range<usize>, window, cx| {
                            range
                                .map(|index| {
                                    rows.node_element(
                                        &visible[index],
                                        index,
                                        &theme,
                                        icon_size,
                                        reorder.as_ref(),
                                        window,
                                        cx,
                                    )
                                })
                                .collect::<Vec<_>>()
                        },
                    )
                    .track_scroll(&scroll)
                    .w_full()
                    .with_sizing_behavior(ListSizingBehavior::Auto)
                    // A hierarchy shorter than the bound ends where its last
                    // row ends, so a cap is not a claim about how much there
                    // is to disclose.
                    .h(px(height * count.min(bound) as f32)),
                );
            }
            _ => {
                for (index, node) in visible.iter().enumerate() {
                    stack = stack.child(rows.node_element(
                        node,
                        index,
                        &theme,
                        metrics.icon_size,
                        reorder.as_ref(),
                        window,
                        cx,
                    ));
                }
            }
        }

        stack
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Tree)
                    .value(cx.numbers().count(count)),
            )
            .into_any_element()
    }
}

/// Everything a node needs that does not come from the node itself.
///
/// A virtualized tree builds its rows inside a `'static` closure, which cannot
/// borrow the tree, so the few fields a row reads travel into the closure by
/// value and the unbounded path reads the same ones.
#[derive(Clone)]
struct Rows {
    ident: Ident,
    selected: Option<SharedString>,
    disabled: bool,
    size: ControlSize,
    on_select: Option<SelectHandler>,
    on_toggle: Option<ToggleHandler>,
}

impl Rows {
    #[allow(clippy::too_many_arguments)]
    fn node_element(
        &self,
        node: &Visible,
        index: usize,
        theme: &Theme,
        icon_size: f32,
        reorder: Option<&Reorder>,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::AnyElement {
        if node.kind != VisibleKind::Node {
            return self.status_row(node, theme, icon_size, cx);
        }
        let ident = self.ident.child(node.id.as_ref());
        let selected = self.selected.as_ref() == Some(&node.id);
        let disabled = self.disabled || node.disabled;
        let draggable = reorder.filter(|_| !disabled);
        let drag = draggable.and_then(|reorder| reorder.drag.as_ref());
        let carried = drag.is_some_and(|drag| drag.carries(&node.id));
        let landing = drag.and_then(|drag| drag.indicator_for(&node.id));
        let selectable = !disabled && self.on_select.is_some();
        let toggleable = !disabled && node.has_children && self.on_toggle.is_some();
        let color = if disabled {
            theme.colors.text_faint
        } else {
            theme.colors.text
        };
        let direction = cx.layout_direction();

        let chevron = node.has_children.then(|| {
            let toggle = ident.child("toggle");
            let mut glyph = div()
                .id(toggle.element_id())
                .row()
                .flex_none()
                .size(px(icon_size))
                .child(
                    icon(Icon::AltArrowRight)
                        .size(px(icon_size))
                        .text_color(theme.colors.text_muted)
                        .when(node.open, |glyph| {
                            glyph.with_transformation(Transformation::rotate(radians(FRAC_PI_2)))
                        })
                        // An open chevron already points down, which is the
                        // same way down in either reading direction, so only
                        // the shut one turns around.
                        .when(
                            !node.open && flips(Icon::AltArrowRight, direction),
                            |glyph| {
                                glyph.with_transformation(Transformation::scale(gpui::size(
                                    -1.0, 1.0,
                                )))
                            },
                        ),
                )
                .when(toggleable, |element| {
                    element
                        .cursor_pointer()
                        .tab_index(0)
                        .pressable(cx)
                        .focus_ring(theme)
                });

            if let (true, Some(handler)) = (toggleable, self.on_toggle.clone()) {
                let id = node.id.clone();
                let open = node.open;
                let keyed = Rc::clone(&handler);
                let keyed_id = id.clone();
                glyph = glyph.on_click(move |_, window, cx| {
                    handler(id.clone(), !open, window, cx);
                    // A disclosure is not a selection, so the row underneath
                    // must not also report one.
                    cx.stop_propagation();
                });
                glyph = glyph.on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        keyed(keyed_id.clone(), !open, window, cx);
                        cx.stop_propagation();
                    }
                });
            }

            glyph.semantic_in(
                cx,
                NodeSpec::new(toggle.semantic_id(), Role::Button)
                    .parent(ident.semantic_id())
                    .text(node.label.clone())
                    .expanded(node.open)
                    .disabled(!toggleable),
            )
        });

        let mut row = div()
            .id(ident.element_id())
            .row_reading(direction)
            .w_full()
            .h(px(theme.control.get(self.size).height))
            .pe(direction, px(theme.space(Space::Sm)))
            // The indent is the only thing that says how deep a node sits, so
            // it steps once per level from the edge reading starts at, and
            // each step is drawn rather than left as empty padding: a guide
            // per ancestor is what lets a reader follow a child at the bottom
            // of a long branch back up to the branch it belongs to.
            .ps(direction, px(theme.space(Space::Sm)))
            .gap(px(theme.space(Space::Xs)))
            .children(indent_guides(
                theme,
                direction,
                node.level,
                theme.control.get(self.size).height,
            ))
            .text_color(color)
            .selected_fill(theme, selected)
            .when(disabled, |element| element.opacity(theme.opacity.disabled))
            .when(carried, |element| element.opacity(theme.opacity.muted))
            .when(selectable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .when(!selected, |element| element.hover_row(theme))
                    .focus_ring(theme)
            })
            .children(chevron)
            // A leaf still lines up with its siblings, which is what makes the
            // indent readable as depth rather than as decoration.
            .when(!node.has_children, |element| {
                element.child(div().flex_none().size(px(icon_size)))
            })
            .children(
                node.icon
                    .map(|glyph| icon(glyph).size(px(icon_size)).text_color(color)),
            )
            .child(
                text(theme, TypeScale::Body, node.label.clone())
                    .flex_1()
                    .overflow_hidden()
                    .text_start(direction)
                    .text_color(color),
            )
            .children(landing.map(|(position, accepted)| {
                match position {
                    // A drop *into* a node is a landing zone, and a solid ring
                    // around a row is what focus already means here. The
                    // dashed outline is the same mark every landing zone in
                    // the library wears, so the two states cannot be read as
                    // each other.
                    DropPosition::Into(_) => {
                        let color = if accepted {
                            theme.colors.accent
                        } else {
                            theme.colors.danger
                        };
                        div()
                            .absolute()
                            .inset_0()
                            .radius(theme, Radius::Small)
                            .bg(color.opacity(theme.effects.semantic_wash_alpha))
                            .border(px(theme.borders.hairline))
                            .border_dashed()
                            .border_color(color.opacity(theme.effects.semantic_wash_strong_alpha))
                            .shadow(theme.glow(color))
                    }
                    position => dnd::indicator(&position, accepted, DropAxis::Vertical, cx),
                }
            }));

        if let (true, Some(handler)) = (selectable, self.on_select.clone()) {
            let id = node.id.clone();
            row = row.on_click(move |_, window, cx| handler(id.clone(), window, cx));
        }

        if let Some(reorder) = draggable {
            let mut item =
                DragItem::new(reorder.surface.clone(), node.id.clone(), node.label.clone());
            if let Some(glyph) = node.icon {
                item = item.icon(glyph);
            }
            row = dnd::draggable(row, item);
            row = dnd::drop_target(
                row,
                RowTarget {
                    surface: reorder.surface.clone(),
                    id: node.id.clone(),
                    index,
                    // Only a branch can be entered; a leaf offers the slots
                    // beside it and nothing else.
                    allow_into: node.has_children,
                    axis: DropAxis::Vertical,
                    accepts: Rc::clone(&reorder.accepts),
                    on_drop: Rc::clone(&reorder.on_drop),
                },
            );
        }

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::TreeItem)
            .parent(
                node.parent
                    .as_ref()
                    .map_or(self.ident.semantic_id(), |parent| {
                        self.ident.child(parent.as_ref()).semantic_id()
                    }),
            )
            .text(node.label.clone())
            .selected(selected)
            .disabled(disabled)
            .level(node.level);
        // Only a node that has something to disclose claims a disclosure
        // state; a leaf that reported `expanded: false` would look shut.
        if node.has_children {
            spec = spec.expanded(node.open);
        }

        let row = row.semantic_in(cx, spec);
        match draggable {
            Some(reorder) => {
                let shift = reorder
                    .drag
                    .as_ref()
                    .filter(|drag| drag.makes_way(index))
                    .map_or(px(0.0), |_| dnd::make_way_gap(cx, DropAxis::Vertical));
                row.make_way(ident.semantic_id(), point(px(0.0), shift), window, cx)
                    .into_any_element()
            }
            None => row.into_any_element(),
        }
    }

    fn status_row(
        &self,
        node: &Visible,
        theme: &Theme,
        icon_size: f32,
        cx: &mut App,
    ) -> AnyElement {
        let ident = self.ident.child(node.id.as_ref());
        let (label, detail, value, busy) = match node.kind {
            VisibleKind::Loading => (
                cx.strings().text(StringKey::TreeLoadingChildren),
                None,
                "loading",
                true,
            ),
            VisibleKind::Unavailable => (
                cx.strings().text(StringKey::TreeChildrenUnavailable),
                (!node.label.is_empty()).then(|| node.label.clone()),
                "unavailable",
                false,
            ),
            VisibleKind::Failed => (
                cx.strings().text(StringKey::TreeChildrenUnavailable),
                (!node.label.is_empty()).then(|| node.label.clone()),
                "failed",
                false,
            ),
            VisibleKind::Node => unreachable!("status rows are not nodes"),
        };
        let direction = cx.layout_direction();
        let height = theme.control.get(self.size).height;
        // Loading, refusal, failure, and disabled are different facts. A
        // branch that is fetching moves, a refusal carries the forbidden
        // mark, and a failed attempt carries the danger mark.
        let (mark, mark_color) = match node.kind {
            VisibleKind::Loading => (
                PulseLoader::new(ident.child("mark"))
                    .control_size(ControlSize::Xs)
                    .into_any_element(),
                theme.colors.text_muted,
            ),
            // A drawing carries its own colour: an SVG does not take the
            // text colour of the box it is put in, so a mark tinted only by
            // its parent came out invisible and left a refusal and a failure
            // looking like the same row.
            VisibleKind::Unavailable => (
                icon(Icon::Forbidden)
                    .size(px(icon_size))
                    .text_color(theme.colors.warning)
                    .into_any_element(),
                theme.colors.warning,
            ),
            VisibleKind::Failed => (
                icon(Icon::Danger)
                    .size(px(icon_size))
                    .text_color(theme.colors.danger)
                    .into_any_element(),
                theme.colors.danger,
            ),
            VisibleKind::Node => unreachable!("status rows are not nodes"),
        };
        let semantic_text = detail.clone().unwrap_or_else(|| label.clone());
        motion::surface_in(
            ident.element_id(),
            theme,
            div()
                .id(ident.element_id())
                .row_reading(direction)
                .w_full()
                .h(px(height))
                .ps(direction, px(theme.space(Space::Sm)))
                .gap(px(theme.space(Space::Xs)))
                .children(indent_guides(theme, direction, node.level, height))
                .child(
                    // A floor rather than a box: the moving mark is a row of
                    // dots wider than a glyph, and boxed at the glyph's width
                    // it overran the gap and sat against the label.
                    div()
                        .flex_none()
                        .h(px(icon_size))
                        .min_w(px(icon_size))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(mark_color)
                        .child(mark),
                )
                .children(detail.map(|detail| {
                    text(theme, TypeScale::Caption, detail)
                        .flex_1()
                        .text_start(direction)
                        .text_color(theme.colors.text_faint)
                }))
                .tip(ident.clone(), label)
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Status)
                        .parent(
                            node.parent
                                .as_ref()
                                .map_or(self.ident.semantic_id(), |parent| {
                                    self.ident.child(parent.as_ref()).semantic_id()
                                }),
                        )
                        .text(semantic_text)
                        .value(value)
                        .busy(busy)
                        .level(node.level),
                ),
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<TreeNode> {
        vec![
            TreeNode::new("workspace", "Workspace").children([
                TreeNode::new("src", "src").children([TreeNode::new("lib", "lib.rs")]),
                TreeNode::new("docs", "docs"),
            ]),
            TreeNode::new("target", "target").disabled(true),
        ]
    }

    fn visible(expanded: &[&str]) -> Vec<Visible> {
        let expanded: Vec<SharedString> = expanded
            .iter()
            .map(|id| SharedString::from(id.to_string()))
            .collect();
        let mut out = Vec::new();
        flatten(&sample(), &expanded, 1, None, &mut out);
        out
    }

    #[test]
    fn a_tree_fingerprint_changes_only_when_the_nodes_or_open_set_do() {
        let nodes = sample();
        let first = fingerprint_tree(&nodes, &[]);
        let again = fingerprint_tree(&nodes, &[]);
        let opened = fingerprint_tree(&nodes, &[SharedString::from("workspace")]);
        assert_eq!(first, again);
        assert_ne!(first, opened);
    }

    #[test]
    fn a_collapsed_branch_contributes_only_itself() {
        let nodes = visible(&[]);
        let ids: Vec<&str> = nodes.iter().map(|node| node.id.as_ref()).collect();
        assert_eq!(ids, vec!["workspace", "target"]);
    }

    #[test]
    fn an_open_branch_levels_its_children_one_deeper() {
        let nodes = visible(&["workspace"]);
        assert_eq!(nodes[0].level, 1);
        assert_eq!(nodes[1].level, 2);
        assert_eq!(nodes[1].parent.as_deref(), Some("workspace"));
    }

    #[test]
    fn a_move_down_skips_a_refusal_and_stops_at_the_end() {
        let nodes = visible(&["workspace"]);
        let from = SharedString::from("docs");
        // `target` refuses selection and is the last node, so the move lands
        // nowhere rather than wrapping.
        assert!(
            keystroke_move("down", LayoutDirection::LeftToRight, &nodes, Some(&from)).is_none()
        );
    }

    #[test]
    fn right_opens_a_shut_branch_and_then_descends() {
        let shut = visible(&[]);
        let workspace = SharedString::from("workspace");
        match keystroke_move(
            "right",
            LayoutDirection::LeftToRight,
            &shut,
            Some(&workspace),
        ) {
            Some(Move::Toggle(id, next)) => {
                assert_eq!(id.as_ref(), "workspace");
                assert!(next);
            }
            _ => panic!("right must open a shut branch"),
        }

        let open = visible(&["workspace"]);
        match keystroke_move(
            "right",
            LayoutDirection::LeftToRight,
            &open,
            Some(&workspace),
        ) {
            Some(Move::Select(id)) => assert_eq!(id.as_ref(), "src"),
            _ => panic!("right must descend into an open branch"),
        }
    }

    #[test]
    fn a_loading_branch_keeps_a_disclosure_and_a_status_row() {
        let nodes = [TreeNode::new("src", "src").branch(BranchState::Loading)];
        let expanded = [SharedString::from("src")];
        let mut out = Vec::new();
        flatten(&nodes, &expanded, 1, None, &mut out);
        assert!(out[0].has_children);
        assert_eq!(out[1].kind, VisibleKind::Loading);
        assert_eq!(out[1].parent.as_deref(), Some("src"));
    }

    #[test]
    fn an_unavailable_branch_publishes_the_refusal() {
        let nodes =
            [TreeNode::new("src", "src").branch(BranchState::Unavailable("host refused".into()))];
        let expanded = [SharedString::from("src")];
        let mut out = Vec::new();
        flatten(&nodes, &expanded, 1, None, &mut out);
        assert_eq!(out[1].kind, VisibleKind::Unavailable);
        assert_eq!(out[1].label.as_ref(), "host refused");
    }

    #[test]
    fn a_failed_branch_publishes_the_attempt_failure() {
        let nodes =
            [TreeNode::new("src", "src").branch(BranchState::Failed("listing failed".into()))];
        let expanded = [SharedString::from("src")];
        let mut out = Vec::new();
        flatten(&nodes, &expanded, 1, None, &mut out);
        assert_eq!(out[1].kind, VisibleKind::Failed);
        assert_eq!(out[1].label.as_ref(), "listing failed");
    }

    #[test]
    fn left_shuts_an_open_branch_and_otherwise_ascends() {
        let open = visible(&["workspace"]);
        let src = SharedString::from("src");
        match keystroke_move("left", LayoutDirection::LeftToRight, &open, Some(&src)) {
            Some(Move::Select(id)) => assert_eq!(id.as_ref(), "workspace"),
            _ => panic!("left must ascend from a leaf"),
        }

        let deeper = visible(&["workspace", "src"]);
        match keystroke_move("left", LayoutDirection::LeftToRight, &deeper, Some(&src)) {
            Some(Move::Toggle(id, next)) => {
                assert_eq!(id.as_ref(), "src");
                assert!(!next);
            }
            _ => panic!("left must shut an open branch"),
        }
    }
}
