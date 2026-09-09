//! A caller-owned route history and a single active page viewport.
//!
//! The host applies navigation only after accepting an intent. Route payloads
//! never enter Kit; entries are stable visit identities (two visits to one
//! route must have different ids). Back retains forward history, push forks
//! it, and replace changes only the current visit.

use std::collections::HashMap;

use gpui::{
    AnyElement, App, FocusHandle, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};

use crate::foundation::Ident;
use crate::motion::{Animated, Entrance};

/// Durable visit identities. Store this data at the call site, not in a
/// component; serialization and route resolution belong to the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavHistory {
    entries: Vec<SharedString>,
    cursor: usize,
    revision: u64,
}

impl NavHistory {
    pub fn new(root: impl Into<SharedString>) -> Self {
        Self {
            entries: vec![root.into()],
            cursor: 0,
            revision: 0,
        }
    }

    pub fn current(&self) -> &SharedString {
        &self.entries[self.cursor]
    }
    pub fn entries(&self) -> &[SharedString] {
        &self.entries
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn can_pop(&self) -> bool {
        self.cursor > 0
    }
    pub fn can_forward(&self) -> bool {
        self.cursor + 1 < self.entries.len()
    }

    /// Reject duplicate visit ids without modifying history. Pushing after
    /// back discards only the abandoned forward branch.
    pub fn push(&mut self, visit: impl Into<SharedString>) -> bool {
        let visit = visit.into();
        if self.entries.contains(&visit) {
            return false;
        }
        self.entries.truncate(self.cursor + 1);
        self.entries.push(visit);
        self.cursor += 1;
        self.revision = self.revision.wrapping_add(1);
        true
    }

    /// Back by one visit, retaining the forward branch. Root cannot be popped.
    pub fn pop(&mut self) -> bool {
        if !self.can_pop() {
            return false;
        }
        self.cursor -= 1;
        self.revision = self.revision.wrapping_add(1);
        true
    }

    pub fn forward(&mut self) -> bool {
        if !self.can_forward() {
            return false;
        }
        self.cursor += 1;
        self.revision = self.revision.wrapping_add(1);
        true
    }

    /// Replace the current visit without discarding either history branch.
    /// The same id is a no-op; ids used by another visit are rejected.
    pub fn replace(&mut self, visit: impl Into<SharedString>) -> bool {
        let visit = visit.into();
        if self.entries.contains(&visit) {
            return false;
        }
        self.entries[self.cursor] = visit;
        self.revision = self.revision.wrapping_add(1);
        true
    }

    /// Validate caller-restored history atomically. No page is invented when
    /// persisted state is empty, duplicated, or points outside the entries.
    pub fn restore(entries: Vec<SharedString>, cursor: usize) -> Option<Self> {
        let unique: std::collections::HashSet<_> = entries.iter().collect();
        (cursor < entries.len() && unique.len() == entries.len()).then_some(Self {
            entries,
            cursor,
            revision: 0,
        })
    }
}

#[derive(Default)]
struct PageFocus {
    active: Option<SharedString>,
    root: Option<FocusHandle>,
    saved: HashMap<SharedString, FocusHandle>,
    pending: Option<FocusHandle>,
}

/// Active navigation content. Inactive pages are not mounted, so their input
/// handlers and accessibility nodes cannot remain active during a transition.
/// Uses a token-backed fade on each accepted history change; reduced motion
/// settles immediately. It restores the last focused descendant on revisits
/// when focus was inside the outgoing page, and never steals external focus.
#[derive(IntoElement)]
pub struct NavStack {
    ident: Ident,
    history: NavHistory,
    label: SharedString,
    focus: FocusHandle,
    content: AnyElement,
}

impl std::fmt::Debug for NavStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NavStack")
            .field("ident", &self.ident)
            .field("history", &self.history)
            .finish()
    }
}

impl NavStack {
    /// `focus` is the active page's initial focus target and remains owned by
    /// the caller. It must be stable while that visit is in history.
    pub fn new(
        ident: impl Into<Ident>,
        history: &NavHistory,
        label: impl Into<SharedString>,
        focus: FocusHandle,
        content: impl IntoElement,
    ) -> Self {
        Self {
            ident: ident.into(),
            history: history.clone(),
            label: label.into(),
            focus,
            content: content.into_any_element(),
        }
    }
}

impl RenderOnce for NavStack {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state =
            window.use_keyed_state(self.ident.child("focus-state").element_id(), cx, |_, _| {
                PageFocus::default()
            });
        state.update(cx, |state, cx| {
            state
                .saved
                .retain(|id, _| self.history.entries.contains(id));
            if state.active.as_ref() != Some(self.history.current()) {
                state.pending = None;
                let owned_focus = state
                    .root
                    .as_ref()
                    .is_some_and(|root| root.contains_focused(window, cx));
                if owned_focus {
                    if let (Some(old), Some(handle)) = (&state.active, window.focused(cx)) {
                        state.saved.insert(old.clone(), handle);
                    }
                    let target = state
                        .saved
                        .get(self.history.current())
                        .unwrap_or(&self.focus)
                        .clone();
                    target.focus(window, cx);
                    state.pending = Some(target);
                    // The new dispatch tree exists only after this frame.
                    // Revalidate on our next render, not in a deferred callback
                    // that could outlive this mounted navigation surface.
                    window.request_animation_frame();
                }
                state.active = Some(self.history.current().clone());
                state.root = Some(self.focus.clone());
            } else if let Some(target) = state.pending.take()
                && target.is_focused(window)
                && !self.focus.contains_focused(window, cx)
            {
                self.focus.focus(window, cx);
            }
        });
        let page_id = self.ident.child(self.history.current().as_ref());
        div()
            .id(self.ident.element_id())
            .w_full()
            .overflow_hidden()
            .child(
                div()
                    .id(page_id.element_id())
                    .track_focus(&self.focus)
                    .w_full()
                    .child(self.content)
                    .semantic_in(
                        cx,
                        NodeSpec::new(page_id.semantic_id(), Role::Region)
                            .parent(self.ident.semantic_id())
                            .text(self.label),
                    )
                    .animate_in(
                        self.ident
                            .child(format!("transition-{}", self.history.revision))
                            .element_id(),
                        cx,
                        Entrance::Fade,
                    ),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .value(self.history.current().clone()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_forks_and_replaces_without_losing_the_other_branch() {
        let mut h = NavHistory::new("root");
        assert!(!h.pop());
        assert!(h.push("first"));
        assert!(h.push("second"));
        assert!(h.pop());
        assert!(h.replace("replacement"));
        assert!(h.forward());
        assert_eq!(h.current(), "second");
        assert!(h.pop());
        assert_eq!(h.current(), "replacement");
        assert!(h.push("fork"));
        assert!(!h.can_forward());
        assert_eq!(
            h.entries(),
            &[
                SharedString::from("root"),
                "replacement".into(),
                "fork".into()
            ]
        );
        let before = h.clone();
        assert!(!h.push("root"));
        assert!(!h.replace("replacement"));
        assert_eq!(h, before);
    }

    #[test]
    fn restoration_refuses_invalid_cursor_and_duplicate_visits() {
        assert!(NavHistory::restore(vec![], 0).is_none());
        assert!(NavHistory::restore(vec!["a".into()], 1).is_none());
        assert!(NavHistory::restore(vec!["a".into(), "a".into()], 0).is_none());
        let h = NavHistory::restore(vec!["a".into(), "b".into()], 0).expect("valid history");
        assert_eq!(h.current(), "a");
        assert!(h.can_forward());
    }
}
