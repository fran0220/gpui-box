//! Carrying an item from where it is to where it should go.
//!
//! # The library never moves anything
//!
//! A drop reports an intent — "this item goes before that one", "into that
//! folder" — and stops. The host applies it to the data it owns and hands back
//! a new order; the list shows the reorder on the frame that new order
//! arrives, and not before. A host that refuses the move keeps the order that
//! still holds, which is the same rule every other component in this library
//! follows for values, sorts and selections.
//!
//! # What GPUI provides, and what is added here
//!
//! GPUI owns the gesture: [`gpui::StatefulInteractiveElement::on_drag`] turns
//! a press plus two pixels of travel into a drag, keeps the payload in the
//! application, paints one drag view at the pointer, routes
//! [`gpui::InteractiveElement::on_drag_move`] to every registered element and
//! [`gpui::InteractiveElement::on_drop`] to whatever the pointer is over on
//! release, and lets a target refuse a payload through
//! [`gpui::InteractiveElement::can_drop`].
//!
//! What GPUI does not have, and this module adds: a vocabulary for where a
//! drop lands ([`DropPosition`]), one payload type every surface in the
//! library agrees on ([`DragItem`]), the speed the gesture was moving at as
//! well as where it was ([`DropIntent::velocity`]), a published record of the
//! drag so a test can read what is being carried and where it would land,
//! escape as a cancel,
//! a ghost that follows the pointer on a spring instead of snapping to it, and
//! the make-way slide that opens the slot the drop would land in.
//!
//! # What a drag publishes
//!
//! While a drag is in flight the semantic tree carries one extra node, with
//! the id [`DRAG_NODE_ID`] and the role [`Role::Drag`]:
//!
//! - `text` — the label of the item being carried;
//! - `value` — the item's id and where it would land, as
//!   `"<item id> before:<anchor>"`, `"after:<anchor>"`, `"into:<anchor>"`, or
//!   `"<item id> none"` when the pointer is over nothing that offers a slot;
//! - `invalid` — set when the target under the pointer refuses the payload.
//!
//! The node exists only while the drag does, so a test reads it from an
//! ordinary snapshot and never has to sleep.
//!
//! # Reduced motion
//!
//! The ghost is direct manipulation, not decoration: it is the thing the hand
//! is holding, so it keeps following the pointer under reduced motion. What it
//! loses is the spring — it tracks the pointer exactly instead of trailing it.
//! The make-way slides are decoration, so they settle instantly and the slot
//! is simply open from the first frame.

use std::any::Any;
use std::cell::RefCell;
use std::panic::Location;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, AppContext, Bounds, Context, Element, ElementId, GlobalElementId, InspectorElementId,
    InteractiveElement, IntoElement, LayoutId, ParentElement, Pixels, Point, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, SpringPreset, Surface};
use web_time::Instant;

use crate::display::icon::Icon as IconView;
use crate::foundation::Sizable;
use crate::foundation::StyledExt;
use crate::motion::{Interpolate, Spring, Velocity, VelocityTracker, keyed};
use crate::strings::{ActiveStrings, StringKey};

/// The semantic id of the node a drag publishes while it is in flight.
pub const DRAG_NODE_ID: &str = "dnd.drag";

/// The payload kind a row, node, or tab carries.
pub const ROW_KIND: &str = "row";

/// The payload kind an external file drop carries.
pub const FILE_KIND: &str = "file";

/// What a drag is carrying.
///
/// The identity is the item's business identity, never its position, because a
/// reorder changes every position and nothing else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DragItem {
    /// The surface the drag started in, so a target can tell its own rows from
    /// somebody else's.
    pub source: SharedString,
    pub id: SharedString,
    /// The name shown on the ghost and published in the tree.
    pub label: SharedString,
    /// What sort of thing this is, so a target can refuse a payload it does
    /// not handle.
    pub kind: SharedString,
    pub icon: Option<Icon>,
}

impl DragItem {
    pub fn new(
        source: impl Into<SharedString>,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            source: source.into(),
            id: id.into(),
            label: label.into(),
            kind: SharedString::new_static(ROW_KIND),
            icon: None,
        }
    }

    pub fn kind(mut self, kind: impl Into<SharedString>) -> Self {
        self.kind = kind.into();
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// Where a dropped item goes, expressed against something already there.
///
/// "At index N" is deliberately absent: an index is a position, and a position
/// stops meaning anything the moment the host applies the move. A drop at the
/// top of a list is `Before` the first item; a drop into an empty container is
/// `Into` the container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DropPosition {
    Before(SharedString),
    After(SharedString),
    /// Inside the named container — a folder in a tree, or a zone that holds
    /// items without ordering them.
    Into(SharedString),
}

impl DropPosition {
    /// The item the position is expressed against.
    pub fn anchor(&self) -> &SharedString {
        match self {
            Self::Before(id) | Self::After(id) | Self::Into(id) => id,
        }
    }

    pub fn verb(&self) -> &'static str {
        match self {
            Self::Before(_) => "before",
            Self::After(_) => "after",
            Self::Into(_) => "into",
        }
    }
}

impl std::fmt::Display for DropPosition {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}:{}", self.verb(), self.anchor())
    }
}

/// One drop, as it is reported to the host.
#[derive(Clone, Debug, PartialEq)]
pub struct DropIntent {
    pub item: DragItem,
    pub position: DropPosition,
    /// How fast the pointer was moving when it let go, in pixels a second.
    ///
    /// A gesture that stopped before it was released reports
    /// [`Velocity::ZERO`], which is the difference between a flick and a
    /// deliberate placement. See [`crate::motion::flick`].
    pub velocity: Velocity,
}

/// Which way a target's slots are laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropAxis {
    Vertical,
    Horizontal,
}

/// The drag as anything outside the module can see it.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveDrag {
    pub item: DragItem,
    /// How fast the pointer is moving right now, in pixels a second. A drag
    /// the user has paused reports [`Velocity::ZERO`].
    pub velocity: Velocity,
    /// The surface the pointer is currently over, when it offers a slot.
    pub surface: Option<SharedString>,
    /// Where the drop would land right now.
    pub position: Option<DropPosition>,
    /// Whether the target under the pointer accepts the payload. A drag over
    /// nothing is neither accepted nor refused.
    pub accepted: bool,
}

/// The state a target published on the last pointer move.
#[derive(Clone, Debug)]
struct Landing {
    surface: SharedString,
    position: DropPosition,
    /// How many rows precede the insertion point, for deciding which rows
    /// slide out of the way. `None` when the drop goes inside something and
    /// therefore opens no slot.
    slot: Option<usize>,
    accepted: bool,
    /// The pointer position that produced this landing.
    ///
    /// Every target sees the same position for one move, so a landing whose
    /// position is not the current one was written by an earlier move and the
    /// pointer has since left the target that wrote it. `None` marks a staged
    /// landing, which has no pointer behind it.
    at: Option<Point<Pixels>>,
}

#[derive(Default)]
struct Session {
    item: Option<DragItem>,
    landing: Option<Landing>,
    /// Set by [`stage`]. A staged drag has no pointer and no gesture, so it
    /// neither expires with the pointer nor settles over time.
    staged: bool,
    /// How fast the pointer is travelling, so a drop reports the speed it was
    /// let go at and not only where.
    speed: VelocityTracker,
    /// The last position and instant handed to the tracker. Every registered
    /// target hears the same move, so without this one gesture would be
    /// sampled once per target.
    sampled: Option<(Instant, Point<Pixels>)>,
}

#[derive(Default)]
struct SessionGlobal(RefCell<Session>);

impl gpui::Global for SessionGlobal {}

/// Installs the drag session and the key that cancels a drag.
///
/// Escape is observed at the application rather than bound to an element,
/// because the pointer can be anywhere by the time a drag is abandoned and the
/// element the drag started on may no longer be under it.
pub fn install(cx: &mut App) {
    if cx.has_global::<SessionGlobal>() {
        return;
    }
    cx.set_global(SessionGlobal::default());
    cx.observe_keystrokes(|event, window, cx| {
        if event.keystroke.key == "escape" && cx.has_active_drag() {
            cancel(window, cx);
        }
    })
    .detach();
}

fn session<R>(cx: &mut App, act: impl FnOnce(&mut Session) -> R) -> R {
    if !cx.has_global::<SessionGlobal>() {
        cx.set_global(SessionGlobal::default());
    }
    let mut state = cx.global::<SessionGlobal>().0.borrow_mut();
    act(&mut state)
}

fn read<R>(cx: &App, act: impl FnOnce(&Session) -> R) -> Option<R> {
    cx.try_global::<SessionGlobal>()
        .map(|global| act(&global.0.borrow()))
}

/// Records the item a gesture just picked up.
fn begin(item: DragItem, cx: &mut App) {
    session(cx, |state| {
        state.item = Some(item);
        state.landing = None;
        state.staged = false;
        // A new gesture starts from rest: the speed of the one before it is
        // not this one's.
        state.speed.clear();
        state.sampled = None;
    });
}

/// Drops everything the session remembers.
fn clear(cx: &mut App) {
    session(cx, |state| *state = Session::default());
}

/// Forgets the session once a drop has been reported.
pub(crate) fn finish(cx: &mut App) {
    clear(cx);
}

/// Records a platform file drag, which never passes through [`draggable`].
///
/// The label counts the files rather than naming them: a path is
/// user-generated content and the label is published in the semantic tree.
pub(crate) fn adopt_external(count: usize, cx: &mut App) {
    let label = if count == 1 {
        cx.strings().text(StringKey::DragFileOne)
    } else {
        cx.strings()
            .format(StringKey::DragFileMany, &[&count.to_string()])
    };
    let carried = read(cx, |state| state.item.clone()).flatten();
    if carried.is_some_and(|item| item.kind.as_ref() == FILE_KIND && item.label == label) {
        return;
    }
    begin(
        DragItem::new(SharedString::new_static("platform"), "files", label).kind(FILE_KIND),
        cx,
    );
}

/// Abandons a drag in flight. A cancelled drag reports nothing.
pub fn cancel(window: &mut Window, cx: &mut App) {
    cx.stop_active_drag(window);
    clear(cx);
}

/// Forgets a session whose gesture has ended.
///
/// A component calls this before it reads the session, so a drag that finished
/// between two frames cannot leave an indicator behind.
pub(crate) fn sync(cx: &mut App) {
    let stale = session(cx, |state| state.item.is_some() && !state.staged);
    if stale && !cx.has_active_drag() {
        clear(cx);
    }
}

/// The drag in flight, if there is one.
pub fn active(window: &Window, cx: &App) -> Option<ActiveDrag> {
    let pointer = window.mouse_position();
    read(cx, |state| {
        let item = state.item.clone()?;
        let landing = state
            .landing
            .as_ref()
            .filter(|landing| landing.at.is_none_or(|at| at == pointer));
        Some(ActiveDrag {
            item,
            velocity: state.speed.velocity_at(cx.background_executor().now()),
            surface: landing.map(|landing| landing.surface.clone()),
            position: landing.map(|landing| landing.position.clone()),
            accepted: landing.is_some_and(|landing| landing.accepted),
        })
    })
    .flatten()
}

/// The landing this surface published for the current pointer position.
fn landing_for(surface: &SharedString, pointer: Point<Pixels>, cx: &App) -> Option<Landing> {
    read(cx, |state| {
        state
            .landing
            .as_ref()
            .filter(|landing| &landing.surface == surface)
            .filter(|landing| landing.at.is_none_or(|at| at == pointer))
            .cloned()
    })
    .flatten()
}

fn fresh_landing(pointer: Point<Pixels>, cx: &App) -> Option<Landing> {
    read(cx, |state| {
        state
            .landing
            .as_ref()
            .filter(|landing| landing.at.is_none_or(|at| at == pointer))
            .cloned()
    })
    .flatten()
}

fn set_landing(landing: Landing, cx: &mut App) {
    session(cx, |state| state.landing = Some(landing));
}

fn clear_landing(cx: &mut App) {
    session(cx, |state| state.landing = None);
}

fn is_staged(cx: &App) -> bool {
    read(cx, |state| state.staged).unwrap_or(false)
}

/// Feeds one pointer move to the gesture's velocity tracker.
fn record_pointer(pointer: Point<Pixels>, at: Instant, cx: &mut App) {
    session(cx, |state| {
        if state.sampled == Some((at, pointer)) {
            return;
        }
        state.sampled = Some((at, pointer));
        state.speed.sample(pointer, at);
    });
}

/// How fast the drag in flight is moving.
///
/// The clock is passed in rather than read from the samples, because a pointer
/// that has stopped sends nothing at all and a pause is only visible against a
/// clock.
fn velocity(cx: &App) -> Velocity {
    read(cx, |state| {
        state.speed.velocity_at(cx.background_executor().now())
    })
    .unwrap_or(Velocity::ZERO)
}

/// What a surface needs to know to draw a drag it is taking part in.
#[derive(Clone, Debug)]
pub(crate) struct SurfaceDrag {
    pub item: DragItem,
    pub position: Option<DropPosition>,
    pub slot: Option<usize>,
    pub accepted: bool,
}

impl SurfaceDrag {
    /// Whether the row named `id` is the one being carried.
    pub fn carries(&self, id: &SharedString) -> bool {
        &self.item.id == id
    }

    /// The indicator this row should draw, if any.
    pub fn indicator_for(&self, id: &SharedString) -> Option<(DropPosition, bool)> {
        let position = self.position.clone()?;
        (position.anchor() == id).then_some((position, self.accepted))
    }

    /// Whether a row sitting at `index` has to slide to open the slot.
    pub fn makes_way(&self, index: usize) -> bool {
        self.slot.is_some_and(|slot| index >= slot)
    }
}

/// The drag `surface` is currently taking part in.
pub(crate) fn surface_drag(
    surface: &SharedString,
    window: &Window,
    cx: &mut App,
) -> Option<SurfaceDrag> {
    sync(cx);
    let pointer = window.mouse_position();
    let item = read(cx, |state| state.item.clone()).flatten()?;
    let landing = landing_for(surface, pointer, cx);
    Some(SurfaceDrag {
        item,
        position: landing.as_ref().map(|landing| landing.position.clone()),
        slot: landing.as_ref().and_then(|landing| landing.slot),
        accepted: landing.is_some_and(|landing| landing.accepted),
    })
}

// -- staging -----------------------------------------------------------------

/// A drag placed by hand rather than by a pointer.
///
/// A still image cannot photograph a gesture, and a capture that waited for a
/// real drag would race the pointer and the spring. Staging puts the system
/// into one fixed state so a scene renders the same pixels every run.
#[derive(Clone, Debug)]
pub struct StagedDrag {
    item: DragItem,
    landing: Option<Landing>,
}

impl StagedDrag {
    pub fn new(item: DragItem) -> Self {
        Self {
            item,
            landing: None,
        }
    }

    /// Where the staged drag would land. `slot` is how many rows precede the
    /// insertion point, which is what decides the rows that slide aside.
    pub fn landing(
        mut self,
        surface: impl Into<SharedString>,
        position: DropPosition,
        slot: Option<usize>,
        accepted: bool,
    ) -> Self {
        self.landing = Some(Landing {
            surface: surface.into(),
            position,
            slot,
            accepted,
            at: None,
        });
        self
    }
}

/// Places the drag system in a fixed state, for a capture or a review that has
/// to show a drag in flight.
pub fn stage(drag: StagedDrag, cx: &mut App) {
    session(cx, |state| {
        state.item = Some(drag.item.clone());
        state.landing = drag.landing.clone();
        state.staged = true;
    });
}

/// The ghost of a staged drag, for a scene that places it itself.
pub fn staged_ghost(cx: &mut App) -> Option<gpui::Div> {
    if !is_staged(cx) {
        return None;
    }
    let item = read(cx, |state| state.item.clone()).flatten()?;
    let landing = read(cx, |state| state.landing.clone()).flatten();
    Some(ghost_element(&item, landing.as_ref(), cx))
}

// -- picking a payload up -----------------------------------------------------

/// Makes `element` the handle of a drag carrying `item`.
///
/// GPUI decides when the press becomes a drag; this only says what is being
/// carried and what the ghost looks like.
pub fn draggable<E>(element: E, item: DragItem) -> E
where
    E: StatefulInteractiveElement + Sized,
{
    element.on_drag(item, |item, _offset, _window, cx| {
        begin(item.clone(), cx);
        let carried = item.clone();
        cx.new(|_| DragGhost::new(carried))
    })
}

// -- putting a payload down ---------------------------------------------------

/// Whether a payload may land, and where.
type Accepts = Rc<dyn Fn(&DragItem, &DropPosition) -> bool>;
type Dropped = Rc<dyn Fn(&DropIntent, &mut Window, &mut App)>;

/// One row, node, or tab offering the slots around itself.
pub(crate) struct RowTarget {
    pub surface: SharedString,
    pub id: SharedString,
    /// How many rows precede this one, for deciding the make-way slot.
    pub index: usize,
    /// Whether the middle of the row offers [`DropPosition::Into`].
    pub allow_into: bool,
    pub axis: DropAxis,
    pub accepts: Accepts,
    pub on_drop: Dropped,
}

/// Which slot a pointer standing `fraction` of the way across a target asks
/// for.
///
/// A target that can be dropped into keeps its middle half for that, leaving a
/// quarter at each end for the slots beside it. A target that cannot splits in
/// two, so every pixel of it asks for something.
pub(crate) fn zone(fraction: f32, allow_into: bool) -> DropZone {
    if allow_into {
        if fraction < 0.25 {
            DropZone::Before
        } else if fraction > 0.75 {
            DropZone::After
        } else {
            DropZone::Into
        }
    } else if fraction < 0.5 {
        DropZone::Before
    } else {
        DropZone::After
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DropZone {
    Before,
    After,
    Into,
}

impl DropZone {
    fn resolve(self, id: &SharedString, index: usize) -> (DropPosition, Option<usize>) {
        match self {
            Self::Before => (DropPosition::Before(id.clone()), Some(index)),
            Self::After => (DropPosition::After(id.clone()), Some(index + 1)),
            Self::Into => (DropPosition::Into(id.clone()), None),
        }
    }
}

fn fraction_of(bounds: Bounds<Pixels>, pointer: Point<Pixels>, axis: DropAxis) -> f32 {
    let (offset, extent) = match axis {
        DropAxis::Vertical => (
            f32::from(pointer.y - bounds.origin.y),
            f32::from(bounds.size.height),
        ),
        DropAxis::Horizontal => (
            f32::from(pointer.x - bounds.origin.x),
            f32::from(bounds.size.width),
        ),
    };
    if extent <= 0.0 {
        return 0.0;
    }
    (offset / extent).clamp(0.0, 1.0)
}

/// Wires `element` up as a target for the slots around itself.
pub(crate) fn drop_target<E>(element: E, target: RowTarget) -> E
where
    E: InteractiveElement + Sized,
{
    let RowTarget {
        surface,
        id,
        index,
        allow_into,
        axis,
        accepts,
        on_drop,
    } = target;

    let element = element.on_drag_move::<DragItem>({
        let surface = surface.clone();
        let id = id.clone();
        let accepts = Rc::clone(&accepts);
        move |event, _window, cx| {
            let pointer = event.event.position;
            // Sampled before the bounds check: the speed of the gesture is a
            // property of the hand, not of whatever it happens to be over.
            record_pointer(pointer, cx.background_executor().now(), cx);
            if !event.bounds.contains(&pointer) {
                return;
            }
            let item = event.drag(cx).clone();
            // An item cannot be moved relative to itself, so its own row
            // offers no slot at all rather than a refusal.
            if item.id == id && item.source == surface {
                clear_landing(cx);
                return;
            }
            let (position, slot) =
                zone(fraction_of(event.bounds, pointer, axis), allow_into).resolve(&id, index);
            let accepted = accepts(&item, &position);
            set_landing(
                Landing {
                    surface: surface.clone(),
                    position,
                    slot,
                    accepted,
                    at: Some(pointer),
                },
                cx,
            );
        }
    });

    let element = element.can_drop({
        let surface = surface.clone();
        move |payload: &dyn Any, window: &mut Window, cx: &mut App| {
            payload.downcast_ref::<DragItem>().is_some()
                && landing_for(&surface, window.mouse_position(), cx)
                    .is_some_and(|landing| landing.accepted)
        }
    });

    element.on_drop::<DragItem>(move |item, window, cx| {
        let Some(landing) = landing_for(&surface, window.mouse_position(), cx) else {
            return;
        };
        if !landing.accepted {
            return;
        }
        let intent = DropIntent {
            item: item.clone(),
            position: landing.position.clone(),
            velocity: velocity(cx),
        };
        clear(cx);
        on_drop(&intent, window, cx);
    })
}

// -- the ghost ----------------------------------------------------------------

/// The rendering GPUI paints at the pointer while a drag is in flight.
///
/// It is a view rather than a builder because the spring that trails the
/// pointer has to remember where it was on the previous frame.
struct DragGhost {
    item: DragItem,
    /// The pointer position the current trail started from.
    from: Point<Pixels>,
    current: Point<Pixels>,
    anchor: Option<Point<Pixels>>,
    elapsed: Duration,
    last_frame: Option<Instant>,
}

impl DragGhost {
    fn new(item: DragItem) -> Self {
        Self {
            item,
            from: Point::default(),
            current: Point::default(),
            anchor: None,
            elapsed: Duration::ZERO,
            last_frame: None,
        }
    }

    /// The offset the ghost is painted at, relative to the pointer.
    fn trail(&mut self, pointer: Point<Pixels>, spring: Spring, settle: Duration, now: Instant) {
        if let Some(last) = self.last_frame {
            self.elapsed += now.saturating_duration_since(last);
        }
        self.last_frame = Some(now);
        let residual = self.sample(spring, settle);
        match self.anchor {
            Some(anchor) if anchor != pointer => {
                self.from = residual + anchor - pointer;
                self.elapsed = Duration::ZERO;
            }
            None => self.from = Point::default(),
            _ => {}
        }
        self.anchor = Some(pointer);
        self.current = self.sample(spring, settle);
        if self.elapsed >= settle {
            self.last_frame = None;
        }
    }

    fn sample(&self, spring: Spring, settle: Duration) -> Point<Pixels> {
        if self.elapsed >= settle {
            return Point::default();
        }
        self.from.lerp(Point::default(), spring.value(self.elapsed))
    }

    fn snap(&mut self, pointer: Point<Pixels>) {
        self.anchor = Some(pointer);
        self.from = Point::default();
        self.current = Point::default();
        self.elapsed = Duration::ZERO;
        self.last_frame = None;
    }
}

impl Render for DragGhost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pointer = window.mouse_position();
        if cx.reduce_motion() {
            self.snap(pointer);
        } else {
            let spring = Spring::preset(cx.theme(), SpringPreset::Grab);
            let settle = spring.settle_time();
            let now = cx.background_executor().now();
            self.trail(pointer, spring, settle, now);
        }
        let offset = self.current;
        if offset != Point::default() {
            window.request_animation_frame();
        }

        let landing = fresh_landing(pointer, cx);
        let card = ghost_element(&self.item, landing.as_ref(), cx);
        div().child(card.ml(offset.x).mt(offset.y))
    }
}

/// The ghost itself: a rendering of the item, not a grey box.
fn ghost_element(item: &DragItem, landing: Option<&Landing>, cx: &mut App) -> gpui::Div {
    let theme = cx.theme().clone();
    let refused = landing.is_some_and(|landing| !landing.accepted);
    let where_to = match landing {
        Some(landing) => format!("{} {}", item.id, landing.position),
        None => format!("{} none", item.id),
    };

    // What is in the hand is a thing lifted off the surface, so it says so
    // the way every other lifted surface in this library does: an overlay
    // colour and an overlay shadow. A refusal is the one case that draws a
    // line, because refusal is exactly what a line is reserved for here, and
    // it is the danger colour so it cannot be read as ordinary chrome.
    let ghost = div()
        .row()
        .flex_none()
        .gap_token(&theme, Space::Xs)
        .px_token(&theme, Space::Sm)
        .py_token(&theme, Space::Xs)
        .frame(&theme, Surface::Overlay, Elevation::Overlay)
        .border(px(theme.borders.hairline))
        .border_color(if refused {
            theme.colors.danger
        } else {
            gpui::transparent_black()
        })
        .radius(&theme, Radius::Control)
        .text_color(theme.colors.text)
        .type_scale(&theme, gpui_kit_theme::TypeScale::Body)
        // The same glyph, the same token step, the same role: this is what
        // hand-drawing an icon was already spelling out.
        .children(item.icon.map(|glyph| IconView::new(glyph).medium().muted()))
        .child(item.label.clone())
        .semantic_in(
            cx,
            NodeSpec::new(DRAG_NODE_ID, Role::Drag)
                .text(item.label.clone())
                .value(where_to)
                .invalid(refused),
        );
    div().child(ghost)
}

// -- the drop indicator -------------------------------------------------------

/// The line or highlight that says where the drop would land.
///
/// It is drawn over the row rather than between rows, because a real gap would
/// move the layout and a virtualized list has no room between its slots.
pub(crate) fn indicator(
    position: &DropPosition,
    accepted: bool,
    axis: DropAxis,
    cx: &App,
) -> gpui::Div {
    let theme = cx.theme();
    let color = if accepted {
        theme.colors.accent
    } else {
        theme.colors.danger
    };
    let thickness = px(theme.borders.thick);
    // The line is inset from the ends and rounded, so it reads as a mark
    // placed between two rows rather than as a rule that has divided the
    // whole surface. It is the same shape a text caret is, for the same
    // reason: it points at a position, it does not partition anything.
    let inset = px(theme.space(Space::Sm));
    let line = div().absolute().bg(color).rounded(thickness / 2.0);
    match (position, axis) {
        // A drop *into* a row is the row itself becoming the target, so it is
        // a wash and a soft ring rather than an outline: an outlined row in a
        // list of rows is a second border language competing with the one the
        // list already speaks.
        (DropPosition::Into(_), _) => div()
            .absolute()
            .inset_0()
            .rounded(px(theme.radii.small))
            .bg(color.opacity(theme.effects.selected_ring_alpha))
            .shadow(vec![gpui::BoxShadow {
                color: color.opacity(0.5),
                offset: gpui::point(px(0.0), px(0.0)),
                blur_radius: px(0.0),
                spread_radius: px(theme.borders.hairline),
                inset: true,
            }]),
        (DropPosition::Before(_), DropAxis::Vertical) => {
            line.left(inset).right(inset).top_0().h(thickness)
        }
        (DropPosition::After(_), DropAxis::Vertical) => {
            line.left(inset).right(inset).bottom_0().h(thickness)
        }
        (DropPosition::Before(_), DropAxis::Horizontal) => {
            line.top(inset).bottom(inset).left_0().w(thickness)
        }
        (DropPosition::After(_), DropAxis::Horizontal) => {
            line.top(inset).bottom(inset).right_0().w(thickness)
        }
    }
}

// -- making way ---------------------------------------------------------------

/// How far a row travels to open the slot a drop would land in.
pub(crate) fn make_way_gap(cx: &App, axis: DropAxis) -> Pixels {
    let theme = cx.theme();
    match axis {
        DropAxis::Vertical => px(theme.space(Space::Md)),
        DropAxis::Horizontal => px(theme.space(Space::Lg)),
    }
}

#[derive(Default)]
struct SlideState {
    target: Point<Pixels>,
    from: Point<Pixels>,
    current: Point<Pixels>,
    elapsed: Duration,
    last_frame: Option<Instant>,
}

impl SlideState {
    fn advance(&mut self, target: Point<Pixels>, spring: Spring, settle: Duration, now: Instant) {
        if target != self.target {
            self.from = self.current;
            self.target = target;
            self.elapsed = Duration::ZERO;
            self.last_frame = Some(now);
        } else if let Some(last) = self.last_frame {
            self.elapsed += now.saturating_duration_since(last);
            self.last_frame = Some(now);
        }
        self.current = if self.elapsed >= settle {
            self.last_frame = None;
            self.target
        } else {
            self.from.lerp(self.target, spring.value(self.elapsed))
        };
    }

    fn settle_at(&mut self, target: Point<Pixels>) {
        self.target = target;
        self.from = target;
        self.current = target;
        self.elapsed = Duration::ZERO;
        self.last_frame = None;
    }
}

/// Slides an element away from where layout put it, to open a slot.
///
/// This is the invert-and-play half of [`crate::motion::Flipping::flip`]
/// without the measurement half: a row inside a virtualized list keeps the
/// layout slot its index gives it, so the gap can only ever be painted.
pub(crate) trait MakingWay: IntoElement + Sized {
    fn make_way(
        self,
        id: impl Into<SharedString>,
        offset: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> MakeWay {
        let id = id.into();
        let state = keyed::slot::<SlideState>(&id, cx);
        let spring = Spring::preset(cx.theme(), SpringPreset::Grab);
        let instant = cx.reduce_motion() || is_staged(cx);
        if state.borrow().current != offset {
            window.request_animation_frame();
        }
        MakeWay {
            element: self.into_any_element(),
            state,
            offset,
            spring,
            settle: spring.settle_time(),
            instant,
        }
    }
}

impl<E: IntoElement> MakingWay for E {}

/// An element painted beside where layout put it.
pub struct MakeWay {
    element: gpui::AnyElement,
    state: Rc<RefCell<SlideState>>,
    offset: Point<Pixels>,
    spring: Spring,
    settle: Duration,
    instant: bool,
}

impl std::fmt::Debug for MakeWay {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MakeWay")
            .field("offset", &self.offset)
            .field("instant", &self.instant)
            .finish()
    }
}

impl IntoElement for MakeWay {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for MakeWay {
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
        (self.element.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let painted = {
            let mut state = self.state.borrow_mut();
            if self.instant {
                state.settle_at(self.offset);
            } else {
                state.advance(
                    self.offset,
                    self.spring,
                    self.settle,
                    cx.background_executor().now(),
                );
            }
            state.current
        };

        window.with_element_offset(painted, |window| {
            self.element.prepaint(window, cx);
        });

        if painted != self.offset {
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

    #[test]
    fn a_target_that_cannot_be_entered_splits_in_two() {
        assert_eq!(zone(0.0, false), DropZone::Before);
        assert_eq!(zone(0.49, false), DropZone::Before);
        assert_eq!(zone(0.5, false), DropZone::After);
        assert_eq!(zone(1.0, false), DropZone::After);
    }

    #[test]
    fn a_target_that_can_be_entered_keeps_its_middle() {
        assert_eq!(zone(0.1, true), DropZone::Before);
        assert_eq!(zone(0.5, true), DropZone::Into);
        assert_eq!(zone(0.9, true), DropZone::After);
    }

    #[test]
    fn a_slot_counts_the_rows_that_precede_the_insertion_point() {
        let id = SharedString::new_static("beta");
        assert_eq!(DropZone::Before.resolve(&id, 3).1, Some(3));
        assert_eq!(DropZone::After.resolve(&id, 3).1, Some(4));
        // Entering something opens no slot, so nothing slides.
        assert_eq!(DropZone::Into.resolve(&id, 3).1, None);
    }

    #[test]
    fn a_position_reads_as_a_verb_and_an_anchor() {
        let position = DropPosition::Before(SharedString::new_static("beta"));
        assert_eq!(position.to_string(), "before:beta");
        assert_eq!(position.anchor().as_ref(), "beta");
        assert_eq!(
            DropPosition::Into(SharedString::new_static("docs")).to_string(),
            "into:docs"
        );
    }

    #[test]
    fn a_pointer_is_placed_by_the_axis_it_is_measured_on() {
        let bounds = Bounds::new(point(px(10.0), px(20.0)), size(px(100.0), px(40.0)));
        let pointer = point(px(60.0), px(50.0));
        assert_eq!(fraction_of(bounds, pointer, DropAxis::Horizontal), 0.5);
        assert_eq!(fraction_of(bounds, pointer, DropAxis::Vertical), 0.75);
    }

    #[test]
    fn an_unmeasured_target_offers_its_first_slot() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(0.0), px(0.0)));
        assert_eq!(
            fraction_of(bounds, point(px(0.0), px(0.0)), DropAxis::Vertical),
            0.0
        );
    }

    #[test]
    fn a_row_slides_only_once_the_insertion_point_reaches_it() {
        let drag = SurfaceDrag {
            item: DragItem::new("list", "gamma", "Gamma"),
            position: Some(DropPosition::Before(SharedString::new_static("beta"))),
            slot: Some(2),
            accepted: true,
        };
        assert!(!drag.makes_way(1));
        assert!(drag.makes_way(2));
        assert!(drag.makes_way(9));
    }

    #[test]
    fn entering_a_folder_moves_no_row_aside() {
        let drag = SurfaceDrag {
            item: DragItem::new("tree", "lib", "lib.rs"),
            position: Some(DropPosition::Into(SharedString::new_static("docs"))),
            slot: None,
            accepted: true,
        };
        assert!(!drag.makes_way(0));
        assert!(
            drag.indicator_for(&SharedString::new_static("docs"))
                .is_some()
        );
        assert!(
            drag.indicator_for(&SharedString::new_static("src"))
                .is_none()
        );
    }

    #[test]
    fn the_ghost_trails_the_pointer_and_catches_up() {
        let spring = grab();
        let settle = spring.settle_time();
        let mut ghost = DragGhost::new(DragItem::new("list", "gamma", "Gamma"));
        let start = Instant::now();
        ghost.trail(point(px(0.0), px(0.0)), spring, settle, start);
        assert_eq!(ghost.current, Point::default());

        ghost.trail(point(px(0.0), px(40.0)), spring, settle, start);
        assert_eq!(ghost.current, point(px(0.0), px(-40.0)));

        ghost.trail(point(px(0.0), px(40.0)), spring, settle, start + settle);
        assert_eq!(ghost.current, Point::default());
    }

    #[test]
    fn reduced_motion_pins_the_ghost_to_the_pointer() {
        let mut ghost = DragGhost::new(DragItem::new("list", "gamma", "Gamma"));
        ghost.snap(point(px(0.0), px(0.0)));
        ghost.snap(point(px(0.0), px(80.0)));
        assert_eq!(ghost.current, Point::default());
    }

    #[test]
    fn a_slide_starts_from_what_is_on_screen() {
        let spring = grab();
        let settle = spring.settle_time();
        let mut slide = SlideState::default();
        let start = Instant::now();
        slide.advance(point(px(0.0), px(12.0)), spring, settle, start);
        assert_eq!(slide.current, Point::default());
        slide.advance(point(px(0.0), px(12.0)), spring, settle, start + settle);
        assert_eq!(slide.current, point(px(0.0), px(12.0)));
    }

    #[test]
    fn an_instant_slide_is_already_where_it_belongs() {
        let mut slide = SlideState::default();
        slide.settle_at(point(px(0.0), px(12.0)));
        assert_eq!(slide.current, point(px(0.0), px(12.0)));
    }
}
