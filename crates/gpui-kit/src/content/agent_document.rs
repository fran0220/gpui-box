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

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, StyledText,
    Window, div, prelude::FluentBuilder,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Space, TextTone, TypeScale};

use crate::content::markdown::{
    Markdown, MarkdownEvent, parse,
    stream::{BACKGROUND_BYTES, Background, Stream},
};
use crate::data::{List, ListItem};
use crate::display::badge::Tone;
use crate::display::empty::{EmptyKind, EmptyState};
use crate::display::status::StatusLine;
use crate::foundation::slot::{self, Slots, Slotted};
use crate::foundation::{Ident, StyledExt};
use crate::motion::keyed;
use crate::state::{HasPhase, Phase};
use crate::strings::{ActiveStrings, StringKey};

type EventHandler = Rc<dyn Fn(&AgentDocumentEvent, &mut Window, &mut App)>;
type MarkdownOptions = Rc<dyn Fn(&SharedString, Markdown) -> Markdown>;

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

    /// Supporting evidence in a transcript, rather than the answer or a
    /// media/result surface that deserves its own presentation.
    pub fn is_evidence(self) -> bool {
        matches!(
            self,
            Self::Code | Self::ToolCall | Self::Diff | Self::Schema | Self::Notice
        )
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

impl HasPhase for AgentDocumentState {
    fn phase(&self) -> Phase {
        match self {
            Self::Idle(_) => Phase::Idle,
            Self::Loading(_) => Phase::Loading,
            Self::Ready => Phase::Ready,
            Self::Empty(_) => Phase::Empty,
            Self::Unavailable(_) => Phase::Unavailable,
            Self::Failed(_) => Phase::Error,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Idle(reason)
            | Self::Loading(reason)
            | Self::Empty(reason)
            | Self::Unavailable(reason)
            | Self::Failed(reason) => Some(reason.as_ref()),
            Self::Ready => None,
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

/// Work performed by a mounted virtual document's retained plans.
/// Input checks are linear comparisons of caller records, not byte scans.
/// Parser totals cover currently retained Markdown blocks. These counters do
/// not include GPUI layout, key-vector copies into List, or wall-clock time.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AgentDocumentWork {
    pub input_checks: usize,
    pub planned_rows: usize,
    pub parser: crate::content::markdown::MarkdownWork,
}

/// A read-only sequence of typed agent output blocks.
#[derive(IntoElement)]
pub struct AgentDocument {
    ident: Ident,
    state: AgentDocumentState,
    blocks: Vec<AgentDocumentBlock>,
    visible_rows: Option<usize>,
    on_event: Option<EventHandler>,
    markdown_options: Option<MarkdownOptions>,
    slots: Slots,
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
    /// Invalidates geometry for this caller-owned block after asynchronous
    /// image resolution or a native plugin changes size. Unrelated rows and
    /// the current scroll anchor stay intact; semantic ids do not change.
    pub fn remeasure_block(ident: &Ident, block: &str, window: &mut Window, cx: &mut App) {
        let plans = keyed::slot::<RowPlans>(
            &ident.child("row-plans").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let plans = plans.borrow();
        for (index, row) in plans.plan.rows.iter().enumerate() {
            if plans.inputs[row.block].id.as_ref() == block {
                crate::data::viewport::remeasure_rows(
                    &ident.child("blocks"),
                    index..index + 1,
                    window,
                    cx,
                );
            }
        }
    }

    /// Reports actual retained planning work for this virtualized identity.
    pub fn work(ident: &Ident, window: &Window, cx: &mut App) -> AgentDocumentWork {
        let plans = keyed::slot::<RowPlans>(
            &ident.child("row-plans").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let plans = plans.borrow();
        let mut work = AgentDocumentWork {
            input_checks: plans.input_checks,
            planned_rows: plans.planned_rows,
            ..Default::default()
        };
        for plan in plans.markdown.values() {
            let parser = plan
                .background
                .as_ref()
                .map_or_else(|| plan.reader.work(), |background| background.borrow().work);
            work.parser.parser_passes += parser.parser_passes;
            work.parser.parsed_bytes += parser.parsed_bytes;
            work.parser.copied_bytes += parser.copied_bytes;
        }
        work
    }

    pub fn new(ident: impl Into<Ident>) -> Self {
        Self {
            ident: ident.into(),
            state: AgentDocumentState::Ready,
            blocks: Vec::new(),
            visible_rows: None,
            on_event: None,
            markdown_options: None,
            slots: Slots::default(),
        }
    }

    /// Configures each mounted Markdown row with host-owned image resolution,
    /// highlighting or block plugins. The block id is the caller's identity,
    /// independent of revision and virtualization. The callback is repeatable.
    pub fn configure_markdown(
        mut self,
        configure: impl Fn(&SharedString, Markdown) -> Markdown + 'static,
    ) -> Self {
        self.markdown_options = Some(Rc::new(configure));
        self
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

impl Slotted for AgentDocument {
    const SLOTS: &'static [&'static str] = &[slot::EMPTY, slot::FAILED, slot::LOADING];

    fn slots_mut(&mut self) -> &mut Slots {
        &mut self.slots
    }
}

impl RenderOnce for AgentDocument {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state_name = self.state.as_str();
        let mut busy = matches!(&self.state, AgentDocumentState::Loading(_));
        let ident = self.ident.clone();
        let block_count = self.blocks.len();

        let content = match self.state {
            AgentDocumentState::Idle(label) => {
                self.slots.or_else(slot::EMPTY, window, cx, |_, _| {
                    EmptyState::new(ident.child("idle"), label)
                        .kind(EmptyKind::Unstarted)
                        .into_any_element()
                })
            }
            AgentDocumentState::Loading(label) => {
                self.slots.or_else(slot::LOADING, window, cx, |_, _| {
                    div()
                        .w_full()
                        .py_token(&theme, Space::Lg)
                        .child(StatusLine::new(label, Tone::Info).busy(ident.child("loading")))
                        .into_any_element()
                })
            }
            AgentDocumentState::Empty(reason) => {
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        ident.child("empty"),
                        cx.strings().text(StringKey::AgentDocumentEmpty),
                    )
                    .detail(reason.clone())
                    .into_any_element()
                })
            }
            AgentDocumentState::Unavailable(reason) => {
                self.slots.or_else(slot::EMPTY, window, cx, |_, cx| {
                    EmptyState::new(
                        ident.child("unavailable"),
                        cx.strings().text(StringKey::AgentDocumentUnavailable),
                    )
                    .kind(EmptyKind::Unavailable)
                    .detail(reason.clone())
                    .into_any_element()
                })
            }
            AgentDocumentState::Failed(reason) => {
                self.slots.or_else(slot::FAILED, window, cx, |_, cx| {
                    EmptyState::new(
                        ident.child("failed"),
                        cx.strings().text(StringKey::AgentDocumentFailed),
                    )
                    .kind(EmptyKind::Failed)
                    .detail(reason.clone())
                    .into_any_element()
                })
            }
            AgentDocumentState::Ready => match self.visible_rows {
                Some(rows) => {
                    let blocks = Rc::new(self.blocks);
                    let retained = keyed::slot::<RowPlans>(
                        &ident.child("row-plans").semantic_id(),
                        window.window_handle().window_id(),
                        cx,
                    );
                    let mut retained = retained.borrow_mut();
                    busy |= retained.prepare(&blocks, window, cx);
                    let plan = retained.read(&blocks);
                    // Ids rather than a count, so a block that grew re-measures
                    // itself alone instead of discarding every height the list
                    // had learned.
                    let keys = plan.keys.clone();
                    let revisions = plan.revisions.clone();
                    let count = plan.rows.len();
                    let listed = Rc::clone(&blocks);
                    let rows_plan = Rc::clone(&plan);
                    let list_ident = ident.child("blocks");
                    let document = ident.clone();
                    let on_event = self.on_event.clone();
                    let markdown_options = self.markdown_options.clone();
                    List::new(list_ident, count, move |index, window, cx| {
                        let row = &rows_plan.rows[index];
                        let block = &listed[row.block];
                        let theme = cx.theme().clone();
                        // A row carries the space that follows it, because a
                        // list of rows has no gap of its own to give them.
                        // Between the blocks of one answer that is the space
                        // Markdown sets its own blocks in; between one block
                        // and the next it is the document's.
                        let after = match &row.part {
                            Some(part) if !part.last => Space::Md,
                            _ => rows_plan
                                .rows
                                .get(index + 1)
                                .map(|next| block_space(block.kind, listed[next.block].kind))
                                .unwrap_or(Space::Lg),
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
                                    (on_event.clone(), markdown_options.clone()),
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
                    .revisions(revisions)
                    .visible_rows(rows)
                    .into_any_element()
                }
                None => {
                    let mut column = div().w_full().column();
                    for (order, block) in self.blocks.iter().enumerate() {
                        let rendered = render_block(
                            &ident,
                            block,
                            // A block's place in the document is its reading
                            // order.
                            order as u64,
                            None,
                            (self.on_event.clone(), self.markdown_options.clone()),
                            window,
                            cx,
                        );
                        column = match self.blocks.get(order + 1) {
                            Some(next) => column.child(
                                div()
                                    .w_full()
                                    .pb(gpui::px(theme.space(block_space(block.kind, next.kind))))
                                    .child(rendered),
                            ),
                            None => column.child(rendered),
                        };
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

fn block_space(current: AgentBlockKind, next: AgentBlockKind) -> Space {
    match (current.is_evidence(), next.is_evidence()) {
        (true, true) => Space::Sm,
        (true, false) | (false, true) => Space::Md,
        (false, false) => Space::Lg,
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
    document: Rc<parse::Document>,
    revision: u64,
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
#[derive(Default)]
struct RowPlans {
    markdown: HashMap<SharedString, MarkdownPlan>,
    inputs: Vec<PlanInput>,
    plan: Rc<RowPlan>,
    input_checks: usize,
    planned_rows: usize,
}

struct PlanInput {
    id: SharedString,
    source: Option<SharedString>,
    revision: u64,
    streaming: bool,
    kind: AgentBlockKind,
    label: Option<SharedString>,
}

impl PlanInput {
    fn matches(&self, block: &AgentDocumentBlock) -> bool {
        let source = match &block.body {
            AgentBlockBody::Markdown(source) => Some(source),
            _ => None,
        };
        self.id == block.id
            && self.source.as_ref() == source
            && self.revision == block.revision
            && self.streaming == block.streaming
            && self.kind == block.kind
            && self.label == block.label
    }

    fn from_block(block: &AgentDocumentBlock) -> Self {
        Self {
            id: block.id.clone(),
            source: match &block.body {
                AgentBlockBody::Markdown(source) => Some(source.clone()),
                _ => None,
            },
            revision: block.revision,
            streaming: block.streaming,
            kind: block.kind,
            label: block.label.clone(),
        }
    }
}

#[derive(Default)]
struct RowPlan {
    rows: Vec<PlannedRow>,
    keys: Vec<SharedString>,
    revisions: Vec<u64>,
}

#[derive(Default)]
struct MarkdownPlan {
    source: Option<SharedString>,
    streaming: bool,
    reader: Stream,
    parts: Vec<Part>,
    generation: u64,
    background: Option<Rc<RefCell<Background>>>,
    background_generation: Option<u64>,
}

struct ParsedParts<'a> {
    document: &'a parse::Document,
    starts: &'a [usize],
    stable: usize,
    mended: Option<&'a parse::Document>,
}

impl MarkdownPlan {
    fn read(&mut self, source: &SharedString, streaming: bool) {
        if let Some(background) = &self.background {
            let background = background.borrow();
            if self.background_generation == Some(background.generation) {
                return;
            }
            self.generation += 1;
            self.parts = plan_parts(
                &self.parts,
                self.generation,
                &background.source,
                self.streaming,
                background.streaming,
                ParsedParts {
                    document: &background.document,
                    starts: &background.starts,
                    stable: background.stable,
                    mended: background.mended.as_deref(),
                },
            );
            if self.parts.is_empty() && background.pending() {
                self.parts.push(Part {
                    index: 0,
                    last: true,
                    range: 0..0,
                    document: background.document.clone(),
                    revision: self.generation,
                });
            }
            self.source = Some(background.source.clone());
            self.streaming = background.streaming;
            self.background_generation = Some(background.generation);
            return;
        }
        if self.source.as_ref() == Some(source) && self.streaming == streaming {
            return;
        }
        self.reader.read(source);
        self.generation += 1;
        let mended = streaming.then(|| self.reader.mended_tail()).flatten();
        self.parts = plan_parts(
            &self.parts,
            self.generation,
            source,
            self.streaming,
            streaming,
            ParsedParts {
                document: self.reader.document(),
                starts: self.reader.starts(),
                stable: self.reader.stable(),
                mended: mended.as_deref(),
            },
        );
        self.source = Some(source.clone());
        self.streaming = streaming;
    }
}

fn plan_parts(
    old: &[Part],
    generation: u64,
    source: &str,
    previous_streaming: bool,
    streaming: bool,
    parsed: ParsedParts<'_>,
) -> Vec<Part> {
    let ParsedParts {
        document,
        starts,
        stable,
        mended,
    } = parsed;
    let split = starts.len() == document.blocks.len();
    let count = if split { starts.len() } else { 1 };
    (0..count)
        .map(|index| {
            let range = if split {
                starts[index]..starts.get(index + 1).copied().unwrap_or(source.len())
            } else {
                0..source.len()
            };
            let last = index + 1 == count;
            if index < stable
                && let Some(part) = old.get(index)
                && part.range == range
                && part.last == last
                && !(last && previous_streaming != streaming)
            {
                return part.clone();
            }
            let parsed = if split {
                if last && let Some(mended) = mended {
                    mended.clone()
                } else {
                    parse::Document {
                        blocks: vec![document.blocks[index].clone()],
                    }
                }
            } else {
                document.clone()
            };
            let unchanged = old.get(index).filter(|part| {
                part.range == range && part.last == last && *part.document == parsed
            });
            Part {
                index,
                last,
                range,
                document: unchanged.map_or_else(|| Rc::new(parsed), |part| part.document.clone()),
                revision: unchanged.map_or(generation, |part| part.revision),
            }
        })
        .collect()
}

impl RowPlans {
    fn prepare(&mut self, blocks: &[AgentDocumentBlock], window: &Window, cx: &mut App) -> bool {
        let mut pending = false;
        for block in blocks {
            let AgentBlockBody::Markdown(source) = &block.body else {
                continue;
            };
            if source.len() >= BACKGROUND_BYTES {
                let plan = self.markdown.entry(block.id.clone()).or_default();
                let background = plan.background.get_or_insert_with(|| {
                    Rc::new(RefCell::new(Background::seeded(
                        std::mem::take(&mut plan.reader),
                        plan.streaming,
                    )))
                });
                Background::read(background, source.clone(), block.streaming, window, cx);
                let background = background.borrow();
                pending |= background.pending();
                if plan.background_generation != Some(background.generation) {
                    self.inputs.clear();
                }
            } else if let Some(plan) = self.markdown.get_mut(&block.id)
                && plan.background.take().is_some()
            {
                plan.source = None;
                plan.background_generation = None;
                self.inputs.clear();
            }
        }
        pending
    }

    fn read(&mut self, blocks: &[AgentDocumentBlock]) -> Rc<RowPlan> {
        if self.inputs.len() == blocks.len()
            && self.inputs.iter().zip(blocks).all(|(input, block)| {
                self.input_checks += 1;
                input.matches(block)
            })
        {
            return self.plan.clone();
        }
        let identities: std::collections::HashSet<_> =
            blocks.iter().map(|block| &block.id).collect();
        self.markdown.retain(|id, _| identities.contains(id));
        self.inputs = blocks.iter().map(PlanInput::from_block).collect();
        let mut plan = Vec::with_capacity(blocks.len());
        for (index, block) in blocks.iter().enumerate() {
            let parts = match &block.body {
                AgentBlockBody::Markdown(source) => {
                    let retained = self.markdown.entry(block.id.clone()).or_default();
                    retained.read(source, block.streaming);
                    Some(&retained.parts)
                }
                _ => None,
            };
            match parts {
                Some(parts) => {
                    for part in parts {
                        plan.push(PlannedRow {
                            block: index,
                            part: Some(part.clone()),
                        });
                    }
                }
                _ => plan.push(PlannedRow {
                    block: index,
                    part: None,
                }),
            }
        }
        self.planned_rows += plan.len();
        let keys = plan.iter().map(|row| row.key(blocks)).collect();
        let revisions = plan
            .iter()
            .enumerate()
            .map(|(index, row)| {
                use std::hash::{Hash, Hasher};
                let block = &blocks[row.block];
                let mut revision = std::collections::hash_map::DefaultHasher::new();
                block.kind.hash(&mut revision);
                if row.part.as_ref().is_none_or(|part| part.last) {
                    plan.get(index + 1)
                        .map(|next| blocks[next.block].kind)
                        .hash(&mut revision);
                }
                row.part
                    .as_ref()
                    .map_or(block.revision, |part| part.revision)
                    .hash(&mut revision);
                if row.part.as_ref().is_none_or(|part| part.index == 0) {
                    block.label.hash(&mut revision);
                }
                (block.streaming && row.part.as_ref().is_none_or(|part| part.last))
                    .hash(&mut revision);
                revision.finish()
            })
            .collect();
        self.plan = Rc::new(RowPlan {
            rows: plan,
            keys,
            revisions,
        });
        self.plan.clone()
    }
}

fn render_block(
    document: &Ident,
    block: &AgentDocumentBlock,
    order: u64,
    part: Option<&Part>,
    callbacks: (Option<EventHandler>, Option<MarkdownOptions>),
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (on_event, markdown_options) = callbacks;
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
            Markdown::new(ident.child("markdown"), source.clone())
                .when_some(part, |markdown, part| {
                    markdown
                        .parsed(part.document.clone())
                        .parsing(part.document.blocks.is_empty())
                })
                .selection_order_start(selection_order)
                .streaming(streaming)
                .when_some(markdown_options, |markdown, configure| {
                    configure(&block.id, markdown)
                })
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
        .gap_token(
            &theme,
            if block.kind.is_evidence() {
                Space::Xs
            } else {
                Space::Sm
            },
        )
        // A block's heading names the block, so it belongs to the row the
        // block starts in and not to every row it runs through.
        .children(
            block
                .label
                .clone()
                .filter(|_| part.is_none_or(|part| part.index == 0))
                .map(|label| {
                    div()
                        .type_scale(
                            &theme,
                            if block.kind.is_evidence() {
                                TypeScale::Caption
                            } else {
                                TypeScale::Label
                            },
                        )
                        .text_tone(
                            &theme,
                            if block.kind.is_evidence() {
                                TextTone::Faint
                            } else {
                                TextTone::Muted
                            },
                        )
                        .child(label)
                }),
        )
        .child(body);

    let rendered = div()
        .w_full()
        .child(frame)
        .semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Group)
                .parent(document.semantic_id())
                .value(state)
                .busy(streaming),
        )
        .into_any_element();
    if part.is_some_and(|part| part.index == 0) {
        // The caller's block remains addressable when its first paragraph
        // becomes several virtual rows. Part identities belong underneath it.
        div()
            .w_full()
            .child(rendered)
            .semantic_in(
                cx,
                NodeSpec::new(
                    document.child(format!("block.{}", block.id)).semantic_id(),
                    Role::Group,
                )
                .parent(document.semantic_id())
                .value(format!(
                    "{}:revision-{}",
                    block.kind.as_str(),
                    block.revision
                ))
                .busy(block.streaming),
            )
            .into_any_element()
    } else {
        rendered
    }
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

    #[test]
    fn static_long_document_reuses_plans_without_parser_work() {
        let source: SharedString = (0..2000)
            .map(|i| format!("Paragraph {i}.\n\n"))
            .collect::<String>()
            .into();
        let mut plan = MarkdownPlan::default();
        plan.read(&source, false);
        let first = plan.parts[0].document.clone();
        let work = plan.reader.work();
        assert_eq!(work.parser_passes, 1);
        for _ in 0..100 {
            plan.read(&source, false);
        }
        assert_eq!(plan.reader.work(), work);
        assert!(Rc::ptr_eq(&first, &plan.parts[0].document));
        let grown = format!("{source}A new tail.").into();
        plan.read(&grown, true);
        assert!(Rc::ptr_eq(&first, &plan.parts[0].document));
        assert!(plan.reader.work().parsed_bytes - work.parsed_bytes < 100);
        assert_eq!(plan.parts[0].revision, 1);
    }

    #[test]
    fn ten_thousand_blocks_have_linear_static_checks_and_no_row_replanning() {
        let mut plans = RowPlans::default();
        let mut blocks: Vec<_> = (0..10_000)
            .map(|index| {
                AgentDocumentBlock::markdown(
                    format!("message-{index}"),
                    format!("Message {index}."),
                )
            })
            .collect();
        let first = plans.read(&blocks);
        for _ in 0..10 {
            assert!(Rc::ptr_eq(&first, &plans.read(&blocks)));
        }
        assert_eq!(plans.input_checks, 100_000);
        assert_eq!(plans.planned_rows, 10_000);
        assert_eq!(
            plans
                .markdown
                .values()
                .map(|plan| plan.reader.work().parser_passes)
                .sum::<usize>(),
            10_000
        );
        blocks.pop();
        let next = plans.read(&blocks);
        assert_eq!(plans.markdown.len(), 9_999);
        assert_eq!(next.rows.len(), 9_999);
        assert_eq!(first.keys[0], next.keys[0]);
        assert_eq!(first.revisions[0], next.revisions[0]);
    }

    #[test]
    fn row_revisions_change_only_for_affected_markdown_content() {
        let mut plans = RowPlans::default();
        let first = plans
            .read(&[AgentDocumentBlock::markdown("answer", "One.\n\nTwo.\n\nThree.").revision(1)]);
        let next =
            plans.read(&[
                AgentDocumentBlock::markdown("answer", "One.\n\nTwo.\n\nThree. More.").revision(2),
            ]);
        assert_eq!(first.keys, next.keys);
        assert_eq!(first.revisions[..2], next.revisions[..2]);
        assert_ne!(first.revisions[2], next.revisions[2]);
    }

    #[test]
    fn planned_rows_preserve_reference_context_and_replacement() {
        let mut plan = MarkdownPlan::default();
        for source in [
            "See [home].\n\nOther paragraph.\n\n[home]: /index\n",
            "See [home].\n\nOther paragraph.\n\n[home]: /changed\n",
        ] {
            plan.read(&source.into(), false);
            let blocks: Vec<_> = plan
                .parts
                .iter()
                .flat_map(|part| part.document.blocks.clone())
                .collect();
            assert_eq!(blocks, parse::Document::parse(source).blocks);
            assert_eq!(plan.parts[0].name(), "part-at-0");
        }
        assert_eq!(plan.parts[0].revision, 2);
    }

    #[test]
    fn streaming_plan_settles_hanging_markers_without_renaming_rows() {
        let mut plan = MarkdownPlan::default();
        let source: SharedString = "Stable.\n\n**hanging".into();
        plan.read(&source, true);
        let first = plan.parts[0].document.clone();
        let last = plan.parts.last().expect("hanging paragraph").clone();
        plan.read(&source, false);
        assert!(Rc::ptr_eq(&first, &plan.parts[0].document));
        let settled = plan.parts.last().expect("settled paragraph");
        assert_eq!(last.name(), settled.name());
        assert_ne!(last.document, settled.document);
        assert_ne!(last.revision, settled.revision);
    }
}

#[cfg(test)]
mod agent_document_phase_tests {
    use super::*;

    #[test]
    fn idle_and_failed_keep_their_host_sentences() {
        let idle = AgentDocumentState::Idle("nobody asked".into());
        assert_eq!(idle.phase(), Phase::Idle);
        assert_eq!(idle.reason(), Some("nobody asked"));
        let failed = AgentDocumentState::Failed("offline".into());
        assert_eq!(failed.phase(), Phase::Error);
        assert_eq!(failed.reason(), Some("offline"));
    }
}
