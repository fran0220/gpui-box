//! Holding a surface against its own end while that end keeps moving.
//!
//! A log, a conversation and a build output all grow at the bottom while
//! someone is reading them, and all three want the same thing: the newest
//! content stays in view without the reader being yanked, and the moment the
//! reader scrolls up the surface lets go and stays let go until they come back.
//!
//! Scrolling to the last row does not do this. It jumps, it re-jumps on every
//! token of a streaming reply, and it cannot tell a reader who left from a
//! reader who never moved. What does is a spring whose target is the end of
//! the content and whose position is the scroll offset — with one addition
//! that matters more than the spring: a feed-forward term. A spring alone
//! always trails a target that is moving away from it, so a streaming reply
//! would read at a permanent lag that grows with the token rate. Estimating
//! how fast the end is receding and carrying the viewport at that speed
//! removes the lag, leaving the spring to correct what the estimate misses.
//!
//! Following is broken by input and by nothing else. Content arriving is not
//! an interruption, so a surface that grows under a reader who is at the end
//! keeps them at the end; a wheel notch upward is an interruption, so it stops.
//! Coming back inside a band near the end re-engages, but only while moving
//! toward it, or a reader nudging up from the bottom would be snapped back
//! down by their own gesture and the pin would be unbreakable.

use std::collections::HashMap;
use std::time::Duration;

use gpui::{App, ListState, Pixels, SharedString, Window, WindowId, px};
use web_time::Instant;

use crate::data::viewport::flow_state;
use crate::foundation::{Ident, window_state};
use crate::motion::reduce_motion;

/// How much velocity survives a frame — higher glides longer.
const DAMPING: f32 = 0.7;
/// How hard the end pulls — higher is snappier.
const STIFFNESS: f32 = 0.05;
/// Inertia — higher is slower to start and slower to stop.
const MASS: f32 = 1.25;
/// The frame this integration is written in terms of, so a 144Hz display and a
/// 60Hz display travel the same distance in the same wall time.
const FRAME_MS: f32 = 1000.0 / 60.0;
/// The most frames one step will simulate. A hitch catches up over the frames
/// it missed instead of teleporting the distance in one.
const MAX_CATCHUP: f32 = 8.0;
/// A render that reports no elapsed time still advanced something, and this is
/// what it is worth. Small enough that a display running faster than the
/// reference frame is not sped up by rounding.
const MIN_STEP: f32 = 0.25;
/// How quickly the growth estimate follows what it observes.
const GROWTH_EMA: f32 = 0.12;
/// How far above the true end the spring aims while the end is receding.
///
/// Aiming exactly at a moving end means the last line is always arriving at
/// the edge of the viewport. Aiming a little short of it keeps the newest
/// content off the boundary, which is where it can actually be read.
const CHASE_LEAD: f32 = 32.0;
/// Within this, the surface is at its end.
const AT_END: f32 = 2.0;
/// How long the loop stays warm after landing, so a stream that pauses for a
/// moment resumes at speed instead of accelerating from nothing.
const SETTLE_GRACE: Duration = Duration::from_millis(500);
/// Further than this many viewports from the end, close the gap instantly and
/// glide the rest. Nobody reads a two-second scroll past content they did not
/// ask to see.
const GLIDE_MAX_VIEWPORTS: f32 = 2.5;
/// How near the end a returning reader has to get before the surface takes
/// over again.
pub const STICK_BAND: f32 = 70.0;

/// The scroll velocity of a surface being held against a moving end.
///
/// Position and target are scroll offsets in pixels, larger meaning nearer the
/// end. The integration is fixed-timestep at one 60Hz frame so that behaviour is
/// a property of the spring rather than of the display it is running on, and a
/// step is clamped at the target: this never overshoots, never oscillates, and
/// lands exactly rather than asymptotically.
#[derive(Debug, Clone, Copy)]
pub struct Chase {
    /// Pixels per reference frame.
    velocity: f32,
    /// The smoothed estimate of how fast the target is receding, in pixels per
    /// reference frame.
    growth: f32,
    /// The target as the previous step saw it, or `None` when parked.
    target: Option<f32>,
}

impl Default for Chase {
    fn default() -> Self {
        Self::new()
    }
}

impl Chase {
    /// A parked chase, carrying no speed and no estimate.
    pub fn new() -> Self {
        Self {
            velocity: 0.0,
            growth: 0.0,
            target: None,
        }
    }

    /// Park it. The next step starts cold.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Whether there is any motion left to spend.
    pub fn is_idle(&self) -> bool {
        self.velocity < 0.05 && self.growth < 0.05
    }

    /// How fast the end is estimated to be receding, in pixels per reference
    /// frame.
    pub fn growth(&self) -> f32 {
        self.growth
    }

    /// Advance by `frames` reference frames and answer the new position.
    pub fn step(&mut self, mut position: f32, target: f32, mut frames: f32) -> f32 {
        let grew = self.target.map_or(0.0, |last| target - last);
        self.target = Some(target);
        if grew < -1.0 {
            // The content shrank: a row collapsed, a message was withdrawn,
            // the surface was replaced. Whatever the estimate had learned
            // described a different sequence.
            self.growth = 0.0;
        } else {
            let observed = grew.max(0.0) / frames.max(MIN_STEP);
            self.growth += GROWTH_EMA * (observed - self.growth);
        }

        // Aim short of the end by roughly the distance the next few frames of
        // growth will cover, bounded so a burst does not aim into the middle
        // of the content.
        let chase = target - (self.growth * 9.0).min(CHASE_LEAD);
        let mut velocity = self.velocity;
        while frames > 0.0 {
            let step = frames.min(1.0);
            frames -= step;
            let gap = (chase - position).max(0.0);
            velocity += step * ((DAMPING * velocity + STIFFNESS * gap) / MASS - velocity);
            position = (position + (velocity + self.growth) * step).min(target);
        }
        self.velocity = velocity;

        // Land exactly. Half a pixel of remaining gap is not motion anyone can
        // see, and leaving it there keeps the loop awake forever.
        if target - position <= 0.5 {
            target
        } else {
            position
        }
    }
}

/// What one frame found out about a surface's relationship to its end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtEnd {
    /// Whether the end is being held.
    pub pinned: bool,
    /// How far above the end the view is sitting.
    pub distance: Pixels,
    /// Whether the motion has finished, so a caller can tell a surface that
    /// has arrived from one still travelling.
    pub settled: bool,
}

/// What one surface's follow is doing between frames.
struct Follower {
    chase: Chase,
    pinned: bool,
    /// The distance the last frame or the last gesture saw, which is what
    /// makes a gesture's direction readable.
    distance: f32,
    ticked: Option<Instant>,
    settled: Option<Instant>,
    /// Whether the loop has to run again whatever the measurements say.
    ///
    /// A frame that draws newly arrived content is measuring the layout from
    /// before it arrived, so the end still reads as being underfoot and the
    /// loop would park exactly when it is needed. Something that knows content
    /// changed sets this, and the frame it forces is the one that can see the
    /// new end.
    woken: bool,
    /// Whether this surface's list is already reporting its gestures here.
    listening: bool,
}

impl Default for Follower {
    fn default() -> Self {
        Self {
            chase: Chase::new(),
            // A surface is not followed until something says it is. A list
            // that opened at its top is showing what its caller asked for.
            pinned: false,
            distance: 0.0,
            ticked: None,
            settled: None,
            woken: false,
            listening: false,
        }
    }
}

type Followers = HashMap<SharedString, Follower>;

fn with_follower<R>(
    ident: &Ident,
    window_id: WindowId,
    cx: &mut App,
    act: impl FnOnce(&mut Follower) -> R,
) -> R {
    window_state::with(window_id, cx, |followers: &mut Followers| {
        act(followers.entry(ident.semantic_id()).or_default())
    })
}

/// How far above its end this list is sitting.
fn distance_from_end(state: &ListState) -> f32 {
    let max = f32::from(state.max_offset_for_scrollbar().y);
    let offset = f32::from(state.scroll_px_offset_for_scrollbar().y);
    (max + offset).max(0.0)
}

/// Keeps the surface named by `ident` against its end, and reports where it is.
///
/// Called once per frame by whatever draws the list. It installs the gesture
/// listener the first time it sees the list, advances the spring while the end
/// is being held, and asks for another frame while there is anywhere left to
/// travel — so a caller does not schedule anything itself.
///
/// A surface that has not yet drawn as a variable-height list has nothing to
/// follow and is reported as settled where it is.
pub fn follow_end(ident: &Ident, window: &mut Window, cx: &mut App) -> AtEnd {
    let window_id = window.window_handle().window_id();
    let Some(state) = flow_state(ident, window_id, cx) else {
        let pinned = with_follower(ident, window_id, cx, |follower| follower.pinned);
        return AtEnd {
            pinned,
            distance: px(0.0),
            settled: true,
        };
    };

    let listening = with_follower(ident, window_id, cx, |follower| {
        std::mem::replace(&mut follower.listening, true)
    });
    if !listening {
        listen(ident, &state);
    }

    let distance = distance_from_end(&state);
    let pinned = with_follower(ident, window_id, cx, |follower| {
        follower.distance = distance;
        follower.pinned
    });

    if !pinned {
        with_follower(ident, window_id, cx, |follower| {
            follower.chase.reset();
            follower.ticked = None;
            follower.settled = None;
        });
        return AtEnd {
            pinned: false,
            distance: px(distance),
            settled: true,
        };
    }

    // Someone who asked for no motion asked for no motion. The end is still
    // held; it is simply held by arriving there.
    if reduce_motion(cx) {
        if distance > 0.0 {
            state.scroll_to_end();
        }
        with_follower(ident, window_id, cx, |follower| {
            follower.chase.reset();
            follower.distance = 0.0;
        });
        return AtEnd {
            pinned: true,
            distance: px(0.0),
            settled: true,
        };
    }

    let now = cx.background_executor().now();
    let target = f32::from(state.max_offset_for_scrollbar().y);
    let viewport = f32::from(state.viewport_bounds().size.height);

    // A gap this wide is not a scroll anyone wants to watch: a conversation
    // opened part-way up its history, a paste of a thousand lines. Close it to
    // within a couple of viewports and glide the rest, which is the part that
    // reads as continuous.
    let mut distance = distance;
    let glide_max = GLIDE_MAX_VIEWPORTS * viewport;
    if viewport > 0.0 && distance > glide_max {
        state.scroll_by(px(distance - glide_max));
        distance = glide_max;
    }

    let (moved, remaining, settled) = with_follower(ident, window_id, cx, |follower| {
        let woken = std::mem::take(&mut follower.woken);
        let frames = match follower.ticked {
            Some(last) => ((now.saturating_duration_since(last).as_secs_f32() * 1000.0) / FRAME_MS)
                .clamp(MIN_STEP, MAX_CATCHUP),
            // The first step of a fresh follow is worth one frame, not zero:
            // there is no previous tick to measure against, and standing still
            // is not the answer.
            None => 1.0,
        };
        follower.ticked = Some(now);

        let position = target - distance;
        let next = follower.chase.step(position, target, frames);
        let remaining = (target - next).max(0.0);
        follower.distance = remaining;

        let settled = if remaining <= 0.5 {
            let landed = *follower.settled.get_or_insert(now);
            !woken
                && now.saturating_duration_since(landed) >= SETTLE_GRACE
                && follower.chase.is_idle()
        } else {
            follower.settled = None;
            false
        };
        if settled {
            follower.chase.reset();
            follower.ticked = None;
            follower.settled = None;
        }
        (next - position, remaining, settled)
    });

    if moved > 0.0 {
        state.scroll_by(px(moved));
    }
    if !settled {
        window.request_animation_frame();
    }

    AtEnd {
        pinned: true,
        distance: px(remaining),
        settled,
    }
}

/// Hold the end from now on, gliding to it from wherever the view is.
///
/// This is what an own send and a "jump to newest" affordance do. It is not
/// what arriving content does: content that arrives while the surface is
/// already following is followed, and content that arrives while it is not
/// following must not drag the reader anywhere.
pub fn engage_end(ident: &Ident, window: &Window, cx: &mut App) {
    with_follower(ident, window.window_handle().window_id(), cx, |follower| {
        follower.pinned = true;
        follower.woken = true;
        follower.settled = None;
    });
    // Asking to travel is asking for the frames to travel in. Leaving that to
    // the caller means the journey starts only when something else happens to
    // redraw, which is a wait nobody asked for and a bug nobody would think to
    // look for here.
    cx.refresh_windows();
}

/// Stop holding the end, leaving the view exactly where it is.
pub fn release_end(ident: &Ident, window: &Window, cx: &mut App) {
    with_follower(ident, window.window_handle().window_id(), cx, |follower| {
        follower.pinned = false;
        follower.chase.reset();
        follower.ticked = None;
        follower.settled = None;
    });
}

/// Whether this surface is currently holding its end.
pub fn follows_end(ident: &Ident, window: &Window, cx: &App) -> bool {
    window_state::read(
        window.window_handle().window_id(),
        cx,
        |followers: &Followers| {
            followers
                .get(&ident.semantic_id())
                .is_some_and(|follower| follower.pinned)
        },
    )
    .unwrap_or(false)
}

/// Whether a gesture that ended up this far from the end should re-engage.
///
/// Direction is half the answer. A reader nudging up from the very bottom is
/// still inside the band, and re-engaging on that would snap them straight
/// back down: the pin would be unbreakable by the only gesture anyone would
/// use to break it.
fn should_restick(distance: f32, previous: f32) -> bool {
    distance <= STICK_BAND && distance < previous
}

/// Teaches the list to report its own gestures.
///
/// The list calls this handler from its wheel and touch path only —
/// programmatic scrolling never re-enters it — which is exactly the
/// distinction following needs, because content arriving must not read as the
/// reader leaving.
fn listen(ident: &Ident, state: &ListState) {
    let ident = ident.clone();
    state.set_scroll_handler(move |_event, window, cx| {
        // The list is holding its own borrow while it calls this, so reading
        // the position back now would panic. By the end of the effect cycle it
        // has let go.
        let ident = ident.clone();
        let window_id = window.window_handle().window_id();
        cx.defer(move |cx| {
            let Some(state) = flow_state(&ident, window_id, cx) else {
                return;
            };
            let distance = distance_from_end(&state);
            let changed = with_follower(&ident, window_id, cx, |follower| {
                let previous = follower.distance;
                follower.distance = distance;
                let was = follower.pinned;
                if distance > previous + 1.0 && distance > AT_END {
                    follower.pinned = false;
                    follower.chase.reset();
                    follower.ticked = None;
                    follower.settled = None;
                } else if !follower.pinned
                    && (distance <= AT_END || should_restick(distance, previous))
                {
                    follower.pinned = true;
                    follower.woken = true;
                    follower.settled = None;
                }
                was != follower.pinned
            });
            if changed {
                cx.refresh_windows();
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chase_lands_exactly_on_a_fixed_end_and_stays() {
        let mut chase = Chase::new();
        let target = 400.0;
        let mut position = 0.0;
        let mut frames = 0;
        while position < target && frames < 600 {
            position = chase.step(position, target, 1.0);
            frames += 1;
        }
        assert_eq!(position, target, "a chase lands on its target, not near it");
        assert!(frames < 300, "400px took {frames} frames, which is a crawl");
        for _ in 0..120 {
            position = chase.step(position, target, 1.0);
            assert_eq!(position, target, "landed is landed");
        }
        assert!(chase.is_idle());
    }

    #[test]
    fn a_chase_never_overshoots_and_never_goes_backwards() {
        // Overshoot in a conversation reads as the content bouncing, and
        // moving backwards reads as the surface fighting the reader.
        let mut chase = Chase::new();
        let target = 250.0;
        let mut position = 0.0;
        let mut last = position;
        for _ in 0..600 {
            position = chase.step(position, target, 1.0);
            assert!(position <= target, "overshot to {position}");
            assert!(
                position >= last - 1e-3,
                "went backwards {last} -> {position}"
            );
            last = position;
        }
        assert_eq!(position, target);
    }

    #[test]
    fn a_receding_end_is_tracked_at_its_own_speed() {
        // Two pixels a frame is an ordinary streaming reply. The thing being
        // tested is not that the viewport arrives — it is that it moves every
        // frame by about what the content grew, because a viewport that
        // catches up in bursts is the jerk this exists to remove.
        let growth = 2.0;
        let mut chase = Chase::new();
        let mut target = 600.0;
        let mut position = 600.0;
        let mut steady: Vec<f32> = Vec::new();
        for frame in 0..400 {
            target += growth;
            let next = chase.step(position, target, 1.0);
            if frame >= 200 {
                steady.push(next - position);
            }
            position = next;
        }

        let mean = steady.iter().sum::<f32>() / steady.len() as f32;
        assert!(
            (mean - growth).abs() < 0.2,
            "settled speed {mean} should match the growth {growth} it is tracking"
        );
        for moved in &steady {
            assert!(*moved > 0.0, "the viewport stalled while content arrived");
            assert!(*moved < growth * 3.0, "the viewport jumped {moved}px");
        }
        assert!(
            (chase.growth() - growth).abs() < 0.3,
            "the estimate itself should have converged"
        );
        assert!(
            target - position <= CHASE_LEAD + growth,
            "the lag should stay within the lead it is aiming with"
        );
    }

    #[test]
    fn the_estimate_is_dropped_when_the_content_shrinks() {
        let mut chase = Chase::new();
        let mut position = 0.0;
        for step in 1..=50 {
            position = chase.step(position, 100.0 + step as f32 * 4.0, 1.0);
        }
        assert!(chase.growth() > 1.0, "the estimate should have locked on");

        // A row collapsed. What the estimate learned was about a sequence that
        // no longer exists.
        chase.step(position.min(120.0), 120.0, 1.0);
        assert_eq!(chase.growth(), 0.0);
    }

    #[test]
    fn a_dropped_frame_catches_up_instead_of_teleporting() {
        let target = 300.0;
        let mut stepped = Chase::new();
        let mut position = 0.0;
        for _ in 0..5 {
            position = stepped.step(position, target, 1.0);
        }
        let mut hitched = Chase::new();
        let caught_up = hitched.step(0.0, target, 5.0);
        assert!(
            (position - caught_up).abs() < 1.0,
            "five frames at once should land where five frames did: {position} vs {caught_up}"
        );
        assert!(caught_up <= target);
    }

    #[test]
    fn re_engaging_reads_the_direction_of_the_gesture() {
        // Leaving the bottom never re-engages, however small the movement.
        assert!(!should_restick(20.0, 0.0));
        assert!(!should_restick(69.0, 30.0));
        // Coming back does, once inside the band.
        assert!(should_restick(69.0, 120.0));
        assert!(should_restick(0.0, 30.0));
        // But not from outside it.
        assert!(!should_restick(200.0, 300.0));
        // A gesture that moved nothing decides nothing.
        assert!(!should_restick(50.0, 50.0));
    }
}
