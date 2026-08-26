//! Playback controls for audio or video. Nothing here plays anything.
//!
//! # What this component will not do
//!
//! **It does not play, seek, or apply.** Every control reports a
//! [`TransportEvent`] and stops. The position drawn is the one the caller says
//! is true, so a host that refuses a seek keeps the position that still holds
//! — the same rule the slider, the sort, and the selection keep.
//!
//! **It does not format a time.** The elapsed and remaining readouts are
//! strings the host already wrote, exactly as [`crate::display::timeline::Timeline`] and
//! [`crate::content::MessageList`] take theirs, because turning a number of
//! seconds into words is locale work and whoever owns the clock owns the
//! wording. The numeric position is separate, and it is what drives the track.
//!
//! **It does not invent a duration.** A live stream has a position and no
//! total, which is the state [`crate::navigation::PageTotal::Unknown`] already
//! names for pages: the scrubber then shows elapsed, says the total is
//! unknown, and draws no track fraction at all rather than a fraction of
//! nothing.
//!
//! **It does not imply a buffer.** Buffered ranges are the host's, drawn
//! distinctly from the played position; a host that supplies none gets no
//! buffer drawn, not a full one.
//!
//! **It does not call waiting "paused".** A transport that is playing and
//! stalled says it is waiting for data. A reader who is told it is paused
//! would reach for the control that would actually stop it.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder, px, relative,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, TypeScale};

use crate::controls::button::{Button, IconButton};
use crate::controls::segmented::{Segment, SegmentedControl};
use crate::controls::slider::Slider;
use crate::display::badge::Tone;
use crate::display::signature;
use crate::display::status::StatusLine;
use crate::foundation::{Disableable, FocusRing, Ident, Sizable, StyledExt};
use crate::layout::measure;
use crate::motion::{self, keyed};
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// How tall the scrubber's track is, and how wide the volume control is.
/// Neither value repeats anywhere else.
const TRACK_HEIGHT: f32 = 4.0;
const KNOB: f32 = 11.0;
const VOLUME_WIDTH: f32 = 96.0;

/// How far one arrow key moves the position when the caller says nothing.
const DEFAULT_STEP: f32 = 5.0;

/// How long the media runs, as far as the host knows.
///
/// `Unknown` is a live stream: there is a position and there is no total, and
/// a bar that drew a fraction anyway would be reporting a number nobody has.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransportDuration {
    Known(f32),
    Unknown,
}

impl TransportDuration {
    pub fn is_known(self) -> bool {
        matches!(self, Self::Known(_))
    }

    pub fn seconds(self) -> Option<f32> {
        match self {
            Self::Known(seconds) if seconds > 0.0 => Some(seconds),
            _ => None,
        }
    }
}

/// What the transport is doing, as the host reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransportState {
    Playing,
    #[default]
    Paused,
    /// Playing, and waiting for data. This is not paused, and it does not
    /// offer the control that would resume, because nothing has stopped.
    Buffering,
}

impl TransportState {
    /// The name a semantic node publishes, so a test asserts the state rather
    /// than the label drawn for it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Buffering => "buffering",
        }
    }

    fn is_playing(self) -> bool {
        matches!(self, Self::Playing | Self::Buffering)
    }
}

impl HasPhase for TransportState {
    fn phase(&self) -> Phase {
        match self {
            Self::Playing | Self::Paused => Phase::Ready,
            Self::Buffering => Phase::Refreshing,
        }
    }
}

/// One span of media the host already holds, in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BufferedRange {
    pub start: f32,
    pub end: f32,
}

impl BufferedRange {
    pub fn new(start: f32, end: f32) -> Self {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        Self { start, end }
    }
}

/// Which way a previous or next control steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackStep {
    Previous,
    Next,
}

impl TrackStep {
    pub fn name(self) -> &'static str {
        match self {
            Self::Previous => "previous",
            Self::Next => "next",
        }
    }
}

/// What a transport reports. It applies none of it.
#[derive(Debug, Clone, PartialEq)]
pub enum TransportEvent {
    PlayRequested,
    PauseRequested,
    /// Where the scrubber is while it is being dragged, in seconds.
    ///
    /// Reported on every move so a host can show a preview frame, and never
    /// as a seek, so a host does not seek once per pixel.
    SeekPreview(f32),
    /// Where the reader let go, or where a key asked to go, in seconds.
    SeekRequested(f32),
    VolumeRequested(f32),
    MuteToggled,
    SpeedRequested(f32),
    Stepped(TrackStep),
}

type EventHandler = Rc<dyn Fn(&TransportEvent, &mut Window, &mut App)>;

/// Set by the scrubber's own pointer handlers and read by the next build.
///
/// A `RenderOnce` builder is rebuilt every frame, so the press that starts a
/// scrub and the moves that continue it happen in different builds and need
/// somewhere outside the frame to agree. It is the arrangement `SplitPane`
/// uses for its divider, for the same reason.
#[derive(Debug, Default)]
struct Scrubbing {
    held: bool,
}

/// A transport for one piece of media.
#[derive(IntoElement)]
pub struct TransportBar {
    ident: Ident,
    label: Option<SharedString>,
    state: TransportState,
    position: f32,
    duration: TransportDuration,
    elapsed: Option<SharedString>,
    remaining: Option<SharedString>,
    buffered: Vec<BufferedRange>,
    volume: f32,
    muted: bool,
    speed: f32,
    speeds: Vec<f32>,
    step: f32,
    seekable: bool,
    volume_control: bool,
    has_previous: bool,
    has_next: bool,
    disabled: bool,
    on_event: Option<EventHandler>,
}

impl std::fmt::Debug for TransportBar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TransportBar")
            .field("ident", &self.ident)
            .field("state", &self.state)
            .field("position", &self.position)
            .field("duration", &self.duration)
            .field("buffered", &self.buffered.len())
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_event.is_some())
            .finish()
    }
}

impl TransportBar {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            label: None,
            state: TransportState::default(),
            position: 0.0,
            duration: TransportDuration::Unknown,
            elapsed: None,
            remaining: None,
            buffered: Vec::new(),
            volume: 1.0,
            muted: false,
            speed: 1.0,
            speeds: Vec::new(),
            step: DEFAULT_STEP,
            seekable: true,
            volume_control: true,
            has_previous: false,
            has_next: false,
            disabled: false,
            on_event: None,
        }
    }

    /// What is playing, shown beside the controls.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn state(mut self, state: TransportState) -> Self {
        self.state = state;
        self
    }

    /// Where the caller says the head is, in seconds.
    pub fn position(mut self, seconds: f32) -> Self {
        self.position = seconds.max(0.0);
        self
    }

    pub fn duration(mut self, seconds: f32) -> Self {
        self.duration = TransportDuration::Known(seconds.max(0.0));
        self
    }

    /// Says there is a position and no total, for a live stream.
    pub fn unknown_duration(mut self) -> Self {
        self.duration = TransportDuration::Unknown;
        self
    }

    /// How far in the media is, in the host's own words. This crate formats
    /// no durations.
    pub fn elapsed(mut self, elapsed: impl Into<SharedString>) -> Self {
        self.elapsed = Some(elapsed.into());
        self
    }

    /// How much is left, in the host's own words. Without one nothing is
    /// shown, because the remainder is a duration and this crate computes
    /// none.
    pub fn remaining(mut self, remaining: impl Into<SharedString>) -> Self {
        self.remaining = Some(remaining.into());
        self
    }

    /// The spans the host already holds. Supplying none draws no buffer.
    pub fn buffered(mut self, ranges: impl IntoIterator<Item = BufferedRange>) -> Self {
        self.buffered = ranges.into_iter().collect();
        self
    }

    /// Where the volume control sits, between zero and one.
    pub fn volume(mut self, volume: f32) -> Self {
        self.volume = volume.clamp(0.0, 1.0);
        self
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// The speeds the host offers, and which of them holds. Offering none
    /// leaves the control out entirely.
    pub fn speeds(mut self, speeds: impl IntoIterator<Item = f32>, current: f32) -> Self {
        self.speeds = speeds
            .into_iter()
            .filter(|speed| speed.is_finite() && *speed > 0.0)
            .collect();
        self.speed = current;
        self
    }

    /// How far one arrow key jumps, in seconds.
    ///
    /// A caller input, because whether an arrow should move a second or a
    /// minute is a judgement about the media, and this component knows
    /// nothing about the media.
    pub fn step_seconds(mut self, seconds: f32) -> Self {
        self.step = seconds.max(0.0);
        self
    }

    /// Whether the transport accepts seeks.
    ///
    /// A false capability keeps the position visible but installs no pointer,
    /// keyboard, or native slider action for moving it.
    pub fn seekable(mut self, seekable: bool) -> Self {
        self.seekable = seekable;
        self
    }

    /// Whether mute and volume controls are meaningful for this transport.
    /// Unsupported controls are absent rather than disabled forever.
    pub fn volume_control(mut self, available: bool) -> Self {
        self.volume_control = available;
        self
    }

    pub fn has_previous(mut self, has_previous: bool) -> Self {
        self.has_previous = has_previous;
        self
    }

    pub fn has_next(mut self, has_next: bool) -> Self {
        self.has_next = has_next;
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(&TransportEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }

    fn fraction(&self) -> Option<f32> {
        self.duration
            .seconds()
            .map(|total| (self.position / total).clamp(0.0, 1.0))
    }
}

impl Disableable for TransportBar {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// The share of a known duration one buffered range covers.
fn buffered_span(range: BufferedRange, total: f32) -> (f32, f32) {
    let start = (range.start / total).clamp(0.0, 1.0);
    let end = (range.end / total).clamp(0.0, 1.0);
    (start, (end - start).max(0.0))
}

/// How far into the media a pointer standing at `x` is asking for.
fn seek_at(x: f32, left: f32, width: f32, total: f32) -> f32 {
    if width <= 0.0 {
        return 0.0;
    }
    (((x - left) / width).clamp(0.0, 1.0) * total).clamp(0.0, total)
}

/// How far inside its own box a small glyph button's ink starts.
fn glyph_inset(theme: &gpui_kit_theme::Theme) -> f32 {
    let metrics = theme.control.get(ControlSize::Sm);
    (metrics.height - metrics.icon_size) / 2.0
}

/// An id-safe stem for a speed, so `1.5×` addresses as `speed-1-5`.
fn speed_id(speed: f32) -> String {
    format!("speed-{}", format!("{speed}").replace('.', "-"))
}

fn speed_precision(speed: f32) -> usize {
    speed
        .to_string()
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len())
}

impl RenderOnce for TransportBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();
        let actionable = !self.disabled && self.on_event.is_some();
        let handler = self.on_event.clone().filter(|_| actionable);
        let report = {
            let handler = handler.clone();
            Rc::new(
                move |event: TransportEvent, window: &mut Window, cx: &mut App| {
                    if let Some(handler) = &handler {
                        handler(&event, window, cx);
                    }
                },
            )
        };

        let total = self.duration.seconds();
        let fraction = self.fraction();
        let scrubbing = keyed::slot::<Scrubbing>(
            &ident.child("scrubber").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let held = scrubbing.borrow().held;
        // A head the pointer is holding has to be exactly under the pointer,
        // so the spring is skipped for a move this bar caused itself.
        let drawn = fraction.map(|fraction| {
            motion::tracked_or_snap(
                &ident.child("scrubber").semantic_id(),
                fraction,
                motion::tracking(&theme),
                held,
                window,
                cx,
            )
        });

        let measured = measure::cell(&ident.child("scrubber").semantic_id(), window, cx);
        let scrubbable = actionable && self.seekable && total.is_some();

        // The groove holds everything drawn along the bar, so the buffered
        // band and the played fill are clipped to its height and its ends
        // rather than each choosing a geometry of its own.
        let mut groove = div()
            .absolute()
            .left_0()
            .right_0()
            .h(px(TRACK_HEIGHT))
            .radius(&theme, Radius::Pill)
            .overflow_hidden()
            .bg(signature::track(&theme));

        // Buffered spans are the host's fact and are drawn as their own band,
        // never as part of the played fill: a reader who cannot tell them
        // apart has been told the media is further along than it is. A host
        // that supplied none gets no band and no node, so "nothing buffered"
        // is distinguishable from "buffered to the start".
        if let (Some(total), false) = (total, self.buffered.is_empty()) {
            let mut band = div().absolute().left_0().right_0().top_0().bottom_0();
            for range in &self.buffered {
                let (start, width) = buffered_span(*range, total);
                if width <= 0.0 {
                    continue;
                }
                band = band.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(start))
                        .w(relative(width))
                        .bg(theme.colors.hairline_strong),
                );
            }
            let furthest = self
                .buffered
                .iter()
                .map(|range| buffered_span(*range, total))
                .map(|(start, width)| start + width)
                .fold(0.0_f32, f32::max);
            // The node describes the buffered extent rather than the whole
            // track, so a test reads how far the buffer reaches from its
            // bounds as well as from its range.
            groove = groove.child(
                band.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left_0()
                        .w(relative(furthest.max(f32::EPSILON)))
                        .semantic_in(
                            cx,
                            NodeSpec::new(ident.child("buffered").semantic_id(), Role::Status)
                                .parent(ident.semantic_id())
                                .text(cx.strings().text(StringKey::TransportBuffered))
                                .range(0.0, 1.0, furthest)
                                .value(cx.strings().format_plural(
                                    StringKey::TransportRangeOne,
                                    StringKey::TransportRanges,
                                    cx.numbers().plural(self.buffered.len()),
                                    &[cx.numbers().count(self.buffered.len()).as_ref()],
                                )),
                        ),
                ),
            );
        }

        // Playing and paused are told apart by the control that changes them
        // and by the state line beside it, never by painting the fill in a
        // second colour: how far in the media is does not change when it
        // stops, so neither does the mark that says so.
        if let Some(drawn) = drawn {
            groove = groove.child(signature::determined(&theme, drawn));
        }

        let mut track = div()
            .id(ident.child("scrubber").element_id())
            .relative()
            .w_full()
            .h(px(KNOB))
            .flex()
            .items_center()
            .when(scrubbable, |element| element.cursor_pointer())
            .child(groove);

        // The head stands exactly over the end of the fill, which is why both
        // read the one tracked fraction: a head drawn from the reported
        // position while the fill was still travelling toward it would leave a
        // gap that says the media is somewhere it is not.
        if let Some(drawn) = drawn {
            track = track.child(
                div()
                    .absolute()
                    .left(relative(drawn))
                    .ml(px(-KNOB / 2.0))
                    .size(px(KNOB))
                    .radius(&theme, Radius::Pill)
                    .bg(theme.colors.text)
                    .border(px(theme.borders.hairline))
                    .border_color(theme.colors.hairline_strong),
            );
        }

        if let (true, Some(total)) = (scrubbable, total) {
            let press = Rc::clone(&report);
            let press_bounds = Rc::clone(&measured);
            let press_held = Rc::clone(&scrubbing);
            track = track.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                let bounds = press_bounds.get();
                press_held.borrow_mut().held = true;
                press(
                    TransportEvent::SeekPreview(seek_from(event.position, bounds, total)),
                    window,
                    cx,
                );
            });
        }

        let readout = |text: SharedString, tone: gpui::Hsla| {
            div()
                .flex_none()
                .type_scale(&theme, TypeScale::Caption)
                .text_color(tone)
                .child(text)
        };

        let elapsed = self
            .elapsed
            .clone()
            .unwrap_or_else(|| cx.strings().text(StringKey::TransportTimeUnknown));
        // With no total there is nothing to count down to, so the right-hand
        // readout states the fact rather than a number.
        let trailing = match (self.duration, self.remaining.clone()) {
            (TransportDuration::Unknown, _) => {
                Some(cx.strings().text(StringKey::TransportDurationUnknown))
            }
            (TransportDuration::Known(_), remaining) => remaining,
        };

        let scrubber_spec = match total {
            Some(total) => NodeSpec::new(ident.child("scrubber").semantic_id(), Role::Slider)
                .parent(ident.semantic_id())
                .text(cx.strings().text(StringKey::TransportPosition))
                .disabled(!scrubbable)
                .range(0.0, total, self.position.clamp(0.0, total))
                .value(elapsed.clone()),
            None => NodeSpec::new(ident.child("scrubber").semantic_id(), Role::Status)
                .parent(ident.semantic_id())
                .text(cx.strings().text(StringKey::TransportDurationUnknown))
                .value(elapsed.clone()),
        };

        let scrubber_row = div()
            .row()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .child(readout(elapsed.clone(), theme.colors.text_muted))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .on_children_prepainted({
                        let measured = Rc::clone(&measured);
                        move |bounds, window, _| {
                            if let Some(first) = bounds.first() {
                                measure::record(&measured, *first, window);
                            }
                        }
                    })
                    .child(track)
                    .semantic_in(cx, scrubber_spec),
            )
            .children(trailing.map(|text| readout(text, theme.colors.text_faint)));

        let playing = self.state.is_playing();
        let play = {
            let report = Rc::clone(&report);
            let mut control = Button::new(ident.child(if playing { "pause" } else { "play" }))
                .label(cx.strings().text(if playing {
                    StringKey::TransportPause
                } else {
                    StringKey::TransportPlay
                }))
                // The glyph carries the same word the label does, which is
                // what makes this the control the eye lands on in a row of
                // step buttons that are glyphs alone.
                .icon(if playing { Icon::Pause } else { Icon::Play })
                .secondary()
                .control_size(ControlSize::Sm)
                .semantic_parent(ident.semantic_id())
                .disabled(!actionable);
            if actionable {
                control = control.on_click(move |window, cx| {
                    let event = if playing {
                        TransportEvent::PauseRequested
                    } else {
                        TransportEvent::PlayRequested
                    };
                    report(event, window, cx);
                });
            }
            control
        };

        let strings = cx.strings().clone();
        let step_control =
            |name: &'static str, glyph: Icon, key: StringKey, step: TrackStep, enabled: bool| {
                let mut control = IconButton::new(ident.child(name), glyph, strings.text(key))
                    .ghost()
                    .control_size(ControlSize::Sm)
                    .semantic_parent(ident.semantic_id())
                    .disabled(!enabled || !actionable);
                if enabled && actionable {
                    let report = Rc::clone(&report);
                    control = control.on_click(move |window, cx| {
                        report(TransportEvent::Stepped(step), window, cx)
                    });
                }
                control
            };

        let status = match self.state {
            TransportState::Playing => (strings.text(StringKey::TransportPlaying), Tone::Success),
            TransportState::Paused => (strings.text(StringKey::TransportPaused), Tone::Neutral),
            TransportState::Buffering => {
                (strings.text(StringKey::TransportBuffering), Tone::Warning)
            }
        };

        let mute = self.volume_control.then(|| {
            let report = Rc::clone(&report);
            let mut control = Button::new(ident.child("mute"))
                .label(strings.text(if self.muted {
                    StringKey::TransportUnmute
                } else {
                    StringKey::TransportMute
                }))
                // Muted is said by the label, which is the control's own
                // wording for what it will do next. What the label cannot say
                // on its own is that it is a control at all: with no surface
                // under it, the one word on the bar that is not a glyph or a
                // chip read as a caption sitting between two controls. It
                // takes the same chip the transport's other worded control
                // takes, and stays quieter than it by carrying no glyph.
                .secondary()
                .control_size(ControlSize::Sm)
                .semantic_parent(ident.semantic_id())
                .disabled(!actionable);
            if actionable {
                control = control
                    .on_click(move |window, cx| report(TransportEvent::MuteToggled, window, cx));
            }
            control
        });

        let volume = self.volume_control.then(|| {
            let report = Rc::clone(&report);
            let mut control = Slider::new(ident.child("volume"))
                .label(strings.text(StringKey::TransportVolume))
                .range(0.0, 1.0)
                .value(self.volume)
                .control_size(ControlSize::Sm)
                .display(cx.numbers().percent(self.volume))
                .disabled(!actionable);
            if actionable {
                control = control.on_change(move |value, window, cx| {
                    report(TransportEvent::VolumeRequested(value), window, cx)
                });
            }
            div().flex_none().w(px(VOLUME_WIDTH)).child(control)
        });

        let speeds = (!self.speeds.is_empty()).then(|| {
            let offered = self.speeds.clone();
            let report = Rc::clone(&report);
            let mut control = SegmentedControl::new(ident.child("speed"))
                .control_size(ControlSize::Xs)
                .segments(offered.iter().map(|speed| {
                    Segment::new(
                        speed_id(*speed),
                        cx.numbers()
                            .multiplier(f64::from(*speed), speed_precision(*speed)),
                    )
                }))
                .selected(speed_id(self.speed))
                .disabled(!actionable);
            if actionable {
                control = control.on_select(move |id, window, cx| {
                    let Some(speed) = offered
                        .iter()
                        .find(|speed| speed_id(**speed) == id.as_ref())
                    else {
                        return;
                    };
                    report(TransportEvent::SpeedRequested(*speed), window, cx);
                });
            }
            control
        });

        // One skeleton for every transport: the controls that move the media
        // at the reading edge, what is playing in the middle, and the controls
        // that shape it at the trailing edge. A transport that offers fewer of
        // them leaves a slot out rather than rearranging the rest, and nothing
        // wraps, so a narrow bar keeps one row instead of inventing a second
        // layout.
        let controls = div()
            .row()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .child(
                div()
                    .row()
                    .flex_none()
                    .gap_token(&theme, Space::Xs)
                    // A glyph button is a square with the glyph centred in it,
                    // so its ink starts a control's own inset to the right of
                    // where the box does. The run is pulled back by exactly
                    // that inset, which puts the first mark of the transport
                    // in the same column as the readout under it and the
                    // titles above it rather than a control's padding inside
                    // them.
                    .ml(px(-glyph_inset(&theme)))
                    .child(step_control(
                        "previous",
                        Icon::AltArrowLeft,
                        StringKey::TransportPreviousTrack,
                        TrackStep::Previous,
                        self.has_previous,
                    ))
                    .child(play)
                    .child(step_control(
                        "next",
                        Icon::AltArrowRight,
                        StringKey::TransportNextTrack,
                        TrackStep::Next,
                        self.has_next,
                    )),
            )
            .child(
                div()
                    .row()
                    .flex_1()
                    .min_w_0()
                    .gap_token(&theme, Space::Sm)
                    .children(self.label.clone().map(|label| {
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .type_scale(&theme, TypeScale::Label)
                            .text_color(theme.colors.text)
                            .child(label)
                    }))
                    .child(
                        div()
                            .flex_none()
                            .child(StatusLine::new(status.0, status.1).id(ident.child("status"))),
                    ),
            )
            .child(
                div()
                    .row()
                    .flex_none()
                    .gap_token(&theme, Space::Sm)
                    .children(mute)
                    .children(volume)
                    .children(speeds),
            );

        let mut root = div()
            .id(ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .when(actionable, |element| {
                element.tab_index(0).focus_ring(&theme)
            })
            .child(controls)
            .child(scrubber_row);

        if actionable {
            // A scrub is followed across the whole bar rather than the few
            // pixels of the track, so letting go outside it still commits
            // once instead of leaving the scrub running.
            if let (true, Some(total)) = (self.seekable, total) {
                let drag = Rc::clone(&report);
                let drag_bounds = Rc::clone(&measured);
                let drag_held = Rc::clone(&scrubbing);
                root = root.on_mouse_move(move |event, window, cx| {
                    if !drag_held.borrow().held {
                        return;
                    }
                    if event.pressed_button != Some(MouseButton::Left) {
                        drag_held.borrow_mut().held = false;
                        return;
                    }
                    drag(
                        TransportEvent::SeekPreview(seek_from(
                            event.position,
                            drag_bounds.get(),
                            total,
                        )),
                        window,
                        cx,
                    );
                });

                let release = Rc::clone(&report);
                let release_bounds = Rc::clone(&measured);
                let release_held = Rc::clone(&scrubbing);
                root = root.on_mouse_up(MouseButton::Left, move |event, window, cx| {
                    if !std::mem::take(&mut release_held.borrow_mut().held) {
                        return;
                    }
                    release(
                        TransportEvent::SeekRequested(seek_from(
                            event.position,
                            release_bounds.get(),
                            total,
                        )),
                        window,
                        cx,
                    );
                });
            }

            let keys = Rc::clone(&report);
            let (position, step, state, seekable) =
                (self.position, self.step, self.state, self.seekable);
            root.interactivity().on_key_down(move |event, window, cx| {
                let event = match event.keystroke.key.as_str() {
                    "space" => {
                        if state.is_playing() {
                            TransportEvent::PauseRequested
                        } else {
                            TransportEvent::PlayRequested
                        }
                    }
                    // The forward end of a stream nobody measured is the
                    // host's to clamp, because only the host knows where the
                    // live edge is.
                    "left" if seekable => {
                        TransportEvent::SeekRequested(clamp_to(position - step, total))
                    }
                    "right" if seekable => {
                        TransportEvent::SeekRequested(clamp_to(position + step, total))
                    }
                    _ => return,
                };
                cx.stop_propagation();
                keys(event, window, cx);
            });
        }

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Group)
            .disabled(self.disabled)
            .busy(matches!(self.state, TransportState::Buffering))
            .value(self.state.name());
        if let Some(label) = self.label.clone() {
            spec = spec.text(label);
        }

        root.semantic_in(cx, spec)
    }
}

fn seek_from(at: Point<Pixels>, bounds: gpui::Bounds<Pixels>, total: f32) -> f32 {
    seek_at(
        f32::from(at.x),
        f32::from(bounds.left()),
        f32::from(bounds.size.width),
        total,
    )
}

fn clamp_to(seconds: f32, total: Option<f32>) -> f32 {
    match total {
        Some(total) => seconds.clamp(0.0, total),
        None => seconds.max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_live_stream_has_a_position_and_no_fraction() {
        let live = TransportBar::new("live").position(90.0).unknown_duration();
        assert!(!live.duration.is_known());
        assert_eq!(live.fraction(), None, "there is no total to be a part of");
    }

    #[test]
    fn a_zero_duration_is_no_more_a_total_than_an_unknown_one() {
        let empty = TransportBar::new("empty").duration(0.0).position(0.0);
        assert_eq!(empty.duration.seconds(), None);
        assert_eq!(empty.fraction(), None);
    }

    #[test]
    fn a_known_duration_places_the_head_and_stops_at_the_end() {
        let bar = TransportBar::new("clip").duration(200.0).position(50.0);
        assert_eq!(bar.fraction(), Some(0.25));
        let past = TransportBar::new("clip").duration(200.0).position(400.0);
        assert_eq!(past.fraction(), Some(1.0));
    }

    #[test]
    fn a_buffered_range_covers_its_own_share_and_no_more() {
        assert_eq!(
            buffered_span(BufferedRange::new(0.0, 50.0), 200.0),
            (0.0, 0.25)
        );
        assert_eq!(
            buffered_span(BufferedRange::new(100.0, 400.0), 200.0),
            (0.5, 0.5)
        );
        assert_eq!(
            BufferedRange::new(80.0, 20.0),
            BufferedRange::new(20.0, 80.0),
            "a reversed range is corrected rather than drawn backwards"
        );
    }

    #[test]
    fn a_pointer_asks_for_the_position_it_is_standing_over() {
        assert_eq!(seek_at(100.0, 100.0, 400.0, 200.0), 0.0);
        assert_eq!(seek_at(300.0, 100.0, 400.0, 200.0), 100.0);
        assert_eq!(seek_at(900.0, 100.0, 400.0, 200.0), 200.0);
    }

    #[test]
    fn an_unmeasured_track_asks_for_nothing() {
        assert_eq!(seek_at(300.0, 100.0, 0.0, 200.0), 0.0);
    }

    #[test]
    fn a_step_stops_at_a_known_end_and_only_at_zero_without_one() {
        assert_eq!(clamp_to(-5.0, Some(200.0)), 0.0);
        assert_eq!(clamp_to(400.0, Some(200.0)), 200.0);
        assert_eq!(clamp_to(-5.0, None), 0.0);
        assert_eq!(clamp_to(400.0, None), 400.0);
    }

    #[test]
    fn buffering_is_playing_rather_than_paused() {
        assert!(TransportState::Buffering.is_playing());
        assert!(TransportState::Playing.is_playing());
        assert!(!TransportState::Paused.is_playing());
        assert_eq!(TransportState::Buffering.name(), "buffering");
    }

    #[test]
    fn a_speed_addresses_by_its_value_rather_than_its_place() {
        assert_eq!(speed_id(1.5), "speed-1-5");
        assert_eq!(speed_id(2.0), "speed-2");
        assert_eq!(speed_precision(1.5), 1);
        assert_eq!(speed_precision(2.0), 0);
    }
}

#[cfg(test)]
mod transport_phase_tests {
    use super::*;

    #[test]
    fn playing_and_paused_are_ready_buffering_is_refreshing() {
        assert_eq!(TransportState::Playing.phase(), Phase::Ready);
        assert_eq!(TransportState::Paused.phase(), Phase::Ready);
        assert_eq!(TransportState::Buffering.phase(), Phase::Refreshing);
        assert_ne!(
            TransportState::Playing.name(),
            TransportState::Paused.name()
        );
    }
}
