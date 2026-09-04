//! One step of a run, drawn as a card on a graph canvas.
//!
//! A node reports four things and invents none of them: what it is, what it is
//! doing now, how it ended, and what it cost. The cost figures are the
//! caller's strings, because a component that formatted a token count would be
//! deciding a product question — thousands separators, units, rounding — on
//! behalf of every host that ever draws one.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use web_time::Instant;

use gpui::{
    AnyElement, App, Bounds, BoxShadow, FontWeight, Hsla, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window, div, linear_color_stop, linear_gradient_stops, point, prelude::FluentBuilder,
    px, size,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, ColorChoice, Elevation, Radius, SemanticWash, Surface, Theme, Variant,
};

use crate::foundation::{FocusRing, Ident, Pressable, Selectable, StyledExt};
use crate::layout::measure;
use crate::motion::{Activity, MotionPolicy, MotionRole, MotionSpec, ResolvedMotion, keyed};
use crate::overlay::{Glass, GlassPreset, Tooltipped};
use crate::strings::ActiveNumbers;

use super::composite_id;
use super::edge::PortSide;

/// The default width of a node, in pixels.
///
/// Nodes on one canvas share a width so the columns of a graph line up and the
/// eye can compare two steps without measuring them.
pub const NODE_WIDTH: f32 = 216.0;

/// The shape of a thumbnail whose caller did not state another one.
const DEFAULT_THUMBNAIL_RATIO: f32 = 16.0 / 9.0;

/// How much of the top edge an indeterminate sweep occupies at once.
const INDETERMINATE_SWEEP: f32 = 0.3;
/// Flex shrink weight of the kind word against the title's 1: large enough
/// that the word gives up all of its width before the title loses any.
const KIND_YIELDS_FIRST: f32 = 100.0;

/// Whether a port receives or produces data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortDirection {
    #[default]
    Input,
    Output,
}

impl PortDirection {
    pub fn name(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

/// What a port carries, in the caller's taxonomy.
///
/// A type is a colour and, optionally, a glyph. The colour is the one fact a
/// reader uses to tell at a glance which sockets can be joined and which wire
/// carries what: a connection inherits the colour of the port it leaves. The
/// id is the caller's own vocabulary for the type — `"image"`, `"tensor"`,
/// `"text"` — and is published on the port, never invented here.
#[derive(Debug, Clone, PartialEq)]
pub struct PortType {
    id: SharedString,
    glyph: Option<Icon>,
    color: ColorChoice,
}

impl PortType {
    pub fn new(id: impl Into<SharedString>, color: impl Into<ColorChoice>) -> Self {
        Self {
            id: id.into(),
            glyph: None,
            color: color.into(),
        }
    }

    /// Seats a glyph inside the port ring, for a type whose colour alone is
    /// not enough to tell it from its neighbours.
    pub fn glyph(mut self, glyph: Icon) -> Self {
        self.glyph = Some(glyph);
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn icon(&self) -> Option<Icon> {
        self.glyph
    }

    pub fn color(&self) -> &ColorChoice {
        &self.color
    }

    /// The one paint that stands for this type on a ring and along a wire.
    ///
    /// It resolves through the shared [`Variant::Light`] tier, so a teal port
    /// here, a teal wire leaving it, and a teal chip elsewhere are the same
    /// teal, readable on both appearances.
    pub(crate) fn tint(&self, theme: &Theme) -> Hsla {
        theme.variant_colors(Variant::Light, &self.color).text
    }
}

/// A typed connection point on a [`GraphNode`].
///
/// Port ids must be unique within their node. They are caller-owned identity,
/// while labels are the caller-owned words shown for that identity.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphPort {
    id: SharedString,
    label: SharedString,
    direction: PortDirection,
    side: PortSide,
    port_type: Option<PortType>,
}

impl GraphPort {
    pub fn input(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            direction: PortDirection::Input,
            side: PortSide::Left,
            port_type: None,
        }
    }

    pub fn output(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            direction: PortDirection::Output,
            side: PortSide::Right,
            port_type: None,
        }
    }

    pub fn side(mut self, side: PortSide) -> Self {
        self.side = side;
        self
    }

    /// States what this port carries. The ring takes the type's colour and
    /// glyph, and a wire leaving an output of this type takes its colour.
    pub fn typed(mut self, port_type: PortType) -> Self {
        self.port_type = Some(port_type);
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    pub fn direction(&self) -> PortDirection {
        self.direction
    }

    /// Returns the side selected for this port.
    ///
    /// Named distinctly from the [`GraphPort::side`] builder because Rust
    /// does not overload methods by argument count.
    pub fn port_side(&self) -> PortSide {
        self.side
    }

    pub fn port_type(&self) -> Option<&PortType> {
        self.port_type.as_ref()
    }

    /// Whether this port is seated in the card's own rows rather than on the
    /// card's top or bottom edge, where there is no row to seat it in.
    pub(crate) fn seated_in_row(&self) -> bool {
        matches!(self.side, PortSide::Left | PortSide::Right)
    }
}

/// The measurement cell id for where one port's row landed inside its card.
pub(crate) fn port_measure_id(node: &str, port: &str) -> SharedString {
    composite_id("port-measure", &[node, port])
}

/// Screen-space values derived from world-space theme values in one place.
#[derive(Debug, Clone, Copy, PartialEq)]
struct NodeMetrics {
    /// The zoom these values were scaled by, for the few that scale here.
    scale: f32,
    width: f32,
    height: Option<f32>,
    padding: f32,
    /// The vertical inset of the header band, tighter than the body's so the
    /// name reads as a title bar rather than as the first row of content.
    header_padding: f32,
    gap: f32,
    figure_gap: f32,
    label_size: f32,
    label_height: f32,
    caption_size: f32,
    caption_height: f32,
    icon_size: f32,
    badge: f32,
    /// The state dot's diameter.
    mark: f32,
    /// The thickness of the progress bar along the top edge.
    progress: f32,
    /// The height of one port row, which is what a wire aims at.
    row_height: f32,
    /// How far a port row's text stands in from the card edge: past the
    /// half of the ring that sits inside the card, plus a gap.
    port_inset: f32,
    radius: f32,
}

impl NodeMetrics {
    fn new(theme: &gpui_kit_theme::Theme, width: f32, zoom: f32, height: Option<f32>) -> Self {
        let scale = if zoom.is_finite() && zoom > 0.0 {
            zoom
        } else {
            1.0
        };
        let scaled = |value: f32| value * scale;
        Self {
            scale,
            width: scaled(width),
            height: height.map(scaled),
            padding: scaled(theme.spacing.sm),
            header_padding: scaled(theme.spacing.xs),
            gap: scaled(theme.spacing.xs),
            figure_gap: scaled(theme.spacing.sm),
            label_size: scaled(theme.typography.label.size),
            label_height: scaled(theme.typography.label.line_height),
            caption_size: scaled(theme.typography.caption.size),
            caption_height: scaled(theme.typography.caption.line_height),
            icon_size: scaled(theme.control.sm.icon_size),
            badge: scaled(theme.control.xs.height),
            mark: scaled(theme.measures.status_mark),
            progress: scaled(theme.measures.node_progress),
            row_height: scaled(theme.typography.caption.line_height),
            port_inset: scaled(theme.measures.node_port / 2.0 + theme.spacing.xs),
            // A node is a card, so it takes the card role: the same rounding
            // the group box around it and every other card in the library
            // already read. Bubble is the dialog and message step, and a board
            // of cards rounded one step past the box enclosing them is the
            // inconsistency a reader sees before they read anything.
            radius: scaled(theme.radius(Radius::Card)),
        }
    }
}

/// The exact execution state a step reports.
///
/// Queued, waiting, blocked, refused, failed, cancelled, and unavailable are
/// separate answers. Keeping the full vocabulary here prevents a run adapter
/// from painting a host refusal as a failure or a wait as an unreached step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeState {
    /// Not reached yet.
    #[default]
    Pending,
    Idle,
    Queued,
    Starting,
    Running,
    Waiting,
    Blocked,
    Succeeded,
    Partial,
    Failed,
    /// The host declined to run it. Shown as a refusal, never as a failure and
    /// never as an empty step.
    Refused,
    Cancelling,
    Cancelled,
    TimedOut,
    Unavailable,
}

impl NodeState {
    pub fn color(self, theme: &gpui_kit_theme::Theme) -> Hsla {
        match self {
            Self::Pending | Self::Idle | Self::Cancelled | Self::Unavailable => {
                theme.colors.text_faint
            }
            Self::Queued | Self::Starting => theme.colors.info,
            Self::Running => theme.colors.accent,
            Self::Succeeded => theme.colors.success,
            Self::Waiting | Self::Partial | Self::Refused | Self::Cancelling => {
                theme.colors.warning
            }
            Self::Blocked | Self::Failed | Self::TimedOut => theme.colors.danger,
        }
    }

    /// What the node publishes as its value.
    pub(crate) fn value(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Idle => "idle",
            Self::Queued => "queued",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Blocked => "blocked",
            Self::Succeeded => "succeeded",
            Self::Partial => "partial",
            Self::Failed => "failed",
            Self::Refused => "refused",
            Self::Cancelling => "cancelling",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed-out",
            Self::Unavailable => "unavailable",
        }
    }

    pub(crate) fn is_busy(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Starting | Self::Running | Self::Cancelling
        )
    }

    fn is_invalid(self) -> bool {
        matches!(self, Self::Blocked | Self::Failed | Self::TimedOut)
    }

    /// The ambient state paint, if this state needs to reach past the card.
    ///
    /// This vocabulary is deliberately smaller than the semantic state list:
    /// the glyph and semantic value carry exact identity, while the aura lets
    /// a reader scan for live, attention, successful handoff, and danger.
    fn aura_color(self, theme: &Theme) -> Option<Hsla> {
        match self {
            Self::Running | Self::Starting => Some(theme.colors.node.aura_active),
            Self::Queued | Self::Waiting | Self::Partial | Self::Refused | Self::Cancelling => {
                Some(theme.colors.node.aura_attention)
            }
            Self::Blocked | Self::Failed | Self::TimedOut => Some(theme.colors.node.aura_danger),
            // Success is a one-shot handoff, not a permanent live signal.
            Self::Succeeded | Self::Pending | Self::Idle | Self::Cancelled | Self::Unavailable => {
                None
            }
        }
    }

    /// Whether the state dot breathes: the step is doing something now, and
    /// a still dot would say it had stopped.
    fn breathes(self) -> bool {
        matches!(self, Self::Starting | Self::Running | Self::Cancelling)
    }
}

/// How far a step has got, when it says.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Progress {
    /// Work is underway and the step cannot say how much is left.
    Indeterminate,
    /// The fraction done, from 0 to 1.
    Fraction(f32),
}

/// One figure a step reports about itself, such as a token count or an elapsed
/// time. Both halves are the caller's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeMetric {
    pub label: SharedString,
    pub value: SharedString,
}

impl NodeMetric {
    pub fn new(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

/// How much a step changed, in lines.
///
/// Kept apart from [`NodeMetric`] because the two halves are coloured against
/// each other, and a reader who sees green and red beside each other is
/// entitled to assume they mean added and removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Diff {
    pub added: usize,
    pub removed: usize,
}

impl Diff {
    pub fn new(added: usize, removed: usize) -> Self {
        Self { added, removed }
    }

    pub fn is_empty(self) -> bool {
        self.added == 0 && self.removed == 0
    }
}

/// About how wide a run of text will come out, before anything is laid out.
///
/// Wide scripts take about one em per character and Latin about half of one,
/// which is the difference between one row of figures and two. This is only
/// ever an opening estimate for geometry that must exist before a card has
/// been measured; the real measurement replaces it one frame later.
fn text_advance(text: &str, size: f32) -> f32 {
    text.chars()
        .map(|character| if character.is_ascii() { 0.55 } else { 1.0 })
        .sum::<f32>()
        * size
}

/// How many rows a wrapping strip of these widths needs.
fn wrapped_rows(widths: &[f32], available: f32, gap: f32) -> usize {
    if widths.is_empty() {
        return 0;
    }
    if available <= 0.0 {
        return widths.len();
    }
    let mut rows = 1;
    let mut used = 0.0;
    for width in widths {
        if used > 0.0 && used + gap + width > available {
            rows += 1;
            used = *width;
        } else if used > 0.0 {
            used += gap + width;
        } else {
            used = *width;
        }
    }
    rows
}

pub(crate) type ClickHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A step of a run, as a card on the canvas.
#[derive(IntoElement)]
pub struct GraphNode {
    ident: Ident,
    title: SharedString,
    icon: Option<Icon>,
    thumbnail: Option<AnyElement>,
    thumbnail_ratio: f32,
    /// What the step is doing now, for a step that is doing something.
    action: Option<SharedString>,
    state: NodeState,
    /// What kind of step this is, in the caller's own terms.
    category: Option<ColorChoice>,
    /// The word for that kind, when the caller has one.
    kind: Option<SharedString>,
    metrics: Vec<NodeMetric>,
    ports: Vec<GraphPort>,
    diff: Option<Diff>,
    status: Option<SharedString>,
    progress: Option<Progress>,
    /// Caller-owned content seated below the ports: a prompt, a preview, a
    /// control. The card lays it out and keeps its hands off it.
    content: Vec<AnyElement>,
    selected: bool,
    active_glass: GlassPreset,
    width: f32,
    display_zoom: f32,
    declared_height: Option<f32>,
    pointer_click: bool,
    compact: bool,
    on_click: Option<ClickHandler>,
    on_delete: Option<ClickHandler>,
}

impl std::fmt::Debug for GraphNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GraphNode")
            .field("ident", &self.ident)
            .field("title", &self.title)
            .field("has_thumbnail", &self.thumbnail.is_some())
            .field("state", &self.state)
            .field("metrics", &self.metrics.len())
            .field("ports", &self.ports.len())
            .field("content", &self.content.len())
            .finish_non_exhaustive()
    }
}

impl GraphNode {
    pub fn new(ident: impl Into<Ident>, title: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            title: title.into(),
            icon: None,
            thumbnail: None,
            thumbnail_ratio: DEFAULT_THUMBNAIL_RATIO,
            action: None,
            state: NodeState::default(),
            category: None,
            kind: None,
            metrics: Vec::new(),
            ports: Vec::new(),
            diff: None,
            status: None,
            progress: None,
            content: Vec::new(),
            selected: false,
            active_glass: GlassPreset::Frosted,
            width: NODE_WIDTH,
            display_zoom: 1.0,
            declared_height: None,
            pointer_click: true,
            compact: false,
            on_click: None,
            on_delete: None,
        }
    }

    /// Seats a caller-owned category glyph in the leading badge.
    ///
    /// The node resolves its wash from [`GraphNode::color`] and never infers a
    /// category from the glyph. A host that has no category icon leaves the
    /// seat absent rather than receiving a generic substitute.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Supplies the caller-owned visual preview for this node.
    ///
    /// The slot fills the card's content width and keeps a 16:9 ratio unless
    /// [`GraphNode::thumbnail_ratio`] states another one. The element may be
    /// an image, a video still, a waveform, or any other visual the caller can
    /// render; this component does not fetch or decode it.
    pub fn thumbnail(mut self, thumbnail: impl IntoElement) -> Self {
        self.thumbnail = Some(thumbnail.into_any_element());
        self
    }

    /// Sets the thumbnail width divided by its height.
    pub fn thumbnail_ratio(mut self, ratio: f32) -> Self {
        if ratio.is_finite() && ratio > 0.0 {
            self.thumbnail_ratio = ratio;
        }
        self
    }

    /// What the step is doing right now, in the caller's words.
    ///
    /// Shown only while the step is busy: a finished step's last action is a
    /// record, and a record belongs in the step's detail, not on a card
    /// whose job is to say where the run is now.
    pub fn action(mut self, action: impl Into<SharedString>) -> Self {
        self.action = Some(action.into());
        self
    }

    pub fn state(mut self, state: NodeState) -> Self {
        self.state = state;
        self
    }

    /// What kind of step this is, painted as the node's header and its edge
    /// stripe.
    ///
    /// Category is the caller's taxonomy, not this component's: a graph of
    /// sources, transforms and sinks and a graph of agents, tools and
    /// approvals both need their kinds told apart at a glance, and neither
    /// list belongs in a UI kit. It resolves through the shared
    /// [`Variant::Light`] tier, so a teal node here and a teal chip elsewhere
    /// are the same teal.
    ///
    /// This is deliberately separate from [`GraphNode::state`]. What a step
    /// *is* does not change while it runs, and a node that changed colour on
    /// failure would lose the identity a reader was using to find it.
    pub fn color(mut self, category: impl Into<ColorChoice>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// The word for what this node is — "image", "review", "deploy" — set
    /// quietly beside the title.
    ///
    /// It is a word and not a chip: the badge already carries the category's
    /// colour, and a wash behind the word would say the kind twice on every
    /// card. A node with a colour and no badge keeps the wash on the card,
    /// because then the wash is the only thing saying it.
    pub fn kind(mut self, kind: impl Into<SharedString>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    /// One figure the step reports. The value is what the card shows; the
    /// label names it on hover and in the semantic tree, so a card carrying
    /// three figures is three numbers wide rather than three sentences.
    pub fn metric(
        mut self,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        self.metrics.push(NodeMetric::new(label, value));
        self
    }

    pub fn metrics(mut self, metrics: impl IntoIterator<Item = NodeMetric>) -> Self {
        self.metrics.extend(metrics);
        self
    }

    pub fn port(mut self, port: GraphPort) -> Self {
        self.ports.push(port);
        self
    }

    pub fn ports(mut self, ports: impl IntoIterator<Item = GraphPort>) -> Self {
        self.ports.extend(ports);
        self
    }

    /// What the step changed. An empty diff is not shown, because "nothing
    /// changed" and "no diff was reported" are different claims and only the
    /// caller knows which one it has.
    pub fn diff(mut self, diff: Diff) -> Self {
        self.diff = Some(diff);
        self
    }

    /// The caller's own words for the step's state, published on the state
    /// dot's help and in the semantic tree.
    ///
    /// [`GraphNode::state`] owns the semantic state, colour and motion, and
    /// the dot is what the card shows for it; these words are caller-owned
    /// because products use different operational vocabulary for the same
    /// state, and they are not painted on the card because the dot has
    /// already said it.
    pub fn status(mut self, status: impl Into<SharedString>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// How far the step has got, as a bar along the card's top edge.
    ///
    /// `Some(fraction)` fills that much of the edge. `None` says the step is
    /// working without knowing how much is left, and the bar sweeps while the
    /// step is busy. A step that succeeds fills the bar and lets it go; one
    /// that fails keeps the bar where it stopped, in the failure's colour,
    /// because how far it got is part of what went wrong.
    pub fn progress(mut self, fraction: Option<f32>) -> Self {
        self.progress = Some(match fraction {
            Some(fraction) if fraction.is_finite() => Progress::Fraction(fraction.clamp(0.0, 1.0)),
            _ => Progress::Indeterminate,
        });
        self
    }

    /// Seats caller-owned content below the ports.
    ///
    /// The card lays the content out at its own width and keeps its hands
    /// off it: pointer and keyboard that land on the content belong to what
    /// the caller mounted, not to the card's activation or the canvas's drag.
    /// A prompt field, a preview, a slider, and a button are all content.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.content.push(child.into_any_element());
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.content
            .extend(children.into_iter().map(IntoElement::into_any_element));
        self
    }

    /// Chooses the shared glass preset used while the node is running,
    /// selected, or under the pointer.
    ///
    /// Resting cards remain the zero-snapshot pseudo-glass material. The
    /// promoted state uses [`Glass`] itself, so blur, refraction, fallback and
    /// renderer admission stay one framework contract. Frosted is the default.
    pub fn active_glass(mut self, preset: GlassPreset) -> Self {
        self.active_glass = preset;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    pub(crate) fn ident(&self) -> &Ident {
        &self.ident
    }

    pub(crate) fn node_width(&self) -> f32 {
        self.width
    }

    pub(crate) fn node_state(&self) -> NodeState {
        self.state
    }

    pub(crate) fn node_category(&self) -> Option<&ColorChoice> {
        self.category.as_ref()
    }

    /// The one colour that stands for this node away from the card itself.
    ///
    /// An overview and a stripe answer the same question, so they resolve it
    /// the same way: the category if the caller gave one, and otherwise how
    /// the step is doing, which every node reports.
    pub(crate) fn node_tint(&self, theme: &gpui_kit_theme::Theme) -> Hsla {
        match self.node_category() {
            Some(category) => theme.variant_colors(Variant::Light, category).text,
            None => self.state.color(theme),
        }
    }

    pub(crate) fn node_selected(&self) -> bool {
        self.selected
    }

    pub(crate) fn graph_ports(&self) -> &[GraphPort] {
        &self.ports
    }

    /// The ports seated in the card's rows, in row order, one pair per row:
    /// the left-side port and the right-side port that share it.
    fn port_rows(&self) -> Vec<(Option<&GraphPort>, Option<&GraphPort>)> {
        let left: Vec<&GraphPort> = self
            .ports
            .iter()
            .filter(|port| port.port_side() == PortSide::Left)
            .collect();
        let right: Vec<&GraphPort> = self
            .ports
            .iter()
            .filter(|port| port.port_side() == PortSide::Right)
            .collect();
        (0..left.len().max(right.len()))
            .map(|row| (left.get(row).copied(), right.get(row).copied()))
            .collect()
    }

    pub(crate) fn click_handler(&self) -> Option<ClickHandler> {
        self.on_click.clone()
    }

    /// Adds the keyboard actions owned by a graph while leaving pointer
    /// click-versus-drag arbitration on the graph's outer card.
    pub(crate) fn graph_handlers(
        mut self,
        on_activate: Option<ClickHandler>,
        on_delete: Option<ClickHandler>,
    ) -> Self {
        self.on_click = on_activate;
        self.on_delete = on_delete;
        self
    }

    /// Configures this card for graph display. Dimensions remain logical world
    /// values until render, so graph routing and card layout use one scale.
    pub(crate) fn display_at(mut self, zoom: f32, declared_height: Option<f32>) -> Self {
        self.display_zoom = if zoom.is_finite() && zoom > 0.0 {
            zoom
        } else {
            1.0
        };
        self.declared_height = declared_height.filter(|height| height.is_finite() && *height > 0.0);
        self
    }

    /// Leaves keyboard activation on the node while an owning canvas
    /// arbitrates pointer click versus drag on its stable outer surface.
    pub(crate) fn pointer_click(mut self, enabled: bool) -> Self {
        self.pointer_click = enabled;
        self
    }

    /// Drops ports-adjacent detail so a far-away card stays a title.
    pub(crate) fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    #[cfg(test)]
    pub(crate) fn logical_height(&self, theme: &gpui_kit_theme::Theme) -> f32 {
        self.declared_height
            .unwrap_or_else(|| self.measured_height(theme))
    }

    /// The width each figure in the strip will ask for before it wraps.
    fn figure_widths(&self, theme: &gpui_kit_theme::Theme) -> Vec<f32> {
        let size = theme.typography.caption.size;
        let inner = theme.spacing.xs / 2.0;
        let mut widths: Vec<f32> = self
            .metrics
            .iter()
            .map(|metric| text_advance(&metric.value, size))
            .collect();
        if let Some(diff) = self.diff.filter(|diff| !diff.is_empty()) {
            let counted =
                |value: usize| (value.to_string().chars().count() + 1) as f32 * size * 0.55;
            widths.push(counted(diff.added) + inner + counted(diff.removed));
        }
        widths
    }

    /// How tall the card will come out, from the rows it actually has.
    ///
    /// Edges are geometry and need a box before the card has been laid out,
    /// and the node is the only thing that knows how many rows it carries. A
    /// graph-wide constant would leave every connection to a step with no
    /// metrics entering at a different place from every connection to a step
    /// with three, and an edge that misses the card it joins is the one
    /// detail a reader will read as meaningful.
    pub(crate) fn measured_height(&self, theme: &gpui_kit_theme::Theme) -> f32 {
        let title = if self.icon.is_some() {
            theme
                .typography
                .label
                .line_height
                .max(theme.control.xs.height)
        } else {
            theme.typography.label.line_height
        };
        let header = title + theme.spacing.xs * 2.0;
        if self.compact {
            return header;
        }
        // Port rows sit in their own zone under the header, one caption line
        // each, and a wire aims at the middle of its row. Content the caller
        // mounted has no estimate at all: only the measurement one frame
        // later knows how tall a prompt field or a preview came out.
        let port_rows = self.port_rows().len();
        let ports = (port_rows > 0).then(|| {
            theme.typography.caption.line_height * port_rows as f32
                + theme.spacing.xs * port_rows.saturating_sub(1) as f32
                + theme.spacing.xs * 2.0
        });
        let mut rows = Vec::new();
        if self.thumbnail.is_some() {
            let content = self.width - theme.spacing.sm * 2.0;
            rows.push(content.max(0.0) / self.thumbnail_ratio);
        }
        if self.action.is_some() && self.state.is_busy() {
            rows.push(theme.typography.caption.line_height);
        }
        // The figure strip wraps, so a card carrying three figures on a narrow
        // node is two rows tall. An estimate that always answered one row
        // would put every edge into such a card above the socket it joins,
        // and would leave the card's own last row outside the box it is
        // clipped to.
        let figures = self.figure_widths(theme);
        if !figures.is_empty() {
            let content = (self.width - theme.spacing.sm * 2.0).max(0.0);
            let lines = wrapped_rows(&figures, content, theme.spacing.sm);
            rows.push(
                theme.typography.caption.line_height * lines as f32
                    + theme.spacing.sm * lines.saturating_sub(1) as f32,
            );
        }
        let body = (!rows.is_empty()).then(|| {
            let gaps = theme.spacing.xs * (rows.len() - 1) as f32;
            theme.spacing.sm * 2.0 + rows.iter().sum::<f32>() + gaps
        });
        header + ports.unwrap_or(0.0) + body.unwrap_or(0.0)
    }
}

impl Selectable for GraphNode {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

/// What a card was showing and what it is showing now, kept between frames so
/// a state change can be watched rather than only noticed afterwards.
#[derive(Debug, Clone, Copy, PartialEq)]
struct NodePaint {
    mark: Hsla,
    aura: Hsla,
    aura_alpha: f32,
}

impl NodePaint {
    fn for_state(state: NodeState, theme: &Theme) -> Self {
        let mark = state.color(theme);
        let aura = state.aura_color(theme).unwrap_or(mark);
        Self {
            mark,
            aura,
            aura_alpha: state
                .aura_color(theme)
                .map_or(0.0, |_| theme.effects.node_aura_resting_alpha),
        }
    }

    fn mix(self, to: Self, amount: f32, theme: &Theme) -> Self {
        Self {
            mark: theme.mix(self.mark, to.mark, amount),
            aura: theme.mix(self.aura, to.aura, amount),
            aura_alpha: self.aura_alpha + (to.aura_alpha - self.aura_alpha) * amount,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct StateFade {
    state: NodeState,
    from: Option<NodePaint>,
    to: Option<NodePaint>,
    /// When the crossover began, or `None` while the card is settled.
    started: Option<Instant>,
    /// Whether this card has been drawn before.
    ///
    /// A canvas opening onto a finished run draws it. Without this every card
    /// would cross over from the default state on its first frame, so a page
    /// of succeeded steps would fade up out of grey — which is a claim that
    /// they all just finished.
    drawn: bool,
    /// A successful handoff flashes once only when it was observed happening.
    succeeded_at: Option<Instant>,
}

impl StateFade {
    fn at(&self, now: Instant, spec: MotionSpec, theme: &Theme) -> NodePaint {
        let to = self.to.expect("a drawn state fade has a destination");
        let (Some(from), Some(started)) = (self.from, self.started) else {
            return to;
        };
        let span = spec.total().as_secs_f32().max(f32::EPSILON);
        let raw = (now.duration_since(started).as_secs_f32() / span).clamp(0.0, 1.0);
        if raw <= 0.0 {
            return from;
        }
        if raw >= 1.0 {
            return to;
        }
        from.mix(to, spec.progress(raw), theme)
    }

    /// Returns the visible paint, whether its crossover is live, and the
    /// remaining successful-handoff flash from 1 to 0.
    fn show(
        &mut self,
        state: NodeState,
        target: NodePaint,
        now: Instant,
        change: ResolvedMotion,
        feedback: ResolvedMotion,
        theme: &Theme,
    ) -> (NodePaint, bool, f32) {
        if !self.drawn {
            self.state = state;
            self.from = Some(target);
            self.to = Some(target);
            self.drawn = true;
            return (target, false, 0.0);
        }
        if self.state != state {
            let visible = self.at(now, change.spec(), theme);
            self.state = state;
            self.from = Some(if change.animates() { visible } else { target });
            self.to = Some(target);
            self.started = change.animates().then_some(now);
            self.succeeded_at =
                (state == NodeState::Succeeded && feedback.animates()).then_some(now);
        } else if self.to != Some(target) {
            // A theme change is not a node-state event.
            self.from = Some(target);
            self.to = Some(target);
            self.started = None;
        }
        let visible = self.at(now, change.spec(), theme);
        let crossing = self.started.is_some_and(|started| {
            now.duration_since(started).as_secs_f32()
                < change.spec().total().as_secs_f32().max(f32::EPSILON)
        });
        if !crossing {
            self.from = self.to;
            self.started = None;
        }
        let settle = self.succeeded_at.map_or(0.0, |started| {
            let span = feedback.spec().total().as_secs_f32().max(f32::EPSILON);
            let raw = (now.duration_since(started).as_secs_f32() / span).clamp(0.0, 1.0);
            1.0 - feedback.spec().progress(raw)
        });
        if settle <= 0.0 {
            self.succeeded_at = None;
        }
        (visible, crossing, settle)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct AuraClock {
    started: Option<Instant>,
}

/// Visual state local to one node material. The caller owns selection and
/// execution; hover is the only transient fact the material keeps itself.
#[derive(Debug, Clone, Copy, Default)]
struct NodeMaterialState {
    hovered: bool,
}

/// What one frame of a card's state looks like: the paint, how far the
/// success settle has reached, and where the live signal is in its breath.
#[derive(Debug, Clone, Copy)]
struct StateShow {
    paint: NodePaint,
    /// How much further than at rest the aura reaches, from the settle.
    aura_expansion: f32,
    /// The remaining successful-handoff flash, from 1 down to 0.
    settle: f32,
    /// The live signal's breath from 0 to 1, while the state breathes.
    breath: Option<f32>,
}

impl GraphNode {
    /// The paint and glow strength this card is showing, partway between the
    /// state it had and the state it has.
    ///
    /// Returns the settled answer whenever nothing is crossing: a card whose
    /// state has not changed, a reader who asked for reduced motion, and a
    /// card whose first frame this is all get the same values the hard cut
    /// gave.
    fn state_paint(&self, theme: &Theme, window: &mut Window, cx: &mut App) -> StateShow {
        let target = NodePaint::for_state(self.state, theme);
        let change = MotionPolicy::resolve(MotionRole::StateChange, cx);
        let feedback = MotionPolicy::resolve(MotionRole::Feedback, cx);
        let now = cx.background_executor().now();
        let fade = keyed::slot::<StateFade>(
            &self.ident.child("state").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let (mut paint, crossing, settle) = fade
            .borrow_mut()
            .show(self.state, target, now, change, feedback, theme);
        if crossing || settle > 0.0 {
            window.request_animation_frame();
        }

        // The live signal is one breath shared by the dot and the aura: the
        // dot breathes whenever the step is doing something, and the aura
        // breathes with it while the step runs.
        let clock = keyed::slot::<AuraClock>(
            &self.ident.child("aura").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let mut breath = None;
        if self.state.breathes() {
            let signal = MotionPolicy::resolve(MotionRole::Activity(Activity::Signaling), cx);
            if signal.animates() {
                let started = *clock.borrow_mut().started.get_or_insert(now);
                let phase = (now.duration_since(started).as_secs_f32()
                    / signal.spec().total().as_secs_f32().max(f32::EPSILON))
                .rem_euclid(1.0);
                let half = if phase < 0.5 {
                    phase * 2.0
                } else {
                    (1.0 - phase) * 2.0
                };
                let progress = signal.spec().progress(half);
                breath = Some(progress);
                if self.state == NodeState::Running {
                    paint.aura_alpha = theme.effects.node_aura_pulse_floor_alpha
                        + (theme.effects.node_aura_pulse_peak_alpha
                            - theme.effects.node_aura_pulse_floor_alpha)
                            * progress;
                }
                window.request_animation_frame();
            } else {
                clock.borrow_mut().started = None;
                if self.state == NodeState::Running {
                    paint.aura_alpha = theme.effects.node_aura_resting_alpha;
                }
            }
        } else {
            clock.borrow_mut().started = None;
        }

        // The successful handoff is an overlay on the crossing paint. Its
        // strength and reach contract together, then disappear completely.
        if settle > 0.0 {
            paint.aura = theme.mix(paint.aura, theme.colors.node.aura_success, settle);
            paint.aura_alpha = paint
                .aura_alpha
                .max(theme.effects.node_aura_settle_peak_alpha * settle);
        }
        StateShow {
            paint,
            aura_expansion: theme.effects.node_aura_settle_expansion * settle,
            settle,
            breath,
        }
    }
}

impl RenderOnce for GraphNode {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        // The card's semantic id, built once: every part of the card that
        // reports itself names this as its parent.
        let node_id = self.ident.semantic_id();
        let material_state = keyed::slot::<NodeMaterialState>(
            &self.ident.child("material").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let promoted =
            self.selected || self.state == NodeState::Running || material_state.borrow().hovered;
        // A state change is the one thing a reader watching a canvas is
        // watching for, and it used to be a hard cut: a node went from running
        // to failed between two frames, so anybody who blinked saw a failed
        // node and never saw it fail. The colour and the glow cross over
        // instead, on the same state-change timing every control in the
        // library answers a local change with.
        //
        // The crossfade is perceptual, so a run that goes from accent to
        // danger passes through the colours between them rather than through
        // the mud two gamma-encoded paints average to.
        let StateShow {
            paint,
            aura_expansion,
            settle,
            breath,
        } = self.state_paint(&theme, window, cx);
        let metrics = NodeMetrics::new(&theme, self.width, self.display_zoom, self.declared_height);

        // A category resolves through the same tier vocabulary as every other
        // coloured surface, so the wash behind a node's name and the wash
        // behind a chip of that colour are one decision made once.
        let identity = self
            .category
            .as_ref()
            .map(|category| theme.variant_colors(Variant::Light, category));

        let badge = self.icon.map(|glyph| {
            let (background, foreground) = identity.map_or(
                (
                    theme.color_wash(paint.mark, SemanticWash::Faint),
                    paint.mark,
                ),
                |identity| (identity.background, identity.text),
            );
            div()
                .flex_none()
                .size(px(metrics.badge))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme.radius(Radius::Control) * metrics.scale))
                .bg(background)
                .child(
                    icon(glyph)
                        .size(px(metrics.icon_size))
                        .text_color(foreground),
                )
        });

        // The word for the kind, set quietly after the title. A canvas is
        // read by scanning for one kind among many, and the badge's colour
        // already answers that; the word is there for the reader who has
        // found the card and wants it confirmed.
        // When the header runs out of room the word yields before the
        // title does: the title is what the card is, the word only what
        // sort of thing it is, and the badge already says that.
        let kind = self.kind.clone().map(|kind| {
            div()
                .min_w_0()
                .flex_shrink(KIND_YIELDS_FIRST)
                .truncate()
                .text_size(px(metrics.caption_size))
                .line_height(px(metrics.label_height))
                .font_weight(FontWeight(theme.typography.caption.weight))
                .text_color(theme.colors.text_muted)
                .child(kind)
        });

        // The state is one dot. It takes the state's colour, breathes while
        // the step is doing something, and carries the caller's words for the
        // state as help — so a board of a dozen cards is scanned by colour and
        // motion, and the words are there for whoever reaches for them. The
        // dot closes the row rather than opening it: what this node *is*
        // leads, and what it is *doing* answers.
        // The card's own semantic node carries the state as its value and
        // the status words as its description, so the dot registers nothing
        // of its own; it only becomes a stateful element when there are
        // words for a tooltip to show.
        let mark = div()
            .flex_none()
            .ml_auto()
            .size(px(metrics.mark))
            .rounded_full()
            .bg(paint.mark)
            .when(self.state.aura_color(&theme).is_some(), |dot| {
                dot.shadow(theme.glow(paint.mark))
            })
            .when_some(breath, |dot, breath| {
                dot.opacity(
                    theme.effects.node_aura_pulse_floor_alpha
                        + (1.0 - theme.effects.node_aura_pulse_floor_alpha) * breath,
                )
            })
            .map(|dot| match self.status.clone() {
                Some(status) => {
                    let mark_ident = self.ident.child("mark");
                    dot.id(mark_ident.element_id())
                        .tip(mark_ident, status)
                        .into_any_element()
                }
                None => dot.into_any_element(),
            });

        let header = div()
            .row()
            .w_full()
            .items_center()
            .gap(px(metrics.gap))
            .px(px(metrics.padding))
            .py(px(metrics.header_padding))
            .children(badge)
            .child(
                div()
                    .min_w_0()
                    .text_size(px(metrics.label_size))
                    .line_height(px(metrics.label_height))
                    .font_weight(FontWeight(theme.typography.strong.weight))
                    .text_color(theme.colors.text)
                    .truncate()
                    .child(self.title.clone()),
            )
            .children(kind)
            .child(mark);

        // How far the step has got, along the top edge. A known fraction
        // fills that much of the edge; unknown progress sweeps while the step
        // is busy; a success fills the edge and lets it go on the same settle
        // the aura uses, so the two say "done" together.
        let progress = self.progress.and_then(|progress| {
            let (fraction, sweep, alpha) = match (progress, self.state) {
                (_, NodeState::Succeeded) => (settle > 0.0).then_some((1.0, None, settle))?,
                (Progress::Fraction(fraction), _) => (fraction, None, 1.0),
                (Progress::Indeterminate, state) if state.is_busy() => {
                    let sweep = MotionPolicy::resolve(MotionRole::Activity(Activity::Working), cx);
                    if sweep.animates() {
                        let now = cx.background_executor().now();
                        let clock = keyed::slot::<AuraClock>(
                            &self.ident.child("progress").semantic_id(),
                            window.window_handle().window_id(),
                            cx,
                        );
                        let started = *clock.borrow_mut().started.get_or_insert(now);
                        let phase = (now.duration_since(started).as_secs_f32()
                            / sweep.spec().total().as_secs_f32().max(f32::EPSILON))
                        .rem_euclid(1.0);
                        window.request_animation_frame();
                        (INDETERMINATE_SWEEP, Some(sweep.spec().progress(phase)), 1.0)
                    } else {
                        (INDETERMINATE_SWEEP, Some(0.5), 1.0)
                    }
                }
                (Progress::Indeterminate, _) => return None,
            };
            // The bar runs between the card's corners rather than across
            // them, so its ends are never clipped by the rounding and the
            // rest of the edge reads as the track it fills.
            let track = (metrics.width - metrics.radius * 2.0).max(0.0);
            let width = track * fraction;
            let left = metrics.radius + sweep.map_or(0.0, |phase| (track - width) * phase);
            Some(
                div()
                    .absolute()
                    .top_0()
                    .left(px(left))
                    .w(px(width))
                    .h(px(metrics.progress))
                    .rounded_full()
                    .bg(paint.mark.opacity(alpha))
                    .semantic_in(cx, {
                        let spec = NodeSpec::new(
                            self.ident.child("progress").semantic_id(),
                            Role::Progress,
                        )
                        .parent(node_id.clone())
                        .busy(self.state.is_busy());
                        if sweep.is_none() {
                            spec.range(0.0, 1.0, fraction)
                        } else {
                            spec
                        }
                    }),
            )
        });

        // The ports, one row per pair, each name inside the card beside the
        // socket it belongs to. A wire aims at the middle of its row, and the
        // row says where it landed so the graph can aim there rather than at
        // an even division of the card's height.
        let seen_rows: Rc<RefCell<Vec<(SharedString, f32, f32)>>> = Rc::default();
        // Each port row reports where it sat; the outer wrapper records that
        // as an offset from the card's top, in graph units, so the graph can
        // read it back with the node's placement and aim a wire at the row.
        let cells: Vec<(SharedString, Rc<Cell<Bounds<Pixels>>>)> = self
            .port_rows()
            .into_iter()
            .flat_map(|(left, right)| [left, right])
            .flatten()
            .map(|port| {
                (
                    port.id().clone(),
                    measure::cell(&port_measure_id(&node_id, port.id()), window, cx),
                )
            })
            .collect();
        let zoom = self.display_zoom.max(f32::EPSILON);
        let rows = if self.compact {
            Vec::new()
        } else {
            self.port_rows()
        };
        let port_rows: Vec<AnyElement> = rows
            .into_iter()
            .map(|(left, right)| {
                let socket = |port: &GraphPort, trailing: bool| {
                    // The row carries the name and nothing else: the ring
                    // on the edge beside it already shows the type's glyph
                    // in the type's colour, and a second copy in the row
                    // made every port say its type twice.
                    let name = div()
                        .min_w_0()
                        .truncate()
                        .text_color(theme.colors.text_muted)
                        .child(port.label().clone());
                    div()
                        .row()
                        .min_w_0()
                        .items_center()
                        .when(trailing, |cell| cell.justify_end())
                        .child(name)
                        .semantic_in(cx, {
                            let spec = NodeSpec::new(
                                composite_id("port-name", &[node_id.as_ref(), port.id().as_ref()]),
                                Role::Text,
                            )
                            .parent(node_id.clone())
                            .text(port.label().clone());
                            match port.port_type() {
                                Some(port_type) => spec.value(port_type.id().clone()),
                                None => spec,
                            }
                        })
                };
                let ids: Vec<SharedString> = [left, right]
                    .into_iter()
                    .flatten()
                    .map(|port| port.id().clone())
                    .collect();
                let seen = Rc::clone(&seen_rows);
                div()
                    .row()
                    .w_full()
                    .h(px(metrics.row_height))
                    .items_center()
                    .justify_between()
                    .gap(px(metrics.gap))
                    .text_size(px(metrics.caption_size))
                    .line_height(px(metrics.row_height))
                    .font_weight(FontWeight(theme.typography.caption.weight))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .children(left.map(|port| socket(port, false))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .children(right.map(|port| socket(port, true))),
                    )
                    .on_children_prepainted(move |bounds, _, _| {
                        // The row's extent is whichever cell has something
                        // in it: an empty cell on the other side is flat.
                        let Some((top, bottom)) = bounds
                            .iter()
                            .filter(|cell| cell.size.height > px(0.0))
                            .map(|cell| (f32::from(cell.top()), f32::from(cell.bottom())))
                            .reduce(|(top, bottom), (t, b)| (top.min(t), bottom.max(b)))
                        else {
                            return;
                        };
                        let center = (top + bottom) / 2.0;
                        let height = bottom - top;
                        let mut seen = seen.borrow_mut();
                        for id in &ids {
                            seen.push((id.clone(), center, height));
                        }
                    })
                    .into_any_element()
            })
            .collect();
        let sockets = (!port_rows.is_empty()).then(|| {
            div()
                .w_full()
                .column()
                .gap(px(metrics.gap))
                .px(px(metrics.padding.max(metrics.port_inset)))
                .py(px(metrics.gap))
                .children(port_rows)
        });

        // What the step is doing, while it is doing it.
        let action = self
            .action
            .clone()
            .filter(|_| self.state.is_busy())
            .map(|action| {
                div()
                    .w_full()
                    .text_size(px(metrics.caption_size))
                    .line_height(px(metrics.caption_height))
                    .font_weight(FontWeight(theme.typography.caption.weight))
                    .text_color(theme.colors.text_muted)
                    .truncate()
                    .child(action)
            });

        // The figures are values; each one's name is help and semantics. A
        // card carrying three figures is three numbers wide rather than
        // three sentences, and the reader who wants to know which is which
        // reaches for one.
        let mut figures: Vec<AnyElement> = self
            .metrics
            .iter()
            .map(|metric| {
                let figure_ident = self.ident.child("metric").child(metric.label.clone());
                div()
                    .id(figure_ident.element_id())
                    .text_color(theme.colors.text_muted)
                    .child(metric.value.clone())
                    .tip(figure_ident.clone(), metric.label.clone())
                    .semantic_in(
                        cx,
                        NodeSpec::new(figure_ident.semantic_id(), Role::Text)
                            .parent(node_id.clone())
                            .text(metric.value.clone())
                            .description(metric.label.clone()),
                    )
                    .into_any_element()
            })
            .collect();

        if let Some(diff) = self.diff.filter(|diff| !diff.is_empty()) {
            figures.push(
                div()
                    .row()
                    .gap(px(metrics.gap / 2.0))
                    .child(
                        div()
                            .text_color(theme.colors.success)
                            .child(cx.numbers().positive_count(diff.added)),
                    )
                    .child(
                        div()
                            .text_color(theme.colors.danger)
                            .child(cx.numbers().negative_count(diff.removed)),
                    )
                    .into_any_element(),
            );
        }

        let strip = (!figures.is_empty()).then(|| {
            div()
                .row()
                .w_full()
                .flex_wrap()
                .gap(px(metrics.figure_gap))
                .text_size(px(metrics.caption_size))
                .line_height(px(metrics.caption_height))
                .font_weight(FontWeight(theme.typography.caption.weight))
                .children(figures)
        });

        let thumbnail = self.thumbnail.map(|thumbnail| {
            div()
                .w_full()
                .flex_none()
                .aspect_ratio(self.thumbnail_ratio)
                .overflow_hidden()
                .rounded(px(theme.radius(Radius::Control) * self.display_zoom))
                .child(thumbnail)
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("thumbnail").semantic_id(), Role::Image)
                        .parent(node_id.clone())
                        .text(self.title.clone()),
                )
        });

        // The caller's content is the caller's. Pointer and keys that land on
        // it stop here, before the card's activation and the canvas's drag
        // can read a click on a prompt field as picking the card up, or a
        // Backspace in it as deleting the step.
        let content = (!self.compact && !self.content.is_empty()).then(|| {
            div()
                .w_full()
                .column()
                .gap(px(metrics.gap))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_key_down(|_, _, cx| cx.stop_propagation())
                .children(self.content)
                .semantic_in(
                    cx,
                    NodeSpec::new(self.ident.child("content").semantic_id(), Role::Group)
                        .parent(node_id.clone()),
                )
        });

        // The body is a zone of its own rather than more rows under the title,
        // so the header's tint has something to stop against. A node with
        // nothing but a name has no body at all: an empty padded box below the
        // title would claim there is content that failed to arrive.
        let body = (!self.compact
            && (thumbnail.is_some() || action.is_some() || strip.is_some() || content.is_some()))
        .then(|| {
            div()
                .w_full()
                .column()
                .gap(px(metrics.gap))
                .p(px(metrics.padding))
                .children(thumbnail)
                .children(action)
                .children(strip)
                .children(content)
        });

        let stack = div()
            .flex_1()
            .min_w_0()
            .column()
            .child(header)
            .children(sockets)
            .children(body);

        // A full-bleed layer inside the card carries the card's own rounding.
        // `overflow_hidden` masks children to a rectangle, so a wash that
        // fills the card without saying it is round paints square corners over
        // the round ones underneath — which is why a plain node used to come
        // out with right angles while a promoted one, drawn by Glass, did not.
        // One radius, stated wherever something fills the card.
        let fill = || div().absolute().inset_0().rounded(px(metrics.radius));

        // The ordinary card is a zero-snapshot glass reading: a quiet tonal
        // plane with a soft top-light gradient and an inset specular cast.
        // Promoted cards drop the opaque plane so the shared Glass layer below
        // can show its real backdrop through the same highlight.
        let material = fill()
            .when(!promoted, |element| {
                element.bg(linear_gradient_stops(
                    180.0,
                    [
                        linear_color_stop(theme.colors.raised, 0.0),
                        linear_color_stop(theme.colors.panel, 1.0),
                    ],
                ))
            })
            .child(fill().bg(linear_gradient_stops(
                180.0,
                [
                    linear_color_stop(
                        theme.colors.white_fill.opacity(theme.effects.sheen_alpha),
                        0.0,
                    ),
                    linear_color_stop(gpui::transparent_black(), 0.42),
                    linear_color_stop(gpui::transparent_black(), 1.0),
                ],
            )));
        // A category with no badge or kind remains visible as a material wash
        // rather than the ornamental edge stripe nodes used to wear.
        let category_wash = identity
            .filter(|_| self.icon.is_none() && self.kind.is_none())
            .map(|identity| fill().bg(identity.background));
        // Selection is an outline in the accent rather than a wash: the wash
        // read as a colour of state, and a selected running node then said
        // two things with one tint. A ring says "this one" and leaves the
        // material to say what it is doing.
        let selection = self.selected.then(|| {
            fill()
                .border(px(theme.measures.node_edge_width * self.display_zoom))
                .border_color(theme.colors.accent)
        });
        let inset = BoxShadow {
            color: theme
                .colors
                .white_fill
                .opacity(theme.effects.glass_specular.max(theme.effects.sheen_alpha)),
            offset: point(px(0.0), px(theme.effects.glass_hairline)),
            blur_radius: px(theme.effects.glass_hairline * 2.0),
            spread_radius: px(-theme.effects.glass_hairline),
            style: gpui::ShadowStyle::Inset,
        };
        let mut card = div()
            .id(self.ident.element_id())
            .w(px(metrics.width))
            .when_some(metrics.height, |element, height| element.h(px(height)))
            .flex()
            .flex_row()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .rounded(px(metrics.radius))
            .overflow_hidden()
            .shadow(vec![inset])
            .child(material)
            .children(category_wash)
            .child(stack)
            .children(progress)
            .children(selection);

        let hover_state = Rc::clone(&material_state);
        card = card.on_hover(move |hovered, window, _| {
            let mut state = hover_state.borrow_mut();
            if state.hovered != *hovered {
                state.hovered = *hovered;
                window.refresh();
            }
        });

        // A node that takes a click is a button and a node that does not is a
        // group, so the role is decided before the spec is built rather than
        // patched afterwards.
        let role = if self.on_click.is_some() || self.on_delete.is_some() {
            Role::Button
        } else {
            Role::Group
        };
        let spec = NodeSpec::new(node_id.clone(), role)
            .text(self.title.clone())
            .value(self.state.value())
            .selected(self.selected)
            .busy(self.state.is_busy())
            .invalid(self.state.is_invalid());
        let spec = match self.status.clone() {
            Some(status) => spec.description(status),
            None => spec,
        };

        let card = if self.on_click.is_none() && self.on_delete.is_none() {
            card.semantic_in(cx, spec).into_any_element()
        } else {
            let mut card = card
                .cursor_pointer()
                .tab_index(0)
                .focus_ring_on(&theme, theme.colors.raised)
                .pressable(cx);
            if self.pointer_click
                && let Some(handler) = self.on_click.as_ref()
            {
                let click = Rc::clone(handler);
                card.interactivity()
                    .on_click(move |_, window, cx| click(window, cx));
            }
            let activate = self.on_click;
            let delete = self.on_delete;
            card.interactivity().on_key_down(move |event, window, cx| {
                match event.keystroke.key.as_str() {
                    "enter" | "space" => {
                        if let Some(handler) = &activate {
                            handler(window, cx);
                            cx.stop_propagation();
                        }
                    }
                    "backspace" | "delete" => {
                        if let Some(handler) = &delete {
                            handler(window, cx);
                            cx.stop_propagation();
                        }
                    }
                    _ => {}
                }
            });
            card.semantic_in(cx, spec).into_any_element()
        };

        // Elevation and state light are outside the material. A promoted
        // surface snapshots them before drawing its frost, which is what lets
        // the aura enter the card body instead of stopping at its perimeter.
        let mut shadows = theme.shadow(Elevation::Raised).to_vec();
        if paint.aura_alpha > 0.0 {
            shadows.push(BoxShadow {
                color: paint
                    .aura
                    .opacity(theme.effects.glow_alpha * paint.aura_alpha),
                offset: point(px(0.0), px(0.0)),
                blur_radius: px(theme.effects.glow_blur * (1.0 + aura_expansion)),
                spread_radius: px(theme.effects.glow_spread * (1.0 + aura_expansion)),
                style: gpui::ShadowStyle::Drop,
            });
        }
        let material = if promoted {
            Glass::new(self.ident.child("glass"))
                .surface(Surface::Raised)
                // The glass clips the blur, so it rounds by the card's own
                // measured radius rather than resolving the role a second
                // time: the card is scaled by the viewport's zoom and the role
                // is not.
                .radius_px(metrics.radius)
                .preset(self.active_glass)
                .child(card)
                .into_any_element()
        } else {
            card
        };
        div()
            .rounded(px(metrics.radius))
            .shadow(shadows)
            .child(material)
            // A card with no side ports has nothing to report and installs
            // no callback: a board of plain cards should cost no more per
            // card than it did before rows existed.
            .when(!cells.is_empty(), |wrapper| {
                wrapper.on_children_prepainted(move |bounds, window, _| {
                    let Some(card) = bounds.first() else {
                        return;
                    };
                    let top = f32::from(card.origin.y);
                    let seen = seen_rows.borrow();
                    for (id, cell) in &cells {
                        let Some((_, center, height)) = seen.iter().find(|(seen, _, _)| seen == id)
                        else {
                            continue;
                        };
                        measure::record(
                            cell,
                            Bounds {
                                origin: point(px(0.0), px((center - top) / zoom)),
                                size: size(px(0.0), px(height / zoom)),
                            },
                            window,
                        );
                    }
                })
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn theme() -> gpui_kit_theme::Theme {
        gpui_kit_theme::Theme::studio_dark()
    }

    fn show(
        fade: &mut StateFade,
        state: NodeState,
        now: Instant,
        theme: &Theme,
        animates: bool,
    ) -> (NodePaint, bool, f32) {
        fade.show(
            state,
            NodePaint::for_state(state, theme),
            now,
            MotionPolicy::resolve_for(MotionRole::StateChange, theme, !animates),
            MotionPolicy::resolve_for(MotionRole::Feedback, theme, !animates),
            theme,
        )
    }

    /// A state change was a hard cut: a node went from running to failed
    /// between two frames, so anybody who blinked saw a failed node and never
    /// saw it fail.
    #[test]
    fn a_state_change_crosses_over_rather_than_cutting() {
        let theme = theme();
        let start = Instant::now();
        let mut fade = StateFade::default();

        // The first frame is a drawing, not a change. A canvas opening onto a
        // finished run would otherwise fade every succeeded step up out of
        // grey, which claims they all just finished.
        let (paint, crossing, settle) = show(&mut fade, NodeState::Succeeded, start, &theme, true);
        assert_eq!(paint, NodePaint::for_state(NodeState::Succeeded, &theme));
        assert!(!crossing, "the first frame crossed over from nothing");
        assert_eq!(settle, 0.0, "the first frame replayed a success flash");

        let changed = start + Duration::from_millis(100);
        let (paint, crossing, _) = show(&mut fade, NodeState::Failed, changed, &theme, true);
        assert_eq!(paint, NodePaint::for_state(NodeState::Succeeded, &theme));
        assert!(crossing);

        let (paint, crossing, _) = show(
            &mut fade,
            NodeState::Failed,
            changed + Duration::from_millis(100),
            &theme,
            true,
        );
        assert!(crossing);
        assert_ne!(paint, NodePaint::for_state(NodeState::Succeeded, &theme));
        assert_ne!(paint, NodePaint::for_state(NodeState::Failed, &theme));

        let (paint, crossing, _) = show(
            &mut fade,
            NodeState::Failed,
            changed + Duration::from_millis(400),
            &theme,
            true,
        );
        assert_eq!(paint, NodePaint::for_state(NodeState::Failed, &theme));
        assert!(!crossing);
    }

    /// A card that changes twice quickly must not jump back to the first
    /// colour before setting off for the third.
    #[test]
    fn a_change_interrupted_partway_leaves_from_the_paint_on_screen() {
        let theme = theme();
        let start = Instant::now();
        let mut fade = StateFade::default();
        show(&mut fade, NodeState::Running, start, &theme, true);

        let first = start + Duration::from_millis(10);
        show(&mut fade, NodeState::Waiting, first, &theme, true);
        let second = first + Duration::from_millis(60);
        let (visible, _, _) = show(&mut fade, NodeState::Waiting, second, &theme, true);
        let (paint, crossing, _) = show(&mut fade, NodeState::Failed, second, &theme, true);
        assert_eq!(
            paint, visible,
            "the interrupted crossover restarted from the state it had already left"
        );
        assert_eq!(fade.from, Some(visible));
        assert!(crossing);

        // Once one has finished, the next starts from where it landed.
        show(
            &mut fade,
            NodeState::Failed,
            second + Duration::from_millis(300),
            &theme,
            true,
        );
        let third = second + Duration::from_millis(400);
        let (paint, crossing, settle) = show(&mut fade, NodeState::Succeeded, third, &theme, true);
        assert_eq!(paint, NodePaint::for_state(NodeState::Failed, &theme));
        assert!(crossing);
        assert_eq!(settle, 1.0);
    }

    #[test]
    fn reduced_motion_settles_state_and_success_without_a_timeline() {
        let theme = theme();
        let start = Instant::now();
        let mut fade = StateFade::default();
        show(&mut fade, NodeState::Running, start, &theme, false);
        let (paint, crossing, settle) = show(
            &mut fade,
            NodeState::Succeeded,
            start + Duration::from_millis(10),
            &theme,
            false,
        );
        assert_eq!(paint, NodePaint::for_state(NodeState::Succeeded, &theme));
        assert!(!crossing);
        assert_eq!(settle, 0.0);
    }

    /// Tone is intentionally shared by related states, so the semantic word
    /// remains the exact state rather than asking colour to carry identity.
    #[test]
    fn every_state_has_a_distinct_semantic_word() {
        let states = [
            NodeState::Pending,
            NodeState::Idle,
            NodeState::Queued,
            NodeState::Starting,
            NodeState::Running,
            NodeState::Waiting,
            NodeState::Blocked,
            NodeState::Succeeded,
            NodeState::Partial,
            NodeState::Failed,
            NodeState::Refused,
            NodeState::Cancelling,
            NodeState::Cancelled,
            NodeState::TimedOut,
            NodeState::Unavailable,
        ];
        for (index, state) in states.iter().enumerate() {
            for other in &states[index + 1..] {
                assert_ne!(state.value(), other.value(), "{state:?} {other:?}");
            }
        }
    }

    /// A refusal is the host declining, which is neither a failure nor an
    /// absence of work, and it may not be reported as either.
    #[test]
    fn a_refusal_is_not_a_failure() {
        let theme = theme();
        assert_ne!(
            NodeState::Refused.color(&theme),
            NodeState::Failed.color(&theme)
        );
        assert_eq!(NodeState::Refused.value(), "refused");
    }

    #[test]
    fn only_the_states_worth_scanning_for_reach_past_the_card() {
        let theme = theme();
        assert_eq!(
            NodeState::Running.aura_color(&theme),
            Some(theme.colors.node.aura_active)
        );
        assert_eq!(
            NodeState::Failed.aura_color(&theme),
            Some(theme.colors.node.aura_danger)
        );
        assert_eq!(
            NodeState::Refused.aura_color(&theme),
            Some(theme.colors.node.aura_attention)
        );
        assert!(NodeState::Pending.aura_color(&theme).is_none());
        assert!(NodeState::Succeeded.aura_color(&theme).is_none());
        assert!(NodeState::Running.breathes());
        assert!(NodeState::Starting.breathes());
        assert!(NodeState::Cancelling.breathes());
        assert!(!NodeState::Queued.breathes());
    }

    /// The mark is one dot in one colour per state, so the states a board
    /// tells apart at a glance have to be told apart by colour alone.
    #[test]
    fn states_a_reader_must_tell_apart_take_different_mark_colours() {
        let theme = theme();
        let mark = |state: NodeState| NodePaint::for_state(state, &theme).mark;
        assert_ne!(mark(NodeState::Running), mark(NodeState::Succeeded));
        assert_ne!(mark(NodeState::Running), mark(NodeState::Failed));
        assert_ne!(mark(NodeState::Succeeded), mark(NodeState::Failed));
        assert_ne!(mark(NodeState::Pending), mark(NodeState::Running));
        assert_ne!(mark(NodeState::Failed), mark(NodeState::Refused));
    }

    #[test]
    fn an_empty_diff_reports_itself_as_empty() {
        assert!(Diff::default().is_empty());
        assert!(!Diff::new(0, 3).is_empty());
        assert!(!Diff::new(3, 0).is_empty());
    }

    #[test]
    fn a_node_starts_pending_and_at_the_shared_width() {
        let node = GraphNode::new("run.plan", "Plan");
        assert_eq!(node.state, NodeState::Pending);
        assert_eq!(node.active_glass, GlassPreset::Frosted);
        assert_eq!(node.node_width(), NODE_WIDTH);
        assert_eq!(node.ident().as_str(), "run.plan");
    }

    #[test]
    fn anatomy_seats_are_opt_in_and_use_the_shared_material_axis() {
        let node = GraphNode::new("render", "Render")
            .icon(Icon::Image)
            .status("Rendering")
            .active_glass(GlassPreset::Lens);
        assert_eq!(node.icon, Some(Icon::Image));
        assert_eq!(node.status.as_deref(), Some("Rendering"));
        assert_eq!(node.active_glass, GlassPreset::Lens);
    }

    #[test]
    fn ports_have_directional_defaults_and_allow_side_override() {
        let input = GraphPort::input("source", "Source");
        assert_eq!(input.direction(), PortDirection::Input);
        assert_eq!(input.direction().name(), "input");
        assert_eq!(input.port_side(), PortSide::Left);

        let output = GraphPort::output("result", "Result").side(PortSide::Bottom);
        assert_eq!(output.direction(), PortDirection::Output);
        assert_eq!(output.direction().name(), "output");
        assert_eq!(output.port_side(), PortSide::Bottom);
    }

    #[test]
    fn node_port_builders_preserve_caller_identity_and_labels() {
        let node = GraphNode::new("transform", "Transform")
            .port(GraphPort::input("in", "Rows"))
            .ports([GraphPort::output("out", "Records")]);
        assert_eq!(node.graph_ports().len(), 2);
        assert_eq!(node.graph_ports()[0].id().as_ref(), "in");
        assert_eq!(node.graph_ports()[0].label().as_ref(), "Rows");
        assert_eq!(node.graph_ports()[1].id().as_ref(), "out");
    }

    #[test]
    fn declared_height_is_the_logical_geometry_contract() {
        let theme = theme();
        let node = GraphNode::new("step", "Step").display_at(2.0, Some(140.0));
        assert_eq!(node.logical_height(&theme), 140.0);
        let metrics = NodeMetrics::new(
            &theme,
            node.node_width(),
            node.display_zoom,
            node.declared_height,
        );
        assert_eq!(metrics.height, Some(280.0));
    }

    #[test]
    fn a_thumbnail_has_a_predictable_default_shape_in_graph_geometry() {
        let theme = theme();
        let plain = GraphNode::new("plain", "Plain").measured_height(&theme);
        let thumbnail = GraphNode::new("picture", "Picture")
            .thumbnail(div())
            .measured_height(&theme);
        // A node with nothing but a name is its header band alone. Giving it a
        // picture opens the body zone, so it gains that zone's padding as well
        // as the picture itself. Material separation never changes geometry.
        let expected = theme.spacing.sm * 2.0
            + (NODE_WIDTH - theme.spacing.sm * 2.0) / DEFAULT_THUMBNAIL_RATIO;
        assert!((thumbnail - plain - expected).abs() < 0.001);

        let square = GraphNode::new("square", "Square")
            .thumbnail(div())
            .thumbnail_ratio(1.0)
            .measured_height(&theme);
        assert!(square > thumbnail);
    }

    /// The strip wraps, so the box edges are routed into has to know that a
    /// card carrying many figures is more than one figure row tall.
    #[test]
    fn a_wrapping_figure_strip_makes_the_card_taller() {
        let theme = theme();
        let one = GraphNode::new("one", "One")
            .metric("Model", "A")
            .measured_height(&theme);
        // Figures are values only, so it takes long values to fill a row:
        // four short ones sit side by side and the card stays one row tall.
        let few = GraphNode::new("few", "Few")
            .metric("Model", "GPT Image")
            .metric("Ratio", "16:9")
            .metric("Duration", "12s")
            .metric("Seed", "118")
            .measured_height(&theme);
        assert_eq!(few, one, "{few} should match {one}");
        let many = GraphNode::new("many", "Many")
            .metric("Model", "gpt-image-1-high-fidelity")
            .metric("Prompt", "a lit interior at dusk, 35mm")
            .metric("Seed", "118443921")
            .measured_height(&theme);
        assert!(many > one, "{many} should exceed {one}");
    }

    /// Wide scripts are about twice the advance of Latin, and a strip measured
    /// as if they were the same width would under-report its own rows.
    #[test]
    fn wide_scripts_count_for_their_own_width() {
        let size = 12.0;
        assert!(text_advance("模型比例时长", size) > text_advance("model", size));
    }

    #[test]
    fn a_strip_wraps_only_when_the_row_is_really_full() {
        assert_eq!(wrapped_rows(&[], 100.0, 8.0), 0);
        assert_eq!(wrapped_rows(&[40.0, 40.0], 100.0, 8.0), 1);
        assert_eq!(wrapped_rows(&[60.0, 60.0], 100.0, 8.0), 2);
        // A single figure wider than the row still occupies exactly one row:
        // it overflows its own line rather than starting a second one.
        assert_eq!(wrapped_rows(&[400.0], 100.0, 8.0), 1);
    }

    #[test]
    fn scale_is_normalized_and_applied_to_all_layout_metrics() {
        let theme = theme();
        let normal = NodeMetrics::new(&theme, NODE_WIDTH, f32::NAN, Some(100.0));
        assert_eq!(normal.width, NODE_WIDTH);
        assert_eq!(normal.height, Some(100.0));

        let doubled = NodeMetrics::new(&theme, NODE_WIDTH, 2.0, Some(100.0));
        assert_eq!(doubled.width, NODE_WIDTH * 2.0);
        assert_eq!(doubled.height, Some(200.0));
        assert_eq!(doubled.padding, theme.spacing.sm * 2.0);
        assert_eq!(doubled.caption_size, theme.typography.caption.size * 2.0);
        assert_eq!(doubled.icon_size, theme.control.sm.icon_size * 2.0);
        assert_eq!(doubled.badge, theme.control.xs.height * 2.0);
        assert_eq!(doubled.mark, theme.measures.status_mark * 2.0);
        assert_eq!(doubled.progress, theme.measures.node_progress * 2.0);
        assert_eq!(doubled.radius, theme.radius(Radius::Card) * 2.0);
    }

    /// A node is a card and is rounded like one. The group box that encloses a
    /// run of nodes already reads the card role, so a node reading any other
    /// step would be visibly rounder or squarer than the box around it in
    /// every theme.
    #[test]
    fn a_node_is_rounded_by_the_card_role_in_every_theme() {
        for theme in [
            gpui_kit_theme::Theme::studio_dark(),
            gpui_kit_theme::Theme::studio_light(),
        ] {
            let metrics = NodeMetrics::new(&theme, NODE_WIDTH, 1.0, None);
            assert_eq!(metrics.radius, theme.radius(Radius::Card));
        }
    }
}
