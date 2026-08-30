//! Recursive, persistable dock groups over the shared split, tab, and drag
//! primitives.
//!
//! [`DockTopology`] is caller-owned data. [`DockTree`] projects its stacks
//! through [`SplitTree`], uses [`Tabs`] for group ordering, and reports every
//! selection, move, edge split, resize, and collapse request as a
//! [`DockTreeEvent`]. It never edits the topology. An empty stack remains a
//! real drop target, so moving its last panel away does not make the place
//! impossible to restore.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px, relative,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, TypeScale};

use crate::foundation::{Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt};
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
            panels: Vec::new(),
            disabled: false,
            on_event: None,
        }
    }

    pub fn panel(mut self, panel: DockPanel) -> Self {
        self.panels.push(panel);
        self
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

    fn owns_prefix(&self) -> String {
        format!("{}.", self.ident.as_str())
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
            let prefix = self.owns_prefix();
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
                .accepts(move |item: &DragItem, _| item.source.starts_with(&prefix))
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
                .type_scale(&theme, TypeScale::Caption)
                .text_color(theme.colors.text_faint)
                .child(cx.strings().text(StringKey::DockEmptyStack))
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
        let prefix = self.owns_prefix();
        let destination = stack.id.clone();
        dnd::drop_target(
            frame.children(indicator),
            RowTarget {
                surface,
                id,
                index: 0,
                allow_into: true,
                axis: DropAxis::Vertical,
                accepts: Rc::new(move |item: &DragItem, _| item.source.starts_with(&prefix)),
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
        let prefix = self.owns_prefix();
        let destination = stack.id.clone();
        dnd::drop_target(
            zone,
            RowTarget {
                surface,
                id: target_id,
                index: 0,
                allow_into: true,
                axis: DropAxis::Vertical,
                accepts: Rc::new(move |item: &DragItem, _| item.source.starts_with(&prefix)),
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
                        .when(selected, |element| element.bg(theme.colors.selected))
                        .when(actionable, |element| {
                            element
                                .cursor_pointer()
                                .tab_index(0)
                                .pressable(cx)
                                .hover(|style| style.bg(theme.colors.hover))
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
        div()
            .id(self.ident.element_id())
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().colors.canvas)
            .child(tree)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group).value(
                    self.topology
                        .stacks()
                        .iter()
                        .map(|stack| stack.panels.len())
                        .sum::<usize>()
                        .to_string(),
                ),
            )
    }
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
