//! A headless window that renders components and publishes their semantics.
//!
//! Tests assert against the semantic snapshot and simulate real input, never
//! against source text or private component state.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, AnyWindowHandle, App, Bounds, Context, IntoElement, Modifiers, MouseButton, Pixels,
    Point, Render, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase, VisualTestContext,
    Window, WindowBounds, WindowOptions, div, point, prelude::*, px, size,
};
use gpui_kit_semantics::{DiagnosticArm, Node, SemanticCoordinator, Snapshot};

type Build = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

struct Scene {
    build: Rc<RefCell<Build>>,
}

impl Render for Scene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SemanticCoordinator::global(cx).begin_frame(window);
        let content = (self.build.borrow())(window, cx);
        div().size_full().child(content)
    }
}

/// One window under test.
pub struct Harness {
    cx: VisualTestContext,
    window: AnyWindowHandle,
    build: Rc<RefCell<Build>>,
    _diagnostics: DiagnosticArm,
    /// Where the simulated pointer is, so a drag can be driven one step at a
    /// time without a test having to carry the position itself.
    pointer: Point<Pixels>,
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
        let diagnostics = cx.update(|cx| SemanticCoordinator::global(cx).arm());
        let build = Rc::new(RefCell::new(Box::new(build) as Build));
        let handle = cx.add_window({
            let build = Rc::clone(&build);
            move |_, _| Scene { build }
        });
        cx.activate_accessibility(handle.into());
        let visual = VisualTestContext::from_window(handle.into(), cx);
        visual.run_until_parked();
        Self {
            window: handle.into(),
            cx: visual,
            build,
            _diagnostics: diagnostics,
            pointer: Point::default(),
        }
    }

    /// Replaces the tree this window is rendering, without opening another one.
    ///
    /// Catalog audits walk every scene on one `TestAppContext`. Opening a
    /// window per scene repeats install work the first `new` already did.
    pub fn remount(&mut self, build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) {
        *self.build.borrow_mut() = Box::new(build);
        self.frame();
    }

    pub fn window(&self) -> AnyWindowHandle {
        self.window
    }

    pub fn context(&mut self) -> &mut VisualTestContext {
        &mut self.cx
    }

    /// Redraws and returns the semantic tree published by the latest frame.
    pub fn snapshot(&mut self) -> Snapshot {
        // A forced successful frame is the authority. Besides settling normal
        // updates, this deliberately rebuilds cached GPUI views so diagnostic
        // probes and the committed AccessKit tree describe the same draw.
        self.frame();
        self.cx.update(|window, cx| {
            SemanticCoordinator::global(cx)
                .snapshot(window.window_handle().window_id())
                .expect("this window published a semantic frame")
        })
    }

    /// Returns the diagnostic semantic tree from the most recently completed
    /// frame without scheduling another draw.
    pub fn current_snapshot(&mut self) -> Snapshot {
        self.cx.update(|window, cx| {
            SemanticCoordinator::global(cx)
                .snapshot(window.window_handle().window_id())
                .expect("this window published a semantic frame")
        })
    }

    pub fn node(&mut self, id: &str) -> Option<Node> {
        self.snapshot().find(id).cloned()
    }

    /// Draws and returns GPUI's deterministic AccessKit tree.
    pub fn accessibility_tree(&mut self) -> serde_json::Value {
        self.frame();
        self.cx.update(|window, _| {
            serde_json::from_str(
                &window
                    .debug_a11y_tree_json()
                    .expect("accessibility adapter is active"),
            )
            .expect("valid accessibility tree JSON")
        })
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

    /// Mutates application state the way a host would, then settles the window
    /// so the next snapshot reflects the change.
    pub fn update<R>(&mut self, update: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        let result = self.cx.update(|window, cx| update(window, cx));
        self.cx.run_until_parked();
        result
    }

    /// Advances the simulated clock and delivers one animation frame.
    ///
    /// Tests have no platform frame loop, so motion driven by
    /// `Window::request_animation_frame` only progresses when a test asks for
    /// the next frame explicitly.
    /// Draws one frame without advancing the clock.
    ///
    /// Motion keyed by identity is measured and pruned while rendering, so a
    /// test that asserts what a component learned from the last layout needs
    /// another frame rather than more time.
    pub fn frame(&mut self) {
        self.cx.update(|_, cx| cx.refresh_windows());
        self.cx.run_until_parked();
    }

    /// Returns GPUI's structural counters for the most recently completed
    /// frame without causing another draw.
    pub fn frame_stats(&mut self) -> gpui::FrameStats {
        self.cx.update(|window, _| window.frame_stats())
    }

    pub fn advance(&mut self, delta: Duration) {
        self.cx.executor().advance_clock(delta);
        self.cx.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
        self.cx.run_until_parked();
    }

    /// Turns a wheel over the centre of a node. A positive `pixels` scrolls
    /// toward the end of the content, the way a reader moves down a list.
    ///
    /// A virtualized surface decides what to build from where it is scrolled
    /// to, so a test that wants to assert what is on screen has to move it the
    /// way a reader would rather than by reaching into the element.
    pub fn scroll(&mut self, id: &str, pixels: f32) {
        let at = self.point_in(id);
        self.cx.simulate_event(ScrollWheelEvent {
            position: at,
            delta: ScrollDelta::Pixels(point(px(0.0), px(-pixels))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
        self.cx.run_until_parked();
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

    /// A point `fraction` of the way down a node, horizontally centred.
    ///
    /// A drop target offers different slots at different heights, so a test
    /// that wants one of them has to aim at it.
    pub fn point_down(&mut self, id: &str, fraction: f32) -> Point<Pixels> {
        let node = self
            .node(id)
            .unwrap_or_else(|| panic!("semantic node `{id}` is missing"));
        point(
            px(node.bounds.x + node.bounds.width / 2.0),
            px(node.bounds.y + node.bounds.height * fraction),
        )
    }

    /// A point `fraction` of the way across a node, vertically centred.
    pub fn point_across(&mut self, id: &str, fraction: f32) -> Point<Pixels> {
        let node = self
            .node(id)
            .unwrap_or_else(|| panic!("semantic node `{id}` is missing"));
        point(
            px(node.bounds.x + node.bounds.width * fraction),
            px(node.bounds.y + node.bounds.height / 2.0),
        )
    }

    /// Where the simulated pointer currently is.
    pub fn pointer(&self) -> Point<Pixels> {
        self.pointer
    }

    /// Presses on a node and travels far enough for the press to become a
    /// drag.
    ///
    /// GPUI only calls a press a drag once it has moved past its own
    /// threshold, so a test that pressed and released without moving would be
    /// testing a click.
    pub fn drag_start(&mut self, id: &str) {
        let from = self.point_in(id);
        self.cx
            .simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
        self.cx.run_until_parked();
        self.pointer = from;
        self.drag_to(from + point(px(4.0), px(4.0)));
    }

    /// Moves the pointer, with the button still down.
    pub fn drag_to(&mut self, position: Point<Pixels>) {
        self.pointer = position;
        self.cx
            .simulate_mouse_move(position, MouseButton::Left, Modifiers::none());
        self.cx.run_until_parked();
    }

    /// Moves the pointer to the middle of a node.
    pub fn drag_over(&mut self, id: &str) {
        let over = self.point_in(id);
        self.drag_to(over);
    }

    /// Releases wherever the pointer is.
    pub fn drop_here(&mut self) {
        let at = self.pointer;
        self.cx
            .simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        self.cx.run_until_parked();
    }

    /// Abandons a drag in flight.
    pub fn cancel_drag(&mut self) {
        self.keystrokes("escape");
    }

    /// The whole gesture: press on `from`, travel to the middle of `to`, and
    /// let go.
    pub fn drag(&mut self, from: &str, to: &str) {
        self.drag_start(from);
        self.drag_over(to);
        self.drop_here();
    }
}

fn _assert_unused(_: WindowOptions, _: WindowBounds) {}
