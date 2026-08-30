use std::{cell::Cell, rc::Rc};

use crate::{
    AnyElement, App, AvailableSpace, Axis, Bounds, ContentMask, Element, ElementId,
    GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Size, Style, Window, point,
    size,
};

/// Measures `child` at its natural extent, contributes `progress` of that
/// extent to layout, and clips the subtree to the contributed bounds.
///
/// `Reveal` owns geometry, clipping, hit testing, and accessibility bounds; it
/// owns no clock or easing policy. The caller supplies a normalized progress
/// value on each frame. Values outside `0..=1` are clamped, and a non-finite
/// value is treated as zero.
///
/// The primary axis is measured without a maximum so the child does not chase
/// the shrinking frame around it. Cross-axis constraints still come from the
/// parent. At zero progress the child is measured but is neither prepainted nor
/// addressable. During a partial reveal the child keeps its natural size under
/// a content mask, so visible pixels, hitboxes, and accessibility bounds agree.
/// A stable `id` retains the latest natural measurement across frames; when
/// that measurement changes, `Reveal` requests one ordinary refresh.
pub fn reveal(id: impl Into<ElementId>, progress: f32, child: impl IntoElement) -> Reveal {
    Reveal {
        id: id.into(),
        progress: if progress.is_finite() {
            progress.clamp(0.0, 1.0)
        } else {
            0.0
        },
        axis: Axis::Vertical,
        from_end: false,
        child: child.into_any_element(),
    }
}

/// A measured subtree revealed along one physical axis.
pub struct Reveal {
    id: ElementId,
    progress: f32,
    axis: Axis,
    from_end: bool,
    child: AnyElement,
}

impl Reveal {
    /// Reveal along `axis`. The default is [`Axis::Vertical`].
    pub fn axis(mut self, axis: Axis) -> Self {
        self.axis = axis;
        self
    }

    /// Anchor the natural child to the physical bottom or right edge instead
    /// of the top or left edge while it is partially revealed.
    pub fn from_end(mut self) -> Self {
        self.from_end = true;
        self
    }
}

impl std::fmt::Debug for Reveal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Reveal")
            .field("id", &self.id)
            .field("progress", &self.progress)
            .field("axis", &self.axis)
            .field("from_end", &self.from_end)
            .finish()
    }
}

impl IntoElement for Reveal {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Reveal {
    type RequestLayoutState = Rc<Cell<Size<AvailableSpace>>>;
    type PrepaintState = bool;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let global_id = global_id.expect("Reveal requires its stable element id");
        let child_space = Rc::new(Cell::new(Size::default()));
        let offered = Rc::clone(&child_space);
        let progress = self.progress;
        let axis = self.axis;
        window.with_element_state(global_id, |state: Option<RevealState>, window| {
            let state = state.unwrap_or_default();
            let natural = state.natural.unwrap_or_default();
            let layout_id = window.request_measured_layout(
                Style::default(),
                move |known, available, _window, _cx| {
                    offered.set(match axis {
                        Axis::Vertical => size(
                            known
                                .width
                                .map(AvailableSpace::Definite)
                                .unwrap_or(available.width),
                            AvailableSpace::MaxContent,
                        ),
                        Axis::Horizontal => size(
                            AvailableSpace::MaxContent,
                            known
                                .height
                                .map(AvailableSpace::Definite)
                                .unwrap_or(available.height),
                        ),
                    });
                    match axis {
                        Axis::Vertical => size(
                            known.width.unwrap_or(natural.width),
                            known.height.unwrap_or(natural.height * progress),
                        ),
                        Axis::Horizontal => size(
                            known.width.unwrap_or(natural.width * progress),
                            known.height.unwrap_or(natural.height),
                        ),
                    }
                },
            );
            ((layout_id, child_space), state)
        })
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child_space: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let natural = self.child.layout_as_root(child_space.get(), window, cx);
        let global_id = global_id.expect("Reveal requires its stable element id");
        let changed = window.with_element_state(global_id, |state: Option<RevealState>, _| {
            let mut state = state.unwrap_or_default();
            let changed = state.natural != Some(natural);
            state.natural = Some(natural);
            (changed, state)
        });
        if changed {
            window.refresh();
        }
        if self.progress <= 0.0 || bounds.is_empty() {
            return false;
        }

        let origin = if self.from_end {
            match self.axis {
                Axis::Vertical => point(bounds.origin.x, bounds.bottom() - natural.height),
                Axis::Horizontal => point(bounds.right() - natural.width, bounds.origin.y),
            }
        } else {
            bounds.origin
        };
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            self.child.prepaint_at(origin, window, cx);
        });
        true
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _natural: &mut Self::RequestLayoutState,
        prepainted: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if *prepainted {
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                self.child.paint(window, cx);
            });
        }
    }
}

#[derive(Default)]
struct RevealState {
    natural: Option<Size<Pixels>>,
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use crate::{
        AnyWindowHandle, AppContext as _, Axis, Context, InputEvent, Modifiers, MouseButton,
        MouseDownEvent, Render, Role, TestAppContext, div, point, prelude::*, px, reveal, size,
    };

    struct RevealView {
        progress: Rc<Cell<f32>>,
        height: Rc<Cell<f32>>,
        pressed: Rc<Cell<usize>>,
        horizontal: bool,
    }

    impl Render for RevealView {
        fn render(
            &mut self,
            _window: &mut crate::Window,
            _cx: &mut Context<Self>,
        ) -> impl IntoElement {
            let pressed = self.pressed.clone();
            let child = div()
                .id("reveal-child")
                .debug_selector(|| "REVEAL_CHILD".into())
                .role(Role::Button)
                .w(px(100.0))
                .h(px(self.height.get()))
                .flex_none()
                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                    pressed.set(pressed.get() + 1);
                });
            let mut revealed = reveal("reveal", self.progress.get(), child);
            if self.horizontal {
                revealed = revealed.axis(Axis::Horizontal).from_end();
            }
            div()
                .flex()
                .when(self.horizontal, |element| element.flex_row())
                .when(!self.horizontal, |element| element.flex_col())
                .items_start()
                .child(revealed)
                .child(
                    div()
                        .debug_selector(|| "REVEAL_MARKER".into())
                        .size(px(10.0))
                        .flex_none(),
                )
        }
    }

    fn window(
        cx: &mut TestAppContext,
        progress: Rc<Cell<f32>>,
        pressed: Rc<Cell<usize>>,
        horizontal: bool,
    ) -> AnyWindowHandle {
        window_with_height(cx, progress, Rc::new(Cell::new(100.0)), pressed, horizontal)
    }

    fn window_with_height(
        cx: &mut TestAppContext,
        progress: Rc<Cell<f32>>,
        height: Rc<Cell<f32>>,
        pressed: Rc<Cell<usize>>,
        horizontal: bool,
    ) -> AnyWindowHandle {
        cx.add_window(move |_, _| RevealView {
            progress,
            height,
            pressed,
            horizontal,
        })
        .into()
    }

    fn draw(cx: &mut TestAppContext, window: AnyWindowHandle) {
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .expect("draw reveal");
    }

    #[gpui::test]
    fn vertical_reveal_moves_flow_without_squashing_the_child(cx: &mut TestAppContext) {
        let progress = Rc::new(Cell::new(0.4));
        let pressed = Rc::new(Cell::new(0));
        let window = window(cx, progress, pressed, false);
        draw(cx, window);

        cx.update_window(window, |_, window, _| {
            assert_eq!(
                window.rendered_frame.debug_bounds["REVEAL_CHILD"].size,
                size(px(100.0), px(100.0)),
                "the child retains the natural size measured before clipping"
            );
            assert_eq!(
                window.rendered_frame.debug_bounds["REVEAL_MARKER"].origin.y,
                px(40.0),
                "the revealed extent participates in ordinary flow"
            );
        })
        .expect("inspect reveal");
    }

    #[gpui::test]
    fn changed_natural_extent_is_adopted_on_the_requested_refresh(cx: &mut TestAppContext) {
        let progress = Rc::new(Cell::new(0.5));
        let height = Rc::new(Cell::new(100.0));
        let pressed = Rc::new(Cell::new(0));
        let window = window_with_height(cx, progress, height.clone(), pressed, false);
        draw(cx, window);

        height.set(160.0);
        draw(cx, window);
        cx.update_window(window, |_, window, _| {
            assert_eq!(
                window.rendered_frame.debug_bounds["REVEAL_MARKER"].origin.y,
                px(50.0),
                "the measurement discovered in prepaint cannot rewrite that frame's layout"
            );
        })
        .expect("inspect retained extent");

        draw(cx, window);
        cx.update_window(window, |_, window, _| {
            assert_eq!(
                window.rendered_frame.debug_bounds["REVEAL_MARKER"].origin.y,
                px(80.0),
                "the ordinary refresh adopts the latest natural measurement"
            );
        })
        .expect("inspect refreshed extent");
    }

    #[gpui::test]
    fn clipping_agrees_for_pointer_and_accessibility_geometry(cx: &mut TestAppContext) {
        let progress = Rc::new(Cell::new(0.4));
        let pressed = Rc::new(Cell::new(0));
        let window = window(cx, progress, pressed.clone(), false);
        cx.activate_accessibility(window);
        draw(cx, window);

        cx.update_window(window, |_, window, cx| {
            assert!(window.a11y.node_bounds.values().any(|bounds| {
                bounds.origin == point(px(0.0), px(0.0)) && bounds.size == size(px(100.0), px(40.0))
            }));
            for position in [point(px(10.0), px(20.0)), point(px(10.0), px(70.0))] {
                window.dispatch_event(
                    MouseDownEvent {
                        position,
                        button: MouseButton::Left,
                        modifiers: Modifiers::none(),
                        click_count: 1,
                        first_mouse: false,
                    }
                    .to_platform_input(),
                    cx,
                );
            }
        })
        .expect("inspect and operate reveal");
        assert_eq!(
            pressed.get(),
            1,
            "only the point inside the visible mask reaches the child"
        );
    }

    #[gpui::test]
    fn zero_progress_omits_the_subtree_from_prepaint_and_accessibility(cx: &mut TestAppContext) {
        let progress = Rc::new(Cell::new(0.0));
        let pressed = Rc::new(Cell::new(0));
        let window = window(cx, progress, pressed, false);
        cx.activate_accessibility(window);
        draw(cx, window);

        cx.update_window(window, |_, window, _| {
            assert!(
                !window
                    .rendered_frame
                    .debug_bounds
                    .contains_key("REVEAL_CHILD")
            );
            assert!(window.a11y.node_bounds.is_empty());
            assert_eq!(
                window.rendered_frame.debug_bounds["REVEAL_MARKER"].origin.y,
                px(0.0)
            );
        })
        .expect("inspect shut reveal");
    }

    #[gpui::test]
    fn horizontal_end_reveal_anchors_the_natural_child_to_the_right(cx: &mut TestAppContext) {
        let progress = Rc::new(Cell::new(0.5));
        let pressed = Rc::new(Cell::new(0));
        let window = window(cx, progress, pressed, true);
        draw(cx, window);

        cx.update_window(window, |_, window, _| {
            assert_eq!(
                window.rendered_frame.debug_bounds["REVEAL_CHILD"].origin.x,
                px(-50.0)
            );
            assert_eq!(
                window.rendered_frame.debug_bounds["REVEAL_MARKER"].origin.x,
                px(50.0)
            );
        })
        .expect("inspect horizontal reveal");
    }
}
