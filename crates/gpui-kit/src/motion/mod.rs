//! Motion primitives that respect the user's reduced-motion preference.
//!
//! The layers build on each other:
//!
//! - [`CubicBezier`] and [`Easing`] name curves;
//! - [`Spring`] solves physical motion in closed form;
//! - [`MotionSpec`] pairs a curve with a duration and delay;
//! - [`Interpolate`] moves a value between two states;
//! - [`Keyframes`] takes a value through named stops rather than straight
//!   across;
//! - [`Transition`] animates a value whose target can change mid-flight,
//!   carrying the speed it already had across a retarget;
//! - [`VelocityTracker`] measures how fast a gesture is moving, which is what
//!   [`flick`], [`rubber_band`] and [`Transition::release`] need;
//! - [`ScrollLink`] reads a scroll offset as a progress, which is motion with
//!   no clock in it at all;
//! - [`Presence`] keeps an element alive long enough to animate out, and plays
//!   a phase backwards from where it had got to when the other cancels it;
//! - [`Stagger`] spreads one specification across a group, forwards or in
//!   reverse;
//! - [`Sequence`] runs specifications one after another and knows how long
//!   they take together;
//! - [`Flipping::flip`] slides an element from where it was to where it is,
//!   and [`Flipping::flip_size`] additionally resizes it;
//! - [`Animated::animate_in`] puts any of this on an element in one call,
//!   which is the layer most components should reach for.
//!
//! Motion never changes what a surface publishes. A slide, a press response
//! and a counting number are all painted over a layout, a hit target and a
//! semantic tree that already report the settled value.
//!
//! Decorative motion built on GPUI's `with_animation` already stops when
//! [`gpui::App::reduce_motion`] is set. [`Transition::animate`] and
//! [`Presence::animate`] honor the same preference by finishing immediately.

mod animated;
mod busy;
mod easing;
mod flip;
mod gesture;
mod interpolate;
pub(crate) mod keyed;
mod keyframes;
mod presence;
mod scroll_link;
mod sequence;
mod spec;
mod spring;
mod stagger;
mod transition;

pub use animated::{Animated, Entrance};
pub use busy::{Activity, breathe, breathe_as, breathing_dot, spin, sweep};
pub use easing::{CubicBezier, Easing};
pub use flip::{Flip, Flipped, Flipping, Shape, Shaping, flip, shared_flip, tracked_ids};
pub use gesture::{
    Flick, VELOCITY_WINDOW, Velocity, VelocityTracker, flick, overscroll, rubber_band,
};
pub use interpolate::Interpolate;
pub use keyframes::{Keyframe, Keyframes};
pub use presence::{Phase, Presence};
pub use scroll_link::ScrollLink;
pub use sequence::Sequence;
pub use spec::{
    MotionSpec, content_in, dialog, dialog_arrival, dialog_in, entrance, fade_in, gradient_opacity,
    menu, menu_in, pulse_wave, resize, row_in, shimmer_offset, state_change, surface_in, tracking,
};
pub use spring::Spring;
pub use stagger::{ROW_STAGGER_CAP, Stagger, staggered_phase};
pub use transition::Transition;
pub(crate) use transition::{tracked, tracked_or_snap};

pub use gpui::AnimationExt;

/// Whether the user asked for non-essential motion to be suppressed.
pub fn reduce_motion(cx: &gpui::App) -> bool {
    cx.reduce_motion()
}
