//! A value read off a scroll offset rather than off a clock.
//!
//! Everything else in [`motion`](crate::motion) is a function of time: it is
//! started, it runs, it settles, and while it runs it asks for the next frame.
//! A scroll-linked value is a function of where the content is. It has no
//! duration, no start and no end, it never requests an animation frame, and
//! there is no such thing as interrupting it — scrolling back up runs it
//! backwards because the offset went backwards.
//!
//! That is why this is a plain value with no `animate` and no `Window`: there
//! is nothing to drive. A caller reads the offset it already has and asks what
//! the progress is.
//!
//! ```
//! # use gpui::px;
//! # use gpui_kit::motion::ScrollLink;
//! let header = ScrollLink::new(px(0.0), px(64.0));
//! let height = header.sample(px(32.0), px(96.0), px(40.0));
//! ```

use gpui::{Pixels, px};

use super::Interpolate;

/// Scroll offsets mapped onto progress from 0 to 1.
///
/// # Reduced motion
///
/// A link makes no decision of its own, and that is deliberate rather than
/// lazy. A header that collapses as the content scrolls under it, or a shadow
/// that appears once there is something above the fold, is not gratuitous
/// motion: it is a direct, one-to-one response to a movement the user is
/// making with their own hand, and suppressing it would remove information
/// rather than calm. A decorative parallax — a background drifting at a
/// different rate to say nothing at all — is the opposite, and under reduced
/// motion it should not drift.
///
/// Only the caller knows which of those it is building, so the caller says so
/// with [`ScrollLink::decorative`]. A decorative link marked under reduced
/// motion reports 0 at every offset, which is the resting end of the effect:
/// the parallax layer simply sits where it belongs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollLink {
    start: f32,
    end: f32,
    suppressed: bool,
}

impl ScrollLink {
    /// Over the offsets `start..end`, measured the way a reader thinks about
    /// scrolling: 0 is the top of the content and the number grows as the
    /// content moves up.
    ///
    /// A range with no length is a threshold rather than a ramp: progress is 0
    /// below it and 1 from it on.
    pub fn new(start: Pixels, end: Pixels) -> Self {
        Self {
            start: f32::from(start),
            end: f32::from(end),
            suppressed: false,
        }
    }

    /// Over the first `distance` of scrolling.
    pub fn over(distance: Pixels) -> Self {
        Self::new(px(0.0), distance)
    }

    /// Marks the effect decorative, and suppresses it when the user has asked
    /// for less motion.
    ///
    /// Pass [`reduce_motion`](super::reduce_motion). A link left unmarked
    /// always reports the offset, because a response to the user's own
    /// scrolling is not the motion the preference is about.
    pub fn decorative(mut self, reduce_motion: bool) -> Self {
        self.suppressed = reduce_motion;
        self
    }

    /// Where `offset` sits in the range: 0 before it, 1 after it, and
    /// monotonically between.
    pub fn progress(self, offset: Pixels) -> f32 {
        if self.suppressed {
            return 0.0;
        }
        let offset = f32::from(offset);
        let span = self.end - self.start;
        if span <= 0.0 {
            return if offset >= self.start { 1.0 } else { 0.0 };
        }
        ((offset - self.start) / span).clamp(0.0, 1.0)
    }

    /// Anything interpolable, read off the offset.
    ///
    /// Anything that takes a progress can be driven from here, including
    /// [`Keyframes::sample`](super::Keyframes::sample) for a value that passes
    /// through stops on the way.
    pub fn sample<T: Interpolate>(self, offset: Pixels, from: T, to: T) -> T {
        from.lerp(to, self.progress(offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    fn link() -> ScrollLink {
        ScrollLink::new(px(20.0), px(120.0))
    }

    #[test]
    fn progress_is_nothing_before_the_range_and_all_of_it_after() {
        assert_eq!(link().progress(px(0.0)), 0.0);
        assert_eq!(link().progress(px(20.0)), 0.0);
        assert_eq!(link().progress(px(120.0)), 1.0);
        assert_eq!(link().progress(px(4_000.0)), 1.0);
    }

    #[test]
    fn progress_only_ever_grows_within_the_range() {
        let mut previous = 0.0;
        for step in 0..=200 {
            let progress = link().progress(px(step as f32));
            assert!(progress >= previous, "progress went backwards at {step}");
            previous = progress;
        }
        assert_eq!(previous, 1.0);
    }

    #[test]
    fn a_range_with_no_length_is_a_threshold() {
        let threshold = ScrollLink::new(px(50.0), px(50.0));
        assert_eq!(threshold.progress(px(49.9)), 0.0);
        assert_eq!(threshold.progress(px(50.0)), 1.0);
    }

    #[test]
    fn a_sampled_value_travels_across_the_range() {
        assert_eq!(ScrollLink::over(px(100.0)).sample(px(50.0), 0.0, 10.0), 5.0);
        assert_eq!(
            ScrollLink::over(px(100.0)).sample(px(400.0), px(80.0), px(40.0)),
            px(40.0)
        );
    }

    #[test]
    fn a_decorative_effect_rests_under_reduced_motion() {
        let parallax = link().decorative(true);
        assert_eq!(parallax.progress(px(80.0)), 0.0);
        assert_eq!(parallax.progress(px(400.0)), 0.0);
        // The same link, when it is answering the user's own scrolling.
        assert!(link().decorative(false).progress(px(80.0)) > 0.0);
    }
}
