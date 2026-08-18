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
//!
//! # Position and size are not the same promise
//!
//! [`Flipping::flip`] animates position only, and that is free: an offset
//! applied after layout has no box to push, so nothing beside the element can
//! move.
//!
//! [`Flipping::flip_size`] additionally animates size, and that is **not**
//! free. The pinned GPUI revision has no transform for an element subtree —
//! `TransformationMatrix` reaches sprites alone — so a size animation cannot
//! be faked with a scale the way a browser does it. The element really is a
//! different size on every frame of the animation, which means it really does
//! move its siblings, and it owns a layout node of its own to do it. Ask for
//! it deliberately, on an element whose neighbours can stand being pushed.

use std::cell::RefCell;
use std::panic::Location;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId, Hsla, InspectorElementId,
    IntoElement, LayoutId, Pixels, Point, SharedString, Size, Style, Window, px, size,
};
use gpui_kit_theme::{ActiveTheme, SpringPreset};
use web_time::Instant;

use super::keyed;
use super::{Interpolate, MotionSpec, Spring, Transition};

/// How far an origin or an edge has to move before it counts as a move.
///
/// Sub-pixel drift would otherwise start an animation on every frame forever.
const EPSILON: f32 = 0.5;

/// How long a recorded rectangle is worth inverting from.
///
/// A shared element handed from one tree to another needs its rectangle to
/// outlive the gap where neither tree renders it. A rectangle older than this
/// is not a handoff, it is a memory, and flying in from where something stood
/// half a minute ago is worse than not animating at all.
const MEMORY: Duration = Duration::from_millis(500);

/// How many frames a shared id survives while nothing renders it.
///
/// Wall clock and frames bound the same gap from two directions, because an
/// idle window advances one and not the other; whichever runs out first ends
/// the handoff.
const HANDOFF_GRACE: u64 = 30;

/// The visual form an element is drawn with, as opposed to where it is.
///
/// A slide moves an element; it does not restyle it, because [`Flipped`]
/// wraps an `AnyElement` it did not build and cannot reach the radius or the
/// border inside it. So the caller keeps ownership of the shape and asks the
/// handle what to draw with this frame, the same way it asks a container for
/// its width. What comes back is a shape, not a style: applying it is
/// [`Shaping::shaped`], and everything else about the element stays the
/// caller's.
///
/// This is what a row becoming a card needs. The two forms differ by a radius,
/// a border and a surface, and cutting between them on the frame the layout
/// changes is the one part of that transition a reader actually notices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    pub radius: Pixels,
    pub border: Pixels,
    pub border_color: Hsla,
    pub background: Hsla,
}

impl Shape {
    /// A shape that is a surface and nothing else.
    pub fn surface(background: Hsla) -> Self {
        Self {
            radius: px(0.0),
            border: px(0.0),
            border_color: gpui::transparent_black(),
            background,
        }
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = px(radius);
        self
    }

    /// A border of `width` in `color`.
    ///
    /// A width of zero still carries its colour, so a shape animating from no
    /// border to one grows the line rather than fading a colour in from
    /// whatever happened to be behind it.
    pub fn border(mut self, width: f32, color: Hsla) -> Self {
        self.border = px(width);
        self.border_color = color;
        self
    }
}

impl Interpolate for Shape {
    fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            radius: self.radius.lerp(other.radius, t),
            border: self.border.lerp(other.border, t),
            border_color: self.border_color.lerp(other.border_color, t),
            background: self.background.lerp(other.background, t),
        }
    }

    fn distance(self, other: Self) -> f32 {
        self.radius.distance(other.radius)
            + self.border.distance(other.border)
            + self.border_color.distance(other.border_color)
            + self.background.distance(other.background)
    }
}

/// Applies a [`Shape`] to an element.
pub trait Shaping: gpui::Styled + Sized {
    fn shaped(self, shape: Shape) -> Self {
        self.rounded(shape.radius)
            .border(shape.border)
            .border_color(shape.border_color)
            .bg(shape.background)
    }
}

impl<E: gpui::Styled> Shaping for E {}

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
    /// The size the element is being drawn at, animating toward whatever size
    /// it would naturally take. `None` until an element opts into size.
    size: Option<Transition<Size<Pixels>>>,
    /// The size layout would give the element if nothing were animating.
    natural: Option<Size<Pixels>>,
    /// The space the parent last offered, so the natural size is measured
    /// against the same constraints the element would really be laid out in.
    available: Option<Size<AvailableSpace>>,
    /// Whether this frame's offer has been taken already.
    ///
    /// Layout asks a measured node its size several times, narrowing toward
    /// the size the node itself last reported. Only the first question of a
    /// frame is the parent saying what it has to give; taking a later one
    /// would measure the natural size against the animated size and stop the
    /// animation dead where it stood.
    offered: bool,
    /// When the rectangle was last recorded, for bounding a handoff.
    recorded_at: Option<Instant>,
    /// When the size transition was last advanced.
    size_frame: Option<Instant>,
    /// The shape the element is being drawn with, animating toward the shape
    /// the caller asked for. `None` until an element opts into shape.
    shape: Option<Transition<Shape>>,
    /// When the shape transition was last advanced.
    shape_frame: Option<Instant>,
    /// The frame an element last claimed this id on.
    seen_frame: Option<u64>,
    /// The last frame on which two elements were sharing this id, plus one.
    contested_through: Option<u64>,
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
        if let Some(previous) = self.origin
            && (moved(previous.x, origin.x) || moved(previous.y, origin.y))
        {
            self.from = previous - origin + residual;
            self.elapsed = Duration::ZERO;
        }
        self.origin = Some(origin);
    }

    /// Records the size layout would give the element, and returns the size to
    /// draw it at.
    ///
    /// The target keeps being measured while the animation forces a different
    /// size, so a target that changes mid-flight is noticed on the frame it
    /// changes. A change past [`EPSILON`] retargets the transition, which
    /// continues from the size on screen and carries the speed it already had.
    fn record_size(
        &mut self,
        natural: Size<Pixels>,
        spec: MotionSpec,
        now: Instant,
    ) -> Size<Pixels> {
        self.natural = Some(natural);
        let mut transition = self
            .size
            .unwrap_or_else(|| Transition::new(natural, spec))
            .spec(spec);
        if let Some(last) = self.size_frame {
            transition.advance(now.saturating_duration_since(last));
        }
        self.size_frame = Some(now);
        let target = transition.target();
        if moved(target.width, natural.width) || moved(target.height, natural.height) {
            transition.set(natural);
        }
        self.size = Some(transition);
        transition.value()
    }

    /// Records the shape the caller asked for, and returns the shape to draw
    /// with. Retargeting mid-flight continues from what is on screen, the way
    /// a size change does.
    fn record_shape(&mut self, target: Shape, spec: MotionSpec, now: Instant) -> Shape {
        let mut transition = self
            .shape
            .unwrap_or_else(|| Transition::new(target, spec))
            .spec(spec);
        if let Some(last) = self.shape_frame {
            transition.advance(now.saturating_duration_since(last));
        }
        self.shape_frame = Some(now);
        if transition.target() != target {
            transition.set(target);
        }
        self.shape = Some(transition);
        transition.value()
    }

    /// Puts the element at the shape asked for with nothing in flight.
    fn settle_shape(&mut self, target: Shape, spec: MotionSpec, now: Instant) -> Shape {
        let mut transition = self
            .shape
            .unwrap_or_else(|| Transition::new(target, spec))
            .spec(spec);
        transition.snap(target);
        self.shape = Some(transition);
        self.shape_frame = Some(now);
        transition.value()
    }

    /// Puts the element at its natural size with nothing in flight.
    fn settle_size(&mut self, natural: Size<Pixels>, spec: MotionSpec, now: Instant) {
        self.natural = Some(natural);
        let mut transition = self
            .size
            .unwrap_or_else(|| Transition::new(natural, spec))
            .spec(spec);
        transition.snap(natural);
        self.size = Some(transition);
        self.size_frame = Some(now);
    }

    /// Whether layout and paint already agree, so invert would be a no-op.
    fn is_home(&self, origin: Point<Pixels>, settle: Duration) -> bool {
        self.origin == Some(origin)
            && self.current == Point::default()
            && self.elapsed >= settle
            && self.size.is_none_or(|size| !size.is_animating())
            && self.shape.is_none_or(|shape| !shape.is_animating())
    }

    /// Puts the element where layout put it with nothing in flight.
    fn settle(&mut self, origin: Point<Pixels>, settle: Duration) {
        self.origin = Some(origin);
        self.from = Point::default();
        self.current = Point::default();
        self.elapsed = settle;
        self.last_frame = None;
    }

    /// Drops a rectangle too old to invert from, so the next measurement is
    /// read as a first one.
    fn forget_if_stale(&mut self, now: Instant) {
        let stale = self
            .recorded_at
            .is_some_and(|at| now.saturating_duration_since(at) > MEMORY);
        if stale {
            self.origin = None;
            self.from = Point::default();
            self.current = Point::default();
            self.elapsed = Duration::ZERO;
            self.last_frame = None;
            self.size = None;
            self.natural = None;
            self.size_frame = None;
            self.shape = None;
            self.shape_frame = None;
        }
        self.recorded_at = Some(now);
    }

    /// Claims the id for this frame, reporting whether another element already
    /// claimed it.
    ///
    /// Two elements sharing one slot would each read the other's rectangle as
    /// its own previous one and throw the other across the window, every
    /// frame, forever. Neither of them animates instead. The refusal outlasts
    /// the collision by a frame, because the frame after a contested one is
    /// the first that can record a rectangle nothing else is writing to, and
    /// inverting from a rectangle written by the loser of a fight is the same
    /// jump wearing a different shape.
    fn claim(&mut self, frame: Option<u64>) -> bool {
        let Some(frame) = frame else {
            return false;
        };
        if self.seen_frame == Some(frame) {
            self.contested_through = Some(frame + 1);
        }
        self.seen_frame = Some(frame);
        self.contested_through
            .is_some_and(|through| frame <= through)
    }
}

fn moved(a: Pixels, b: Pixels) -> bool {
    (f32::from(a) - f32::from(b)).abs() > EPSILON
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
            .field("size", &self.size())
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

    /// The size currently painted, once an element has opted into animating
    /// its size with [`Flipping::flip_size`].
    ///
    /// Unlike [`Flip::offset`], this one *is* what the layout and the semantic
    /// tree report, because a size animation is a real layout change.
    pub fn size(&self) -> Option<Size<Pixels>> {
        self.state.borrow().size.map(|size| size.value())
    }

    /// The shape to draw with this frame, animating toward `target`.
    ///
    /// The caller applies it with [`Shaping::shaped`]; the handle only says
    /// what the shape is right now. Under reduced motion the answer is
    /// `target` from the first frame.
    ///
    /// ```ignore
    /// let handle = flip(id, cx);
    /// let shape = handle.shape(if open { card } else { row }, window, cx);
    /// div().shaped(shape).flip(&handle, window, cx)
    /// ```
    pub fn shape(&self, target: Shape, window: &mut Window, cx: &App) -> Shape {
        let spring = Spring::preset(cx.theme(), SpringPreset::Grab);
        let spec = MotionSpec::sprung(spring);
        let now = Instant::now();
        let mut state = self.state.borrow_mut();
        let drawn = if cx.reduce_motion() {
            state.settle_shape(target, spec, now)
        } else {
            state.record_shape(target, spec, now)
        };
        if state.shape.is_some_and(|shape| shape.is_animating()) {
            window.request_animation_frame();
        }
        drawn
    }

    /// The shape currently painted, once an element has asked for one.
    pub fn drawn_shape(&self) -> Option<Shape> {
        self.state.borrow().shape.map(|shape| shape.value())
    }

    /// The size layout would give the element if nothing were animating.
    pub fn target_size(&self) -> Option<Size<Pixels>> {
        self.state.borrow().natural
    }

    /// Whether two elements are currently sharing this id.
    ///
    /// A contested id does not animate at all.
    pub fn is_contended(&self) -> bool {
        let state = self.state.borrow();
        match (state.seen_frame, state.contested_through) {
            (Some(frame), Some(through)) => frame <= through,
            _ => false,
        }
    }

    pub fn is_animating(&self) -> bool {
        let offset = self.offset();
        let sliding = offset.x.abs() > px(EPSILON) || offset.y.abs() > px(EPSILON);
        let state = self.state.borrow();
        sliding
            || state.size.is_some_and(|size| size.is_animating())
            || state.shape.is_some_and(|shape| shape.is_animating())
    }
}

/// The slide handle for `id`.
pub fn flip(id: impl Into<SharedString>, cx: &mut App) -> Flip {
    let id = id.into();
    let state = keyed::slot::<FlipState>(&id, cx);
    Flip { id, state }
}

/// The slide handle for `id`, kept alive across a handoff between two element
/// trees.
///
/// A row that opens into a detail panel is two elements in two trees with one
/// identity. Because the state is keyed by id rather than by element, the
/// panel inverts from the rectangle the row last recorded and travels there
/// instead of cutting. What this adds over [`flip`] is only patience: the
/// rectangle survives the frames in which neither tree renders the id, up to
/// 30 frames and 500 ms of wall clock, whichever ends first. Past that the
/// arriving element is simply already in place.
///
/// Both trees rendering the id at once is a collision, and a collision does
/// not animate; see [`Flip::is_contended`].
pub fn shared_flip(id: impl Into<SharedString>, cx: &mut App) -> Flip {
    let id = id.into();
    let state = keyed::slot_retained::<FlipState>(&id, HANDOFF_GRACE, cx);
    Flip { id, state }
}

/// The ids the flip global currently retains.
///
/// An id that stops rendering is dropped within two frames — or within 30
/// when it was last asked for through [`shared_flip`] — so this is the set
/// of elements on screen rather than every element ever flipped.
pub fn tracked_ids(cx: &App) -> Vec<SharedString> {
    keyed::ids::<FlipState>(cx)
}

/// Slides an element from where it was to where it is.
pub trait Flipping: IntoElement + Sized {
    /// Wraps the element so a change in its position is played back as a
    /// slide.
    ///
    /// The wrapper contributes no box of its own and the offset is applied
    /// after layout, so this cannot move anything beside the element.
    ///
    /// Under reduced motion the element is simply at its new place from the
    /// first frame.
    fn flip(self, flip: &Flip, window: &mut Window, cx: &mut App) -> Flipped {
        flipped(self, flip, false, window, cx)
    }

    /// The same, and a change in the element's size is played back too.
    ///
    /// This costs what [`Flipping::flip`] does not. The element takes a layout
    /// node of its own, is measured twice a frame while it animates, and is
    /// genuinely the animated size rather than a scaled picture of the settled
    /// one — so its siblings move with it, and the size the semantic tree
    /// publishes is the size in flight. Position and size run independently,
    /// so an element that both moves and grows does both.
    ///
    /// Under reduced motion the element is at its new size from the first
    /// frame.
    fn flip_size(self, flip: &Flip, window: &mut Window, cx: &mut App) -> Flipped {
        flipped(self, flip, true, window, cx)
    }
}

fn flipped<E: IntoElement>(
    element: E,
    flip: &Flip,
    sized: bool,
    window: &mut Window,
    cx: &mut App,
) -> Flipped {
    let spring = Spring::preset(cx.theme(), SpringPreset::Grab);
    let element = Flipped {
        element: element.into_any_element(),
        state: Rc::clone(&flip.state),
        spring,
        settle: spring.settle_time(),
        sized,
        measuring: false,
        measured_against: None,
        reduce_motion: cx.reduce_motion(),
        frame: keyed::frame_counter(cx),
    };
    // A slide that is still running needs the next frame even when nothing
    // else on the window asks for one.
    if flip.is_animating() {
        window.request_animation_frame();
    }
    element
}

impl<E: IntoElement> Flipping for E {}

/// An element painted at an offset from where layout put it, and optionally at
/// a size on its way to the size layout would give it.
pub struct Flipped {
    element: gpui::AnyElement,
    state: Rc<RefCell<FlipState>>,
    spring: Spring,
    settle: Duration,
    sized: bool,
    /// Whether this frame is one the element owns its layout node on. The
    /// first frame an id is ever seen on is not: see
    /// [`Element::request_layout`].
    measuring: bool,
    /// The constraints this frame's natural size was measured against, so
    /// prepaint can tell whether layout has since offered different ones.
    measured_against: Option<Size<AvailableSpace>>,
    reduce_motion: bool,
    frame: Option<u64>,
}

impl std::fmt::Debug for Flipped {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Flipped")
            .field("offset", &self.state.borrow().current)
            .field("sized", &self.sized)
            .field("reduce_motion", &self.reduce_motion)
            .finish()
    }
}

impl Flipped {
    fn spec(&self) -> MotionSpec {
        MotionSpec::sprung(self.spring)
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
        // A rectangle too old to invert from is dropped before anything is
        // measured against it, so the measurement that follows is read as a
        // first one rather than as the far end of a very long journey.
        let now = cx.background_executor().now();
        self.state.borrow_mut().forget_if_stale(now);

        if !self.sized {
            // The wrapper contributes no box of its own: it hands layout the
            // same node the wrapped element asked for.
            return (self.element.request_layout(window, cx), ());
        }

        // Measuring against the space the parent offered last frame needs a
        // frame in which the parent laid the element out for itself. Until
        // then the element is passed straight through, which is both the
        // correct first frame and where the constraints are learned: guessing
        // them instead would draw an element sized as a fraction of its
        // container at nothing at all.
        let Some(available) = self.state.borrow().available else {
            self.measuring = false;
            return (self.element.request_layout(window, cx), ());
        };
        self.measuring = true;
        self.measured_against = Some(available);

        // Measuring the natural size here rather than in prepaint is what lets
        // a size change be seen on the frame it happens: the node this element
        // hands to layout is already the first frame of the animation.
        let natural = self.element.layout_as_root(available, window, cx);
        let spec = self.spec();

        let drawn = {
            let mut state = self.state.borrow_mut();
            if self.reduce_motion {
                state.settle_size(natural, spec, now);
                natural
            } else {
                state.record_size(natural, spec, now)
            }
        };

        self.state.borrow_mut().offered = false;
        let state = Rc::clone(&self.state);
        let layout_id = window.request_measured_layout(
            Style::default(),
            move |known, available, _window, _cx| {
                let mut state = state.borrow_mut();
                if !state.offered {
                    state.offered = true;
                    state.available = Some(available);
                }
                drop(state);
                size(
                    known.width.unwrap_or(drawn.width),
                    known.height.unwrap_or(drawn.height),
                )
            },
        );
        (layout_id, ())
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
        let now = cx.background_executor().now();
        let offset = {
            let mut state = self.state.borrow_mut();
            let contested = state.claim(self.frame);
            if self.sized && !self.measuring {
                // The frame the parent laid this element out for itself: what
                // it came out as is both the size to animate from next time
                // and the constraints to measure the next one against.
                state.available = Some(size(
                    AvailableSpace::Definite(bounds.size.width),
                    AvailableSpace::Definite(bounds.size.height),
                ));
                state.settle_size(bounds.size, self.spec(), now);
            }
            if self.reduce_motion || contested {
                state.settle(origin, self.settle);
                if self.sized
                    && let Some(natural) = state.natural
                {
                    state.settle_size(natural, self.spec(), now);
                }
            } else if state.is_home(origin, self.settle) {
                // Same bounds and already at rest: skip invert and the next tick.
            } else {
                state.advance(now);
                let residual = state.sample(self.spring, self.settle);
                state.record(origin, residual);
                state.current = state.sample(self.spring, self.settle);
                if state.elapsed >= self.settle {
                    state.last_frame = None;
                }
            }
            state.current
        };

        if self.measuring {
            // The element owns its layout node, so it also owns where its
            // child sits: the slide is added to the origin rather than pushed
            // through the ambient element offset.
            self.element.prepaint_as_root(
                bounds.origin + offset,
                size(
                    AvailableSpace::Definite(bounds.size.width),
                    AvailableSpace::Definite(bounds.size.height),
                ),
                window,
                cx,
            );
        } else {
            window.with_element_offset(offset, |window| {
                self.element.prepaint(window, cx);
            });
        }

        let state = self.state.borrow();
        let growing = state.size.is_some_and(|size| size.is_animating());
        // Layout offered constraints this element has not been measured
        // against yet, which only the next frame can do. Nothing else on the
        // window knows that, so an element sized from a container that just
        // resized would otherwise wait for an unrelated frame to notice.
        let told_something_new = self.measuring && state.available != self.measured_against;
        drop(state);
        if offset != Point::default() || growing || told_something_new {
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
    use gpui::{point, size};
    use gpui_kit_theme::Theme;

    fn grab() -> Spring {
        Spring::preset(&Theme::studio_dark(), SpringPreset::Grab)
    }

    fn spec() -> MotionSpec {
        MotionSpec::sprung(grab())
    }

    /// A clock a test advances by hand, so a size animation is driven the way
    /// a window would drive it.
    struct Frames(Instant);

    impl Frames {
        fn new() -> Self {
            Self(Instant::now())
        }

        fn step(&mut self) -> Instant {
            self.0 += Duration::from_millis(8);
            self.0
        }
    }

    /// Keeps measuring the same size until the animation comes to rest.
    fn run_to_rest(
        state: &mut FlipState,
        natural: Size<Pixels>,
        frames: &mut Frames,
    ) -> Size<Pixels> {
        let mut drawn = natural;
        for _ in 0..600 {
            if !state.size.is_some_and(|size| size.is_animating()) {
                break;
            }
            drawn = state.record_size(natural, spec(), frames.step());
        }
        drawn
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

    #[test]
    fn a_first_size_is_drawn_at_once() {
        let mut frames = Frames::new();
        let mut state = FlipState::default();
        let first = size(px(100.0), px(40.0));
        assert_eq!(state.record_size(first, spec(), frames.step()), first);
        assert!(!state.size.expect("recorded").is_animating());
    }

    #[test]
    fn a_size_change_starts_at_the_old_size_and_lands_on_the_new_one() {
        let mut frames = Frames::new();
        let mut state = FlipState::default();
        state.record_size(size(px(100.0), px(40.0)), spec(), frames.step());
        let grown = size(px(200.0), px(80.0));
        let drawn = state.record_size(grown, spec(), frames.step());
        assert_eq!(
            drawn,
            size(px(100.0), px(40.0)),
            "the first frame of a resize is the size it had"
        );
        assert_eq!(run_to_rest(&mut state, grown, &mut frames), grown);
    }

    #[test]
    fn a_size_change_mid_animation_continues_from_the_size_on_screen() {
        let mut frames = Frames::new();
        let mut state = FlipState::default();
        state.record_size(size(px(100.0), px(40.0)), spec(), frames.step());
        let wider = size(px(200.0), px(40.0));
        state.record_size(wider, spec(), frames.step());

        let mut interrupted = size(px(100.0), px(40.0));
        let mut caught = frames.step();
        for _ in 0..6 {
            caught = frames.step();
            interrupted = state.record_size(wider, spec(), caught);
        }
        assert!(
            interrupted.width > px(100.0) && interrupted.width < px(200.0),
            "the animation has to be in flight for the claim to mean anything: {interrupted:?}"
        );

        // The same instant, so the only thing that could move the drawn size
        // is the retarget itself.
        let widest = size(px(300.0), px(40.0));
        let drawn = state.record_size(widest, spec(), caught);
        assert_eq!(
            drawn, interrupted,
            "a retarget starts from what is on screen rather than from the old size"
        );
        assert_eq!(run_to_rest(&mut state, widest, &mut frames), widest);
    }

    #[test]
    fn sub_pixel_size_churn_starts_nothing() {
        let mut frames = Frames::new();
        let mut state = FlipState::default();
        state.record_size(size(px(100.0), px(40.0)), spec(), frames.step());
        let drawn = state.record_size(size(px(100.3), px(40.2)), spec(), frames.step());
        assert_eq!(drawn, size(px(100.0), px(40.0)));
        assert!(!state.size.expect("recorded").is_animating());
    }

    #[test]
    fn a_settled_size_is_the_new_size_with_nothing_in_flight() {
        let mut frames = Frames::new();
        let mut state = FlipState::default();
        state.record_size(size(px(100.0), px(40.0)), spec(), frames.step());
        state.settle_size(size(px(200.0), px(80.0)), spec(), frames.step());
        let transition = state.size.expect("recorded");
        assert_eq!(transition.value(), size(px(200.0), px(80.0)));
        assert!(!transition.is_animating());
    }

    #[test]
    fn position_and_size_run_independently() {
        let mut frames = Frames::new();
        let spring = grab();
        let settle = spring.settle_time();
        let mut state = FlipState::default();
        state.record(point(px(0.0), px(0.0)), Point::default());
        state.record_size(size(px(100.0), px(40.0)), spec(), frames.step());

        state.record(point(px(0.0), px(40.0)), Point::default());
        let taller = size(px(100.0), px(90.0));
        let drawn = state.record_size(taller, spec(), frames.step());
        assert_eq!(state.sample(spring, settle), point(px(0.0), px(-40.0)));
        assert_eq!(drawn, size(px(100.0), px(40.0)));
        assert_eq!(run_to_rest(&mut state, taller, &mut frames), taller);
    }

    #[test]
    fn a_rectangle_older_than_the_handoff_window_is_not_inverted_from() {
        let start = Instant::now();
        let mut state = FlipState::default();
        state.forget_if_stale(start);
        state.record(point(px(0.0), px(0.0)), Point::default());
        state.record_size(size(px(100.0), px(40.0)), spec(), start);

        state.forget_if_stale(start + MEMORY / 2);
        state.record(point(px(0.0), px(300.0)), Point::default());
        assert_ne!(
            state.sample(grab(), grab().settle_time()),
            Point::default(),
            "a gap inside the window is a handoff and travels"
        );

        state.forget_if_stale(start + MEMORY / 2 + MEMORY * 2);
        assert_eq!(state.origin, None, "a stale rectangle is forgotten");
        assert_eq!(state.size, None);
        state.record(point(px(0.0), px(600.0)), Point::default());
        assert_eq!(
            state.sample(grab(), grab().settle_time()),
            Point::default(),
            "an element with no recent rectangle is simply already in place"
        );
    }

    #[test]
    fn two_elements_sharing_an_id_in_one_frame_contest_it() {
        let mut state = FlipState::default();
        assert!(
            !state.claim(Some(7)),
            "one element per frame is no collision"
        );
        assert!(
            state.claim(Some(7)),
            "the second element in a frame collides"
        );
        assert!(
            state.claim(Some(8)),
            "the frame after a collision is still refused"
        );
        assert!(
            !state.claim(Some(9)),
            "a single renderer resumes once it has a rectangle of its own"
        );
    }

    #[test]
    fn a_host_without_a_frame_counter_never_reports_a_collision() {
        let mut state = FlipState::default();
        assert!(!state.claim(None));
        assert!(!state.claim(None));
    }

    #[test]
    fn an_unchanged_origin_that_has_settled_is_already_home() {
        let settle = grab().settle_time();
        let mut state = FlipState::default();
        state.settle(point(px(12.0), px(8.0)), settle);
        assert!(state.is_home(point(px(12.0), px(8.0)), settle));
        assert!(!state.is_home(point(px(40.0), px(8.0)), settle));
    }
}
