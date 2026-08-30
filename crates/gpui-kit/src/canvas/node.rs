//! One step of a run, drawn as a card on a graph canvas.
//!
//! A node reports four things and invents none of them: what it is, what it is
//! doing now, how it ended, and what it cost. The cost figures are the
//! caller's strings, because a component that formatted a token count would be
//! deciding a product question — thousands separators, units, rounding — on
//! behalf of every host that ever draws one.

use std::rc::Rc;

use web_time::Instant;

use gpui::{
    AnyElement, App, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ColorChoice, Elevation, Radius, Surface, Theme, Variant};

use crate::foundation::{FocusRing, Ident, Pressable, Selectable, StyledExt};
use crate::motion;
use crate::motion::{Activity, MotionPolicy, MotionRole, MotionSpec, ResolvedMotion, keyed};
use crate::strings::ActiveNumbers;

use super::edge::PortSide;

/// The default width of a node, in pixels.
///
/// Nodes on one canvas share a width so the columns of a graph line up and the
/// eye can compare two steps without measuring them.
pub const NODE_WIDTH: f32 = 216.0;

/// The shape of a thumbnail whose caller did not state another one.
const DEFAULT_THUMBNAIL_RATIO: f32 = 16.0 / 9.0;

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

/// A typed connection point on a [`GraphNode`].
///
/// Port ids must be unique within their node. They are caller-owned identity,
/// while labels are the caller-owned words shown for that identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphPort {
    id: SharedString,
    label: SharedString,
    direction: PortDirection,
    side: PortSide,
}

impl GraphPort {
    pub fn input(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            direction: PortDirection::Input,
            side: PortSide::Left,
        }
    }

    pub fn output(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            direction: PortDirection::Output,
            side: PortSide::Right,
        }
    }

    pub fn side(mut self, side: PortSide) -> Self {
        self.side = side;
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
    /// The identity stripe down the reading edge.
    rail: f32,
    gap: f32,
    figure_gap: f32,
    label_size: f32,
    label_height: f32,
    caption_size: f32,
    caption_height: f32,
    icon_size: f32,
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
            rail: scaled(theme.effects.rail_width),
            gap: scaled(theme.spacing.xs),
            figure_gap: scaled(theme.spacing.sm),
            label_size: scaled(theme.typography.label.size),
            label_height: scaled(theme.typography.label.line_height),
            caption_size: scaled(theme.typography.caption.size),
            caption_height: scaled(theme.typography.caption.line_height),
            icon_size: scaled(theme.control.sm.icon_size),
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

    fn glyph(self) -> Option<Icon> {
        match self {
            Self::Pending | Self::Idle => None,
            Self::Queued | Self::Waiting => Some(Icon::Info),
            Self::Starting | Self::Running | Self::Cancelling => Some(Icon::Refresh),
            Self::Blocked | Self::Partial | Self::TimedOut => Some(Icon::Danger),
            Self::Succeeded => Some(Icon::Check),
            Self::Failed => Some(Icon::Close),
            Self::Refused => Some(Icon::Danger),
            Self::Cancelled => Some(Icon::Close),
            Self::Unavailable => Some(Icon::CloseCircle),
        }
    }

    /// What the node publishes as its value.
    fn value(self) -> &'static str {
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

    fn animates_mark(self) -> bool {
        matches!(self, Self::Starting | Self::Running | Self::Cancelling)
    }
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
    selected: bool,
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
            .finish_non_exhaustive()
    }
}

impl GraphNode {
    pub fn new(ident: impl Into<Ident>, title: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            title: title.into(),
            thumbnail: None,
            thumbnail_ratio: DEFAULT_THUMBNAIL_RATIO,
            action: None,
            state: NodeState::default(),
            category: None,
            kind: None,
            metrics: Vec::new(),
            ports: Vec::new(),
            diff: None,
            selected: false,
            width: NODE_WIDTH,
            display_zoom: 1.0,
            declared_height: None,
            pointer_click: true,
            compact: false,
            on_click: None,
            on_delete: None,
        }
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

    /// The word for what this node is — "image", "review", "deploy" — shown as
    /// a small chip before the title.
    ///
    /// This is where [`GraphNode::color`] lands when both are given, and it is
    /// the reason to give both: a wash behind two or three letters says the
    /// kind once, where a tinted title bar and a stripe down the card say it
    /// twice and turn a board of cards into a board of colour. A node with a
    /// colour and no word keeps the stripe, because then the stripe is the
    /// only thing saying it.
    pub fn kind(mut self, kind: impl Into<SharedString>) -> Self {
        self.kind = Some(kind.into());
        self
    }

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
            .map(|metric| {
                text_advance(&metric.label, size) + inner + text_advance(&metric.value, size)
            })
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
        let header = theme.typography.label.line_height + theme.spacing.xs * 2.0;
        if self.compact {
            return header;
        }
        let mut rows = Vec::new();
        if self.thumbnail.is_some() {
            // The stripe takes its width off the content before the padding
            // does, so a picture measured here is the width one actually gets.
            let content = self.width - theme.effects.rail_width - theme.spacing.sm * 2.0;
            rows.push(content.max(0.0) / self.thumbnail_ratio);
        }
        if self.action.is_some() {
            rows.push(theme.typography.caption.line_height);
        }
        // The figure strip wraps, so a card carrying three figures on a narrow
        // node is two rows tall. An estimate that always answered one row
        // would put every edge into such a card above the socket it joins,
        // and would leave the card's own last row outside the box it is
        // clipped to.
        let figures = self.figure_widths(theme);
        if !figures.is_empty() {
            let content = (self.width - theme.effects.rail_width - theme.spacing.sm * 2.0).max(0.0);
            let lines = wrapped_rows(&figures, content, theme.spacing.sm);
            rows.push(
                theme.typography.caption.line_height * lines as f32
                    + theme.spacing.sm * lines.saturating_sub(1) as f32,
            );
        }
        if rows.is_empty() {
            return header;
        }
        let gaps = theme.spacing.xs * (rows.len() - 1) as f32;
        header + theme.borders.hairline + theme.spacing.sm * 2.0 + rows.iter().sum::<f32>() + gaps
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

impl GraphNode {
    /// The paint and glow strength this card is showing, partway between the
    /// state it had and the state it has.
    ///
    /// Returns the settled answer whenever nothing is crossing: a card whose
    /// state has not changed, a reader who asked for reduced motion, and a
    /// card whose first frame this is all get the same values the hard cut
    /// gave.
    fn state_paint(&self, theme: &Theme, window: &mut Window, cx: &mut App) -> (NodePaint, f32) {
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

        let clock = keyed::slot::<AuraClock>(
            &self.ident.child("aura").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        if self.state == NodeState::Running {
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
                let breath = signal.spec().progress(half);
                paint.aura_alpha = theme.effects.node_aura_pulse_floor_alpha
                    + (theme.effects.node_aura_pulse_peak_alpha
                        - theme.effects.node_aura_pulse_floor_alpha)
                        * breath;
                window.request_animation_frame();
            } else {
                clock.borrow_mut().started = None;
                paint.aura_alpha = theme.effects.node_aura_resting_alpha;
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
        (paint, theme.effects.node_aura_settle_expansion * settle)
    }
}

impl RenderOnce for GraphNode {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
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
        let (paint, aura_expansion) = self.state_paint(&theme, window, cx);
        let metrics = NodeMetrics::new(&theme, self.width, self.display_zoom, self.declared_height);

        // The mark is the one part of a node that moves, and it moves because
        // the step is still running. It turns through the shared vocabulary,
        // so a running node and a running tool call turn at one rate.
        let mark = self.state.glyph().map(|glyph| {
            let element = icon(glyph)
                .size(px(metrics.icon_size))
                .text_color(paint.mark);
            if self.state.animates_mark() {
                motion::spin(element, self.ident.child("mark").element_id(), &theme, cx)
            } else {
                element.into_any_element()
            }
        });

        // A category resolves through the same tier vocabulary as every other
        // coloured surface, so the wash behind a node's name and the wash
        // behind a chip of that colour are one decision made once.
        let identity = self
            .category
            .as_ref()
            .map(|category| theme.variant_colors(Variant::Light, category));

        // The word for the kind, with the category's wash behind it. A canvas
        // is read by scanning for one kind among many, and a chip is the
        // smallest mark that answers that: tinting the whole title bar answers
        // it at ten times the size, and on a board of a dozen cards the tints
        // become the picture instead of the work.
        let kind = self.kind.clone().map(|kind| {
            div()
                .flex_none()
                .px(px(metrics.gap))
                .rounded(px(theme.radius(Radius::Small) * metrics.scale))
                .text_size(px(metrics.caption_size))
                .line_height(px(metrics.label_height))
                .font_weight(FontWeight(theme.typography.label.weight))
                .map(|element| match identity {
                    Some(identity) => element.bg(identity.background).text_color(identity.text),
                    None => element
                        .bg(theme.colors.node.label_wash)
                        .text_color(theme.colors.text_muted),
                })
                .child(kind)
        });

        let header = div()
            .row()
            .w_full()
            .gap(px(metrics.gap))
            .px(px(metrics.padding))
            .py(px(metrics.header_padding))
            .children(kind)
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .text_size(px(metrics.label_size))
                    .line_height(px(metrics.label_height))
                    .font_weight(FontWeight(theme.typography.label.weight))
                    .text_color(theme.colors.text)
                    .truncate()
                    .child(self.title.clone()),
            )
            // What this node *is* leads the row and what it is *doing* closes
            // it. Put the running mark first and every card starts with a
            // status, so a board of a dozen is scanned by state before it is
            // scanned by kind — which is backwards for the reader arranging
            // one, and it costs the name a mark's width on every card whether
            // or not there is anything to report.
            .children(mark);

        let action = self.action.clone().map(|action| {
            div()
                .w_full()
                .text_size(px(metrics.caption_size))
                .line_height(px(metrics.caption_height))
                .font_weight(FontWeight(theme.typography.caption.weight))
                .text_color(theme.colors.text_muted)
                .truncate()
                .child(action)
        });

        let mut figures: Vec<AnyElement> = self
            .metrics
            .iter()
            .map(|metric| {
                div()
                    .row()
                    .gap(px(metrics.gap / 2.0))
                    .child(
                        div()
                            .text_color(theme.colors.text_faint)
                            .child(metric.label.clone()),
                    )
                    .child(
                        div()
                            .text_color(theme.colors.text_muted)
                            .child(metric.value.clone()),
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
                        .parent(self.ident.semantic_id())
                        .text(self.title.clone()),
                )
        });

        // The body is a zone of its own rather than more rows under the title,
        // so the header's tint has something to stop against. A node with
        // nothing but a name has no body at all: an empty padded box below the
        // title would claim there is content that failed to arrive.
        let body = (!self.compact && (thumbnail.is_some() || action.is_some() || strip.is_some()))
            .then(|| {
                div()
                    .w_full()
                    .column()
                    .gap(px(metrics.gap))
                    .p(px(metrics.padding))
                    .children(thumbnail)
                    .children(action)
                    .children(strip)
            });

        // A category stripe runs the whole height, so a node that has scrolled
        // until only its edge is visible still says what it is. A node without
        // a category gets no neutral substitute: a stripe with no meaning is
        // decoration that invites the reader to search for one.
        //
        // It is drawn only where nothing else is saying the same thing: with a
        // kind chip present the card has already said what it is, and a stripe
        // beside it is the second telling that made every card read as a block
        // of colour. Compact cards have no chip — they are a title and nothing
        // else — so there the stripe is the whole answer.
        let rail = identity
            .filter(|_| self.compact || self.kind.is_none())
            .map(|identity| div().flex_none().w(px(metrics.rail)).bg(identity.text));

        let stack = div()
            .flex_1()
            .min_w_0()
            .column()
            .child(header)
            .children(body);

        let card = div()
            .w(px(metrics.width))
            .when_some(metrics.height, |element, height| element.h(px(height)))
            // Not `row()`: that centres its children, and a stripe centred on
            // its own zero content height is a stripe nobody can see.
            .flex()
            .flex_row()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .rounded(px(metrics.radius))
            .overflow_hidden()
            .frame(&theme, Surface::Raised, Elevation::Raised)
            // The state bleeds out of the card rather than being drawn round
            // it, so a running node and a failed one differ by the colour the
            // canvas takes near them and not by which of two lines they wear.
            .when(paint.aura_alpha > 0.0, |element| {
                element.shadow(vec![gpui::BoxShadow {
                    color: paint
                        .aura
                        .opacity(theme.effects.glow_alpha * paint.aura_alpha),
                    offset: gpui::point(px(0.0), px(0.0)),
                    blur_radius: px(theme.effects.glow_blur * (1.0 + aura_expansion)),
                    spread_radius: px(theme.effects.glow_spread * (1.0 + aura_expansion)),
                    inset: false,
                }])
            })
            // A node floats on the canvas rather than sitting in a column, so
            // it is the one place selection cannot be a rail at a reading
            // edge. It gets the accent all the way round instead, outward, so
            // it reads as the node being picked up rather than as a border the
            // node has always had.
            //
            // The ring stands off the card by a band of the card's own
            // surface, and that gap is what carries the meaning rather than
            // the hue: a node whose category is the accent colour would
            // otherwise wear a ring indistinguishable from its own identity,
            // and "selected" and "indigo" are not a distinction a reader
            // should have to make by memory.
            .when(self.selected, |element| {
                let gap = theme.effects.selection_rail_width * 0.6;
                let ring = |spread: f32, color: Hsla| gpui::BoxShadow {
                    color,
                    offset: gpui::point(px(0.0), px(0.0)),
                    blur_radius: px(0.0),
                    spread_radius: px(spread),
                    inset: false,
                };
                element.shadow(vec![
                    ring(gap * 2.0, theme.colors.accent),
                    ring(gap, theme.colors.raised),
                ])
            })
            .children(rail)
            .child(stack);

        // A node that takes a click is a button and a node that does not is a
        // group, so the role is decided before the spec is built rather than
        // patched afterwards.
        let role = if self.on_click.is_some() || self.on_delete.is_some() {
            Role::Button
        } else {
            Role::Group
        };
        let spec = NodeSpec::new(self.ident.semantic_id(), role)
            .text(self.title.clone())
            .value(self.state.value())
            .selected(self.selected)
            .busy(self.state.is_busy())
            .invalid(self.state.is_invalid());

        if self.on_click.is_none() && self.on_delete.is_none() {
            return card.semantic_in(cx, spec).into_any_element();
        }

        let mut card = card
            .id(self.ident.element_id())
            .cursor_pointer()
            .tab_index(0)
            .focus_ring(&theme)
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
        assert!(NodeState::Running.animates_mark());
        assert!(!NodeState::Queued.animates_mark());
    }

    #[test]
    fn pending_and_idle_carry_no_glyph_and_settled_states_do() {
        assert!(NodeState::Pending.glyph().is_none());
        assert!(NodeState::Idle.glyph().is_none());
        for state in [
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
        ] {
            assert!(state.glyph().is_some(), "{state:?}");
        }
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
        assert_eq!(node.node_width(), NODE_WIDTH);
        assert_eq!(node.ident().as_str(), "run.plan");
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
        // picture opens the body zone, so it gains that zone's rule and
        // padding as well as the picture itself.
        let expected = theme.borders.hairline
            + theme.spacing.sm * 2.0
            + (NODE_WIDTH - theme.effects.rail_width - theme.spacing.sm * 2.0)
                / DEFAULT_THUMBNAIL_RATIO;
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
        let many = GraphNode::new("many", "Many")
            .metric("Model", "GPT Image")
            .metric("Ratio", "16:9")
            .metric("Duration", "12s")
            .metric("Seed", "118")
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
        assert_eq!(doubled.radius, theme.radius(Radius::Card) * 2.0);
    }
}
