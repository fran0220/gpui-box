//! Controlled raw-coordinate range editing shared by visualization families.
//!
//! This module owns gesture state, not data, pixels, capture, labels or actions.
//! A family adapts its existing invertible scale and passes pointer fractions.
//! Start/End retain ascending raw endpoint identity, even when Start projects
//! to the right of End. Input raw ranges must be ascending. Zero-width
//! ranges are valid. Resize clamps at the other endpoint without swapping IDs.
//! Move preserves projected width, including on log scales. No scalar scale
//! arithmetic is duplicated here, and no raw value passes through f32.
//!
//! Adapters must call `sync` on controlled redraws, route capture cancellation
//! to `cancel`, and cancel/release their owned pointer capture when disabling
//! the strip. Disabled adapters install no action handlers. Arbitrary unmount
//! is handled by GPUI: it cancels disappearing capture owners through their
//! previous frame's cancellation listeners. The adapter's listener must clear
//! this draft. This pure state machine does not own GPUI capture cleanup or IDs.

/// An existing monotonic, invertible coordinate mapping. Freeze it for the gesture.
/// `unproject(0)` and `unproject(1)` are finite distinct raw domain endpoints;
/// they may descend. `project` must preserve that orientation and `unproject`
/// must invert it within f64 representation. Out-of-domain caller ranges are
/// rejected at begin, not silently normalized. No method mutates caller data.
/// Equality must change when its domain or coordinate interpretation changes.
pub trait RangeMapping: Clone + PartialEq {
    fn project(&self, value: f64) -> Option<f64>;
    fn unproject(&self, fraction: f64) -> Option<f64>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeTarget {
    Start,
    End,
    Window,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeIntent {
    Create,
    Resize(RangeTarget),
    Move,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RangeKey {
    /// Signed display-fraction step, chosen by the caller's keyboard policy.
    Step(f64),
    Home,
    End,
}

/// These are proposals only. Cancellation never proposes a rollback: earlier
/// updates may already have been accepted by a controlled host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RangeEvent {
    Update {
        intent: RangeIntent,
        value: [f64; 2],
    },
    Commit {
        intent: RangeIntent,
        value: [f64; 2],
    },
    Cancel {
        intent: RangeIntent,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeError {
    NonFinite,
    MissingRange,
    InvalidRange,
    InvalidIntent,
    Unrepresentable,
}

struct Drag<S> {
    mapping: S,
    intent: RangeIntent,
    anchor: f64,
    original: Option<[f64; 2]>,
    fractions: [f64; 2],
    accepted: Option<[f64; 2]>,
    proposal: [f64; 2],
}

/// Retain this transient state alongside a caller-controlled range. The host
/// remains authoritative; only [`Self::preview`] holds an unaccepted proposal.
pub struct RangeInteraction<S> {
    drag: Option<Drag<S>>,
}

impl<S> Default for RangeInteraction<S> {
    fn default() -> Self {
        Self { drag: None }
    }
}

fn finite(value: f64) -> Result<f64, RangeError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(RangeError::NonFinite)
}

fn fractions<S: RangeMapping>(mapping: &S, range: [f64; 2]) -> Result<[f64; 2], RangeError> {
    let domain = [
        finite(mapping.unproject(0.).ok_or(RangeError::Unrepresentable)?)?,
        finite(mapping.unproject(1.).ok_or(RangeError::Unrepresentable)?)?,
    ];
    if domain[0] == domain[1] {
        return Err(RangeError::Unrepresentable);
    }
    if range[0] > range[1]
        || range[0] < domain[0].min(domain[1])
        || range[1] > domain[0].max(domain[1])
    {
        return Err(RangeError::InvalidRange);
    }
    let mut result = [0.; 2];
    for i in 0..2 {
        finite(range[i])?;
        result[i] = finite(
            mapping
                .project(range[i])
                .ok_or(RangeError::Unrepresentable)?,
        )?;
    }
    if result.iter().any(|v| !(0.0..=1.0).contains(v)) {
        return Err(RangeError::InvalidRange);
    }
    Ok(result)
}

fn resolve<S: RangeMapping>(
    mapping: &S,
    desired: [f64; 2],
    original: Option<([f64; 2], [f64; 2])>,
) -> Result<[f64; 2], RangeError> {
    let mut result = [0.; 2];
    for i in 0..2 {
        // Do not round-trip an untouched endpoint through projection. Raw
        // source distinctions can be finer than representable screen fractions.
        result[i] = match original {
            Some((raw, projected)) if projected[i] == desired[i] => raw[i],
            Some((raw, projected)) if projected[1 - i] == desired[i] => raw[1 - i],
            _ => finite(
                mapping
                    .unproject(desired[i])
                    .ok_or(RangeError::Unrepresentable)?,
            )?,
        };
    }
    Ok(result)
}

impl<S: RangeMapping> RangeInteraction<S> {
    /// Begin at a finite pointer fraction, possibly outside the viewport.
    /// Invalid input preserves any previous gesture. A valid begin replaces it.
    pub fn begin(
        &mut self,
        mapping: S,
        value: Option<[f64; 2]>,
        intent: RangeIntent,
        pointer: f64,
    ) -> Result<RangeEvent, RangeError> {
        finite(pointer)?;
        let projected = value.map(|value| fractions(&mapping, value)).transpose()?;
        if matches!(intent, RangeIntent::Resize(RangeTarget::Window)) {
            return Err(RangeError::InvalidIntent);
        }
        if intent != RangeIntent::Create && value.is_none() {
            return Err(RangeError::MissingRange);
        }
        let anchor = if intent == RangeIntent::Create {
            pointer.clamp(0., 1.)
        } else {
            pointer
        };
        let proposal = if intent == RangeIntent::Create {
            resolve(&mapping, [anchor; 2], None)?
        } else {
            value.ok_or(RangeError::MissingRange)?
        };
        self.drag = Some(Drag {
            mapping,
            intent,
            anchor,
            original: value,
            fractions: projected.unwrap_or([anchor; 2]),
            accepted: value,
            proposal,
        });
        Ok(RangeEvent::Update {
            intent,
            value: proposal,
        })
    }

    /// Reconcile a render's authoritative input before handling more events.
    /// Unchanged caller values mean no observed acceptance yet. Accepting the latest
    /// proposal does not rebase the gesture, avoiding cumulative drag drift.
    /// An unrelated replacement or changed mapping cancels transient state.
    /// Acceptance is synchronous/latest-value: a delayed older proposal is an
    /// external replacement and cancels, never silently rewrites the draft.
    pub fn sync(&mut self, mapping: &S, value: Option<[f64; 2]>) -> Option<RangeEvent> {
        let drag = self.drag.as_mut()?;
        if mapping != &drag.mapping || (value != drag.accepted && value != Some(drag.proposal)) {
            return self.cancel();
        }
        drag.accepted = value;
        None
    }

    pub fn preview(&self) -> Option<[f64; 2]> {
        self.drag.as_ref().map(|drag| drag.proposal)
    }

    /// Update from a finite fraction. Bounding belongs here, not to the input
    /// harness: captured movement/release can be outside the window.
    /// Resize preserves the initial pointer-to-handle offset, so a wide handle
    /// does not jump to the grabbed pixel on the first movement.
    pub fn update(&mut self, pointer: f64) -> Result<Option<RangeEvent>, RangeError> {
        finite(pointer)?;
        let Some(drag) = self.drag.as_mut() else {
            return Ok(None);
        };
        let [a, b] = drag.fractions;
        let movement = pointer - drag.anchor;
        let ascending = drag
            .mapping
            .unproject(0.)
            .ok_or(RangeError::Unrepresentable)?
            < drag
                .mapping
                .unproject(1.)
                .ok_or(RangeError::Unrepresentable)?;
        let desired = match drag.intent {
            RangeIntent::Create => [
                drag.anchor.min(pointer.clamp(0., 1.)),
                drag.anchor.max(pointer.clamp(0., 1.)),
            ],
            RangeIntent::Resize(RangeTarget::Start) => [
                if ascending {
                    (a + movement).clamp(0., b)
                } else {
                    (a + movement).clamp(b, 1.)
                },
                b,
            ],
            RangeIntent::Resize(RangeTarget::End) => [
                a,
                if ascending {
                    (b + movement).clamp(a, 1.)
                } else {
                    (b + movement).clamp(0., a)
                },
            ],
            RangeIntent::Move => {
                let delta = movement.clamp(-a.min(b), 1. - a.max(b));
                [a + delta, b + delta]
            }
            RangeIntent::Resize(RangeTarget::Window) => return Err(RangeError::InvalidIntent),
        };
        let original = if drag.intent == RangeIntent::Create {
            None
        } else {
            drag.original.map(|raw| (raw, drag.fractions))
        };
        let mut proposal = resolve(&drag.mapping, desired, original)?;
        if drag.intent == RangeIntent::Create && proposal[0] > proposal[1] {
            proposal.swap(0, 1);
        }
        drag.proposal = proposal;
        Ok(Some(RangeEvent::Update {
            intent: drag.intent,
            value: proposal,
        }))
    }

    /// Resolve the actual release coordinate, then discard all transient state.
    /// The returned commit is not installed until the caller accepts it.
    pub fn release(&mut self, pointer: f64) -> Result<Option<RangeEvent>, RangeError> {
        self.update(pointer)?;
        Ok(self.drag.take().map(|drag| RangeEvent::Commit {
            intent: drag.intent,
            value: drag.proposal,
        }))
    }

    /// Platform cancellation/Escape, not a synthetic release. Idempotent.
    pub fn cancel(&mut self) -> Option<RangeEvent> {
        self.drag.take().map(|drag| RangeEvent::Cancel {
            intent: drag.intent,
        })
    }

    /// A discrete keyboard edit is a commit proposal. It supersedes an active
    /// pointer preview; it never installs the proposed range. Tab/focus and
    /// direction-to-step conversion belong to the family adapter.
    pub fn keyboard(
        &mut self,
        mapping: &S,
        value: [f64; 2],
        target: RangeTarget,
        key: RangeKey,
    ) -> Result<RangeEvent, RangeError> {
        let [a, b] = fractions(mapping, value)?;
        let ascending = mapping.unproject(0.).ok_or(RangeError::Unrepresentable)?
            < mapping.unproject(1.).ok_or(RangeError::Unrepresentable)?;
        let position = match target {
            RangeTarget::Start | RangeTarget::Window => a,
            RangeTarget::End => b,
        };
        let requested = match key {
            RangeKey::Step(step) => position + finite(step)?,
            RangeKey::Home => 0.,
            RangeKey::End => 1.,
        };
        let desired = match target {
            RangeTarget::Start => [
                if ascending {
                    requested.clamp(0., b)
                } else {
                    requested.clamp(b, 1.)
                },
                b,
            ],
            RangeTarget::End => [
                a,
                if ascending {
                    requested.clamp(a, 1.)
                } else {
                    requested.clamp(0., a)
                },
            ],
            RangeTarget::Window => {
                let delta = (requested - a).clamp(-a.min(b), 1. - a.max(b));
                [a + delta, b + delta]
            }
        };
        let proposal = resolve(mapping, desired, Some((value, [a, b])))?;
        self.drag = None;
        let intent = if target == RangeTarget::Window {
            RangeIntent::Move
        } else {
            RangeIntent::Resize(target)
        };
        Ok(RangeEvent::Commit {
            intent,
            value: proposal,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::chart::scale::{NumericScale, ScaleKind};
    #[derive(Clone, PartialEq)]
    struct Scale(NumericScale);
    impl RangeMapping for Scale {
        fn project(&self, value: f64) -> Option<f64> {
            self.0.map(value)
        }
        fn unproject(&self, value: f64) -> Option<f64> {
            self.0.invert(value)
        }
    }
    fn scale(domain: [f64; 2]) -> Scale {
        Scale(NumericScale::new(ScaleKind::Linear, domain).expect("fixture domain"))
    }
    fn values(event: Option<RangeEvent>) -> [f64; 2] {
        match event.expect("proposal") {
            RangeEvent::Update { value, .. } | RangeEvent::Commit { value, .. } => value,
            _ => panic!("value proposal"),
        }
    }

    #[gpui::test]
    fn disabling_cancels_explicitly_but_unmount_uses_framework_cancellation(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::{IntoElement, MouseButton, div, prelude::*, px};
        use gpui_kit_semantics::{NodeSpec, Role, Semantic};
        use gpui_kit_testkit::harness::Harness;
        use std::{
            cell::{Cell, RefCell},
            rc::Rc,
        };
        let mode = Rc::new(Cell::new(0));
        let edit = Rc::new(RefCell::new(RangeInteraction::default()));
        let rendering = edit.clone();
        let state = mode.clone();
        let mut harness = Harness::new(cx, crate::install, move |window, cx| {
            let mode = state.get();
            if mode == 1 && rendering.borrow_mut().cancel().is_some() {
                window.release_pointer();
            }
            if mode == 2 {
                // The owner is gone: no adapter pre-cancel or release here.
                return div().into_any_element();
            }
            let down = rendering.clone();
            let cancel = rendering.clone();
            div()
                .id("range-owner")
                .w(px(200.))
                .h(px(40.))
                .when(mode == 0, |el| {
                    el.on_mouse_down_with_pointer_capture(MouseButton::Left, move |_, _, _| {
                        down.borrow_mut()
                            .begin(scale([0., 100.]), Some([23., 61.]), RangeIntent::Move, 0.5)
                            .expect("adapter begin");
                    })
                })
                .child(crate::interaction::on_pointer_cancel(move |_, _| {
                    cancel.borrow_mut().cancel();
                }))
                .semantic_in(
                    cx,
                    NodeSpec::new("range-owner", Role::Status).text("Range fixture"),
                )
                .into_any_element()
        });
        for next in [1, 2] {
            mode.set(0);
            harness.frame();
            harness.drag_start("range-owner");
            assert!(harness.update(|w, _| w.captured_hitbox().is_some()));
            assert!(edit.borrow().preview().is_some());
            mode.set(next);
            harness.frame();
            assert!(!harness.update(|w, _| w.captured_hitbox().is_some()));
            assert!(edit.borrow().preview().is_none());
            harness.drop_here();
            if next == 1 {
                harness.drag_start("range-owner");
                assert!(
                    !harness.update(|w, _| w.captured_hitbox().is_some()),
                    "disabled strip has no press handler"
                );
                harness.drop_here();
            }
        }
    }

    #[test]
    fn accepted_and_refused_redraws_never_rebase_and_cancel_never_rolls_back() {
        let s = scale([0., 100.]);
        let mut interaction = RangeInteraction::default();
        let original = Some([23., 61.]);
        interaction
            .begin(s.clone(), original, RangeIntent::Move, 0.4)
            .expect("begin");
        let first = values(interaction.update(0.5).expect("first update"));
        assert!((first[0] - 33.).abs() < 1e-12 && (first[1] - 71.).abs() < 1e-12);
        assert_eq!(interaction.sync(&s, original), None);
        assert_eq!(
            values(interaction.update(0.5).expect("duplicate update")),
            first
        );
        assert_eq!(interaction.sync(&s, Some(first)), None);
        assert_eq!(interaction.sync(&s, Some(first)), None);
        let next = values(interaction.update(0.6).expect("move"));
        assert!((next[0] - 43.).abs() < 1e-12 && (next[1] - 81.).abs() < 1e-12);
        assert_eq!(
            interaction.cancel(),
            Some(RangeEvent::Cancel {
                intent: RangeIntent::Move
            })
        );
        assert_eq!(interaction.preview(), None);
        assert_eq!(interaction.release(0.9), Ok(None));
        assert_eq!(interaction.cancel(), None);
    }

    #[test]
    fn external_replacement_and_mapping_change_cancel_but_latest_acceptance_does_not() {
        let s = scale([0., 100.]);
        let mut edit = RangeInteraction::default();
        for external in [Some([9., 72.]), None] {
            edit.begin(s.clone(), Some([23., 61.]), RangeIntent::Move, 0.4)
                .expect("begin");
            assert_eq!(
                edit.sync(&s, external),
                Some(RangeEvent::Cancel {
                    intent: RangeIntent::Move
                })
            );
        }
        edit.begin(s, Some([23., 61.]), RangeIntent::Move, 0.4)
            .expect("begin");
        assert_eq!(
            edit.sync(&scale([0., 200.]), Some([23., 61.])),
            Some(RangeEvent::Cancel {
                intent: RangeIntent::Move
            })
        );
        let s = scale([0., 100.]);
        edit.begin(s.clone(), Some([23., 61.]), RangeIntent::Move, 0.4)
            .expect("begin");
        let older = values(edit.update(0.5).expect("first proposal"));
        edit.update(0.6).expect("newer proposal");
        assert_eq!(
            edit.sync(&s, Some(older)),
            Some(RangeEvent::Cancel {
                intent: RangeIntent::Move
            }),
            "delayed old acceptance is external replacement"
        );
    }

    #[test]
    fn outside_release_creation_and_resize_keep_endpoint_identity() {
        let s = scale([100., 0.]);
        let mut edit = RangeInteraction::default();
        edit.begin(s.clone(), None, RangeIntent::Create, 0.73)
            .expect("create");
        assert_eq!(values(edit.release(-13.).expect("create")), [27., 100.]);
        edit.begin(
            s.clone(),
            Some([37., 81.]),
            RangeIntent::Resize(RangeTarget::Start),
            0.63,
        )
        .expect("resize");
        assert_eq!(values(edit.release(-2.).expect("crossing")), [81., 81.]);
        edit.begin(s, Some([37., 81.]), RangeIntent::Move, 0.3)
            .expect("move");
        let range = values(edit.release(900.).expect("outside"));
        assert!((range[1] - 44.).abs() < 1e-12);
        assert_eq!(range[0], 0.);
    }

    #[test]
    fn keyboard_uses_display_order_and_preserves_exact_unedited_epoch_endpoint() {
        let epoch = 1_700_000_000_000.;
        let s = scale([epoch, epoch + 1000.]);
        let raw = [epoch + 17., epoch + 681.];
        let mut edit = RangeInteraction::default();
        assert_eq!(
            values(Some(
                edit.keyboard(&s, raw, RangeTarget::Start, RangeKey::Step(0.1))
                    .expect("step")
            )),
            [epoch + 117., epoch + 681.]
        );
        assert_eq!(
            values(Some(
                edit.keyboard(&s, raw, RangeTarget::End, RangeKey::Home)
                    .expect("home")
            )),
            [epoch + 17.; 2]
        );
        assert_eq!(
            values(Some(
                edit.keyboard(&s, raw, RangeTarget::Window, RangeKey::End)
                    .expect("end")
            )),
            [epoch + 336., epoch + 1000.]
        );
        let reverse = scale([100., 0.]);
        assert_eq!(
            values(Some(
                edit.keyboard(&reverse, [37., 81.], RangeTarget::End, RangeKey::Home)
                    .expect("reversed home")
            )),
            [37., 100.]
        );
        let reversed_epoch = scale([epoch + 1000., epoch]);
        edit.begin(
            reversed_epoch,
            Some(raw),
            RangeIntent::Resize(RangeTarget::Start),
            0.983,
        )
        .expect("reversed epoch resize");
        let resized = values(edit.release(0.883).expect("resize"));
        assert_eq!(resized, [epoch + 117., epoch + 681.]);
    }

    #[test]
    fn log_move_preserves_projected_width_and_noop_keeps_unresolvable_source_precision() {
        let s = Scale(NumericScale::new(ScaleKind::Log, [1., 1000.]).expect("log domain"));
        let mut edit = RangeInteraction::default();
        edit.begin(s, Some([10., 100.]), RangeIntent::Move, 0.5)
            .expect("begin");
        let range = values(edit.release(5.).expect("move"));
        assert!((range[0] - 100.).abs() < 1e-10);
        assert_eq!(range[1], 1000.);
        let s = scale([0., 1e300]);
        let raw = [1e-300, 2e-300];
        edit.begin(s.clone(), Some(raw), RangeIntent::Move, 0.5)
            .expect("begin");
        assert_eq!(values(edit.release(0.5).expect("noop release")), raw);
        assert_eq!(
            values(Some(
                edit.keyboard(&s, raw, RangeTarget::End, RangeKey::Step(0.))
                    .expect("noop key")
            )),
            raw
        );
    }

    #[test]
    fn invalid_input_cannot_damage_an_active_gesture() {
        let s = scale([0., 100.]);
        let mut edit = RangeInteraction::default();
        edit.begin(s.clone(), Some([23., 61.]), RangeIntent::Move, 0.4)
            .expect("begin");
        assert_eq!(edit.update(f64::NAN), Err(RangeError::NonFinite));
        assert_eq!(
            edit.begin(s.clone(), Some([61., 23.]), RangeIntent::Move, 0.4),
            Err(RangeError::InvalidRange)
        );
        assert_eq!(
            edit.begin(s.clone(), Some([-3., 61.]), RangeIntent::Move, 0.4),
            Err(RangeError::InvalidRange)
        );
        assert_eq!(
            edit.begin(s, None, RangeIntent::Move, 0.4),
            Err(RangeError::MissingRange)
        );
        assert_eq!(edit.preview(), Some([23., 61.]));
    }
}
