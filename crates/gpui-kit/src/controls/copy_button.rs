//! Copying caller-supplied text, and saying truthfully whether it worked.
//!
//! # What GPUI offers, and what it does not
//!
//! [`gpui::App::write_to_clipboard`] takes a [`gpui::ClipboardItem`] and
//! returns `()`. There is no `Result`, no error, and no callback: the platform
//! layer takes the item and the call is over. So a button that showed a tick
//! because `write_to_clipboard` returned would be showing a tick because a
//! function with no failure mode did not fail, which is not evidence of
//! anything.
//!
//! The one thing GPUI does offer is [`gpui::App::read_from_clipboard`], which
//! returns `Option<ClipboardItem>`. That is real evidence, so it is what this
//! component uses: it writes, reads back, and compares. A read that comes back
//! empty, or comes back holding something else, is reported as a failure.
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
    App, ClipboardItem, Context, EventEmitter, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, SharedString, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Space, TypeScale};
use web_time::Instant;

use crate::controls::button::{Button, ButtonVariant};
use crate::foundation::{Disableable, Ident, Sizable, StyledExt, text as foundation_text};
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
/// The readback is the whole point: the write itself cannot report anything.
pub fn verified_clipboard_copy(text: &str, cx: &mut App) -> Result<(), SharedString> {
    cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
    match cx.read_from_clipboard().and_then(|item| item.text()) {
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
        let text = self.text.clone();
        let outcome = match copier {
            Some(copier) => copier(text.as_ref(), cx),
            None => verified_clipboard_copy(text.as_ref(), cx),
        };
        match outcome {
            Ok(()) => {
                self.state = CopyState::Copied;
                self.remaining = Some(self.confirmation);
                self.last_tick = None;
                cx.emit(CopyEvent::Copied);
            }
            Err(reason) => {
                // A refusal is not put on a timer: see the module note.
                self.state = CopyState::Failed(reason.clone());
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

    fn glyph(&self) -> Icon {
        match self.state {
            CopyState::Copied => Icon::Check,
            CopyState::Failed(_) => Icon::Danger,
            CopyState::Idle => Icon::Copy,
        }
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
        let glyph = self.glyph();
        let parent = self.ident.semantic_id();

        // The three answers are three chips, not one chip with three captions:
        // a refusal takes the danger tint the rest of the library refuses in,
        // and the confirmation is carried by the mark beside it.
        let variant = match self.state {
            CopyState::Failed(_) => ButtonVariant::Danger,
            _ => self.variant,
        };

        let button = Button::new(self.ident.child("action"))
            .semantic_parent(parent.clone())
            .variant(variant)
            .control_size(self.size)
            .disabled(self.disabled)
            .track_focus(&self.focus_handle)
            .map(|button| {
                match (self.glyph_only, self.name.clone()) {
                    (true, Some(name)) => button.icon_only(glyph, name),
                    // The words change with the state, so the name a reader
                    // hears changes with it too; there is no second name that
                    // would go stale.
                    _ => button.icon(glyph).label(label.clone()),
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
            CopyState::Copied => Some((cx.strings().text(StringKey::CopyDone), false)),
            CopyState::Failed(reason) => Some((reason.clone(), true)),
        };
        let status_ident = self.ident.child("status");
        let metrics = theme.control.get(self.size);
        let status = status.map(|(text, failed)| {
            let tone = if failed {
                theme.colors.danger
            } else {
                theme.colors.success
            };
            div()
                .row()
                .flex_none()
                .gap_token(&theme, Space::Xs)
                .child(
                    gpui_kit_assets::icon(if failed { Icon::Danger } else { Icon::Check })
                        .size(px(metrics.icon_size))
                        .text_color(tone),
                )
                .child(foundation_text(&theme, TypeScale::Caption, text.clone()).text_color(tone))
                .semantic_in(
                    cx,
                    NodeSpec::new(status_ident.semantic_id(), Role::Status)
                        .parent(parent.clone())
                        .text(text)
                        .invalid(failed),
                )
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
