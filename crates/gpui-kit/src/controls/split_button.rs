//! A default action with a menu of alternatives beside it.

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, div, prelude::FluentBuilder,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::ControlSize;

use crate::controls::button::{Button, ButtonJoin, ButtonVariant, IconButton};
use crate::foundation::{Disableable, Ident, Sizable, StyledExt};
use crate::overlay::{Menu, MenuItem};

type ClickHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One action, plus the ones it stands in for.
///
/// The default action and the arrow are separate targets with separate ids,
/// because they do separate things: clicking the button acts, clicking the
/// arrow only offers. Refusing the whole control refuses both. Refusing only
/// the default action — [`SplitButton::default_disabled`] — leaves the arrow
/// working, because the alternatives may well be exactly what a typist needs
/// when the usual thing cannot be done.
pub struct SplitButton {
    ident: Ident,
    label: SharedString,
    icon: Option<Icon>,
    variant: ButtonVariant,
    size: ControlSize,
    disabled: bool,
    default_disabled: bool,
    focus_handle: FocusHandle,
    menu: Entity<Menu>,
    /// The paint already pushed into the menu trigger, so a frame that
    /// changes nothing does not ask for another frame.
    applied_trigger: Option<(ButtonVariant, ControlSize)>,
    on_click: Option<ClickHandler>,
}

impl std::fmt::Debug for SplitButton {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SplitButton")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("disabled", &self.disabled)
            .field("default_disabled", &self.default_disabled)
            .field("has_handler", &self.on_click.is_some())
            .finish()
    }
}

impl SplitButton {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let menu = cx.new(|cx| {
            Menu::new(ident.child("menu"), window, cx)
                .trigger_icon(Icon::AltArrowDown)
                .trigger_name("More actions")
                .trigger_join(ButtonJoin::Trailing)
        });
        Self {
            ident,
            label: SharedString::default(),
            icon: None,
            variant: ButtonVariant::Secondary,
            size: ControlSize::Md,
            disabled: false,
            default_disabled: false,
            focus_handle: cx.focus_handle(),
            menu,
            applied_trigger: None,
            on_click: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    pub fn icon(mut self, glyph: Icon) -> Self {
        self.icon = Some(glyph);
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(ButtonVariant::Primary)
    }

    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Secondary)
    }

    /// The alternatives. They are reported by the menu, as
    /// [`crate::overlay::MenuEvent::Invoked`].
    pub fn items(self, items: impl IntoIterator<Item = MenuItem>, cx: &mut Context<Self>) -> Self {
        let items = items.into_iter().collect();
        self.menu.update(cx, |menu, cx| menu.set_items(items, cx));
        self
    }

    /// What the arrow is named for a reader that cannot see it.
    pub fn menu_name(self, name: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        self.menu
            .update(cx, |menu, cx| menu.set_trigger_name(name, cx));
        self
    }

    /// Refuses the default action while leaving the alternatives reachable.
    pub fn default_disabled(mut self, disabled: bool) -> Self {
        self.default_disabled = disabled;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    pub fn menu(&self) -> &Entity<Menu> {
        &self.menu
    }

    pub fn is_open(&self, cx: &App) -> bool {
        self.menu.read(cx).is_open()
    }

    pub fn open_menu(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.menu.update(cx, |menu, cx| menu.open(window, cx));
    }

    pub fn set_disabled(&mut self, disabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.menu.update(cx, |menu, cx| menu.close(window, cx));
        }
        cx.notify();
    }
}

impl Disableable for SplitButton {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for SplitButton {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Focusable for SplitButton {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SplitButton {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let paint = (self.variant, self.size);
        if self.applied_trigger != Some(paint) {
            self.applied_trigger = Some(paint);
            self.menu
                .update(cx, |menu, cx| menu.set_trigger_style(paint.0, paint.1, cx));
        }
        let refused = self.disabled || self.default_disabled;
        let action = Button::new(self.ident.child("action"))
            .label(self.label.clone())
            .variant(self.variant)
            .control_size(self.size)
            .join(ButtonJoin::Leading)
            .semantic_parent(self.ident.semantic_id())
            .disabled(refused)
            .track_focus(&self.focus_handle)
            .when_some(self.icon, |button, glyph| button.icon(glyph))
            .when_some(self.on_click.clone(), |button, handler| {
                button.on_click(move |window, cx| handler(window, cx))
            });

        let arrow = if self.disabled {
            // A refused control still shows both of its targets, so what is
            // unavailable is visible rather than missing.
            IconButton::new(
                self.ident.child("menu").child("trigger"),
                Icon::AltArrowDown,
                SharedString::from("More actions"),
            )
            .variant(self.variant)
            .control_size(self.size)
            .join(ButtonJoin::Trailing)
            .semantic_parent(self.ident.semantic_id())
            .disabled(true)
            .into_any_element()
        } else {
            self.menu.clone().into_any_element()
        };

        div()
            .id(self.ident.element_id())
            .row()
            .flex_none()
            .items_start()
            .child(action)
            .child(arrow)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .disabled(self.disabled)
                    .text(self.label.clone()),
            )
    }
}
