//! Enter and exit lifecycles for elements that must outlive their own removal.
//!
//! An element cannot animate out after it has been dropped from the tree, so a
//! caller keeps rendering while [`Presence::is_rendered`] is true and drops the
//! element only once the exit has finished.

use std::time::Duration;

use gpui::{App, Window};
use web_time::Instant;

use super::MotionSpec;

/// Where an element is in its arrival or departure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Entering,
    Present,
    Exiting,
    Gone,
}

/// An element's arrival and departure, either of which can be cancelled by the
/// other while it is still running.
///
/// A cancelled phase is played backwards from where it had got to rather than
/// restarted. The two phases are separate specifications with separate
/// durations and separate curves, so "where it had got to" is a position and
/// not a time: the visible progress is looked up in the other specification —
/// [`MotionSpec::time_at`] — and the reversal starts from the point that
/// produces it. An entrance cancelled at 30% therefore leaves from 30%,
/// through the exit's own curve, in the part of the exit's time that is left
/// once 70% of it is already behind.
///
/// This is deliberately not the velocity handover
/// [`Transition`](super::Transition) performs on a retarget. A value aimed
/// somewhere new is still going the way it was going; a phase that is
/// cancelled has been told to go back, and carrying the speed across would
/// mean an element on its way in overshooting past being present, which is not
/// a state a lifecycle has.
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
                let visible = self.progress();
                self.phase = Phase::Entering;
                self.elapsed = self.enter.time_at(visible);
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
                let visible = self.progress();
                self.phase = Phase::Exiting;
                // An exit runs from present to gone, so being `visible` at all
                // means it has already covered the rest of its path.
                self.elapsed = self.exit.time_at(1.0 - visible);
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

/// A floating surface's whole life, from its arrival to the frame after it is
/// dismissed.
///
/// The library's arrival helpers — [`menu_in`](super::menu_in),
/// [`dialog_in`](super::dialog_in), [`fade_in`](super::fade_in) — are built on
/// `with_animation`, which runs once when an element appears and has nowhere
/// to put a departure. An element cannot animate out after it has been dropped
/// from the tree, so a surface built on them either snaps away or keeps a
/// [`Presence`] and hand-writes everything around it. `Toast` and `Drawer` each
/// did the second, which is why an application wanting a command palette that
/// closes had to copy a component's private recipe rather than use one.
///
/// This is that recipe, named. It pairs the enter specification the role
/// carries with the library's one exit, keeps the two phases cancellable by
/// each other, and hands out the progress that
/// [`presenting`](super::presenting) turns into an appearance. A host that
/// holds one of these and honours [`is_rendered`](Self::is_rendered) has the
/// same lifecycle every overlay in the library has.
///
/// ```no_run
/// # use gpui_kit::motion::{MotionRole, Presenting, presenting};
/// # use gpui_kit::prelude::*;
/// # fn example(theme: &gpui_kit_theme::Theme) {
/// let mut palette = Presenting::closed(theme, MotionRole::MenuEnter);
/// palette.open();
/// // While `palette.is_rendered()` the surface stays in the tree, even
/// // after `palette.close()`, which is what gives the exit somewhere to run.
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presenting {
    presence: Presence,
    role: super::MotionRole,
}

impl Presenting {
    /// Starts closed, so the first [`open`](Self::open) animates in.
    ///
    /// The role decides the arrival: a menu answers the action that opened it,
    /// a modal arrives with weight, and content that becomes part of the
    /// surface rises. Every departure is
    /// [`MotionRole::Exit`](crate::motion::MotionRole::Exit), because leaving is
    /// one thing however the surface arrived — a surface that took its entrance
    /// curve back out would decelerate on the way off screen, which reads as
    /// reluctance rather than dismissal.
    pub fn closed(theme: &gpui_kit_theme::Theme, role: super::MotionRole) -> Self {
        Self {
            presence: Presence::hidden(
                super::MotionPolicy::spec(role, theme),
                super::MotionPolicy::spec(super::MotionRole::Exit, theme),
            ),
            role,
        }
    }

    /// Starts open, for a surface that exists before the first frame.
    pub fn opened(theme: &gpui_kit_theme::Theme, role: super::MotionRole) -> Self {
        let mut presenting = Self::closed(theme, role);
        presenting.presence = Presence::visible(
            super::MotionPolicy::spec(role, theme),
            super::MotionPolicy::spec(super::MotionRole::Exit, theme),
        );
        presenting
    }

    /// The role this surface arrives on.
    pub const fn role(&self) -> super::MotionRole {
        self.role
    }

    /// Arrives, or reverses a departure still in flight.
    pub fn open(&mut self) {
        self.presence.show();
    }

    /// Starts the departure. The surface stays rendered until it finishes.
    pub fn close(&mut self) {
        self.presence.hide();
    }

    pub fn toggle(&mut self) {
        self.presence.toggle();
    }

    /// Finishes whichever phase is running, for a host that wants the surface
    /// where it is going without the frames in between.
    pub fn settle(&mut self) {
        self.presence.settle();
    }

    /// True while the host must keep the surface in the tree, including for
    /// the whole departure.
    pub fn is_rendered(&self) -> bool {
        self.presence.is_rendered()
    }

    /// True while the surface is here or on its way here, which is the
    /// question a focus trap, a stack push or an escape binding asks — not
    /// whether it is painted.
    pub fn is_open(&self) -> bool {
        matches!(self.phase(), Phase::Entering | Phase::Present)
    }

    pub fn phase(&self) -> Phase {
        self.presence.phase()
    }

    pub fn progress(&self) -> f32 {
        self.presence.progress()
    }

    /// Advances one frame and requests the next while a phase is running.
    /// Reduced motion settles immediately.
    pub fn animate(&mut self, window: &mut Window, cx: &mut App) -> f32 {
        self.presence.animate(window, cx)
    }

    pub fn advance(&mut self, delta: Duration) {
        self.presence.advance(delta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::{CubicBezier, MotionSpec};

    /// The gap this closes: `with_animation` runs once and only forwards, so
    /// every surface built on the `*_in` helpers snapped away, and the two
    /// that did not each kept a `Presence` and hand-wrote the rest.
    #[test]
    fn a_surface_stays_rendered_through_its_whole_departure() {
        let theme = gpui_kit_theme::Theme::studio_dark();
        let mut palette = Presenting::closed(&theme, crate::motion::MotionRole::MenuEnter);
        assert!(!palette.is_rendered());
        assert!(!palette.is_open());

        palette.open();
        palette.advance(Duration::from_millis(1_000));
        assert_eq!(palette.phase(), Phase::Present);
        assert_eq!(palette.progress(), 1.0);
        assert!(palette.is_open());

        palette.close();
        // Still on screen, and no longer open: it takes no keyboard and
        // answers no dismissal while it is only finishing a departure.
        assert!(palette.is_rendered());
        assert!(!palette.is_open());
        assert_eq!(palette.phase(), Phase::Exiting);

        palette.advance(Duration::from_millis(1_000));
        assert_eq!(palette.phase(), Phase::Gone);
        assert!(!palette.is_rendered());
    }

    /// A departure interrupted by the reader opening the surface again plays
    /// back from where it had got to rather than restarting, which is the
    /// property the underlying `Presence` exists for.
    #[test]
    fn reopening_a_departing_surface_resumes_rather_than_restarts() {
        let theme = gpui_kit_theme::Theme::studio_dark();
        let mut palette = Presenting::opened(&theme, crate::motion::MotionRole::ModalEnter);
        assert!(palette.is_open());
        palette.close();
        palette.advance(Duration::from_millis(60));
        let partway = palette.progress();
        assert!(
            partway > 0.0 && partway < 1.0,
            "the exit did not get partway: {partway}"
        );

        palette.open();
        assert_eq!(palette.phase(), Phase::Entering);
        assert!((palette.progress() - partway).abs() < 0.05);
    }

    /// Every departure is the one exit, whatever the arrival was: a surface
    /// that took its entrance curve back out would decelerate on the way off
    /// screen, which reads as reluctance rather than dismissal.
    #[test]
    fn arrivals_differ_by_role_and_departures_do_not() {
        let theme = gpui_kit_theme::Theme::studio_dark();
        let exit = crate::motion::MotionPolicy::spec(crate::motion::MotionRole::Exit, &theme);
        for role in [
            crate::motion::MotionRole::MenuEnter,
            crate::motion::MotionRole::ModalEnter,
            crate::motion::MotionRole::Entrance,
        ] {
            let mut surface = Presenting::opened(&theme, role);
            assert_eq!(surface.role(), role);
            surface.close();
            surface.advance(exit.total());
            assert_eq!(surface.phase(), Phase::Gone, "{role:?} outlived the exit");
        }
    }

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

    /// Enter and exit on a curve that is nowhere near linear, so a reversal
    /// that assumed the two timelines were proportional would be caught.
    fn curved() -> Presence {
        let curve = CubicBezier::new(0.42, 0.0, 0.58, 1.0);
        Presence::hidden(MotionSpec::new(200, curve), MotionSpec::new(100, curve))
    }

    /// Runs the current phase out a millisecond at a time and reports how long
    /// it took.
    fn run_out(presence: &mut Presence) -> Duration {
        let mut elapsed = Duration::ZERO;
        while presence.is_animating() {
            presence.advance(Duration::from_millis(1));
            elapsed += Duration::from_millis(1);
        }
        elapsed
    }

    #[test]
    fn a_cancelled_entrance_leaves_from_the_opacity_it_reached() {
        let mut presence = curved();
        presence.show();
        presence.advance(Duration::from_millis(60));
        let interrupted = presence.progress();
        assert!(interrupted < 0.25, "the curve starts slowly: {interrupted}");

        presence.hide();
        assert!(
            (presence.progress() - interrupted).abs() < 0.01,
            "the element jumped from {interrupted} to {} on being cancelled",
            presence.progress()
        );
        let took = run_out(&mut presence);
        assert_eq!(presence.phase(), Phase::Gone);
        assert!(
            took < Duration::from_millis(40),
            "leaving from {interrupted} took {took:?} of a 100ms exit"
        );
    }

    #[test]
    fn a_cancelled_exit_comes_back_the_way_it_went() {
        let mut presence = curved();
        presence.show();
        presence.advance(Duration::from_millis(200));
        presence.hide();
        presence.advance(Duration::from_millis(40));
        let interrupted = presence.progress();

        presence.show();
        assert!(
            (presence.progress() - interrupted).abs() < 0.01,
            "the element jumped from {interrupted} to {}",
            presence.progress()
        );
        let took = run_out(&mut presence);
        assert_eq!(presence.phase(), Phase::Present);
        assert!(
            took < Duration::from_millis(200),
            "returning from {interrupted} took the whole {took:?} entrance"
        );
    }

    #[test]
    fn the_earlier_a_phase_is_cancelled_the_less_of_the_other_it_costs() {
        let took = |after_ms| {
            let mut presence = curved();
            presence.show();
            presence.advance(Duration::from_millis(after_ms));
            presence.hide();
            run_out(&mut presence)
        };
        assert!(
            took(40) < took(100) && took(100) < took(180),
            "{:?}, {:?}, {:?}",
            took(40),
            took(100),
            took(180)
        );
    }

    #[test]
    fn a_delayed_entrance_reverses_from_what_is_on_screen() {
        let linear = |ms| MotionSpec::new(ms, CubicBezier::new(0.0, 0.0, 1.0, 1.0));
        let mut presence = Presence::hidden(linear(100).with_delay(100), linear(100));
        presence.show();
        presence.advance(Duration::from_millis(150));
        let interrupted = presence.progress();
        assert!((interrupted - 0.5).abs() < 0.02);

        presence.hide();
        assert!((presence.progress() - interrupted).abs() < 0.02);
        let took = run_out(&mut presence);
        assert!(
            took.abs_diff(Duration::from_millis(50)) <= Duration::from_millis(2),
            "half an exit is 50ms, not {took:?}"
        );
    }

    #[test]
    fn a_sprung_entrance_reverses_from_what_is_on_screen() {
        let spring = MotionSpec::sprung(crate::motion::Spring::perceptual(
            Duration::from_millis(300),
            0.4,
        ));
        let mut presence = Presence::hidden(spring, spring);
        presence.show();
        presence.advance(Duration::from_millis(80));
        let interrupted = presence.progress();

        presence.hide();
        assert!(
            (presence.progress() - interrupted).abs() < 0.02,
            "a sprung entrance jumped from {interrupted} to {}",
            presence.progress()
        );
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
