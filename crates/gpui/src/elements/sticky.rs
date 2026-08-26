use crate::{
    AnyElement, App, Bounds, ContentMask, Element, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window, point, px,
};

/// A horizontal physical viewport edge that can hold a [`Sticky`] element in
/// place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StickyEdge {
    /// The left edge of the current content mask.
    Left,
    /// The right edge of the current content mask.
    Right,
}

/// Keeps `child` against `edge` while its normal layout position crosses the
/// current clipped viewport.
///
/// The child still participates in ordinary layout. During prepaint its whole
/// subtree is translated to the held position and deferred above scrolling
/// siblings, while retaining the content mask that established the viewport.
/// Paint, hit testing, and accessibility bounds therefore use the same
/// translated geometry. The edge is physical; callers implementing reading
/// direction choose left or right explicitly.
pub fn sticky(edge: StickyEdge, child: impl IntoElement) -> Sticky {
    Sticky {
        edge,
        child: Some(child.into_any_element()),
    }
}

/// An element held against one horizontal physical edge of the current clipped
/// viewport.
pub struct Sticky {
    edge: StickyEdge,
    child: Option<AnyElement>,
}

impl Element for Sticky {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<crate::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let layout_id = self
            .child
            .as_mut()
            .expect("required framework invariant must hold")
            .request_layout(window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let mask = window.content_mask();
        let translation = match self.edge {
            StickyEdge::Left => point((mask.bounds.left() - bounds.left()).max(px(0.0)), px(0.0)),
            StickyEdge::Right => {
                point((mask.bounds.right() - bounds.right()).min(px(0.0)), px(0.0))
            }
        };
        let absolute_offset = window.element_offset() + translation;
        let child = self
            .child
            .take()
            .expect("required framework invariant must hold");
        window.defer_draw(
            child,
            absolute_offset,
            1,
            Some(ContentMask {
                bounds: mask.bounds,
            }),
        );
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

impl IntoElement for Sticky {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use crate::{
        AnyWindowHandle, AppContext as _, Context, InputEvent, Modifiers, MouseButton,
        MouseDownEvent, Render, ScrollHandle, TestAppContext, div, point, prelude::*, px, size,
        sticky,
    };

    use super::StickyEdge;

    struct StickyTestView {
        scroll: ScrollHandle,
        pressed: Rc<Cell<usize>>,
    }

    impl Render for StickyTestView {
        fn render(
            &mut self,
            _window: &mut crate::Window,
            _cx: &mut Context<Self>,
        ) -> impl IntoElement {
            let pressed = self.pressed.clone();
            div().size_full().pt(px(20.0)).pl(px(100.0)).child(
                div()
                    .id("sticky-scroll")
                    .w(px(100.0))
                    .h(px(40.0))
                    .overflow_x_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .w(px(200.0))
                            .h(px(40.0))
                            .flex_none()
                            .child(sticky(
                                StickyEdge::Left,
                                div()
                                    .id("sticky-target")
                                    .debug_selector(|| "STICKY_TARGET".into())
                                    .role(crate::Role::Button)
                                    .w(px(30.0))
                                    .h(px(40.0))
                                    .flex_none()
                                    .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                        pressed.set(pressed.get() + 1)
                                    }),
                            )),
                    ),
            )
        }
    }

    #[gpui::test]
    fn sticky_translates_geometry_and_keeps_hitboxes_clipped(cx: &mut TestAppContext) {
        let scroll = ScrollHandle::new();
        scroll.set_offset(point(px(-50.0), px(0.0)));
        let pressed = Rc::new(Cell::new(0));
        let window: AnyWindowHandle = cx
            .add_window({
                let scroll = scroll.clone();
                let pressed = pressed.clone();
                move |_, _| StickyTestView { scroll, pressed }
            })
            .into();

        cx.activate_accessibility(window);
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .expect("draw sticky element");

        cx.update_window(window, |_, window, cx| {
            let bounds = window
                .rendered_frame
                .debug_bounds
                .get("STICKY_TARGET")
                .copied()
                .expect("sticky bounds");
            assert_eq!(bounds.origin, point(px(100.0), px(20.0)));
            let accessible = window
                .a11y
                .node_bounds
                .values()
                .find(|bounds| bounds.size == size(px(30.0), px(40.0)))
                .copied()
                .expect("sticky accessibility bounds");
            assert_eq!(accessible, bounds);

            window.dispatch_event(
                MouseDownEvent {
                    position: point(px(110.0), px(30.0)),
                    button: MouseButton::Left,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                    first_mouse: false,
                }
                .to_platform_input(),
                cx,
            );
            // The child's old translated-by-scroll position lies outside the
            // viewport. A deferred subtree must not leave a live hitbox there.
            window.dispatch_event(
                MouseDownEvent {
                    position: point(px(60.0), px(30.0)),
                    button: MouseButton::Left,
                    modifiers: Modifiers::none(),
                    click_count: 1,
                    first_mouse: false,
                }
                .to_platform_input(),
                cx,
            );
        })
        .expect("dispatch pointer events");

        assert_eq!(pressed.get(), 1);
    }

    #[gpui::test]
    fn sticky_right_holds_a_child_at_the_viewport_edge(cx: &mut TestAppContext) {
        struct RightView;
        impl Render for RightView {
            fn render(
                &mut self,
                _window: &mut crate::Window,
                _cx: &mut Context<Self>,
            ) -> impl IntoElement {
                div().size_full().pt(px(20.0)).pl(px(100.0)).child(
                    div().w(px(100.0)).h(px(40.0)).overflow_hidden().child(
                        div()
                            .flex()
                            .flex_row()
                            .w(px(200.0))
                            .h(px(40.0))
                            .flex_none()
                            .child(div().w(px(170.0)).h_full().flex_none())
                            .child(sticky(
                                StickyEdge::Right,
                                div()
                                    .debug_selector(|| "STICKY_RIGHT".into())
                                    .w(px(30.0))
                                    .h_full()
                                    .flex_none(),
                            )),
                    ),
                )
            }
        }

        let window: AnyWindowHandle = cx.add_window(|_, _| RightView).into();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
            .expect("draw right sticky element");
        cx.update_window(window, |_, window, _| {
            let bounds = window
                .rendered_frame
                .debug_bounds
                .get("STICKY_RIGHT")
                .copied()
                .expect("right sticky bounds");
            assert_eq!(bounds.origin, point(px(170.0), px(20.0)));
            assert_eq!(bounds.size, size(px(30.0), px(40.0)));
        })
        .expect("inspect right sticky bounds");
    }
}
