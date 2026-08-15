//! One step of a run, drawn as a card on a graph canvas.
//!
//! A node reports four things and invents none of them: what it is, what it is
//! doing now, how it ended, and what it cost. The cost figures are the
//! caller's strings, because a component that formatted a token count would be
//! deciding a product question — thousands separators, units, rounding — on
//! behalf of every host that ever draws one.

use std::rc::Rc;

use gpui::{
    AnyElement, App, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Surface};

use crate::foundation::{FocusRing, Ident, Pressable, Selectable, StyledExt};
use crate::motion;

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
    width: f32,
    height: Option<f32>,
    padding: f32,
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
            width: scaled(width),
            height: height.map(scaled),
            padding: scaled(theme.spacing.sm),
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

/// How a step ended, or that it has not.
///
/// These are five separate answers and stay separate: a step that the host
/// refused to run is not a step that failed, and a step nobody has reached yet
/// is not a step that succeeded quietly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeState {
    /// Not reached yet.
    #[default]
    Pending,
    Running,
    Succeeded,
    Failed,
    /// The host declined to run it. Shown as a refusal, never as a failure and
    /// never as an empty step.
    Refused,
}

impl NodeState {
    pub fn color(self, theme: &gpui_kit_theme::Theme) -> Hsla {
        match self {
            Self::Pending => theme.colors.text_faint,
            Self::Running => theme.colors.accent,
            Self::Succeeded => theme.colors.success,
            Self::Failed => theme.colors.danger,
            Self::Refused => theme.colors.warning,
        }
    }

    fn glyph(self) -> Option<Icon> {
        match self {
            Self::Pending => None,
            Self::Running => Some(Icon::Refresh),
            Self::Succeeded => Some(Icon::Check),
            Self::Failed => Some(Icon::Close),
            Self::Refused => Some(Icon::Danger),
        }
    }

    /// What the node publishes as its value.
    fn value(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Refused => "refused",
        }
    }

    /// Whether the state is worth bleeding into the pixels around the card.
    ///
    /// Only the states a reader is scanning for glow. A canvas where every
    /// node glowed would be a canvas where none of them stood out, which is
    /// the same as no glow at all but more expensive to draw.
    fn is_notable(self) -> bool {
        matches!(self, Self::Running | Self::Failed | Self::Refused)
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
    metrics: Vec<NodeMetric>,
    ports: Vec<GraphPort>,
    diff: Option<Diff>,
    selected: bool,
    width: f32,
    display_zoom: f32,
    declared_height: Option<f32>,
    pointer_click: bool,
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
            metrics: Vec::new(),
            ports: Vec::new(),
            diff: None,
            selected: false,
            width: NODE_WIDTH,
            display_zoom: 1.0,
            declared_height: None,
            pointer_click: true,
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

    #[cfg(test)]
    pub(crate) fn logical_height(&self, theme: &gpui_kit_theme::Theme) -> f32 {
        self.declared_height
            .unwrap_or_else(|| self.measured_height(theme))
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
        let mut rows = vec![theme.typography.label.line_height];
        if self.thumbnail.is_some() {
            rows.push((self.width - theme.spacing.sm * 2.0).max(0.0) / self.thumbnail_ratio);
        }
        if self.action.is_some() {
            rows.push(theme.typography.caption.line_height);
        }
        if !self.metrics.is_empty() || self.diff.is_some_and(|diff| !diff.is_empty()) {
            rows.push(theme.typography.caption.line_height);
        }
        let gaps = theme.spacing.xs * (rows.len() - 1) as f32;
        theme.spacing.sm * 2.0 + rows.iter().sum::<f32>() + gaps
    }
}

impl Selectable for GraphNode {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl RenderOnce for GraphNode {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let color = self.state.color(&theme);
        let metrics = NodeMetrics::new(&theme, self.width, self.display_zoom, self.declared_height);

        // The mark is the one part of a node that moves, and it moves because
        // the step is still running. It turns through the shared vocabulary,
        // so a running node and a running tool call turn at one rate.
        let mark = self.state.glyph().map(|glyph| {
            let element = icon(glyph).size(px(metrics.icon_size)).text_color(color);
            match self.state {
                NodeState::Running => {
                    motion::spin(element, self.ident.child("mark").element_id(), &theme, cx)
                }
                _ => element.into_any_element(),
            }
        });

        let header = div()
            .row()
            .w_full()
            .gap(px(metrics.gap))
            .children(mark)
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
            );

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
                            .child(format!("+{}", diff.added)),
                    )
                    .child(
                        div()
                            .text_color(theme.colors.danger)
                            .child(format!("-{}", diff.removed)),
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

        let card = div()
            .w(px(metrics.width))
            .when_some(metrics.height, |element, height| element.h(px(height)))
            .column()
            .font_fallbacks(gpui_kit_assets::text_fallbacks())
            .gap(px(metrics.gap))
            .p(px(metrics.padding))
            .rounded(px(metrics.radius))
            .frame(&theme, Surface::Raised, Elevation::Raised)
            // The state bleeds out of the card rather than being drawn round
            // it, so a running node and a failed one differ by the colour the
            // canvas takes near them and not by which of two lines they wear.
            .when(self.state.is_notable(), |element| {
                element.glow(&theme, color)
            })
            .when(self.selected, |element| {
                element.shadow(theme.selected_ring())
            })
            .child(header)
            .children(thumbnail)
            .children(action)
            .children(strip);

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
            .busy(self.state == NodeState::Running)
            .invalid(self.state == NodeState::Failed);

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

    fn theme() -> gpui_kit_theme::Theme {
        gpui_kit_theme::Theme::studio_dark()
    }

    /// The states exist to be told apart, so no two of them may report the
    /// same colour or the same word.
    #[test]
    fn every_state_is_distinguishable_from_every_other() {
        let theme = theme();
        let states = [
            NodeState::Pending,
            NodeState::Running,
            NodeState::Succeeded,
            NodeState::Failed,
            NodeState::Refused,
        ];
        for (index, state) in states.iter().enumerate() {
            for other in &states[index + 1..] {
                assert_ne!(
                    state.color(&theme),
                    other.color(&theme),
                    "{state:?} {other:?}"
                );
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
        assert!(NodeState::Running.is_notable());
        assert!(NodeState::Failed.is_notable());
        assert!(NodeState::Refused.is_notable());
        assert!(!NodeState::Pending.is_notable());
        assert!(!NodeState::Succeeded.is_notable());
    }

    #[test]
    fn a_pending_step_carries_no_glyph_and_the_rest_do() {
        assert!(NodeState::Pending.glyph().is_none());
        for state in [
            NodeState::Running,
            NodeState::Succeeded,
            NodeState::Failed,
            NodeState::Refused,
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
        let expected =
            (NODE_WIDTH - theme.spacing.sm * 2.0) / DEFAULT_THUMBNAIL_RATIO + theme.spacing.xs;
        assert!((thumbnail - plain - expected).abs() < 0.001);

        let square = GraphNode::new("square", "Square")
            .thumbnail(div())
            .thumbnail_ratio(1.0)
            .measured_height(&theme);
        assert!(square > thumbnail);
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
