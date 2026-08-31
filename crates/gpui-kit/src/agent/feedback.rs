//! A caller-owned rating of a reply.
//!
//! The host owns the current vote and the reason tags. This component reports
//! the vote and any tag that was chosen; it never submits feedback itself.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::Icon as Glyph;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Radius, Space, Surface, TypeScale};

use crate::display::icon::{Icon as IconView, IconTone};
use crate::foundation::{
    Disableable, FocusRing, Ident, Pressable, SelectedFill, Sizable, StyledExt,
};
use crate::strings::{ActiveStrings, StringKey};

type VoteHandler = Rc<dyn Fn(FeedbackVote, &mut Window, &mut App)>;
type TagHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// The two-way vote a reader can cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackVote {
    Up,
    Down,
}

impl FeedbackVote {
    pub fn name(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
        }
    }
}

/// What the rating reported.
#[derive(Debug, Clone, PartialEq)]
pub enum FeedbackRatingEvent {
    Voted(FeedbackVote),
    Tagged(SharedString),
}

/// A helpful / not-helpful control with optional host-owned reason tags.
#[derive(IntoElement)]
pub struct FeedbackRating {
    ident: Ident,
    vote: Option<FeedbackVote>,
    tags: Vec<(SharedString, SharedString)>,
    current_tag: Option<SharedString>,
    disabled: bool,
    on_vote: Option<VoteHandler>,
    on_tag: Option<TagHandler>,
}

impl FeedbackRating {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            vote: None,
            tags: Vec::new(),
            current_tag: None,
            disabled: false,
            on_vote: None,
            on_tag: None,
        }
    }

    pub fn vote(mut self, vote: Option<FeedbackVote>) -> Self {
        self.vote = vote;
        self
    }

    /// Reason tags as `(id, label)` pairs. Identity is the host's.
    pub fn tags(
        mut self,
        tags: impl IntoIterator<Item = (impl Into<SharedString>, impl Into<SharedString>)>,
    ) -> Self {
        self.tags = tags
            .into_iter()
            .map(|(id, label)| (id.into(), label.into()))
            .collect();
        self
    }

    pub fn current_tag(mut self, tag: impl Into<SharedString>) -> Self {
        self.current_tag = Some(tag.into());
        self
    }

    pub fn on_vote(
        mut self,
        handler: impl Fn(FeedbackVote, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_vote = Some(Rc::new(handler));
        self
    }

    pub fn on_tag(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_tag = Some(Rc::new(handler));
        self
    }
}

impl Disableable for FeedbackRating {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for FeedbackRating {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let selected_chip = |element: gpui::Stateful<gpui::Div>, selected: bool| {
            element.selected_fill(&theme, selected)
        };
        let vote_chip = |vote: FeedbackVote, label: SharedString, selected: bool| {
            let handler = self.on_vote.clone().filter(|_| !self.disabled);
            let actionable = handler.is_some();
            let glyph = match vote {
                FeedbackVote::Up => Glyph::ArrowUp,
                FeedbackVote::Down => Glyph::ArrowDown,
            };
            let mut chip = selected_chip(
                div()
                    .id(self.ident.child(vote.name()).element_id())
                    .focus_ring(&theme)
                    .row()
                    .items_center()
                    .gap(px(theme.space(Space::Xs)))
                    .px(px(theme.space(Space::Sm)))
                    .py(px(theme.space(Space::Xs)))
                    .radius(&theme, Radius::Small)
                    .surface(&theme, Surface::Raised)
                    .text_color(theme.colors.text),
                selected,
            )
            .type_scale(&theme, TypeScale::Caption)
            .child(IconView::new(glyph).small().tone(if selected {
                IconTone::Accent
            } else {
                IconTone::Faint
            }))
            .child(label)
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.child(vote.name()).semantic_id(), Role::Button)
                    .parent(self.ident.semantic_id())
                    .text(vote.name())
                    .selected(selected)
                    .disabled(!actionable),
            );
            if let Some(handler) = handler {
                let click = Rc::clone(&handler);
                chip = chip
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .on_click(move |_, window, cx| click(vote, window, cx))
                    .on_key_down(move |event, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            handler(vote, window, cx);
                            cx.stop_propagation();
                        }
                    });
            }
            chip
        };
        let tags = self
            .tags
            .into_iter()
            .map(|(id, label)| {
                let selected = self.current_tag.as_ref() == Some(&id);
                let handler = self.on_tag.clone().filter(|_| !self.disabled);
                let actionable = handler.is_some();
                let mut chip = selected_chip(
                    div()
                        .id(self.ident.child("tag").child(id.as_ref()).element_id())
                        .focus_ring(&theme)
                        .px(px(theme.space(Space::Xs)))
                        .py(px(theme.space(Space::Xxs)))
                        .radius(&theme, Radius::Small)
                        .surface(&theme, Surface::Raised),
                    selected,
                )
                .type_scale(&theme, TypeScale::Caption)
                .child(label.clone())
                .semantic_in(
                    cx,
                    NodeSpec::new(
                        self.ident.child("tag").child(id.as_ref()).semantic_id(),
                        Role::Button,
                    )
                    .parent(self.ident.semantic_id())
                    .text(label)
                    .selected(selected)
                    .disabled(!actionable),
                );
                if let Some(handler) = handler {
                    let reported = id.clone();
                    let click = Rc::clone(&handler);
                    let clicked = reported.clone();
                    chip = chip
                        .cursor_pointer()
                        .tab_index(0)
                        .pressable(cx)
                        .on_click(move |_, window, cx| click(clicked.clone(), window, cx))
                        .on_key_down(move |event, window, cx| {
                            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                handler(reported.clone(), window, cx);
                                cx.stop_propagation();
                            }
                        });
                }
                chip
            })
            .collect::<Vec<_>>();
        // A rating nobody may cast has to look like one. Disabling only
        // removed the handlers, which left the control drawn exactly as the
        // live one beside it.
        div()
            .column()
            .gap_token(&theme, Space::Sm)
            .when(self.disabled, |element| {
                element.opacity(theme.opacity.disabled)
            })
            .child(
                div()
                    .row()
                    .gap_token(&theme, Space::Xs)
                    .child(vote_chip(
                        FeedbackVote::Up,
                        cx.strings().text(StringKey::FeedbackUp),
                        self.vote == Some(FeedbackVote::Up),
                    ))
                    .child(vote_chip(
                        FeedbackVote::Down,
                        cx.strings().text(StringKey::FeedbackDown),
                        self.vote == Some(FeedbackVote::Down),
                    )),
            )
            .child(
                div()
                    .row()
                    .flex_wrap()
                    .gap_token(&theme, Space::Xs)
                    .children(tags),
            )
            .semantic_in(
                cx,
                NodeSpec::new(self.ident.semantic_id(), Role::Group).value(
                    self.vote
                        .map(|vote| vote.name())
                        .unwrap_or("none")
                        .to_string(),
                ),
            )
    }
}
