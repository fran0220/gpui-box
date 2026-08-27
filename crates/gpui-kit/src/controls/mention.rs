//! Caller-owned mention candidates over one shared text editor.
//!
//! `MentionInput` owns the `@query` interaction, not the people or objects the
//! query may name. The caller supplies candidates with stable identity and
//! advances their [`AsyncValue`] state. The component detects a token at the
//! caret, reports the query, filters through the installed locale matcher,
//! anchors one menu to painted caret geometry, and replaces the token as one
//! undoable edit. Accepted identity and the resulting byte range are reported
//! back so the caller can attach meaning to the plain text.

use std::ops::Range;

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Render, ScrollHandle,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div, point,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TypeScale};

use crate::controls::textarea::{Pasted, TextArea, TextAreaEvent};
use crate::display::loading::PulseLoader;
use crate::foundation::{Ident, Pressable, StyledExt, text as foundation_text};
use crate::motion;
use crate::overlay::popover;
use crate::state::{AsyncStatus, AsyncValue, HasPhase};
use crate::strings::{ActiveSearch, ActiveStrings, SearchMatcher, StringKey};

/// One caller-owned answer to a mention query.
#[derive(Clone, PartialEq, Eq)]
pub struct MentionCandidate {
    pub id: SharedString,
    pub label: SharedString,
    pub description: Option<SharedString>,
    replacement: SharedString,
    search_terms: Vec<SharedString>,
    refusal: Option<SharedString>,
}

impl std::fmt::Debug for MentionCandidate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MentionCandidate")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("has_description", &self.description.is_some())
            .field("search_terms", &self.search_terms.len())
            .field("available", &self.refusal.is_none())
            .finish()
    }
}

impl MentionCandidate {
    /// Creates a candidate whose visible label is also its inserted text.
    /// Use [`Self::replacement`] when the transcript spelling differs.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            id: id.into(),
            replacement: label.clone(),
            label,
            description: None,
            search_terms: Vec::new(),
            refusal: None,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// The exact text that replaces `@query` when this candidate is accepted.
    pub fn replacement(mut self, replacement: impl Into<SharedString>) -> Self {
        self.replacement = replacement.into();
        self
    }

    /// Caller-owned aliases that participate in locale-aware matching but are
    /// never rendered or published in semantic snapshots.
    pub fn search_terms(
        mut self,
        terms: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.search_terms.extend(terms.into_iter().map(Into::into));
        self
    }

    /// Keeps the answer visible with the caller's reason, but installs no
    /// pointer or keyboard acceptance handler for it.
    pub fn unavailable(mut self, reason: impl Into<SharedString>) -> Self {
        self.refusal = Some(reason.into());
        self
    }

    pub fn refusal(&self) -> Option<&SharedString> {
        self.refusal.as_ref()
    }

    fn rank(&self, query: &str, matcher: &dyn SearchMatcher) -> Option<usize> {
        std::iter::once(&self.label)
            .chain(self.description.iter())
            .chain(self.search_terms.iter())
            .filter_map(|text| matcher.rank(query, text.as_ref()))
            .min()
    }
}

/// The active token, in UTF-8 byte offsets into the editor's current value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionQuery {
    pub text: SharedString,
    pub range: Range<usize>,
}

/// What a mention editor reports without applying caller-owned meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MentionInputEvent {
    Changed(SharedString),
    Submitted,
    Cancelled,
    Pasted(Pasted),
    Focused,
    Blurred,
    /// `None` means the former query closed, moved away from the caret, or was
    /// dismissed. A later focus or changed token reports it again.
    QueryChanged(Option<MentionQuery>),
    /// The caller-owned identity and the byte range of the inserted text.
    Accepted {
        id: SharedString,
        range: Range<usize>,
    },
}

impl EventEmitter<MentionInputEvent> for MentionInput {}

/// A multiline editor with one caret-anchored mention completion surface.
///
/// The supplied [`TextArea`] entity must not also be mounted elsewhere. It is
/// exposed through [`Self::editor`] so the caller keeps the full plain-text
/// configuration and value API rather than receiving a reduced proxy.
pub struct MentionInput {
    ident: Ident,
    editor: Entity<TextArea>,
    suggestions: AsyncValue<Vec<MentionCandidate>, SharedString>,
    trigger: Option<MentionQuery>,
    reported_query: Option<MentionQuery>,
    dismissed: Option<MentionQuery>,
    active: Option<SharedString>,
    focused: bool,
    scroll: ScrollHandle,
    reveal_active: bool,
    /// Where the editor was painted on the previous frame, so the completion
    /// surface can clear the whole control instead of only the caret.
    editor_bounds: Rc<Cell<Bounds<Pixels>>>,
    _subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for MentionInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MentionInput")
            .field("ident", &self.ident)
            .field("phase", &self.suggestions.phase().name())
            .field(
                "candidates",
                &self.suggestions.value.as_ref().map_or(0, Vec::len),
            )
            .field("open", &self.is_open())
            .finish()
    }
}

impl MentionInput {
    pub fn new(ident: impl Into<Ident>, editor: Entity<TextArea>, cx: &mut Context<Self>) -> Self {
        let ident = ident.into();
        let subscription = cx.subscribe(&editor, |input, _editor, event, cx| {
            input.on_editor_event(event, cx)
        });
        let observation = cx.observe(&editor, |input, _editor, cx| input.sync_trigger(cx));
        Self {
            ident,
            editor,
            suggestions: AsyncValue::idle(),
            trigger: None,
            reported_query: None,
            dismissed: None,
            active: None,
            focused: false,
            scroll: ScrollHandle::new(),
            reveal_active: false,
            editor_bounds: Rc::default(),
            _subscriptions: vec![subscription, observation],
        }
    }

    pub fn suggestions(
        mut self,
        suggestions: AsyncValue<Vec<MentionCandidate>, SharedString>,
    ) -> Self {
        self.suggestions = suggestions;
        self
    }

    pub fn candidates(mut self, candidates: impl IntoIterator<Item = MentionCandidate>) -> Self {
        self.suggestions = AsyncValue::ready(candidates.into_iter().collect());
        self
    }

    pub fn set_suggestions(
        &mut self,
        suggestions: AsyncValue<Vec<MentionCandidate>, SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.suggestions = suggestions;
        self.reconcile_active(cx);
        cx.notify();
    }

    pub fn suggestions_state(&self) -> &AsyncValue<Vec<MentionCandidate>, SharedString> {
        &self.suggestions
    }

    pub fn editor(&self) -> &Entity<TextArea> {
        &self.editor
    }

    pub fn active_query(&self) -> Option<&MentionQuery> {
        self.reported_query.as_ref()
    }

    pub fn is_open(&self) -> bool {
        self.focused && self.trigger.is_some() && self.dismissed.as_ref() != self.trigger.as_ref()
    }

    fn on_editor_event(&mut self, event: &TextAreaEvent, cx: &mut Context<Self>) {
        match event {
            TextAreaEvent::Change(value) => {
                cx.emit(MentionInputEvent::Changed(value.clone()));
                self.sync_trigger(cx);
            }
            TextAreaEvent::Submit => cx.emit(MentionInputEvent::Submitted),
            TextAreaEvent::Cancel => cx.emit(MentionInputEvent::Cancelled),
            TextAreaEvent::Pasted(pasted) => cx.emit(MentionInputEvent::Pasted(pasted.clone())),
            TextAreaEvent::Focus => {
                if !self.focused {
                    self.focused = true;
                    cx.emit(MentionInputEvent::Focused);
                }
                self.sync_trigger(cx);
            }
            TextAreaEvent::Blur => {
                if self.focused {
                    self.focused = false;
                    cx.emit(MentionInputEvent::Blurred);
                }
                self.publish_query(cx);
                cx.notify();
            }
            TextAreaEvent::SelectionChanged(_) => self.sync_trigger(cx),
            TextAreaEvent::GeometryChanged => {
                if self.is_open() {
                    cx.notify();
                }
            }
            TextAreaEvent::MoveUp => self.move_active(-1, cx),
            TextAreaEvent::MoveDown => self.move_active(1, cx),
            TextAreaEvent::AcceptCompletion => self.accept_active(cx),
            TextAreaEvent::DismissCompletion => self.dismiss(cx),
        }
    }

    fn sync_trigger(&mut self, cx: &mut Context<Self>) {
        self.update_trigger(cx);
        self.publish_query(cx);
        cx.notify();
    }

    fn update_trigger(&mut self, cx: &App) {
        let next = {
            let editor = self.editor.read(cx);
            if editor.is_disabled() || editor.is_read_only() {
                None
            } else {
                mention_query(
                    editor.value().as_ref(),
                    editor.selected_range(),
                    editor.cursor_offset(),
                )
            }
        };
        if next != self.trigger {
            self.trigger = next;
            self.dismissed = None;
            self.reconcile_active(cx);
        }
    }

    fn publish_query(&mut self, cx: &mut Context<Self>) {
        let next = self.is_open().then(|| self.trigger.clone()).flatten();
        if next != self.reported_query {
            self.reported_query = next.clone();
            cx.emit(MentionInputEvent::QueryChanged(next));
        }
    }

    fn visible_candidates(&self) -> Option<&[MentionCandidate]> {
        match self.suggestions.status {
            AsyncStatus::Ready | AsyncStatus::Refreshing | AsyncStatus::Error(_) => {
                self.suggestions.value.as_deref()
            }
            AsyncStatus::Idle
            | AsyncStatus::Loading
            | AsyncStatus::Empty
            | AsyncStatus::Unavailable(_) => None,
        }
    }

    fn ranked_indices(&self, cx: &App) -> Vec<usize> {
        let (Some(query), Some(candidates)) = (&self.trigger, self.visible_candidates()) else {
            return Vec::new();
        };
        let matcher = cx.search();
        let mut ranked = candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                candidate
                    .rank(query.text.as_ref(), matcher.as_ref())
                    .map(|rank| (rank, index))
            })
            .collect::<Vec<_>>();
        ranked.sort_by_key(|&(rank, index)| (rank, index));
        ranked.into_iter().map(|(_, index)| index).collect()
    }

    fn reconcile_active(&mut self, cx: &App) {
        let matches = self.ranked_indices(cx);
        let candidates = self.visible_candidates();
        let still_active = self.active.as_ref().is_some_and(|active| {
            matches.iter().any(|index| {
                candidates.is_some_and(|candidates| {
                    candidates[*index].id == *active && candidates[*index].refusal.is_none()
                })
            })
        });
        if !still_active {
            self.active = matches.into_iter().find_map(|index| {
                let candidate = &candidates?[index];
                candidate.refusal.is_none().then(|| candidate.id.clone())
            });
            self.reveal_active = true;
        }
    }

    fn move_active(&mut self, delta: isize, cx: &mut Context<Self>) {
        if !self.is_open() {
            return;
        }
        let matches = self.ranked_indices(cx);
        let Some(candidates) = self.visible_candidates() else {
            return;
        };
        let enabled = matches
            .into_iter()
            .filter(|index| candidates[*index].refusal.is_none())
            .map(|index| candidates[index].id.clone())
            .collect::<Vec<_>>();
        let current = self
            .active
            .as_ref()
            .and_then(|active| enabled.iter().position(|id| id == active));
        self.active =
            popover::step(current, enabled.len(), delta).map(|index| enabled[index].clone());
        self.reveal_active = true;
        cx.notify();
    }

    fn accept_active(&mut self, cx: &mut Context<Self>) {
        let (Some(trigger), Some(active), Some(candidates)) = (
            self.trigger.clone(),
            self.active.clone(),
            self.visible_candidates(),
        ) else {
            return;
        };
        let Some(candidate) = candidates
            .iter()
            .find(|candidate| candidate.id == active && candidate.refusal.is_none())
        else {
            return;
        };
        let id = candidate.id.clone();
        let replacement = candidate.replacement.clone();
        let inserted = self.editor.update(cx, |editor, cx| {
            editor.replace_range(trigger.range, replacement.as_ref(), cx)
        });
        if let Some(range) = inserted {
            cx.emit(MentionInputEvent::Accepted { id, range });
        }
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        if !self.is_open() {
            return;
        }
        self.dismissed = self.trigger.clone();
        self.active = None;
        self.publish_query(cx);
        cx.notify();
    }

    fn menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let menu_ident = self.ident.child("menu");
        let matches = self.ranked_indices(cx);
        let active_position = {
            let candidates = self.visible_candidates();
            self.active.as_ref().and_then(|active| {
                matches.iter().position(|index| {
                    candidates.is_some_and(|candidates| candidates[*index].id == *active)
                })
            })
        };
        if self.reveal_active {
            if let Some(position) = active_position {
                self.scroll.scroll_to_item(position);
            }
            self.reveal_active = false;
        }

        let candidates = self.visible_candidates();
        let has_value = candidates.is_some();
        let rows = if has_value && !matches.is_empty() {
            matches
                .iter()
                .enumerate()
                .map(|(position, index)| self.candidate_row(*index, position, matches.len(), cx))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let top = match &self.suggestions.status {
            AsyncStatus::Refreshing if has_value => Some(self.status_row(
                "refreshing",
                cx.strings().text(StringKey::MentionRefreshing),
                None,
                StatusMark::Busy,
                cx,
            )),
            _ => None,
        };
        let bottom = match &self.suggestions.status {
            AsyncStatus::Error(reason) if has_value => Some(self.status_row(
                "stale",
                cx.strings().text(StringKey::MentionStale),
                Some(reason.clone()),
                StatusMark::Danger,
                cx,
            )),
            _ => None,
        };
        let state = rows
            .is_empty()
            .then(|| self.empty_menu_state(has_value, cx));

        popover::card_flush(&theme)
            .w(px(theme.measures.compact_overlay_width))
            .max_h(px(theme.measures.compact_menu_max_height))
            .id(menu_ident.element_id())
            .children(top)
            .child(
                div()
                    .p(px(theme.space(Space::Xs)))
                    .max_h(px(theme.measures.compact_menu_max_height))
                    .id(menu_ident.child("scroll").element_id())
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .children(rows)
                    .children(state),
            )
            .children(bottom)
            .semantic_in(
                cx,
                NodeSpec::new(menu_ident.semantic_id(), Role::Menu)
                    .parent(self.editor.read(cx).semantic_id())
                    .text(cx.strings().text(StringKey::MentionSuggestions)),
            )
            .into_any_element()
    }

    fn empty_menu_state(&self, has_value: bool, cx: &mut Context<Self>) -> AnyElement {
        match &self.suggestions.status {
            AsyncStatus::Idle => self.status_row(
                "idle",
                cx.strings().text(StringKey::MentionIdle),
                None,
                StatusMark::Info,
                cx,
            ),
            AsyncStatus::Loading => self.status_row(
                "loading",
                cx.strings().text(StringKey::MentionLoading),
                None,
                StatusMark::Busy,
                cx,
            ),
            AsyncStatus::Empty => self.status_row(
                "empty",
                cx.strings().text(StringKey::MentionEmpty),
                None,
                StatusMark::Info,
                cx,
            ),
            AsyncStatus::Unavailable(reason) => self.status_row(
                "unavailable",
                cx.strings().text(StringKey::MentionUnavailable),
                Some(reason.clone().into()),
                StatusMark::Info,
                cx,
            ),
            AsyncStatus::Error(reason) if !has_value => self.status_row(
                "error",
                cx.strings().text(StringKey::MentionError),
                Some(reason.clone()),
                StatusMark::Danger,
                cx,
            ),
            AsyncStatus::Ready | AsyncStatus::Refreshing | AsyncStatus::Error(_) => {
                let empty = self
                    .visible_candidates()
                    .is_none_or(|candidates| candidates.is_empty());
                self.status_row(
                    if empty { "empty" } else { "no-match" },
                    cx.strings().text(if empty {
                        StringKey::MentionEmpty
                    } else {
                        StringKey::MentionNoMatch
                    }),
                    None,
                    StatusMark::Info,
                    cx,
                )
            }
        }
    }

    fn status_row(
        &self,
        suffix: &str,
        title: SharedString,
        detail: Option<SharedString>,
        mark: StatusMark,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let ident = self.ident.child("status").child(suffix);
        let text_color = if mark == StatusMark::Danger {
            theme.colors.danger
        } else {
            theme.colors.text_muted
        };
        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Status)
            .parent(self.ident.child("menu").semantic_id())
            .text(title.clone())
            .busy(mark == StatusMark::Busy)
            .invalid(mark == StatusMark::Danger);
        if let Some(detail) = detail.clone() {
            spec = spec.description(detail);
        }
        div()
            .row()
            .items_start()
            .gap_token(&theme, Space::Sm)
            .px_token(&theme, Space::Sm)
            .py_token(&theme, Space::Sm)
            .child(match mark {
                StatusMark::Busy => PulseLoader::new(ident.child("mark")).into_any_element(),
                StatusMark::Info => icon(Icon::Info)
                    .size(px(theme.control.sm.icon_size))
                    .text_color(theme.colors.text_faint)
                    .into_any_element(),
                StatusMark::Danger => icon(Icon::CloseCircle)
                    .size(px(theme.control.sm.icon_size))
                    .text_color(theme.colors.danger)
                    .into_any_element(),
            })
            .child(
                div()
                    .column()
                    .min_w_0()
                    .gap(px(theme.space(Space::Xxs)))
                    .child(
                        foundation_text(&theme, TypeScale::Caption, title).text_color(text_color),
                    )
                    .children(detail.map(|detail| {
                        foundation_text(&theme, TypeScale::Caption, detail).text_color(text_color)
                    })),
            )
            .semantic_in(cx, spec)
            .into_any_element()
    }

    fn candidate_row(
        &self,
        index: usize,
        position: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let candidate = &self.visible_candidates().expect("candidate value")[index];
        let active = self.active.as_ref() == Some(&candidate.id);
        let disabled = candidate.refusal.is_some();
        let ident = self.ident.child("option").child(candidate.id.as_ref());
        let hover_group = ident.child("hover").semantic_id();
        let id = candidate.id.clone();
        let weak = cx.entity().downgrade();
        let editor_element_id = self.editor.read(cx).element_id();
        let mut spec = NodeSpec::new(ident.semantic_id(), Role::Option)
            .parent(self.ident.child("menu").semantic_id())
            .text(candidate.label.clone())
            .disabled(disabled)
            .hovered(active);
        if let Some(description) = candidate
            .refusal
            .clone()
            .or_else(|| candidate.description.clone())
        {
            spec = spec.description(description);
        }

        let row = popover::menu_row(&theme, false, active)
            .id(ident.element_id())
            .group(hover_group.clone())
            .when(active, |element| {
                element.aria_active_descendant_of(editor_element_id)
            })
            .when(!disabled, |element| element.cursor_pointer().pressable(cx))
            .child(
                div()
                    .column()
                    .flex_1()
                    .min_w_0()
                    .gap(px(theme.space(Space::Xxs)))
                    .child(popover::menu_label_state(
                        &theme,
                        candidate.label.clone(),
                        false,
                        active,
                        disabled,
                        hover_group,
                    ))
                    .children(candidate.description.clone().map(|description| {
                        foundation_text(&theme, TypeScale::Caption, description).text_color(
                            if disabled {
                                theme.colors.text_disabled
                            } else {
                                theme.colors.text_muted
                            },
                        )
                    }))
                    .children(candidate.refusal.clone().map(|refusal| {
                        foundation_text(&theme, TypeScale::Caption, refusal)
                            .text_color(theme.colors.danger)
                    })),
            )
            .when(!disabled, |element| {
                element.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let _ = weak.update(cx, |input, cx| input.accept(id.clone(), cx));
                })
            })
            .semantic_in(cx, spec);

        motion::row_in(ident.child("in").element_id(), &theme, position, count, row)
            .into_any_element()
    }

    fn accept(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.active = Some(id);
        self.accept_active(cx);
    }
}

impl Focusable for MentionInput {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.editor.read(cx).focus_handle(cx)
    }
}

impl Render for MentionInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.editor.read(cx).focus_handle(cx).is_focused(window);
        if focused != self.focused {
            self.focused = focused;
            cx.emit(if focused {
                MentionInputEvent::Focused
            } else {
                MentionInputEvent::Blurred
            });
            self.update_trigger(cx);
            self.publish_query(cx);
        }
        let open = self.is_open();
        self.editor
            .update(cx, |editor, _| editor.set_completion_claimed(open));
        let theme = cx.theme().clone();
        let editor_bottom = self.editor_bounds.get().bottom();
        let popup = open
            .then(|| self.editor.read(cx).caret_bounds())
            .flatten()
            .map(|caret| {
                // The caret sits on one line inside a multiline editor, so
                // hanging the surface off it covers the rest of the field and
                // the focus ring around it. The query is still what the caret
                // is on, so the column stays with the caret and only the row
                // comes from the control.
                let position = point(
                    caret.left(),
                    caret.bottom().max(editor_bottom) + px(popover::trigger_gap(&theme)),
                );
                let menu = self.menu(cx);
                popover::at(
                    self.ident.child("anchor").element_id(),
                    &theme,
                    position,
                    menu,
                )
            });
        let measured = Rc::clone(&self.editor_bounds);
        div()
            .on_children_prepainted(move |bounds, window, _| {
                if let Some(editor) = bounds.first() {
                    crate::layout::measure::record(&measured, *editor, window);
                }
            })
            .id(self.ident.element_id())
            .w_full()
            .child(self.editor.clone())
            .children(popup)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusMark {
    Busy,
    Info,
    Danger,
}

fn mention_query(text: &str, selection: Range<usize>, cursor: usize) -> Option<MentionQuery> {
    if !selection.is_empty() || cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    if text[cursor..].chars().next().is_some_and(query_character) {
        return None;
    }
    let before = &text[..cursor];
    for (index, character) in before.char_indices().rev() {
        if character == '@' {
            let boundary = before[..index]
                .chars()
                .next_back()
                .is_none_or(|previous| previous != '@' && !query_character(previous));
            return boundary.then(|| MentionQuery {
                text: SharedString::from(before[index + character.len_utf8()..].to_string()),
                range: index..cursor,
            });
        }
        if !query_character(character) {
            break;
        }
    }
    None
}

fn query_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '-' | '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_or_email_prefix_is_not_a_trigger_boundary() {
        assert!(mention_query("hello @ada", 10..10, 10).is_some());
        assert!(mention_query("hello,@ada", 10..10, 10).is_some());
        assert!(mention_query("mail@example", 12..12, 12).is_none());
        assert!(mention_query("word@person", 11..11, 11).is_none());
    }

    #[test]
    fn a_query_must_end_at_an_empty_caret() {
        assert!(mention_query("@ada", 0..4, 4).is_none());
        assert!(mention_query("@ada", 2..2, 2).is_none());
        assert_eq!(
            mention_query("给 @阿达", 11..11, 11),
            Some(MentionQuery {
                text: "阿达".into(),
                range: 4..11,
            })
        );
    }
}
