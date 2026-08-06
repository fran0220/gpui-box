//! The conditions a list is currently filtered by, as chips that can be taken
//! off again.
//!
//! The conditions belong to the caller and so does the count. The bar renders
//! exactly what it was handed and reports every gesture, so a host that
//! refuses to drop a condition keeps showing it.

use std::rc::Rc;

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, TypeScale};

use crate::controls::button::Button;
use crate::display::badge::Tone;
use crate::display::tag::Tag;
use crate::foundation::{Disableable, Ident, Sizable, StyledExt};

type RemoveHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type ClearHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type AddHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One condition the list is narrowed by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterCondition {
    id: SharedString,
    field: SharedString,
    operator: SharedString,
    value: SharedString,
    tone: Tone,
}

impl FilterCondition {
    pub fn new(
        id: impl Into<SharedString>,
        field: impl Into<SharedString>,
        operator: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            field: field.into(),
            operator: operator.into(),
            value: value.into(),
            tone: Tone::Accent,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// How the condition reads as one line: field, operator, value.
    pub fn label(&self) -> SharedString {
        SharedString::from(format!("{} {} {}", self.field, self.operator, self.value))
    }
}

/// How many rows the host says the filter matched.
///
/// Counting is not zero. A bar that renders an unfinished count as "0 results"
/// tells the typist their filter matched nothing, which nobody has established
/// yet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ResultCount {
    /// Nothing has been counted, and nothing is claimed.
    #[default]
    Unknown,
    /// The host is counting right now.
    Counting,
    /// The host counted, and this is the number.
    Known(usize),
    /// The host could not count, in its own words.
    Unavailable(SharedString),
}

impl ResultCount {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Counting => "counting",
            Self::Known(_) => "known",
            Self::Unavailable(_) => "unavailable",
        }
    }

    /// The words the bar shows, and the value it publishes.
    fn sentence(&self, noun: &SharedString) -> Option<SharedString> {
        match self {
            Self::Unknown => None,
            Self::Counting => Some(SharedString::new_static("Counting…")),
            Self::Known(count) => Some(SharedString::from(format!("{count} {noun}"))),
            Self::Unavailable(reason) => Some(reason.clone()),
        }
    }
}

/// A row of active conditions, a way to add one, a clear-all, and the count.
#[derive(IntoElement)]
pub struct FilterBar {
    ident: Ident,
    conditions: Vec<FilterCondition>,
    count: ResultCount,
    noun: SharedString,
    add_control: Option<AnyElement>,
    add_label: SharedString,
    clear_label: SharedString,
    size: gpui_kit_theme::ControlSize,
    disabled: bool,
    on_add: Option<AddHandler>,
    on_remove: Option<RemoveHandler>,
    on_clear: Option<ClearHandler>,
}

impl std::fmt::Debug for FilterBar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FilterBar")
            .field("ident", &self.ident)
            .field("conditions", &self.conditions.len())
            .field("count", &self.count)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl FilterBar {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            conditions: Vec::new(),
            count: ResultCount::default(),
            noun: SharedString::new_static("results"),
            add_control: None,
            add_label: SharedString::new_static("Add filter"),
            clear_label: SharedString::new_static("Clear all"),
            size: gpui_kit_theme::ControlSize::Sm,
            disabled: false,
            on_add: None,
            on_remove: None,
            on_clear: None,
        }
    }

    pub fn condition(mut self, condition: FilterCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    pub fn conditions(mut self, conditions: impl IntoIterator<Item = FilterCondition>) -> Self {
        self.conditions.extend(conditions);
        self
    }

    /// The count the host established. The bar never counts anything itself.
    pub fn count(mut self, count: ResultCount) -> Self {
        self.count = count;
        self
    }

    /// What is being counted, for the sentence beside the number.
    pub fn noun(mut self, noun: impl Into<SharedString>) -> Self {
        self.noun = noun.into();
        self
    }

    /// The affordance that offers the fields a filter can be built on, usually
    /// a [`crate::overlay::Popover`] or a [`crate::overlay::Menu`]. It takes
    /// the place of the built-in button.
    pub fn add_control(mut self, control: impl IntoElement) -> Self {
        self.add_control = Some(control.into_any_element());
        self
    }

    pub fn add_label(mut self, label: impl Into<SharedString>) -> Self {
        self.add_label = label.into();
        self
    }

    pub fn clear_label(mut self, label: impl Into<SharedString>) -> Self {
        self.clear_label = label.into();
        self
    }

    /// Reports that the typist asked to add a condition. Ignored when the
    /// caller supplied their own affordance with [`FilterBar::add_control`].
    pub fn on_add(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_add = Some(Rc::new(handler));
        self
    }

    /// Reports the condition the typist took off. The bar removes nothing.
    pub fn on_remove(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }

    pub fn on_clear(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_clear = Some(Rc::new(handler));
        self
    }
}

impl Disableable for FilterBar {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Sizable for FilterBar {
    fn control_size(mut self, size: gpui_kit_theme::ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for FilterBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let active = self.conditions.len();
        let removable = self.on_remove.clone().filter(|_| !self.disabled);

        let chips = self.conditions.iter().map(|condition| {
            let ident = self.ident.child(condition.id.as_ref());
            let mut chip = Tag::new(ident, condition.label())
                .tone(condition.tone)
                .disabled(self.disabled);
            if let Some(handler) = removable.clone() {
                let id = condition.id.clone();
                chip = chip.on_remove(move |window, cx| handler(id.clone(), window, cx));
            }
            chip
        });

        let add = match self.add_control {
            Some(control) => Some(div().flex_none().child(control).into_any_element()),
            None => self
                .on_add
                .clone()
                .filter(|_| !self.disabled)
                .map(|handler| {
                    Button::new(self.ident.child("add"))
                        .label(self.add_label.clone())
                        .ghost()
                        .control_size(self.size)
                        .semantic_parent(self.ident.semantic_id())
                        .icon(gpui_kit_assets::Icon::Plus)
                        .on_click(move |window, cx| handler(window, cx))
                        .into_any_element()
                }),
        };

        let clear = self
            .on_clear
            .clone()
            .filter(|_| !self.disabled && active > 0)
            .map(|handler| {
                Button::new(self.ident.child("clear"))
                    .label(self.clear_label.clone())
                    .ghost()
                    .control_size(self.size)
                    .semantic_parent(self.ident.semantic_id())
                    .on_click(move |window, cx| handler(window, cx))
            });

        let count_ident = self.ident.child("count");
        let count = self.count.sentence(&self.noun).map(|sentence| {
            div()
                .flex_none()
                .type_scale(&theme, TypeScale::Caption)
                .text_color(match self.count {
                    ResultCount::Unavailable(_) => theme.colors.warning,
                    ResultCount::Counting => theme.colors.text_faint,
                    _ => theme.colors.text_muted,
                })
                .child(sentence.clone())
                .semantic_in(
                    cx,
                    NodeSpec::new(count_ident.semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .text(sentence)
                        // The state is published by name so a test tells a
                        // count in progress from a count of zero.
                        .value(self.count.as_str())
                        .busy(self.count == ResultCount::Counting),
                )
        });

        div()
            .row()
            .w_full()
            .flex_wrap()
            .gap_token(&theme, Space::Sm)
            .px_token(&theme, Space::Sm)
            .py_token(&theme, Space::Xs)
            .radius(&theme, Radius::Card)
            .hairline(&theme)
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .children(chips)
            .children(add)
            .child(div().flex_1())
            .children(count)
            .children(clear)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Toolbar)
                    .text("Filters")
                    .disabled(self.disabled)
                    .value(active.to_string()),
            )
    }
}
