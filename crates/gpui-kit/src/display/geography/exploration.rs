//! Transient capture state. Camera proposals never become displayed state
//! until the caller accepts them and supplies a new viewport.
use super::*;
use gpui::{App, Bounds, Div, MouseButton, Pixels, Point, Stateful, Window, canvas, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, Copy)]
struct Gesture {
    origin: GeoViewport,
    camera: GeoViewport,
    start: Point<Pixels>,
    last: Point<Pixels>,
    moved: bool,
}

#[derive(Default)]
pub(super) struct Exploration {
    gesture: Option<Gesture>,
    pub(super) direct: bool,
    pub(super) hover: Option<Point<Pixels>>,
    source: Option<std::rc::Weak<GeoData>>,
    size: [f64; 2],
}

impl Exploration {
    fn begin(&mut self, camera: GeoViewport, at: Point<Pixels>) {
        self.gesture = Some(Gesture {
            origin: camera,
            camera,
            start: at,
            last: at,
            moved: false,
        });
        self.hover = None;
    }
    fn pan(&mut self, at: Point<Pixels>, size: [f64; 2]) -> Option<GeoViewport> {
        let gesture = self.gesture.as_mut()?;
        if size[0].min(size[1]) <= 0.0 {
            return None;
        }
        let dx = f64::from(f32::from(at.x - gesture.last.x));
        let dy = f64::from(f32::from(at.y - gesture.last.y));
        gesture.moved |=
            f32::from(at.x - gesture.start.x).hypot(f32::from(at.y - gesture.start.y)) >= 3.0;
        if !gesture.moved {
            return None;
        }
        gesture.last = at;
        let scale = size[0].min(size[1]) * gesture.camera.zoom;
        gesture.camera = gesture.camera.pan(-dx / scale, -dy / scale);
        self.direct = true;
        Some(gesture.camera)
    }
    pub(super) fn cancel(&mut self) -> Option<GeoViewport> {
        let gesture = self.gesture.take()?;
        self.direct = true;
        gesture.moved.then_some(gesture.origin)
    }
    fn interrupt(&mut self, window: &mut Window) {
        if self.gesture.take().is_some() {
            window.release_pointer();
        }
        self.hover = None;
        self.direct = true;
    }
    pub(super) fn sync(
        &mut self,
        data: Option<&Rc<GeoData>>,
        size: [f64; 2],
        interactive: bool,
        window: &mut Window,
    ) {
        let same = self
            .source
            .as_ref()
            .and_then(std::rc::Weak::upgrade)
            .zip(data)
            .is_some_and(|(old, new)| Rc::ptr_eq(&old, new));
        if !same || self.size != size || (!interactive && self.active()) {
            self.interrupt(window);
        }
        self.source = data.map(Rc::downgrade);
        self.size = size;
    }
    pub(super) fn active(&self) -> bool {
        self.gesture.is_some()
    }
}

pub(super) type Report = Rc<dyn Fn(GeoEvent, &mut Window, &mut App)>;

pub(super) fn install(
    mut frame: Stateful<Div>,
    state: Rc<RefCell<Exploration>>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    camera: GeoViewport,
    data: Rc<presentation::Frame>,
    report: Report,
    gesture_id: gpui::ElementId,
) -> Stateful<Div> {
    let down = state.clone();
    frame = frame.on_mouse_down_with_pointer_capture(MouseButton::Left, move |event, _, _| {
        down.borrow_mut().begin(camera, event.position);
    });
    let moved = state.clone();
    let move_bounds = bounds.clone();
    let move_report = report.clone();
    frame = frame.on_mouse_move(move |event, window, cx| {
        if moved.borrow().gesture.is_some() {
            if event.pressed_button != Some(MouseButton::Left) {
                return;
            }
            let next = moved
                .borrow_mut()
                .pan(event.position, extent(move_bounds.get()));
            if let Some(next) = next {
                move_report(GeoEvent::Viewport(next), window, cx);
            }
        } else {
            moved.borrow_mut().hover = Some(event.position);
        }
        window.refresh();
    });
    let up = state.clone();
    let up_bounds = bounds.clone();
    let up_report = report.clone();
    frame = frame.on_mouse_up(MouseButton::Left, move |event, window, cx| {
        let gesture = up.borrow_mut().gesture.take();
        if let Some(gesture) = gesture
            && !gesture.moved
        {
            let bounds = up_bounds.get();
            if bounds.contains(&event.position) {
                up_report(
                    GeoEvent::Select(data.hit_test(
                        camera,
                        extent(bounds),
                        local(event.position, bounds),
                    )),
                    window,
                    cx,
                );
            }
        }
        window.refresh();
    });
    let cancelled = state.clone();
    let cancel_report = report.clone();
    frame = frame.child(crate::interaction::on_pointer_cancel(move |window, cx| {
        let restore = cancelled.borrow_mut().cancel();
        if let Some(camera) = restore {
            cancel_report(GeoEvent::Viewport(camera), window, cx);
        }
        window.refresh();
    }));
    frame
        .on_hover({
            let state = state.clone();
            move |inside, window, _| {
                if !inside {
                    state.borrow_mut().hover = None;
                    window.refresh();
                }
            }
        })
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let visible = bounds.intersect(&window.content_mask().bounds);
                    let pan = state.clone();
                    let pan_report = report.clone();
                    window.on_touch_pan(gesture_id.clone(), move |event, window, cx| {
                        match event.phase {
                            gpui::TouchPhase::Started => {
                                if !visible.contains(&event.touch_start_position) {
                                    return;
                                }
                                window.prevent_default();
                                pan.borrow_mut().begin(camera, event.start_position);
                            }
                            gpui::TouchPhase::Cancelled => {
                                let restore = pan.borrow_mut().cancel();
                                if let Some(camera) = restore {
                                    pan_report(GeoEvent::Viewport(camera), window, cx);
                                }
                                window.refresh();
                                return;
                            }
                            _ => {}
                        }
                        let next = pan.borrow_mut().pan(event.position, extent(bounds));
                        if let Some(camera) = next {
                            pan_report(GeoEvent::Viewport(camera), window, cx);
                        }
                        if event.phase == gpui::TouchPhase::Ended {
                            pan.borrow_mut().gesture = None;
                        }
                        window.refresh();
                    });
                    let pinch = state.clone();
                    let pinch_report = report.clone();
                    window.on_touch_pinch(gesture_id.clone(), move |event, window, cx| {
                        if event.phase == gpui::TouchPhase::Started {
                            if !visible.contains(&event.position) {
                                return;
                            }
                            window.prevent_default();
                            pinch.borrow_mut().begin(camera, event.position);
                        }
                        if event.phase == gpui::TouchPhase::Cancelled {
                            let restore = pinch.borrow_mut().cancel();
                            if let Some(camera) = restore {
                                pinch_report(GeoEvent::Viewport(camera), window, cx);
                            }
                            window.refresh();
                            return;
                        }
                        let next = {
                            let mut state = pinch.borrow_mut();
                            state.direct = true;
                            state.gesture.as_mut().map(|g| {
                                let size = extent(bounds);
                                let old_anchor = g.camera.world(local(g.last, bounds), size);
                                let mut next =
                                    g.camera.zoom_at(1.0 + f64::from(event.delta), old_anchor);
                                let desired = next.world(local(event.position, bounds), size);
                                next = next.pan(old_anchor.x - desired.x, old_anchor.y - desired.y);
                                g.camera = next;
                                g.last = event.position;
                                g.moved = true;
                                next
                            })
                        };
                        if let Some(camera) = next {
                            pinch_report(GeoEvent::Viewport(camera), window, cx);
                        }
                        if event.phase == gpui::TouchPhase::Ended {
                            pinch.borrow_mut().gesture = None;
                        }
                        window.refresh();
                    });
                },
            )
            .absolute()
            .inset_0()
            .size_full(),
        )
}

pub(super) fn extent(bounds: Bounds<Pixels>) -> [f64; 2] {
    [
        f64::from(f32::from(bounds.size.width)),
        f64::from(f32::from(bounds.size.height)),
    ]
}
pub(super) fn local(at: Point<Pixels>, bounds: Bounds<Pixels>) -> [f64; 2] {
    [
        f64::from(f32::from(at.x - bounds.left())),
        f64::from(f32::from(at.y - bounds.top())),
    ]
}

impl crate::motion::Interpolate for GeoViewport {
    fn lerp(self, other: Self, t: f32) -> Self {
        // A camera is bounded even if a caller samples an overshooting curve.
        let t = t.clamp(0.0, 1.0);
        if t == 0.0 {
            return self;
        }
        if t == 1.0 {
            return other;
        }
        Self {
            center: GeoProjected {
                x: self.center.x.lerp(other.center.x, t),
                y: self.center.y.lerp(other.center.y, t),
            },
            zoom: self.zoom.ln().lerp(other.zoom.ln(), t).exp(),
        }
    }
    fn distance(self, other: Self) -> f32 {
        ((self.center.x - other.center.x).hypot(self.center.y - other.center.y)
            + (self.zoom.ln() - other.zoom.ln()).abs()) as f32
    }
}
