//! Transient Kit state scoped to one GPUI window.
//!
//! `RenderOnce` builders need somewhere to retain measurements, scroll
//! handles, and motion between frames. The application owns that storage, but
//! a component identity is only unique inside its window. This helper keeps
//! every registry under [`WindowId`], ages keyed entries by semantic frame,
//! and removes the whole window entry when GPUI closes it.

use std::any::TypeId;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use gpui::{App, EffectOwner, Global, SharedString, WindowId};
use gpui_kit_semantics::SemanticCoordinator;

/// How many generations an untouched key survives.
///
/// One generation of slack prevents a frame boundary between two consecutive
/// renders from looking like an unmount. An entry last seen in generation N is
/// therefore removed when another key of the same state type is touched in
/// generation N + 2.
const KEY_GRACE: u64 = 2;

struct WindowStates<T>(RefCell<HashMap<WindowId, T>>);

impl<T> Default for WindowStates<T> {
    fn default() -> Self {
        Self(RefCell::new(HashMap::new()))
    }
}

impl<T: 'static> Global for WindowStates<T> {}

struct KeyedEntry<T> {
    seen: u64,
    grace: u64,
    value: T,
}

struct KeyedStates<T>(HashMap<Option<EffectOwner>, HashMap<SharedString, KeyedEntry<T>>>);

impl<T> Default for KeyedStates<T> {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

type ReleaseKeyed = fn(EffectOwner, &mut App) -> usize;

#[derive(Default)]
struct OwnerStates {
    release: HashMap<TypeId, ReleaseKeyed>,
    // Only an explicit host mount operation registers authority to retain state.
    // Ambient scopes and late callbacks never insert here.
    active: HashSet<EffectOwner>,
}

impl Global for OwnerStates {}

/// Registers a freshly minted mount owner before constructing its Kit state.
/// Only the host mount lifecycle calls this; never call it from a component,
/// render, or delayed callback. Preserve registration across ordinary renders,
/// release on unmount, and mint a new token for a subsequent mount.
/// This grants cache lifetime only, not clipboard or other effect permission.
pub fn register_owner_state(owner: EffectOwner, cx: &mut App) {
    if !cx.has_global::<OwnerStates>() {
        cx.set_global(OwnerStates::default());
    }
    cx.global_mut::<OwnerStates>().active.insert(owner);
}

/// Whether an owner may still retain Kit keyed state. This is cache lifetime,
/// not permission to perform effects. Native unowned state remains compatible.
pub fn owner_state_is_live(owner: EffectOwner, cx: &App) -> bool {
    cx.try_global::<OwnerStates>()
        .is_some_and(|state| state.active.contains(&owner))
}

/// Immediately drops all typed keyed-registry entries belonging to this exact
/// owner, across windows, and removes its registration. Ambient owner scopes
/// cannot recreate it; no retired-token tombstones are retained.
/// Returns the number of removed keys. No semantic-id parsing is involved.
///
/// Hosts use one owner per actual mount, preserve it across ordinary renders,
/// and retire it on removal. Typed children must preserve `EffectScoped<T>`.
/// This releases registry references, not arbitrary externally retained Rc or
/// Entity handles, and does not partition window-wide overlay coordination.
pub fn release_owner_state(owner: EffectOwner, cx: &mut App) -> usize {
    if !cx.has_global::<OwnerStates>() {
        return 0;
    }
    let callbacks = {
        let states = cx.global_mut::<OwnerStates>();
        if !states.active.remove(&owner) {
            return 0;
        }
        // Shrink geometrically, not once per removal; keep neither history nor
        // an unbounded high-water allocation after a large mount batch exits.
        if states.active.capacity() > states.active.len().saturating_mul(4) {
            states
                .active
                .shrink_to(states.active.len().saturating_mul(2));
        }
        states.release.values().copied().collect::<Vec<_>>()
    };
    callbacks
        .into_iter()
        .map(|release| release(owner, cx))
        .sum()
}

fn release_keyed<T: 'static>(owner: EffectOwner, cx: &mut App) -> usize {
    let Some(states) = cx.try_global::<WindowStates<KeyedStates<T>>>() else {
        return 0;
    };
    states
        .0
        .borrow_mut()
        .values_mut()
        .map(|keys| {
            let removed = keys
                .0
                .remove(&Some(owner))
                .map_or(0, |entries| entries.len());
            if keys.0.capacity() > keys.0.len().saturating_mul(4) {
                keys.0.shrink_to(keys.0.len().saturating_mul(2));
            }
            removed
        })
        .sum()
}

fn install<T: 'static>(cx: &mut App) {
    if cx.has_global::<WindowStates<T>>() {
        return;
    }
    cx.set_global(WindowStates::<T>::default());
    cx.on_window_closed(|cx, window_id| {
        if let Some(states) = cx.try_global::<WindowStates<T>>() {
            states.0.borrow_mut().remove(&window_id);
        }
    })
    .detach();
}

/// Mutates the state belonging to `window_id`, creating its default on first
/// use.
pub(crate) fn with<T: Default + 'static, R>(
    window_id: WindowId,
    cx: &mut App,
    update: impl FnOnce(&mut T) -> R,
) -> R {
    install::<T>(cx);
    let states = cx.global::<WindowStates<T>>();
    let mut states = states.0.borrow_mut();
    update(states.entry(window_id).or_default())
}

/// Reads the state of a known window without creating one.
pub(crate) fn read<T: 'static, R>(
    window_id: WindowId,
    cx: &App,
    read: impl FnOnce(&T) -> R,
) -> Option<R> {
    let states = cx.try_global::<WindowStates<T>>()?;
    let states = states.0.borrow();
    states.get(&window_id).map(read)
}

/// Mutates one identity's state and marks it live in this frame.
///
/// Pruning happens before the key is touched. It is deliberately lazy: if no
/// identity of a state type is used again, its backing allocation remains
/// dormant until that window closes; the next use discards all expired keys
/// before exposing state.
pub(crate) fn with_key<T: Default + 'static, R>(
    id: &SharedString,
    window_id: WindowId,
    cx: &mut App,
    update: impl FnOnce(&mut T) -> R,
) -> R {
    with_key_retained(id, KEY_GRACE, window_id, cx, update)
}

/// The keyed state operation with a caller-selected bounded handoff grace.
pub(crate) fn with_key_retained<T: Default + 'static, R>(
    id: &SharedString,
    grace: u64,
    window_id: WindowId,
    cx: &mut App,
    update: impl FnOnce(&mut T) -> R,
) -> R {
    let owner = cx.current_effect_owner();
    if owner.is_some_and(|owner| !owner_state_is_live(owner, cx)) {
        // Preserve the infallible builder contract without retaining stale
        // owner data or re-inserting it under the ambient/unowned namespace.
        return update(&mut T::default());
    }
    if !cx.has_global::<OwnerStates>() {
        cx.set_global(OwnerStates::default());
    }
    cx.global_mut::<OwnerStates>()
        .release
        .entry(TypeId::of::<T>())
        .or_insert(release_keyed::<T>);
    let generation = generation(window_id, cx);
    with(window_id, cx, |states: &mut KeyedStates<T>| {
        let keys = states.0.entry(owner).or_default();
        keys.retain(|_, entry| generation.saturating_sub(entry.seen) < entry.grace);
        let entry = keys.entry(id.clone()).or_insert_with(|| KeyedEntry {
            seen: generation,
            grace,
            value: T::default(),
        });
        entry.seen = generation;
        entry.grace = grace;
        update(&mut entry.value)
    })
}

/// Reads one identity without reviving or creating it.
pub(crate) fn read_key<T: 'static, R>(
    id: &SharedString,
    window_id: WindowId,
    cx: &App,
    read: impl FnOnce(&T) -> R,
) -> Option<R> {
    let owner = cx.current_effect_owner();
    self::read(window_id, cx, |states: &KeyedStates<T>| {
        states
            .0
            .get(&owner)?
            .get(id)
            .map(|entry| read(&entry.value))
    })
    .flatten()
}

pub(crate) fn keyed_ids<T: 'static>(window_id: WindowId, cx: &App) -> Vec<SharedString> {
    let owner = cx.current_effect_owner();
    self::read(window_id, cx, |states: &KeyedStates<T>| {
        let mut ids: Vec<_> = states
            .0
            .get(&owner)
            .into_iter()
            .flat_map(|keys| keys.keys().cloned())
            .collect();
        ids.sort();
        ids
    })
    .unwrap_or_default()
}

fn generation(window_id: WindowId, cx: &App) -> u64 {
    SemanticCoordinator::try_global(cx)
        .and_then(|coordinator| coordinator.generation(window_id))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use gpui::{
        AnyWindowHandle, AppContext, Context, IntoElement, Render, TestAppContext, Window, div,
    };

    use super::*;

    struct Fixture;

    #[derive(Default)]
    struct Remembered(usize);

    impl Render for Fixture {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[gpui::test]
    fn identical_local_keys_are_isolated_and_closed_windows_are_removed(cx: &mut TestAppContext) {
        let left = AnyWindowHandle::from(cx.add_window(|_, _| Fixture));
        let right = AnyWindowHandle::from(cx.add_window(|_, _| Fixture));

        cx.update(|cx| {
            with::<HashMap<&'static str, usize>, _>(left.window_id(), cx, |state| {
                state.insert("shared", 1);
            });
            with::<HashMap<&'static str, usize>, _>(right.window_id(), cx, |state| {
                state.insert("shared", 2);
            });
            let key = SharedString::new_static("keyed");
            with_key(&key, left.window_id(), cx, |state: &mut Remembered| {
                state.0 = 3
            });
            with_key(&key, right.window_id(), cx, |state: &mut Remembered| {
                state.0 = 4
            });
            assert_eq!(
                read(
                    left.window_id(),
                    cx,
                    |state: &HashMap<&'static str, usize>| state["shared"]
                ),
                Some(1)
            );
            assert_eq!(
                read(
                    right.window_id(),
                    cx,
                    |state: &HashMap<&'static str, usize>| state["shared"]
                ),
                Some(2)
            );
            assert_eq!(
                read_key(&key, left.window_id(), cx, |state: &Remembered| state.0),
                Some(3)
            );
            assert_eq!(
                read_key(&key, right.window_id(), cx, |state: &Remembered| state.0),
                Some(4)
            );
        });

        cx.update_window(left, |_, window, _| window.remove_window())
            .expect("left window");
        cx.run_until_parked();
        cx.update(|cx| {
            let key = SharedString::new_static("keyed");
            assert_eq!(
                read(left.window_id(), cx, |state: &HashMap<&str, usize>| state
                    .len()),
                None
            );
            assert_eq!(
                read(right.window_id(), cx, |state: &HashMap<&str, usize>| state
                    ["shared"]),
                Some(2)
            );
            assert_eq!(
                read_key(&key, left.window_id(), cx, |state: &Remembered| state.0),
                None
            );
            assert_eq!(
                read_key(&key, right.window_id(), cx, |state: &Remembered| state.0),
                Some(4)
            );
        });
    }

    #[gpui::test]
    fn owner_release_drops_every_typed_composite_key_without_touching_siblings(
        cx: &mut TestAppContext,
    ) {
        use std::rc::Rc;
        let owner = EffectOwner::new();
        let sibling = EffectOwner::new();
        let left = WindowId::from(31);
        let right = WindowId::from(32);
        cx.update(|cx| {
            register_owner_state(owner, cx);
            register_owner_state(sibling, cx);
            let composite: SharedString = "effect-particles:unrelated-prefix".into();
            let removed = cx.with_effect_owner(Some(owner), |cx| {
                with_key(&composite, left, cx, |state: &mut Rc<()>| {
                    Rc::downgrade(state)
                })
            });
            let kept = cx.with_effect_owner(Some(sibling), |cx| {
                with_key(&composite, left, cx, |state: &mut Rc<()>| {
                    Rc::downgrade(state)
                })
            });
            assert!(
                !removed.ptr_eq(&kept),
                "identical composite keys are owner-isolated"
            );
            cx.with_effect_owner(Some(owner), |cx| {
                with_key(
                    &"cinematic-effect:anything".into(),
                    right,
                    cx,
                    |state: &mut Remembered| state.0 = 19,
                );
                with_key(
                    &"canvas:measurement".into(),
                    left,
                    cx,
                    |state: &mut Vec<u8>| state.push(23),
                );
            });
            assert_eq!(release_owner_state(owner, cx), 3);
            assert_eq!(release_owner_state(owner, cx), 0, "release is idempotent");
            assert!(
                removed.upgrade().is_none(),
                "registry-only reference dropped immediately"
            );
            assert!(kept.upgrade().is_some());
            cx.with_effect_owner(Some(owner), |cx| {
                assert!(read_key(&composite, left, cx, |_: &Rc<()>| ()).is_none());
                let stale = with_key(&composite, left, cx, |state: &mut Rc<()>| {
                    Rc::downgrade(state)
                });
                assert!(
                    stale.upgrade().is_none(),
                    "retired access is transient, never recached"
                );
                assert!(keyed_ids::<Rc<()>>(left, cx).is_empty());
            });
            cx.with_effect_owner(Some(sibling), |cx| {
                assert_eq!(keyed_ids::<Rc<()>>(left, cx), vec![composite])
            });
            assert!(!owner_state_is_live(owner, cx));
            assert!(owner_state_is_live(sibling, cx));
        });
    }

    #[gpui::test]
    fn a_late_explicit_owner_continuation_cannot_repopulate_released_state(
        cx: &mut TestAppContext,
    ) {
        let owner = EffectOwner::new();
        let window = WindowId::from(41);
        cx.update(|cx| {
            register_owner_state(owner, cx);
            cx.with_effect_owner(Some(owner), |cx| {
                with_key(&"late".into(), window, cx, |state: &mut Remembered| {
                    state.0 = 17
                })
            });
            cx.defer(move |cx| {
                cx.with_effect_owner(Some(owner), |cx| {
                    with_key(&"late".into(), window, cx, |state: &mut Remembered| {
                        assert_eq!(state.0, 0);
                        state.0 = 29;
                    });
                    assert!(
                        read_key(&"late".into(), window, cx, |state: &Remembered| state.0)
                            .is_none()
                    );
                })
            });
            assert_eq!(release_owner_state(owner, cx), 1);
        });
    }

    #[gpui::test]
    fn owner_churn_retains_no_history_or_peak_table_with_a_live_peer(cx: &mut TestAppContext) {
        let window = WindowId::from(51);
        let key = SharedString::from("same-composite:key");
        cx.update(|cx| {
            let peer = EffectOwner::new();
            register_owner_state(peer, cx);
            cx.with_effect_owner(Some(peer), |cx| {
                with_key(&key, window, cx, |state: &mut Remembered| state.0 = 73);
            });
            for index in 0..100_000 {
                let owner = EffectOwner::new();
                assert!(!owner_state_is_live(owner, cx));
                cx.with_effect_owner(Some(owner), |cx| {
                    with_key(&key, window, cx, |state: &mut Remembered| state.0 = 91);
                    assert!(read_key(&key, window, cx, |_: &Remembered| ()).is_none());
                });
                register_owner_state(owner, cx);
                cx.with_effect_owner(Some(owner), |cx| {
                    with_key(&key, window, cx, |state: &mut Remembered| {
                        assert_eq!(state.0, 0);
                        state.0 = 19;
                    });
                });
                assert_eq!(release_owner_state(owner, cx), 1);
                cx.with_effect_owner(Some(owner), |cx| {
                    with_key(&key, window, cx, |state: &mut Remembered| state.0 = 97);
                    assert!(!owner_state_is_live(owner, cx));
                    assert!(read_key(&key, window, cx, |_: &Remembered| ()).is_none());
                });
                if index == 9_999 || index == 99_999 {
                    let active = &cx.global::<OwnerStates>().active;
                    let (groups, capacity) = read(window, cx, |keys: &KeyedStates<Remembered>| {
                        (keys.0.len(), keys.0.capacity())
                    }).expect("registered state type");
                    assert_eq!(active.len(), 1);
                    assert_eq!(groups, 1);
                    assert!(active.capacity() <= 4);
                    assert!(capacity <= 4);
                    println!("retirements={} peak_live=2 active_len={} active_capacity={} groups={} group_capacity={}", index + 1, active.len(), active.capacity(), groups, capacity);
                }
            }
            // A burst, unlike sequential churn, grows table capacity. Removal
            // must shrink that high water while preserving the live peer.
            let burst: Vec<_> = (0..10_000).map(|_| EffectOwner::new()).collect();
            for &owner in &burst {
                register_owner_state(owner, cx);
                cx.with_effect_owner(Some(owner), |cx| {
                    with_key(&key, window, cx, |_: &mut Remembered| ());
                });
            }
            for owner in burst { release_owner_state(owner, cx); }
            assert!(cx.global::<OwnerStates>().active.capacity() <= 4);
            assert!(read(window, cx, |keys: &KeyedStates<Remembered>| keys.0.capacity()).expect("state") <= 4);
            cx.with_effect_owner(Some(peer), |cx| {
                assert_eq!(read_key(&key, window, cx, |state: &Remembered| state.0), Some(73));
            });
            assert_eq!(release_owner_state(peer, cx), 1);
            assert_eq!(cx.global::<OwnerStates>().active.capacity(), 0);
            assert_eq!(read(window, cx, |keys: &KeyedStates<Remembered>| keys.0.capacity()), Some(0));
        });
    }

    #[gpui::test]
    fn keyed_state_keeps_live_ids_and_reclaims_missing_ids(cx: &mut TestAppContext) {
        let window_id = WindowId::from(1);
        let kept = SharedString::new_static("kept");
        let removed = SharedString::new_static("removed");

        cx.update(|cx| {
            gpui_kit_semantics::install(cx);
            let coordinator = SemanticCoordinator::global(cx);
            coordinator.begin_window_frame(window_id);
            with_key(&kept, window_id, cx, |state: &mut Remembered| state.0 = 1);
            with_key(&removed, window_id, cx, |state: &mut Remembered| {
                state.0 = 2
            });

            coordinator.begin_window_frame(window_id);
            with_key(&kept, window_id, cx, |state: &mut Remembered| {
                assert_eq!(state.0, 1)
            });
            assert_eq!(
                keyed_ids::<Remembered>(window_id, cx),
                vec![kept.clone(), removed.clone()]
            );

            coordinator.begin_window_frame(window_id);
            with_key(&kept, window_id, cx, |state: &mut Remembered| {
                assert_eq!(state.0, 1)
            });
            assert_eq!(keyed_ids::<Remembered>(window_id, cx), vec![kept]);

            with_key(&removed, window_id, cx, |state: &mut Remembered| {
                assert_eq!(state.0, 0, "an expired identity returns with fresh state")
            });
        });
    }
}
