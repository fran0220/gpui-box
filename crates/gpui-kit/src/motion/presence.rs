//! Enter and exit lifecycles for elements that must outlive their own removal.
//!
//! An element cannot animate out after it has been dropped from the tree, so a
//! caller keeps rendering while [`Presence::is_rendered`] is true and drops the
//! element only once the exit has finished.

use std::time::Duration;

use gpui::{App, Window};
use web_time::Instant;

use super::MotionSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Entering,
    Present,
    Exiting,
    Gone,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presence {
    phase: Phase,
    elapsed: Duration,
    enter: MotionSpec,
    exit: MotionSpec,
    last_frame: Option<Instant>,
}

impl Presence {
    /// Starts hidden, so the first [`Presence::show`] animates in.
    pub fn hidden(enter: MotionSpec, exit: MotionSpec) -> Self {
        Self {
            phase: Phase::Gone,
            elapsed: Duration::ZERO,
            enter,
            exit,
            last_frame: None,
        }
    }

    /// Starts fully present, for content that exists before the first frame.
    pub fn visible(enter: MotionSpec, exit: MotionSpec) -> Self {
        Self {
            phase: Phase::Present,
            elapsed: Duration::ZERO,
            enter,
            exit,
            last_frame: None,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// True while the caller must keep the element in the tree, including for
    /// the whole exit animation.
    pub fn is_rendered(&self) -> bool {
        self.phase != Phase::Gone
    }

    pub fn is_animating(&self) -> bool {
        matches!(self.phase, Phase::Entering | Phase::Exiting)
    }

    /// 0 while absent, 1 while fully present.
    pub fn progress(&self) -> f32 {
        match self.phase {
            Phase::Gone => 0.0,
            Phase::Present => 1.0,
            Phase::Entering => self.span_progress(self.enter),
            Phase::Exiting => 1.0 - self.span_progress(self.exit),
        }
    }

    fn span_progress(&self, spec: MotionSpec) -> f32 {
        let total = spec.total().as_secs_f32();
        if total <= 0.0 {
            return 1.0;
        }
        spec.progress((self.elapsed.as_secs_f32() / total).clamp(0.0, 1.0))
    }

    /// Enters, or reverses an exit that is still in flight.
    pub fn show(&mut self) {
        match self.phase {
            Phase::Present | Phase::Entering => {}
            Phase::Gone => {
                self.phase = Phase::Entering;
                self.elapsed = Duration::ZERO;
            }
            Phase::Exiting => {
                // Resume the entrance from the opacity currently on screen
                // instead of restarting it.
                let remaining = self.progress();
                self.phase = Phase::Entering;
                self.elapsed = self.enter.total().mul_f32(remaining);
            }
        }
    }

    pub fn hide(&mut self) {
        match self.phase {
            Phase::Gone | Phase::Exiting => {}
            Phase::Present => {
                self.phase = Phase::Exiting;
                self.elapsed = Duration::ZERO;
            }
            Phase::Entering => {
                let shown = self.progress();
                self.phase = Phase::Exiting;
                self.elapsed = self.exit.total().mul_f32(1.0 - shown);
            }
        }
    }

    pub fn toggle(&mut self) {
        if matches!(self.phase, Phase::Present | Phase::Entering) {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Finishes the current phase instantly.
    pub fn settle(&mut self) {
        self.phase = match self.phase {
            Phase::Entering | Phase::Present => Phase::Present,
            Phase::Exiting | Phase::Gone => Phase::Gone,
        };
        self.elapsed = Duration::ZERO;
    }

    pub fn advance(&mut self, delta: Duration) {
        if !self.is_animating() {
            return;
        }
        self.elapsed += delta;
        let span = match self.phase {
            Phase::Entering => self.enter.total(),
            _ => self.exit.total(),
        };
        if self.elapsed >= span {
            self.settle();
        }
    }

    /// Advances by the time since the previous frame and requests the next one
    /// while a phase is running. Reduced motion skips straight to the end.
    pub fn animate(&mut self, window: &mut Window, cx: &mut App) -> f32 {
        if cx.reduce_motion() {
            self.settle();
            self.last_frame = None;
            return self.progress();
        }

        let now = cx.background_executor().now();
        if let Some(last) = self.last_frame {
            self.advance(now.saturating_duration_since(last));
        }
        if self.is_animating() {
            self.last_frame = Some(now);
            window.request_animation_frame();
        } else {
            self.last_frame = None;
        }
        self.progress()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{CubicBezier, MotionSpec};

    fn presence() -> Presence {
        let linear = |ms| MotionSpec::new(ms, CubicBezier::new(0.0, 0.0, 1.0, 1.0));
        Presence::hidden(linear(200), linear(100))
    }

    #[test]
    fn a_hidden_presence_renders_nothing() {
        let presence = presence();
        assert!(!presence.is_rendered());
        assert_eq!(presence.progress(), 0.0);
    }

    #[test]
    fn entering_becomes_present_only_after_the_full_span() {
        let mut presence = presence();
        presence.show();
        assert_eq!(presence.phase(), Phase::Entering);
        presence.advance(Duration::from_millis(100));
        assert_eq!(presence.phase(), Phase::Entering);
        assert!((presence.progress() - 0.5).abs() < 0.05);
        presence.advance(Duration::from_millis(100));
        assert_eq!(presence.phase(), Phase::Present);
        assert_eq!(presence.progress(), 1.0);
    }

    #[test]
    fn an_exiting_element_stays_rendered_until_the_exit_completes() {
        let mut presence = presence();
        presence.show();
        presence.advance(Duration::from_millis(200));
        presence.hide();

        assert!(
            presence.is_rendered(),
            "the exit needs the element on screen"
        );
        presence.advance(Duration::from_millis(50));
        assert!(presence.is_rendered());
        assert!(presence.progress() < 1.0);
        presence.advance(Duration::from_millis(50));
        assert_eq!(presence.phase(), Phase::Gone);
        assert!(!presence.is_rendered());
    }

    #[test]
    fn reversing_an_exit_resumes_from_what_is_on_screen() {
        let mut presence = presence();
        presence.show();
        presence.advance(Duration::from_millis(200));
        presence.hide();
        presence.advance(Duration::from_millis(50));
        let interrupted = presence.progress();

        presence.show();
        assert_eq!(presence.phase(), Phase::Entering);
        assert!((presence.progress() - interrupted).abs() < 0.05);
    }

    #[test]
    fn reversing_an_entrance_resumes_from_what_is_on_screen() {
        let mut presence = presence();
        presence.show();
        presence.advance(Duration::from_millis(100));
        let interrupted = presence.progress();

        presence.hide();
        assert_eq!(presence.phase(), Phase::Exiting);
        assert!((presence.progress() - interrupted).abs() < 0.05);
    }

    #[test]
    fn toggling_alternates_between_the_two_ends() {
        let mut presence = presence();
        presence.toggle();
        assert_eq!(presence.phase(), Phase::Entering);
        presence.settle();
        presence.toggle();
        assert_eq!(presence.phase(), Phase::Exiting);
    }
}
