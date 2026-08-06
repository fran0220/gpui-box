//! Motion primitives that respect the user's reduced-motion preference.
//!
//! The layers build on each other:
//!
//! - [`CubicBezier`] and [`Easing`] name curves;
//! - [`Spring`] solves physical motion in closed form;
//! - [`MotionSpec`] pairs a curve with a duration and delay;
//! - [`Interpolate`] moves a value between two states;
//! - [`Transition`] animates a value whose target can change mid-flight;
//! - [`Presence`] keeps an element alive long enough to animate out;
//! - [`Stagger`] spreads one specification across a group.
//!
//! Decorative motion built on GPUI's `with_animation` already stops when
//! [`gpui::App::reduce_motion`] is set. [`Transition::animate`] and
//! [`Presence::animate`] honor the same preference by finishing immediately.

mod easing;
mod interpolate;
mod presence;
mod spec;
mod spring;
mod stagger;
mod transition;

pub use easing::{CubicBezier, Easing};
pub use interpolate::Interpolate;
pub use presence::{Phase, Presence};
pub use spec::{
    MotionSpec, dialog, dialog_in, entrance, fade_in, gradient_opacity, menu, menu_in, pulse_wave,
};
pub use spring::Spring;
pub use stagger::{Stagger, staggered_phase};
pub use transition::Transition;

pub use gpui::AnimationExt;

/// Whether the user asked for non-essential motion to be suppressed.
pub fn reduce_motion(cx: &gpui::App) -> bool {
    cx.reduce_motion()
}
