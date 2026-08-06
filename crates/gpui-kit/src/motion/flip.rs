//! Sliding an element from where it was to where it now is.
//!
//! FLIP is the standard trick for animating a layout change: read where the
//! element was (First), let layout put it where it belongs (Last), Invert that
//! difference into a visual offset, and Play the offset back to zero. Nothing
//! about the layout changes — a reordered row lands in its new slot on the
//! frame the caller reorders it, and only the pixels take their time.
//!
//! ```no_run
//! # use gpui::{App, Window, div, prelude::*};
//! # use gpui_kit::motion::{Flipping, flip};
//! # fn row(id: &'static str, window: &mut Window, cx: &mut App) -> impl IntoElement {
//! let handle = flip(id, cx);
//! div().child("Row").flip(&handle, window, cx)
//! # }
//! ```
//!
//! The offset is applied during prepaint through [`gpui::Window::with_element_offset`],
//! after layout has already run, so it cannot move a sibling. The origin the
//! element is measured against excludes the ambient element offset, so
//! scrolling a list — which offsets every row at once — is not mistaken for a
//! reorder.

use std::cell::RefCell;
use std::panic::Location;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Point, SharedString, Window, px,
};
use gpui_kit_theme::{ActiveTheme, SpringPreset};
use web_time::Instant;

use super::keyed;
use super::{Interpolate, Spring};

/// How far an origin has to move before it counts as a move.
///
/// Sub-pixel drift would otherwise start an animation on every frame forever.
const EPSILON: f32 = 0.5;

#[derive(Default)]
struct FlipState {
    /// Where layout last put the element, with the ambient offset removed.
    origin: Option<Point<Pixels>>,
    /// The offset the current slide started from.
    from: Point<Pixels>,
    /// What is on screen right now.
    current: Point<Pixels>,
    elapsed: Duration,
    last_frame: Option<Instant>,
}

impl FlipState {
    fn advance(&mut self, now: Instant) {
        if let Some(last) = self.last_frame {
            self.elapsed += now.saturating_duration_since(last);
        }
        self.last_frame = Some(now);
    }

    fn sample(&self, spring: Spring, settle: Duration) -> Point<Pixels> {
        if self.elapsed >= settle {
            return Point::default();
        }
        self.from.lerp(Point::default(), spring.value(self.elapsed))
    }

    /// Records where layout put the element, and inverts a move into an offset.
    ///
    /// A move that arrives mid-slide continues from the offset on screen
    /// rather than restarting, so a list reordered twice in quick succession
    /// does not jump.
    fn record(&mut self, origin: Point<Pixels>, residual: Point<Pixels>) {
        let moved = |a: Pixels, b: Pixels| (f32::from(a) - f32::from(b)).abs() > EPSILON;
        if let Some(previous) = self.origin
            && (moved(previous.x, origin.x) || moved(previous.y, origin.y))
        {
            self.from = previous - origin + residual;
            self.elapsed = Duration::ZERO;
        }
        self.origin = Some(origin);
    }
}

/// A handle to one element's slide, keyed by semantic id.
///
/// Cheap to clone and to rebuild: the state lives in an application global, so
/// both a `RenderOnce` builder and a `Render` view can ask for the same handle
/// on every frame.
#[derive(Clone)]
pub struct Flip {
    id: SharedString,
    state: Rc<RefCell<FlipState>>,
}

impl std::fmt::Debug for Flip {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Flip")
            .field("id", &self.id)
            .field("offset", &self.offset())
            .finish()
    }
}

impl Flip {
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// The offset currently painted, in pixels.
    ///
    /// This is where the element is drawn, not where it is: the layout, the
    /// hit target and the semantic tree all report the settled position. It
    /// exists so a test or an inspector can watch a slide without treating a
    /// value in flight as a fact about the interface.
    pub fn offset(&self) -> Point<Pixels> {
        self.state.borrow().current
    }

    pub fn is_animating(&self) -> bool {
        let offset = self.offset();
        offset.x.abs() > px(EPSILON) || offset.y.abs() > px(EPSILON)
    }
}

/// The slide handle for `id`.
pub fn flip(id: impl Into<SharedString>, cx: &mut App) -> Flip {
    let id = id.into();
    let state = keyed::slot::<FlipState>(&id, cx);
    Flip { id, state }
}

/// The ids the flip global currently retains.
///
/// An id that stops rendering is dropped within two frames, so this is the set
/// of elements on screen rather than every element ever flipped.
pub fn tracked_ids(cx: &App) -> Vec<SharedString> {
    keyed::ids::<FlipState>(cx)
}

/// Slides an element from where it was to where it is.
pub trait Flipping: IntoElement + Sized {
    /// Wraps the element so a change in its position is played back as a
    /// slide.
    ///
    /// Under reduced motion the element is simply at its new place from the
    /// first frame.
    fn flip(self, flip: &Flip, window: &mut Window, cx: &mut App) -> Flipped {
        let spring = Spring::preset(cx.theme(), SpringPreset::Grab);
        let element = Flipped {
            element: self.into_any_element(),
            state: Rc::clone(&flip.state),
            spring,
            settle: spring.settle_time(),
            reduce_motion: cx.reduce_motion(),
        };
        // A slide that is still running needs the next frame even when nothing
        // else on the window asks for one.
        if flip.is_animating() {
            window.request_animation_frame();
        }
        element
    }
}

impl<E: IntoElement> Flipping for E {}

/// An element painted at an offset from where layout put it.
pub struct Flipped {
    element: gpui::AnyElement,
    state: Rc<RefCell<FlipState>>,
    spring: Spring,
    settle: Duration,
    reduce_motion: bool,
}

impl std::fmt::Debug for Flipped {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Flipped")
            .field("offset", &self.state.borrow().current)
            .field("reduce_motion", &self.reduce_motion)
            .finish()
    }
}

impl IntoElement for Flipped {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Flipped {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        // The wrapper contributes no box of its own: it hands layout the same
        // node the wrapped element asked for.
        (self.element.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let origin = bounds.origin - window.element_offset();
        let offset = {
            let mut state = self.state.borrow_mut();
            if self.reduce_motion {
                state.origin = Some(origin);
                state.from = Point::default();
                state.elapsed = self.settle;
                state.last_frame = None;
                state.current = Point::default();
            } else {
                state.advance(cx.background_executor().now());
                let residual = state.sample(self.spring, self.settle);
                state.record(origin, residual);
                state.current = state.sample(self.spring, self.settle);
                if state.elapsed >= self.settle {
                    state.last_frame = None;
                }
            }
            state.current
        };

        window.with_element_offset(offset, |window| {
            self.element.prepaint(window, cx);
        });

        if offset != Point::default() {
            window.request_animation_frame();
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.element.paint(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::point;
    use gpui_kit_theme::Theme;

    fn grab() -> Spring {
        Spring::preset(&Theme::studio_dark(), SpringPreset::Grab)
    }

    #[test]
    fn the_grab_spring_settles_sooner_than_the_snappy_one() {
        let snappy = Spring::preset(&Theme::studio_dark(), SpringPreset::Snappy);
        assert!(grab().settle_time() < snappy.settle_time());
    }

    #[test]
    fn a_first_measurement_produces_no_offset() {
        let mut state = FlipState::default();
        state.record(point(px(10.0), px(20.0)), Point::default());
        assert_eq!(state.sample(grab(), grab().settle_time()), Point::default());
    }

    #[test]
    fn a_move_inverts_into_the_distance_travelled() {
        let spring = grab();
        let settle = spring.settle_time();
        let mut state = FlipState::default();
        state.record(point(px(0.0), px(0.0)), Point::default());
        state.record(point(px(0.0), px(40.0)), Point::default());
        assert_eq!(state.sample(spring, settle), point(px(0.0), px(-40.0)));

        state.elapsed = settle;
        assert_eq!(state.sample(spring, settle), Point::default());
    }

    #[test]
    fn a_move_mid_slide_continues_from_what_is_on_screen() {
        let spring = grab();
        let settle = spring.settle_time();
        let mut state = FlipState::default();
        state.record(point(px(0.0), px(0.0)), Point::default());
        state.record(point(px(0.0), px(40.0)), Point::default());

        state.elapsed = settle / 2;
        let residual = state.sample(spring, settle);
        assert!(residual.y > px(-40.0) && residual.y < px(0.0));

        state.record(point(px(0.0), px(60.0)), residual);
        assert_eq!(
            state.sample(spring, settle),
            residual - point(px(0.0), px(20.0))
        );
    }

    #[test]
    fn sub_pixel_drift_does_not_start_a_slide() {
        let spring = grab();
        let settle = spring.settle_time();
        let mut state = FlipState::default();
        state.record(point(px(0.0), px(0.0)), Point::default());
        state.record(point(px(0.2), px(0.3)), Point::default());
        assert_eq!(state.sample(spring, settle), Point::default());
    }
}
