//! Key and value pairs for a detail page.
//!
//! An empty string is not a fact. A value nobody knows, a value that does not
//! apply here, and a value that exists but may not be shown are three
//! different sentences, so they are three different values — and a redacted
//! one publishes its shape and never its text.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px, relative,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};
use unicode_segmentation::UnicodeSegmentation;

use crate::foundation::{FocusRing, Ident, Pressable, StyledExt};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

type CopyHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// What a detail row holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptionValue {
    Text(SharedString),
    /// Nobody knows what this is. Not the same as empty.
    Unknown,
    /// The question does not arise for this record.
    NotApplicable,
    /// The value exists and may not be shown. Only its shape is carried, so
    /// there is no text here for a snapshot or an export to leak.
    Redacted(SharedString),
}

impl DescriptionValue {
    pub fn text(value: impl Into<SharedString>) -> Self {
        Self::Text(value.into())
    }

    /// A redacted value described by its shape, such as `"51 characters"`.
    ///
    /// The shape is all the component ever sees. Callers that hold the secret
    /// itself want [`DescriptionValue::redacted_from`], which measures the
    /// text and drops it.
    pub fn redacted(shape: impl Into<SharedString>) -> Self {
        Self::Redacted(shape.into())
    }

    /// Measures a secret and keeps only the measurement.
    ///
    /// The measurement is a sentence a reader reads, so it comes from the
    /// installed catalogue rather than from this file.
    pub fn redacted_from(secret: &str, cx: &App) -> Self {
        let count = secret.graphemes(true).count();
        Self::Redacted(cx.strings().format_plural(
            StringKey::DescriptionCharacterOne,
            StringKey::DescriptionCharacters,
            cx.numbers().plural(count),
            &[cx.numbers().count(count).as_ref()],
        ))
    }

    /// The name a semantic node publishes for the kind of value this is.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Unknown => "unknown",
            Self::NotApplicable => "not-applicable",
            Self::Redacted(_) => "redacted",
        }
    }

    /// What the node publishes as its value: the text when there is text to
    /// show, and the shape or the state when there is not.
    fn published(&self) -> SharedString {
        match self {
            Self::Text(value) => value.clone(),
            Self::Unknown => SharedString::new_static("unknown"),
            Self::NotApplicable => SharedString::new_static("not applicable"),
            Self::Redacted(shape) => SharedString::from(format!("redacted, {shape}")),
        }
    }

    /// Whether there is anything for the host to put on the clipboard.
    fn is_copyable(&self) -> bool {
        matches!(self, Self::Text(_) | Self::Redacted(_))
    }
}

impl From<SharedString> for DescriptionValue {
    fn from(value: SharedString) -> Self {
        Self::Text(value)
    }
}

impl From<&'static str> for DescriptionValue {
    fn from(value: &'static str) -> Self {
        Self::Text(SharedString::new_static(value))
    }
}

impl From<String> for DescriptionValue {
    fn from(value: String) -> Self {
        Self::Text(SharedString::from(value))
    }
}

/// One term and what it describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptionItem {
    id: SharedString,
    term: SharedString,
    value: DescriptionValue,
    copyable: bool,
}

impl DescriptionItem {
    pub fn new(
        id: impl Into<SharedString>,
        term: impl Into<SharedString>,
        value: impl Into<DescriptionValue>,
    ) -> Self {
        Self {
            id: id.into(),
            term: term.into(),
            value: value.into(),
            copyable: false,
        }
    }

    /// Offers a copy action, which reports the item and copies nothing itself.
    pub fn copyable(mut self, copyable: bool) -> Self {
        self.copyable = copyable;
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn value(&self) -> &DescriptionValue {
        &self.value
    }
}

/// A list of term and description pairs, in one or two columns.
#[derive(IntoElement)]
pub struct DescriptionList {
    ident: Ident,
    items: Vec<DescriptionItem>,
    columns: usize,
    on_copy: Option<CopyHandler>,
}

impl std::fmt::Debug for DescriptionList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DescriptionList")
            .field("ident", &self.ident)
            .field("items", &self.items.len())
            .field("columns", &self.columns)
            .finish()
    }
}

impl DescriptionList {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            items: Vec::new(),
            columns: 1,
            on_copy: None,
        }
    }

    pub fn item(mut self, item: DescriptionItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = DescriptionItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// One column or two. Anything else is one, because a detail page that
    /// needs three columns needs a table.
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = if columns >= 2 { 2 } else { 1 };
        self
    }

    /// Reports the item whose value the typist asked for. The list holds no
    /// clipboard of its own, and a redacted value's text never reaches it.
    pub fn on_copy(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_copy = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for DescriptionList {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let columns = self.columns;
        let count = self.items.len();

        let rows = self.items.into_iter().map(|item| {
            let ident = self.ident.child(item.id.as_ref());
            let copyable = item.copyable && item.value.is_copyable();
            let value = match &item.value {
                DescriptionValue::Text(text) => div()
                    .type_scale(&theme, TypeScale::Label)
                    .text_color(theme.colors.text)
                    .child(text.clone()),
                DescriptionValue::Unknown => div()
                    .type_scale(&theme, TypeScale::Label)
                    .text_color(theme.colors.text_faint)
                    .child(cx.strings().text(StringKey::DescriptionUnknown)),
                DescriptionValue::NotApplicable => div()
                    .type_scale(&theme, TypeScale::Label)
                    .text_color(theme.colors.text_faint)
                    .child(cx.strings().text(StringKey::DescriptionNotApplicable)),
                // The dots are the value: the text is not here to draw, and a
                // masked rendering of the real thing would still be the real
                // thing one screenshot away.
                DescriptionValue::Redacted(shape) => div()
                    .row()
                    .gap_token(&theme, Space::Sm)
                    .type_scale(&theme, TypeScale::Label)
                    .text_color(theme.colors.text_muted)
                    .child(SharedString::new_static("••••••••"))
                    .child(
                        div()
                            .type_scale(&theme, TypeScale::Caption)
                            .text_color(theme.colors.text_faint)
                            .child(shape.clone()),
                    ),
            };

            let copy = self.on_copy.clone().filter(|_| copyable).map(|handler| {
                let copy_ident = ident.child("copy");
                let id = item.id.clone();
                let name = cx
                    .strings()
                    .format(StringKey::DescriptionCopy, &[&item.term]);
                div()
                    .id(copy_ident.element_id())
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(theme.control.xs.height))
                    .radius(&theme, Radius::Small)
                    .cursor_pointer()
                    .tab_index(0)
                    .text_color(theme.colors.text_faint)
                    .hover(|style| style.bg(theme.colors.hover))
                    .pressable(cx)
                    .focus_ring(&theme)
                    .child(icon(Icon::Copy).size(px(theme.control.xs.icon_size)))
                    .on_click(move |_, window, cx| handler(id.clone(), window, cx))
                    .semantic_in(
                        cx,
                        NodeSpec::new(copy_ident.semantic_id(), Role::Button)
                            .parent(ident.semantic_id())
                            .text(name),
                    )
            });

            div()
                .row()
                .items_start()
                .gap_token(&theme, Space::Md)
                .py_token(&theme, Space::Xs)
                .when(columns == 2, |element| element.w(relative(0.5)).flex_none())
                .when(columns == 1, |element| element.w_full())
                .child(
                    div()
                        .w(px(140.0))
                        .flex_none()
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_muted)
                        .child(item.term.clone()),
                )
                .child(div().flex_1().min_w_0().child(value))
                .children(copy)
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Row)
                        .parent(self.ident.semantic_id())
                        .text(item.term.clone())
                        .value(item.value.published()),
                )
        });

        div()
            .w_full()
            .flex()
            .flex_row()
            .flex_wrap()
            .children(rows)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::List)
                    .value(cx.numbers().count(count)),
            )
    }
}
