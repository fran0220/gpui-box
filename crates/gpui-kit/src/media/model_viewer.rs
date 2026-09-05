//! A bounded viewer for a 3D model: orbit, flat shading, and a refusal.
//!
//! # What this component will not do
//!
//! **It does not load anything.** The bytes are read by
//! [`ModelScene::parse`], which the host calls, and the outcome — a scene or a
//! refusal — is what this component is handed. There is no file, no network,
//! and no asset resolution here.
//!
//! **It does not draw what it did not read.** The reader takes positions and
//! triangles; it does not take materials, textures, or normals. So the model
//! is drawn flat-shaded from face normals it computed, or as a wireframe, and
//! neither is presented as the material the document described.
//!
//! **It does not scale a refusal down into an empty frame.** A document past
//! a bound and a document outside the subset are two different sentences, and
//! each names what it asked for and what was allowed. A viewer with no model
//! at all is a third.
//!
//! **It does not turn the model by itself.** Orbit is caller-owned, exactly
//! as an image viewer's fit is: a drag reports the angles it asks for, and the
//! model turns when the caller says it did.

use std::f32::consts::PI;
use std::rc::Rc;

use gpui::{
    App, Bounds, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement, PathBuilder,
    Pixels, Point, RenderOnce, SharedString, Size, Styled, Window, canvas, div,
    prelude::FluentBuilder, px, size,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, TextTone, TypeScale,
};

use crate::controls::button::IconButton;
use crate::controls::segmented::{Segment, SegmentedControl};
use crate::foundation::{Disableable, FocusRing, Ident, Sizable, StyledExt, text};
use crate::layout::measure;
use crate::media::gltf::{ModelBounds, ModelError, ModelScene};
use crate::media::notice;
use crate::motion::keyed;
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// How tall the frame is when it holds a sentence rather than a model.
const SENTENCE_HEIGHT: f32 = 112.0;

/// How much of the shorter side of the frame the model's own sphere fills.
const FIT: f32 = 0.42;

/// How far a full drag across the frame turns the model, in turns.
const DRAG_TURNS: f32 = 1.0;

/// How far the pitch may go before the model would pass through its own pole.
const PITCH_LIMIT: f32 = PI / 2.0 - 0.01;

/// The darkest a lit face gets, so a face turned away is still a face.
const AMBIENT: f32 = 0.32;

/// How the model is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModelShading {
    /// Every face filled, lit by its own normal. Nothing here is the
    /// document's material, because the reader does not read one.
    #[default]
    Flat,
    /// Every triangle's three edges, and no fill.
    Wireframe,
}

impl ModelShading {
    /// The name a semantic node publishes and a control addresses.
    pub fn name(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Wireframe => "wireframe",
        }
    }
}

/// What the viewer has, as the host reports it.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelState {
    /// No model has been handed to this viewer.
    Empty,
    /// The host is still reading one.
    Loading,
    /// A document the reader accepted.
    Ready(Rc<ModelScene>),
    /// A document the reader refused, and why.
    Rejected(ModelError),
}

impl ModelState {
    /// Reads a document and answers with the state it produced.
    ///
    /// This is the whole of what a host has to do: the refusal is a state and
    /// not an error to be handled somewhere the reader cannot see it.
    pub fn read(bytes: &[u8], bounds: ModelBounds) -> Self {
        match ModelScene::parse(bytes, bounds) {
            Ok(scene) => Self::Ready(Rc::new(scene)),
            Err(error) => Self::Rejected(error),
        }
    }

    /// The name a semantic node publishes.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Loading => "loading",
            Self::Ready(_) => "ready",
            Self::Rejected(error) => error.name(),
        }
    }
}

impl HasPhase for ModelState {
    fn phase(&self) -> Phase {
        match self {
            Self::Empty => Phase::Empty,
            Self::Loading => Phase::Loading,
            Self::Ready(_) => Phase::Ready,
            Self::Rejected(_) => Phase::Unavailable,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Rejected(error) => Some(error.name()),
            _ => None,
        }
    }
}

/// What a model viewer reports. It applies none of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModelViewerEvent {
    /// A drag or the reset control asked for these angles, in radians.
    OrbitChanged { yaw: f32, pitch: f32 },
    /// The reader asked for the other shading.
    ShadingChanged(ModelShading),
}

type EventHandler = Rc<dyn Fn(&ModelViewerEvent, &mut Window, &mut App)>;

/// What a drag remembers between two builds of the same viewer.
#[derive(Debug, Default)]
struct Orbiting {
    held: bool,
    at: Option<Point<Pixels>>,
}

/// A bounded viewer for one 3D model.
#[derive(IntoElement)]
pub struct ModelViewer {
    ident: Ident,
    title: Option<SharedString>,
    state: ModelState,
    shading: ModelShading,
    yaw: f32,
    pitch: f32,
    height: Option<f32>,
    disabled: bool,
    on_event: Option<EventHandler>,
}

impl std::fmt::Debug for ModelViewer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelViewer")
            .field("ident", &self.ident)
            .field("state", &self.state.name())
            .field("shading", &self.shading)
            .field("orbit", &(self.yaw, self.pitch))
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_event.is_some())
            .finish()
    }
}

impl ModelViewer {
    /// A viewer with nothing in it.
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            title: None,
            state: ModelState::Empty,
            shading: ModelShading::default(),
            yaw: PI / 6.0,
            pitch: PI / 8.0,
            height: None,
            disabled: false,
            on_event: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn state(mut self, state: ModelState) -> Self {
        self.state = state;
        self
    }

    /// A document the reader accepted.
    pub fn scene(self, scene: Rc<ModelScene>) -> Self {
        self.state(ModelState::Ready(scene))
    }

    /// A document the reader refused.
    pub fn rejected(self, error: ModelError) -> Self {
        self.state(ModelState::Rejected(error))
    }

    pub fn loading(self) -> Self {
        self.state(ModelState::Loading)
    }

    pub fn shading(mut self, shading: ModelShading) -> Self {
        self.shading = shading;
        self
    }

    /// Where the caller says the camera stands, in radians. The viewer draws
    /// this and reports every request to change it.
    pub fn orbit(mut self, yaw: f32, pitch: f32) -> Self {
        self.yaw = yaw;
        self.pitch = pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(1.0));
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(&ModelViewerEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

impl Disableable for ModelViewer {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for ModelViewer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();
        let strings = cx.strings().clone();
        let actionable = !self.disabled && self.on_event.is_some();

        let report = {
            let handler = self.on_event.clone().filter(|_| actionable);
            Rc::new(
                move |event: ModelViewerEvent, window: &mut Window, cx: &mut App| {
                    if let Some(handler) = &handler {
                        handler(&event, window, cx);
                    }
                },
            )
        };

        let measured = measure::cell(&ident.child("frame").semantic_id(), window, cx);
        let dragging =
            keyed::slot::<Orbiting>(&ident.semantic_id(), window.window_handle().window_id(), cx);

        let camera = Camera {
            yaw: self.yaw,
            pitch: self.pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT),
        };

        // A frame with a model in it, or one waiting for the model that will
        // fill it, keeps the shape it was given. A refusal and an empty
        // document have nothing that wants that shape, so they take the height
        // of the sentence in them: a viewport kept at full height around two
        // lines of text is mostly a claim that something is about to appear
        // there.
        let vacant = matches!(self.state, ModelState::Rejected(_) | ModelState::Empty);
        let viewer_height = self.height.unwrap_or(theme.measures.media_viewer_height);
        let height = if vacant {
            viewer_height.min(SENTENCE_HEIGHT)
        } else {
            viewer_height
        };
        let mut frame = div()
            .id(ident.child("frame").element_id())
            .relative()
            .w_full()
            .h(px(height))
            .overflow_hidden()
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Sunken, Elevation::Flat);

        match &self.state {
            ModelState::Ready(scene) => {
                frame = frame.child(
                    div()
                        .absolute()
                        .inset_0()
                        // Geometry is not information: a face is drawn in the
                        // neutral the shading ramp is built from, and what the
                        // reader is being told about the model is its shape.
                        .child(paint(
                            Rc::clone(scene),
                            camera,
                            self.shading,
                            theme.colors.text_muted,
                            theme.colors.hairline_strong,
                        ))
                        // Where the camera stands is published only where
                        // there is something for it to look at.
                        .semantic_in(
                            cx,
                            NodeSpec::new(ident.child("camera").semantic_id(), Role::Status)
                                .parent(ident.semantic_id())
                                .value(
                                    strings.format(
                                        StringKey::ModelCamera,
                                        &[
                                            cx.numbers()
                                                .decimal(degrees(camera.yaw) as f64, 0)
                                                .as_ref(),
                                            cx.numbers()
                                                .decimal(degrees(camera.pitch) as f64, 0)
                                                .as_ref(),
                                        ],
                                    ),
                                ),
                        ),
                );
            }
            ModelState::Loading => {
                frame = frame.child(notice(
                    &theme,
                    theme.colors.text_muted,
                    None,
                    self.title
                        .clone()
                        .unwrap_or_else(|| strings.text(StringKey::ModelEmpty)),
                    strings.text(StringKey::Loading),
                ));
            }
            ModelState::Empty => {
                frame = frame.child(notice(
                    &theme,
                    theme.colors.text_muted,
                    None,
                    strings.text(StringKey::ModelEmpty),
                    SharedString::default(),
                ));
            }
            ModelState::Rejected(error) => {
                frame = frame.child(notice(
                    &theme,
                    theme.colors.danger,
                    None,
                    strings.text(StringKey::ModelRefused),
                    refusal(&strings, *error, cx),
                ));
            }
        }

        let orbitable = actionable && matches!(self.state, ModelState::Ready(_));
        if orbitable {
            let down = Rc::clone(&dragging);
            frame = frame
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |event, _, _| {
                    let mut state = down.borrow_mut();
                    state.held = true;
                    state.at = Some(event.position);
                });

            let moved = Rc::clone(&dragging);
            let bounds = Rc::clone(&measured);
            let turn = Rc::clone(&report);
            let (yaw, pitch) = (camera.yaw, camera.pitch);
            frame = frame.on_mouse_move(move |event, window, cx| {
                let previous = {
                    let mut state = moved.borrow_mut();
                    if !state.held {
                        return;
                    }
                    if event.pressed_button != Some(MouseButton::Left) {
                        state.held = false;
                        state.at = None;
                        return;
                    }
                    let previous = state.at;
                    state.at = Some(event.position);
                    previous
                };
                let Some(previous) = previous else {
                    return;
                };
                let extent = frame_extent(bounds.get());
                if extent.width <= 0.0 || extent.height <= 0.0 {
                    return;
                }
                let (yaw, pitch) = orbit_by(
                    yaw,
                    pitch,
                    point(
                        f32::from(event.position.x - previous.x),
                        f32::from(event.position.y - previous.y),
                    ),
                    extent,
                );
                turn(ModelViewerEvent::OrbitChanged { yaw, pitch }, window, cx);
            });

            let cancelled = Rc::clone(&dragging);
            frame = frame.child(crate::interaction::on_pointer_cancel(move |_, _| {
                let mut state = cancelled.borrow_mut();
                state.held = false;
                state.at = None;
            }));
            let up = Rc::clone(&dragging);
            frame = frame.on_mouse_up(MouseButton::Left, move |_, _, _| {
                let mut state = up.borrow_mut();
                state.held = false;
                state.at = None;
            });
        }

        let shadings = {
            let report = Rc::clone(&report);
            let mut control = SegmentedControl::new(ident.child("shading"))
                .control_size(ControlSize::Sm)
                .segments([
                    Segment::new(
                        ModelShading::Flat.name(),
                        strings.text(StringKey::ModelFlat),
                    ),
                    Segment::new(
                        ModelShading::Wireframe.name(),
                        strings.text(StringKey::ModelWireframe),
                    ),
                ])
                .selected(self.shading.name())
                .disabled(!orbitable);
            if orbitable {
                control = control.on_select(move |id, window, cx| {
                    let shading = match id.as_ref() {
                        "wireframe" => ModelShading::Wireframe,
                        _ => ModelShading::Flat,
                    };
                    report(ModelViewerEvent::ShadingChanged(shading), window, cx);
                });
            }
            control
        };

        let reset = {
            let report = Rc::clone(&report);
            let mut control = IconButton::new(
                ident.child("reset"),
                Icon::Refresh,
                strings.text(StringKey::ModelReset),
            )
            .ghost()
            .control_size(ControlSize::Sm)
            .semantic_parent(ident.semantic_id())
            .disabled(!orbitable);
            if orbitable {
                control = control.on_click(move |window, cx| {
                    report(
                        ModelViewerEvent::OrbitChanged {
                            yaw: PI / 6.0,
                            pitch: PI / 8.0,
                        },
                        window,
                        cx,
                    )
                });
            }
            control
        };

        // Every number here is one the reader counted. A viewer holding no
        // model publishes no counts rather than three zeroes, because zero
        // triangles is a thing a document can contain and this is not it.
        let counts = match &self.state {
            ModelState::Ready(scene) => Some(
                div()
                    .row()
                    .w_full()
                    .flex_wrap()
                    .gap_token(&theme, Space::Md)
                    .child(count(
                        cx,
                        &theme,
                        &strings,
                        &ident,
                        "meshes",
                        strings.text(StringKey::ModelMeshes),
                        scene.mesh_count(),
                    ))
                    .child(count(
                        cx,
                        &theme,
                        &strings,
                        &ident,
                        "vertices",
                        strings.text(StringKey::ModelVertices),
                        scene.vertex_count(),
                    ))
                    .child(count(
                        cx,
                        &theme,
                        &strings,
                        &ident,
                        "triangles",
                        strings.text(StringKey::ModelTriangles),
                        scene.triangle_count(),
                    )),
            ),
            _ => None,
        };

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Group)
            .disabled(self.disabled)
            .busy(matches!(self.state, ModelState::Loading))
            .invalid(matches!(self.state, ModelState::Rejected(_)))
            .value(self.state.name());
        if let Some(title) = self.title.clone() {
            spec = spec.text(title);
        }
        if matches!(self.state, ModelState::Empty) {
            spec = spec.description(strings.text(StringKey::ModelEmptyDetail));
        }

        div()
            .id(ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .when(orbitable, |element| element.tab_index(0).focus_ring(&theme))
            .child(
                div()
                    .row()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_token(&theme, Space::Sm)
                    .children(
                        self.title
                            .clone()
                            .map(|title| text(&theme, TypeScale::Subtitle, title)),
                    )
                    .child(
                        div()
                            .row()
                            .gap_token(&theme, Space::Xs)
                            .child(shadings)
                            .child(reset),
                    ),
            )
            // The frame is measured through a plain wrapper, because only that
            // element carries the prepaint hook and only prepaint knows how
            // wide the frame turned out — which is what a drag's angle per
            // pixel is computed against.
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
                    .child(frame)
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.child("frame").semantic_id(), Role::Image)
                            .parent(ident.semantic_id())
                            .busy(matches!(self.state, ModelState::Loading))
                            .invalid(matches!(self.state, ModelState::Rejected(_)))
                            .value(self.state.name()),
                    ),
            )
            .children(counts)
            .semantic_in(cx, spec)
    }
}

/// One counted fact, published so a test reads the count rather than the row.
fn count(
    cx: &mut App,
    theme: &gpui_kit_theme::Theme,
    strings: &crate::strings::Strings,
    ident: &Ident,
    name: &'static str,
    label: SharedString,
    value: usize,
) -> impl IntoElement {
    text(
        theme,
        TypeScale::Caption,
        strings.format(
            StringKey::ModelCount,
            &[&label, cx.numbers().count(value).as_ref()],
        ),
    )
    .text_tone(theme, TextTone::Muted)
    .semantic_in(
        cx,
        NodeSpec::new(ident.child(name).semantic_id(), Role::Text)
            .parent(ident.semantic_id())
            .text(label)
            .value(cx.numbers().count(value)),
    )
}

/// The host-facing sentence for a refusal, with the reader's own code in it.
fn refusal(strings: &crate::strings::Strings, error: ModelError, cx: &App) -> SharedString {
    match error {
        ModelError::TooLarge {
            limit,
            found,
            allowed,
        } => strings.format(
            StringKey::ModelTooLarge,
            &[
                limit.name(),
                cx.numbers().count(found).as_ref(),
                cx.numbers().count(allowed).as_ref(),
            ],
        ),
        ModelError::Rejected(defect) => strings.format(StringKey::ModelRejected, &[defect.name()]),
    }
}

fn degrees(radians: f32) -> i64 {
    (radians * 180.0 / PI).round() as i64
}

fn frame_extent(bounds: Bounds<Pixels>) -> Size<f32> {
    size(f32::from(bounds.size.width), f32::from(bounds.size.height))
}

fn point(x: f32, y: f32) -> Point<f32> {
    Point { x, y }
}

/// Where the camera stands. Distance is not one of its facts: the projection
/// is orthographic and fits the model's own sphere, so orbiting cannot change
/// how big the model is.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Camera {
    yaw: f32,
    pitch: f32,
}

impl Camera {
    /// A point in the document's space, in the camera's.
    ///
    /// The camera looks down its own negative Z, so a larger `z` is nearer,
    /// which is what both the depth sort and the facing test read.
    fn view(self, point: [f32; 3], centre: [f32; 3]) -> [f32; 3] {
        let (x, y, z) = (
            point[0] - centre[0],
            point[1] - centre[1],
            point[2] - centre[2],
        );
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (x, z) = (x * cos_yaw + z * sin_yaw, -x * sin_yaw + z * cos_yaw);
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let (y, z) = (y * cos_pitch - z * sin_pitch, y * sin_pitch + z * cos_pitch);
        [x, y, z]
    }
}

/// The angles a drag of `delta` across a frame of `extent` asks for.
fn orbit_by(yaw: f32, pitch: f32, delta: Point<f32>, extent: Size<f32>) -> (f32, f32) {
    let turns = |travel: f32, across: f32| {
        if across <= 0.0 {
            0.0
        } else {
            travel / across * DRAG_TURNS * 2.0 * PI
        }
    };
    (
        yaw + turns(delta.x, extent.width),
        // Pitch stops short of the pole: past it the model would turn inside
        // out, which reads as a rendering fault rather than as a limit.
        (pitch + turns(delta.y, extent.height)).clamp(-PITCH_LIMIT, PITCH_LIMIT),
    )
}

/// How much of a face's own colour a normal pointing this way keeps.
///
/// The light is fixed and in front of the model, so a face turned away is
/// darker rather than black: nothing here is the document's material, and a
/// black face would read as a hole in the geometry.
fn shade(normal: [f32; 3]) -> f32 {
    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if length <= f32::EPSILON {
        return AMBIENT;
    }
    const LIGHT: [f32; 3] = [0.35, 0.58, 0.74];
    let lambert = (normal[0] * LIGHT[0] + normal[1] * LIGHT[1] + normal[2] * LIGHT[2]) / length;
    AMBIENT + (1.0 - AMBIENT) * lambert.clamp(0.0, 1.0)
}

/// The face normal of a triangle already in camera space.
fn normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

/// One triangle, ready to paint.
struct Face {
    corners: [Point<Pixels>; 3],
    depth: f32,
    shade: f32,
}

/// Every triangle of a scene, projected, culled, and sorted back to front.
fn faces(scene: &ModelScene, camera: Camera, bounds: Bounds<Pixels>, cull: bool) -> Vec<Face> {
    let extent = frame_extent(bounds);
    let aabb = scene.aabb();
    let radius = aabb.radius();
    if extent.width <= 0.0 || extent.height <= 0.0 || radius <= f32::EPSILON {
        return Vec::new();
    }
    let centre = aabb.centre();
    let scale = FIT * extent.width.min(extent.height) / radius;
    let origin = (
        f32::from(bounds.origin.x) + extent.width / 2.0,
        f32::from(bounds.origin.y) + extent.height / 2.0,
    );
    // Screen y grows downwards and the model's does not, so the projection
    // negates it rather than the model being flipped on the way in.
    let project = |view: [f32; 3]| Point {
        x: px(origin.0 + view[0] * scale),
        y: px(origin.1 - view[1] * scale),
    };

    let mut out = Vec::new();
    for mesh in scene.meshes() {
        let positions = mesh.positions();
        for triangle in mesh.indices().chunks_exact(3) {
            let Some(corners) = triangle
                .iter()
                .map(|index| positions.get(*index as usize).copied())
                .collect::<Option<Vec<[f32; 3]>>>()
            else {
                continue;
            };
            let view = [
                camera.view(corners[0], centre),
                camera.view(corners[1], centre),
                camera.view(corners[2], centre),
            ];
            let normal = normal(view[0], view[1], view[2]);
            // glTF winds a front face counter-clockwise, so a normal pointing
            // away from the camera is the back of a surface. A wireframe keeps
            // both, because the far edges are what makes it read as a solid.
            if cull && normal[2] <= 0.0 {
                continue;
            }
            out.push(Face {
                corners: [project(view[0]), project(view[1]), project(view[2])],
                depth: (view[0][2] + view[1][2] + view[2][2]) / 3.0,
                shade: shade(normal),
            });
        }
    }
    // Painter's order: the furthest face is drawn first, so a nearer one
    // covers it. There is no depth buffer to reach from a canvas.
    out.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    out
}

/// The canvas the model is drawn on.
fn paint(
    scene: Rc<ModelScene>,
    camera: Camera,
    shading: ModelShading,
    fill: Hsla,
    line: Hsla,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let wireframe = matches!(shading, ModelShading::Wireframe);
            for face in faces(&scene, camera, bounds, !wireframe) {
                let mut builder = if wireframe {
                    PathBuilder::stroke(px(1.0))
                } else {
                    PathBuilder::fill()
                };
                builder.move_to(face.corners[0]);
                builder.line_to(face.corners[1]);
                builder.line_to(face.corners[2]);
                builder.close();
                let Ok(path) = builder.build() else {
                    continue;
                };
                let color = if wireframe {
                    line
                } else {
                    Hsla {
                        l: (fill.l * face.shade).clamp(0.0, 1.0),
                        ..fill
                    }
                };
                window.paint_path(path, color);
                if wireframe {
                    continue;
                }
                // Two filled triangles that share an edge each antialias their
                // own side of it, and the two half-covered runs do not add back
                // to one: the join shows as a lighter seam across a face that
                // is one flat surface. Stroking the same outline in the same
                // colour closes it.
                let mut edge = PathBuilder::stroke(px(1.0));
                edge.move_to(face.corners[0]);
                edge.line_to(face.corners[1]);
                edge.line_to(face.corners[2]);
                edge.close();
                if let Ok(edge) = edge.build() {
                    window.paint_path(edge, color);
                }
            }
        },
    )
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXTENT: Size<f32> = Size {
        width: 400.0,
        height: 300.0,
    };

    #[test]
    fn a_drag_across_the_frame_turns_the_model_a_whole_turn() {
        let (yaw, _) = orbit_by(0.0, 0.0, point(400.0, 0.0), EXTENT);
        assert!(
            (yaw - 2.0 * PI * DRAG_TURNS).abs() < 0.001,
            "a drag the width of the frame is one turn: {yaw}"
        );
    }

    #[test]
    fn a_drag_past_the_pole_stops_short_of_it() {
        let (_, up) = orbit_by(0.0, 0.0, point(0.0, 5000.0), EXTENT);
        assert!((up - PITCH_LIMIT).abs() < 0.001);
        let (_, down) = orbit_by(0.0, 0.0, point(0.0, -5000.0), EXTENT);
        assert!((down + PITCH_LIMIT).abs() < 0.001);
    }

    #[test]
    fn a_frame_nobody_measured_turns_nothing() {
        let flat = Size {
            width: 0.0,
            height: 0.0,
        };
        assert_eq!(orbit_by(1.0, 0.5, point(80.0, 80.0), flat), (1.0, 0.5));
    }

    #[test]
    fn the_camera_turns_the_model_rather_than_moving_it() {
        let camera = Camera {
            yaw: PI / 2.0,
            pitch: 0.0,
        };
        let turned = camera.view([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        assert!(turned[0].abs() < 0.001, "{turned:?}");
        assert!(
            (turned[2] + 1.0).abs() < 0.001,
            "a quarter turn puts +X behind"
        );

        let still = Camera {
            yaw: 0.0,
            pitch: 0.0,
        };
        assert_eq!(
            still.view([2.0, 3.0, 4.0], [1.0, 1.0, 1.0]),
            [1.0, 2.0, 3.0]
        );
    }

    #[test]
    fn a_face_turned_away_is_darker_and_never_black() {
        let towards = shade([0.35, 0.58, 0.74]);
        let away = shade([-0.35, -0.58, -0.74]);
        assert!(towards > away);
        assert!(away >= AMBIENT, "an unlit face is still a face: {away}");
        assert!(towards <= 1.0);
        assert_eq!(shade([0.0, 0.0, 0.0]), AMBIENT);
    }

    #[test]
    fn a_normal_points_out_of_a_counter_clockwise_face() {
        let facing = normal([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!(facing[2] > 0.0, "{facing:?}");
        let away = normal([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]);
        assert!(away[2] < 0.0, "{away:?}");
    }

    #[test]
    fn a_state_names_itself_and_a_refusal_names_the_refusal() {
        assert_eq!(ModelState::Empty.name(), "empty");
        assert_eq!(
            ModelState::Rejected(ModelError::TooLarge {
                limit: crate::media::gltf::ModelLimit::Vertices,
                found: 9,
                allowed: 2,
            })
            .name(),
            "too-large"
        );
    }
}

#[cfg(test)]
mod model_phase_tests {
    use super::*;

    #[test]
    fn a_rejected_document_is_unavailable() {
        assert_eq!(ModelState::Empty.phase(), Phase::Empty);
        assert_eq!(ModelState::Loading.phase(), Phase::Loading);
    }
}
