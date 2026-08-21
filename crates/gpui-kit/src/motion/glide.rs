//! Eased travel toward a target whose distance is not known until you get
//! there.
//!
//! Ordinary interpolation needs both ends up front: it computes
//! `start + eased(t) · (target − start)` and needs `target − start` on the
//! first frame. Scrolling a virtualized list to a row that is far above the
//! viewport cannot supply that. The rows in between have never been laid out,
//! so how many pixels away the row is can only be estimated, and the estimate
//! is corrected every frame as rows are measured.
//!
//! The obvious repair — re-deriving the interpolation from the new distance
//! each frame — restarts the curve, so the motion eases in again from
//! wherever it had got to and the whole travel reads as a series of little
//! lurches. The other obvious repair — moving a fixed fraction of whatever is
//! left each frame — is not a curve at all: it is exponential decay, which
//! starts at its fastest and never quite lands.
//!
//! [`Glide`] is neither. It holds the curve and hands out, each frame, the
//! share of the *current* remaining distance that the curve says belongs to
//! this frame:
//!
//! ```text
//! frame's share = (eased_now − eased_before) / (1 − eased_before)
//! ```
//!
//! While the distance estimate holds still, consuming those shares telescopes
//! to exactly `start + eased(t) · total`: the fixed timeline, to the pixel. When
//! the estimate changes mid-flight, the same timeline simply continues over
//! the corrected remainder — no restart and no compensating jump, because the
//! share was never a share of the old distance. And because the curve reaches
//! 1, the last share is the whole remainder, so the motion lands exactly
//! rather than approaching forever.

/// A curve's progress, expressed as how much of what is left to go.
///
/// One `Glide` is one journey. It is driven by eased progress rather than by
/// time so that the caller keeps ownership of the clock: the same timeline
/// serves a frame loop, a test that steps by hand, and a motion scale the
/// reader has slowed down.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Glide {
    eased: f32,
}

impl Glide {
    pub fn new() -> Self {
        Self::default()
    }

    /// The share of the remaining distance this frame should consume, for a
    /// curve that has now reached `eased`.
    ///
    /// Progress that went backwards yields nothing rather than moving back:
    /// a curve is monotone, so a lower reading is a clock that stuttered, and
    /// undoing travel already made would be visible as a twitch.
    pub fn step(&mut self, eased: f32) -> f32 {
        let eased = eased.clamp(self.eased, 1.0);
        let left = 1.0 - self.eased;
        // At the end there is no proportion left to take a share of, and the
        // answer is the whole remainder — which is what lands the motion
        // exactly instead of asymptotically.
        let share = if left <= f32::EPSILON {
            1.0
        } else {
            (eased - self.eased) / left
        };
        self.eased = eased;
        share.clamp(0.0, 1.0)
    }

    /// Whether the journey is over.
    pub fn arrived(&self) -> bool {
        self.eased >= 1.0
    }

    /// How far along the curve this glide is, for a caller that wants to
    /// interpolate something else on the same timeline.
    pub fn progress(&self) -> f32 {
        self.eased
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::CubicBezier;

    const EASE: CubicBezier = CubicBezier::new(0.42, 0.0, 0.58, 1.0);

    #[test]
    fn shares_of_a_steady_distance_telescope_into_the_plain_eased_timeline() {
        // The claim the whole primitive rests on: while nothing is
        // re-estimated, this is ordinary interpolation, to the pixel.
        let mut glide = Glide::new();
        let (start, target) = (1000.0f32, 0.0f32);
        let mut here = start;

        for frame in 1..=60 {
            let time = frame as f32 / 60.0;
            let eased = EASE.eval(time);
            here -= glide.step(eased) * (here - target);
            let plainly = start + eased * (target - start);
            assert!(
                (here - plainly).abs() < 0.05,
                "frame {frame}: glided to {here}, interpolation says {plainly}"
            );
        }
        assert_eq!(here, target, "and it lands exactly rather than nearly");
    }

    #[test]
    fn a_distance_corrected_mid_flight_continues_the_same_curve() {
        // A row got measured and turned out to be twice as far away. Nothing
        // restarts: the shares are the curve's, not the old distance's.
        let mut glide = Glide::new();
        let mut here = 500.0f32;
        let mut before = 0.0f32;

        for frame in 1..=60 {
            let share = glide.step(EASE.eval(frame as f32 / 60.0));
            if frame == 30 {
                here *= 2.0;
            }
            here -= share * here;
            assert!((0.0..=1.0).contains(&share));
            if (2..55).contains(&frame) {
                assert!(
                    share >= before - 0.05,
                    "frame {frame}: the curve went backwards after a correction"
                );
            }
            before = share;
        }
        assert_eq!(here, 0.0);
    }

    #[test]
    fn the_first_frame_of_an_ease_in_out_is_a_crawl() {
        // The failure this prevents: a travel that covers most of its distance
        // in one frame and then creeps, which reads as a jump followed by a
        // stall rather than as motion.
        let mut glide = Glide::new();
        let first = glide.step(EASE.eval(16.0 / 500.0));
        assert!(first < 0.02, "the first frame took {first} of the distance");
    }

    #[test]
    fn a_clock_that_stutters_moves_nothing_backwards() {
        let mut glide = Glide::new();
        assert_eq!(glide.step(0.4), 0.4);
        assert_eq!(glide.step(0.3), 0.0);
        assert!(!glide.arrived());
        assert_eq!(glide.step(1.0), 1.0);
        assert!(glide.arrived());
        // Asking again after arriving is still "all of what is left", which is
        // nothing, so a caller that overruns its frame count lands rather than
        // panicking or drifting.
        assert_eq!(glide.step(1.0), 1.0);
    }
}
