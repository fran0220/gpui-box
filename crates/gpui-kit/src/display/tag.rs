//! A removable label for one item in a set the typist assembled.

use std::rc::Rc;

use gpui::{
    App, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};

use crate::display::badge::Tone;
use crate::foundation::{Disableable, FocusRing, Ident, Pressable, Selectable, StyledExt, text};
use crate::strings::{ActiveStrings, StringKey};

type RemoveHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A label with an optional remove action.
///
/// The remove handler is only installed when the tag is removable and enabled,
/// so a tag the host will not let go of shows no way to let go of it.
#[derive(IntoElement)]
pub struct Tag {
    ident: Ident,
    label: SharedString,
    tone: Tone,
    tint: Option<Hsla>,
    disabled: bool,
    selected: bool,
    on_remove: Option<RemoveHandler>,
}

impl std::fmt::Debug for Tag {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Tag")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("tone", &self.tone)
            .field("tinted", &self.tint.is_some())
            .field("removable", &self.on_remove.is_some())
            .finish()
    }
}

impl Tag {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            tone: Tone::Neutral,
            tint: None,
            disabled: false,
            selected: false,
            on_remove: None,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Paints the tag in a caller-owned colour, leaving the severity it
    /// reports alone. See [`Tone`].
    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn on_remove(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }
}

impl Disableable for Tag {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A tag the keyboard has singled out, which is not the same as a tag that is
/// gone: the selection is what the next keystroke would act on.
impl Selectable for Tag {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl RenderOnce for Tag {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let color = self.tone.mark_color(self.tint, &theme);
        let removable = !self.disabled && self.on_remove.is_some();
        let remove_ident = self.ident.child("remove");

        let remove = removable.then(|| {
            let handler = self.on_remove.clone().expect("checked above");
            let mut button = div()
                .id(remove_ident.element_id())
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .tab_index(0)
                .focus_ring(&theme)
                .pressable(cx)
                .child(icon(Icon::Close).size(px(10.0)).text_color(color))
                .semantic_in(
                    cx,
                    NodeSpec::new(remove_ident.semantic_id(), Role::Button)
                        .parent(self.ident.semantic_id())
                        .text(cx.strings().format(StringKey::TagRemove, &[&self.label])),
                );
            let click = Rc::clone(&handler);
            button
                .interactivity()
                .on_click(move |_, window, cx| click(window, cx));
            button
                .interactivity()
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        handler(window, cx);
                        cx.stop_propagation();
                    }
                });
            button
        });

        div()
            .flex()
            .flex_row()
            .items_center()
            .flex_none()
            .gap(px(theme.space(Space::Xs)))
            .px(px(theme.space(Space::Sm)))
            .py(px(2.0))
            .radius(&theme, Radius::Pill)
            // Selection is carried by the depth of the block rather than by an
            // outline drawn round it, so the two states differ by more than a
            // line a reader has to look for.
            .bg(color.opacity(if self.selected { 0.34 } else { 0.14 }))
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .child(text(&theme, TypeScale::Label, self.label.clone()).text_color(color))
            .children(remove)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Text)
                    .disabled(self.disabled)
                    .selected(self.selected)
                    .text(self.label.clone())
                    .value(self.tone.name()),
            )
    }
}
