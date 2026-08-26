//! A motion written as a description rather than as a closure.
//!
//! [`MotionSpec`] says how long a motion takes and what curve it travels on,
//! and every layer above it — [`Transition`](super::Transition),
//! [`Presence`](super::Presence), GPUI's `with_animation` — takes the progress
//! it reports and hands it to a closure that decides what moves. The closure
//! is where the description stops: the fact that a panel fades in while rising
//! twelve pixels lives inside a lambda, so nothing can sample it, diff it or
//! scrub it.
//!
//! [`Motion`] is that closure written down. It carries the same duration,
//! delay and curve — it resolves to a [`MotionSpec`], it is not a second
//! model of time — and adds which properties travel and between which values,
//! so a caller can ask what the motion looks like at any point in its run
//! without a window, a frame or an element.
//!
//! The [`motion!`](crate::motion) macro writes one:
//!
//! ```
//! use gpui_kit::theme::Theme;
//!
//! let arrive = gpui_kit::motion! {
//!     duration: 420;
//!     ease: overshoot;
//!     opacity: 0.0 => 1.0;
//!     y: 12.0 => 0.0;
//! };
//!
//! let theme = Theme::studio_dark();
//! assert_eq!(arrive.sample(&theme, 0.0).y, 12.0);
//! assert_eq!(arrive.sample(&theme, 1.0).y, 0.0);
//! ```

use gpui::{IntoElement, Styled, px};
use gpui_kit_theme::Theme;

use super::easing::Easing;
use super::interpolate::Interpolate;
use super::spec::MotionSpec;

/// A property a described motion can move.
///
/// Deliberately short: these are the four values a caller can apply to any
/// element without publishing a different box than the settled one. A
/// component that animates its own colour, width or corner radius is animating
/// something only it knows the meaning of, and reaches for
/// [`Transition`](super::Transition) with the value in hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionProperty {
    Opacity,
    /// Horizontal paint offset, in pixels, relative to the settled position.
    X,
    /// Vertical paint offset, in pixels, relative to the settled position.
    Y,
    /// A multiplier around the element's own size, where 1 is settled.
    Scale,
}

impl MotionProperty {
    pub fn name(self) -> &'static str {
        match self {
            Self::Opacity => "opacity",
            Self::X => "x",
            Self::Y => "y",
            Self::Scale => "scale",
        }
    }

    /// The value the property has when nothing is animating it, which is also
    /// what a sample falls back to when the arithmetic that produced it did
    /// not yield a number.
    pub fn neutral(self) -> f32 {
        match self {
            Self::Opacity | Self::Scale => 1.0,
            Self::X | Self::Y => 0.0,
        }
    }
}

/// One property and the stops it passes through, as fractions of the run.
#[derive(Debug, Clone, PartialEq)]
struct Track {
    property: MotionProperty,
    stops: Vec<(f32, f32)>,
}

impl Track {
    /// The value at eased progress `eased`.
    ///
    /// Outside the stops the nearest segment is extrapolated rather than
    /// clamped, which is what keeps an overshooting curve overshooting: a
    /// curve that reports 1.06 is asking to be 6% past the last value, and a
    /// clamp here would quietly delete the only thing that curve was chosen
    /// for.
    fn sample(&self, eased: f32) -> f32 {
        let value = match self.stops.len() {
            0 => self.property.neutral(),
            1 => self.stops[0].1,
            _ => {
                let last = self.stops.len() - 2;
                let reached = self
                    .stops
                    .iter()
                    .rposition(|(offset, _)| *offset <= eased)
                    .unwrap_or(0)
                    .min(last);
                let (from_offset, from) = self.stops[reached];
                let (to_offset, to) = self.stops[reached + 1];
                let span = to_offset - from_offset;
                if span <= 0.0 {
                    to
                } else {
                    from.lerp(to, (eased - from_offset) / span)
                }
            }
        };
        finite(value, self.property)
    }
}

/// A number that has escaped the range a layout can be told about is not
/// worth propagating: a NaN offset reaches GPUI as a NaN `Pixels`, and from
/// there it is a laid-out box nobody can see and nobody can explain.
fn finite(value: f32, property: MotionProperty) -> f32 {
    if value.is_finite() {
        value
    } else {
        property.neutral()
    }
}

/// Where a described motion has got to: one value per property, ready to be
/// applied to an element.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSample {
    /// Already inside 0..=1, because an overshooting curve overshoots opacity
    /// too and GPUI has nothing to do with an element that is 106% opaque.
    pub opacity: f32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

impl Default for MotionSample {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            x: 0.0,
            y: 0.0,
            scale: 1.0,
        }
    }
}

impl MotionSample {
    pub fn get(self, property: MotionProperty) -> f32 {
        match property {
            MotionProperty::Opacity => self.opacity,
            MotionProperty::X => self.x,
            MotionProperty::Y => self.y,
            MotionProperty::Scale => self.scale,
        }
    }

    /// Applies the sample to an element as paint, never as layout input.
    ///
    /// The offsets are relative, so the element occupies its settled box for
    /// the whole run and nothing beside it moves — the same rule
    /// [`Animated::animate_in`](super::Animated::animate_in) follows.
    ///
    /// [`MotionSample::scale`] is not applied here, because GPUI carries a
    /// subtree scale on `Svg` alone; a caller that has a scalable element
    /// reads the field and applies it itself.
    pub fn apply<E: Styled + IntoElement>(self, element: E) -> E {
        let element = element.opacity(self.opacity);
        if self.x == 0.0 && self.y == 0.0 {
            element
        } else {
            element.relative().left(px(self.x)).top(px(self.y))
        }
    }
}

/// A motion written as what moves rather than as a closure that moves it.
///
/// A description resolves to a [`MotionSpec`] — [`Motion::spec`] — so anything
/// that already runs a specification runs a description without knowing it is
/// one. What the description adds is [`Motion::sample`]: the values the
/// properties hold at a point in the run, which is what makes a motion
/// testable without a window and scrubbable by an [`Animator`](super::Animator).
///
/// The curve is kept as an [`Easing`] rather than as resolved control points,
/// because the point of naming a curve is that the token document decides what
/// it is. That is why sampling takes a theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Motion {
    duration_ms: u64,
    delay_ms: u64,
    easing: Easing,
    alternate: bool,
    tracks: Vec<Track>,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            duration_ms: 0,
            delay_ms: 0,
            easing: Easing::Standard,
            alternate: false,
            tracks: Vec::new(),
        }
    }
}

impl Motion {
    pub fn new(duration_ms: u64) -> Self {
        Self {
            duration_ms,
            ..Self::default()
        }
    }

    pub fn duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    pub fn delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    pub fn eased(mut self, easing: impl Into<Easing>) -> Self {
        self.easing = easing.into();
        self
    }

    /// Plays the run forwards over its first half and backwards over its
    /// second, so the description returns to where it started.
    ///
    /// The duration is still the whole run: an alternating motion of 400ms
    /// arrives at its far end after 200ms and is back where it began at 400.
    pub fn alternating(mut self) -> Self {
        self.alternate = true;
        self
    }

    /// A property that travels straight from `from` to `to`.
    pub fn tween(self, property: MotionProperty, from: f32, to: f32) -> Self {
        self.track(property, [(0.0, from), (1.0, to)])
    }

    /// A property that passes through stops given as `(offset, value)`, where
    /// the offset is a fraction of the run.
    ///
    /// Stops may be given in any order and a property given twice keeps the
    /// last description of it, so a caller building one up in a loop does not
    /// have to check what it already said.
    pub fn track(
        mut self,
        property: MotionProperty,
        stops: impl IntoIterator<Item = (f32, f32)>,
    ) -> Self {
        let mut stops: Vec<(f32, f32)> = stops
            .into_iter()
            .map(|(offset, value)| (offset.clamp(0.0, 1.0), value))
            .collect();
        stops.sort_by(|a, b| a.0.total_cmp(&b.0));
        self.tracks.retain(|track| track.property != property);
        if !stops.is_empty() {
            self.tracks.push(Track { property, stops });
        }
        self
    }

    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    pub fn delay_ms(&self) -> u64 {
        self.delay_ms
    }

    pub fn easing(&self) -> Easing {
        self.easing
    }

    pub fn is_alternating(&self) -> bool {
        self.alternate
    }

    /// The properties this motion describes, in the order they were given.
    pub fn properties(&self) -> Vec<MotionProperty> {
        self.tracks.iter().map(|track| track.property).collect()
    }

    /// The description as the specification everything else already runs.
    pub fn spec(&self, theme: &Theme) -> MotionSpec {
        MotionSpec::new(self.duration_ms, self.easing.curve(theme)).with_delay(self.delay_ms)
    }

    /// The eased progress at `raw`, which is `raw` through the whole run
    /// including the delay — the same clock [`MotionSpec::progress`] reads.
    pub fn progress(&self, theme: &Theme, raw: f32) -> f32 {
        let raw = if raw.is_finite() {
            raw.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let raw = if self.alternate {
            if raw <= 0.5 {
                raw * 2.0
            } else {
                (1.0 - raw) * 2.0
            }
        } else {
            raw
        };
        self.spec(theme).progress(raw)
    }

    /// What every property holds when the run is `raw` through.
    ///
    /// Pure: no window, no clock, no element. Properties the motion never
    /// mentions report their neutral value, so a sample is always complete
    /// and a caller never has to ask which of the four were described.
    pub fn sample(&self, theme: &Theme, raw: f32) -> MotionSample {
        let eased = self.progress(theme, raw);
        let mut sample = MotionSample::default();
        for track in &self.tracks {
            let value = track.sample(eased);
            match track.property {
                MotionProperty::Opacity => sample.opacity = value.clamp(0.0, 1.0),
                MotionProperty::X => sample.x = value,
                MotionProperty::Y => sample.y = value,
                MotionProperty::Scale => sample.scale = value,
            }
        }
        sample
    }
}

/// Writes a [`Motion`]: how long it takes, what curve it travels on, and what
/// moves.
///
/// ```
/// let arrive = gpui_kit::motion! {
///     duration: 420;
///     delay: 40;
///     ease: overshoot;
///     opacity: 0.0 => 1.0;
///     y: 12.0 => 0.0;
/// };
/// assert_eq!(arrive.duration_ms(), 420);
/// ```
///
/// The entries, in any order and each ended by a semicolon:
///
/// - `duration: <ms>;` and `delay: <ms>;`
/// - `ease: <name>;` where the name is one of the [`Easing`] roles in
///   snake case — `linear`, `standard`, `ease_in`, `ease_out`, `ease_in_out`,
///   `emphasized`, `overshoot`, `exit`, `settle` — or any expression that
///   converts into an [`Easing`], which is how a [`CubicBezier`](super::CubicBezier)
///   is written;
/// - `alternate;`, which plays the run out and back;
/// - `opacity`, `x`, `y` or `scale`, each either `<from> => <to>;` or a stop
///   list `[<offset> => <value>, ...];`.
///
/// Values are `f32` expressions, so `0.0` rather than `0`.
///
/// ```
/// let pulse = gpui_kit::motion! {
///     duration: 600;
///     ease: linear;
///     opacity: [0.0 => 0.4, 0.5 => 1.0, 1.0 => 0.4];
/// };
/// let theme = gpui_kit::theme::Theme::studio_dark();
/// assert!(pulse.sample(&theme, 0.5).opacity > pulse.sample(&theme, 0.0).opacity);
/// ```
#[macro_export]
macro_rules! motion {
    (@field $described:ident;) => {};
    (@field $described:ident; duration: $ms:expr; $($rest:tt)*) => {
        $described = $described.duration($ms);
        $crate::motion!(@field $described; $($rest)*);
    };
    (@field $described:ident; delay: $ms:expr; $($rest:tt)*) => {
        $described = $described.delay($ms);
        $crate::motion!(@field $described; $($rest)*);
    };
    (@field $described:ident; alternate; $($rest:tt)*) => {
        $described = $described.alternating();
        $crate::motion!(@field $described; $($rest)*);
    };
    (@field $described:ident; ease: linear; $($rest:tt)*) => {
        $crate::motion!(@ease $described; $crate::motion::Easing::Linear; $($rest)*);
    };
    (@field $described:ident; ease: standard; $($rest:tt)*) => {
        $crate::motion!(@ease $described; $crate::motion::Easing::Standard; $($rest)*);
    };
    (@field $described:ident; ease: ease_in; $($rest:tt)*) => {
        $crate::motion!(@ease $described; $crate::motion::Easing::EaseIn; $($rest)*);
    };
    (@field $described:ident; ease: ease_out; $($rest:tt)*) => {
        $crate::motion!(@ease $described; $crate::motion::Easing::EaseOut; $($rest)*);
    };
    (@field $described:ident; ease: ease_in_out; $($rest:tt)*) => {
        $crate::motion!(@ease $described; $crate::motion::Easing::EaseInOut; $($rest)*);
    };
    (@field $described:ident; ease: emphasized; $($rest:tt)*) => {
        $crate::motion!(@ease $described; $crate::motion::Easing::Emphasized; $($rest)*);
    };
    (@field $described:ident; ease: overshoot; $($rest:tt)*) => {
        $crate::motion!(@ease $described; $crate::motion::Easing::Overshoot; $($rest)*);
    };
    (@field $described:ident; ease: exit; $($rest:tt)*) => {
        $crate::motion!(@ease $described; $crate::motion::Easing::Exit; $($rest)*);
    };
    (@field $described:ident; ease: settle; $($rest:tt)*) => {
        $crate::motion!(@ease $described; $crate::motion::Easing::Settle; $($rest)*);
    };
    (@field $described:ident; ease: $easing:expr; $($rest:tt)*) => {
        $crate::motion!(@ease $described; $easing; $($rest)*);
    };
    (@ease $described:ident; $easing:expr; $($rest:tt)*) => {
        $described = $described.eased($easing);
        $crate::motion!(@field $described; $($rest)*);
    };
    (@field $described:ident; opacity: [$($offset:expr => $value:expr),+ $(,)?]; $($rest:tt)*) => {
        $crate::motion!(@stops $described; $crate::motion::MotionProperty::Opacity; [$($offset => $value),+]; $($rest)*);
    };
    (@field $described:ident; opacity: $from:expr => $to:expr; $($rest:tt)*) => {
        $crate::motion!(@tween $described; $crate::motion::MotionProperty::Opacity; $from => $to; $($rest)*);
    };
    (@field $described:ident; x: [$($offset:expr => $value:expr),+ $(,)?]; $($rest:tt)*) => {
        $crate::motion!(@stops $described; $crate::motion::MotionProperty::X; [$($offset => $value),+]; $($rest)*);
    };
    (@field $described:ident; x: $from:expr => $to:expr; $($rest:tt)*) => {
        $crate::motion!(@tween $described; $crate::motion::MotionProperty::X; $from => $to; $($rest)*);
    };
    (@field $described:ident; y: [$($offset:expr => $value:expr),+ $(,)?]; $($rest:tt)*) => {
        $crate::motion!(@stops $described; $crate::motion::MotionProperty::Y; [$($offset => $value),+]; $($rest)*);
    };
    (@field $described:ident; y: $from:expr => $to:expr; $($rest:tt)*) => {
        $crate::motion!(@tween $described; $crate::motion::MotionProperty::Y; $from => $to; $($rest)*);
    };
    (@field $described:ident; scale: [$($offset:expr => $value:expr),+ $(,)?]; $($rest:tt)*) => {
        $crate::motion!(@stops $described; $crate::motion::MotionProperty::Scale; [$($offset => $value),+]; $($rest)*);
    };
    (@field $described:ident; scale: $from:expr => $to:expr; $($rest:tt)*) => {
        $crate::motion!(@tween $described; $crate::motion::MotionProperty::Scale; $from => $to; $($rest)*);
    };
    (@stops $described:ident; $property:expr; [$($offset:expr => $value:expr),+]; $($rest:tt)*) => {
        $described = $described.track($property, [$(($offset, $value)),+]);
        $crate::motion!(@field $described; $($rest)*);
    };
    (@tween $described:ident; $property:expr; $from:expr => $to:expr; $($rest:tt)*) => {
        $described = $described.tween($property, $from, $to);
        $crate::motion!(@field $described; $($rest)*);
    };
    (@field $described:ident; $($rest:tt)*) => {
        compile_error!(concat!(
            "motion! does not know this entry: ",
            stringify!($($rest)*)
        ));
    };
    ($($described:tt)*) => {{
        #[allow(unused_mut)]
        let mut described = $crate::motion::Motion::default();
        $crate::motion!(@field described; $($described)*);
        described
    }};
}

/// Writes a [`Sequence`](super::Sequence): specifications that run one after
/// another, with an optional gap before any of them.
///
/// ```
/// use gpui_kit::theme::Theme;
///
/// let theme = Theme::studio_dark();
/// let panel = gpui_kit::motion::menu(&theme);
/// let content = gpui_kit::motion::entrance(&theme);
///
/// let opening = gpui_kit::sequence![panel, +80 content];
/// assert_eq!(opening.len(), 2);
/// assert_eq!(
///     opening.total(),
///     panel.total() + content.total() + std::time::Duration::from_millis(80),
/// );
/// ```
///
/// An entry is a [`MotionSpec`] expression. `+<ms>` before one delays it that
/// much further past the end of the step before it, which is the same thing as
/// writing the delay on the specification — it is written here because that is
/// where the gap is.
#[macro_export]
macro_rules! sequence {
    (@step $sequenced:ident;) => {};
    (@step $sequenced:ident; + $delay:literal $spec:expr, $($rest:tt)*) => {
        $crate::sequence!(@push $sequenced; $delay; $spec);
        $crate::sequence!(@step $sequenced; $($rest)*);
    };
    (@step $sequenced:ident; + $delay:literal $spec:expr) => {
        $crate::sequence!(@push $sequenced; $delay; $spec);
    };
    (@step $sequenced:ident; $spec:expr, $($rest:tt)*) => {
        $crate::sequence!(@push $sequenced; 0; $spec);
        $crate::sequence!(@step $sequenced; $($rest)*);
    };
    (@step $sequenced:ident; $spec:expr) => {
        $crate::sequence!(@push $sequenced; 0; $spec);
    };
    (@push $sequenced:ident; $delay:literal; $spec:expr) => {
        let step: $crate::motion::MotionSpec = $spec;
        $sequenced = $sequenced.then(step.with_delay(step.delay_ms + $delay));
    };
    ($($sequenced:tt)*) => {{
        #[allow(unused_mut)]
        let mut sequenced = $crate::motion::Sequence::default();
        $crate::sequence!(@step sequenced; $($sequenced)*);
        sequenced
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{CubicBezier, Sequence};
    use std::time::Duration;

    fn theme() -> Theme {
        Theme::studio_dark()
    }

    #[test]
    fn a_described_motion_holds_the_run_it_was_written_with() {
        let described = motion! {
            duration: 420;
            delay: 40;
            ease: overshoot;
            opacity: 0.0 => 1.0;
            y: 12.0 => 0.0;
        };
        assert_eq!(described.duration_ms(), 420);
        assert_eq!(described.delay_ms(), 40);
        assert_eq!(described.easing(), Easing::Overshoot);
        assert_eq!(
            described.properties(),
            vec![MotionProperty::Opacity, MotionProperty::Y]
        );
        assert_eq!(described.spec(&theme()).total(), Duration::from_millis(460));
    }

    #[test]
    fn a_tween_reaches_both_of_the_values_it_was_given() {
        let described = motion! {
            duration: 200;
            ease: linear;
            opacity: 0.0 => 1.0;
            y: 12.0 => 0.0;
        };
        let theme = theme();
        let start = described.sample(&theme, 0.0);
        assert_eq!(start.opacity, 0.0);
        assert_eq!(start.y, 12.0);
        let middle = described.sample(&theme, 0.5);
        assert!((middle.opacity - 0.5).abs() < 0.02, "{middle:?}");
        assert!((middle.y - 6.0).abs() < 0.2, "{middle:?}");
        let end = described.sample(&theme, 1.0);
        assert_eq!(end.opacity, 1.0);
        assert_eq!(end.y, 0.0);
    }

    #[test]
    fn a_property_nobody_described_reports_its_neutral_value() {
        let described = motion! { duration: 200; opacity: 0.0 => 1.0; };
        let sample = described.sample(&theme(), 0.5);
        assert_eq!(sample.x, 0.0);
        assert_eq!(sample.y, 0.0);
        assert_eq!(sample.scale, 1.0);
        assert_eq!(
            MotionSample::default(),
            MotionSample {
                opacity: 1.0,
                x: 0.0,
                y: 0.0,
                scale: 1.0,
            }
        );
    }

    #[test]
    fn a_stop_list_passes_through_every_stop_it_names() {
        let described = motion! {
            duration: 600;
            ease: linear;
            opacity: [0.0 => 0.4, 0.5 => 1.0, 1.0 => 0.4];
        };
        let theme = theme();
        assert!((described.sample(&theme, 0.0).opacity - 0.4).abs() < 1e-3);
        assert!((described.sample(&theme, 0.5).opacity - 1.0).abs() < 0.02);
        assert!((described.sample(&theme, 1.0).opacity - 0.4).abs() < 1e-3);
        // Between two stops it is on the way, not at either of them.
        let quarter = described.sample(&theme, 0.25).opacity;
        assert!((0.4..1.0).contains(&quarter), "{quarter}");
    }

    #[test]
    fn stops_are_ordered_however_they_were_given() {
        let described = Motion::new(100)
            .eased(Easing::Linear)
            .track(MotionProperty::X, [(1.0, 10.0), (0.0, 0.0), (0.5, 8.0)]);
        let theme = theme();
        assert_eq!(described.sample(&theme, 0.0).x, 0.0);
        assert!((described.sample(&theme, 0.5).x - 8.0).abs() < 0.1);
        assert_eq!(described.sample(&theme, 1.0).x, 10.0);
    }

    #[test]
    fn describing_a_property_twice_keeps_the_last_description() {
        let described = Motion::new(100)
            .eased(Easing::Linear)
            .tween(MotionProperty::Y, 0.0, 4.0)
            .tween(MotionProperty::Y, 0.0, 40.0);
        assert_eq!(described.properties(), vec![MotionProperty::Y]);
        assert_eq!(described.sample(&theme(), 1.0).y, 40.0);
    }

    #[test]
    fn a_delay_holds_every_property_where_it_started() {
        let described = motion! {
            duration: 200;
            delay: 200;
            ease: linear;
            y: 10.0 => 0.0;
        };
        let theme = theme();
        assert_eq!(described.sample(&theme, 0.25).y, 10.0);
        assert_eq!(described.sample(&theme, 1.0).y, 0.0);
    }

    #[test]
    fn an_overshooting_curve_passes_the_value_it_is_travelling_to() {
        let described = motion! {
            duration: 300;
            ease: overshoot;
            y: 12.0 => 0.0;
        };
        let theme = theme();
        let least = (0..=100)
            .map(|step| described.sample(&theme, step as f32 / 100.0).y)
            .fold(f32::MAX, f32::min);
        assert!(
            least < 0.0,
            "an overshoot must pass its target, got {least}"
        );
        assert_eq!(described.sample(&theme, 1.0).y, 0.0);
    }

    #[test]
    fn opacity_never_leaves_the_range_an_element_can_be_told_about() {
        let described = motion! {
            duration: 300;
            ease: overshoot;
            opacity: 0.0 => 1.0;
        };
        let theme = theme();
        for step in 0..=100 {
            let opacity = described.sample(&theme, step as f32 / 100.0).opacity;
            assert!((0.0..=1.0).contains(&opacity), "{opacity}");
        }
    }

    #[test]
    fn an_alternating_motion_comes_back_to_where_it_started() {
        let described = motion! {
            duration: 400;
            ease: linear;
            alternate;
            y: 0.0 => 10.0;
        };
        let theme = theme();
        assert!(described.is_alternating());
        assert_eq!(described.sample(&theme, 0.0).y, 0.0);
        assert!((described.sample(&theme, 0.5).y - 10.0).abs() < 0.1);
        assert_eq!(described.sample(&theme, 1.0).y, 0.0);
        assert!(
            (described.sample(&theme, 0.25).y - described.sample(&theme, 0.75).y).abs() < 0.1,
            "an alternating run is a mirror of itself"
        );
    }

    #[test]
    fn a_curve_can_be_written_where_a_name_will_not_do() {
        let described = motion! {
            duration: 200;
            ease: CubicBezier::new(0.9, 0.0, 1.0, 0.4);
            opacity: 0.0 => 1.0;
        };
        let linear = motion! { duration: 200; ease: linear; opacity: 0.0 => 1.0; };
        let theme = theme();
        assert!(described.sample(&theme, 0.5).opacity < linear.sample(&theme, 0.5).opacity);
    }

    /// A NaN in a layout property is a box nobody can see, so it never
    /// reaches one.
    #[test]
    fn a_value_that_is_not_a_number_falls_back_to_the_neutral_one() {
        let described = motion! {
            duration: 200;
            ease: linear;
            y: 0.0 => f32::NAN;
            scale: 1.0 => f32::NAN;
            opacity: 1.0 => f32::NAN;
        };
        let sample = described.sample(&theme(), 0.5);
        assert_eq!(sample.y, 0.0);
        assert_eq!(sample.scale, 1.0);
        assert_eq!(sample.opacity, 1.0);
        assert!(described.sample(&theme(), f32::NAN).y.is_finite());
    }

    #[test]
    fn a_sample_reports_each_property_by_name() {
        let sample = MotionSample {
            opacity: 0.5,
            x: 1.0,
            y: 2.0,
            scale: 3.0,
        };
        assert_eq!(sample.get(MotionProperty::Opacity), 0.5);
        assert_eq!(sample.get(MotionProperty::X), 1.0);
        assert_eq!(sample.get(MotionProperty::Y), 2.0);
        assert_eq!(sample.get(MotionProperty::Scale), 3.0);
        assert_eq!(MotionProperty::Opacity.name(), "opacity");
        assert_eq!(MotionProperty::Y.neutral(), 0.0);
    }

    fn linear(ms: u64) -> MotionSpec {
        MotionSpec::new(ms, CubicBezier::new(0.0, 0.0, 1.0, 1.0))
    }

    #[test]
    fn a_written_sequence_is_the_sequence_it_describes() {
        let sequenced = sequence![linear(200), linear(100)];
        assert_eq!(sequenced, Sequence::new([linear(200)]).then(linear(100)));
        assert_eq!(sequenced.total(), Duration::from_millis(300));
    }

    #[test]
    fn a_gap_before_a_step_pushes_it_that_much_further_out() {
        let sequenced = sequence![linear(200), +80 linear(100)];
        assert_eq!(sequenced.step(1).expect("two steps").delay_ms, 280);
        assert_eq!(sequenced.total(), Duration::from_millis(380));
    }

    #[test]
    fn a_gap_is_added_to_the_delay_a_step_already_had() {
        let sequenced = sequence![linear(200), +80 linear(100).with_delay(20),];
        assert_eq!(sequenced.step(1).expect("two steps").delay_ms, 300);
    }

    #[test]
    fn a_sequence_of_one_and_of_none_are_both_sequences() {
        assert_eq!(sequence![linear(200)].len(), 1);
        let empty = sequence![];
        assert!(empty.is_empty());
        assert_eq!(empty.total(), Duration::ZERO);
    }
}
