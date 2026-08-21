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

use crate::content::markdown::{Markdown, MarkdownEvent, parse};
use crate::data::{List, ListItem};
use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::status::StatusLine;
use crate::foundation::{Ident, StyledExt};
use crate::motion;

type EventHandler = Rc<dyn Fn(&AgentDocumentEvent, &mut Window, &mut App)>;

/// Each block receives this many reading-order values for selectable runs.
/// A block count or a Markdown run count that reaches 2³² cannot be laid out
/// by this component in practice; partitioning here keeps nested Markdown runs
/// ordered inside their block instead of competing with sibling block orders.
const SELECTION_ORDER_STRIDE: u64 = 1 << 32;

/// Each row of a Markdown block drawn as rows receives this many reading-order
/// values, partitioning a block's stride between its parts the same way the
/// stride above partitions the document between its blocks.
const PART_ORDER_STRIDE: u64 = 1 << 16;

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

type Build = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

enum AgentBlockBody {
    Text(SharedString),
    Markdown(SharedString),
    Notice {
        message: SharedString,
        tone: Tone,
    },
    /// A typed block is a *way to build* its element rather than a built one.
    ///
    /// A finished element can be drawn once. A document that only lays out
    /// what is on screen has to be able to draw a block when the reader
    /// scrolls back to it, hours later, without having kept it laid out in
    /// between — so what it holds has to be the recipe, not the result.
    Element(Build),
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
    ///
    /// Takes a way to build the element rather than a built one, so the block
    /// can be drawn again when the reader scrolls back to it. See
    /// [`AgentDocument::virtualized`] for why that is not optional.
    pub fn element(
        id: impl Into<SharedString>,
        kind: AgentBlockKind,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            revision: 0,
            kind,
            label: None,
            streaming: false,
            body: AgentBlockBody::Element(Rc::new(build)),
        }
    }

    pub fn code(
        id: impl Into<SharedString>,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self::element(id, AgentBlockKind::Code, build)
    }

    pub fn tool_call(
        id: impl Into<SharedString>,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self::element(id, AgentBlockKind::ToolCall, build)
    }

    pub fn diff(
        id: impl Into<SharedString>,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self::element(id, AgentBlockKind::Diff, build)
    }

    pub fn artifact(
        id: impl Into<SharedString>,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self::element(id, AgentBlockKind::Artifact, build)
    }

    pub fn schema(
        id: impl Into<SharedString>,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self::element(id, AgentBlockKind::Schema, build)
    }

    pub fn chart(
        id: impl Into<SharedString>,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self::element(id, AgentBlockKind::Chart, build)
    }

    pub fn image(
        id: impl Into<SharedString>,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self::element(id, AgentBlockKind::Image, build)
    }

    pub fn choice(
        id: impl Into<SharedString>,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self::element(id, AgentBlockKind::Choice, build)
    }

    pub fn custom(
        id: impl Into<SharedString>,
        build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self::element(id, AgentBlockKind::Custom, build)
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
    visible_rows: Option<usize>,
    on_event: Option<EventHandler>,
}

impl std::fmt::Debug for AgentDocument {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentDocument")
            .field("ident", &self.ident)
            .field("state", &self.state)
            .field("blocks", &self.blocks.len())
            .field("visible_rows", &self.visible_rows)
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
            visible_rows: None,
            on_event: None,
        }
    }

    /// Lays out only the blocks that are on screen, in a frame this many
    /// blocks tall.
    ///
    /// A conversation is not a document that happens to be long. It is a
    /// surface that grows all day, is scrolled back through, and whose oldest
    /// blocks are a diff of four hundred lines and a tool call with a table in
    /// it. Drawn as a column, every one of those is laid out on every frame,
    /// including while the newest block is still arriving a token at a time —
    /// so the cost of showing the *last* block grows with everything said
    /// before it, which is the shape of a surface that gets slower the longer
    /// the conversation is useful.
    ///
    /// Virtualized, the cost of a frame is the cost of the screenful. That is
    /// why a block holds a way to build its element rather than a built one:
    /// there is no other way to draw a block that scrolled off and came back.
    ///
    /// Blocks are matched across frames by their ids, so a block whose text
    /// grew re-measures that block and nothing else. Without that a streaming
    /// reply would discard every height the list had learned on every token,
    /// and the scrollbar would shudder for as long as the answer took to
    /// arrive.
    ///
    /// The trade is the usual one: a block that has never been on screen has
    /// no bounds and publishes nothing, so a copy across the whole document
    /// reaches what was mounted rather than everything that exists.
    pub fn virtualized(mut self, rows: usize) -> Self {
        self.visible_rows = Some(rows.max(1));
        self
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
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
            AgentDocumentState::Ready => match self.visible_rows {
                Some(rows) => {
                    let blocks = Rc::new(self.blocks);
                    let plan = Rc::new(plan_rows(&blocks));
                    // Ids rather than a count, so a block that grew re-measures
                    // itself alone instead of discarding every height the list
                    // had learned.
                    let keys: Vec<SharedString> = plan.iter().map(|row| row.key(&blocks)).collect();
                    let count = plan.len();
                    let listed = Rc::clone(&blocks);
                    let rows_plan = Rc::clone(&plan);
                    let list_ident = ident.child("blocks");
                    let document = ident.clone();
                    let on_event = self.on_event.clone();
                    List::new(list_ident, count, move |index, window, cx| {
                        let row = &rows_plan[index];
                        let block = &listed[row.block];
                        let theme = cx.theme().clone();
                        // A row carries the space that follows it, because a
                        // list of rows has no gap of its own to give them.
                        // Between the blocks of one answer that is the space
                        // Markdown sets its own blocks in; between one block
                        // and the next it is the document's.
                        let after = match &row.part {
                            Some(part) if !part.last => Space::Md,
                            _ => Space::Lg,
                        };
                        ListItem::new(
                            row.key(&listed),
                            div()
                                .w_full()
                                .pb(gpui::px(theme.space(after)))
                                .child(render_block(
                                    &document,
                                    block,
                                    row.block as u64,
                                    row.part.as_ref(),
                                    on_event.clone(),
                                    window,
                                    cx,
                                ))
                                .into_any_element(),
                        )
                    })
                    // Blocks are as tall as what is in them: a line of prose
                    // and a four-hundred-line diff are both one block.
                    .flowing()
                    .keys(keys)
                    .visible_rows(rows)
                    .into_any_element()
                }
                None => {
                    let mut column = div().w_full().column().gap_token(&theme, Space::Lg);
                    for (order, block) in self.blocks.iter().enumerate() {
                        column = column.child(render_block(
                            &ident,
                            block,
                            // A block's place in the document is its reading
                            // order.
                            order as u64,
                            None,
                            self.on_event.clone(),
                            window,
                            cx,
                        ));
                    }
                    column.into_any_element()
                }
            },
        };

        div().w_full().column().child(content).semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Region)
                .value(format!("{state_name}:{block_count}"))
                .busy(busy),
        )
    }
}

/// One top-level Markdown block of a document block that was drawn as rows.
#[derive(Debug, Clone)]
struct Part {
    /// Which of its block's reading-order slices this part takes.
    index: usize,
    /// Nothing follows it inside its block, so it is the part a stream is
    /// still writing into.
    last: bool,
    range: std::ops::Range<usize>,
}

impl Part {
    /// What this part is called inside its block: where it starts, rather than
    /// which one it is.
    ///
    /// An answer grows at its end, so every part before the one being written
    /// keeps its offset for the life of the answer. An ordinal would rename
    /// every part after any block that is ever dropped, and a name that moves
    /// is a height re-measured, a semantic node replaced, and a reader's
    /// selection let go of.
    fn name(&self) -> String {
        format!("part-at-{}", self.range.start)
    }
}

/// One row of a virtualized document.
#[derive(Debug, Clone)]
struct PlannedRow {
    block: usize,
    /// `None` when the row is the whole block.
    part: Option<Part>,
}

impl PlannedRow {
    /// What the list matches this row by across frames.
    ///
    /// A part is matched by where it sits in its block rather than by what it
    /// says: a paragraph whose text grew is the same paragraph, and the row
    /// that has to re-measure is that one alone.
    fn key(&self, blocks: &[AgentDocumentBlock]) -> SharedString {
        let id = &blocks[self.block].id;
        match &self.part {
            Some(part) => SharedString::from(format!("{id}#{}", part.name())),
            None => id.clone(),
        }
    }
}

/// Which rows a virtualized document is drawn as.
///
/// Every block is a row, except a Markdown block long enough to have several
/// top-level blocks of its own: that one becomes a row per paragraph, fence,
/// list or table. It matters most for the block still arriving. Drawn whole, a
/// four-thousand-word answer is relaid out from its first word on every token;
/// drawn as rows, only the row the token landed in is, and the rest of the
/// answer is not even on screen.
///
/// A source this reader cannot cut at trustworthy boundaries stays one row,
/// because a fence split across two rows is worse than a long row.
fn plan_rows(blocks: &[AgentDocumentBlock]) -> Vec<PlannedRow> {
    let mut plan = Vec::with_capacity(blocks.len());
    for (index, block) in blocks.iter().enumerate() {
        let ranges = match &block.body {
            AgentBlockBody::Markdown(source) => parse::Document::block_ranges(source),
            _ => None,
        };
        match ranges {
            Some(ranges) if ranges.len() > 1 => {
                let last = ranges.len() - 1;
                for (part, range) in ranges.into_iter().enumerate() {
                    plan.push(PlannedRow {
                        block: index,
                        part: Some(Part {
                            index: part,
                            last: part == last,
                            range,
                        }),
                    });
                }
            }
            _ => plan.push(PlannedRow {
                block: index,
                part: None,
            }),
        }
    }
    plan
}

fn render_block(
    document: &Ident,
    block: &AgentDocumentBlock,
    order: u64,
    part: Option<&Part>,
    on_event: Option<EventHandler>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = cx.theme().clone();
    let ident = document.child(format!("block.{}", block.id));
    let ident = match part {
        Some(part) => ident.child(part.name()),
        None => ident,
    };
    let state = match part {
        Some(part) => format!(
            "{}:revision-{}:{}",
            block.kind.as_str(),
            block.revision,
            part.name()
        ),
        None => format!("{}:revision-{}", block.kind.as_str(), block.revision),
    };
    // A part takes its own slice of its block's partition, so a drag down a
    // split answer reads it in the order it was written.
    let selection_order = order
        .saturating_mul(SELECTION_ORDER_STRIDE)
        .saturating_add(part.map_or(0, |part| part.index as u64 * PART_ORDER_STRIDE));
    // Only the last part of a block is still being written into; the ones
    // before it settled the moment the block after them began.
    let streaming = block.streaming && part.is_none_or(|part| part.last);
    let body = match &block.body {
        AgentBlockBody::Text(text) => div()
            .w_full()
            .type_scale(&theme, TypeScale::Body)
            .text_tone(&theme, TextTone::Primary)
            .child({
                let text_ident = ident.child("text");
                StyledText::new(text.clone()).selectable_in_document(
                    text_ident.element_id(),
                    text_ident.semantic_id(),
                    selection_order,
                )
            })
            .into_any_element(),
        AgentBlockBody::Markdown(source) => {
            let block_id = block.id.clone();
            let source = match part {
                Some(part) => SharedString::from(source[part.range.clone()].to_string()),
                None => source.clone(),
            };
            Markdown::new(ident.child("markdown"), source)
                .selection_order_start(selection_order)
                .streaming(streaming)
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
            crate::display::status::Callout::new(message.clone(), *tone)
                .id(ident.child("notice"))
                .into_any_element()
        }
        AgentBlockBody::Element(build) => build(window, cx),
    };

    let frame = div()
        .w_full()
        .column()
        .gap_token(&theme, Space::Sm)
        // A block's heading names the block, so it belongs to the row the
        // block starts in and not to every row it runs through.
        .children(
            block
                .label
                .clone()
                .filter(|_| part.is_none_or(|part| part.index == 0))
                .map(|label| {
                    div()
                        .type_scale(&theme, TypeScale::Label)
                        .text_tone(&theme, TextTone::Muted)
                        .child(label)
                }),
        )
        .child(body);

    let frame = if streaming {
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
                .busy(streaming),
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
