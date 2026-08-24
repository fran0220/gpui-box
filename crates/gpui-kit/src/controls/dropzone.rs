//! An area that accepts something dropped on it from outside itself.
//!
//! A dropzone distinguishes three states and never conflates two of them:
//!
//! - **Idle** — nothing is being dragged over it;
//! - **Accepting** — a payload it handles is over it;
//! - **Refusing** — a payload it does not handle is over it.
//!
//! Refusing is drawn as a refusal, with the reason the caller gave, and never
//! as idle. A zone that looked idle while refusing would tell a typist that
//! letting go was going to work.

use std::rc::Rc;

use gpui::{
    App, ExternalPaths, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};

use crate::foundation::{Disableable, Ident, StyledExt, text as foundation_text};
use crate::interaction::dnd::{self, DragItem, FILE_KIND};
use crate::layout::measure;
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

type DropHandler = Rc<dyn Fn(&DragItem, &mut Window, &mut App)>;
type FilesHandler = Rc<dyn Fn(&ExternalPaths, &mut Window, &mut App)>;

/// What a dropzone is currently saying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropzoneState {
    Idle,
    Accepting,
    Refusing,
}

impl DropzoneState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Accepting => "accepting",
            Self::Refusing => "refusing",
        }
    }
}

impl HasPhase for DropzoneState {
    fn phase(&self) -> Phase {
        match self {
            Self::Idle => Phase::Idle,
            Self::Accepting => Phase::Ready,
            Self::Refusing => Phase::Unavailable,
        }
    }
}

/// An area that accepts a drop.
#[derive(IntoElement)]
pub struct Dropzone {
    ident: Ident,
    label: SharedString,
    hint: Option<SharedString>,
    /// Why a payload this zone does not handle is refused.
    refusal: Option<SharedString>,
    kinds: Vec<SharedString>,
    pinned: Option<DropzoneState>,
    disabled: bool,
    /// Validation owned by a surrounding field, separate from a live drag
    /// payload this zone itself refuses.
    invalid: bool,
    icon: Option<Icon>,
    on_drop: Option<DropHandler>,
    on_files: Option<FilesHandler>,
}

impl std::fmt::Debug for Dropzone {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Dropzone")
            .field("ident", &self.ident)
            .field("kinds", &self.kinds)
            .field("pinned", &self.pinned)
            .field("disabled", &self.disabled)
            .field(
                "has_handler",
                &(self.on_drop.is_some() || self.on_files.is_some()),
            )
            .finish()
    }
}

impl Dropzone {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            hint: None,
            refusal: None,
            kinds: vec![SharedString::new_static(FILE_KIND)],
            pinned: None,
            disabled: false,
            invalid: false,
            icon: Some(Icon::Paperclip),
            on_drop: None,
            on_files: None,
        }
    }

    /// A second line, shown while the zone is idle.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// What the zone says when it refuses a payload.
    pub fn refusal(mut self, refusal: impl Into<SharedString>) -> Self {
        self.refusal = Some(refusal.into());
        self
    }

    /// Which payload kinds this zone handles. Anything else is refused.
    pub fn accepts<S: Into<SharedString>>(mut self, kinds: impl IntoIterator<Item = S>) -> Self {
        self.kinds = kinds.into_iter().map(Into::into).collect();
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Pins the zone to one state.
    ///
    /// A live drag decides the state otherwise. Pinning exists so all three
    /// can be reviewed side by side in a capture, which no single pointer
    /// position could produce.
    pub fn state(mut self, state: DropzoneState) -> Self {
        self.pinned = Some(state);
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn on_drop(mut self, handler: impl Fn(&DragItem, &mut Window, &mut App) + 'static) -> Self {
        self.on_drop = Some(Rc::new(handler));
        self
    }

    /// Reports paths the platform handed over when files were dropped.
    ///
    /// The paths are user-generated content, so they reach the handler and
    /// never the semantic tree.
    pub fn on_files(
        mut self,
        handler: impl Fn(&ExternalPaths, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_files = Some(Rc::new(handler));
        self
    }
}

impl Disableable for Dropzone {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for Dropzone {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        dnd::sync(cx);

        // The zone has to know whether the pointer is inside it before any
        // drop event arrives, and only prepaint knows where it ended up.
        let measured = measure::cell(&self.ident.semantic_id(), window, cx);
        let over = measured.get().contains(&window.mouse_position());
        let carried = dnd::active(window, cx).map(|drag| drag.item);
        let handles = |item: &DragItem| self.kinds.contains(&item.kind);

        let state = self.pinned.unwrap_or(match &carried {
            Some(item) if over && !self.disabled => {
                if handles(item) {
                    DropzoneState::Accepting
                } else {
                    DropzoneState::Refusing
                }
            }
            _ => DropzoneState::Idle,
        });

        let (mut border, text) = match state {
            DropzoneState::Idle => (theme.colors.hairline_strong, theme.colors.text_muted),
            DropzoneState::Accepting => (theme.colors.accent, theme.colors.text),
            DropzoneState::Refusing => (theme.colors.danger, theme.colors.danger),
        };
        if self.invalid {
            border = theme.colors.danger;
        }
        // The zone always says what it is. A refusal is a second line under
        // that, in the tone of a refusal, so a refusing zone is still
        // recognisably the same target and not an unlabelled red box.
        let message = self.label.clone();
        let refusal = (state == DropzoneState::Refusing).then(|| {
            self.refusal
                .clone()
                .unwrap_or_else(|| cx.strings().text(StringKey::DropzoneRefusal))
        });

        let mut zone = div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .h_full()
            .items_center()
            .justify_center()
            .gap_token(&theme, Space::Xs)
            .p_token(&theme, Space::Lg)
            .border(px(theme.borders.thick))
            .border_color(border)
            // A solid ring is what a disabled field wears. A drop target is
            // an opening, and the dashes are how every platform says so.
            .when(state == DropzoneState::Idle, |element| {
                element.border_dashed()
            })
            .radius(&theme, Radius::Card)
            .when(state == DropzoneState::Accepting, |element| {
                element.bg(theme
                    .colors
                    .accent
                    .opacity(theme.effects.selected_ring_alpha))
            })
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .children(self.icon.map(|glyph| {
                icon(glyph)
                    .size(px(theme.control.md.icon_size))
                    .text_color(text)
            }))
            .child(foundation_text(&theme, TypeScale::Label, message.clone()).text_color(text))
            .children(refusal.clone().map(|refusal| {
                foundation_text(&theme, TypeScale::Caption, refusal).text_color(theme.colors.danger)
            }))
            .children(
                self.hint
                    .clone()
                    .filter(|_| state != DropzoneState::Refusing)
                    .map(|hint| {
                        foundation_text(&theme, TypeScale::Caption, hint)
                            .text_tone(&theme, gpui_kit_theme::TextTone::Faint)
                    }),
            );

        let live = !self.disabled;

        // A platform file drag never passes through `on_drag`, so the zone
        // records the payload itself and everything downstream sees the same
        // session an in-application drag produces.
        if live {
            zone = zone.on_drag_move::<ExternalPaths>(|event, _window, cx| {
                let count = event.drag(cx).paths().len();
                dnd::adopt_external(count, cx);
            });
        }

        if let (true, Some(handler)) = (live, self.on_drop.clone()) {
            let kinds = self.kinds.clone();
            zone = zone
                .can_drop(move |payload, _, _| {
                    payload
                        .downcast_ref::<DragItem>()
                        .is_some_and(|item| kinds.contains(&item.kind))
                })
                .on_drop::<DragItem>(move |item, window, cx| {
                    dnd::finish(cx);
                    handler(item, window, cx);
                });
        }

        if let (true, Some(handler)) = (live, self.on_files.clone()) {
            zone = zone.on_drop::<ExternalPaths>(move |paths, window, cx| {
                dnd::finish(cx);
                handler(paths, window, cx);
            });
        }

        let zone = zone.semantic_in(
            cx,
            NodeSpec::new(self.ident.semantic_id(), Role::Region)
                .text(refusal.unwrap_or(message))
                .value(state.name())
                .disabled(self.disabled)
                .selected(state == DropzoneState::Accepting)
                .invalid(self.invalid || state == DropzoneState::Refusing),
        );

        // The measurement has to be of the zone itself, not of what it
        // contains, so it is taken one level up where the zone is the only
        // child.
        div()
            .column()
            .on_children_prepainted(move |bounds, window, _| {
                if let Some(first) = bounds.first() {
                    measure::record(&measured, *first, window);
                }
            })
            .child(zone)
    }
}

#[cfg(test)]
mod dropzone_phase_tests {
    use super::*;

    #[test]
    fn accepting_is_ready_and_refusing_is_unavailable() {
        assert_eq!(DropzoneState::Idle.phase(), Phase::Idle);
        assert_eq!(DropzoneState::Accepting.phase(), Phase::Ready);
        assert_eq!(DropzoneState::Refusing.phase(), Phase::Unavailable);
    }
}
