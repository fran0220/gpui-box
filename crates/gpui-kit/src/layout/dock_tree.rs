//! Recursive, persistable dock groups over the shared split, tab, and drag
//! primitives.
//!
//! [`DockTopology`] is caller-owned data. [`DockTree`] projects its stacks
//! through [`SplitTree`], uses [`Tabs`] for group ordering, and reports every
//! selection, move, edge split, resize, and collapse request as a
//! [`DockTreeEvent`]. It never edits the topology. An empty stack remains a
//! real drop target, so moving its last panel away does not make the place
//! impossible to restore.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels,
    Point, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px, relative,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, TypeScale};

use crate::foundation::{
    Disableable, FocusRing, Hoverable, Ident, Pressable, SelectedFill, Sizable, StyledExt,
};
use crate::interaction::dnd::{self, DragItem, DropAxis, DropIntent, DropPosition, RowTarget};
use crate::layout::dock::DockPanel;
use crate::layout::split::SplitAxis;
use crate::layout::tree::{SplitChange, SplitLayout, SplitPaneSpec, SplitTree};
use crate::motion::{Flipping, flip};
use crate::navigation::tabs::{TabItem, Tabs};
use crate::overlay::Tooltipped;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

const DEFAULT_MIN: f32 = 160.0;
const DEFAULT_RAIL: f32 = 44.0;

type EventHandler = Rc<dyn Fn(DockTreeEvent, &mut Window, &mut App)>;

/// One tab stack at a leaf of a [`DockTopology`].
#[derive(Debug, Clone, PartialEq)]
pub struct DockStack {
    id: SharedString,
    panels: Vec<SharedString>,
    active: Option<SharedString>,
    min_width: f32,
    min_height: f32,
    rail: f32,
    collapsed: bool,
}

impl DockStack {
    pub fn new(
        id: impl Into<SharedString>,
        panels: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        Self {
            id: id.into(),
            panels: panels.into_iter().map(Into::into).collect(),
            active: None,
            min_width: DEFAULT_MIN,
            min_height: DEFAULT_MIN,
            rail: DEFAULT_RAIL,
            collapsed: false,
        }
    }

    pub fn active(mut self, panel: impl Into<SharedString>) -> Self {
        self.active = Some(panel.into());
        self
    }

    pub fn min(mut self, minimum: f32) -> Self {
        self.min_width = minimum.max(0.0);
        self.min_height = minimum.max(0.0);
        self
    }

    pub fn min_width(mut self, minimum: f32) -> Self {
        self.min_width = minimum.max(0.0);
        self
    }

    pub fn min_height(mut self, minimum: f32) -> Self {
        self.min_height = minimum.max(0.0);
        self
    }

    pub fn rail(mut self, extent: f32) -> Self {
        self.rail = extent.max(0.0);
        self
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn panels(&self) -> &[SharedString] {
        &self.panels
    }

    pub fn active_panel(&self) -> Option<&SharedString> {
        self.active
            .as_ref()
            .filter(|active| self.panels.contains(active))
            .or_else(|| self.panels.first())
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn effectively_collapsed(&self) -> bool {
        self.collapsed && !self.panels.is_empty()
    }

    fn split_spec(&self) -> SplitPaneSpec {
        SplitPaneSpec::new(self.id.clone())
            .min_width(self.min_width)
            .min_height(self.min_height)
            .rail(self.rail)
            .collapsed(self.effectively_collapsed())
    }
}

/// An in-surface floating stack. Bounds are fractions of the dock viewport,
/// preserved across window-size changes. Tile order is caller-owned z-order.
/// This does not create operating-system windows.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatingDock {
    stack: DockStack,
    bounds: Bounds<f32>,
}

/// Caller-persisted state. Storage and serialization remain host policy.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatingDockRecord {
    pub stack: DockRecord,
    pub bounds: Bounds<f32>,
}

impl FloatingDock {
    pub fn new(stack: DockStack, bounds: Bounds<f32>) -> Result<Self, DockRecordError> {
        let tile = Self { stack, bounds };
        Self::from_record(&tile.to_record())
    }

    pub fn stack(&self) -> &DockStack {
        &self.stack
    }
    pub fn bounds(&self) -> Bounds<f32> {
        self.bounds
    }

    pub fn to_record(&self) -> FloatingDockRecord {
        FloatingDockRecord {
            stack: DockTopology::Stack(self.stack.clone())
                .to_records()
                .remove(0),
            bounds: self.bounds,
        }
    }

    pub fn from_record(record: &FloatingDockRecord) -> Result<Self, DockRecordError> {
        let b = record.bounds;
        if ![b.origin.x, b.origin.y, b.size.width, b.size.height]
            .iter()
            .all(|v| v.is_finite())
            || b.origin.x < 0.0
            || b.origin.y < 0.0
            || b.origin.x >= 1.0
            || b.origin.y >= 1.0
            || b.size.width <= 0.0
            || b.size.height <= 0.0
            || b.origin.x + b.size.width > 1.0
            || b.origin.y + b.size.height > 1.0
        {
            return Err(DockRecordError::InvalidFloatingBounds(
                record.stack.id.clone(),
            ));
        }
        // A valid single record must be a stack, since splits require children.
        let topology = DockTopology::from_records(std::slice::from_ref(&record.stack))?;
        Ok(Self {
            stack: topology.stacks()[0].clone(),
            bounds: b,
        })
    }
}

/// A recursive arrangement of tab stacks.
#[derive(Debug, Clone, PartialEq)]
pub enum DockTopology {
    Stack(DockStack),
    Split {
        id: SharedString,
        axis: SplitAxis,
        ratio: f32,
        start: Box<DockTopology>,
        end: Box<DockTopology>,
    },
}

impl DockTopology {
    /// Restore both surfaces atomically, validating identities across them and
    /// retaining caller z-order. Kit owns neither storage nor serialization.
    pub fn restore_with_floating(
        records: &[DockRecord],
        floating: &[FloatingDockRecord],
    ) -> Result<(Self, Vec<FloatingDock>), DockRecordError> {
        let topology = Self::from_records(records)?;
        let floating = floating
            .iter()
            .map(FloatingDock::from_record)
            .collect::<Result<Vec<_>, _>>()?;
        validate_floating(&topology, &floating)?;
        Ok((topology, floating))
    }

    pub fn stack(
        id: impl Into<SharedString>,
        panels: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        Self::Stack(DockStack::new(id, panels))
    }

    pub fn split(
        id: impl Into<SharedString>,
        axis: SplitAxis,
        ratio: f32,
        start: DockTopology,
        end: DockTopology,
    ) -> Self {
        Self::Split {
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
        start: DockTopology,
        end: DockTopology,
    ) -> Self {
        Self::split(id, SplitAxis::Horizontal, ratio, start, end)
    }

    pub fn vertical(
        id: impl Into<SharedString>,
        ratio: f32,
        start: DockTopology,
        end: DockTopology,
    ) -> Self {
        Self::split(id, SplitAxis::Vertical, ratio, start, end)
    }

    pub fn id(&self) -> &SharedString {
        match self {
            Self::Stack(stack) => stack.id(),
            Self::Split { id, .. } => id,
        }
    }

    pub fn stacks(&self) -> Vec<&DockStack> {
        let mut stacks = Vec::new();
        self.walk(&mut |node, _| {
            if let Self::Stack(stack) = node {
                stacks.push(stack);
            }
        });
        stacks
    }

    pub fn find_stack(&self, id: &str) -> Option<&DockStack> {
        match self {
            Self::Stack(stack) => (stack.id.as_ref() == id).then_some(stack),
            Self::Split { start, end, .. } => start.find_stack(id).or_else(|| end.find_stack(id)),
        }
    }

    pub fn split_layout(&self) -> SplitLayout {
        match self {
            Self::Stack(stack) => SplitLayout::leaf(stack.split_spec()),
            Self::Split {
                id,
                axis,
                ratio,
                start,
                end,
            } => SplitLayout::split(
                id.clone(),
                *axis,
                *ratio,
                start.split_layout(),
                end.split_layout(),
            ),
        }
    }

    /// The topology flattened into plain records, parents before children.
    pub fn to_records(&self) -> Vec<DockRecord> {
        let mut records = Vec::new();
        self.record_into(None, &mut records);
        records
    }

    fn record_into(&self, parent: Option<SharedString>, records: &mut Vec<DockRecord>) {
        match self {
            Self::Stack(stack) => records.push(DockRecord {
                id: stack.id.clone(),
                parent,
                kind: DockRecordKind::Stack,
                ratio: 0.0,
                panels: stack.panels.clone(),
                active: stack.active.clone(),
                min_width: stack.min_width,
                min_height: stack.min_height,
                rail: stack.rail,
                collapsed: stack.collapsed,
            }),
            Self::Split {
                id,
                axis,
                ratio,
                start,
                end,
            } => {
                records.push(DockRecord {
                    id: id.clone(),
                    parent,
                    kind: match axis {
                        SplitAxis::Horizontal => DockRecordKind::Horizontal,
                        SplitAxis::Vertical => DockRecordKind::Vertical,
                    },
                    ratio: *ratio,
                    panels: Vec::new(),
                    active: None,
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

    pub fn from_records(records: &[DockRecord]) -> Result<Self, DockRecordError> {
        let mut children: HashMap<&str, Vec<&DockRecord>> = HashMap::new();
        let mut by_id: HashMap<&str, &DockRecord> = HashMap::new();
        let mut roots = Vec::new();
        for record in records {
            if by_id.insert(record.id.as_ref(), record).is_some() {
                return Err(DockRecordError::DuplicateId(record.id.clone()));
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
                return Err(DockRecordError::MissingParent {
                    id: record.id.clone(),
                    parent: parent.clone(),
                });
            }
        }
        let root = match roots.as_slice() {
            [] => return Err(DockRecordError::NoRoot),
            [root] => *root,
            _ => {
                return Err(DockRecordError::ManyRoots(
                    roots.iter().map(|record| record.id.clone()).collect(),
                ));
            }
        };
        let mut built = 0;
        let topology = build(root, &children, &mut built)?;
        if built != records.len() {
            return Err(DockRecordError::Unreachable);
        }
        let mut panels = HashSet::new();
        for stack in topology.stacks() {
            for panel in &stack.panels {
                if !panels.insert(panel.as_ref()) {
                    return Err(DockRecordError::DuplicatePanel(panel.clone()));
                }
            }
            if let Some(active) = &stack.active
                && !stack.panels.contains(active)
            {
                return Err(DockRecordError::MissingActive {
                    stack: stack.id.clone(),
                    panel: active.clone(),
                });
            }
        }
        Ok(topology)
    }

    fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a DockTopology, Option<SplitAxis>)) {
        fn recurse<'a>(
            node: &'a DockTopology,
            parent_axis: Option<SplitAxis>,
            visit: &mut impl FnMut(&'a DockTopology, Option<SplitAxis>),
        ) {
            visit(node, parent_axis);
            if let DockTopology::Split {
                axis, start, end, ..
            } = node
            {
                recurse(start, Some(*axis), visit);
                recurse(end, Some(*axis), visit);
            }
        }
        recurse(self, None, visit);
    }
}

fn build(
    record: &DockRecord,
    children: &HashMap<&str, Vec<&DockRecord>>,
    built: &mut usize,
) -> Result<DockTopology, DockRecordError> {
    *built += 1;
    let own = children
        .get(record.id.as_ref())
        .map(Vec::as_slice)
        .unwrap_or_default();
    match record.kind {
        DockRecordKind::Stack => {
            if !own.is_empty() {
                return Err(DockRecordError::StackWithChildren(record.id.clone()));
            }
            Ok(DockTopology::Stack(DockStack {
                id: record.id.clone(),
                panels: record.panels.clone(),
                active: record.active.clone(),
                min_width: record.min_width.max(0.0),
                min_height: record.min_height.max(0.0),
                rail: record.rail.max(0.0),
                collapsed: record.collapsed,
            }))
        }
        DockRecordKind::Horizontal | DockRecordKind::Vertical => {
            if !record.panels.is_empty() || record.active.is_some() {
                return Err(DockRecordError::SplitWithPanels(record.id.clone()));
            }
            let [start, end] = own else {
                return Err(DockRecordError::WrongChildCount {
                    id: record.id.clone(),
                    found: own.len(),
                });
            };
            Ok(DockTopology::split(
                record.id.clone(),
                match record.kind {
                    DockRecordKind::Horizontal => SplitAxis::Horizontal,
                    _ => SplitAxis::Vertical,
                },
                record.ratio,
                build(start, children, built)?,
                build(end, children, built)?,
            ))
        }
    }
}

/// What one [`DockRecord`] describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockRecordKind {
    Stack,
    Horizontal,
    Vertical,
}

/// One node of a [`DockTopology`] as plain caller-serializable fields.
#[derive(Debug, Clone, PartialEq)]
pub struct DockRecord {
    pub id: SharedString,
    pub parent: Option<SharedString>,
    pub kind: DockRecordKind,
    pub ratio: f32,
    pub panels: Vec<SharedString>,
    pub active: Option<SharedString>,
    pub min_width: f32,
    pub min_height: f32,
    pub rail: f32,
    pub collapsed: bool,
}

/// Why persisted dock records do not form one valid topology.
#[derive(Debug, Clone, PartialEq)]
pub enum DockRecordError {
    NoRoot,
    ManyRoots(Vec<SharedString>),
    DuplicateId(SharedString),
    MissingParent {
        id: SharedString,
        parent: SharedString,
    },
    WrongChildCount {
        id: SharedString,
        found: usize,
    },
    StackWithChildren(SharedString),
    SplitWithPanels(SharedString),
    DuplicatePanel(SharedString),
    MissingActive {
        stack: SharedString,
        panel: SharedString,
    },
    Unreachable,
    InvalidFloatingBounds(SharedString),
}

impl std::fmt::Display for DockRecordError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRoot => write!(formatter, "no record without a parent"),
            Self::ManyRoots(ids) => write!(
                formatter,
                "more than one root: {}",
                ids.iter()
                    .map(SharedString::as_ref)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::DuplicateId(id) => write!(formatter, "`{id}` appears more than once"),
            Self::MissingParent { id, parent } => {
                write!(formatter, "`{id}` names a parent `{parent}` that is absent")
            }
            Self::WrongChildCount { id, found } => {
                write!(formatter, "split `{id}` has {found} children, not 2")
            }
            Self::StackWithChildren(id) => write!(formatter, "stack `{id}` has children"),
            Self::SplitWithPanels(id) => write!(formatter, "split `{id}` contains panels"),
            Self::DuplicatePanel(id) => write!(formatter, "panel `{id}` appears more than once"),
            Self::MissingActive { stack, panel } => {
                write!(
                    formatter,
                    "stack `{stack}` activates absent panel `{panel}`"
                )
            }
            Self::Unreachable => write!(formatter, "records the root does not reach"),
            Self::InvalidFloatingBounds(id) => write!(
                formatter,
                "floating stack `{id}` has invalid viewport bounds"
            ),
        }
    }
}

impl std::error::Error for DockRecordError {}

/// Which side of an existing stack a dropped panel should split into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockPlacement {
    Left,
    Right,
    Top,
    Bottom,
}

impl DockPlacement {
    fn string_key(self) -> StringKey {
        match self {
            Self::Left => StringKey::DockSplitLeft,
            Self::Right => StringKey::DockSplitRight,
            Self::Top => StringKey::DockSplitTop,
            Self::Bottom => StringKey::DockSplitBottom,
        }
    }
}

/// Caller-owned changes requested through [`DockTree`].
#[derive(Debug, Clone, PartialEq)]
pub enum DockTreeEvent {
    /// Request caller-controlled back-to-front reordering.
    FloatingRaised {
        stack: SharedString,
    },
    /// Normalized geometry; the caller chooses whether to save live changes or
    /// only completed gestures. Keyboard adjustments finish immediately.
    FloatingChanged {
        stack: SharedString,
        bounds: Bounds<f32>,
        finished: bool,
    },
    /// A gesture was interrupted, not completed. The caller may roll back its
    /// live geometry to its last persisted record.
    FloatingCancelled {
        stack: SharedString,
    },
    PanelSelected {
        stack: SharedString,
        panel: SharedString,
    },
    PanelMoved {
        panel: SharedString,
        to_stack: SharedString,
        before: Option<SharedString>,
    },
    PanelSplit {
        panel: SharedString,
        target_stack: SharedString,
        placement: DockPlacement,
    },
    SplitResized {
        split: SharedString,
        ratio: f32,
    },
    StackCollapsed {
        stack: SharedString,
        collapsed: bool,
    },
}

/// Draws a recursive [`DockTopology`] without owning it.
#[derive(IntoElement)]
pub struct DockTree {
    ident: Ident,
    topology: DockTopology,
    floating: Vec<FloatingDock>,
    panels: Vec<DockPanel>,
    disabled: bool,
    on_event: Option<EventHandler>,
}

impl std::fmt::Debug for DockTree {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DockTree")
            .field("ident", &self.ident)
            .field("stacks", &self.topology.stacks().len())
            .field("panels", &self.panels.len())
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl DockTree {
    pub fn new(ident: impl Into<Ident>, topology: DockTopology) -> Self {
        Self {
            ident: ident.into(),
            topology,
            floating: Vec::new(),
            panels: Vec::new(),
            disabled: false,
            on_event: None,
        }
    }

    pub fn panel(mut self, panel: DockPanel) -> Self {
        self.panels.push(panel);
        self
    }

    /// Floating stacks in back-to-front order, sharing panel and stack identity
    /// with the docked topology. Invalid identities are rejected atomically.
    pub fn floating(
        mut self,
        tiles: impl IntoIterator<Item = FloatingDock>,
    ) -> Result<Self, DockRecordError> {
        let tiles: Vec<_> = tiles.into_iter().collect();
        validate_floating(&self.topology, &tiles)?;
        self.floating = tiles;
        Ok(self)
    }

    pub fn panels(mut self, panels: impl IntoIterator<Item = DockPanel>) -> Self {
        self.panels.extend(panels);
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(DockTreeEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }

    fn panel_by_id(&self, id: &str) -> Option<&DockPanel> {
        self.panels.iter().find(|panel| panel.id().as_ref() == id)
    }

    fn surface(&self, stack: &DockStack) -> SharedString {
        self.ident
            .child(stack.id().as_ref())
            .child("tabs")
            .semantic_id()
    }

    // Semantic surface identities must be unique within a window, including
    // paths formed from dock and stack ids. Prefix-related roots are valid:
    // only an exact source surface and a member of that stack are accepted.
    fn owned_sources(&self) -> Vec<(SharedString, Vec<SharedString>)> {
        self.topology
            .stacks()
            .into_iter()
            .chain(self.floating.iter().map(|tile| &tile.stack))
            .map(|stack| (self.surface(stack), stack.panels.clone()))
            .collect()
    }

    fn header(&self, stack: &DockStack, window: &mut Window, cx: &mut App) -> AnyElement {
        let ident = self.ident.child(stack.id().as_ref());
        let mut tabs = Tabs::new(ident.child("tabs"))
            .small()
            .tabs(stack.panels.iter().filter_map(|id| {
                let panel = self.panel_by_id(id)?;
                let mut tab = TabItem::new(panel.id().clone(), panel.title().clone());
                if let Some(glyph) = panel.glyph() {
                    tab = tab.icon(glyph);
                }
                if let Some(badge) = panel.badge_text() {
                    tab = tab.badge(badge);
                }
                Some(tab)
            }))
            .disabled(self.disabled);
        if let Some(active) = stack.active_panel() {
            tabs = tabs.selected(active.clone());
        }
        if let (false, Some(handler)) = (self.disabled, self.on_event.clone()) {
            let selected = handler.clone();
            let stack_id = stack.id.clone();
            let destination = stack.id.clone();
            let sources = self.owned_sources();
            let panels = stack.panels.clone();
            tabs = tabs
                .on_select(move |panel, window, cx| {
                    selected(
                        DockTreeEvent::PanelSelected {
                            stack: stack_id.clone(),
                            panel,
                        },
                        window,
                        cx,
                    );
                })
                .reorderable(true)
                .accepts(move |item: &DragItem, _| {
                    sources.iter().any(|(surface, panels)| {
                        surface == &item.source && panels.contains(&item.id)
                    })
                })
                .on_reorder(move |intent, window, cx| {
                    handler(
                        DockTreeEvent::PanelMoved {
                            panel: intent.item.id.clone(),
                            to_stack: destination.clone(),
                            before: before_in(&panels, &intent.position),
                        },
                        window,
                        cx,
                    );
                });
        }
        let collapse = self
            .on_event
            .clone()
            .filter(|_| !self.disabled && !stack.panels.is_empty())
            .map(|handler| {
                let button = ident.child("collapse");
                let stack = stack.id.clone();
                let name = cx.strings().text(StringKey::DockCollapseRegion);
                div()
                    .id(button.element_id())
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .mb(px(cx.theme().borders.thick))
                    .size(px(cx.theme().control.get(ControlSize::Sm).height))
                    .radius(cx.theme(), Radius::Control)
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .hover(|style| style.bg(cx.theme().colors.hover))
                    .focus_ring(cx.theme())
                    .child(
                        icon(Icon::Sidebar)
                            .size(px(cx.theme().control.get(ControlSize::Sm).icon_size))
                            .text_color(cx.theme().colors.text_muted),
                    )
                    .on_click(move |_, window, cx| {
                        handler(
                            DockTreeEvent::StackCollapsed {
                                stack: stack.clone(),
                                collapsed: true,
                            },
                            window,
                            cx,
                        );
                    })
                    .tip(button.clone(), name.clone())
                    .semantic_in(
                        cx,
                        NodeSpec::new(button.semantic_id(), Role::Button)
                            .parent(ident.semantic_id())
                            .text(name),
                    )
            });
        let _ = window;
        div()
            .row()
            .w_full()
            .flex_none()
            .items_end()
            .gap_token(cx.theme(), Space::Xs)
            .px_token(cx.theme(), Space::Xs)
            .overflow_hidden()
            .bg(cx.theme().colors.panel)
            .child(div().flex_1().min_w(px(0.0)).overflow_hidden().child(tabs))
            .children(collapse)
            .into_any_element()
    }

    fn body(&self, stack: &DockStack, window: &mut Window, cx: &mut App) -> AnyElement {
        let theme = cx.theme().clone();
        let ident = self.ident.child(stack.id().as_ref());
        let active = stack
            .active_panel()
            .and_then(|id| self.panel_by_id(id.as_ref()));
        let content = match active {
            Some(panel) => match panel.unavailable_reason() {
                Some(reason) => div()
                    .column()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_token(&theme, Space::Sm)
                    .p_token(&theme, Space::Lg)
                    .child(
                        icon(Icon::CloseCircle)
                            .size(px(theme.measures.standalone_icon))
                            .text_color(theme.colors.warning),
                    )
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Body)
                            .text_color(theme.colors.text)
                            .child(panel.title().clone()),
                    )
                    .child(
                        div()
                            .max_w(px(theme.measures.readable_width))
                            .text_align(gpui::TextAlign::Center)
                            .type_scale(&theme, TypeScale::Caption)
                            .text_color(theme.colors.text_muted)
                            .child(reason),
                    )
                    .into_any_element(),
                None => div()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .children(panel.take_content())
                    .into_any_element(),
            },
            None => div()
                .id(ident.child("empty").element_id())
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    icon(Icon::CornersIn)
                        .size(px(theme.measures.standalone_icon))
                        .text_color(theme.colors.text_faint),
                )
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child("empty").semantic_id(), Role::Status)
                        .parent(ident.semantic_id())
                        .text(cx.strings().text(StringKey::DockEmptyStack)),
                )
                .into_any_element(),
        };
        let mut frame = div()
            .id(ident.child("body").element_id())
            .relative()
            .column()
            .flex_1()
            .min_h(px(0.0))
            .overflow_hidden()
            .child(content);
        if let (false, Some(handler)) = (self.disabled, self.on_event.clone()) {
            frame = self.merge_target(frame, stack, handler.clone(), window, cx);
            if dnd::active(window, cx).is_some() {
                frame = frame.children([
                    self.split_target(stack, DockPlacement::Left, handler.clone(), window, cx),
                    self.split_target(stack, DockPlacement::Right, handler.clone(), window, cx),
                    self.split_target(stack, DockPlacement::Top, handler.clone(), window, cx),
                    self.split_target(stack, DockPlacement::Bottom, handler, window, cx),
                ]);
            }
        }
        let title = active.map(|panel| panel.title().clone());
        let panel_id = active.map(|panel| panel.id().clone());
        let mut spec = NodeSpec::new(ident.child("body").semantic_id(), Role::TabPanel)
            .parent(ident.semantic_id());
        if let Some(title) = title {
            spec = spec.text(title);
        }
        let frame = frame.semantic_in(cx, spec);
        match panel_id {
            Some(panel) => {
                let slide = flip(self.ident.child(panel.as_ref()).semantic_id(), window, cx);
                frame.flip(&slide, window, cx).into_any_element()
            }
            None => frame.into_any_element(),
        }
    }

    fn merge_target(
        &self,
        frame: gpui::Stateful<gpui::Div>,
        stack: &DockStack,
        handler: EventHandler,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Stateful<gpui::Div> {
        let surface = self.surface(stack);
        let id = stack.id.clone();
        let indicator = dnd::surface_drag(&surface, window, cx)
            .and_then(|drag| drag.indicator_for(&id))
            .map(|(position, accepted)| {
                dnd::indicator(&position, accepted, DropAxis::Vertical, cx)
            });
        let sources = self.owned_sources();
        let destination = stack.id.clone();
        dnd::drop_target(
            frame.children(indicator),
            RowTarget {
                surface,
                id,
                index: 0,
                allow_into: true,
                axis: DropAxis::Vertical,
                accepts: Rc::new(move |item: &DragItem, _| {
                    sources.iter().any(|(surface, panels)| {
                        surface == &item.source && panels.contains(&item.id)
                    })
                }),
                on_drop: Rc::new(move |intent: &DropIntent, window, cx| {
                    handler(
                        DockTreeEvent::PanelMoved {
                            panel: intent.item.id.clone(),
                            to_stack: destination.clone(),
                            before: None,
                        },
                        window,
                        cx,
                    );
                }),
            },
        )
    }

    fn split_target(
        &self,
        stack: &DockStack,
        placement: DockPlacement,
        handler: EventHandler,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let name = match placement {
            DockPlacement::Left => "split-left",
            DockPlacement::Right => "split-right",
            DockPlacement::Top => "split-top",
            DockPlacement::Bottom => "split-bottom",
        };
        let target = self.ident.child(stack.id().as_ref()).child(name);
        let surface = self.surface(stack);
        let target_id = target.semantic_id();
        let accepted =
            dnd::surface_drag(&surface, window, cx).and_then(|drag| drag.indicator_for(&target_id));
        let mut zone = div()
            .id(target.element_id())
            .absolute()
            .when(matches!(placement, DockPlacement::Left), |element| {
                element.left_0().top_0().bottom_0().w(relative(0.2))
            })
            .when(matches!(placement, DockPlacement::Right), |element| {
                element.right_0().top_0().bottom_0().w(relative(0.2))
            })
            .when(matches!(placement, DockPlacement::Top), |element| {
                element
                    .top_0()
                    .left(relative(0.2))
                    .w(relative(0.6))
                    .h(relative(0.2))
            })
            .when(matches!(placement, DockPlacement::Bottom), |element| {
                element
                    .bottom_0()
                    .left(relative(0.2))
                    .w(relative(0.6))
                    .h(relative(0.2))
            });
        if let Some((_, accepted)) = accepted {
            zone = zone.child(dnd::indicator(
                &DropPosition::Into(target_id.clone()),
                accepted,
                DropAxis::Vertical,
                cx,
            ));
        }
        zone = zone.semantic_in(
            cx,
            NodeSpec::new(target_id.clone(), Role::Button)
                .parent(
                    self.ident
                        .child(stack.id().as_ref())
                        .child("body")
                        .semantic_id(),
                )
                .text(cx.strings().text(placement.string_key())),
        );
        let sources = self.owned_sources();
        let destination = stack.id.clone();
        dnd::drop_target(
            zone,
            RowTarget {
                surface,
                id: target_id,
                index: 0,
                allow_into: true,
                axis: DropAxis::Vertical,
                accepts: Rc::new(move |item: &DragItem, _| {
                    sources.iter().any(|(surface, panels)| {
                        surface == &item.source && panels.contains(&item.id)
                    })
                }),
                on_drop: Rc::new(move |intent: &DropIntent, window, cx| {
                    handler(
                        DockTreeEvent::PanelSplit {
                            panel: intent.item.id.clone(),
                            target_stack: destination.clone(),
                            placement,
                        },
                        window,
                        cx,
                    );
                }),
            },
        )
        .into_any_element()
    }

    fn stack_element(
        &self,
        stack: &DockStack,
        parent_axis: Option<SplitAxis>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let ident = self.ident.child(stack.id().as_ref());
        let content = if stack.effectively_collapsed() {
            let upright = parent_axis != Some(SplitAxis::Vertical);
            let active = stack.active_panel().cloned();
            let actionable = !self.disabled && self.on_event.is_some();
            div()
                .flex()
                .when(upright, |element| element.flex_col())
                .when(!upright, |element| element.flex_row())
                .items_center()
                .gap_token(&theme, Space::Xs)
                .p_token(&theme, Space::Xs)
                .children(stack.panels.iter().filter_map(|id| {
                    let panel = self.panel_by_id(id)?;
                    let item = ident.child("rail").child(id.as_ref());
                    let selected = active.as_ref() == Some(id);
                    let mut element = div()
                        .id(item.element_id())
                        .size(px(theme.control.sm.height))
                        .flex()
                        .items_center()
                        .justify_center()
                        .radius(&theme, Radius::Control)
                        .selected_fill(&theme, selected)
                        .when(actionable, |element| {
                            element
                                .cursor_pointer()
                                .tab_index(0)
                                .pressable(cx)
                                .when(!selected, |element| element.hover_row(&theme))
                                .focus_ring(&theme)
                        })
                        .child(match panel.glyph() {
                            Some(glyph) => icon(glyph)
                                .size(px(theme.control.sm.icon_size))
                                .text_color(theme.colors.text_muted)
                                .into_any_element(),
                            None => div()
                                .type_scale(&theme, TypeScale::Caption)
                                .text_color(theme.colors.text_muted)
                                .child(initial(panel.title()))
                                .into_any_element(),
                        })
                        .tip(item.clone(), panel.title().clone());
                    if let Some(handler) = self.on_event.clone().filter(|_| actionable) {
                        let stack = stack.id.clone();
                        let panel = id.clone();
                        element = element.on_click(move |_, window, cx| {
                            handler(
                                DockTreeEvent::PanelSelected {
                                    stack: stack.clone(),
                                    panel: panel.clone(),
                                },
                                window,
                                cx,
                            );
                            handler(
                                DockTreeEvent::StackCollapsed {
                                    stack: stack.clone(),
                                    collapsed: false,
                                },
                                window,
                                cx,
                            );
                        });
                    }
                    Some(
                        element.semantic_in(
                            cx,
                            NodeSpec::new(item.semantic_id(), Role::Button)
                                .parent(ident.semantic_id())
                                .selected(selected)
                                .disabled(!actionable)
                                .text(panel.title().clone()),
                        ),
                    )
                }))
                .into_any_element()
        } else {
            div()
                .column()
                .size_full()
                .overflow_hidden()
                .child(self.header(stack, window, cx))
                .child(self.body(stack, window, cx))
                .into_any_element()
        };
        div()
            .id(ident.element_id())
            .column()
            .size_full()
            .overflow_hidden()
            .bg(theme.colors.panel)
            .child(content)
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Region)
                    .parent(self.ident.semantic_id())
                    .expanded(!stack.effectively_collapsed())
                    .value(cx.numbers().count(stack.panels.len())),
            )
            .into_any_element()
    }
}

impl Disableable for DockTree {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for DockTree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let measured = crate::layout::measure::cell(&self.ident.semantic_id(), window, cx);
        let mut tree =
            SplitTree::new(self.ident.child("layout")).layout(self.topology.split_layout());
        let mut stacks = Vec::new();
        self.topology.walk(&mut |node, parent_axis| {
            if let DockTopology::Stack(stack) = node {
                stacks.push((stack, parent_axis));
            }
        });
        for (stack, parent_axis) in stacks {
            tree = tree.pane(
                stack.id().clone(),
                self.stack_element(stack, parent_axis, window, cx),
            );
        }
        if let (false, Some(handler)) = (self.disabled, self.on_event.clone()) {
            tree = tree.on_change(move |change, window, cx| {
                if let SplitChange::Ratio { split, ratio } = change {
                    handler(DockTreeEvent::SplitResized { split, ratio }, window, cx);
                }
            });
        }
        let floating = self
            .floating
            .iter()
            .map(|tile| self.floating_element(tile, measured.clone(), window, cx))
            .collect::<Vec<_>>();
        div()
            .on_children_prepainted(move |bounds, window, _| {
                if let Some(first) = bounds.first() {
                    crate::layout::measure::record(&measured, *first, window);
                }
            })
            .id(self.ident.element_id())
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().colors.canvas)
            .child(tree)
            .children(floating)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group).value(
                    self.topology
                        .stacks()
                        .into_iter()
                        .chain(self.floating.iter().map(|tile| &tile.stack))
                        .map(|stack| stack.panels.len())
                        .sum::<usize>()
                        .to_string(),
                ),
            )
    }
}

impl DockTree {
    fn floating_element(
        &self,
        tile: &FloatingDock,
        measured: Rc<Cell<Bounds<Pixels>>>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let ident = self.ident.child(format!("floating.{}", tile.stack.id));
        let actionable = !self.disabled && self.on_event.is_some();
        let mut frame = div()
            .relative()
            .flex()
            .flex_col()
            .absolute()
            .left(relative(tile.bounds.origin.x))
            .top(relative(tile.bounds.origin.y))
            .w(relative(tile.bounds.size.width))
            .h(relative(tile.bounds.size.height))
            .bg(theme.colors.canvas)
            .border_1()
            .border_color(theme.colors.control_hairline)
            .occlude()
            .overflow_hidden();
        if let Some(handler) = self.on_event.clone().filter(|_| actionable) {
            let stack = tile.stack.id.clone();
            frame = frame.capture_any_mouse_down(move |event, window, cx| {
                if event.button == MouseButton::Left {
                    handler(
                        DockTreeEvent::FloatingRaised {
                            stack: stack.clone(),
                        },
                        window,
                        cx,
                    );
                }
            });
        }
        for (name, resize, key) in [
            ("move", false, StringKey::DockMoveFloating),
            ("resize", true, StringKey::DockResizeFloating),
        ] {
            let handle = ident.child(name);
            let held = crate::foundation::window_state::with_key(
                &handle.semantic_id(),
                window.window_handle().window_id(),
                cx,
                |state: &mut Rc<Cell<Option<FloatingGesture>>>| state.clone(),
            );
            if !actionable {
                held.set(None);
            }
            let mut control = div()
                .id(handle.element_id())
                .h(px(theme.control.sm.height))
                .flex()
                .items_center()
                .justify_center()
                .flex_none()
                .when(resize, |el| {
                    el.absolute()
                        .bottom_0()
                        .right_0()
                        .w(px(theme.control.sm.height))
                })
                .when(!resize, |el| el.w_full())
                .bg(theme.colors.canvas)
                .child(
                    icon(if resize {
                        Icon::AltArrowDown
                    } else {
                        Icon::DragHandle
                    })
                    .size(px(theme.control.sm.icon_size))
                    .text_color(theme.colors.text_muted),
                )
                .semantic_in(
                    cx,
                    NodeSpec::new(handle.semantic_id(), Role::Button)
                        .text(cx.strings().text(key))
                        .disabled(!actionable),
                );
            if let Some(handler) = self.on_event.clone().filter(|_| actionable) {
                let id = tile.stack.id.clone();
                let gesture = FloatingGesture {
                    position: Point::default(),
                    bounds: tile.bounds,
                    resize,
                    min: (tile.stack.min_width, tile.stack.min_height),
                };
                let state = held.clone();
                control = control
                    .tab_index(0)
                    .focus_ring(&theme)
                    .cursor_pointer()
                    .on_mouse_down_with_pointer_capture(MouseButton::Left, move |event, _, _| {
                        state.set(Some(FloatingGesture {
                            position: event.position,
                            ..gesture
                        }));
                        // Let GPUI's default pointer-focus listener run.
                    });
                let state = held.clone();
                let change = handler.clone();
                let stack = id.clone();
                let bounds = measured.clone();
                control = control.on_mouse_move(move |event, window, cx| {
                    if let Some(gesture) = state.get() {
                        change(
                            DockTreeEvent::FloatingChanged {
                                stack: stack.clone(),
                                bounds: gesture.request(event.position, bounds.get()),
                                finished: false,
                            },
                            window,
                            cx,
                        );
                        cx.stop_propagation();
                    }
                });
                let state = held.clone();
                let change = handler.clone();
                let stack = id.clone();
                let bounds = measured.clone();
                control = control.on_mouse_up(MouseButton::Left, move |event, window, cx| {
                    if let Some(gesture) = state.take() {
                        change(
                            DockTreeEvent::FloatingChanged {
                                stack: stack.clone(),
                                bounds: gesture.request(event.position, bounds.get()),
                                finished: true,
                            },
                            window,
                            cx,
                        );
                        cx.stop_propagation();
                    }
                });
                let state = held.clone();
                let change = handler.clone();
                let stack = id.clone();
                let bounds = measured.clone();
                control = control.on_key_down(move |event, window, cx| {
                    if event.keystroke.key == "escape" {
                        if state.take().is_some() {
                            window.release_pointer();
                            change(
                                DockTreeEvent::FloatingCancelled {
                                    stack: stack.clone(),
                                },
                                window,
                                cx,
                            );
                            cx.stop_propagation();
                        }
                        return;
                    }
                    if state.get().is_some() {
                        return;
                    }
                    let (x, y) = match event.keystroke.key.as_str() {
                        "left" => (-10.0, 0.0),
                        "right" => (10.0, 0.0),
                        "up" => (0.0, -10.0),
                        "down" => (0.0, 10.0),
                        _ => return,
                    };
                    change(
                        DockTreeEvent::FloatingChanged {
                            stack: stack.clone(),
                            bounds: gesture.request(gpui::point(px(x), px(y)), bounds.get()),
                            finished: true,
                        },
                        window,
                        cx,
                    );
                    cx.stop_propagation();
                });
                control =
                    control.child(crate::interaction::on_pointer_cancel(move |window, cx| {
                        if held.take().is_some() {
                            handler(
                                DockTreeEvent::FloatingCancelled { stack: id.clone() },
                                window,
                                cx,
                            );
                        }
                    }));
            }
            frame = frame.child(control);
            if !resize {
                frame = frame.child(div().flex_1().min_h_0().child(self.stack_element(
                    &tile.stack,
                    None,
                    window,
                    cx,
                )));
            }
        }
        frame.into_any_element()
    }
}

#[derive(Clone, Copy)]
struct FloatingGesture {
    position: Point<Pixels>,
    bounds: Bounds<f32>,
    resize: bool,
    min: (f32, f32),
}

impl FloatingGesture {
    fn request(self, position: Point<Pixels>, viewport: Bounds<Pixels>) -> Bounds<f32> {
        let width = f32::from(viewport.size.width).max(1.0);
        let height = f32::from(viewport.size.height).max(1.0);
        let delta = position - self.position;
        let mut b = self.bounds;
        if self.resize {
            let max_w = 1.0 - b.origin.x;
            let max_h = 1.0 - b.origin.y;
            b.size.width = (b.size.width + f32::from(delta.x) / width).clamp(
                (self.min.0 / width).min(max_w).max(f32::MIN_POSITIVE),
                max_w,
            );
            b.size.height = (b.size.height + f32::from(delta.y) / height).clamp(
                (self.min.1 / height).min(max_h).max(f32::MIN_POSITIVE),
                max_h,
            );
        } else {
            b.origin.x = (b.origin.x + f32::from(delta.x) / width).clamp(0.0, 1.0 - b.size.width);
            b.origin.y = (b.origin.y + f32::from(delta.y) / height).clamp(0.0, 1.0 - b.size.height);
        }
        b
    }
}

fn validate_floating(
    topology: &DockTopology,
    floating: &[FloatingDock],
) -> Result<(), DockRecordError> {
    let records = topology.to_records();
    DockTopology::from_records(&records)?;
    let mut ids: HashSet<_> = records.iter().map(|record| record.id.clone()).collect();
    let mut panels: HashSet<_> = topology
        .stacks()
        .iter()
        .flat_map(|stack| stack.panels.iter().cloned())
        .collect();
    for tile in floating {
        if !ids.insert(tile.stack.id.clone()) {
            return Err(DockRecordError::DuplicateId(tile.stack.id.clone()));
        }
        for panel in &tile.stack.panels {
            if !panels.insert(panel.clone()) {
                return Err(DockRecordError::DuplicatePanel(panel.clone()));
            }
        }
    }
    Ok(())
}

fn before_in(panels: &[SharedString], position: &DropPosition) -> Option<SharedString> {
    let anchor = position.anchor();
    match position {
        DropPosition::Before(_) => Some(anchor.clone()),
        DropPosition::After(_) => panels
            .iter()
            .position(|id| id == anchor)
            .and_then(|at| panels.get(at + 1))
            .cloned(),
        DropPosition::Into(_) => None,
    }
}

fn initial(title: &SharedString) -> SharedString {
    SharedString::from(
        title
            .chars()
            .next()
            .map(|first| first.to_uppercase().to_string())
            .unwrap_or_default(),
    )
}
