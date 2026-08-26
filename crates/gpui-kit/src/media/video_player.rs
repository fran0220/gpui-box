//! A player for one piece of video: a frame surface, and the same transport.
//!
//! # What this component will not do
//!
//! **It does not decode a frame.** The picture is an element the host hands
//! back per frame through [`VideoPlayer::frame`] — a `gpui::surface`, a
//! `gpui::img`, [`gpui::platform_view`], or anything else it can build.
//! [`super::PlatformMediaTransport::frame`] supplies the native macOS/Windows
//! view; a host that hands back nothing has said so.
//!
//! **It does not use a poster as a frame.** A poster is one still, and a still
//! standing in for playback is the most convincing possible lie about a player
//! that is not playing. When the surface is showing a poster it publishes
//! `poster`, and the reason no frames are arriving is drawn over it.
//!
//! **It never draws a progress bar over a picture that is not moving.** The
//! bar is the transport's snapshot, and a surface with no transport gets no
//! bar at all.
//!
//! **It states its aspect ratio rather than distorting the picture.** The
//! frame is an [`AspectRatio`], so a 16∶9 stream
//! in a 4∶3 slot is letterboxed by the layout instead of stretched by it.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, TypeScale};

use crate::content::transport::{TransportBar, TransportDuration};
use crate::display::badge::Badge;
use crate::foundation::{Disableable, Ident, StyledExt, text};
use crate::layout::{AspectFit, AspectRatio};
use crate::media::audio_player::{command_for, unready};
use crate::media::transport::{
    MediaAvailability, MediaCapabilities, MediaEvent, MediaSnapshot, MediaTransport,
};
use crate::media::{NoticePlace, notice_at};
use crate::strings::{ActiveStrings, StringKey};

/// The shape a video slot takes when the caller states none.
const DEFAULT_RATIO: f32 = 16.0 / 9.0;

type EventHandler = Rc<dyn Fn(&MediaEvent, &mut Window, &mut App)>;
type FrameSupplier = Rc<dyn Fn(&mut Window, &mut App) -> Option<AnyElement>>;

/// What is in the frame, as the surface publishes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceContent {
    /// A frame the host supplied for this moment.
    Frame,
    /// A still, with the reason there is no moving picture over it.
    Poster,
    /// Neither, and the reason.
    Nothing,
}

impl SurfaceContent {
    fn name(self) -> &'static str {
        match self {
            Self::Frame => "frame",
            Self::Poster => "poster",
            Self::Nothing => "none",
        }
    }
}

/// A player for one piece of video.
#[derive(IntoElement)]
pub struct VideoPlayer {
    ident: Ident,
    title: Option<SharedString>,
    transport: Option<Rc<dyn MediaTransport>>,
    frame: Option<FrameSupplier>,
    poster: Option<FrameSupplier>,
    ratio: f32,
    elapsed: Option<SharedString>,
    remaining: Option<SharedString>,
    step: Option<f32>,
    speeds: Vec<f32>,
    disabled: bool,
    on_event: Option<EventHandler>,
}

impl std::fmt::Debug for VideoPlayer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VideoPlayer")
            .field("ident", &self.ident)
            .field("title", &self.title)
            .field("transport", &self.transport)
            .field("ratio", &self.ratio)
            .field("has_frames", &self.frame.is_some())
            .field("has_poster", &self.poster.is_some())
            .field("disabled", &self.disabled)
            .field("has_handler", &self.on_event.is_some())
            .finish()
    }
}

impl VideoPlayer {
    /// A player with no transport and no frames, which is a player that says
    /// there is no playback here.
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            title: None,
            transport: None,
            frame: None,
            poster: None,
            ratio: DEFAULT_RATIO,
            elapsed: None,
            remaining: None,
            step: None,
            speeds: Vec::new(),
            disabled: false,
            on_event: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The player behind the controls.
    pub fn transport(mut self, transport: Rc<dyn MediaTransport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Supplies the element that draws the current frame.
    ///
    /// Answering `None` is a statement, not a gap: the surface falls back to
    /// the poster and says why the picture is not moving.
    pub fn frame(
        mut self,
        supplier: impl Fn(&mut Window, &mut App) -> Option<AnyElement> + 'static,
    ) -> Self {
        self.frame = Some(Rc::new(supplier));
        self
    }

    /// Supplies the still shown when there are no frames.
    pub fn poster(
        mut self,
        supplier: impl Fn(&mut Window, &mut App) -> Option<AnyElement> + 'static,
    ) -> Self {
        self.poster = Some(Rc::new(supplier));
        self
    }

    /// The picture's own shape, width divided by height.
    pub fn ratio(mut self, ratio: f32) -> Self {
        if ratio > 0.0 {
            self.ratio = ratio;
        }
        self
    }

    /// The picture's shape as the two numbers the caller already has.
    pub fn ratio_of(self, width: f32, height: f32) -> Self {
        self.ratio(if height == 0.0 {
            DEFAULT_RATIO
        } else {
            width / height
        })
    }

    /// How far in the video is, in the host's own words.
    pub fn elapsed(mut self, elapsed: impl Into<SharedString>) -> Self {
        self.elapsed = Some(elapsed.into());
        self
    }

    /// How much is left, in the host's own words.
    pub fn remaining(mut self, remaining: impl Into<SharedString>) -> Self {
        self.remaining = Some(remaining.into());
        self
    }

    /// How far one arrow key jumps, in seconds.
    pub fn step_seconds(mut self, seconds: f32) -> Self {
        self.step = Some(seconds.max(0.0));
        self
    }

    /// The speeds the transport offers. Offering none leaves the control out.
    pub fn speeds(mut self, speeds: impl IntoIterator<Item = f32>) -> Self {
        self.speeds = speeds.into_iter().filter(|speed| *speed > 0.0).collect();
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(&MediaEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }
}

impl Disableable for VideoPlayer {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for VideoPlayer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();
        let strings = cx.strings().clone();

        let snapshot = self
            .transport
            .as_ref()
            .map(|transport| transport.snapshot());
        let capabilities = self
            .transport
            .as_ref()
            .map(|transport| transport.capabilities())
            .unwrap_or_else(MediaCapabilities::none);
        let origin = self.transport.as_ref().map(|transport| transport.origin());
        let availability = snapshot.as_ref().map(|snapshot| &snapshot.availability);
        let state = match availability {
            Some(availability) => availability.name(),
            None => "no-transport",
        };

        // Frames are only asked for while the transport says it is holding
        // media. Asking a host for a picture of something it has told us it
        // cannot open would invite it to answer with one.
        let supplied = availability
            .filter(|availability| availability.is_ready())
            .and(self.frame.as_ref())
            .and_then(|supplier| supplier(window, cx));
        let poster = self
            .poster
            .as_ref()
            .and_then(|supplier| supplier(window, cx));

        let content = match (&supplied, &poster) {
            (Some(_), _) => SurfaceContent::Frame,
            (None, Some(_)) => SurfaceContent::Poster,
            (None, None) => SurfaceContent::Nothing,
        };

        let mut picture = div()
            .relative()
            .size_full()
            .overflow_hidden()
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Sunken, Elevation::Flat);
        if let Some(element) = supplied {
            picture = picture.child(div().absolute().inset_0().child(element));
        } else if let Some(element) = poster {
            picture = picture.child(div().absolute().inset_0().child(element));
        }

        // Whatever is behind it, the surface says what it is showing whenever
        // there is no live frame in it. A caller-supplied poster is a valid
        // fallback, not an error; an entirely empty surface remains a warning.
        if !matches!(content, SurfaceContent::Frame) {
            let place = match content {
                SurfaceContent::Poster => NoticePlace::Foot,
                _ => NoticePlace::Middle,
            };
            // A still already says what kind of surface this is, so the mark
            // is only drawn over an empty one.
            let mark =
                matches!(content, SurfaceContent::Nothing).then_some(gpui_kit_assets::Icon::Video);
            picture = picture.child(match (&snapshot, &self.transport) {
                (Some(snapshot), _)
                    if snapshot.availability.is_ready()
                        && matches!(content, SurfaceContent::Poster) =>
                {
                    notice_at(
                        &theme,
                        theme.colors.text_muted,
                        None,
                        strings.text(StringKey::VideoPoster),
                        strings.text(StringKey::VideoPosterDetail),
                        place,
                    )
                }
                (Some(snapshot), _) if snapshot.availability.is_ready() => notice_at(
                    &theme,
                    theme.colors.warning,
                    mark,
                    strings.text(StringKey::VideoNoFrames),
                    strings.text(StringKey::VideoNoFramesDetail),
                    place,
                ),
                (Some(snapshot), _) => unready(
                    &theme,
                    &strings,
                    &snapshot.availability,
                    self.title.clone(),
                    mark,
                    place,
                ),
                (None, _) => notice_at(
                    &theme,
                    theme.colors.warning,
                    mark,
                    strings.text(StringKey::MediaNoTransport),
                    strings.text(StringKey::MediaNoTransportDetail),
                    place,
                ),
            });
        }

        let surface = AspectRatio::new(ident.child("aspect"), self.ratio)
            .fit(AspectFit::Width)
            .child(
                picture.semantic_in(
                    cx,
                    NodeSpec::new(ident.child("surface").semantic_id(), Role::Image)
                        .parent(ident.semantic_id())
                        .busy(matches!(availability, Some(MediaAvailability::Loading)))
                        .invalid(matches!(availability, Some(MediaAvailability::Failed(_))))
                        .value(content.name()),
                ),
            );

        let titles = div()
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
            .children(
                origin
                    .filter(|origin| origin.is_fixture())
                    .map(|_| Badge::new(strings.text(StringKey::MediaFixture)).warning()),
            );
        let titles = match origin {
            Some(origin) => titles
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child("origin").semantic_id(), Role::Status)
                        .parent(ident.semantic_id())
                        .value(origin.name()),
                )
                .into_any_element(),
            None => titles.into_any_element(),
        };

        let actionable = !self.disabled
            && capabilities.video
            && snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.availability.is_ready());

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Group)
            .disabled(self.disabled)
            .busy(matches!(availability, Some(MediaAvailability::Loading)))
            .invalid(matches!(availability, Some(MediaAvailability::Failed(_))))
            .value(state);
        if let Some(title) = self.title.clone() {
            spec = spec.text(title);
        }

        let controls = match (&snapshot, &self.transport) {
            (Some(snapshot), Some(transport)) => Some(bar(
                &ident,
                &self,
                snapshot,
                capabilities,
                Rc::clone(transport),
                actionable,
            )),
            // No transport is no controls. A disabled bar would still say
            // playback exists here, and it does not.
            _ => None,
        };

        div()
            .id(ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .child(titles)
            .child(surface)
            .children(controls)
            .semantic_in(cx, spec)
    }
}

/// The transport bar, wired to ask the transport and report the answer.
fn bar(
    ident: &Ident,
    player: &VideoPlayer,
    snapshot: &MediaSnapshot,
    capabilities: MediaCapabilities,
    transport: Rc<dyn MediaTransport>,
    actionable: bool,
) -> TransportBar {
    let mut bar = TransportBar::new(ident.child("transport"))
        .state(snapshot.state)
        .position(snapshot.position)
        .volume(snapshot.volume)
        .muted(snapshot.muted)
        .buffered(snapshot.buffered.iter().copied())
        .seekable(capabilities.seek)
        .volume_control(capabilities.volume)
        .disabled(!actionable);
    bar = match snapshot.duration {
        TransportDuration::Known(seconds) => bar.duration(seconds),
        TransportDuration::Unknown => bar.unknown_duration(),
    };
    if let Some(elapsed) = player.elapsed.clone() {
        bar = bar.elapsed(elapsed);
    }
    if let Some(remaining) = player.remaining.clone() {
        bar = bar.remaining(remaining);
    }
    if let Some(step) = player.step {
        bar = bar.step_seconds(step);
    }
    if capabilities.rates && !player.speeds.is_empty() {
        bar = bar.speeds(player.speeds.iter().copied(), snapshot.speed);
    }
    if actionable {
        let handler = player.on_event.clone();
        bar = bar.on_event(move |event, window, cx| {
            let Some(command) = command_for(event) else {
                return;
            };
            let outcome = transport.apply(command);
            if let Some(handler) = &handler {
                handler(&MediaEvent::of(command, outcome), window, cx);
            }
            window.refresh();
        });
    }
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poster_is_published_as_a_poster_and_never_as_a_frame() {
        assert_eq!(SurfaceContent::Frame.name(), "frame");
        assert_eq!(SurfaceContent::Poster.name(), "poster");
        assert_eq!(SurfaceContent::Nothing.name(), "none");
    }

    #[test]
    fn a_shape_nobody_stated_is_the_one_a_video_slot_usually_is() {
        let player = VideoPlayer::new("clip");
        assert!((player.ratio - DEFAULT_RATIO).abs() < f32::EPSILON);
        assert!((VideoPlayer::new("clip").ratio_of(4.0, 3.0).ratio - 4.0 / 3.0).abs() < 0.001);
        assert!(
            (VideoPlayer::new("clip").ratio(-1.0).ratio - DEFAULT_RATIO).abs() < f32::EPSILON,
            "a ratio that is not a ratio is refused rather than divided by"
        );
    }
}
