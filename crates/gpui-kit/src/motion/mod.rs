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
//! - [`Presence`] keeps an element alive long enough to animate out;
//! - [`Stagger`] spreads one specification across a group;
//! - [`Flipping::flip`] slides an element from where it was to where it is.
//!
//! Motion never changes what a surface publishes. A slide, a press response
//! and a counting number are all painted over a layout, a hit target and a
//! semantic tree that already report the settled value.
//!
//! Decorative motion built on GPUI's `with_animation` already stops when
//! [`gpui::App::reduce_motion`] is set. [`Transition::animate`] and
//! [`Presence::animate`] honor the same preference by finishing immediately.

mod easing;
mod flip;
mod interpolate;
pub(crate) mod keyed;
mod keyframes;
mod presence;
mod spec;
mod spring;
mod stagger;
mod transition;

pub use easing::{CubicBezier, Easing};
pub use flip::{Flip, Flipped, Flipping, flip, tracked_ids};
pub use interpolate::Interpolate;
pub use keyframes::{Keyframe, Keyframes};
pub use presence::{Phase, Presence};
pub use spec::{
    MotionSpec, content_in, dialog, dialog_arrival, dialog_in, entrance, fade_in, gradient_opacity,
    menu, menu_in, pulse_wave, resize, row_in, shimmer_offset, state_change, tracking,
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
