//! Model reasoning, collapsed by default.
//!
//! # Why this is not a disclosure with a string in it
//!
//! Three facts get collapsed into one wherever reasoning is shown, and only
//! one of them is true at a time:
//!
//! - reasoning exists and is not on screen, because nobody opened it;
//! - reasoning exists and cannot be shown, because whoever produced it
//!   withheld it;
//! - there is no reasoning, because none was produced.
//!
//! An `Option<String>` can express two of those and quietly loses the third:
//! `None` would have to stand for both "withheld" and "none", and a block that
//! says nothing was produced when in fact it was withheld is stating something
//! nobody established. So [`Reasoning`] has three variants, no `Option`, and
//! no conversion from one — a caller holding a `None` has to decide which of
//! the two absences it is before it can build this component.
//!
//! Withholding is somebody's decision, so [`Reasoning::Withheld`] carries that
//! somebody's words and shows them verbatim.

use std::rc::Rc;

use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::Icon as Glyph;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, TextTone, TypeScale};

use crate::display::icon::{Icon as IconView, IconTone};
use crate::foundation::{FocusRing, Ident, Pressable, Sizable, StyledExt, text};
use crate::strings::{ActiveStrings, StringKey};

type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// What is known about a turn's reasoning.
///
/// Deliberately not `Option<SharedString>`, and deliberately without a
/// `From<Option<_>>`: the two ways of having no text to show are different
/// facts, and the type is the thing that stops them being confused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reasoning {
    /// It exists and this is it. An empty string is still reasoning that
    /// exists; it is not [`Reasoning::Absent`].
    Present(SharedString),
    /// It exists and was withheld, in the withholder's own words.
    Withheld(SharedString),
    /// None was produced.
    Absent,
}

impl Reasoning {
    pub fn present(text: impl Into<SharedString>) -> Self {
        Self::Present(text.into())
    }

    pub fn withheld(reason: impl Into<SharedString>) -> Self {
        Self::Withheld(reason.into())
    }

    /// The name the semantic node publishes.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Present(_) => "present",
            Self::Withheld(_) => "withheld",
            Self::Absent => "absent",
        }
    }

    /// Whether there is anything a disclosure could disclose.
    pub fn is_disclosable(&self) -> bool {
        matches!(self, Self::Present(_))
    }
}

impl From<SharedString> for Reasoning {
    fn from(value: SharedString) -> Self {
        Self::Present(value)
    }
}

impl From<&'static str> for Reasoning {
    fn from(value: &'static str) -> Self {
        Self::Present(SharedString::new_static(value))
    }
}

impl From<String> for Reasoning {
    fn from(value: String) -> Self {
        Self::Present(SharedString::from(value))
    }
}

/// A collapsed block of model reasoning.
///
/// Whether it is open is the caller's, as it is for
/// [`Accordion`](crate::navigation::accordion::Accordion): the block reports
/// the state it should take and shows exactly the state it was given.
#[derive(IntoElement)]
pub struct ThinkingBlock {
    ident: Ident,
    reasoning: Reasoning,
    expanded: bool,
    /// Whether the reasoning is still arriving. Separate from the reasoning
    /// itself, because text that has stopped growing and text that is still
    /// growing look identical and mean different things.
    thinking: bool,
    on_toggle: Option<ToggleHandler>,
}

impl std::fmt::Debug for ThinkingBlock {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ThinkingBlock")
            .field("ident", &self.ident)
            .field("reasoning", &self.reasoning.as_str())
            .field("expanded", &self.expanded)
            .field("has_handler", &self.on_toggle.is_some())
            .finish()
    }
}

impl ThinkingBlock {
    /// The reasoning is a constructor argument because there is no sensible
    /// default: which of the three states holds is the whole question.
    pub fn new(ident: impl Into<Ident>, reasoning: Reasoning) -> Self {
        Self {
            ident: ident.into(),
            reasoning,
            expanded: false,
            thinking: false,
            on_toggle: None,
        }
    }

    /// Collapsed unless the caller says otherwise, and reasoning that cannot
    /// be disclosed stays shut whatever the caller says.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Reports that the reasoning is still being produced.
    ///
    /// Nothing else on the block says this. Reasoning that has finished and
    /// reasoning still being written are the same words in the same place, so
    /// without this a reader watching a stalled run and a reader watching a
    /// working one see the same picture.
    pub fn thinking(mut self, thinking: bool) -> Self {
        self.thinking = thinking;
        self
    }

    /// Reports the state the block should take next.
    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    fn open(&self) -> bool {
        self.expanded && self.reasoning.is_disclosable()
    }

    fn actionable(&self) -> bool {
        self.reasoning.is_disclosable() && self.on_toggle.is_some()
    }
}

impl RenderOnce for ThinkingBlock {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();
        let open = self.open();
        let actionable = self.actionable();
        let label = cx.strings().text(StringKey::AgentReasoning);

        let (mark, tone) = match self.reasoning {
            // Still arriving outranks the settled word, because the breathing
            // glyph that also reports it is not there for a reader who has
            // animation turned off, and two states that look identical are
            // one state as far as that reader is concerned.
            Reasoning::Present(_) if self.thinking => {
                (StringKey::AgentReasoningThinking, IconTone::Accent)
            }
            Reasoning::Present(_) => (StringKey::AgentReasoning, IconTone::Muted),
            Reasoning::Withheld(_) => (StringKey::AgentReasoningWithheld, IconTone::Warning),
            Reasoning::Absent => (StringKey::AgentReasoningAbsent, IconTone::Faint),
        };
        let mark = cx.strings().text(mark);

        let mut header = div()
            .id(ident.element_id())
            .row()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .px_token(&theme, Space::Sm)
            .py(px(theme.space(Space::Xs)))
            .child({
                let mark = IconView::new(Glyph::Chat).small().tone(tone);
                // Deliberation breathes rather than turns: a turn claims work
                // is being got through, and this one has nothing to report
                // beyond that it is still going.
                if self.thinking {
                    mark.breathing(ident.child("mark"))
                } else {
                    mark
                }
            })
            .child(
                text(&theme, TypeScale::Label, label.clone())
                    .flex_1()
                    .min_w_0()
                    .text_tone(&theme, gpui_kit_theme::TextTone::Muted),
            )
            // Which of the three states holds is on screen without opening
            // anything, because two of them can never be opened.
            .child(
                text(&theme, TypeScale::Caption, mark)
                    .flex_none()
                    .text_color(match self.reasoning {
                        Reasoning::Withheld(_) => theme.colors.warning,
                        _ if self.thinking => theme.colors.accent,
                        _ => theme.colors.text_faint,
                    }),
            )
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
                    .hover(|style| style.bg(theme.colors.hover))
                    .focus_ring(&theme)
            });

        // A block with nothing to disclose never reaches the handler at all,
        // rather than installing one that would decline to fire.
        if let Some(handler) = self.on_toggle.clone().filter(|_| actionable) {
            let key_handler = Rc::clone(&handler);
            header = header
                .on_click(move |_, window, cx| handler(!open, window, cx))
                .on_key_down(move |event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        key_handler(!open, window, cx);
                        cx.stop_propagation();
                    }
                });
        }

        let header = header.semantic_in(
            cx,
            NodeSpec::new(
                ident.semantic_id(),
                if actionable { Role::Button } else { Role::Text },
            )
            .text(label)
            .value(self.reasoning.as_str())
            .busy(self.thinking)
            .expanded(open),
        );

        // A withheld or absent block has no body to open, so it says which of
        // the two it is where a body would be. Neither is drawn as the other,
        // and neither is drawn as a shut disclosure.
        let body = match &self.reasoning {
            // A closed section renders no body at all, the rule `Accordion`
            // keeps: nothing invisible stays addressable.
            Reasoning::Present(text) if open => Some(
                div()
                    .w_full()
                    .px_token(&theme, Space::Sm)
                    .pb(px(theme.space(Space::Xs)))
                    .children(text.lines().map(|line| {
                        crate::foundation::text(
                            &theme,
                            TypeScale::Body,
                            SharedString::from(line.to_string()),
                        )
                        .text_tone(&theme, TextTone::Muted)
                    }))
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.child("body").semantic_id(), Role::Text)
                            .parent(ident.semantic_id())
                            .value("present"),
                    ),
            ),
            Reasoning::Present(_) => None,
            Reasoning::Withheld(reason) => Some(
                text(&theme, TypeScale::Body, reason.clone())
                    .w_full()
                    .px_token(&theme, Space::Sm)
                    .pb(px(theme.space(Space::Xs)))
                    .text_color(theme.colors.warning)
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.child("withheld").semantic_id(), Role::Status)
                            .parent(ident.semantic_id())
                            .text(reason.clone())
                            .value("withheld"),
                    ),
            ),
            Reasoning::Absent => None,
        };

        div()
            .w_full()
            .column()
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .overflow_hidden()
            .child(header)
            .children(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_states_publish_three_names() {
        assert_eq!(Reasoning::present("because").as_str(), "present");
        assert_eq!(Reasoning::withheld("policy").as_str(), "withheld");
        assert_eq!(Reasoning::Absent.as_str(), "absent");
    }

    #[test]
    fn reasoning_that_exists_but_is_empty_is_not_absent() {
        assert_eq!(Reasoning::present(String::new()).as_str(), "present");
        assert_ne!(Reasoning::present(String::new()), Reasoning::Absent);
    }

    #[test]
    fn only_reasoning_that_is_there_can_be_opened() {
        assert!(Reasoning::present("because").is_disclosable());
        assert!(!Reasoning::withheld("policy").is_disclosable());
        assert!(!Reasoning::Absent.is_disclosable());
    }
}
