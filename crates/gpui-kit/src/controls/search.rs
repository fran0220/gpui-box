//! Find, and find and replace, over text this crate does not hold.
//!
//! [`SearchField`] is a query field, a hit count, and a way to step between
//! hits. [`FindReplace`] puts a replacement field and two replace actions on
//! top of the same field.
//!
//! Neither searches anything. The text belongs to the caller, the matching
//! rules belong to the caller, and so does the answer; the components report
//! what the typist asked for and render the count the host established. See
//! [`crate::display::highlight`] for the marking side of the same boundary and
//! for exactly what a caller owes.

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled, Subscription,
    Window, div,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Elevation, Radius, Space, Surface, TypeScale};

use crate::controls::button::{Button, IconButton};
use crate::controls::input::{TextInput, TextInputEvent};
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{
    Disableable, Ident, Selectable, Sizable, StyledExt, text as foundation_text,
};
use crate::strings::{ActiveStrings, StringKey};

/// How many hits the host says the query has.
///
/// Counting is not zero, and a count that stopped early is not a count. A
/// field that renders an unfinished search as "No results" tells the typist
/// their query matched nothing, which nobody has established yet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum HitCount {
    /// Nothing has been searched, so nothing is claimed. A field with an
    /// empty query sits here.
    #[default]
    Unsearched,
    /// The host is looking right now.
    Counting,
    /// The host looked and found nothing.
    None,
    /// The host counted. `current` is the hit the typist is on, zero-based,
    /// or `None` when the hits are known and no particular one is current.
    Known {
        total: usize,
        current: Option<usize>,
    },
    /// The host stopped counting at `counted` and there are more.
    ///
    /// Distinct from [`HitCount::Known`] because "at least 500" is not 500,
    /// and a replace-all that trusted it would change a number nobody knows.
    TooMany { counted: usize },
    /// The host could not search, in its own words.
    Unavailable(SharedString),
}

impl HitCount {
    /// The name the semantic tree publishes, so a test tells the five apart.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Unsearched => "unsearched",
            Self::Counting => "counting",
            Self::None => "none",
            Self::Known { .. } => "known",
            Self::TooMany { .. } => "too-many",
            Self::Unavailable(_) => "unavailable",
        }
    }

    /// How many hits this claims exactly, which is only ever a counted one.
    pub fn exact(&self) -> Option<usize> {
        match self {
            Self::Known { total, .. } => Some(*total),
            _ => None,
        }
    }

    /// Whether there is anywhere for next and previous to go.
    fn steppable(&self) -> bool {
        match self {
            Self::Known { total, .. } => *total > 0,
            Self::TooMany { .. } => true,
            _ => false,
        }
    }

    /// The words beside the field.
    fn sentence(&self, cx: &App) -> SharedString {
        let strings = cx.strings();
        match self {
            Self::Unsearched => strings.text(StringKey::SearchNotSearched),
            Self::Counting => strings.text(StringKey::SearchCounting),
            Self::None => strings.text(StringKey::SearchNoHits),
            Self::Known {
                total,
                current: Some(current),
            } => strings.format(
                StringKey::CountOfTotal,
                &[&(current + 1).to_string(), &total.to_string()],
            ),
            Self::Known {
                total: 1,
                current: None,
            } => strings.text(StringKey::SearchHitOne),
            Self::Known {
                total,
                current: None,
            } => strings.format(StringKey::SearchHitMany, &[&total.to_string()]),
            Self::TooMany { counted } => {
                strings.format(StringKey::SearchTooMany, &[&counted.to_string()])
            }
            // The host's own words outrank the catalogue's.
            Self::Unavailable(reason) => reason.clone(),
        }
    }
}

/// What a search field reports. It applies none of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchFieldEvent {
    QueryChanged(SharedString),
    /// Enter, or the next control.
    Next,
    /// Shift-enter, or the previous control.
    Previous,
    /// Escape. The host decides whether that closes the field.
    Cancelled,
    MatchCaseToggled(bool),
    WholeWordToggled(bool),
}

impl EventEmitter<SearchFieldEvent> for SearchField {}

/// A query field, a hit count, and next and previous.
pub struct SearchField {
    ident: Ident,
    focus_handle: FocusHandle,
    query: Entity<TextInput>,
    count: HitCount,
    placeholder: Option<SharedString>,
    size: ControlSize,
    disabled: bool,
    match_case: Option<bool>,
    whole_word: Option<bool>,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for SearchField {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SearchField")
            .field("ident", &self.ident)
            .field("count", &self.count)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl SearchField {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let query = cx.new(|cx| TextInput::new(ident.child("query"), window, cx).bare(true));
        let subscription = cx.subscribe(&query, |_field, _query, event, cx| match event {
            TextInputEvent::Change(text) => {
                cx.emit(SearchFieldEvent::QueryChanged(text.clone()));
            }
            // Enter steps forward, which is what every find field does. The
            // field steps nothing itself; the host owns where the caret is.
            TextInputEvent::Submit => cx.emit(SearchFieldEvent::Next),
            TextInputEvent::Cancel => cx.emit(SearchFieldEvent::Cancelled),
            _ => {}
        });

        Self {
            ident,
            focus_handle: cx.focus_handle(),
            query,
            count: HitCount::default(),
            placeholder: None,
            size: ControlSize::Sm,
            disabled: false,
            match_case: None,
            whole_word: None,
            _subscriptions: vec![subscription],
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Offers a match-case toggle in the state the host holds. Without a call
    /// there is no such control, because a control over a rule the host does
    /// not implement would report a change nothing acts on.
    pub fn match_case(mut self, on: bool) -> Self {
        self.match_case = Some(on);
        self
    }

    /// Offers a whole-word toggle in the state the host holds.
    pub fn whole_word(mut self, on: bool) -> Self {
        self.whole_word = Some(on);
        self
    }

    /// The count the host established. The field counts nothing itself.
    pub fn set_count(&mut self, count: HitCount, cx: &mut Context<Self>) {
        self.count = count;
        cx.notify();
    }

    pub fn count(&self) -> &HitCount {
        &self.count
    }

    pub fn query_text(&self, cx: &App) -> SharedString {
        self.query.read(cx).value().clone()
    }

    pub fn query_input(&self) -> &Entity<TextInput> {
        &self.query
    }

    pub fn set_query(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.query.update(cx, |query, cx| query.set_value(text, cx));
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.query
            .update(cx, |query, cx| query.set_disabled(disabled, cx));
        cx.notify();
    }

    /// Puts the keyboard in the query field, which is where a find surface
    /// that has just opened expects it.
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.query.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    }

    fn step_control(
        &self,
        suffix: &str,
        glyph: Icon,
        key: StringKey,
        event: SearchFieldEvent,
        cx: &mut Context<Self>,
    ) -> IconButton {
        let ident = self.ident.child(suffix);
        let name = cx.strings().text(key);
        // A step with nowhere to go installs no handler at all, which is the
        // same refusal `Pagination` makes at the ends of a run.
        let live = !self.disabled && self.count.steppable();
        let mut control = IconButton::new(ident, glyph, name)
            .semantic_parent(self.ident.semantic_id())
            .control_size(self.size)
            .disabled(!live);
        if live {
            let field = cx.entity().downgrade();
            control = control.on_click(move |_, cx| {
                let event = event.clone();
                field.update(cx, |_, cx| cx.emit(event)).ok();
            });
        }
        control
    }
}

impl Focusable for SearchField {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SearchField {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let count_ident = self.ident.child("count");
        let sentence = self.count.sentence(cx);

        let toggles = [
            (
                "match-case",
                self.match_case,
                StringKey::SearchCaseSensitive,
                Icon::Pen,
                true,
            ),
            (
                "whole-word",
                self.whole_word,
                StringKey::SearchWholeWord,
                Icon::List,
                false,
            ),
        ]
        .into_iter()
        .filter_map(|(suffix, state, key, glyph, is_case)| {
            let on = state?;
            let ident = self.ident.child(suffix);
            let name = cx.strings().text(key);
            let mut control = IconButton::new(ident, glyph, name)
                .semantic_parent(self.ident.semantic_id())
                .control_size(self.size)
                .selected(on)
                .disabled(self.disabled);
            if !self.disabled {
                let field = cx.entity().downgrade();
                control = control.on_click(move |_, cx| {
                    field
                        .update(cx, |_, cx| {
                            cx.emit(if is_case {
                                SearchFieldEvent::MatchCaseToggled(!on)
                            } else {
                                SearchFieldEvent::WholeWordToggled(!on)
                            })
                        })
                        .ok();
                });
            }
            Some(control)
        })
        .collect::<Vec<_>>();

        div()
            .id(self.ident.element_id())
            .row_reading(direction)
            .w_full()
            .gap_token(&theme, Space::Sm)
            .px_token(&theme, Space::Sm)
            .py_token(&theme, Space::Xs)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .child(
                div().flex_1().min_w_0().child(self.query.clone()).child(
                    div().absolute().size_0().semantic_in(
                        cx,
                        NodeSpec::new(self.ident.child("query.label").semantic_id(), Role::Text)
                            .parent(self.ident.semantic_id())
                            .labels(self.ident.child("query").semantic_id())
                            .text(self.placeholder.clone().unwrap_or_else(|| {
                                cx.strings().text(StringKey::SearchPlaceholder)
                            })),
                    ),
                ),
            )
            .child(
                foundation_text(&theme, TypeScale::Caption, sentence.clone())
                    .flex_none()
                    .text_color(match self.count {
                        HitCount::Unavailable(_) => theme.colors.warning,
                        HitCount::Counting | HitCount::Unsearched => theme.colors.text_faint,
                        _ => theme.colors.text_muted,
                    })
                    .semantic_in(
                        cx,
                        NodeSpec::new(count_ident.semantic_id(), Role::Status)
                            .parent(self.ident.semantic_id())
                            .text(sentence)
                            .value(self.count.name())
                            .busy(self.count == HitCount::Counting),
                    ),
            )
            .children(toggles)
            .child(self.step_control(
                "previous",
                Icon::AltArrowLeft,
                StringKey::SearchPrevious,
                SearchFieldEvent::Previous,
                cx,
            ))
            .child(self.step_control(
                "next",
                Icon::AltArrowRight,
                StringKey::SearchNext,
                SearchFieldEvent::Next,
                cx,
            ))
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .disabled(self.disabled)
                    .value(self.count.name()),
            )
    }
}

impl Sizable for SearchField {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

/// What a find-and-replace surface reports. It replaces nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindReplaceEvent {
    /// Whatever the search field itself reported.
    Search(SearchFieldEvent),
    ReplacementChanged(SharedString),
    /// Replace the hit the typist is on.
    ReplaceOne,
    /// Replace every hit. `count` is the number the control stated before it
    /// was taken, so a host that has since found more can refuse.
    ReplaceAll {
        count: usize,
    },
}

impl EventEmitter<FindReplaceEvent> for FindReplace {}

/// A [`SearchField`] with a replacement field and the two replace actions.
///
/// Replace-all says how many hits it will change **before** it is taken, and
/// is only offered when that number is a counted one: an unfinished count, a
/// count that stopped early, and a search the host could not run are all
/// numbers nobody has, so the action is refused with the reason on it rather
/// than fired against a guess.
pub struct FindReplace {
    ident: Ident,
    focus_handle: FocusHandle,
    search: Entity<SearchField>,
    replacement: Entity<TextInput>,
    size: ControlSize,
    disabled: bool,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for FindReplace {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FindReplace")
            .field("ident", &self.ident)
            .field("disabled", &self.disabled)
            .finish()
    }
}

impl FindReplace {
    pub fn new(ident: impl Into<Ident>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let search = cx.new(|cx| SearchField::new(ident.child("find"), window, cx));
        let replacement = cx.new(|cx| {
            TextInput::new(ident.child("replacement"), window, cx)
                .bare(true)
                .placeholder(cx.strings().text(StringKey::ReplacePlaceholder))
        });
        let subscriptions = vec![
            cx.subscribe(&search, |_, _, event: &SearchFieldEvent, cx| {
                cx.emit(FindReplaceEvent::Search(event.clone()));
            }),
            cx.subscribe(&replacement, |_, _, event, cx| {
                if let TextInputEvent::Change(text) = event {
                    cx.emit(FindReplaceEvent::ReplacementChanged(text.clone()));
                }
            }),
        ];

        Self {
            ident,
            focus_handle: cx.focus_handle(),
            search,
            replacement,
            size: ControlSize::Sm,
            disabled: false,
            _subscriptions: subscriptions,
        }
    }

    pub fn search_field(&self) -> &Entity<SearchField> {
        &self.search
    }

    pub fn replacement_input(&self) -> &Entity<TextInput> {
        &self.replacement
    }

    pub fn set_count(&mut self, count: HitCount, cx: &mut Context<Self>) {
        self.search
            .update(cx, |search, cx| search.set_count(count, cx));
        cx.notify();
    }

    pub fn count(&self, cx: &App) -> HitCount {
        self.search.read(cx).count().clone()
    }

    pub fn replacement_text(&self, cx: &App) -> SharedString {
        self.replacement.read(cx).value().clone()
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.search
            .update(cx, |search, cx| search.set_disabled(disabled, cx));
        self.replacement
            .update(cx, |input, cx| input.set_disabled(disabled, cx));
        cx.notify();
    }
}

impl Focusable for FindReplace {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Sizable for FindReplace {
    fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}

impl Render for FindReplace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let direction = cx.layout_direction();
        let count = self.search.read(cx).count().clone();
        let counted = count.exact().filter(|total| *total > 0);
        let one_live =
            !self.disabled && matches!(count, HitCount::Known { total, .. } if total > 0);

        let replace_one = {
            let ident = self.ident.child("replace-one");
            let mut control = Button::new(ident)
                .label(cx.strings().text(StringKey::ReplaceOne))
                .secondary()
                .control_size(self.size)
                .semantic_parent(self.ident.semantic_id())
                .disabled(!one_live);
            if one_live {
                let surface = cx.entity().downgrade();
                control = control.on_click(move |_, cx| {
                    surface
                        .update(cx, |_, cx| cx.emit(FindReplaceEvent::ReplaceOne))
                        .ok();
                });
            }
            control
        };

        let replace_all = {
            let ident = self.ident.child("replace-all");
            // The label carries the number before the action is taken, so
            // nobody has to find out afterwards how much it changed.
            let label = match counted {
                Some(total) => cx
                    .strings()
                    .format(StringKey::ReplaceAllCounted, &[&total.to_string()]),
                None => cx.strings().text(StringKey::ReplaceAllUncounted),
            };
            let live = !self.disabled && counted.is_some();
            let mut control = Button::new(ident)
                .label(label)
                .secondary()
                .control_size(self.size)
                .semantic_parent(self.ident.semantic_id())
                .disabled(!live);
            if let Some(total) = counted.filter(|_| live) {
                let surface = cx.entity().downgrade();
                control = control.on_click(move |_, cx| {
                    surface
                        .update(cx, |_, cx| {
                            cx.emit(FindReplaceEvent::ReplaceAll { count: total })
                        })
                        .ok();
                });
            }
            control
        };

        let uncountable = counted.is_none().then(|| {
            let ident = self.ident.child("replace-all.reason");
            foundation_text(
                &theme,
                TypeScale::Caption,
                cx.strings().text(StringKey::ReplaceAllUncountable),
            )
            .text_tone(&theme, gpui_kit_theme::TextTone::Faint)
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Text)
                    .parent(self.ident.semantic_id())
                    .text(cx.strings().text(StringKey::ReplaceAllUncountable)),
            )
        });

        div()
            .id(self.ident.element_id())
            .column()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .child(self.search.clone())
            .child(
                div()
                    .row_reading(direction)
                    .w_full()
                    .gap_token(&theme, Space::Sm)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .px_token(&theme, Space::Sm)
                            .py_token(&theme, Space::Xs)
                            .radius(&theme, Radius::Card)
                            .frame(&theme, Surface::Panel, Elevation::Raised)
                            .child(self.replacement.clone()),
                    )
                    .child(replace_one)
                    .child(replace_all),
            )
            .children(uncountable)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group)
                    .disabled(self.disabled)
                    // The container publishes the number replace-all claims,
                    // so a snapshot shows the claim and not just the wording.
                    .when_value(counted),
            )
    }
}

/// Adds the counted total to a spec only when there is one.
trait CountedSpec {
    fn when_value(self, count: Option<usize>) -> Self;
}

impl CountedSpec for NodeSpec {
    fn when_value(self, count: Option<usize>) -> Self {
        match count {
            Some(count) => self.value(count.to_string()),
            None => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_counted_total_is_an_exact_one() {
        assert_eq!(
            HitCount::Known {
                total: 4,
                current: Some(0)
            }
            .exact(),
            Some(4)
        );
        assert_eq!(HitCount::TooMany { counted: 500 }.exact(), None);
        assert_eq!(HitCount::Counting.exact(), None);
        assert_eq!(HitCount::None.exact(), None);
        assert_eq!(HitCount::Unsearched.exact(), None);
    }

    #[test]
    fn nothing_steps_until_there_is_somewhere_to_go() {
        assert!(!HitCount::Unsearched.steppable());
        assert!(!HitCount::Counting.steppable());
        assert!(!HitCount::None.steppable());
        assert!(
            !HitCount::Known {
                total: 0,
                current: None
            }
            .steppable()
        );
        assert!(
            HitCount::Known {
                total: 1,
                current: Some(0)
            }
            .steppable()
        );
        assert!(HitCount::TooMany { counted: 9 }.steppable());
    }

    #[test]
    fn every_state_publishes_a_name_of_its_own() {
        let names = [
            HitCount::Unsearched.name(),
            HitCount::Counting.name(),
            HitCount::None.name(),
            HitCount::Known {
                total: 1,
                current: None,
            }
            .name(),
            HitCount::TooMany { counted: 1 }.name(),
            HitCount::Unavailable("offline".into()).name(),
        ];
        let mut sorted = names.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
    }
}
