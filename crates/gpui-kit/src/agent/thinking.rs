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
use gpui_kit_theme::{ActiveTheme, Radius, Space, TextTone, TypeScale};

use crate::display::badge::Tone;
use crate::display::icon::{Icon as IconView, IconTone};
use crate::display::status::StatusDot;
use crate::foundation::direction::{ActiveDirection, DirectionalExt};
use crate::foundation::{FocusRing, Ident, Pressable, Sizable, StyledExt, text};
use crate::motion;
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
    /// How long the host says this turn has been thinking, already formatted.
    elapsed: Option<SharedString>,
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
            elapsed: None,
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

    /// A host-formatted duration shown as “Thought for …” when known.
    pub fn elapsed(mut self, elapsed: impl Into<SharedString>) -> Self {
        self.elapsed = Some(elapsed.into());
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
        let direction = cx.layout_direction();
        let ident = self.ident.clone();
        let open = self.open();
        let actionable = self.actionable();
        let label = match self.reasoning {
            Reasoning::Present(_) if self.thinking => cx.strings().text(StringKey::AgentThinking),
            Reasoning::Present(_) => match &self.elapsed {
                Some(elapsed) => cx
                    .strings()
                    .format(StringKey::AgentThoughtFor, &[elapsed.as_ref()]),
                None => cx.strings().text(StringKey::AgentThought),
            },
            Reasoning::Withheld(_) => cx.strings().text(StringKey::AgentReasoningWithheld),
            Reasoning::Absent => cx.strings().text(StringKey::AgentReasoningAbsent),
        };
        let mut dot = match self.reasoning {
            Reasoning::Present(_) if self.thinking => StatusDot::new(Tone::Accent),
            Reasoning::Present(_) | Reasoning::Absent => StatusDot::new(Tone::Neutral),
            Reasoning::Withheld(_) => StatusDot::new(Tone::Warning),
        };
        if self.thinking {
            dot = dot
                .busy(ident.child("mark"))
                .activity(motion::Activity::Deliberating);
        }

        let mut header = div()
            .id(ident.element_id())
            .row()
            .w_full()
            .min_w_0()
            .items_center()
            .gap_token(&theme, Space::Sm)
            .py(px(2.0))
            .child(dot)
            .child(
                // Absence is drawn quieter than a thought that happened. A
                // finished thought with nothing to show and no reasoning at
                // all were the same neutral mark and the same weight of
                // text, so only the wording told them apart.
                text(&theme, TypeScale::Caption, label.clone())
                    .flex_none()
                    .text_tone(
                        &theme,
                        match self.reasoning {
                            Reasoning::Absent => TextTone::Faint,
                            _ => TextTone::Muted,
                        },
                    ),
            )
            .children(match &self.reasoning {
                Reasoning::Withheld(reason) => Some(
                    text(&theme, TypeScale::Caption, reason.clone())
                        .min_w_0()
                        .truncate()
                        .text_color(theme.colors.warning)
                        .semantic_in(
                            cx,
                            NodeSpec::new(ident.child("withheld").semantic_id(), Role::Status)
                                .parent(ident.semantic_id())
                                .text(reason.clone())
                                .value("withheld"),
                        ),
                ),
                _ => None,
            })
            .children(actionable.then(|| {
                IconView::new(if open {
                    Glyph::AltArrowDown
                } else {
                    Glyph::AltArrowRight
                })
                .small()
                .tone(IconTone::Faint)
            }))
            .when(actionable, |element| {
                element
                    .cursor_pointer()
                    .tab_index(0)
                    .pressable(cx)
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

        // Present reasoning stays caller-owned and out of semantic snapshots.
        // Withheld and absent are already reported inline, so neither gets a
        // second, heavier block underneath the row.
        let body = match &self.reasoning {
            Reasoning::Present(text) if open => Some(
                div()
                    .w_full()
                    .column()
                    .ms(direction, px(theme.space(Space::Lg)))
                    .p_token(&theme, Space::Sm)
                    .radius(&theme, Radius::Control)
                    .well(&theme)
                    // The header is a label and this is what it labels, so
                    // the order of emphasis runs the other way: reasoning
                    // drawn fainter than the word "Thought" and standing on
                    // no surface of its own read as an aside about the row
                    // rather than as the thing the row discloses.
                    .children(text.lines().map(|line| {
                        crate::foundation::text(
                            &theme,
                            TypeScale::Caption,
                            SharedString::from(line.to_string()),
                        )
                        .italic()
                        .text_tone(&theme, TextTone::Primary)
                    }))
                    .semantic_in(
                        cx,
                        NodeSpec::new(ident.child("body").semantic_id(), Role::Text)
                            .parent(ident.semantic_id())
                            .value("present"),
                    ),
            ),
            Reasoning::Present(_) => None,
            Reasoning::Withheld(_) | Reasoning::Absent => None,
        };

        div()
            .w_full()
            .column()
            .gap(px(2.0))
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
