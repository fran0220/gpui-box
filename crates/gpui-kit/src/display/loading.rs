//! Loading indicators.
//!
//! Every mark here paints from `color.loader.*` — the neutral vocabulary of
//! work in progress — so a host retints the whole family from the token
//! document and no indicator invents a colour of its own. Waiting is not
//! information: the family is grey by default, and the only way a loader
//! shows a hue is a caller handing one over with a meaning attached.
//!
//! Every animation runs through the motion module's activity rhythms, and
//! every indicator checks reduced motion itself: a repeating animation held
//! at its first frame is not a quieter version of the animation, it is a
//! different picture, and for most loaders it is the picture of being stuck.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    canvas, div, px, relative,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Surface, TypeScale};

use crate::controls::button::Button;
use crate::display::progress_circle::arc;
use crate::display::signature;
use crate::foundation::{Ident, Sizable, StyledExt};
use crate::motion::{self, Activity, AnimationExt as _, MotionPolicy, MotionRole};
use crate::strings::{ActiveStrings, StringKey};

const PULSE_CELLS: usize = 3;
/// How far one breathing dot trails the one before it, as a fraction of the
/// breath.
const PULSE_STAGGER: f32 = 0.14;
/// How far one skeleton row's sheen trails the row above it.
const SHIMMER_ROW_OFFSET: f32 = 0.08;
/// How much of the turn a spinner's arc covers while it travels, and how much
/// it covers when reduced motion leaves it still. The still arc is longer:
/// with no travel to carry "working", the open gap is what says this is a
/// spinner rather than a ring that filled up.
const SPINNER_ARC: f32 = 0.25;
const SPINNER_STILL_ARC: f32 = 0.75;

/// A row of breathing dots, used while a request is in flight.
///
/// The quietest of the family: it claims only that something is being
/// waited on, so it breathes rather than turns.
#[derive(Debug, IntoElement)]
pub struct PulseLoader {
    ident: Ident,
    size: ControlSize,
    tint: Option<Hsla>,
    label: Option<SharedString>,
}

impl PulseLoader {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            size: ControlSize::Sm,
            tint: None,
            label: None,
        }
    }

    /// What the wait is for. Announced with the busy state.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A caller-meant colour in place of the neutral mark. The caller owns
    /// the meaning; the loader never picks a hue itself.
    pub fn tint(mut self, tint: impl Into<Hsla>) -> Self {
        self.tint = Some(tint.into());
        self
    }
}

impl Sizable for PulseLoader {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for PulseLoader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = self.tint.unwrap_or(signature::mark(theme));
        let dot = (theme.control.get(self.size).icon_size * 0.4).round();
        let motion = MotionPolicy::resolve_for(
            MotionRole::Activity(Activity::Deliberating),
            theme,
            cx.reduce_motion(),
        );
        let still = !motion.animates();
        let ident = self.ident.clone();
        let spec = busy_spec(&self.ident, self.label.clone());
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(dot / 2.0))
            .semantic_in(cx, spec)
            .children((0..PULSE_CELLS).map(move |index| {
                let cell = div().size(px(dot)).rounded_full().bg(color);
                if still {
                    // The state is carried by the published busy flag; a
                    // still row of solid dots reads as a mark, not a stall.
                    return cell.into_any_element();
                }
                cell.with_animation(
                    ident.indexed_element_id(index),
                    motion.spec().repeating(),
                    move |element, delta| {
                        let phase = motion::staggered_phase(delta, index, PULSE_STAGGER);
                        element.opacity(motion::breath(motion::pulse_wave(phase)))
                    },
                )
                .into_any_element()
            }))
    }
}

/// An inline turn, for a wait that sits next to a label.
///
/// An open arc travelling a quiet ring: the gap is what says the ring is not
/// a position. Under reduced motion the arc stands still and longer, which is
/// the resting spinner shape rather than a stalled sweep.
#[derive(Debug, IntoElement)]
pub struct Spinner {
    ident: Ident,
    size: ControlSize,
    tint: Option<Hsla>,
    label: Option<SharedString>,
}

impl Spinner {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            size: ControlSize::Sm,
            tint: None,
            label: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A caller-meant colour in place of the neutral mark.
    pub fn tint(mut self, tint: impl Into<Hsla>) -> Self {
        self.tint = Some(tint.into());
        self
    }
}

impl Sizable for Spinner {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Spinner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let diameter = theme.control.get(self.size).icon_size;
        let stroke = theme.borders.thick;
        let radius = (diameter - stroke) / 2.0;
        let track = signature::track(theme);
        let mark = self.tint.unwrap_or(signature::mark(theme));
        let spec = busy_spec(&self.ident, self.label.clone());

        let motion = MotionPolicy::resolve_for(
            MotionRole::Activity(Activity::Working),
            theme,
            cx.reduce_motion(),
        );
        let ring: AnyElement = if !motion.animates() {
            spinner_canvas(diameter, radius, stroke, track, mark, None).into_any_element()
        } else {
            div()
                .size(px(diameter))
                .with_animation(
                    self.ident.child("turn").element_id(),
                    motion.spec().repeating(),
                    move |element, phase| {
                        element.child(spinner_canvas(
                            diameter,
                            radius,
                            stroke,
                            track,
                            mark,
                            Some(phase),
                        ))
                    },
                )
                .into_any_element()
        };

        div()
            .flex()
            .flex_none()
            .items_center()
            .child(ring)
            .semantic_in(cx, spec)
    }
}

/// The spinner's ring at one phase of its travel, or at rest.
fn spinner_canvas(
    diameter: f32,
    radius: f32,
    stroke: f32,
    track: Hsla,
    mark: Hsla,
    phase: Option<f32>,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let centre = bounds.center();
            arc(window, centre, radius, stroke, 0.0, 1.0, track);
            match phase {
                Some(phase) => arc(
                    window,
                    centre,
                    radius,
                    stroke,
                    phase,
                    phase + SPINNER_ARC,
                    mark,
                ),
                None => arc(window, centre, radius, stroke, 0.0, SPINNER_STILL_ARC, mark),
            }
        },
    )
    .size(px(diameter))
}

/// One placeholder a skeleton can draw.
///
/// A row is the list case. The others exist so a card, an avatar, or a
/// paragraph can wait as themselves rather than as three identical bars.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SkeletonShape {
    Row { width: f32, height: f32 },
    Paragraph { lines: usize },
    Circle { size: f32 },
    Rect { width: f32, height: f32 },
    Card,
}

impl SkeletonShape {
    fn rows(self) -> Vec<(f32, f32, bool)> {
        match self {
            Self::Row { width, height } => vec![(width.clamp(0.16, 1.0), height, false)],
            Self::Paragraph { lines } => (0..lines.max(1))
                .map(|index| {
                    let width = if index + 1 == lines.max(1) { 0.62 } else { 1.0 };
                    (width, 12.0, false)
                })
                .collect(),
            Self::Circle { size } => vec![(size, size, true)],
            Self::Rect { width, height } => vec![(width.clamp(0.16, 1.0), height, false)],
            Self::Card => vec![(1.0, 72.0, false)],
        }
    }
}

/// Placeholder rows shown while a list's real shape is unknown.
///
/// Filled with `color.loader.placeholder`, which the token gate holds inside
/// a loudness band: quieter than content by contract, because a skeleton that
/// outshouts the page is announcing the absence of content as if it were the
/// content.
#[derive(Debug, IntoElement)]
pub struct Skeleton {
    ident: Ident,
    rows: usize,
    row_height: f32,
    /// How wide each row is, as a fraction of the frame. A list that is
    /// still loading rarely has every row the same length; the pattern is
    /// the claim, not a measurement of content that does not exist yet.
    widths: Vec<f32>,
    shapes: Vec<SkeletonShape>,
    label: Option<SharedString>,
}

impl Skeleton {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            rows: 3,
            row_height: 28.0,
            widths: Vec::new(),
            shapes: Vec::new(),
            label: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows;
        self
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = row_height;
        self
    }

    /// The width of each placeholder row, as a fraction of the frame.
    ///
    /// Missing entries reuse the last supplied width; an empty list leaves
    /// every row full-width. Values are clamped to `(0, 1]`.
    pub fn widths(mut self, widths: impl IntoIterator<Item = f32>) -> Self {
        self.widths = widths
            .into_iter()
            .map(|width| width.clamp(0.16, 1.0))
            .collect();
        self
    }

    /// Replaces the row list with an explicit sequence of shapes.
    ///
    /// [`Skeleton::rows`] and [`Skeleton::widths`] stay as the list case.
    pub fn shapes(mut self, shapes: impl IntoIterator<Item = SkeletonShape>) -> Self {
        self.shapes = shapes.into_iter().collect();
        self
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = theme.colors.loader_placeholder;
        let radius = theme.radii.small;
        let row_height = self.row_height;
        let motion = MotionPolicy::resolve_for(
            MotionRole::Activity(Activity::Advancing),
            theme,
            cx.reduce_motion(),
        );
        // A sheen held still is a bright sliver parked on one part of every
        // row; the placeholder fill carries the state on its own.
        let still = !motion.animates();
        let ident = self.ident.clone();
        let spec = busy_spec(&self.ident, self.label.clone());
        let bands: Vec<(f32, f32, bool)> = if self.shapes.is_empty() {
            (0..self.rows)
                .map(|index| {
                    let width = self
                        .widths
                        .get(index)
                        .copied()
                        .or_else(|| self.widths.last().copied())
                        .unwrap_or(1.0);
                    (width, row_height, false)
                })
                .collect()
        } else {
            self.shapes
                .into_iter()
                .flat_map(SkeletonShape::rows)
                .collect()
        };
        div()
            .flex()
            .flex_col()
            .gap(px(theme.space(Space::Sm) * 0.75))
            .semantic_in(cx, spec)
            .children(
                bands
                    .into_iter()
                    .enumerate()
                    .map(move |(index, (width, height, circle))| {
                        let mut row = div()
                            .h(px(height))
                            .rounded(px(if circle { height / 2.0 } else { radius }))
                            .bg(color)
                            .relative()
                            .overflow_hidden();
                        row = if circle {
                            row.flex_none().w(px(height))
                        } else {
                            row.w(relative(width))
                        };
                        if still {
                            return row;
                        }
                        row.child(signature::shimmer_band(theme).with_animation(
                            ident.indexed_element_id(index),
                            motion.spec().repeating(),
                            move |element, delta| {
                                let phase =
                                    motion::staggered_phase(delta, index, SHIMMER_ROW_OFFSET);
                                element.left(relative(motion::shimmer_offset(
                                    phase,
                                    signature::SHIMMER_BAND,
                                )))
                            },
                        ))
                    }),
            )
    }
}

/// A thin working strip for the edge of a region that is filling in.
///
/// The bar form of [`Spinner`]: same claim — running, extent unknown — for a
/// place that has a width rather than a corner. It never shows a position,
/// which is what separates it from an indeterminate
/// [`crate::display::progress::ProgressBar`]: that one is for work that will
/// gain a position, this one is for a surface that is simply not ready.
#[derive(Debug, IntoElement)]
pub struct BarLoader {
    ident: Ident,
    tint: Option<Hsla>,
    label: Option<SharedString>,
}

impl BarLoader {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            tint: None,
            label: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A caller-meant colour in place of the neutral mark.
    pub fn tint(mut self, tint: impl Into<Hsla>) -> Self {
        self.tint = Some(tint.into());
        self
    }
}

impl RenderOnce for BarLoader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let spec = busy_spec(&self.ident, self.label.clone());
        let sweep = match self.tint {
            Some(tint) => motion::sweep(self.ident.child("sweep").element_id(), &theme, tint, cx)
                .unwrap_or_else(|| signature::filled(tint, 1.0).into_any_element()),
            None => signature::unknown(self.ident.child("sweep").element_id(), &theme, cx),
        };
        div()
            .relative()
            .w_full()
            .h(px(3.0))
            .rounded_full()
            .overflow_hidden()
            .bg(signature::track(&theme))
            .child(sweep)
            .semantic_in(cx, spec)
    }
}

/// What the tail of a list is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadMoreState {
    /// More exists and fetching it is the reader's call.
    #[default]
    Idle,
    /// The next page is in flight.
    Loading,
    /// The list has said it has no more.
    Exhausted,
}

type MoreHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The tail row of an incrementally loaded list.
///
/// Three states, three different pictures: an affordance to fetch, a wait,
/// and an end. A list whose tail looks the same while it fetches and once it
/// is done has stopped saying which of the two holds.
#[derive(IntoElement)]
pub struct LoadMore {
    ident: Ident,
    state: LoadMoreState,
    on_more: Option<MoreHandler>,
}

impl std::fmt::Debug for LoadMore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LoadMore")
            .field("ident", &self.ident)
            .field("state", &self.state)
            .finish()
    }
}

impl LoadMore {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            state: LoadMoreState::default(),
            on_more: None,
        }
    }

    pub fn state(mut self, state: LoadMoreState) -> Self {
        self.state = state;
        self
    }

    /// What fetching the next page does. Without it the idle tail shows the
    /// fact and installs nothing, because a control that cannot act is not
    /// drawn as one.
    pub fn on_more(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_more = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for LoadMore {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .w_full()
            .gap(px(theme.space(Space::Sm)))
            .py(px(theme.space(Space::Sm)));
        match self.state {
            LoadMoreState::Idle => {
                let label = cx.strings().text(StringKey::LoadMore);
                let spec =
                    NodeSpec::new(self.ident.semantic_id(), Role::Status).text(label.clone());
                match self.on_more {
                    Some(handler) => row
                        .child(
                            // A tail that can fetch is a control, and it wears
                            // a boundary: ghost chrome in the middle of a list
                            // is indistinguishable from the caption two rows
                            // below it, which cannot be pressed at all.
                            Button::new(self.ident.child("more"))
                                .label(label)
                                .secondary()
                                .control_size(ControlSize::Sm)
                                .semantic_parent(self.ident.semantic_id())
                                .on_click(move |window, cx| handler(window, cx)),
                        )
                        .semantic_in(cx, spec),
                    None => row
                        .child(
                            div()
                                .type_scale(&theme, TypeScale::Caption)
                                .text_color(theme.colors.text_faint)
                                .child(label),
                        )
                        .semantic_in(cx, spec),
                }
            }
            LoadMoreState::Loading => {
                let label = cx.strings().text(StringKey::LoadMoreLoading);
                let spec = busy_spec(&self.ident, Some(label.clone()));
                row.child(PulseLoader::new(self.ident.child("wait")).control_size(ControlSize::Xs))
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Caption)
                            .text_color(theme.colors.text_muted)
                            .child(label),
                    )
                    .semantic_in(cx, spec)
            }
            LoadMoreState::Exhausted => {
                let label = cx.strings().text(StringKey::LoadMoreEnd);
                let spec =
                    NodeSpec::new(self.ident.semantic_id(), Role::Status).text(label.clone());
                row.child(
                    div()
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_faint)
                        .child(label),
                )
                .semantic_in(cx, spec)
            }
        }
    }
}

/// A veil over content that is still the last verified value.
///
/// The content stays. The veil says a refresh is in flight. Erasing the
/// content would be the lie [`crate::state::AsyncValue`] exists to prevent.
#[derive(IntoElement)]
pub struct RefreshVeil {
    ident: Ident,
    content: AnyElement,
    label: Option<SharedString>,
}

impl std::fmt::Debug for RefreshVeil {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RefreshVeil")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .finish()
    }
}

impl RefreshVeil {
    pub fn new(ident: impl Into<Ident>, content: impl IntoElement) -> Self {
        Self {
            ident: ident.into(),
            content: content.into_any_element(),
            label: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl RenderOnce for RefreshVeil {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let spec = busy_spec(&self.ident, self.label.clone());
        // The mark sits in its own chip below the content rather than over the
        // letterforms it is veiling: a chip drawn across text collides with it
        // in exactly the frame a reader is trying to keep reading, and one
        // taller than short content would overflow onto whatever sits above.
        let chip = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(theme.space(Space::Sm)))
            .px(px(theme.space(Space::Md)))
            .py(px(theme.space(Space::Sm)))
            .radius(&theme, Radius::Control)
            .surface(&theme, Surface::Raised)
            .hairline(&theme)
            .child(Spinner::new(self.ident.child("spin")))
            .when_some_label(&theme, self.label.clone());
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(theme.space(Space::Sm)))
            .w_full()
            .child(
                div().relative().w_full().child(self.content).child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(theme.surface(Surface::Panel).opacity(theme.opacity.scrim)),
                ),
            )
            .child(chip)
            .semantic_in(cx, spec)
    }
}

/// Adds the veil's label beside its spinner, when one was given.
trait VeilLabel {
    fn when_some_label(self, theme: &gpui_kit_theme::Theme, label: Option<SharedString>) -> Self;
}

impl VeilLabel for gpui::Div {
    fn when_some_label(self, theme: &gpui_kit_theme::Theme, label: Option<SharedString>) -> Self {
        match label {
            Some(label) if !label.is_empty() => self.child(
                div()
                    .type_scale(theme, TypeScale::Label)
                    .text_color(theme.colors.text_muted)
                    .child(label),
            ),
            _ => self,
        }
    }
}

/// Loading is a distinct state, so every indicator publishes it rather than
/// leaving a test to infer a wait from an absence of content.
fn busy_spec(ident: &Ident, label: Option<SharedString>) -> NodeSpec {
    let mut spec = NodeSpec::new(ident.semantic_id(), Role::Progress).busy(true);
    if let Some(label) = label {
        spec = spec.text(label);
    }
    spec
}
