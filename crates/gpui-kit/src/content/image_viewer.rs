//! One image at a time, framed, zoomed and panned — and never fetched.
//!
//! # What this component will not do
//!
//! **It does not fetch anything.** This crate has no network and no asset
//! resolution, which is the same reason [`crate::content::Markdown`] draws an
//! image reference as a placeholder naming its source. A host that holds the
//! bytes hands an element back through [`ImageViewer::image`]; a host that
//! does not has said so, and the frame names what is missing rather than
//! showing a grey box.
//!
//! **It does not measure the source.** Natural dimensions are a caller input.
//! An image whose pixel size nobody stated reads `Size unknown`, and the zoom
//! and fit controls are refused with that as their reason, because a scale is
//! a ratio against a size and there is no size. Reporting the rendered size as
//! though it were the source's would be inventing the fact the host declined
//! to give.
//!
//! **It does not decide what a refusal means.** Loading, an image the host
//! could not supply, an image that failed to decode, and a ready one are four
//! renderings, and the middle two carry the host's own sentence.
//!
//! **It does not wrap.** Stepping past the last image would put the reader
//! back at the first without saying so, so the control at each end is refused
//! and the position is published instead.

use std::collections::HashSet;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels,
    Point, RenderOnce, ScrollDelta, SharedString, Size, Styled, Window, div, point,
    prelude::FluentBuilder, px, size,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, Theme, TypeScale,
};

use crate::controls::button::IconButton;
use crate::controls::segmented::{Segment, SegmentedControl};
use crate::foundation::{Disableable, FocusRing, Ident, Sizable, StyledExt};
use crate::layout::measure;
use crate::motion::keyed;
use crate::strings::{ActiveStrings, StringKey};

/// How tall the frame is when the caller says nothing. The value occurs once.
const DEFAULT_HEIGHT: f32 = 320.0;

/// How far one keystroke or one wheel notch moves the zoom.
const ZOOM_STEP: f32 = 1.25;

/// How the image is sized inside the frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FitMode {
    /// The whole image, inside the frame.
    #[default]
    Contain,
    /// The whole frame, covered by the image.
    Cover,
    /// One image pixel per frame pixel.
    Actual,
    /// The caller's own scale, where one is the actual size.
    Zoom(f32),
}

impl FitMode {
    /// The name a semantic node publishes, so a test reads the mode rather
    /// than the geometry it produced.
    pub fn name(self) -> &'static str {
        match self {
            Self::Contain => "contain",
            Self::Cover => "cover",
            Self::Actual => "actual",
            Self::Zoom(_) => "zoom",
        }
    }
}

/// The pixel size of a source, as the host stated it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

impl ImageSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    fn as_f32(self) -> Size<f32> {
        size(self.width.max(1) as f32, self.height.max(1) as f32)
    }
}

/// What the host knows about one image right now.
///
/// This is the library's [`crate::state::Loadable`] vocabulary specialized to
/// a picture: an image the host refused and an image that arrived broken fail
/// differently and read differently, so they are not one variant.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ImageState {
    /// The host is still fetching it.
    Loading,
    /// The host could not supply it, in its own words.
    Unavailable(SharedString),
    /// The bytes arrived and could not be read, in the host's words.
    Failed(SharedString),
    /// The host has it.
    #[default]
    Ready,
}

impl ImageState {
    fn name(&self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Unavailable(_) => "unavailable",
            Self::Failed(_) => "failed",
            Self::Ready => "ready",
        }
    }
}

/// One image the viewer can show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageFrame {
    id: SharedString,
    label: SharedString,
    source: SharedString,
    natural: Option<ImageSize>,
    state: ImageState,
}

impl ImageFrame {
    /// `id` is the image's own identity, never its place in the list.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            source: SharedString::default(),
            natural: None,
            state: ImageState::default(),
        }
    }

    /// Where the image came from, shown whenever it is not on screen.
    pub fn source(mut self, source: impl Into<SharedString>) -> Self {
        self.source = source.into();
        self
    }

    /// The source's own pixel dimensions.
    ///
    /// Without this the viewer says the size is unknown and refuses to scale,
    /// because a scale nobody can compute is not a scale.
    pub fn natural(mut self, width: u32, height: u32) -> Self {
        self.natural = Some(ImageSize::new(width, height));
        self
    }

    pub fn state(mut self, state: ImageState) -> Self {
        self.state = state;
        self
    }

    /// The host could not supply this image, and this is why.
    pub fn unavailable(self, reason: impl Into<SharedString>) -> Self {
        self.state(ImageState::Unavailable(reason.into()))
    }

    /// The bytes arrived and could not be read, and this is why.
    pub fn failed(self, reason: impl Into<SharedString>) -> Self {
        self.state(ImageState::Failed(reason.into()))
    }

    pub fn loading(self) -> Self {
        self.state(ImageState::Loading)
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }
}

/// An image the viewer had to draw and the host has not supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRequest {
    pub id: SharedString,
    pub label: SharedString,
    pub source: SharedString,
    /// What the host said the source measures, when it said anything.
    pub natural: Option<ImageSize>,
}

/// What a viewer reports. It applies none of it.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageViewerEvent {
    /// The reader asked for a different fit, by a control, the wheel, or a
    /// key. The viewer draws whatever the caller says is showing.
    FitChanged(FitMode),
    /// The reader asked for another image, by its own id.
    Stepped { id: SharedString },
    /// An image the host has not supplied was drawn as a placeholder.
    ///
    /// Reported once per image per viewer rather than once per frame, so a
    /// host that answers by supplying it is not asked again for answering.
    ImageRequested(ImageRequest),
}

type EventHandler = Rc<dyn Fn(&ImageViewerEvent, &mut Window, &mut App)>;
type ImageSupplier = Rc<dyn Fn(&ImageFrame, &mut Window, &mut App) -> Option<AnyElement>>;

/// The image point pinned under a point of the frame.
///
/// Both are normalized — the image's own extent, and the frame's — which is
/// what makes the pin survive a zoom the caller has not applied yet. A wheel
/// notch records the image point that was under the pointer; the frame it is
/// applied on derives the offset from whatever scale the caller settled on, so
/// a refused zoom moves nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Pin {
    image: Point<f32>,
    frame: Point<f32>,
}

impl Default for Pin {
    fn default() -> Self {
        Self {
            image: point(0.5, 0.5),
            frame: point(0.5, 0.5),
        }
    }
}

/// What the frame remembers between two builds of the same viewer.
#[derive(Debug, Default)]
struct Viewport {
    pin: Pin,
    /// Set while the pointer is holding the image, so a pan that leaves the
    /// frame ends rather than continuing on the next unrelated move.
    panning: bool,
    at: Option<Point<Pixels>>,
}

/// The images already reported as unsupplied for one viewer.
#[derive(Debug, Default)]
struct Requested(HashSet<SharedString>);

/// Where the image sits inside the frame, once every caller-owned fact is in.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Geometry {
    scale: f32,
    drawn: Size<f32>,
    /// The image's top-left corner, in frame pixels.
    offset: Point<f32>,
}

impl Geometry {
    fn pannable(self, frame: Size<f32>) -> bool {
        self.drawn.width > frame.width + 0.5 || self.drawn.height > frame.height + 0.5
    }
}

/// The scale a fit mode asks for, given a frame and a source that measure
/// something.
fn scale_for(fit: FitMode, frame: Size<f32>, natural: Size<f32>, min: f32, max: f32) -> f32 {
    if frame.width <= 0.0 || frame.height <= 0.0 {
        return 0.0;
    }
    let horizontal = frame.width / natural.width;
    let vertical = frame.height / natural.height;
    let raw = match fit {
        FitMode::Contain => horizontal.min(vertical),
        FitMode::Cover => horizontal.max(vertical),
        FitMode::Actual => 1.0,
        FitMode::Zoom(zoom) => zoom,
    };
    raw.clamp(min, max)
}

/// Where the image lands, with the pin honored as far as the clamp allows.
///
/// An image smaller than the frame is centred, and one larger than it cannot
/// be dragged past its own edge, so no gesture can push the picture out of
/// view and leave the reader with nothing to drag back.
fn place(frame: Size<f32>, drawn: Size<f32>, pin: Pin) -> Point<f32> {
    let axis = |frame: f32, drawn: f32, image: f32, at: f32| {
        if drawn <= frame {
            (frame - drawn) / 2.0
        } else {
            (at * frame - image * drawn).clamp(frame - drawn, 0.0)
        }
    };
    point(
        axis(frame.width, drawn.width, pin.image.x, pin.frame.x),
        axis(frame.height, drawn.height, pin.image.y, pin.frame.y),
    )
}

fn layout(
    fit: FitMode,
    frame: Size<f32>,
    natural: Size<f32>,
    pin: Pin,
    zoom: (f32, f32),
) -> Geometry {
    let scale = scale_for(fit, frame, natural, zoom.0, zoom.1);
    let drawn = size(natural.width * scale, natural.height * scale);
    Geometry {
        scale,
        drawn,
        offset: place(frame, drawn, pin),
    }
}

/// The pin that keeps the image point currently under `at` under it.
fn pin_at(frame: Size<f32>, geometry: Geometry, at: Point<f32>) -> Pin {
    if frame.width <= 0.0 || frame.height <= 0.0 || geometry.drawn.width <= 0.0 {
        return Pin::default();
    }
    Pin {
        image: point(
            ((at.x - geometry.offset.x) / geometry.drawn.width).clamp(0.0, 1.0),
            ((at.y - geometry.offset.y) / geometry.drawn.height).clamp(0.0, 1.0),
        ),
        frame: point(at.x / frame.width, at.y / frame.height),
    }
}

/// The pin after the image has been dragged by `delta`.
///
/// The result is expressed against the centre of the frame, so a pan that ran
/// into the clamp does not leave a pin describing somewhere the image cannot
/// go.
fn pan_by(frame: Size<f32>, geometry: Geometry, delta: Point<f32>) -> Pin {
    if frame.width <= 0.0 || frame.height <= 0.0 || geometry.drawn.width <= 0.0 {
        return Pin::default();
    }
    let moved = point(geometry.offset.x + delta.x, geometry.offset.y + delta.y);
    let settled = place(
        frame,
        geometry.drawn,
        Pin {
            image: point(0.0, 0.0),
            frame: point(moved.x / frame.width, moved.y / frame.height),
        },
    );
    Pin {
        image: point(
            (frame.width / 2.0 - settled.x) / geometry.drawn.width,
            (frame.height / 2.0 - settled.y) / geometry.drawn.height,
        ),
        frame: point(0.5, 0.5),
    }
}

/// A framed viewer for one image at a time.
#[derive(IntoElement)]
pub struct ImageViewer {
    ident: Ident,
    frames: Vec<ImageFrame>,
    showing: Option<SharedString>,
    fit: FitMode,
    min_zoom: f32,
    max_zoom: f32,
    height: f32,
    disabled: bool,
    image: Option<ImageSupplier>,
    on_event: Option<EventHandler>,
}

impl std::fmt::Debug for ImageViewer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImageViewer")
            .field("ident", &self.ident)
            .field("images", &self.frames.len())
            .field("showing", &self.showing)
            .field("fit", &self.fit)
            .field("zoom", &(self.min_zoom, self.max_zoom))
            .field("has_images", &self.image.is_some())
            .field("has_handler", &self.on_event.is_some())
            .finish()
    }
}

impl ImageViewer {
    pub fn new(ident: impl Into<Ident>, frames: impl IntoIterator<Item = ImageFrame>) -> Self {
        Self {
            ident: ident.into(),
            frames: frames.into_iter().collect(),
            showing: None,
            fit: FitMode::default(),
            min_zoom: 0.1,
            max_zoom: 8.0,
            height: DEFAULT_HEIGHT,
            disabled: false,
            image: None,
            on_event: None,
        }
    }

    /// Which image the caller says is showing. Without one the first is.
    pub fn showing(mut self, id: impl Into<SharedString>) -> Self {
        self.showing = Some(id.into());
        self
    }

    /// How the caller says the image is sized. The viewer draws this and
    /// reports every request to change it.
    pub fn fit(mut self, fit: FitMode) -> Self {
        self.fit = fit;
        self
    }

    /// How far in and out the reader may go. Both ends are the caller's
    /// judgement: a thumbnail and a scan of a page have different answers.
    pub fn zoom_range(mut self, min: f32, max: f32) -> Self {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        self.min_zoom = min.max(f32::EPSILON);
        self.max_zoom = max.max(self.min_zoom);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height.max(1.0);
        self
    }

    /// Supplies the element to draw for an image the host already holds.
    ///
    /// Answering `None` leaves a placeholder naming the source, so a host that
    /// cannot supply one has said so rather than left a gap.
    pub fn image(
        mut self,
        supplier: impl Fn(&ImageFrame, &mut Window, &mut App) -> Option<AnyElement> + 'static,
    ) -> Self {
        self.image = Some(Rc::new(supplier));
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(&ImageViewerEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }

    fn index(&self) -> usize {
        self.showing
            .as_ref()
            .and_then(|id| self.frames.iter().position(|frame| &frame.id == id))
            .unwrap_or(0)
    }
}

impl Disableable for ImageViewer {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for ImageViewer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();

        let Some(frame) = self.frames.get(self.index()).cloned() else {
            return empty_viewer(&ident, &theme, self.height, cx);
        };

        let index = self.index();
        let count = self.frames.len();
        let position = cx.strings().format(
            StringKey::CountOfTotal,
            &[&(index + 1).to_string(), &count.to_string()],
        );

        let measured = measure::cell(&ident.child("frame").semantic_id(), cx);
        let state = keyed::slot::<Viewport>(&ident.semantic_id(), cx);
        let bounds = measured.get();
        let extent = size(f32::from(bounds.size.width), f32::from(bounds.size.height));
        let pin = state.borrow().pin;

        // A source whose size nobody stated has no scale to compute, so the
        // whole zoom vocabulary is refused rather than answered with a number
        // taken from the frame it happens to be drawn in. A frame the layout
        // has not produced yet has no scale either, and states none until it
        // has one.
        let measurable = frame
            .natural
            .filter(|_| extent.width > 0.0 && extent.height > 0.0)
            .map(ImageSize::as_f32);
        let geometry = measurable.map(|natural| {
            layout(
                self.fit,
                extent,
                natural,
                pin,
                (self.min_zoom, self.max_zoom),
            )
        });
        let actionable = !self.disabled && self.on_event.is_some();
        let zoomable = actionable && geometry.is_some();

        let supplied = matches!(frame.state, ImageState::Ready)
            .then(|| {
                self.image
                    .as_ref()
                    .and_then(|supplier| supplier(&frame, window, cx))
            })
            .flatten();

        if matches!(frame.state, ImageState::Ready) && supplied.is_none() {
            report_missing(&ident, &frame, self.on_event.as_ref(), window, cx);
        }

        let body = match (&frame.state, supplied) {
            (ImageState::Ready, Some(element)) => match geometry {
                Some(geometry) => div()
                    .absolute()
                    .left(px(geometry.offset.x))
                    .top(px(geometry.offset.y))
                    .w(px(geometry.drawn.width))
                    .h(px(geometry.drawn.height))
                    .overflow_hidden()
                    .child(element)
                    .into_any_element(),
                // With no natural size there is no scaled box to place the
                // image in, so it is given the frame and nothing is claimed
                // about how much of the source that shows.
                None => div()
                    .absolute()
                    .inset_0()
                    .overflow_hidden()
                    .child(element)
                    .into_any_element(),
            },
            (ImageState::Ready, None) => notice(
                &theme,
                theme.colors.text_muted,
                frame.label.clone(),
                cx.strings()
                    .format(StringKey::ImageViewerNotSupplied, &[frame.source.as_ref()]),
            ),
            (ImageState::Loading, _) => notice(
                &theme,
                theme.colors.text_muted,
                frame.label.clone(),
                cx.strings().text(StringKey::Loading),
            ),
            (ImageState::Unavailable(reason), _) => notice(
                &theme,
                theme.colors.warning,
                frame.label.clone(),
                reason.clone(),
            ),
            (ImageState::Failed(reason), _) => notice(
                &theme,
                theme.colors.danger,
                frame.label.clone(),
                reason.clone(),
            ),
        };

        let report = {
            let handler = self.on_event.clone().filter(|_| actionable);
            move |event: ImageViewerEvent, window: &mut Window, cx: &mut App| {
                if let Some(handler) = &handler {
                    handler(&event, window, cx);
                }
            }
        };
        let report = Rc::new(report);

        let mut viewport = div()
            .id(ident.child("frame").element_id())
            .relative()
            .w_full()
            .h(px(self.height))
            .overflow_hidden()
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Raised, Elevation::Raised)
            .child(body);

        if let (true, Some(geometry)) = (zoomable, geometry) {
            let pannable = geometry.pannable(extent);
            let report_wheel = Rc::clone(&report);
            let wheel_bounds = Rc::clone(&measured);
            let wheel_state = Rc::clone(&state);
            let (min, max) = (self.min_zoom, self.max_zoom);
            viewport = viewport.on_scroll_wheel(move |event, window, cx| {
                let bounds = wheel_bounds.get();
                let extent = frame_extent(bounds);
                if extent.width <= 0.0 {
                    return;
                }
                let notches = wheel_notches(event.delta);
                if notches == 0.0 {
                    return;
                }
                let next = (geometry.scale * ZOOM_STEP.powf(notches)).clamp(min, max);
                if (next - geometry.scale).abs() < f32::EPSILON {
                    return;
                }
                let at = point(
                    f32::from(event.position.x - bounds.left()),
                    f32::from(event.position.y - bounds.top()),
                );
                wheel_state.borrow_mut().pin = pin_at(extent, geometry, at);
                report_wheel(
                    ImageViewerEvent::FitChanged(FitMode::Zoom(next)),
                    window,
                    cx,
                );
            });

            if pannable {
                let down_state = Rc::clone(&state);
                viewport = viewport.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    move |event, _, _| {
                        let mut state = down_state.borrow_mut();
                        state.panning = true;
                        state.at = Some(event.position);
                    },
                );

                let move_state = Rc::clone(&state);
                let move_bounds = Rc::clone(&measured);
                viewport = viewport.on_mouse_move(move |event, window, _| {
                    let mut state = move_state.borrow_mut();
                    if !state.panning {
                        return;
                    }
                    if event.pressed_button != Some(MouseButton::Left) {
                        state.panning = false;
                        state.at = None;
                        return;
                    }
                    let Some(previous) = state.at else {
                        state.at = Some(event.position);
                        return;
                    };
                    let delta = point(
                        f32::from(event.position.x - previous.x),
                        f32::from(event.position.y - previous.y),
                    );
                    state.at = Some(event.position);
                    state.pin = pan_by(frame_extent(move_bounds.get()), geometry, delta);
                    // Panning is this component's own transient state, so the
                    // frame that shows it has to be asked for.
                    window.refresh();
                });

                let up_state = Rc::clone(&state);
                viewport = viewport.on_mouse_up(MouseButton::Left, move |_, _, _| {
                    let mut state = up_state.borrow_mut();
                    state.panning = false;
                    state.at = None;
                });
            }
        }

        let strings = cx.strings().clone();
        let step =
            |name: &'static str, glyph: Icon, key: StringKey, target: Option<&ImageFrame>| {
                let label = strings.text(key);
                let id = target.map(|frame| frame.id.clone());
                let mut control = IconButton::new(ident.child(name), glyph, label)
                    .ghost()
                    .control_size(ControlSize::Sm)
                    .semantic_parent(ident.semantic_id())
                    .disabled(id.is_none() || !actionable);
                // Stepping past the end would wrap without saying so, so the
                // control at the end holds no handler at all.
                if let (Some(id), true) = (id, actionable) {
                    let report = Rc::clone(&report);
                    control = control.on_click(move |window, cx| {
                        report(ImageViewerEvent::Stepped { id: id.clone() }, window, cx)
                    });
                }
                control
            };

        let previous = step(
            "previous",
            Icon::ArrowLeft,
            StringKey::ImageViewerPrevious,
            index
                .checked_sub(1)
                .and_then(|index| self.frames.get(index)),
        );
        let next = step(
            "next",
            Icon::ArrowRight,
            StringKey::ImageViewerNext,
            self.frames.get(index + 1),
        );

        let fits = {
            let report = Rc::clone(&report);
            let mut control = SegmentedControl::new(ident.child("fit"))
                .control_size(ControlSize::Sm)
                .segments([
                    Segment::new("contain", strings.text(StringKey::ImageViewerContain)),
                    Segment::new("cover", strings.text(StringKey::ImageViewerCover)),
                    Segment::new("actual", "1:1"),
                ])
                .disabled(!zoomable);
            if !matches!(self.fit, FitMode::Zoom(_)) {
                control = control.selected(self.fit.name());
            }
            if zoomable {
                control = control.on_select(move |id, window, cx| {
                    let fit = match id.as_ref() {
                        "cover" => FitMode::Cover,
                        "actual" => FitMode::Actual,
                        _ => FitMode::Contain,
                    };
                    report(ImageViewerEvent::FitChanged(fit), window, cx);
                });
            }
            control
        };

        // Every number here is one the host stated. A source nobody measured
        // says so, rather than reporting the box it was drawn in.
        let measurement = match (frame.natural, geometry) {
            (Some(natural), Some(geometry)) => SharedString::from(format!(
                "{} × {} · {}%",
                natural.width,
                natural.height,
                (geometry.scale * 100.0).round() as i64
            )),
            (Some(natural), None) => {
                SharedString::from(format!("{} × {}", natural.width, natural.height))
            }
            _ => strings.text(StringKey::ImageViewerSizeUnknown),
        };

        let caption = div()
            .row()
            .w_full()
            .justify_between()
            .gap_token(&theme, Space::Sm)
            .type_scale(&theme, TypeScale::Caption)
            .text_color(theme.colors.text_muted)
            .child(
                div().child(measurement.clone()).semantic_in(
                    cx,
                    NodeSpec::new(ident.child("measurement").semantic_id(), Role::Text)
                        .parent(ident.semantic_id())
                        .text(measurement),
                ),
            )
            .child(
                div().child(position.clone()).semantic_in(
                    cx,
                    NodeSpec::new(ident.child("position").semantic_id(), Role::Text)
                        .parent(ident.semantic_id())
                        .text(position.clone()),
                ),
            );

        let mut root = div()
            .id(ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .when(zoomable, |element| element.tab_index(0).focus_ring(&theme))
            .child(
                div()
                    .row()
                    .w_full()
                    .gap_token(&theme, Space::Sm)
                    .justify_between()
                    .child(
                        div()
                            .row()
                            .gap_token(&theme, Space::Xs)
                            .child(previous)
                            .child(next)
                            .child(
                                div()
                                    .type_scale(&theme, TypeScale::Label)
                                    .text_color(theme.colors.text)
                                    .child(frame.label.clone()),
                            ),
                    )
                    .child(fits),
            )
            // The frame is measured through a plain wrapper, because only that
            // element carries the prepaint hook and only prepaint knows how
            // big the frame turned out — which is what zooming at a point and
            // clamping a pan are both computed against.
            .child(
                div()
                    .w_full()
                    .on_children_prepainted({
                        let measured = Rc::clone(&measured);
                        move |bounds, window, _| {
                            if let Some(first) = bounds.first() {
                                measure::record(&measured, *first, window);
                            }
                        }
                    })
                    .child(viewport)
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.child("frame").semantic_id(), Role::Image)
                            .parent(ident.semantic_id())
                            .text(frame.label.clone())
                            .busy(matches!(frame.state, ImageState::Loading))
                            .invalid(matches!(frame.state, ImageState::Failed(_)))
                            .value(frame.state.name()),
                    ),
            )
            .child(caption);

        if let (true, Some(geometry)) = (zoomable, geometry) {
            let report = Rc::clone(&report);
            let (min, max) = (self.min_zoom, self.max_zoom);
            let state = Rc::clone(&state);
            root.interactivity().on_key_down(move |event, window, cx| {
                let fit = match event.keystroke.key.as_str() {
                    "+" | "=" => FitMode::Zoom((geometry.scale * ZOOM_STEP).clamp(min, max)),
                    "-" => FitMode::Zoom((geometry.scale / ZOOM_STEP).clamp(min, max)),
                    // Resetting is the fit the viewer starts in, and it puts
                    // the picture back in the middle as well as back to size.
                    "0" => {
                        state.borrow_mut().pin = Pin::default();
                        FitMode::Contain
                    }
                    _ => return,
                };
                cx.stop_propagation();
                report(ImageViewerEvent::FitChanged(fit), window, cx);
            });
        }

        root.semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Group)
                .disabled(self.disabled)
                .text(frame.label.clone())
                .value(position),
        )
        .into_any_element()
    }
}

fn frame_extent(bounds: Bounds<Pixels>) -> Size<f32> {
    size(f32::from(bounds.size.width), f32::from(bounds.size.height))
}

/// How many notches of zoom one wheel event asks for.
///
/// A line-based wheel reports whole notches and a precise trackpad reports
/// pixels, so the pixels are divided by the same step a notch travels rather
/// than being read as notches themselves.
fn wheel_notches(delta: ScrollDelta) -> f32 {
    match delta {
        ScrollDelta::Lines(lines) => lines.y,
        ScrollDelta::Pixels(pixels) => f32::from(pixels.y) / 40.0,
    }
}

/// Reports an unsupplied image once for as long as the viewer is on screen.
fn report_missing(
    ident: &Ident,
    frame: &ImageFrame,
    handler: Option<&EventHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(handler) = handler.cloned() else {
        return;
    };
    let cell = keyed::slot::<Requested>(&ident.child("requested").semantic_id(), cx);
    if !cell.borrow_mut().0.insert(frame.id.clone()) {
        return;
    }
    handler(
        &ImageViewerEvent::ImageRequested(ImageRequest {
            id: frame.id.clone(),
            label: frame.label.clone(),
            source: frame.source.clone(),
            natural: frame.natural,
        }),
        window,
        cx,
    );
}

/// What stands in the frame when there is no picture in it.
///
/// The name and the host's own sentence, never a grey rectangle: a reader who
/// is shown a blank frame cannot tell a refusal from an image of nothing.
fn notice(
    theme: &Theme,
    tint: gpui::Hsla,
    title: SharedString,
    detail: SharedString,
) -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .column()
        .items_center()
        .justify_center()
        .gap_token(theme, Space::Xs)
        .p_token(theme, Space::Lg)
        .text_align(gpui::TextAlign::Center)
        .child(
            div()
                .type_scale(theme, TypeScale::Label)
                .text_color(theme.colors.text)
                .child(title),
        )
        .child(
            div()
                .max_w(px(360.0))
                .type_scale(theme, TypeScale::Caption)
                .text_color(tint)
                .child(detail),
        )
        .into_any_element()
}

/// A viewer the caller gave nothing to show.
fn empty_viewer(ident: &Ident, theme: &Theme, height: f32, cx: &mut App) -> AnyElement {
    div()
        .id(ident.element_id())
        .column()
        .w_full()
        .h(px(height))
        .items_center()
        .justify_center()
        .radius(theme, Radius::Card)
        .frame(theme, Surface::Raised, Elevation::Raised)
        .type_scale(theme, TypeScale::Label)
        .text_color(theme.colors.text_muted)
        .child(cx.strings().text(StringKey::ImageViewerEmpty))
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Group).value("empty"),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: Size<f32> = Size {
        width: 400.0,
        height: 300.0,
    };
    const NATURAL: Size<f32> = Size {
        width: 800.0,
        height: 400.0,
    };

    fn zoom() -> (f32, f32) {
        (0.1, 8.0)
    }

    #[test]
    fn contain_shows_the_whole_image_and_cover_fills_the_frame() {
        assert_eq!(scale_for(FitMode::Contain, FRAME, NATURAL, 0.1, 8.0), 0.5);
        assert_eq!(scale_for(FitMode::Cover, FRAME, NATURAL, 0.1, 8.0), 0.75);
        assert_eq!(scale_for(FitMode::Actual, FRAME, NATURAL, 0.1, 8.0), 1.0);
    }

    #[test]
    fn a_zoom_never_leaves_the_bounds_the_caller_set() {
        assert_eq!(
            scale_for(FitMode::Zoom(40.0), FRAME, NATURAL, 0.1, 4.0),
            4.0
        );
        assert_eq!(
            scale_for(FitMode::Zoom(0.001), FRAME, NATURAL, 0.25, 4.0),
            0.25
        );
    }

    #[test]
    fn an_unmeasured_frame_has_no_scale_to_report() {
        let empty = Size {
            width: 0.0,
            height: 0.0,
        };
        assert_eq!(scale_for(FitMode::Contain, empty, NATURAL, 0.1, 8.0), 0.0);
    }

    #[test]
    fn an_image_smaller_than_the_frame_is_centred_whatever_the_pin_says() {
        let drawn = Size {
            width: 200.0,
            height: 100.0,
        };
        let pushed = Pin {
            image: point(0.0, 0.0),
            frame: point(1.0, 1.0),
        };
        assert_eq!(place(FRAME, drawn, pushed), point(100.0, 100.0));
    }

    #[test]
    fn an_image_larger_than_the_frame_cannot_be_dragged_off_it() {
        let drawn = Size {
            width: 800.0,
            height: 400.0,
        };
        let far = Pin {
            image: point(0.0, 0.0),
            frame: point(1.0, 1.0),
        };
        assert_eq!(place(FRAME, drawn, far), point(0.0, 0.0));
        let further = Pin {
            image: point(1.0, 1.0),
            frame: point(0.0, 0.0),
        };
        assert_eq!(place(FRAME, drawn, further), point(-400.0, -100.0));
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer_under_it() {
        let before = layout(FitMode::Actual, FRAME, NATURAL, Pin::default(), zoom());
        let at = point(320.0, 60.0);
        let pinned = pin_at(FRAME, before, at);
        let after = layout(FitMode::Zoom(2.0), FRAME, NATURAL, pinned, zoom());

        let image_point_before = point(
            (at.x - before.offset.x) / before.drawn.width,
            (at.y - before.offset.y) / before.drawn.height,
        );
        let drawn_after = point(
            after.offset.x + image_point_before.x * after.drawn.width,
            after.offset.y + image_point_before.y * after.drawn.height,
        );
        assert!((drawn_after.x - at.x).abs() < 0.01, "{drawn_after:?}");
        assert!((drawn_after.y - at.y).abs() < 0.01, "{drawn_after:?}");
    }

    #[test]
    fn a_refused_zoom_moves_nothing() {
        let before = layout(FitMode::Actual, FRAME, NATURAL, Pin::default(), zoom());
        let pinned = pin_at(FRAME, before, point(320.0, 60.0));
        // The caller did not apply the new scale, so the frame renders at the
        // one that still holds and the picture must not have shifted.
        let after = layout(FitMode::Actual, FRAME, NATURAL, pinned, zoom());
        assert!((after.offset.x - before.offset.x).abs() < 0.01);
        assert!((after.offset.y - before.offset.y).abs() < 0.01);
    }

    #[test]
    fn panning_moves_the_picture_and_stops_at_its_edge() {
        let start = layout(FitMode::Actual, FRAME, NATURAL, Pin::default(), zoom());
        let nudged = pan_by(FRAME, start, point(50.0, 0.0));
        let moved = layout(FitMode::Actual, FRAME, NATURAL, nudged, zoom());
        assert!((moved.offset.x - (start.offset.x + 50.0)).abs() < 0.01);

        let shoved = pan_by(FRAME, start, point(5000.0, 0.0));
        let stopped = layout(FitMode::Actual, FRAME, NATURAL, shoved, zoom());
        assert_eq!(stopped.offset.x, 0.0);
    }

    #[test]
    fn only_a_picture_larger_than_the_frame_can_be_panned() {
        let contained = layout(FitMode::Contain, FRAME, NATURAL, Pin::default(), zoom());
        assert!(!contained.pannable(FRAME));
        let magnified = layout(FitMode::Zoom(2.0), FRAME, NATURAL, Pin::default(), zoom());
        assert!(magnified.pannable(FRAME));
    }

    #[test]
    fn a_precise_wheel_and_a_notched_one_both_read_as_notches() {
        assert_eq!(wheel_notches(ScrollDelta::Lines(point(0.0, 1.0))), 1.0);
        assert_eq!(
            wheel_notches(ScrollDelta::Pixels(point(px(0.0), px(40.0)))),
            1.0
        );
    }
}
