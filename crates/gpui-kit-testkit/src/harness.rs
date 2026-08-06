//! A headless window that renders components and publishes their semantics.
//!
//! Tests assert against the semantic snapshot and simulate real input, never
//! against source text or private component state.

use gpui::{
    AnyElement, AnyWindowHandle, App, Bounds, Context, IntoElement, Modifiers, Pixels, Point,
    Render, TestAppContext, VisualTestContext, Window, WindowBounds, WindowOptions, div, point,
    prelude::*, px, size,
};
use gpui_kit_semantics::{Node, SemanticRegistry, Snapshot};

type Build = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

struct Scene {
    build: Build,
}

impl Render for Scene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SemanticRegistry::global(cx).begin_frame();
        let content = (self.build)(window, cx);
        div().size_full().child(content)
    }
}

/// One window under test.
pub struct Harness {
    cx: VisualTestContext,
    window: AnyWindowHandle,
}

impl std::fmt::Debug for Harness {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Harness").finish()
    }
}

impl Harness {
    /// Opens a window rendering `build`.
    ///
    /// `install` runs before the first frame; callers pass their library's
    /// install function so the theme and semantic registry exist.
    pub fn new(
        cx: &mut TestAppContext,
        install: impl FnOnce(&mut App) + 'static,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        cx.update(install);
        let handle = cx.add_window(|_, _| Scene {
            build: Box::new(build),
        });
        let visual = VisualTestContext::from_window(handle.into(), cx);
        visual.run_until_parked();
        Self {
            window: handle.into(),
            cx: visual,
        }
    }

    pub fn window(&self) -> AnyWindowHandle {
        self.window
    }

    pub fn context(&mut self) -> &mut VisualTestContext {
        &mut self.cx
    }

    /// Redraws and returns the semantic tree published by the latest frame.
    pub fn snapshot(&mut self) -> Snapshot {
        self.cx.run_until_parked();
        self.cx
            .update(|_, cx| SemanticRegistry::global(cx).snapshot())
    }

    pub fn node(&mut self, id: &str) -> Option<Node> {
        self.snapshot().find(id).cloned()
    }

    /// Clicks the center of a semantic node.
    ///
    /// Panics when the node is missing, because a test that silently clicks
    /// nothing would report a false pass.
    pub fn click(&mut self, id: &str) {
        let node = self
            .node(id)
            .unwrap_or_else(|| panic!("semantic node `{id}` is missing"));
        let center = node.bounds.center();
        self.cx
            .simulate_click(point(px(center.0), px(center.1)), Modifiers::none());
        self.cx.run_until_parked();
    }

    pub fn bounds(&mut self, id: &str) -> Option<Bounds<Pixels>> {
        self.node(id).map(|node| {
            Bounds::new(
                point(px(node.bounds.x), px(node.bounds.y)),
                size(px(node.bounds.width), px(node.bounds.height)),
            )
        })
    }

    pub fn keystrokes(&mut self, keystrokes: &str) {
        self.cx.simulate_keystrokes(keystrokes);
        self.cx.run_until_parked();
    }

    pub fn point_in(&mut self, id: &str) -> Point<Pixels> {
        let node = self
            .node(id)
            .unwrap_or_else(|| panic!("semantic node `{id}` is missing"));
        let center = node.bounds.center();
        point(px(center.0), px(center.1))
    }
}

fn _assert_unused(_: WindowOptions, _: WindowBounds) {}
