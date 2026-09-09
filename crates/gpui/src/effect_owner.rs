//! Host-owned authority for effects from an inherited element subtree.
//!
//! Tokens carry no product policy. Hosts map them to grants and revoke stale
//! generations in the clipboard policy. Neither focus nor native user input
//! confers authority. Asynchronous work must explicitly retain a token and
//! re-enter `App::with_effect_owner`; unowned effects fail closed with a policy.

use crate::{
    AnyElement, App, Bounds, ClipboardItem, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Window,
};
use std::{
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

/// Opaque host identity. A fresh token never reuses an earlier generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct EffectOwner(u64);

impl EffectOwner {
    /// Allocates a fresh identity. Keep it for the lifetime of one host owner.
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(
            NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .expect("effect owner identities exhausted"),
        )
    }
}

impl Default for EffectOwner {
    fn default() -> Self {
        Self::new()
    }
}

/// A typed child whose owner survives container transformations until render.
///
/// Containers retain this wrapper instead of erasing a child into an element
/// early. Transform a child's options with `map`; inspect them with `as_ref`.
/// Plain values converted with `From` inherit the eventual parent's context.
#[derive(Clone, Debug)]
pub struct EffectScoped<T> {
    owner: Option<EffectOwner>,
    value: T,
}

impl<T> EffectScoped<T> {
    /// Attributes the child's eventual element lifecycle to this mount owner.
    pub fn new(owner: EffectOwner, value: T) -> Self {
        Self {
            owner: Some(owner),
            value,
        }
    }

    /// Transforms a typed value without discarding its scope.
    pub fn map<U>(self, transform: impl FnOnce(T) -> U) -> EffectScoped<U> {
        EffectScoped {
            owner: self.owner,
            value: transform(self.value),
        }
    }
}

impl<T> AsRef<T> for EffectScoped<T> {
    /// Inspects child options without consuming the retained scope.
    fn as_ref(&self) -> &T {
        &self.value
    }
}

impl<T> From<T> for EffectScoped<T> {
    fn from(value: T) -> Self {
        Self { owner: None, value }
    }
}

impl<T: IntoElement> IntoElement for EffectScoped<T> {
    type Element = AnyElement;

    fn into_element(self) -> Self::Element {
        match self.owner {
            Some(owner) => effect_owner(owner, self.value).into_any_element(),
            // Unscoped is inheritance, not an explicit authority reset.
            None => self.value.into_any_element(),
        }
    }
}

/// The clipboard effect requested by a native component.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ClipboardOperation {
    /// Read the ordinary clipboard.
    Read,
    /// Write the ordinary clipboard.
    Write,
    /// Read the primary selection (Linux/FreeBSD).
    ReadPrimary,
    /// Write the primary selection (Linux/FreeBSD).
    WritePrimary,
    /// Read the find pasteboard (macOS).
    ReadFind,
    /// Write the find pasteboard (macOS).
    WriteFind,
}

/// A host refused access; distinct from an empty clipboard.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ClipboardDenied {
    /// Policy is enabled but no owner was explicitly retained.
    #[error("clipboard operation has no effect owner")]
    MissingOwner,
    /// The host refused the owner's operation, including stale identities.
    #[error("clipboard operation denied by host")]
    Denied,
}

pub(crate) type ClipboardPolicy = Rc<dyn Fn(EffectOwner, ClipboardOperation) -> bool>;

impl App {
    /// Installs host policy. Missing owners fail closed; native gestures do
    /// not bypass it. Ordinary apps without policy keep platform behavior.
    pub fn set_clipboard_policy(
        &mut self,
        policy: impl Fn(EffectOwner, ClipboardOperation) -> bool + 'static,
    ) {
        self.clipboard_policy = Some(Rc::new(policy));
    }

    /// The explicitly scoped owner, never inferred from focus or current view.
    pub fn current_effect_owner(&self) -> Option<EffectOwner> {
        self.effect_owner.get()
    }

    /// Runs a synchronous scope with an explicit owner (or without authority).
    /// Restores the previous context even when the callback unwinds.
    pub fn with_effect_owner<R>(
        &mut self,
        owner: Option<EffectOwner>,
        f: impl FnOnce(&mut App) -> R,
    ) -> R {
        let _restore = self.effect_owner_scope(owner);
        f(self)
    }

    pub(crate) fn effect_owner_scope(&self, owner: Option<EffectOwner>) -> impl Drop + use<> {
        let cell = self.effect_owner.clone();
        let previous = cell.replace(owner);
        gpui_util::defer(move || cell.set(previous))
    }

    fn authorize_clipboard(&self, operation: ClipboardOperation) -> Result<(), ClipboardDenied> {
        if let Some(policy) = &self.clipboard_policy {
            let owner = self
                .current_effect_owner()
                .ok_or(ClipboardDenied::MissingOwner)?;
            if !policy(owner, operation) {
                return Err(ClipboardDenied::Denied);
            }
        }
        Ok(())
    }

    /// Reads the clipboard, distinguishing refusal from an empty value.
    pub fn try_read_from_clipboard(&self) -> Result<Option<ClipboardItem>, ClipboardDenied> {
        self.authorize_clipboard(ClipboardOperation::Read)?;
        Ok(self.platform.read_from_clipboard())
    }

    /// Writes only if the current owner's host policy permits the operation.
    pub fn try_write_to_clipboard(&self, item: ClipboardItem) -> Result<(), ClipboardDenied> {
        self.authorize_clipboard(ClipboardOperation::Write)?;
        self.platform.write_to_clipboard(item);
        Ok(())
    }

    /// Reads primary selection, distinguishing refusal from empty.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub fn try_read_from_primary(&self) -> Result<Option<ClipboardItem>, ClipboardDenied> {
        self.authorize_clipboard(ClipboardOperation::ReadPrimary)?;
        Ok(self.platform.read_from_primary())
    }

    /// Writes primary selection only after host permission.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub fn try_write_to_primary(&self, item: ClipboardItem) -> Result<(), ClipboardDenied> {
        self.authorize_clipboard(ClipboardOperation::WritePrimary)?;
        self.platform.write_to_primary(item);
        Ok(())
    }

    /// Reads the find pasteboard, distinguishing refusal from empty.
    #[cfg(target_os = "macos")]
    pub fn try_read_from_find_pasteboard(&self) -> Result<Option<ClipboardItem>, ClipboardDenied> {
        self.authorize_clipboard(ClipboardOperation::ReadFind)?;
        Ok(self.platform.read_from_find_pasteboard())
    }

    /// Writes the find pasteboard only after host permission.
    #[cfg(target_os = "macos")]
    pub fn try_write_to_find_pasteboard(&self, item: ClipboardItem) -> Result<(), ClipboardDenied> {
        self.authorize_clipboard(ClipboardOperation::WriteFind)?;
        self.platform.write_to_find_pasteboard(item);
        Ok(())
    }
}

/// An inherited authority boundary with no layout or semantic identity of its own.
pub struct EffectOwnerElement {
    owner: Option<EffectOwner>,
    child: AnyElement,
}

/// Attributes a subtree's native effects to the host-supplied opaque owner.
pub fn effect_owner(owner: EffectOwner, child: impl IntoElement) -> EffectOwnerElement {
    EffectOwnerElement::optional(Some(owner), child)
}

impl EffectOwnerElement {
    pub(crate) fn optional(owner: Option<EffectOwner>, child: impl IntoElement) -> Self {
        EffectOwnerElement {
            owner,
            child: child.into_any_element(),
        }
    }
}

impl IntoElement for EffectOwnerElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for EffectOwnerElement {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (
            cx.with_effect_owner(self.owner, |cx| self.child.request_layout(window, cx)),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        cx.with_effect_owner(self.owner, |cx| self.child.prepaint(window, cx));
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        cx.with_effect_owner(self.owner, |cx| self.child.paint(window, cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppContext, Context, Entity, FocusHandle, InputEvent, InteractiveElement, MouseButton,
        MouseDownEvent, ParentElement, Render, StatefulInteractiveElement, StyleRefinement, Styled,
        TestAppContext, deferred, div, point, px,
    };
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
    };

    #[gpui::test]
    fn clipboard_policy_denies_missing_stale_and_legacy_effects(cx: &mut TestAppContext) {
        cx.update(|cx| {
            // Native applications with no policy keep the original contract.
            cx.write_to_clipboard(ClipboardItem::new_string("original".into()));
            assert_eq!(
                cx.read_from_clipboard()
                    .expect("native clipboard value")
                    .text()
                    .as_deref(),
                Some("original")
            );
            let owner = EffectOwner::new();
            let live = Rc::new(Cell::new(true));
            let live_policy = live.clone();
            cx.set_clipboard_policy(move |candidate, op| {
                live_policy.get() && candidate == owner && op == ClipboardOperation::Read
            });
            assert!(matches!(
                cx.try_read_from_clipboard(),
                Err(ClipboardDenied::MissingOwner)
            ));
            cx.write_to_clipboard(ClipboardItem::new_string("unowned overwrite".into()));
            cx.with_effect_owner(Some(owner), |cx| {
                assert_eq!(
                    cx.try_read_from_clipboard()
                        .expect("authorized read")
                        .expect("original value")
                        .text()
                        .as_deref(),
                    Some("original")
                );
                assert_eq!(
                    cx.try_write_to_clipboard(ClipboardItem::new_string("denied".into())),
                    Err(ClipboardDenied::Denied)
                );
                cx.write_to_clipboard(ClipboardItem::new_string("legacy denied".into()));
                assert_eq!(
                    cx.try_read_from_clipboard()
                        .expect("authorized verification")
                        .expect("unchanged value")
                        .text()
                        .as_deref(),
                    Some("original")
                );
                #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                {
                    assert!(matches!(
                        cx.try_read_from_primary(),
                        Err(ClipboardDenied::Denied)
                    ));
                    assert_eq!(
                        cx.try_write_to_primary(ClipboardItem::new_string("denied".into())),
                        Err(ClipboardDenied::Denied)
                    );
                }
                live.set(false);
                assert!(matches!(
                    cx.try_read_from_clipboard(),
                    Err(ClipboardDenied::Denied)
                ));
                assert!(cx.read_from_clipboard().is_none());
            });
            assert_eq!(cx.current_effect_owner(), None);
        });
    }

    #[gpui::test]
    fn owner_scopes_restore_on_unwind_and_deferred_work_requires_explicit_retention(
        cx: &mut TestAppContext,
    ) {
        let owner = EffectOwner::new();
        let other = EffectOwner::new();
        let seen = Rc::new(RefCell::new(Vec::new()));
        cx.update(|cx| {
            cx.set_clipboard_policy(|_, _| true);
            cx.with_effect_owner(Some(owner), |cx| {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    cx.with_effect_owner(Some(other), |_| panic!("scope probe"))
                }));
                assert!(result.is_err());
                assert_eq!(cx.current_effect_owner(), Some(owner));
                let seen = seen.clone();
                cx.defer(move |cx| {
                    seen.borrow_mut().push(cx.current_effect_owner());
                    assert!(matches!(
                        cx.try_read_from_clipboard(),
                        Err(ClipboardDenied::MissingOwner)
                    ));
                    cx.with_effect_owner(Some(owner), |cx| {
                        seen.borrow_mut().push(cx.current_effect_owner());
                        assert!(cx.try_read_from_clipboard().is_ok());
                    });
                });
            });
        });
        assert_eq!(&*seen.borrow(), &[None, Some(owner)]);
        cx.update(|cx| assert_eq!(cx.current_effect_owner(), None));
    }

    actions!(effect_owner_test, [OwnerAction]);

    struct Probe {
        focus: FocusHandle,
        renders: Rc<Cell<usize>>,
        seen: Rc<RefCell<Vec<Option<EffectOwner>>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            div()
                .size_full()
                .track_focus(&self.focus)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.seen.borrow_mut().push(cx.current_effect_owner());
                        assert_eq!(
                            cx.try_write_to_clipboard(ClipboardItem::new_string("native".into())),
                            Err(ClipboardDenied::Denied)
                        );
                    }),
                )
                .on_key_down(cx.listener(|this, _, _, cx| {
                    this.seen.borrow_mut().push(cx.current_effect_owner())
                }))
                .on_action(cx.listener(|this, _: &OwnerAction, _, cx| {
                    this.seen.borrow_mut().push(cx.current_effect_owner())
                }))
        }
    }

    struct OwnerRoot {
        outer: EffectOwner,
        inner: Rc<Cell<EffectOwner>>,
        probe: Entity<Probe>,
        seen: Rc<RefCell<Vec<Option<EffectOwner>>>>,
        overlay: bool,
    }

    impl Render for OwnerRoot {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let seen = self.seen.clone();
            let style = StyleRefinement::default().size(px(100.));
            let child = EffectScoped::new(self.inner.get(), self.probe.clone())
                .map(|probe| probe.cached(style));
            let child = if self.overlay {
                deferred(child).into_any_element()
            } else {
                child.into_any_element()
            };
            effect_owner(
                self.outer,
                div()
                    .size_full()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        seen.borrow_mut().push(cx.current_effect_owner())
                    })
                    .child(EffectScoped::from(child)),
            )
        }
    }

    #[gpui::test]
    fn unscoped_typed_children_inherit_but_owned_siblings_keep_their_scope(
        cx: &mut TestAppContext,
    ) {
        struct TypedRoot(
            EffectOwner,
            EffectOwner,
            Rc<RefCell<Vec<Option<EffectOwner>>>>,
        );
        impl Render for TypedRoot {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let make = || {
                    let seen = self.2.clone();
                    div()
                        .w(px(30.))
                        .h(px(30.))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            seen.borrow_mut().push(cx.current_effect_owner())
                        })
                };
                let children: Vec<EffectScoped<gpui::Div>> =
                    vec![EffectScoped::new(self.1, make()), make().into()];
                effect_owner(
                    self.0,
                    div().flex().children(
                        children
                            .into_iter()
                            .map(|child| child.map(|value| value.flex_none())),
                    ),
                )
            }
        }
        let outer = EffectOwner::new();
        let child = EffectOwner::new();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let cx = cx.add_empty_window();
        cx.draw(
            point(px(0.), px(0.)),
            crate::size(px(60.), px(30.)),
            |_, cx| {
                cx.new(|_| TypedRoot(outer, child, seen.clone()))
                    .into_any_element()
            },
        );
        for x in [5., 35.] {
            cx.simulate_event(MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(x), px(5.)),
                ..Default::default()
            });
        }
        assert_eq!(&*seen.borrow(), &[Some(child), Some(outer)]);
    }

    #[gpui::test]
    fn cached_and_deferred_callbacks_retain_nested_owner_and_reparent_safely(
        cx: &mut TestAppContext,
    ) {
        for overlay in [false, true] {
            let outer = EffectOwner::new();
            let inner = Rc::new(Cell::new(EffectOwner::new()));
            let seen = Rc::new(RefCell::new(Vec::new()));
            let renders = Rc::new(Cell::new(0));
            cx.update(|cx| cx.set_clipboard_policy(|_, _| false));
            let window = cx.add_window(|window, cx| {
                let focus = cx.focus_handle();
                focus.focus(window, cx);
                let probe = cx.new(|_| Probe {
                    focus,
                    seen: seen.clone(),
                    renders: renders.clone(),
                });
                OwnerRoot {
                    outer,
                    inner: inner.clone(),
                    probe,
                    seen: seen.clone(),
                    overlay,
                }
            });
            let draw = |cx: &mut TestAppContext| {
                cx.update_window(window.into(), |_, window, cx| {
                    window.draw(cx).clear(cx);
                })
                .expect("draw owned window");
            };
            draw(cx);
            let first_renders = renders.get();
            draw(cx);
            assert_eq!(
                renders.get(),
                first_renders,
                "same-owner subtree should reuse its cache"
            );
            for reparent in [false, true] {
                if reparent {
                    inner.set(EffectOwner::new());
                    draw(cx);
                    assert!(
                        renders.get() > first_renders,
                        "new owner must invalidate cached callbacks"
                    );
                }
                seen.borrow_mut().clear();
                cx.update_window(window.into(), |_, window, cx| {
                    window.dispatch_event(
                        MouseDownEvent {
                            button: MouseButton::Left,
                            position: point(px(5.), px(5.)),
                            ..Default::default()
                        }
                        .to_platform_input(),
                        cx,
                    );
                    assert_eq!(cx.current_effect_owner(), None);
                })
                .expect("dispatch owned mouse callback");
                assert_eq!(&*seen.borrow(), &[Some(inner.get()), Some(outer)]);
                seen.borrow_mut().clear();
                cx.simulate_keystrokes(window.into(), "a");
                assert_eq!(&*seen.borrow(), &[Some(inner.get())]);
                seen.borrow_mut().clear();
                cx.update_window(window.into(), |_, window, cx| {
                    window.dispatch_action(Box::new(OwnerAction), cx)
                })
                .expect("dispatch owned action callback");
                cx.run_until_parked();
                assert_eq!(&*seen.borrow(), &[Some(inner.get())]);
            }
        }
    }

    struct InputProbe {
        focus: FocusHandle,
        seen: Rc<RefCell<Vec<Option<EffectOwner>>>>,
    }

    impl Render for InputProbe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let entity = cx.entity();
            let focus = self.focus.clone();
            crate::canvas(
                |_, _, _| (),
                move |bounds, _, window, cx| {
                    window.handle_input(
                        &focus,
                        crate::ElementInputHandler::new(bounds, entity),
                        cx,
                    );
                },
            )
            .size_full()
        }
    }

    impl crate::EntityInputHandler for InputProbe {
        fn text_for_range(
            &mut self,
            _: std::ops::Range<usize>,
            _: &mut Option<std::ops::Range<usize>>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<String> {
            None
        }
        fn selected_text_range(
            &mut self,
            _: bool,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<crate::UTF16Selection> {
            None
        }
        fn marked_text_range(
            &self,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<std::ops::Range<usize>> {
            None
        }
        fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
            self.seen.borrow_mut().push(cx.current_effect_owner());
        }
        fn replace_text_in_range(
            &mut self,
            _: Option<std::ops::Range<usize>>,
            _: &str,
            _: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.seen.borrow_mut().push(cx.current_effect_owner());
            assert!(matches!(
                cx.try_read_from_clipboard(),
                Err(ClipboardDenied::Denied)
            ));
        }
        fn replace_and_mark_text_in_range(
            &mut self,
            _: Option<std::ops::Range<usize>>,
            _: &str,
            _: Option<std::ops::Range<usize>>,
            _: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.seen.borrow_mut().push(cx.current_effect_owner());
            assert!(matches!(
                cx.try_read_from_clipboard(),
                Err(ClipboardDenied::Denied)
            ));
        }
        fn bounds_for_range(
            &mut self,
            _: std::ops::Range<usize>,
            _: Bounds<Pixels>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<Bounds<Pixels>> {
            None
        }
        fn character_index_for_point(
            &mut self,
            _: crate::Point<Pixels>,
            _: &mut Window,
            _: &mut Context<Self>,
        ) -> Option<usize> {
            None
        }
    }

    struct InputRoot(EffectOwner, Entity<InputProbe>);
    impl Render for InputRoot {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            effect_owner(self.0, self.1.clone())
        }
    }

    #[gpui::test]
    fn installed_ime_owner_is_not_taken_from_later_focus_or_dispatch(cx: &mut TestAppContext) {
        use crate::PlatformWindow;
        let denied = EffectOwner::new();
        let allowed = EffectOwner::new();
        let seen = Rc::new(RefCell::new(Vec::new()));
        cx.update(|cx| cx.set_clipboard_policy(move |owner, _| owner == allowed));
        let window = cx.add_window(|window, cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            InputRoot(
                denied,
                cx.new(|_| InputProbe {
                    focus,
                    seen: seen.clone(),
                }),
            )
        });
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
        })
        .expect("install input handler");
        let mut platform = cx.test_window(window.into());
        let mut input = platform
            .take_input_handler()
            .expect("installed entity IME handler");
        input.replace_text_in_range(None, "paste");
        input.replace_and_mark_text_in_range(None, "composing", None);
        cx.update_window(window.into(), |_, window, cx| {
            let different_focus = cx.focus_handle();
            different_focus.focus(window, cx);
            cx.with_effect_owner(Some(allowed), |cx| {
                input.dispatch_input("old handler", window, cx);
                assert_eq!(cx.current_effect_owner(), Some(allowed));
            });
            assert_eq!(cx.current_effect_owner(), None);
        })
        .expect("dispatch retained input handler");
        assert_eq!(&*seen.borrow(), &[Some(denied), Some(denied), Some(denied)]);
    }

    #[gpui::test]
    fn asynchronous_continuations_require_explicit_owner_reentry(cx: &mut TestAppContext) {
        let owner = EffectOwner::new();
        let seen = Rc::new(RefCell::new(Vec::new()));
        cx.update(|cx| {
            cx.set_clipboard_policy(|_, _| true);
            cx.with_effect_owner(Some(owner), |cx| {
                let seen = seen.clone();
                cx.spawn(async move |cx| {
                    cx.update(|cx| {
                        seen.borrow_mut().push(cx.current_effect_owner());
                        assert!(matches!(
                            cx.try_read_from_clipboard(),
                            Err(ClipboardDenied::MissingOwner)
                        ));
                        cx.with_effect_owner(Some(owner), |cx| {
                            seen.borrow_mut().push(cx.current_effect_owner());
                            assert!(cx.try_read_from_clipboard().is_ok());
                        });
                    });
                })
                .detach();
            });
        });
        cx.run_until_parked();
        assert_eq!(&*seen.borrow(), &[None, Some(owner)]);
    }

    struct TooltipProbe(Rc<RefCell<Vec<Option<EffectOwner>>>>);
    impl Render for TooltipProbe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.0.borrow_mut().push(cx.current_effect_owner());
            let prepaint = self.0.clone();
            let paint = self.0.clone();
            crate::canvas(
                move |_, _, cx| prepaint.borrow_mut().push(cx.current_effect_owner()),
                move |_, _, _, cx| paint.borrow_mut().push(cx.current_effect_owner()),
            )
            .size(px(30.))
        }
    }

    struct TooltipRoot(EffectOwner, Rc<RefCell<Vec<Option<EffectOwner>>>>);
    impl Render for TooltipRoot {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let seen = self.1.clone();
            effect_owner(
                self.0,
                div()
                    .id("owned-tooltip")
                    .size(px(100.))
                    .tooltip(move |_, cx| {
                        seen.borrow_mut().push(cx.current_effect_owner());
                        cx.new(|_| TooltipProbe(seen.clone())).into()
                    }),
            )
        }
    }

    #[gpui::test]
    fn tooltip_timer_and_window_overlay_preserve_owner(cx: &mut TestAppContext) {
        use crate::MouseMoveEvent;
        let owner = EffectOwner::new();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let window = cx.add_window(|_, _| TooltipRoot(owner, seen.clone()));
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
            window.dispatch_event(
                MouseMoveEvent {
                    position: point(px(10.), px(10.)),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
        })
        .expect("hover tooltip owner");
        cx.run_until_parked();
        cx.dispatcher
            .advance_clock(std::time::Duration::from_secs(2));
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
        })
        .expect("draw tooltip overlay");
        assert!(
            seen.borrow().len() >= 4,
            "timer builder, render, prepaint and paint must run"
        );
        assert!(seen.borrow().iter().all(|seen| *seen == Some(owner)));
    }
}
