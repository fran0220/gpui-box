//! A two-pane assignment surface with controlled movement intents.
//!
//! `TransferList` does not move records between panes. It renders the source
//! and target sets supplied by the caller, exposes independent selection and
//! filtering, and reports move requests with stable item identities.

use gpui::{
    AppContext as _, Context, Entity, EventEmitter, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, SharedString, Styled, Subscription, Window, div, prelude::FluentBuilder,
    px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Surface, TextTone, TypeScale};

use crate::controls::button::IconButton;
use crate::controls::field::{FieldState, field_shell};
use crate::controls::input::{TextInput, TextInputEvent};
use crate::foundation::{
    Disableable, FocusRing, Hoverable, Ident, Pressable, SelectedFill, Sizable, StyledExt, text,
};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

/// An item in one side of a transfer list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferItem {
    pub id: SharedString,
    pub label: SharedString,
    pub disabled: bool,
}

impl TransferItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// What a transfer list reports. The caller performs the actual assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferListEvent {
    ToggleSource(SharedString),
    ToggleTarget(SharedString),
    MoveToTarget,
    MoveToSource,
    QueryChanged(SharedString),
}

impl EventEmitter<TransferListEvent> for TransferList {}

/// A controlled source/target assignment control.
pub struct TransferList {
    ident: Ident,
    source: Vec<TransferItem>,
    target: Vec<TransferItem>,
    source_selected: Vec<SharedString>,
    target_selected: Vec<SharedString>,
    query: Entity<TextInput>,
    source_label: SharedString,
    target_label: SharedString,
    size: ControlSize,
    disabled: bool,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for TransferList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TransferList")
            .field("ident", &self.ident)
            .field("source", &self.source.len())
            .field("target", &self.target.len())
            .field("query", &self.query)
            .finish()
    }
}

impl TransferList {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let query = cx.new(|cx| TextInput::new(ident.child("query"), window, cx).bare(true));
        let subscription = cx.subscribe(&query, |_list, _query, event, cx| {
            if let TextInputEvent::Change(query) = event {
                cx.emit(TransferListEvent::QueryChanged(query.clone()));
            }
        });
        Self {
            ident,
            source: Vec::new(),
            target: Vec::new(),
            source_selected: Vec::new(),
            target_selected: Vec::new(),
            query,
            source_label: SharedString::default(),
            target_label: SharedString::default(),
            size: ControlSize::Md,
            disabled: false,
            _subscriptions: vec![subscription],
        }
    }

    pub fn source(mut self, items: impl IntoIterator<Item = TransferItem>) -> Self {
        self.source = items.into_iter().collect();
        self
    }

    pub fn target(mut self, items: impl IntoIterator<Item = TransferItem>) -> Self {
        self.target = items.into_iter().collect();
        self
    }

    pub fn source_selected(
        mut self,
        ids: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.source_selected = ids.into_iter().map(Into::into).collect();
        self
    }

    pub fn target_selected(
        mut self,
        ids: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.target_selected = ids.into_iter().map(Into::into).collect();
        self
    }

    pub fn source_label(mut self, label: impl Into<SharedString>) -> Self {
        self.source_label = label.into();
        self
    }

    pub fn target_label(mut self, label: impl Into<SharedString>) -> Self {
        self.target_label = label.into();
        self
    }

    pub fn set_query(&mut self, query: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.query
            .update(cx, |field, cx| field.set_text_quietly(query.into(), cx));
        cx.notify();
    }

    fn matches(item: &TransferItem, query: &str) -> bool {
        query.trim().is_empty()
            || item
                .label
                .to_string()
                .to_lowercase()
                .contains(&query.to_lowercase())
    }

    fn pane(
        &self,
        name: &str,
        label: &SharedString,
        items: &[TransferItem],
        selected: &[SharedString],
        query: &str,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = cx.theme().clone();
        let pane_id = self.ident.child(name);
        let visible: Vec<&TransferItem> = items
            .iter()
            .filter(|item| Self::matches(item, query))
            .collect();
        let visible_count = visible.len();
        let list_id = pane_id.child("list");
        let mut list = div()
            .id(list_id.element_id())
            .column()
            .gap(px(theme.space(Space::Xxs)))
            .min_h(px(theme.control.get(self.size).height * 3.0));
        for item in visible {
            let item_id = item.id.clone();
            let selected_item = selected.iter().any(|id| id == &item.id);
            let item_ident = list_id.child(item.id.as_ref());
            let disabled = self.disabled || item.disabled;
            let mut row = div()
                .id(item_ident.element_id())
                .row()
                .items_center()
                .gap(px(theme.space(Space::Sm)))
                .px(px(theme.space(Space::Sm)))
                .py(px(theme.space(Space::Xs)))
                .radius(&theme, Radius::Control)
                .selected_fill(&theme, selected_item)
                // A row that answers a click answers the pointer that is over
                // it and can be reached without one, the same way every other
                // pickable row in the library can. It publishes `Role::Option`
                // either way, so a keyboard that could not reach it was the
                // row promising a reader something it did not deliver. The
                // selected wash already occupies the row, so only an unselected
                // one takes the hover step.
                .when(!disabled, |element| {
                    element
                        .cursor_pointer()
                        .tab_index(0)
                        .pressable(cx)
                        .focus_ring(&theme)
                        .when(!selected_item, |element| element.hover_row(&theme))
                })
                .child(
                    text(&theme, TypeScale::Body, item.label.clone()).text_color(if disabled {
                        theme.colors.text_disabled
                    } else {
                        theme.colors.text
                    }),
                )
                .semantic_in(
                    cx,
                    NodeSpec::new(item_ident.semantic_id(), Role::Option)
                        .parent(list_id.semantic_id())
                        .text(item.label.clone())
                        .selected(selected_item)
                        .disabled(disabled),
                );
            if !disabled {
                let event = if name == "source" {
                    TransferListEvent::ToggleSource(item_id)
                } else {
                    TransferListEvent::ToggleTarget(item_id)
                };
                row = row.on_mouse_down(MouseButton::Left, {
                    let event = event.clone();
                    cx.listener(move |_list, _, _, cx| cx.emit(event.clone()))
                });
                // The row is a tab stop that publishes `Role::Option`, so a
                // keyboard that could reach it and not toggle it was the row
                // promising a reader something it did not deliver.
                row = row.on_key_down({
                    let event = event.clone();
                    cx.listener(move |_list, key: &gpui::KeyDownEvent, _, cx| {
                        if matches!(key.keystroke.key.as_str(), "enter" | "space") {
                            cx.emit(event.clone());
                            cx.stop_propagation();
                        }
                    })
                });
            }
            list = list.child(row);
        }
        div()
            .id(pane_id.element_id())
            .column()
            .flex_1()
            .min_w(px(0.0))
            .gap(px(theme.space(Space::Sm)))
            .p(px(theme.space(Space::Sm)))
            .surface(&theme, Surface::Panel)
            .radius(&theme, Radius::Card)
            .child(
                div()
                    .row()
                    .justify_between()
                    .items_center()
                    .child(text(&theme, TypeScale::Label, label.clone()))
                    .child(
                        text(
                            &theme,
                            TypeScale::Caption,
                            cx.numbers().count(visible_count),
                        )
                        .text_tone(&theme, TextTone::Muted),
                    ),
            )
            .child(list.semantic_in(
                cx,
                NodeSpec::new(list_id.semantic_id(), Role::List).parent(pane_id.semantic_id()),
            ))
            .into_any_element()
    }
}

impl Disableable for TransferList {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for TransferList {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Render for TransferList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let metrics = theme.control.get(self.size);
        let query = self.query.read(cx).value().clone();
        self.query.update(cx, |field, cx| {
            field.set_placeholder(cx.strings().text(StringKey::TransferSearch), cx);
            field.set_name(cx.strings().text(StringKey::TransferSearch), cx);
            field.set_disabled(self.disabled, cx);
        });
        let source_selected = !self.source_selected.is_empty();
        let target_selected = !self.target_selected.is_empty();
        let move_to_target = source_selected && !self.disabled;
        let move_to_source = target_selected && !self.disabled;
        let source_label = if self.source_label.is_empty() {
            cx.strings().text(StringKey::TransferAvailable)
        } else {
            self.source_label.clone()
        };
        let target_label = if self.target_label.is_empty() {
            cx.strings().text(StringKey::TransferSelected)
        } else {
            self.target_label.clone()
        };
        let source = self.pane(
            "source",
            &source_label,
            &self.source,
            &self.source_selected,
            query.as_ref(),
            cx,
        );
        let target = self.pane(
            "target",
            &target_label,
            &self.target,
            &self.target_selected,
            query.as_ref(),
            cx,
        );
        let entity = cx.entity();
        let to_target = IconButton::new(
            self.ident.child("move-to-target"),
            gpui_kit_assets::Icon::AltArrowRight,
            cx.strings().text(StringKey::TransferMoveToTarget),
        )
        .secondary()
        .disabled(!move_to_target)
        .on_click(move |_, cx| {
            entity.update(cx, |_list, cx| cx.emit(TransferListEvent::MoveToTarget));
        });
        let entity = cx.entity();
        let to_source = IconButton::new(
            self.ident.child("move-to-source"),
            gpui_kit_assets::Icon::AltArrowLeft,
            cx.strings().text(StringKey::TransferMoveToSource),
        )
        .secondary()
        .disabled(!move_to_source)
        .on_click(move |_, cx| {
            entity.update(cx, |_list, cx| cx.emit(TransferListEvent::MoveToSource));
        });

        div()
            .id(self.ident.element_id())
            .column()
            .gap(px(theme.space(Space::Md)))
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .child(
                field_shell(
                    &theme,
                    self.size,
                    FieldState::default().disabled(self.disabled),
                )
                .id(self.ident.child("search").element_id())
                .min_h(px(metrics.height))
                .child(self.query.clone()),
            )
            .child(
                div()
                    .row()
                    .items_center()
                    .gap(px(theme.space(Space::Md)))
                    .child(source)
                    .child(
                        div()
                            .column()
                            .gap(px(theme.space(Space::Xs)))
                            .items_center()
                            .child(to_target)
                            .child(to_source),
                    )
                    .child(target),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .value(cx.numbers().count(self.source.len() + self.target.len()))
                    .disabled(self.disabled),
            )
    }
}
