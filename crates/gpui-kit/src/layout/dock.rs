//! Panels arranged in regions around a centre, the way a desktop tool is laid
//! out.
//!
//! Every fact the dock draws belongs to the caller: which panels a region
//! holds, which one is on top, whether a region is collapsed, and how much
//! room it takes. The dock reports what the typist asked for as a
//! [`DockEvent`] and moves nothing, so a host that refuses a move keeps the
//! arrangement that still holds.
//!
//! # One resize implementation
//!
//! Region sizes go through [`SplitTree`]: the dock builds a [`SplitLayout`]
//! from the regions that hold panels and hands it over, so a divider between
//! two regions is the same divider a caller would get from a plain split, with
//! the same minimums and the same published range.
//!
//! # Dragging a panel between regions
//!
//! A region's header is a [`Tabs`] strip, so dragging a panel is the drag
//! system in [`crate::interaction::dnd`] and nothing new. Dropping on another
//! region's tab reports [`DockEvent::PanelMoved`] naming the panel the moved
//! one should sit in front of; dropping on a region's body reports the same
//! move with `before: None`, which appends.
//!
//! # What the dock cannot do
//!
//! - A region holding no panels is not drawn, so it cannot be dropped onto.
//!   The last panel cannot be dragged out of a region and then back into it.
//! - A panel is drawn in exactly one region. There is no split within a
//!   region, and no floating panel.
//! - A collapsed region shows a rail. Picking a panel from the rail reports
//!   the selection and the request to expand, and applies neither.
//! - The header is a tab strip whatever the count: with one panel it reads as
//!   a title, which is what gives a lone panel something to drag by.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Theme, TypeScale};

use crate::foundation::{Disableable, FocusRing, Ident, Pressable, Sizable, StyledExt};
use crate::interaction::dnd::{self, DragItem, DropAxis, DropIntent, DropPosition, RowTarget};
use crate::layout::tree::{SplitChange, SplitLayout, SplitPaneSpec, SplitTree};
use crate::motion::{Flipping, flip};
use crate::navigation::tabs::{TabItem, Tabs};
use crate::overlay::Tooltipped;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// How wide a collapsed region's rail is, and how deep a collapsed bottom
/// region's is. The value occurs only here.
const RAIL: f32 = 44.0;

/// The share of its own split a region takes when the caller says nothing.
const DEFAULT_SHARE: f32 = 0.22;

/// The identities of the splits the dock builds, so a reported ratio can be
/// read back as the region it belongs to.
const BODY_SPLIT: &str = "dock.body";
const COLUMNS_SPLIT: &str = "dock.columns";
const ROOT_SPLIT: &str = "dock.root";

type EventHandler = Rc<dyn Fn(DockEvent, &mut Window, &mut App)>;

/// Where a panel sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DockRegion {
    Left,
    Centre,
    Right,
    Bottom,
}

impl DockRegion {
    pub const ALL: [DockRegion; 4] = [Self::Left, Self::Centre, Self::Right, Self::Bottom];

    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Centre => "centre",
            Self::Right => "right",
            Self::Bottom => "bottom",
        }
    }

    /// Whether the region's rail runs down the side rather than across.
    fn upright(self) -> bool {
        !matches!(self, Self::Bottom)
    }

    fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Centre => 1,
            Self::Right => 2,
            Self::Bottom => 3,
        }
    }
}

/// One panel: a name, a glyph, a count, and whatever the caller puts inside.
pub struct DockPanel {
    id: SharedString,
    title: SharedString,
    icon: Option<Icon>,
    badge: Option<SharedString>,
    unavailable: Option<SharedString>,
    /// A region draws its active panel once, and an `AnyElement` can be built
    /// exactly once, so the content is handed over rather than copied.
    content: RefCell<Option<AnyElement>>,
}

impl std::fmt::Debug for DockPanel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DockPanel")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("unavailable", &self.unavailable)
            .finish()
    }
}

impl DockPanel {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: None,
            badge: None,
            unavailable: None,
            content: RefCell::new(None),
        }
    }

    pub fn icon(mut self, glyph: Icon) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// A count shown beside the title, such as how many problems a panel holds.
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Why the host cannot show this panel right now.
    ///
    /// An unavailable panel keeps its tab and states the reason where its
    /// content would be. It does not disappear, because a panel that vanished
    /// would read as one the workspace never had.
    pub fn unavailable(mut self, reason: impl Into<SharedString>) -> Self {
        self.unavailable = Some(reason.into());
        self
    }

    pub fn content(self, content: impl IntoElement) -> Self {
        *self.content.borrow_mut() = Some(content.into_any_element());
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn title(&self) -> &SharedString {
        &self.title
    }

    pub fn is_unavailable(&self) -> bool {
        self.unavailable.is_some()
    }

    pub(crate) fn glyph(&self) -> Option<Icon> {
        self.icon
    }

    pub(crate) fn badge_text(&self) -> Option<SharedString> {
        self.badge.clone()
    }

    pub(crate) fn unavailable_reason(&self) -> Option<SharedString> {
        self.unavailable.clone()
    }

    pub(crate) fn take_content(&self) -> Option<AnyElement> {
        self.content.borrow_mut().take()
    }
}

/// What the dock reports. The caller decides what any of it means.
#[derive(Debug, Clone, PartialEq)]
pub enum DockEvent {
    /// A tab or a rail glyph was picked.
    PanelSelected {
        region: DockRegion,
        panel: SharedString,
    },
    /// A panel was dragged somewhere. Nothing has moved.
    PanelMoved {
        panel: SharedString,
        to_region: DockRegion,
        /// The panel the moved one should sit in front of, or `None` to put it
        /// last. Never an index: an index stops meaning anything the moment
        /// the host applies the move.
        before: Option<SharedString>,
    },
    /// A region was asked to collapse to its rail, or to come back.
    RegionCollapsed { region: DockRegion, collapsed: bool },
    /// A divider beside a region was moved. `ratio` is the share of its own
    /// split the region asked for.
    RegionResized { region: DockRegion, ratio: f32 },
}

#[derive(Default)]
struct Region {
    panels: Vec<DockPanel>,
    active: Option<SharedString>,
    collapsed: bool,
    share: Option<f32>,
    min: Option<f32>,
}

impl Region {
    fn active_panel(&self) -> Option<&DockPanel> {
        match &self.active {
            Some(id) => self.panels.iter().find(|panel| &panel.id == id),
            None => self.panels.first(),
        }
    }

    fn share(&self) -> f32 {
        self.share.unwrap_or(DEFAULT_SHARE).clamp(0.0, 1.0)
    }
}

/// Panels arranged in regions around a centre.
#[derive(IntoElement)]
pub struct Dock {
    ident: Ident,
    regions: [Region; 4],
    disabled: bool,
    on_event: Option<EventHandler>,
}

impl std::fmt::Debug for Dock {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Dock")
            .field("ident", &self.ident)
            .field(
                "panels",
                &self
                    .regions
                    .iter()
                    .map(|region| region.panels.len())
                    .collect::<Vec<_>>(),
            )
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_event.is_some())
            .finish()
    }
}

impl Dock {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            regions: Default::default(),
            disabled: false,
            on_event: None,
        }
    }

    pub fn panel(mut self, region: DockRegion, panel: DockPanel) -> Self {
        self.regions[region.index()].panels.push(panel);
        self
    }

    pub fn panels(
        mut self,
        region: DockRegion,
        panels: impl IntoIterator<Item = DockPanel>,
    ) -> Self {
        self.regions[region.index()].panels.extend(panels);
        self
    }

    /// Which panel the caller says is on top. Without one, the first is.
    pub fn active(mut self, region: DockRegion, panel: impl Into<SharedString>) -> Self {
        self.regions[region.index()].active = Some(panel.into());
        self
    }

    /// Draws the region as a rail of its panels rather than as a panel.
    pub fn collapsed(mut self, region: DockRegion, collapsed: bool) -> Self {
        self.regions[region.index()].collapsed = collapsed;
        self
    }

    /// How much of its own split the region takes, from 0 to 1.
    pub fn share(mut self, region: DockRegion, share: f32) -> Self {
        self.regions[region.index()].share = Some(share);
        self
    }

    /// The smallest the region may be dragged to, in pixels.
    pub fn min_size(mut self, region: DockRegion, min: f32) -> Self {
        self.regions[region.index()].min = Some(min);
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(DockEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }

    fn region(&self, region: DockRegion) -> &Region {
        &self.regions[region.index()]
    }

    fn occupied(&self, region: DockRegion) -> bool {
        !self.region(region).panels.is_empty()
    }

    /// A region as a leaf of the tree.
    ///
    /// The minimum is stated on the axis the region is resized along and
    /// nowhere else: a left panel that needs 176px of width does not thereby
    /// need 176px of height, and saying so would stop the bottom divider.
    fn leaf(&self, region: DockRegion) -> SplitLayout {
        let state = self.region(region);
        let min = state.min.unwrap_or(RAIL * 4.0);
        let spec = SplitPaneSpec::new(region.name())
            .rail(RAIL)
            .collapsed(state.collapsed);
        SplitLayout::leaf(match region {
            DockRegion::Left | DockRegion::Right => spec.min_width(min),
            DockRegion::Bottom => spec.min_height(min),
            // The centre is squeezed from both sides and from below.
            DockRegion::Centre => spec.min(min),
        })
    }

    /// The tree the regions that hold panels make up.
    ///
    /// Left and right sit either side of the centre, and the bottom runs under
    /// all three. A region with no panels is left out of the tree entirely
    /// rather than drawn as an empty box.
    fn layout(&self) -> SplitLayout {
        let mut tree = self.leaf(DockRegion::Centre);
        if self.occupied(DockRegion::Right) {
            tree = SplitLayout::horizontal(
                COLUMNS_SPLIT,
                1.0 - self.region(DockRegion::Right).share(),
                tree,
                self.leaf(DockRegion::Right),
            );
        }
        if self.occupied(DockRegion::Left) {
            tree = SplitLayout::horizontal(
                BODY_SPLIT,
                self.region(DockRegion::Left).share(),
                self.leaf(DockRegion::Left),
                tree,
            );
        }
        if self.occupied(DockRegion::Bottom) {
            tree = SplitLayout::vertical(
                ROOT_SPLIT,
                1.0 - self.region(DockRegion::Bottom).share(),
                tree,
                self.leaf(DockRegion::Bottom),
            );
        }
        tree
    }

    fn surface(&self, region: DockRegion) -> SharedString {
        self.ident.child(region.name()).child("tabs").semantic_id()
    }

    /// Whether a payload came from this dock, so a row dragged out of a list
    /// somewhere else is refused rather than accepted as a panel.
    fn owns_prefix(&self) -> String {
        format!("{}.", self.ident.as_str())
    }

    fn header(
        &self,
        region: DockRegion,
        theme: &Theme,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let state = self.region(region);
        let ident = self.ident.child(region.name());
        let active = state.active_panel().map(|panel| panel.id.clone());

        let mut tabs = Tabs::new(ident.child("tabs"))
            .small()
            .tabs(state.panels.iter().map(|panel| {
                let mut tab = TabItem::new(panel.id.clone(), panel.title.clone());
                if let Some(glyph) = panel.icon {
                    tab = tab.icon(glyph);
                }
                if let Some(badge) = panel.badge.clone() {
                    tab = tab.badge(badge);
                }
                tab
            }))
            .disabled(self.disabled);
        if let Some(active) = active {
            tabs = tabs.selected(active);
        }

        if let (false, Some(handler)) = (self.disabled, self.on_event.clone()) {
            let selected = Rc::clone(&handler);
            let prefix = self.owns_prefix();
            let panels: Vec<SharedString> =
                state.panels.iter().map(|panel| panel.id.clone()).collect();
            tabs = tabs
                .on_select(move |panel, window, cx| {
                    selected(DockEvent::PanelSelected { region, panel }, window, cx);
                })
                .reorderable(true)
                .accepts(move |item: &DragItem, _: &DropPosition| item.source.starts_with(&prefix))
                .on_reorder(move |intent, window, cx| {
                    handler(
                        DockEvent::PanelMoved {
                            panel: intent.item.id.clone(),
                            to_region: region,
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
            .filter(|_| !self.disabled)
            .map(|handler| {
                let button = ident.child("collapse");
                let name = cx.strings().text(StringKey::DockCollapseRegion);
                div()
                    .id(button.element_id())
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .mb(px(theme.borders.thick))
                    .size(px(theme.control.get(ControlSize::Sm).height))
                    .radius(theme, Radius::Control)
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .hover(|style| style.bg(theme.colors.hover))
                    .focus_ring(theme)
                    .child(
                        icon(Icon::Sidebar)
                            .size(px(theme.control.get(ControlSize::Sm).icon_size))
                            .text_color(theme.colors.text_muted),
                    )
                    .on_click(move |_, window, cx| {
                        handler(
                            DockEvent::RegionCollapsed {
                                region,
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
            .gap_token(theme, Space::Xs)
            .px_token(theme, Space::Xs)
            .bg(theme.colors.panel)
            .child(div().flex_1().min_w(px(0.0)).overflow_hidden().child(tabs))
            .children(collapse)
            .into_any_element()
    }

    /// The body of the region: the active panel, or why it cannot be shown.
    fn body(
        &self,
        region: DockRegion,
        theme: &Theme,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let Some(panel) = self.region(region).active_panel() else {
            return div().flex_1().into_any_element();
        };
        let ident = self.ident.child(region.name()).child(panel.id.as_ref());

        let content = match &panel.unavailable {
            // A refusal is drawn in place of the panel, not instead of it.
            Some(reason) => div()
                .column()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_token(theme, Space::Sm)
                .p_token(theme, Space::Lg)
                .child(
                    icon(Icon::CloseCircle)
                        .size(px(theme.measures.standalone_icon))
                        .text_color(theme.colors.warning),
                )
                .child(
                    div()
                        .type_scale(theme, TypeScale::Body)
                        .text_color(theme.colors.text)
                        .child(panel.title.clone()),
                )
                .child(
                    div()
                        .max_w(px(theme.measures.readable_width))
                        .text_align(gpui::TextAlign::Center)
                        .type_scale(theme, TypeScale::Caption)
                        .text_color(theme.colors.text_muted)
                        .child(reason.clone()),
                )
                .into_any_element(),
            None => div()
                .flex_1()
                .min_h(px(0.0))
                .overflow_hidden()
                .children(panel.content.borrow_mut().take())
                .into_any_element(),
        };

        let mut frame = div()
            .id(ident.element_id())
            .column()
            .flex_1()
            .min_h(px(0.0))
            .overflow_hidden()
            .child(content);

        if let (false, Some(handler)) = (self.disabled, self.on_event.clone()) {
            let surface = self.surface(region);
            let name = SharedString::from(region.name());
            let landing =
                dnd::surface_drag(&surface, window, cx).and_then(|drag| drag.indicator_for(&name));
            frame = frame.children(landing.map(|(position, accepted)| {
                dnd::indicator(&position, accepted, DropAxis::Vertical, cx)
            }));
            let prefix = self.owns_prefix();
            frame = dnd::drop_target(
                frame,
                RowTarget {
                    surface,
                    id: name,
                    index: 0,
                    allow_into: true,
                    axis: DropAxis::Vertical,
                    accepts: Rc::new(move |item: &DragItem, _: &DropPosition| {
                        item.source.starts_with(&prefix)
                    }),
                    on_drop: Rc::new(move |intent: &DropIntent, window, cx| {
                        handler(
                            DockEvent::PanelMoved {
                                panel: intent.item.id.clone(),
                                to_region: region,
                                before: None,
                            },
                            window,
                            cx,
                        );
                    }),
                },
            );
        }

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::TabPanel)
            .parent(self.ident.child(region.name()).semantic_id())
            .text(panel.title.clone())
            .invalid(panel.unavailable.is_some());
        if let Some(reason) = panel.unavailable.clone() {
            spec = spec.value(reason);
        }

        // A panel that changed regions lands in its new slot on the frame the
        // host applies the move; only the pixels take their time.
        let slide = flip(
            self.ident.child(panel.id.as_ref()).semantic_id(),
            window,
            cx,
        );
        frame
            .semantic_in(cx, spec)
            .flip(&slide, window, cx)
            .into_any_element()
    }

    /// The rail a collapsed region shows.
    ///
    /// Collapsing is a change of drawing, never of substance: every panel the
    /// region holds is still published by name, so nothing becomes
    /// unaddressable by being made narrow.
    fn rail(&self, region: DockRegion, theme: &Theme, cx: &mut App) -> AnyElement {
        let state = self.region(region);
        let ident = self.ident.child(region.name()).child("rail");
        let metrics = theme.control.get(ControlSize::Sm);
        let active = state.active_panel().map(|panel| panel.id.clone());
        let actionable = !self.disabled && self.on_event.is_some();

        let items: Vec<_> = state
            .panels
            .iter()
            .map(|panel| {
                let item = ident.child(panel.id.as_ref());
                let current = active.as_ref() == Some(&panel.id);
                let color = if current {
                    theme.colors.text
                } else {
                    theme.colors.text_muted
                };
                let mut glyph = div()
                    .id(item.element_id())
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(metrics.height))
                    .radius(theme, Radius::Control)
                    .when(current, |element| element.bg(theme.colors.selected))
                    .child(match panel.icon {
                        Some(glyph) => icon(glyph)
                            .size(px(metrics.icon_size))
                            .text_color(color)
                            .into_any_element(),
                        None => div()
                            .type_scale(theme, TypeScale::Caption)
                            .text_color(color)
                            .child(initial(&panel.title))
                            .into_any_element(),
                    })
                    .when(actionable, |element| {
                        element
                            .cursor_pointer()
                            .tab_index(0)
                            .pressable(cx)
                            .hover(|style| style.bg(theme.colors.hover))
                            .focus_ring(theme)
                    })
                    .tip(item.clone(), panel.title.clone());

                if let (true, Some(handler)) = (actionable, self.on_event.clone()) {
                    let id = panel.id.clone();
                    glyph = glyph.on_click(move |_, window, cx| {
                        // Picking from a rail is two requests, and the host
                        // judges each: show this panel, and give the region
                        // its room back.
                        handler(
                            DockEvent::PanelSelected {
                                region,
                                panel: id.clone(),
                            },
                            window,
                            cx,
                        );
                        handler(
                            DockEvent::RegionCollapsed {
                                region,
                                collapsed: false,
                            },
                            window,
                            cx,
                        );
                    });
                }

                glyph.semantic_in(
                    cx,
                    NodeSpec::new(item.semantic_id(), Role::Button)
                        .parent(ident.semantic_id())
                        .selected(current)
                        .disabled(!actionable)
                        .text(panel.title.clone()),
                )
            })
            .collect();

        let upright = region.upright();
        div()
            .flex()
            .when(upright, |rail| rail.flex_col().size_full())
            .when(!upright, |rail| rail.flex_row().size_full())
            .items_center()
            .gap_token(theme, Space::Xs)
            .p_token(theme, Space::Xs)
            .bg(theme.colors.panel)
            .children(items)
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::List)
                    .parent(self.ident.child(region.name()).semantic_id())
                    .expanded(false)
                    .value(cx.numbers().count(state.panels.len())),
            )
            .into_any_element()
    }

    fn region_element(
        &self,
        region: DockRegion,
        theme: &Theme,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let state = self.region(region);
        let ident = self.ident.child(region.name());
        let inner = if state.collapsed {
            self.rail(region, theme, cx)
        } else {
            div()
                .column()
                .size_full()
                .overflow_hidden()
                .child(self.header(region, theme, window, cx))
                .child(self.body(region, theme, window, cx))
                .into_any_element()
        };

        div()
            .column()
            .size_full()
            .overflow_hidden()
            .bg(if region == DockRegion::Centre {
                theme.colors.canvas
            } else {
                theme.colors.panel
            })
            .child(inner)
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Region)
                    .parent(self.ident.semantic_id())
                    .expanded(!state.collapsed)
                    .value(cx.numbers().count(state.panels.len())),
            )
            .into_any_element()
    }
}

/// The panel a drop lands in front of, given where it landed among `panels`.
fn before_in(panels: &[SharedString], position: &DropPosition) -> Option<SharedString> {
    let anchor = position.anchor();
    match position {
        DropPosition::Before(_) => Some(anchor.clone()),
        DropPosition::After(_) => panels
            .iter()
            .position(|id| id == anchor)
            .and_then(|at| panels.get(at + 1))
            .cloned(),
        // A drop on the region itself names no neighbour, so it appends.
        DropPosition::Into(_) => None,
    }
}

/// Which region a divider the tree reported belongs to.
fn region_change(change: &SplitChange) -> Option<DockEvent> {
    match change {
        SplitChange::Ratio { split, ratio } => {
            let (region, share) = match split.as_ref() {
                BODY_SPLIT => (DockRegion::Left, *ratio),
                COLUMNS_SPLIT => (DockRegion::Right, 1.0 - *ratio),
                ROOT_SPLIT => (DockRegion::Bottom, 1.0 - *ratio),
                _ => return None,
            };
            Some(DockEvent::RegionResized {
                region,
                ratio: share,
            })
        }
        SplitChange::Collapsed { pane, .. } => DockRegion::ALL
            .into_iter()
            .find(|region| region.name() == pane.as_ref())
            .map(|region| DockEvent::RegionCollapsed {
                region,
                collapsed: true,
            }),
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

impl Disableable for Dock {
    /// Freezes every panel header and rail. A frozen dock installs no handler.
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for Dock {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut tree = SplitTree::new(self.ident.child("layout")).layout(self.layout());

        for region in DockRegion::ALL {
            if self.occupied(region) {
                tree = tree.pane(
                    region.name(),
                    self.region_element(region, &theme, window, cx),
                );
            }
        }

        if let (false, Some(handler)) = (self.disabled, self.on_event.clone()) {
            tree = tree.on_change(move |change, window, cx| {
                if let Some(event) = region_change(&change) {
                    handler(event, window, cx);
                }
            });
        }

        div()
            .id(self.ident.element_id())
            .size_full()
            .overflow_hidden()
            .bg(theme.colors.canvas)
            .child(tree)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group).value(
                    self.regions
                        .iter()
                        .map(|region| region.panels.len())
                        .sum::<usize>()
                        .to_string(),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::SplitSide;

    #[test]
    fn a_region_with_no_panels_is_left_out_of_the_tree() {
        let dock = Dock::new("dock").panel(DockRegion::Centre, DockPanel::new("editor", "Editor"));
        let layout = dock.layout();
        let names: Vec<&str> = layout
            .panes()
            .iter()
            .map(|pane| pane.id().as_ref())
            .collect();
        assert_eq!(names, vec!["centre"]);
    }

    #[test]
    fn every_occupied_region_becomes_a_leaf() {
        let dock = Dock::new("dock")
            .panel(DockRegion::Left, DockPanel::new("files", "Files"))
            .panel(DockRegion::Centre, DockPanel::new("editor", "Editor"))
            .panel(DockRegion::Right, DockPanel::new("outline", "Outline"))
            .panel(DockRegion::Bottom, DockPanel::new("terminal", "Terminal"));
        let layout = dock.layout();
        let names: Vec<&str> = layout
            .panes()
            .iter()
            .map(|pane| pane.id().as_ref())
            .collect();
        assert_eq!(names, vec!["left", "centre", "right", "bottom"]);
    }

    #[test]
    fn a_ratio_reads_back_as_the_share_the_region_asked_for() {
        assert_eq!(
            region_change(&SplitChange::Ratio {
                split: COLUMNS_SPLIT.into(),
                ratio: 0.7,
            }),
            Some(DockEvent::RegionResized {
                region: DockRegion::Right,
                ratio: 0.3
            })
        );
        assert_eq!(
            region_change(&SplitChange::Ratio {
                split: BODY_SPLIT.into(),
                ratio: 0.3,
            }),
            Some(DockEvent::RegionResized {
                region: DockRegion::Left,
                ratio: 0.3
            })
        );
    }

    #[test]
    fn a_drop_after_the_last_tab_names_no_neighbour() {
        let panels: Vec<SharedString> = vec!["files".into(), "search".into()];
        assert_eq!(
            before_in(&panels, &DropPosition::Before("search".into())),
            Some(SharedString::from("search"))
        );
        assert_eq!(
            before_in(&panels, &DropPosition::After("files".into())),
            Some(SharedString::from("search"))
        );
        assert_eq!(
            before_in(&panels, &DropPosition::After("search".into())),
            None
        );
        assert_eq!(before_in(&panels, &DropPosition::Into("left".into())), None);
    }

    #[test]
    fn a_payload_from_somewhere_else_is_not_a_panel() {
        let prefix = Dock::new("dock").owns_prefix();
        assert!(
            DragItem::new("dock.left.tabs", "files", "Files")
                .source
                .starts_with(&prefix)
        );
        assert!(
            !DragItem::new("queue", "step-build", "Build")
                .source
                .starts_with(&prefix)
        );
    }

    #[test]
    fn collapsing_names_the_region_the_divider_stood_beside() {
        assert_eq!(
            region_change(&SplitChange::Collapsed {
                split: BODY_SPLIT.into(),
                side: SplitSide::Start,
                pane: "left".into(),
            }),
            Some(DockEvent::RegionCollapsed {
                region: DockRegion::Left,
                collapsed: true
            })
        );
    }
}
