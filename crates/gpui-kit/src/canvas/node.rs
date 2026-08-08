//! One step of a run, drawn as a card on a graph canvas.
//!
//! A node reports four things and invents none of them: what it is, what it is
//! doing now, how it ended, and what it cost. The cost figures are the
//! caller's strings, because a component that formatted a token count would be
//! deciding a product question — thousands separators, units, rounding — on
//! behalf of every host that ever draws one.

use std::rc::Rc;

use gpui::{
    AnimationExt, AnyElement, App, Hsla, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, Styled, Transformation, Window, div, percentage,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, TypeScale};

use crate::foundation::{FocusRing, Ident, Pressable, Selectable, StyledExt};
use crate::motion;

/// The default width of a node, in pixels.
///
/// Nodes on one canvas share a width so the columns of a graph line up and the
/// eye can compare two steps without measuring them.
pub const NODE_WIDTH: f32 = 216.0;

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

type ClickHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A step of a run, as a card on the canvas.
#[derive(IntoElement)]
pub struct GraphNode {
    ident: Ident,
    title: SharedString,
    /// What the step is doing now, for a step that is doing something.
    action: Option<SharedString>,
    state: NodeState,
    metrics: Vec<NodeMetric>,
    diff: Option<Diff>,
    selected: bool,
    width: f32,
    on_click: Option<ClickHandler>,
}

impl std::fmt::Debug for GraphNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GraphNode")
            .field("ident", &self.ident)
            .field("title", &self.title)
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
            action: None,
            state: NodeState::default(),
            metrics: Vec::new(),
            diff: None,
            selected: false,
            width: NODE_WIDTH,
            on_click: None,
        }
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

        // The spinner is the one part of a node that moves, and it moves
        // because the step is still running. Under reduced motion it holds
        // still and the state is carried by the colour and the glyph, which
        // were already carrying it.
        let mark = self.state.glyph().map(|glyph| {
            let element = icon(glyph)
                .size(px(theme.control.sm.icon_size))
                .text_color(color);
            if self.state == NodeState::Running && !motion::reduce_motion(cx) {
                element
                    .with_animation(
                        self.ident.child("mark").element_id(),
                        motion::MotionSpec::new(
                            theme.motion.pulse_ms / 2,
                            motion::Easing::Linear.curve(&theme),
                        )
                        .repeating(),
                        |element, progress| {
                            element
                                .with_transformation(Transformation::rotate(percentage(progress)))
                        },
                    )
                    .into_any_element()
            } else {
                element.into_any_element()
            }
        });

        let header = div()
            .row()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .children(mark)
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .type_scale(&theme, TypeScale::Label)
                    .text_color(theme.colors.text)
                    .truncate()
                    .child(self.title.clone()),
            );

        let action = self.action.clone().map(|action| {
            div()
                .w_full()
                .type_scale(&theme, TypeScale::Caption)
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
                    .gap(px(theme.spacing.xs / 2.0))
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
                    .gap(px(theme.spacing.xs / 2.0))
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
                .gap_token(&theme, Space::Sm)
                .type_scale(&theme, TypeScale::Caption)
                .children(figures)
        });

        let card = div()
            .w(px(self.width))
            .column()
            .gap(px(theme.spacing.xs))
            .p_token(&theme, Space::Sm)
            .radius(&theme, Radius::Card)
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
            .children(action)
            .children(strip);

        // A node that takes a click is a button and a node that does not is a
        // group, so the role is decided before the spec is built rather than
        // patched afterwards.
        let role = if self.on_click.is_some() {
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

        let Some(handler) = self.on_click else {
            return card.semantic_in(cx, spec).into_any_element();
        };

        let mut card = card
            .id(self.ident.element_id())
            .cursor_pointer()
            .tab_index(0)
            .focus_ring(&theme)
            .pressable(cx);
        let click = Rc::clone(&handler);
        card.interactivity()
            .on_click(move |_, window, cx| click(window, cx));
        card.interactivity().on_key_down(move |event, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                handler(window, cx);
                cx.stop_propagation();
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
}
