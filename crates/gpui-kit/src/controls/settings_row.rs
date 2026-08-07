//! The row every preferences page is made of, and the section it sits in.
//!
//! A setting the typist cannot change is the thing settings pages get wrong.
//! `SettingsRow::managed` renders the value the policy holds, names who holds
//! it, and never renders the caller's control — so there is no handler to
//! install and nothing on screen that looks operable and is not.

use std::rc::Rc;

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, Theme, TypeScale};

use crate::display::badge::Badge;
use crate::foundation::{Ident, StyledExt};
use crate::strings::{ActiveStrings, StringKey};

/// Why a row is showing its value instead of its control.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Withheld {
    /// Held by policy: someone else decides this, and this is who.
    Managed(SharedString),
    /// The whole group it belongs to does not apply here.
    Inapplicable(SharedString),
}

impl Withheld {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Managed(_) => "managed",
            Self::Inapplicable(_) => "inapplicable",
        }
    }

    /// What the row says where its control would have been.
    ///
    /// An inapplicable row says only that it does not apply: the reason
    /// belongs to the whole section, which states it once above the rows
    /// rather than once per row.
    fn sentence(&self, cx: &App) -> SharedString {
        match self {
            Self::Managed(controller) => cx
                .strings()
                .format(StringKey::SettingsManagedBy, &[controller.as_ref()]),
            Self::Inapplicable(_) => cx.strings().text(StringKey::SettingsInapplicable),
        }
    }

    fn glyph(&self) -> Icon {
        match self {
            Self::Managed(_) => Icon::Key,
            Self::Inapplicable(_) => Icon::Info,
        }
    }
}

/// One setting: what it is called on the left, what it is set to on the right.
#[derive(IntoElement)]
pub struct SettingsRow {
    ident: Ident,
    label: SharedString,
    description: Option<SharedString>,
    badge: Option<SharedString>,
    /// What the setting currently holds, in the caller's words.
    value: Option<SharedString>,
    control: Option<AnyElement>,
    withheld: Option<Withheld>,
}

impl std::fmt::Debug for SettingsRow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SettingsRow")
            .field("ident", &self.ident)
            .field("label", &self.label)
            .field("badge", &self.badge)
            .field("withheld", &self.withheld)
            .field("has_control", &self.control.is_some())
            .finish()
    }
}

impl SettingsRow {
    pub fn new(ident: impl Into<Ident>, label: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            label: label.into(),
            description: None,
            badge: None,
            value: None,
            control: None,
            withheld: None,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A short note beside the label, such as "Requires restart".
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// What the setting holds, published whether or not the control is drawn.
    /// A managed row has nothing else to show, so it is the only reading the
    /// typist gets.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }

    /// Marks the setting as decided elsewhere, and by whom.
    ///
    /// The control is not rendered at all — a managed setting installs no
    /// handler because there is nothing there to install one on — and the row
    /// states the value and its controller instead.
    pub fn managed(mut self, controller: impl Into<SharedString>) -> Self {
        self.withheld = Some(Withheld::Managed(controller.into()));
        self
    }

    fn inapplicable(mut self, reason: SharedString) -> Self {
        if self.withheld.is_none() {
            self.withheld = Some(Withheld::Inapplicable(reason));
        }
        self
    }

    fn render_in(self, theme: &Theme, first: bool, cx: &mut App) -> AnyElement {
        let withheld = self.withheld.clone();
        let ident = self.ident.clone();

        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Row).text(self.label.clone());
        if let Some(value) = self.value.clone() {
            spec = spec.value(value);
        }
        if withheld.is_some() {
            spec = spec.disabled(true);
        }

        let names = div()
            .column()
            .flex_1()
            .min_w_0()
            .gap(px(2.0))
            .child(
                div()
                    .row()
                    .gap_token(theme, Space::Sm)
                    .type_scale(theme, TypeScale::Label)
                    .text_color(theme.colors.text)
                    .child(self.label.clone())
                    .children(
                        self.badge
                            .clone()
                            .map(|badge| Badge::new(badge).id(ident.child("badge")).warning()),
                    ),
            )
            .children(self.description.clone().map(|description| {
                div()
                    .type_scale(theme, TypeScale::Caption)
                    .text_color(theme.colors.text_muted)
                    .child(description)
            }));

        // A withheld row shows what is set and who set it. The control never
        // reaches the tree, so nothing can be operated by mistake.
        let right = match (&withheld, self.control) {
            (Some(withheld), _) => div()
                .column()
                .items_end()
                .flex_none()
                .gap(px(2.0))
                .children(self.value.clone().map(|value| {
                    div()
                        .type_scale(theme, TypeScale::Label)
                        .text_color(theme.colors.text_muted)
                        .child(value)
                }))
                .child(
                    div()
                        .row()
                        .gap(px(theme.space(Space::Xs)))
                        .type_scale(theme, TypeScale::Caption)
                        .text_color(theme.colors.text_faint)
                        .child(
                            icon(withheld.glyph())
                                .size(px(theme.control.xs.icon_size))
                                .text_color(theme.colors.text_faint),
                        )
                        .child(withheld.sentence(cx))
                        .semantic_in(
                            cx,
                            NodeSpec::new(ident.child("managed").semantic_id(), Role::Status)
                                .parent(ident.semantic_id())
                                .text(withheld.sentence(cx))
                                .value(withheld.as_str()),
                        ),
                )
                .into_any_element(),
            (None, Some(control)) => div().flex_none().child(control).into_any_element(),
            (None, None) => div()
                .flex_none()
                .children(self.value.clone().map(|value| {
                    div()
                        .type_scale(theme, TypeScale::Label)
                        .text_color(theme.colors.text_muted)
                        .child(value)
                }))
                .into_any_element(),
        };

        div()
            .row()
            .w_full()
            .items_center()
            .gap_token(theme, Space::Md)
            .px_token(theme, Space::Lg)
            .py_token(theme, Space::Md)
            .when(!first, |element| {
                element
                    .border_t(px(theme.borders.hairline))
                    .border_color(theme.colors.hairline)
            })
            .child(names)
            .child(right)
            .semantic_in(cx, spec)
            .into_any_element()
    }
}

impl RenderOnce for SettingsRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        self.render_in(&theme, true, cx)
    }
}

type ActionSlot = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// A headed group of settings rows.
#[derive(IntoElement)]
pub struct SettingsSection {
    ident: Ident,
    title: SharedString,
    description: Option<SharedString>,
    dimmed: Option<SharedString>,
    rows: Vec<SettingsRow>,
    action: Option<ActionSlot>,
}

impl std::fmt::Debug for SettingsSection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SettingsSection")
            .field("ident", &self.ident)
            .field("title", &self.title)
            .field("dimmed", &self.dimmed)
            .field("rows", &self.rows.len())
            .finish()
    }
}

impl SettingsSection {
    pub fn new(ident: impl Into<Ident>, title: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            title: title.into(),
            description: None,
            dimmed: None,
            rows: Vec::new(),
            action: None,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// States that nothing in this group applies here, and why.
    ///
    /// A dimmed group renders none of its controls, for the same reason a
    /// managed row renders none of its own: a setting that cannot take effect
    /// must not look as though it can.
    pub fn dimmed_by(mut self, reason: impl Into<SharedString>) -> Self {
        self.dimmed = Some(reason.into());
        self
    }

    pub fn row(mut self, row: SettingsRow) -> Self {
        self.rows.push(row);
        self
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = SettingsRow>) -> Self {
        self.rows.extend(rows);
        self
    }

    /// A control in the section heading, such as "Reset to defaults".
    pub fn action(
        mut self,
        action: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.action = Some(Rc::new(action));
        self
    }
}

impl RenderOnce for SettingsSection {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let dimmed = self.dimmed.clone();
        let ident = self.ident.clone();

        let heading = div()
            .row()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .child(
                div()
                    .column()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Label)
                            .text_color(theme.colors.text)
                            .child(self.title.clone()),
                    )
                    .children(self.description.clone().map(|description| {
                        div()
                            .type_scale(&theme, TypeScale::Caption)
                            .text_color(theme.colors.text_muted)
                            .child(description)
                    })),
            )
            .children(
                self.action
                    .as_ref()
                    .filter(|_| dimmed.is_none())
                    .map(|action| action(window, cx)),
            );

        let reason = dimmed.clone().map(|reason| {
            div()
                .row()
                .w_full()
                .gap_token(&theme, Space::Xs)
                .type_scale(&theme, TypeScale::Caption)
                .text_color(theme.colors.text_faint)
                .child(
                    icon(Icon::Info)
                        .size(px(theme.control.xs.icon_size))
                        .text_color(theme.colors.text_faint),
                )
                .child(reason.clone())
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child("dimmed").semantic_id(), Role::Status)
                        .parent(ident.semantic_id())
                        .text(reason)
                        .value("inapplicable"),
                )
        });

        let rows = self.rows.into_iter().enumerate().map(|(index, row)| {
            let row = match dimmed.clone() {
                Some(reason) => row.inapplicable(reason),
                None => row,
            };
            row.render_in(&theme, index == 0, cx)
        });

        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .child(heading)
            .children(reason)
            .child(
                div()
                    .column()
                    .w_full()
                    .radius(&theme, Radius::Card)
                    .hairline(&theme)
                    .surface(&theme, Surface::Panel)
                    .overflow_hidden()
                    .when(dimmed.is_some(), |element| {
                        element.opacity(theme.opacity.disabled)
                    })
                    .children(rows),
            )
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Group)
                    .text(self.title.clone())
                    .disabled(dimmed.is_some()),
            )
    }
}
