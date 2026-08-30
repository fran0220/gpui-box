//! Semantic motion policy shared by every component.
//!
//! Components name why something moves with [`MotionRole`]. This module is
//! the only place that turns that reason into a duration, curve, spring, and
//! reduced-motion answer. A component may choose a role; it must not invent a
//! second timing vocabulary beside the theme.

use gpui::App;
use gpui_kit_theme::{ActiveTheme, SpringPreset, Theme};

use super::{Activity, Easing, Micro, MotionSpec, Spring};

/// Why a surface is moving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionRole {
    /// Content becomes part of the current surface.
    Entrance,
    /// A pointer-owned menu answers the action that opened it.
    MenuEnter,
    /// A modal surface arrives with physical weight.
    ModalEnter,
    /// Content changes inside a modal surface.
    ModalTransition,
    /// A surface leaves after dismissal.
    Exit,
    /// A control answers a local state change.
    StateChange,
    /// A measured value or region changes extent.
    Resize,
    /// A value follows a pointer or another continuously moving target.
    Tracking,
    /// The reader travels from one location to another.
    Navigation,
    /// Newly arriving content settles without delaying its publication.
    Streaming,
    /// A one-shot visual response reports an outcome or handoff.
    Feedback,
    /// A deliberately prominent one-shot response marks a reward.
    Celebration,
    /// Continuous work whose truth is described by [`Activity`].
    Activity(Activity),
    /// A named procedural reaction.
    Micro(Micro),
}

/// What the resolved policy permits the renderer to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionDisposition {
    /// Run the resolved timeline.
    Animate,
    /// Publish and paint the settled endpoint immediately.
    Settle,
    /// Do not start a timeline; the component's static state carries meaning.
    Suppress,
    /// Paint the component's policy-owned representative frame.
    Poster,
}

/// One role resolved against a theme and the user's motion preference.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedMotion {
    role: MotionRole,
    spec: MotionSpec,
    disposition: MotionDisposition,
}

impl ResolvedMotion {
    pub const fn role(self) -> MotionRole {
        self.role
    }

    pub const fn spec(self) -> MotionSpec {
        self.spec
    }

    pub const fn disposition(self) -> MotionDisposition {
        self.disposition
    }

    pub const fn animates(self) -> bool {
        matches!(self.disposition, MotionDisposition::Animate)
    }
}

/// The sole mapping from semantic roles to motion tokens and preferences.
#[derive(Debug, Clone, Copy, Default)]
pub struct MotionPolicy;

impl MotionPolicy {
    /// Resolves a role using the active theme and platform preference.
    pub fn resolve(role: MotionRole, cx: &App) -> ResolvedMotion {
        Self::resolve_for(role, cx.theme(), cx.reduce_motion())
    }

    /// Resolves a role against an explicit theme and preference.
    ///
    /// This is useful to state policy before a render context exists and to
    /// test both preference branches without constructing a window.
    pub fn resolve_for(role: MotionRole, theme: &Theme, reduce_motion: bool) -> ResolvedMotion {
        let disposition = if reduce_motion {
            reduced_disposition(role)
        } else {
            MotionDisposition::Animate
        };
        ResolvedMotion {
            role,
            spec: Self::spec(role, theme),
            disposition,
        }
    }

    /// Resolves only the tokenized timeline for code whose reduced-motion
    /// branch is handled by [`Transition`](super::Transition),
    /// [`Presence`](super::Presence), or the caller's static presentation.
    pub fn spec(role: MotionRole, theme: &Theme) -> MotionSpec {
        match role {
            MotionRole::Entrance => {
                MotionSpec::new(theme.motion.entrance_ms, Easing::Settle.curve(theme))
            }
            MotionRole::MenuEnter => {
                MotionSpec::new(theme.motion.menu_ms, Easing::Standard.curve(theme))
            }
            MotionRole::ModalEnter => {
                MotionSpec::sprung(Spring::preset(theme, SpringPreset::Smooth))
            }
            MotionRole::ModalTransition => {
                MotionSpec::new(theme.motion.dialog_ms, Easing::Standard.curve(theme))
            }
            // Leaving is not information. An arrival has to be read; a
            // departure has already been decided by the reader, so it is
            // shorter than any arrival and it accelerates away rather than
            // decelerating into place.
            MotionRole::Exit => MotionSpec::new(theme.motion.exit_ms, Easing::Exit.curve(theme)),
            MotionRole::StateChange => {
                MotionSpec::new(theme.motion.quick_ms, Easing::Standard.curve(theme))
            }
            MotionRole::Resize => {
                MotionSpec::new(theme.motion.resize_ms, Easing::Standard.curve(theme))
            }
            MotionRole::Tracking => MotionSpec::sprung(Spring::preset(theme, SpringPreset::Grab)),
            MotionRole::Navigation => {
                MotionSpec::new(theme.motion.entrance_ms, Easing::EaseInOut.curve(theme))
            }
            MotionRole::Streaming => {
                MotionSpec::new(theme.motion.resize_ms, Easing::EaseOut.curve(theme))
            }
            MotionRole::Feedback => {
                MotionSpec::new(theme.motion.feedback_ms, Easing::Settle.curve(theme))
            }
            MotionRole::Celebration => {
                MotionSpec::new(theme.motion.celebration_ms, Easing::Emphasized.curve(theme))
            }
            MotionRole::Activity(activity) => {
                MotionSpec::new(activity.period_ms(theme), activity.curve(theme))
            }
            MotionRole::Micro(kind) => {
                let duration_ms = match kind {
                    Micro::Heartbeat => theme.motion.pulse_ms,
                    Micro::Bounce => theme.motion.micro_bounce_ms,
                    Micro::Wobble => theme.motion.micro_wobble_ms,
                    Micro::Pop => theme.motion.micro_pop_ms,
                    Micro::Sparkle => theme.motion.shimmer_ms,
                };
                MotionSpec::new(duration_ms, Easing::EaseInOut.curve(theme))
            }
        }
    }

    /// Retimes a streaming fade from observed arrival cadence while keeping
    /// every duration inside the shared role vocabulary.
    pub(crate) fn streaming_timing(
        theme: &Theme,
        previous_cadence_ms: Option<f32>,
        observed_gap_ms: Option<f32>,
        backlog: usize,
    ) -> (f32, f32) {
        const CADENCE_WEIGHT: f32 = 0.3;
        const OVERLAP: f32 = 3.0;
        const BACKLOG: usize = 3;
        const BACKLOG_SPEEDUP: f32 = 0.75;

        let seed = Self::spec(MotionRole::Streaming, theme).duration_ms as f32;
        let gap_limit = Self::spec(MotionRole::Feedback, theme).duration_ms as f32;
        let gap = observed_gap_ms.unwrap_or(seed).min(gap_limit);
        let cadence = previous_cadence_ms
            .map(|previous| previous + CADENCE_WEIGHT * (gap - previous))
            .unwrap_or(gap);
        let minimum = Self::spec(MotionRole::MenuEnter, theme).duration_ms as f32;
        let maximum = Self::spec(MotionRole::Entrance, theme).duration_ms as f32;
        let mut duration = (cadence * OVERLAP).clamp(minimum, maximum);
        if backlog >= BACKLOG {
            duration *= BACKLOG_SPEEDUP;
        }
        (cadence, duration)
    }
}

fn reduced_disposition(role: MotionRole) -> MotionDisposition {
    match role {
        MotionRole::Activity(_) | MotionRole::Streaming => MotionDisposition::Suppress,
        MotionRole::Feedback | MotionRole::Celebration => MotionDisposition::Poster,
        MotionRole::Micro(Micro::Heartbeat | Micro::Sparkle) => MotionDisposition::Suppress,
        MotionRole::Micro(Micro::Bounce | Micro::Wobble | Micro::Pop)
        | MotionRole::Entrance
        | MotionRole::MenuEnter
        | MotionRole::ModalEnter
        | MotionRole::ModalTransition
        | MotionRole::Exit
        | MotionRole::StateChange
        | MotionRole::Resize
        | MotionRole::Tracking
        | MotionRole::Navigation => MotionDisposition::Settle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme::studio_dark()
    }

    #[test]
    fn semantic_roles_resolve_to_their_token_authority() {
        let theme = theme();
        assert_eq!(
            MotionPolicy::spec(MotionRole::Feedback, &theme).duration_ms,
            theme.motion.feedback_ms
        );
        assert_eq!(
            MotionPolicy::spec(MotionRole::Celebration, &theme).duration_ms,
            theme.motion.celebration_ms
        );
        assert!(MotionPolicy::spec(MotionRole::ModalEnter, &theme).is_sprung());
        assert!(MotionPolicy::spec(MotionRole::Tracking, &theme).is_sprung());
    }

    /// Leaving is not information. An arrival has to be read — it says what
    /// has appeared and where it came from — while a departure has already
    /// been decided by the reader, and every millisecond of it is a
    /// millisecond they wait for the thing they asked to go to be gone.
    ///
    /// The exit used to be `quick`, which is also the state-change duration,
    /// on the ease-out curve — so a menu took longer to leave than to open and
    /// slowed down on the way out.
    #[test]
    fn a_departure_is_shorter_than_any_arrival_and_does_not_linger() {
        let theme = theme();
        let exit = MotionPolicy::spec(MotionRole::Exit, &theme);
        for arrival in [
            MotionRole::MenuEnter,
            MotionRole::ModalTransition,
            MotionRole::Entrance,
            MotionRole::Navigation,
        ] {
            assert!(
                exit.duration_ms <= MotionPolicy::spec(arrival, &theme).duration_ms,
                "leaving takes longer than {arrival:?}"
            );
        }
        // Behind its own clock halfway through: it holds briefly and then
        // goes, rather than decelerating into being absent.
        assert!(exit.progress(0.5) < 0.5);
        assert!(MotionPolicy::spec(MotionRole::Entrance, &theme).progress(0.5) > 0.5);
    }

    #[test]
    fn reduced_motion_distinguishes_settled_suppressed_and_poster_states() {
        let theme = theme();
        assert_eq!(
            MotionPolicy::resolve_for(MotionRole::StateChange, &theme, true).disposition(),
            MotionDisposition::Settle
        );
        assert_eq!(
            MotionPolicy::resolve_for(MotionRole::Activity(Activity::Working), &theme, true)
                .disposition(),
            MotionDisposition::Suppress
        );
        assert_eq!(
            MotionPolicy::resolve_for(MotionRole::Feedback, &theme, true).disposition(),
            MotionDisposition::Poster
        );
    }

    #[test]
    fn streaming_cadence_stays_inside_semantic_role_bounds() {
        let theme = theme();
        let (_, fast) = MotionPolicy::streaming_timing(&theme, None, Some(1.0), 0);
        let (_, slow) = MotionPolicy::streaming_timing(&theme, None, Some(9_000.0), 0);
        assert_eq!(
            fast,
            MotionPolicy::spec(MotionRole::MenuEnter, &theme).duration_ms as f32
        );
        assert_eq!(
            slow,
            MotionPolicy::spec(MotionRole::Entrance, &theme).duration_ms as f32
        );
    }
}
