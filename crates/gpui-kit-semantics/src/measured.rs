use super::{
    DiagnosticTarget, NodeSpec, Role, SemanticCoordinator, record_diagnostic, redact_sensitive_text,
};
use gpui::{
    A11ySubtreeBuilder, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Refineable, SharedString, Style, StyleRefinement, Styled,
    Window, accesskit,
};

/// Descriptive native roles supported by [`MeasuredLeafBatch`].
///
/// Interactive roles deliberately cannot be supplied. Use ordinary semantic
/// elements for controls, focus, actions, ranges and accessibility relationships.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasuredLeafRole {
    Image,
    Text,
}

/// One noninteractive description of caller-measured geometry.
///
/// Construct inside a batch's prepaint callback. Bounds are current-frame,
/// untransformed logical **window** coordinates, not relative to the batch.
/// The caller owns visibility selection and geometry/viewport intersection;
/// the batch does not infer visibility from shapes, opacity or product data.
/// Business-derived ids must be unique within the batch and diagnostic window.
/// This is intentionally not convertible from `NodeSpec`: unsupported focus,
/// actions and relationships must not be accepted and silently discarded.
///
/// ```compile_fail
/// use gpui::{Bounds, Pixels};
/// use gpui_kit_semantics::{MeasuredLeaf, Role};
/// let control = MeasuredLeaf::new("control", Role::Button, Bounds::<Pixels>::default());
/// ```
#[derive(Clone, Debug)]
pub struct MeasuredLeaf {
    spec: NodeSpec,
    bounds: Bounds<Pixels>,
}

impl MeasuredLeaf {
    pub fn new(
        id: impl Into<SharedString>,
        role: MeasuredLeafRole,
        bounds: Bounds<Pixels>,
    ) -> Self {
        let role = match role {
            MeasuredLeafRole::Image => Role::Image,
            MeasuredLeafRole::Text => Role::Text,
        };
        Self {
            spec: NodeSpec::new(id, role),
            bounds,
        }
    }

    /// Accessible label; credential-shaped text is redacted in both outputs.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.spec = self.spec.text(text);
        self
    }

    /// Literal supplementary description, not a described-by relationship.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.spec = self.spec.description(description);
        self
    }

    /// Readout value; credential-shaped text is redacted in both outputs.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.spec = self.spec.value(value);
        self
    }

    /// Diagnostic selection only. Like ordinary Image/Text semantics, these
    /// native roles do not publish the selected property or a selection action.
    pub fn selected(mut self, selected: bool) -> Self {
        self.spec = self.spec.selected(selected);
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.spec = self.spec.read_only(read_only);
        self
    }
}

type Measure = dyn FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> Vec<MeasuredLeaf>;

/// A single layout element publishing many measured, noninteractive leaves.
///
/// `measure` runs at most once in the current prepaint, after layout, and is
/// skipped when native accessibility and installed diagnostics are both off.
/// It must only describe geometry, not perform required painting or side effects.
/// No per-leaf element, layout, hitbox, focus handle or action handler is created.
///
/// The batch is a real native Group under its mounted accessibility ancestor;
/// all synthetic leaves are its direct native children. Native ids derive from
/// the mounted batch identity plus each business id, never its list position.
/// Moving the batch to a different native ancestry changes those ids.
/// Diagnostic parentage is separate and opt-in via [`Self::diagnostic_parent`];
/// the batch itself does not add a diagnostic node.
///
/// Diagnostics follow ordinary `Semantic` conventions: logical bounds mapped
/// through the inherited visual transform, without ancestor clipping. Native
/// bounds additionally use GPUI's physical-pixel conservative content/rounded
/// clip. Fully clipped leaves remain native zero-area descriptions while their
/// owner is published; a fully clipped nonempty owner suppresses its subtree.
/// Rounded clipping cannot encode curved edges or holes in AccessKit. Callers
/// must exclude invisible product geometry themselves. No retained leaf cache
/// is used: replacement, omission or unmount removes old leaves next frame.
pub struct MeasuredLeafBatch {
    id: SharedString,
    diagnostic_parent: Option<SharedString>,
    measure: Option<Box<Measure>>,
    style: StyleRefinement,
}

impl MeasuredLeafBatch {
    /// Construct a batch with a stable mounted identity and current-prepaint
    /// measurement callback. It allocates one layout node regardless of count.
    pub fn new(
        id: impl Into<SharedString>,
        measure: impl FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> Vec<MeasuredLeaf> + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            diagnostic_parent: None,
            measure: Some(Box::new(measure)),
            style: StyleRefinement::default(),
        }
    }

    /// Declares the diagnostic parent for every leaf; does not reparent native
    /// nodes or create a native relationship. The host must publish that parent.
    pub fn diagnostic_parent(mut self, parent: impl Into<SharedString>) -> Self {
        self.diagnostic_parent = Some(parent.into());
        self
    }
}

impl Styled for MeasuredLeafBatch {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl IntoElement for MeasuredLeafBatch {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

/// Frame-local storage used by the batch's [`Element`] implementation.
#[doc(hidden)]
pub struct MeasuredLeafBatchState {
    leaves: Vec<MeasuredLeaf>,
    scale: f32,
}

impl Element for MeasuredLeafBatch {
    type RequestLayoutState = Style;
    type PrepaintState = MeasuredLeafBatchState;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone().into())
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn a11y_role(&self) -> Option<accesskit::Role> {
        Some(accesskit::Role::Group)
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Style) {
        let mut style = Style::default();
        style.refine(&self.style);
        (window.request_layout(style.clone(), [], cx), style)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Style,
        window: &mut Window,
        cx: &mut App,
    ) -> MeasuredLeafBatchState {
        let target = SemanticCoordinator::try_global(cx)
            .filter(SemanticCoordinator::is_armed)
            .map(DiagnosticTarget::Installed);
        let native = window.is_a11y_active();
        let mut state = MeasuredLeafBatchState {
            leaves: Vec::new(),
            scale: window.scale_factor(),
        };
        if target.is_none() && !native {
            return state;
        }
        state.leaves = self.measure.take().expect("batch prepaint runs once")(bounds, window, cx);
        let mut ids = std::collections::HashSet::with_capacity(state.leaves.len());
        for leaf in &state.leaves {
            assert!(
                ids.insert(&leaf.spec.id),
                "measured leaf ids must be unique within a batch"
            );
        }
        let transform = window.visual_transform();
        for leaf in &mut state.leaves {
            leaf.spec.parent.clone_from(&self.diagnostic_parent);
            record_diagnostic(
                target.as_ref(),
                &leaf.spec,
                transform.map_bounds(leaf.bounds),
                false,
                window,
            );
        }
        if !native {
            state.leaves.clear();
        }
        state
    }

    fn a11y_synthetic_children(
        &mut self,
        state: &mut MeasuredLeafBatchState,
        builder: &mut A11ySubtreeBuilder,
    ) {
        for leaf in &state.leaves {
            let role =
                super::platform_role(leaf.spec.role).expect("descriptive role has a native role");
            let mut node = accesskit::Node::new(role);
            if let Some(text) = &leaf.spec.text {
                node.set_label(redact_sensitive_text(text));
            }
            if let Some(description) = &leaf.spec.description {
                node.set_description(redact_sensitive_text(description));
            }
            if let Some(value) = &leaf.spec.value {
                node.set_value(redact_sensitive_text(value));
            }
            if leaf.spec.read_only {
                node.set_read_only();
            }
            let b = leaf.bounds;
            let scale = state.scale;
            node.set_bounds(accesskit::Rect {
                x0: (f32::from(b.origin.x) * scale) as f64,
                y0: (f32::from(b.origin.y) * scale) as f64,
                x1: (f32::from(b.origin.x + b.size.width) * scale) as f64,
                y1: (f32::from(b.origin.y + b.size.height) * scale) as f64,
            });
            let id = builder.synthetic_node_id(&leaf.spec.id);
            assert!(
                builder.push_child(id, node),
                "measured native leaf identity collision"
            );
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Style,
        _: &mut MeasuredLeafBatchState,
        window: &mut Window,
        cx: &mut App,
    ) {
        style.paint(bounds, window, cx, |_, _| {});
    }
}

#[cfg(test)]
mod tests;
