//! Intrinsic leaves share Taffy's container algorithms, not its aspect-ratio
//! height floor. Layout results live alongside the tree because TaffyTree's
//! public API exposes its cache and topology but no mutable layout storage.

use super::*;
use taffy::util::{MaybeMath, MaybeResolve, ResolveOrZero};
use taffy::{
    BlockContext, CacheTree, CoreStyle, Layout, LayoutBlockContainer, LayoutFlexboxContainer,
    LayoutGridContainer, LayoutInput, LayoutOutput, LayoutPartialTree, RunMode, SizingMode,
    TraversePartialTree, compute_block_layout, compute_cached_layout, compute_flexbox_layout,
    compute_grid_layout, compute_hidden_layout, compute_leaf_layout,
};

pub(super) struct LayoutView<'a> {
    pub tree: &'a mut TaffyTree<NodeContext>,
    pub layouts: &'a mut FxHashMap<NodeId, Layout>,
    pub window: &'a mut Window,
    pub cx: &'a mut App,
    pub scale_factor: f32,
}

impl LayoutView<'_> {
    fn dispatch(
        &mut self,
        node: NodeId,
        inputs: LayoutInput,
        block: Option<&mut BlockContext<'_>>,
    ) -> LayoutOutput {
        if inputs.run_mode == RunMode::PerformHiddenLayout {
            return compute_hidden_layout(self, node);
        }
        compute_cached_layout(self, node, inputs, |this, node, inputs| {
            let style = this.tree.style(node).expect(EXPECT_MESSAGE).clone();
            // Block flow fills the width of ordinary boxes, but a replaced
            // leaf with auto width uses its natural width (or the width
            // derived from an authored height). Flex/grid stretch and
            // absolutely positioned inset constraints remain authoritative.
            let intrinsic_block_width = style.item_is_replaced
                && style.size.width.is_auto()
                && style.position == taffy::Position::Relative
                && this.tree.parent(node).is_some_and(|parent| {
                    this.tree.style(parent).expect(EXPECT_MESSAGE).display == taffy::Display::Block
                });
            match (style.display, this.tree.child_count(node) > 0) {
                (taffy::Display::None, _) => compute_hidden_layout(this, node),
                (taffy::Display::Block, true) => compute_block_layout(this, node, inputs, block),
                (taffy::Display::Flex, true) => compute_flexbox_layout(this, node, inputs),
                (taffy::Display::Grid, true) => compute_grid_layout(this, node, inputs),
                (_, false) => match this.tree.get_node_context_mut(node) {
                    Some(NodeContext::Intrinsic(natural)) => {
                        let mut inputs = inputs;
                        if intrinsic_block_width {
                            inputs.known_dimensions.width = None;
                        }
                        intrinsic_layout(inputs, &style, *natural)
                    }
                    context => compute_leaf_layout(
                        inputs,
                        &style,
                        |_, _| 0.,
                        |known, available| {
                            let Some(NodeContext::Measured(measure)) = context else {
                                return TaffySize::default();
                            };
                            let scale = this.scale_factor;
                            let known = Size {
                                width: known.width.map(|v| Pixels(v / scale)),
                                height: known.height.map(|v| Pixels(v / scale)),
                            };
                            let available: Size<AvailableSpace> = available.into();
                            let unscale = |v| match v {
                                AvailableSpace::Definite(v) => AvailableSpace::Definite(v / scale),
                                other => other,
                            };
                            let measured = measure(
                                known,
                                size(unscale(available.width), unscale(available.height)),
                                this.window,
                                this.cx,
                            );
                            snap_measured_size_to_device_pixels(measured, scale).into()
                        },
                    ),
                },
            }
        })
    }
}

impl TraversePartialTree for LayoutView<'_> {
    type ChildIter<'a>
        = <TaffyTree<NodeContext> as TraversePartialTree>::ChildIter<'a>
    where
        Self: 'a;
    fn child_ids(&self, node: NodeId) -> Self::ChildIter<'_> {
        self.tree.child_ids(node)
    }
    fn child_count(&self, node: NodeId) -> usize {
        self.tree.child_count(node)
    }
    fn get_child_id(&self, node: NodeId, index: usize) -> NodeId {
        self.tree.get_child_id(node, index)
    }
}

impl CacheTree for LayoutView<'_> {
    fn cache_get(&self, node: NodeId, inputs: &LayoutInput) -> Option<LayoutOutput> {
        self.tree.cache_get(node, inputs)
    }
    fn cache_store(&mut self, node: NodeId, inputs: &LayoutInput, output: LayoutOutput) {
        self.tree.cache_store(node, inputs, output);
    }
    fn cache_clear(&mut self, node: NodeId) {
        self.tree.cache_clear(node);
    }
}

impl LayoutPartialTree for LayoutView<'_> {
    type CustomIdent = <taffy::Style as CoreStyle>::CustomIdent;
    type CoreContainerStyle<'a>
        = &'a taffy::Style
    where
        Self: 'a;
    fn get_core_container_style(&self, node: NodeId) -> Self::CoreContainerStyle<'_> {
        self.tree.style(node).expect(EXPECT_MESSAGE)
    }
    fn set_unrounded_layout(&mut self, node: NodeId, layout: &Layout) {
        self.layouts.insert(node, *layout);
    }
    fn compute_child_layout(&mut self, node: NodeId, inputs: LayoutInput) -> LayoutOutput {
        self.dispatch(node, inputs, None)
    }
}

impl LayoutFlexboxContainer for LayoutView<'_> {
    type FlexboxContainerStyle<'a>
        = &'a taffy::Style
    where
        Self: 'a;
    type FlexboxItemStyle<'a>
        = &'a taffy::Style
    where
        Self: 'a;
    fn get_flexbox_container_style(&self, node: NodeId) -> Self::FlexboxContainerStyle<'_> {
        self.tree.style(node).expect(EXPECT_MESSAGE)
    }
    fn get_flexbox_child_style(&self, node: NodeId) -> Self::FlexboxItemStyle<'_> {
        self.tree.style(node).expect(EXPECT_MESSAGE)
    }
}

impl LayoutGridContainer for LayoutView<'_> {
    type GridContainerStyle<'a>
        = &'a taffy::Style
    where
        Self: 'a;
    type GridItemStyle<'a>
        = &'a taffy::Style
    where
        Self: 'a;
    fn get_grid_container_style(&self, node: NodeId) -> Self::GridContainerStyle<'_> {
        self.tree.style(node).expect(EXPECT_MESSAGE)
    }
    fn get_grid_child_style(&self, node: NodeId) -> Self::GridItemStyle<'_> {
        self.tree.style(node).expect(EXPECT_MESSAGE)
    }
}

impl LayoutBlockContainer for LayoutView<'_> {
    type BlockContainerStyle<'a>
        = &'a taffy::Style
    where
        Self: 'a;
    type BlockItemStyle<'a>
        = &'a taffy::Style
    where
        Self: 'a;
    fn get_block_container_style(&self, node: NodeId) -> Self::BlockContainerStyle<'_> {
        self.tree.style(node).expect(EXPECT_MESSAGE)
    }
    fn get_block_child_style(&self, node: NodeId) -> Self::BlockItemStyle<'_> {
        self.tree.style(node).expect(EXPECT_MESSAGE)
    }
    fn compute_block_child_layout(
        &mut self,
        node: NodeId,
        inputs: LayoutInput,
        block: Option<&mut BlockContext<'_>>,
    ) -> LayoutOutput {
        self.dispatch(node, inputs, block)
    }
}

/// Resolve the content's natural ratio only for a missing axis. Taffy's stock
/// leaf still owns padding, borders, overflow, baselines and margin metadata;
/// it receives resolved dimensions without an aspect-ratio height floor.
fn intrinsic_layout(
    mut inputs: LayoutInput,
    style: &taffy::Style,
    natural: TaffySize<f32>,
) -> LayoutOutput {
    let resolve = |_, _| 0.;
    let inset = (style
        .padding
        .resolve_or_zero(inputs.parent_size.width, resolve)
        + style
            .border
            .resolve_or_zero(inputs.parent_size.width, resolve))
    .sum_axes();
    let adjustment = if style.box_sizing == taffy::BoxSizing::ContentBox {
        inset
    } else {
        TaffySize::ZERO
    };
    let preferred = if inputs.sizing_mode == SizingMode::InherentSize {
        style
            .size
            .maybe_resolve(inputs.parent_size, resolve)
            .maybe_add(adjustment)
    } else {
        TaffySize::NONE
    };
    let min = style
        .min_size
        .maybe_resolve(inputs.parent_size, resolve)
        .maybe_add(adjustment);
    let max = style
        .max_size
        .maybe_resolve(inputs.parent_size, resolve)
        .maybe_add(adjustment);
    let known = inputs.known_dimensions.or(preferred).maybe_clamp(min, max);
    let ratio = style.aspect_ratio.or_else(|| {
        (natural.width > 0. && natural.height > 0.).then_some(natural.width / natural.height)
    });
    let content = match (known.width, known.height) {
        (Some(w), Some(h)) => TaffySize {
            width: (w - inset.width).max(0.),
            height: (h - inset.height).max(0.),
        },
        (Some(w), None) => {
            let width = (w - inset.width).max(0.);
            TaffySize {
                width,
                height: ratio.map_or(natural.height, |ratio| width / ratio),
            }
        }
        (None, Some(h)) => {
            let height = (h - inset.height).max(0.);
            TaffySize {
                width: ratio.map_or(natural.width, |ratio| height * ratio),
                height,
            }
        }
        (None, None) => natural,
    };
    let resolved = (content + inset).maybe_clamp(min, max);
    inputs.known_dimensions = resolved.map(Some);
    let mut style = style.clone();
    style.aspect_ratio = None;
    compute_leaf_layout(inputs, &style, resolve, |_, _| resolved - inset)
}
