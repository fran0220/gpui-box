use crate::{
    AnyElement, App, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Window,
};

/// Builds a `Deferred` element, which delays the layout and paint of its child.
pub fn deferred(child: impl IntoElement) -> Deferred {
    Deferred {
        child: Some(child.into_any_element()),
        priority: 0,
        preserve_accessibility: false,
        unclipped: false,
    }
}

/// An element which delays the painting of its child until after all of
/// its ancestors, while keeping its layout as part of the current element tree.
pub struct Deferred {
    child: Option<AnyElement>,
    priority: usize,
    preserve_accessibility: bool,
    unclipped: bool,
}

impl Deferred {
    /// Escapes inherited content masks when scheduling this subtree. Its own
    /// masks still apply. The inherited visual transform and logical layout
    /// coordinates are preserved, not reset to window-root coordinates.
    /// Accessibility ancestry is independent; combine with
    /// [`Self::preserve_accessibility`] for inline logical ownership.
    /// Without this opt-in, deferred clipping behavior is unchanged.
    pub fn unclipped(mut self) -> Self {
        self.unclipped = true;
        self
    }

    /// Retains logical accessibility ancestry and reading order for inline
    /// content. By default, deferred overlay surfaces attach to the window root.
    pub fn preserve_accessibility(mut self) -> Self {
        self.preserve_accessibility = true;
        self
    }

    /// Sets the `priority` value of the `deferred` element, which
    /// determines the drawing order relative to other deferred elements,
    /// with higher values being drawn on top.
    pub fn with_priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }
}

impl Element for Deferred {
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
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let child = self
            .child
            .take()
            .expect("required framework invariant must hold");
        let element_offset = window.element_offset();
        let schedule = |window: &mut Window| {
            if self.preserve_accessibility {
                window.defer_draw_with_accessibility(child, element_offset, self.priority, None)
            } else {
                window.defer_draw(child, element_offset, self.priority, None)
            }
        };
        if self.unclipped {
            window.without_content_masks(schedule)
        } else {
            schedule(window)
        }
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

impl IntoElement for Deferred {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Deferred {
    /// Sets a priority for the element. A higher priority conceptually means painting the element
    /// on top of deferred draws with a lower priority (i.e. closer to the viewer).
    pub fn priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Context, Entity, StyleRefinement, TestAppContext, Window, anchored, deferred, div, point,
        prelude::*, px, size,
    };

    struct Masked {
        child: crate::AnyElement,
        mode: u8,
    }

    fn mask_scope<R>(mode: u8, w: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
        use crate::{Bounds, ContentMask, Corners, RoundedClip};
        if mode != 0 {
            let inner = |w: &mut Window| {
                let bounds = Bounds::new(point(px(10.), px(8.)), size(px(80.), px(60.)));
                w.with_content_mask(Some(ContentMask { bounds }), |w| {
                    w.with_rounded_content_mask(
                        RoundedClip::new(
                            bounds,
                            Corners {
                                top_left: px(12.),
                                ..Corners::default()
                            },
                        ),
                        f,
                    )
                })
            };
            if mode == 1 {
                return w.without_content_masks(inner);
            }
            return inner(w);
        }
        w.with_visual_scale(1.5, point(px(-10.), px(-5.)), |w| {
            w.with_content_mask(
                Some(ContentMask {
                    bounds: Bounds::new(point(px(0.), px(0.)), size(px(45.), px(35.))),
                }),
                |w| {
                    w.with_rounded_content_mask(
                        RoundedClip::new(
                            Bounds::new(point(px(0.), px(0.)), size(px(60.), px(40.))),
                            Corners {
                                top_left: px(20.),
                                ..Corners::default()
                            },
                        ),
                        f,
                    )
                },
            )
        })
    }

    impl IntoElement for Masked {
        type Element = Self;
        fn into_element(self) -> Self {
            self
        }
    }
    impl crate::Element for Masked {
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
            _: Option<&crate::GlobalElementId>,
            _: Option<&crate::InspectorElementId>,
            w: &mut Window,
            cx: &mut crate::App,
        ) -> (crate::LayoutId, ()) {
            (self.child.request_layout(w, cx), ())
        }
        fn prepaint(
            &mut self,
            _: Option<&crate::GlobalElementId>,
            _: Option<&crate::InspectorElementId>,
            _: crate::Bounds<crate::Pixels>,
            _: &mut (),
            w: &mut Window,
            cx: &mut crate::App,
        ) {
            mask_scope(self.mode, w, |w| self.child.prepaint(w, cx));
        }
        fn paint(
            &mut self,
            _: Option<&crate::GlobalElementId>,
            _: Option<&crate::InspectorElementId>,
            _: crate::Bounds<crate::Pixels>,
            _: &mut (),
            _: &mut (),
            w: &mut Window,
            cx: &mut crate::App,
        ) {
            mask_scope(self.mode, w, |w| self.child.paint(w, cx));
        }
    }

    struct EscapePanel {
        escape: bool,
        renders: std::rc::Rc<std::cell::Cell<usize>>,
    }
    impl Render for EscapePanel {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            let leaf = |id, color| {
                div()
                    .id(id)
                    .role(crate::Role::Button)
                    .absolute()
                    .size_full()
                    .bg(color)
                    .on_mouse_down(crate::MouseButton::Left, |_, _, _| {})
            };
            let deferred = deferred(Masked {
                mode: 2,
                child: leaf("escaped", crate::green()).into_any_element(),
            })
            .preserve_accessibility();
            div()
                .id("owner")
                .role(crate::Role::Group)
                .size_full()
                .child(Masked {
                    mode: 1,
                    child: leaf("direct", crate::blue()).into_any_element(),
                })
                .child(leaf("following", crate::red()))
                .child(if self.escape {
                    deferred.unclipped()
                } else {
                    deferred
                })
        }
    }
    struct EscapeRoot(Entity<EscapePanel>);
    impl Render for EscapeRoot {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Masked {
                mode: 0,
                child: self
                    .0
                    .clone()
                    .cached(StyleRefinement::default().size_full())
                    .into_any_element(),
            }
        }
    }

    #[gpui::test]
    fn unclipped_deferred_preserves_scale_inner_masks_restoration_cache_and_a11y(
        cx: &mut TestAppContext,
    ) {
        let renders = std::rc::Rc::new(std::cell::Cell::new(0));
        let handle = cx.open_window(size(px(180.), px(130.)), |_, cx| {
            EscapeRoot(cx.new(|_| EscapePanel {
                escape: false,
                renders: renders.clone(),
            }))
        });
        cx.run_until_parked();
        let verify = |w: &Window, escaped: bool| {
            let hits = &w.rendered_frame.hitboxes;
            assert_eq!(hits.len(), 3);
            let outside_rect = point(px(80.), px(40.));
            let outside_round = point(px(110.), px(60.));
            assert!(
                hits[0].contains(&outside_rect) && hits[0].contains(&outside_round),
                "direct scope clears both masks"
            );
            assert!(
                !hits[1].contains(&outside_rect) && !hits[1].contains(&point(px(6.), px(3.))),
                "following sibling restores rectangle and rounded corner"
            );
            assert_eq!(
                hits[2].contains(&outside_round),
                escaped,
                "deferred opt-in only"
            );
            assert!(
                !hits[2].contains(&point(px(21.), px(15.))),
                "own rounded corner still rejects"
            );
            assert!(
                !hits[2].contains(&point(px(145.), px(60.))),
                "own rectangle still clips"
            );
            assert_eq!(
                hits[2].visual_transform.unmap_point(outside_round),
                point(px(70.), px(38.333332))
            );
        };
        handle
            .update(cx, |_, w, _| verify(w, false))
            .expect("default");
        handle
            .update(cx, |root, _, cx| {
                root.0.update(cx, |panel, cx| {
                    panel.escape = true;
                    cx.notify();
                })
            })
            .expect("opt in");
        cx.run_until_parked();
        let count = renders.get();
        for _ in 0..2 {
            handle
                .update(cx, |_, _, cx| cx.notify())
                .expect("root redraw");
            cx.run_until_parked();
            handle
                .update(cx, |_, w, _| verify(w, true))
                .expect("retained escape");
        }
        assert_eq!(renders.get(), count, "cached subtree reused");
        cx.activate_accessibility(handle.into());
        cx.run_until_parked();
        handle
            .update(cx, |_, w, _| {
                let tree: serde_json::Value =
                    serde_json::from_str(&w.debug_a11y_tree_json().expect("a11y")).expect("json");
                let nodes = tree["nodes"].as_object().expect("nodes");
                let key = |id: &str| {
                    nodes
                        .iter()
                        .find(|(_, n)| n["element_id"] == id)
                        .expect("named node")
                        .0
                };
                let owner = key("Name(\"owner\")");
                let escaped = key("Name(\"escaped\")");
                assert!(
                    nodes[owner]["children"]
                        .as_array()
                        .expect("children")
                        .contains(&serde_json::json!(escaped)),
                    "escape does not reset logical ancestry"
                );
                let escaped_id = crate::accesskit::NodeId(
                    nodes[escaped]["accesskit_id"]
                        .as_str()
                        .expect("node id")
                        .parse()
                        .expect("numeric id"),
                );
                assert_eq!(
                    w.a11y.node_bounds.get(&escaped_id),
                    Some(&crate::Bounds::new(
                        point(px(20.), px(14.5)),
                        size(px(120.), px(90.))
                    )),
                    "a11y exposes displayed inner clip, not ancestor clip"
                );
            })
            .expect("accessibility");
    }

    /// A stand-in for a dock panel hosting a popover (deferred draw) whose
    /// content opens another popover (a deferred draw created while
    /// prepainting the first one's content).
    struct PanelView;

    impl Render for PanelView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("panel")
                .role(crate::Role::Group)
                .key_context("Panel")
                .size_full()
                .child(
                    deferred(
                        anchored().position(point(px(10.), px(10.))).child(
                            div()
                                .id("popover")
                                .role(crate::Role::Group)
                                .key_context("Popover")
                                .w(px(200.))
                                .h(px(200.))
                                .child(
                                    deferred(
                                        anchored().position(point(px(30.), px(30.))).child(
                                            div()
                                                .id("nested-menu")
                                                .role(crate::Role::Menu)
                                                .key_context("NestedMenu")
                                                .debug_selector(|| "NESTED_MENU".into())
                                                .w(px(50.))
                                                .h(px(50.)),
                                        ),
                                    )
                                    .preserve_accessibility()
                                    .with_priority(2),
                                ),
                        ),
                    )
                    .preserve_accessibility()
                    .with_priority(1),
                )
        }
    }

    struct RootView {
        panel: Entity<PanelView>,
    }

    impl Render for RootView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().key_context("Root").size_full().child(
                self.panel
                    .clone()
                    .cached(StyleRefinement::default().size_full()),
            )
        }
    }

    /// Regression test for a crash with nested deferred draws (e.g. a popover
    /// menu inside a popover hosted by a cached dock panel). Prepaint indices
    /// recorded during the deferred draw rounds must index the same
    /// `deferred_draws` vector that `reuse_prepaint` slices on the next frame;
    /// previously they were measured against a transient per-round vector, so
    /// reusing the panel's subtree grafted the wrong deferred draws and
    /// panicked in the dispatch tree.
    #[gpui::test]
    fn test_nested_deferred_draws_with_reused_views(cx: &mut TestAppContext) {
        let window = cx.open_window(size(px(800.), px(600.)), |_, cx| {
            let panel = cx.new(|_| PanelView);
            RootView { panel }
        });
        cx.run_until_parked();

        let menu_bounds = window
            .update(cx, |_, window, _| {
                window
                    .rendered_frame
                    .debug_bounds
                    .get("NESTED_MENU")
                    .copied()
            })
            .expect("required framework invariant must hold")
            .expect("NESTED_MENU debug bounds not found");
        assert_eq!(menu_bounds.size, size(px(50.), px(50.)));

        // Re-render only the root view; the panel is cached, so its subtree -
        // including both deferred draw records - is reused from the previous
        // frame.
        window
            .update(cx, |_, _, cx| cx.notify())
            .expect("required framework invariant must hold");
        cx.run_until_parked();

        // Reuse the subtree a second time, exercising ranges that were
        // themselves recorded during a reused frame.
        window
            .update(cx, |_, _, cx| cx.notify())
            .expect("required framework invariant must hold");
        cx.run_until_parked();

        // Re-render the panel itself again to prove the popovers still draw.
        window
            .update(cx, |root, _, cx| {
                root.panel.update(cx, |_, cx| cx.notify());
            })
            .expect("required framework invariant must hold");
        cx.run_until_parked();

        window
            .update(cx, |_, window, _| {
                assert_eq!(window.rendered_frame.deferred_draws.len(), 2);
                assert!(
                    window
                        .rendered_frame
                        .debug_bounds
                        .contains_key("NESTED_MENU")
                );
            })
            .expect("required framework invariant must hold");

        // Activation must invalidate the visual-only cache, and subsequent
        // root-only notifications must not reuse stale frame-local a11y
        // reservations or drop the cached panel's accessibility subtree.
        cx.activate_accessibility(window.into());
        for _ in 0..3 {
            window
                .update(cx, |_, _, cx| cx.notify())
                .expect("notify root");
            cx.run_until_parked();
            window
                .update(cx, |_, window, _| {
                    let tree: serde_json::Value = serde_json::from_str(
                        &window.debug_a11y_tree_json().expect("active accessibility"),
                    )
                    .expect("accessibility tree JSON");
                    let nodes = tree["nodes"].as_object().expect("nodes");
                    let key = |id: &str| {
                        nodes
                            .iter()
                            .find(|(_, node)| node["element_id"] == id)
                            .expect("deferred accessibility node")
                            .0
                    };
                    let panel = key("Name(\"panel\")");
                    let popover = key("Name(\"popover\")");
                    let menu = key("Name(\"nested-menu\")");
                    assert_eq!(nodes[panel]["children"], serde_json::json!([popover]));
                    assert_eq!(nodes[popover]["children"], serde_json::json!([menu]));
                })
                .expect("inspect deferred accessibility");
        }
    }
}
