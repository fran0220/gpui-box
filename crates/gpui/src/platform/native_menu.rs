//! Native context-menu leases and the platform tracking-loop completion gate.
//!
//! Each presentation is a new revision. Invalidation suppresses its command
//! immediately, but completion means the native tracking loop really ended.

use crate::{Action, EffectOwner, NativeMenuNotSupportedError, NativeMenuOutcome, Task};
use futures::{FutureExt, channel::oneshot, future::Shared};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll},
};

/// Opaque identity for one native menu revision, unique across windows/owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct NativeMenuSessionId(u64);

impl NativeMenuSessionId {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(
            NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .expect("native menu identities exhausted"),
        )
    }
}

/// Native capability/refusal errors. Only `NotSupported` permits an automatic
/// in-window fallback; cancellation failure can leave an old native menu open.
#[derive(Debug, thiserror::Error)]
pub enum NativeMenuError {
    /// This platform does not present application context menus natively.
    #[error(transparent)]
    NotSupported(#[from] NativeMenuNotSupportedError),
    /// The OS refused to end an existing tracking loop. Its command is already
    /// invalidated, but its UI must not be reported as closed.
    #[error("native context-menu cancellation failed: {0}")]
    CancellationFailed(String),
}

/// Awaitable native menu presentation. Retain `id()` for cancellation/revision
/// checks and `effect_owner()` when installing an asynchronous completion
/// callback. Re-enter that owner with `App::with_effect_owner` inside callbacks;
/// awaiting a future does not grant ambient authority to its caller.
pub struct NativeMenuSession {
    pub(crate) id: NativeMenuSessionId,
    pub(crate) owner: Option<EffectOwner>,
    pub(crate) task: Task<NativeMenuOutcome>,
}

impl NativeMenuSession {
    /// The revision that `Window::cancel_context_menu` can invalidate.
    pub fn id(&self) -> NativeMenuSessionId {
        self.id
    }

    /// Explicit effect authority captured at presentation, never from focus.
    pub fn effect_owner(&self) -> Option<EffectOwner> {
        self.owner
    }

    /// Allows dispatch/completion to run without awaiting its result.
    pub fn detach(self) {
        self.task.detach();
    }
}

impl Future for NativeMenuSession {
    type Output = NativeMenuOutcome;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().task).poll(cx)
    }
}

/// Backend result, distinct from unsupported presentation and transport loss.
pub enum PlatformNativeMenuOutcome {
    /// An enabled native command was chosen.
    Selected(Box<dyn Action>),
    /// Tracking ended without choosing a command.
    Dismissed,
    /// Tracking ended after the revision was invalidated.
    Cancelled,
    /// Presentation failed; this is not user dismissal.
    Unavailable,
}

/// Shared foreground-only lifecycle gate for native backend implementations.
/// Replacements invalidate the old gate and await `finished()` before entering
/// another modal native loop. A cancelled pending presentation must still wait
/// for its predecessor before finishing, preserving the replacement chain.
#[derive(Clone)]
pub struct PlatformNativeMenuSession {
    id: NativeMenuSessionId,
    state: Rc<PlatformNativeMenuState>,
    finished: Shared<oneshot::Receiver<()>>,
}

struct PlatformNativeMenuState {
    invalidated: Cell<bool>,
    result: RefCell<Option<oneshot::Sender<PlatformNativeMenuOutcome>>>,
    finished: RefCell<Option<oneshot::Sender<()>>>,
}

impl PlatformNativeMenuSession {
    /// Creates one tracking lifetime for the Window-issued identity.
    pub fn new(id: NativeMenuSessionId) -> (Self, oneshot::Receiver<PlatformNativeMenuOutcome>) {
        let (result, receiver) = oneshot::channel();
        let (finished, closed) = oneshot::channel();
        (
            Self {
                id,
                state: Rc::new(PlatformNativeMenuState {
                    invalidated: Cell::new(false),
                    result: RefCell::new(Some(result)),
                    finished: RefCell::new(Some(finished)),
                }),
                finished: closed.shared(),
            },
            receiver,
        )
    }

    /// The identity to compare before calling a native cancellation API.
    pub fn id(&self) -> NativeMenuSessionId {
        self.id
    }

    /// Prevents any late command from this presentation from being delivered.
    /// This does not claim that its native UI has closed.
    pub fn invalidate(&self) {
        self.state.invalidated.set(true);
    }

    /// Whether the presentation must not begin tracking or deliver a command.
    pub fn is_invalidated(&self) -> bool {
        self.state.invalidated.get()
    }

    /// Waits until tracking has ended (or its backend has disappeared).
    pub async fn finished(&self) {
        let _ = self.finished.clone().await;
    }

    /// Called only after tracking exits, or before tracking when presentation
    /// is cancelled/unavailable. Repeated/late completion cannot send twice.
    pub fn complete(&self, outcome: PlatformNativeMenuOutcome) {
        if let Some(sender) = self.state.result.borrow_mut().take() {
            let outcome = if self.is_invalidated() {
                PlatformNativeMenuOutcome::Cancelled
            } else {
                outcome
            };
            let _ = sender.send(outcome);
        }
        if let Some(sender) = self.state.finished.borrow_mut().take() {
            let _ = sender.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_context_menu_invalidation_waits_for_real_completion() {
        let (session, mut receiver) = PlatformNativeMenuSession::new(NativeMenuSessionId::new());
        session.invalidate();
        assert!(receiver.try_recv().unwrap().is_none());
        assert!(session.finished().now_or_never().is_none());
        session.complete(PlatformNativeMenuOutcome::Selected(Box::new(
            crate::NoAction,
        )));
        assert!(matches!(
            receiver.try_recv().unwrap(),
            Some(PlatformNativeMenuOutcome::Cancelled)
        ));
        assert!(session.finished().now_or_never().is_some());
        session.complete(PlatformNativeMenuOutcome::Dismissed);
    }

    #[test]
    fn native_context_menu_transport_loss_is_not_dismissal() {
        let (session, mut receiver) = PlatformNativeMenuSession::new(NativeMenuSessionId::new());
        drop(session);
        assert!(receiver.try_recv().is_err());
    }
}
