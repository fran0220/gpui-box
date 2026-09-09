//! Copying caller-supplied text, and saying truthfully whether it worked.
//!
//! # What GPUI offers, and what it does not
//!
//! [`gpui::App::try_write_to_clipboard`] checks the current owner's policy and
//! reports a refusal before any platform mutation. An authorized submission
//! still has no platform delivery receipt. It cannot by itself justify a tick.
//!
//! GPUI also offers [`gpui::App::try_read_from_clipboard`]. That is real
//! readback evidence when authorized, so it is what this
//! component uses: it writes, reads back, and compares. A read that comes back
//! empty, or comes back holding something else, is reported as a failure.
//! A write allowed but read denied is reported as unavailable verification,
//! not as a refused write and not as verified success.
//!
//! That check is honest but not complete, and the gap is stated rather than
//! papered over: a platform where a write silently succeeds into a clipboard
//! that a read then reports correctly, while some other application never sees
//! it, would be indistinguishable from success here. Nothing in GPUI's surface
//! can tell that apart. What the check does catch is the common case — a
//! clipboard that refused the write — and it never reports success on the
//! strength of a call that cannot fail.
//!
//! A host that knows better supplies its own [`CopyButton::copier`], which
//! returns a `Result` and whose failure text is shown verbatim.
//!
//! # Why the confirmation does not time the failure out
//!
//! A confirmation is transient: it says "that went through" and there is no
//! reason for it to stay. A failure is not, for the reason
//! `docs/components.md` gives for notifications — a failure nobody saw is a
//! failure that was never reported. So the tick fades on a timer and the
//! refusal stays until the next attempt replaces it.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, ClipboardItem, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, Styled, Window, div, prelude::FluentBuilder,
    px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TypeScale};
use web_time::Instant;

use crate::controls::button::{Button, ButtonVariant};
use crate::foundation::{Disableable, Ident, Sizable, StyledExt, text as foundation_text};
use crate::overlay::Tooltipped;
use crate::strings::{ActiveStrings, StringKey};

/// What the button is currently claiming.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CopyState {
    /// Nothing has been tried, or the last confirmation has expired.
    #[default]
    Idle,
    /// The clipboard took it, and reading it back agreed.
    Copied,
    /// It did not go through, for the reason carried here.
    Failed(SharedString),
}

impl CopyState {
    pub fn is_copied(&self) -> bool {
        matches!(self, Self::Copied)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

/// What a copy button reports. The owner decides what any of it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyEvent {
    Copied,
    /// Carries the reason, which is the same text the button shows.
    Failed(SharedString),
}

impl EventEmitter<CopyEvent> for CopyButton {}

/// Puts text somewhere and says whether it got there.
type Copier = Rc<dyn Fn(&str, &mut App) -> Result<(), SharedString>>;

/// Writes to the platform clipboard and reads it back to check.
///
/// The readback verifies delivery; an authorized write alone is only submission.
pub fn verified_clipboard_copy(text: &str, cx: &mut App) -> Result<(), SharedString> {
    cx.try_write_to_clipboard(ClipboardItem::new_string(text.to_string()))
        .map_err(|denial| SharedString::from(denial.to_string()))?;
    let read = cx.try_read_from_clipboard().map_err(|denial| {
        cx.strings().format(
            StringKey::CopyVerificationUnavailable,
            &[&denial.to_string()],
        )
    })?;
    match read.and_then(|item| item.text()) {
        Some(read) if read == text => Ok(()),
        _ => Err(cx.strings().text(StringKey::CopyFailedDetail)),
    }
}

/// A button that copies caller-supplied text and confirms what happened.
pub struct CopyButton {
    ident: Ident,
    focus_handle: FocusHandle,
    text: SharedString,
    label: Option<SharedString>,
    /// What the button is called when it carries only a glyph.
    name: Option<SharedString>,
    glyph_only: bool,
    variant: ButtonVariant,
    size: ControlSize,
    disabled: bool,
    copier: Option<Copier>,
    confirmation: Duration,
    state: CopyState,
    /// Whether the current refusal came from the fallback clipboard check.
    default_failure: bool,
    /// How much of the confirmation is left, and when it was last spent.
    remaining: Option<Duration>,
    last_tick: Option<Instant>,
}

impl std::fmt::Debug for CopyButton {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CopyButton")
            .field("ident", &self.ident)
            .field("state", &self.state)
            .field("disabled", &self.disabled)
            .field("has_copier", &self.copier.is_some())
            .finish()
    }
}

impl CopyButton {
    pub fn new(ident: impl Into<Ident>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            ident: ident.into(),
            focus_handle: cx.focus_handle(),
            text: SharedString::default(),
            label: None,
            name: None,
            glyph_only: false,
            variant: ButtonVariant::Secondary,
            size: ControlSize::Md,
            disabled: false,
            copier: None,
            confirmation: Duration::from_millis(cx.theme().motion.confirmation_ms),
            state: CopyState::Idle,
            default_failure: false,
            remaining: None,
            last_tick: None,
        }
    }

    /// The text this button copies. Never published: a copy button is a
    /// plausible carrier of a credential, so the tree gets the button, not its
    /// payload.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = text.into();
        self
    }

    /// Replaces the payload from the host side, between frames.
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.text = text.into();
        cx.notify();
    }

    /// Words on the button instead of the catalogue's own.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Draws the button as a square carrying only its glyph, and names it.
    pub fn glyph_only(mut self, name: impl Into<SharedString>) -> Self {
        self.glyph_only = true;
        self.name = Some(name.into());
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Supplies a host that knows whether the copy worked.
    ///
    /// The default writes to the platform clipboard and reads it back; a host
    /// with a better answer replaces it, and the failure text it returns is
    /// shown verbatim rather than reworded.
    pub fn copier(
        mut self,
        copier: impl Fn(&str, &mut App) -> Result<(), SharedString> + 'static,
    ) -> Self {
        self.copier = Some(Rc::new(copier));
        self
    }

    /// How long a confirmation stays. A refusal is not timed and this does not
    /// touch it.
    pub fn confirmation(mut self, confirmation: Duration) -> Self {
        self.confirmation = confirmation;
        self
    }

    pub fn state(&self) -> &CopyState {
        &self.state
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    /// Does the copy and records what actually happened.
    pub fn copy(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let copier = self.copier.clone();
        let default_copier = copier.is_none();
        let text = self.text.clone();
        let outcome = match copier {
            Some(copier) => copier(text.as_ref(), cx),
            None => verified_clipboard_copy(text.as_ref(), cx),
        };
        match outcome {
            Ok(()) => {
                self.state = CopyState::Copied;
                self.default_failure = false;
                self.remaining = Some(self.confirmation);
                self.last_tick = None;
                cx.emit(CopyEvent::Copied);
            }
            Err(reason) => {
                // A refusal is not put on a timer: see the module note.
                self.state = CopyState::Failed(reason.clone());
                self.default_failure =
                    default_copier && reason == cx.strings().text(StringKey::CopyFailedDetail);
                self.remaining = None;
                self.last_tick = None;
                cx.emit(CopyEvent::Failed(reason));
            }
        }
        cx.notify();
    }

    /// Spends one frame of the confirmation, if one is standing.
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(remaining) = self.remaining else {
            self.last_tick = None;
            return;
        };
        let now = cx.background_executor().now();
        let spent = self
            .last_tick
            .map(|last| now.saturating_duration_since(last))
            .unwrap_or_default();
        let left = remaining.saturating_sub(spent);
        if left.is_zero() {
            self.state = CopyState::Idle;
            self.remaining = None;
            self.last_tick = None;
            cx.notify();
            return;
        }
        self.remaining = Some(left);
        self.last_tick = Some(now);
        window.request_animation_frame();
    }

    /// The words on the control name the action, in every state.
    ///
    /// The outcome is the status line's to report, and it says it once: a
    /// button reading "Copied" beside a line reading "Copied" is one claim
    /// drawn twice, and a control whose words change under the pointer is a
    /// control that changes width while it is being aimed at.
    fn button_label(&self, cx: &App) -> SharedString {
        self.label
            .clone()
            .unwrap_or_else(|| cx.strings().text(StringKey::Copy))
    }
}

impl Disableable for CopyButton {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for CopyButton {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for CopyButton {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CopyButton {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.tick(window, cx);
        let theme = cx.theme().clone();
        let label = self.button_label(cx);
        let parent = self.ident.semantic_id();

        let button = Button::new(self.ident.child("action"))
            .semantic_parent(parent.clone())
            .variant(self.variant)
            .control_size(self.size)
            .disabled(self.disabled)
            .track_focus(&self.focus_handle)
            .map(|button| {
                match (self.glyph_only, self.name.clone()) {
                    (true, Some(name)) => button.icon_only(Icon::Copy, name),
                    // The action keeps its name while the separate mark
                    // reports what happened.
                    _ => button.icon(Icon::Copy).label(label.clone()),
                }
            })
            .when(!self.disabled, |button| {
                let copy = cx.entity().downgrade();
                button.on_click(move |_, cx| {
                    copy.update(cx, |copy, cx| copy.copy(cx)).ok();
                })
            });

        // The outcome is a separate node because it is a separate claim. A
        // test asking whether the copy worked reads this, not the wording on
        // the control, and a refusal publishes `invalid` so nothing has to
        // match on prose to tell the two apart.
        let status = match &self.state {
            CopyState::Idle => None,
            CopyState::Copied => Some((cx.strings().text(StringKey::CopyDone), None, false, false)),
            CopyState::Failed(reason) => Some((
                cx.strings().text(StringKey::CopyFailed),
                Some(reason.clone()),
                true,
                self.default_failure,
            )),
        };
        let status_ident = self.ident.child("status");
        let metrics = theme.control.get(self.size);
        let status = status.map(|(state_text, reason, failed, default_failure)| {
            let tone = if failed {
                theme.colors.danger
            } else {
                theme.colors.success
            };
            let mark_ident = status_ident.child("mark");
            // A submitted write with unavailable verification must not acquire
            // a "not copied" claim merely by hovering its failure mark.
            let help = reason.clone().unwrap_or_else(|| state_text.clone());
            // The status reads as the reason it was given, verbatim: a
            // reader of the tree is told why, not merely that.
            let spec = NodeSpec::new(status_ident.semantic_id(), Role::Status)
                .parent(parent.clone())
                .text(reason.clone().unwrap_or(state_text))
                .invalid(failed);
            div()
                .row()
                .flex_none()
                .gap_token(&theme, Space::Xs)
                .child(
                    div()
                        .id(mark_ident.element_id())
                        .child(
                            gpui_kit_assets::icon(if failed { Icon::Danger } else { Icon::Check })
                                .size(px(metrics.icon_size))
                                .text_color(tone),
                        )
                        .tip(mark_ident, help),
                )
                // A host refusal is evidence, not a state caption. The
                // fallback mechanism stays behind the mark as hover help.
                .children(reason.filter(|_| !default_failure).map(|reason| {
                    foundation_text(&theme, TypeScale::Caption, reason).text_color(tone)
                }))
                .semantic_in(cx, spec)
        });

        div()
            .row()
            .flex_none()
            .gap_token(&theme, Space::Xs)
            .child(button)
            .children(status)
            .semantic_in(
                cx,
                NodeSpec::new(parent, Role::Group)
                    .disabled(self.disabled)
                    .invalid(self.state.is_failed()),
            )
            .min_h(px(0.0))
    }
}

#[cfg(test)]
mod clipboard_policy_tests {
    use super::*;
    use gpui::{AppContext as _, ClipboardOperation, EffectOwner, TestAppContext, effect_owner};
    use gpui_kit_testkit::harness::Harness;
    use std::cell::{Cell, RefCell};

    #[gpui::test]
    fn verification_distinguishes_denied_write_denied_read_and_success(cx: &mut TestAppContext) {
        let owner = EffectOwner::new();
        let inspector = EffectOwner::new();
        let slot = Rc::new(RefCell::new(None));
        let build = slot.clone();
        let mut harness = Harness::new(cx, crate::install, move |window, cx| {
            let button = build
                .borrow_mut()
                .get_or_insert_with(|| {
                    cx.new(|cx| CopyButton::new("policy.copy", window, cx).text("replacement"))
                })
                .clone();
            effect_owner(owner, button).into_any_element()
        });
        let button = slot.borrow().clone().expect("copy button built");
        let mode = Rc::new(Cell::new(0));
        let policy_mode = mode.clone();
        let events = Rc::new(RefCell::new(Vec::new()));
        let reports = events.clone();
        let subscription = harness.update(|_, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
            cx.set_clipboard_policy(move |current, operation| {
                current == inspector
                    || (current == owner
                        && (policy_mode.get() == 2
                            || (policy_mode.get() == 1 && operation == ClipboardOperation::Write)))
            });
            cx.subscribe(&button, move |_, event: &CopyEvent, _| {
                reports.borrow_mut().push(event.clone())
            })
        });
        harness.click("policy.copy.action");
        harness.update(|_, cx| {
            assert!(button.read(cx).state().is_failed());
            cx.with_effect_owner(Some(inspector), |cx| {
                assert_eq!(
                    cx.try_read_from_clipboard()
                        .expect("inspector read")
                        .expect("clipboard value")
                        .text()
                        .as_deref(),
                    Some("sentinel")
                )
            });
        });
        mode.set(1);
        harness.update(|_, cx| {
            crate::strings::set_strings(
                [(
                    StringKey::CopyVerificationUnavailable,
                    "写入已提交；无法验证：{0}".into(),
                )],
                cx,
            )
        });
        harness.click("policy.copy.action");
        harness.update(|_, cx| {
            assert!(matches!(button.read(cx).state(), CopyState::Failed(reason) if reason.as_ref() == "写入已提交；无法验证：clipboard operation denied by host"));
            cx.with_effect_owner(Some(inspector), |cx| assert_eq!(cx.try_read_from_clipboard().expect("inspector read").expect("clipboard value").text().as_deref(), Some("replacement")));
        });
        assert!(
            harness
                .node("policy.copy.status")
                .expect("refusal status")
                .invalid
        );
        assert!(
            events
                .borrow()
                .iter()
                .all(|event| matches!(event, CopyEvent::Failed(_)))
        );
        mode.set(2);
        harness.click("policy.copy.action");
        harness.update(|_, cx| assert!(button.read(cx).state().is_copied()));
        assert_eq!(events.borrow().last(), Some(&CopyEvent::Copied));
        drop(subscription);
    }
}
