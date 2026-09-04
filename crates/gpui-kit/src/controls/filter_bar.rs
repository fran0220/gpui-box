//! The conditions a list is currently filtered by, as chips that can be taken
//! off again.
//!
//! The conditions belong to the caller and so does the count. The bar renders
//! exactly what it was handed and reports every gesture, so a host that
//! refuses to drop a condition keeps showing it.

use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, TypeScale};

use crate::controls::button::Button;
use crate::display::badge::Tone;
use crate::display::loading::Spinner;
use crate::display::tag::Tag;
use crate::foundation::{Disableable, Ident, Sizable, StyledExt, text as foundation_text};
use crate::overlay::Tooltipped;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};

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
            // A condition is a value the caller typed, not a status: the
            // default is the neutral chip, and a tone is what a caller reaches
            // for when one condition means something the others do not.
            tone: Tone::Neutral,
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
    fn sentence(&self, noun: Option<&SharedString>, cx: &App) -> Option<SharedString> {
        match self {
            Self::Unknown => None,
            Self::Counting => Some(cx.strings().text(StringKey::FilterBarCounting)),
            Self::Known(count) => Some(match noun {
                Some(noun) => cx.strings().format(
                    StringKey::FilterBarResults,
                    &[cx.numbers().count(*count).as_ref(), noun.as_ref()],
                ),
                None => cx.strings().format_plural(
                    StringKey::FilterBarResultOne,
                    StringKey::FilterBarResultsMany,
                    cx.numbers().plural(*count),
                    &[cx.numbers().count(*count).as_ref()],
                ),
            }),
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
    noun: Option<SharedString>,
    add_control: Option<AnyElement>,
    add_label: Option<SharedString>,
    clear_label: Option<SharedString>,
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
            noun: None,
            add_control: None,
            add_label: None,
            clear_label: None,
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

    /// What is being counted, for the sentence beside the number. This is
    /// caller-owned copy and must already agree with every count the caller
    /// supplies. Without it, the catalogue selects the full localized result
    /// phrase by plural category.
    pub fn noun(mut self, noun: impl Into<SharedString>) -> Self {
        self.noun = Some(noun.into());
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
        self.add_label = Some(label.into());
        self
    }

    pub fn clear_label(mut self, label: impl Into<SharedString>) -> Self {
        self.clear_label = Some(label.into());
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
                        .label(
                            self.add_label
                                .clone()
                                .unwrap_or_else(|| cx.strings().text(StringKey::FilterBarAdd)),
                        )
                        // Adding a condition is what the bar is for; clearing
                        // is the way back out. Drawn at the same weight the
                        // reader has to work out which is which.
                        .secondary()
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
                    .label(
                        self.clear_label
                            .clone()
                            .unwrap_or_else(|| cx.strings().text(StringKey::FilterBarClear)),
                    )
                    .ghost()
                    .control_size(self.size)
                    .semantic_parent(self.ident.semantic_id())
                    .on_click(move |window, cx| handler(window, cx))
            });

        let count_ident = self.ident.child("count");
        let counting = self.count == ResultCount::Counting;
        let count = self.count.sentence(self.noun.as_ref(), cx).map(|sentence| {
            let wording = sentence.clone();
            let sentence = (!counting).then(|| {
                foundation_text(&theme, TypeScale::Caption, sentence)
                    .flex_none()
                    .text_color(match self.count {
                        ResultCount::Unavailable(_) => theme.colors.warning,
                        _ => theme.colors.text_muted,
                    })
            });
            // A count in progress is a loading state, and it wears what every
            // other loading state in the library wears. As bare text it was a
            // value that happened to be worded oddly.
            div()
                .row()
                .flex_none()
                .items_center()
                .gap_token(&theme, Space::Xs)
                .when(counting, |element| {
                    let spinner_ident = count_ident.child("spinner");
                    element.child(
                        div()
                            .id(spinner_ident.element_id())
                            .child(
                                Spinner::new(spinner_ident.clone()).control_size(ControlSize::Xs),
                            )
                            .tip(spinner_ident, wording.clone()),
                    )
                })
                .children(sentence)
                .semantic_in(
                    cx,
                    NodeSpec::new(count_ident.semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .text(wording)
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
            .items_center()
            .gap_token(&theme, Space::Sm)
            .px_token(&theme, Space::Sm)
            .py_token(&theme, Space::Xs)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            // A bar of chips with nothing at its head is a row of chips. The
            // mark says which row it is, and it is the only thing here a
            // reader cannot press.
            .child(
                gpui_kit_assets::icon(gpui_kit_assets::Icon::Filter)
                    .size(gpui::px(theme.control.get(self.size).icon_size))
                    .flex_none()
                    .text_color(theme.colors.text_faint),
            )
            .children(chips)
            .children(add)
            .child(div().flex_1())
            .children(count)
            .children(clear)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Toolbar)
                    .text(cx.strings().text(StringKey::FilterBarLabel))
                    .disabled(self.disabled)
                    .value(cx.numbers().count(active)),
            )
    }
}
