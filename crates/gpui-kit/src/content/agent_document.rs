//! A stream-friendly document made of typed, stable blocks.
//!
//! Markdown remains prose rather than becoming the wire format for every tool
//! result. A caller can place code, a diff, a schema, a chart, an image, a tool
//! call, choices, or another product-neutral element beside Markdown while
//! preserving one stable identity and revision per block.
//!
//! The component stores no conversation or stream. The caller replaces blocks
//! as revisions arrive, and [`AgentDocumentEvent`] reports Markdown actions
//! with the identity of the block that produced them. A reconnect therefore
//! updates the same block rather than appending another anonymous message.

use std::rc::Rc;

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, StyledText,
    Window, div, prelude::FluentBuilder,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TextTone, TypeScale};

use crate::content::markdown::{Markdown, MarkdownEvent};
use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::status::StatusLine;
use crate::foundation::{Ident, StyledExt};
use crate::motion;

type EventHandler = Rc<dyn Fn(&AgentDocumentEvent, &mut Window, &mut App)>;

/// The semantic kind of one document block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentBlockKind {
    Text,
    Markdown,
    Code,
    ToolCall,
    Diff,
    Artifact,
    Schema,
    Chart,
    Image,
    Notice,
    Choice,
    Custom,
}

impl AgentBlockKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Markdown => "markdown",
            Self::Code => "code",
            Self::ToolCall => "tool-call",
            Self::Diff => "diff",
            Self::Artifact => "artifact",
            Self::Schema => "schema",
            Self::Chart => "chart",
            Self::Image => "image",
            Self::Notice => "notice",
            Self::Choice => "choice",
            Self::Custom => "custom",
        }
    }
}

enum AgentBlockBody {
    Text(SharedString),
    Markdown(SharedString),
    Notice { message: SharedString, tone: Tone },
    Element(AnyElement),
}

impl std::fmt::Debug for AgentBlockBody {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(text) => formatter.debug_tuple("Text").field(&text.len()).finish(),
            Self::Markdown(source) => formatter
                .debug_tuple("Markdown")
                .field(&source.len())
                .finish(),
            Self::Notice { message, tone } => formatter
                .debug_struct("Notice")
                .field("bytes", &message.len())
                .field("tone", tone)
                .finish(),
            Self::Element(_) => formatter.write_str("Element(..)"),
        }
    }
}

/// One stable part of an [`AgentDocument`].
pub struct AgentDocumentBlock {
    id: SharedString,
    revision: u64,
    kind: AgentBlockKind,
    label: Option<SharedString>,
    streaming: bool,
    body: AgentBlockBody,
}

impl std::fmt::Debug for AgentDocumentBlock {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentDocumentBlock")
            .field("id", &self.id)
            .field("revision", &self.revision)
            .field("kind", &self.kind)
            .field("label", &self.label)
            .field("streaming", &self.streaming)
            .field("body", &self.body)
            .finish()
    }
}

impl AgentDocumentBlock {
    /// Plain selectable text, with no markup interpreted.
    pub fn text(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            revision: 0,
            kind: AgentBlockKind::Text,
            label: None,
            streaming: false,
            body: AgentBlockBody::Text(text.into()),
        }
    }

    /// Safe read-only Markdown under [`Markdown`]'s host-owned link and image
    /// policy.
    pub fn markdown(id: impl Into<SharedString>, source: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            revision: 0,
            kind: AgentBlockKind::Markdown,
            label: None,
            streaming: false,
            body: AgentBlockBody::Markdown(source.into()),
        }
    }

    /// A typed block whose existing component owns its presentation.
    pub fn element(
        id: impl Into<SharedString>,
        kind: AgentBlockKind,
        element: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            revision: 0,
            kind,
            label: None,
            streaming: false,
            body: AgentBlockBody::Element(element.into_any_element()),
        }
    }

    pub fn code(id: impl Into<SharedString>, element: impl IntoElement) -> Self {
        Self::element(id, AgentBlockKind::Code, element)
    }

    pub fn tool_call(id: impl Into<SharedString>, element: impl IntoElement) -> Self {
        Self::element(id, AgentBlockKind::ToolCall, element)
    }

    pub fn diff(id: impl Into<SharedString>, element: impl IntoElement) -> Self {
        Self::element(id, AgentBlockKind::Diff, element)
    }

    pub fn artifact(id: impl Into<SharedString>, element: impl IntoElement) -> Self {
        Self::element(id, AgentBlockKind::Artifact, element)
    }

    pub fn schema(id: impl Into<SharedString>, element: impl IntoElement) -> Self {
        Self::element(id, AgentBlockKind::Schema, element)
    }

    pub fn chart(id: impl Into<SharedString>, element: impl IntoElement) -> Self {
        Self::element(id, AgentBlockKind::Chart, element)
    }

    pub fn image(id: impl Into<SharedString>, element: impl IntoElement) -> Self {
        Self::element(id, AgentBlockKind::Image, element)
    }

    pub fn choice(id: impl Into<SharedString>, element: impl IntoElement) -> Self {
        Self::element(id, AgentBlockKind::Choice, element)
    }

    pub fn custom(id: impl Into<SharedString>, element: impl IntoElement) -> Self {
        Self::element(id, AgentBlockKind::Custom, element)
    }

    /// A host-authored status or warning inside the document.
    pub fn notice(
        id: impl Into<SharedString>,
        message: impl Into<SharedString>,
        tone: Tone,
    ) -> Self {
        Self {
            id: id.into(),
            revision: 0,
            kind: AgentBlockKind::Notice,
            label: None,
            streaming: false,
            body: AgentBlockBody::Notice {
                message: message.into(),
                tone,
            },
        }
    }

    /// The caller's monotonic version of this block.
    pub fn revision(mut self, revision: u64) -> Self {
        self.revision = revision;
        self
    }

    /// Marks this block as still arriving without changing what has arrived.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// A host-authored heading for a non-prose block.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }

    pub fn kind(&self) -> AgentBlockKind {
        self.kind
    }
}

/// What is known about the document as a whole.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AgentDocumentState {
    Idle(SharedString),
    Loading(SharedString),
    #[default]
    Ready,
    Empty(SharedString),
    Unavailable(SharedString),
    Failed(SharedString),
}

impl AgentDocumentState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle(_) => "idle",
            Self::Loading(_) => "loading",
            Self::Ready => "ready",
            Self::Empty(_) => "empty",
            Self::Unavailable(_) => "unavailable",
            Self::Failed(_) => "failed",
        }
    }
}

/// An action originating in a typed document block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentDocumentEvent {
    Markdown {
        block: SharedString,
        event: MarkdownEvent,
    },
}

/// A read-only sequence of typed agent output blocks.
#[derive(IntoElement)]
pub struct AgentDocument {
    ident: Ident,
    state: AgentDocumentState,
    blocks: Vec<AgentDocumentBlock>,
    on_event: Option<EventHandler>,
}

impl std::fmt::Debug for AgentDocument {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentDocument")
            .field("ident", &self.ident)
            .field("state", &self.state)
            .field("blocks", &self.blocks.len())
            .field("has_handler", &self.on_event.is_some())
            .finish()
    }
}

impl AgentDocument {
    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            state: AgentDocumentState::Ready,
            blocks: Vec::new(),
            on_event: None,
        }
    }

    pub fn state(mut self, state: AgentDocumentState) -> Self {
        self.state = state;
        self
    }

    pub fn block(mut self, block: AgentDocumentBlock) -> Self {
        self.blocks.push(block);
        self
    }

    pub fn blocks(mut self, blocks: impl IntoIterator<Item = AgentDocumentBlock>) -> Self {
        self.blocks.extend(blocks);
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(&AgentDocumentEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }

    /// Repeated block identities are returned exactly as supplied. The host
    /// decides whether repeated records are revisions, duplicates, or distinct
    /// content and gives them distinct ids where appropriate.
    pub fn duplicate_ids(&self) -> Vec<SharedString> {
        let mut seen = std::collections::HashSet::new();
        let mut repeated = std::collections::HashSet::new();
        for block in &self.blocks {
            if !seen.insert(block.id.clone()) {
                repeated.insert(block.id.clone());
            }
        }
        let mut repeated: Vec<_> = repeated.into_iter().collect();
        repeated.sort_by(|left, right| left.as_ref().cmp(right.as_ref()));
        repeated
    }
}

impl RenderOnce for AgentDocument {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state_name = self.state.as_str();
        let busy = matches!(&self.state, AgentDocumentState::Loading(_));
        let ident = self.ident.clone();
        let block_count = self.blocks.len();

        let content = match self.state {
            AgentDocumentState::Idle(label) => EmptyState::new(ident.child("idle"), label)
                .kind(EmptyKind::Unstarted)
                .into_any_element(),
            AgentDocumentState::Loading(label) => div()
                .w_full()
                .py_token(&theme, Space::Lg)
                .child(StatusLine::new(label, Tone::Info).busy(ident.child("loading")))
                .into_any_element(),
            AgentDocumentState::Empty(reason) => {
                EmptyState::new(ident.child("empty"), reason).into_any_element()
            }
            AgentDocumentState::Unavailable(reason) => {
                EmptyState::new(ident.child("unavailable"), reason)
                    .kind(EmptyKind::Unavailable)
                    .into_any_element()
            }
            AgentDocumentState::Failed(reason) => EmptyState::new(ident.child("failed"), reason)
                .kind(EmptyKind::Failed)
                .into_any_element(),
            AgentDocumentState::Ready => {
                let mut column = div().w_full().column().gap_token(&theme, Space::Lg);
                for block in self.blocks {
                    column = column.child(render_block(&ident, block, self.on_event.clone(), cx));
                }
                column.into_any_element()
            }
        };

        div().w_full().column().child(content).semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Region)
                .value(format!("{state_name}:{block_count}"))
                .busy(busy),
        )
    }
}

fn render_block(
    document: &Ident,
    block: AgentDocumentBlock,
    on_event: Option<EventHandler>,
    cx: &mut App,
) -> AnyElement {
    let theme = cx.theme().clone();
    let ident = document.child(format!("block.{}", block.id));
    let state = format!("{}:revision-{}", block.kind.as_str(), block.revision);
    let body = match block.body {
        AgentBlockBody::Text(text) => div()
            .w_full()
            .type_scale(&theme, TypeScale::Body)
            .text_tone(&theme, TextTone::Primary)
            .child(StyledText::new(text).selectable(ident.child("text").element_id()))
            .into_any_element(),
        AgentBlockBody::Markdown(source) => {
            let block_id = block.id.clone();
            Markdown::new(ident.child("markdown"), source)
                .when_some(on_event, |markdown, on_event| {
                    markdown.on_event(move |event, window, cx| {
                        on_event(
                            &AgentDocumentEvent::Markdown {
                                block: block_id.clone(),
                                event: event.clone(),
                            },
                            window,
                            cx,
                        );
                    })
                })
                .into_any_element()
        }
        AgentBlockBody::Notice { message, tone } => {
            crate::display::status::Callout::new(message, tone)
                .id(ident.child("notice"))
                .into_any_element()
        }
        AgentBlockBody::Element(element) => element,
    };

    let frame = div()
        .w_full()
        .column()
        .gap_token(&theme, Space::Sm)
        .children(block.label.map(|label| {
            div()
                .type_scale(&theme, TypeScale::Label)
                .text_tone(&theme, TextTone::Muted)
                .child(label)
        }))
        .child(body);

    let frame = if block.streaming {
        motion::breathe(frame, ident.child("streaming").element_id(), &theme, cx)
    } else {
        frame.into_any_element()
    };

    div()
        .w_full()
        .child(frame)
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Group)
                .parent(document.semantic_id())
                .value(state)
                .busy(block.streaming),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_kinds_have_stable_protocol_names() {
        assert_eq!(AgentBlockKind::ToolCall.as_str(), "tool-call");
        assert_eq!(AgentBlockKind::Schema.as_str(), "schema");
        assert_eq!(AgentBlockKind::Choice.as_str(), "choice");
    }

    #[test]
    fn duplicate_block_ids_are_reported_without_dropping_blocks() {
        let document = AgentDocument::new("document")
            .block(AgentDocumentBlock::text("same", "first"))
            .block(AgentDocumentBlock::markdown("same", "second"));

        assert_eq!(document.blocks.len(), 2);
        assert_eq!(document.duplicate_ids(), vec![SharedString::from("same")]);
    }
}
