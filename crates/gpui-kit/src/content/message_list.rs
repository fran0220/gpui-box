//! A conversation, over the one virtualized list this library has.
//!
//! # The list does not own the conversation
//!
//! Messages, their order, their authors, and their delivery are all the
//! caller's. This component renders what it is handed and reports what was
//! asked for: a failed message stays exactly where it is, with its text
//! intact, and a retry is a request rather than a resend.
//!
//! # Times are strings the host already wrote
//!
//! The time on a message is an [`EntryTime`], the same type
//! [`Timeline`](crate::display::timeline::Timeline) takes, for the same reason: turning
//! an instant into words is calendar, time-zone and locale work, and whoever
//! owns the clock owns the wording. A message whose time nobody knows says so
//! rather than being floated to an end that would claim when it happened.
//!
//! # One message, one slot — unless the caller says otherwise
//!
//! By default virtualization here is a uniform [`List`], so every row is the
//! same height. That is the whole reason a conversation of ten thousand
//! messages costs the same as one of ten, and the price is that a message
//! occupies a slot rather than growing to fit: [`MessageList::body_lines`]
//! decides how tall the slot is, and a body longer than that says how many
//! lines it left out instead of being quietly cut off.
//!
//! [`MessageList::grows_to_fit`] buys the other trade: each message is as tall
//! as what it says, nothing is left out, and the list measures a message when
//! it first comes into view. It costs an exact answer to "how far does this
//! conversation reach" for the part of it nobody has scrolled through, which
//! is what a scrollbar over one is settling towards as the reader moves.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_assets::{Icon, icon};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, ControlSize, Radius, Space, Surface, Theme, TypeScale};
use web_time::Instant;

use crate::content::markdown::{Markdown, MarkdownEvent};
use crate::controls::button::Button;
use crate::data::list::{List, ListItem, scroll_to_row};
use crate::display::avatar::Avatar;
use crate::display::badge::Tone;
use crate::display::status::StatusDot;
use crate::display::timeline::EntryTime;
use crate::foundation::{Ident, Sizable, StyledExt};
use crate::motion::keyed;
use crate::motion::{engage_end, follow_end, follows_end};
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey, Strings};

/// What a host said about a message's journey.
///
/// Five states, five renderings. Collapsing sent, delivered, and read into one
/// tick tells the reader less than the host knows, and folding a failure into
/// any of them tells them something untrue.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DeliveryState {
    /// Handed over, not yet acknowledged.
    Sending,
    /// The host accepted it.
    #[default]
    Sent,
    /// It reached the other side.
    Delivered,
    /// The other side opened it.
    Read,
    /// It did not arrive, and here is why, in the host's own words.
    Failed { reason: SharedString },
}

impl DeliveryState {
    /// The name a node publishes, so a test asserts the state rather than the
    /// colour it was drawn in.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Delivered => "delivered",
            Self::Read => "read",
            Self::Failed { .. } => "failed",
        }
    }

    fn label(&self, strings: &Strings) -> SharedString {
        match self {
            Self::Sending => strings.text(StringKey::MessageSending),
            Self::Sent => strings.text(StringKey::MessageSent),
            Self::Delivered => strings.text(StringKey::MessageDelivered),
            Self::Read => strings.text(StringKey::MessageRead),
            Self::Failed { reason } => reason.clone(),
        }
    }

    fn tone(&self) -> Tone {
        match self {
            Self::Sending => Tone::Neutral,
            Self::Sent => Tone::Info,
            Self::Delivered => Tone::Accent,
            Self::Read => Tone::Success,
            Self::Failed { .. } => Tone::Danger,
        }
    }

    pub fn failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }
}

/// What a message says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageBody {
    /// Shown exactly as it is, with no markup read into it.
    Text(SharedString),
    /// Rendered through [`Markdown`], with its rules about HTML, links, and
    /// images.
    Markdown(SharedString),
}

impl MessageBody {
    /// The text as the caller wrote it, markup and all.
    pub fn source(&self) -> &SharedString {
        match self {
            Self::Text(text) | Self::Markdown(text) => text,
        }
    }
}

impl From<&'static str> for MessageBody {
    fn from(value: &'static str) -> Self {
        Self::Text(SharedString::new_static(value))
    }
}

impl From<String> for MessageBody {
    fn from(value: String) -> Self {
        Self::Text(SharedString::from(value))
    }
}

/// Something carried alongside a message. The list names it and fetches
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    id: SharedString,
    name: SharedString,
    detail: Option<SharedString>,
}

impl Attachment {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            detail: None,
        }
    }

    /// A size, a kind, or whatever else the host already knows about it.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// A reaction and how many people left it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reaction {
    key: SharedString,
    label: SharedString,
    count: usize,
}

impl Reaction {
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>, count: usize) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            count,
        }
    }
}

/// One turn in a conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    id: SharedString,
    author: Option<SharedString>,
    time: EntryTime,
    body: MessageBody,
    delivery: DeliveryState,
    streaming: bool,
    attachments: Vec<Attachment>,
    reactions: Vec<Reaction>,
}

impl Message {
    /// `id` is the message's own identity, which every node it publishes is
    /// derived from, so an assertion survives the conversation growing above
    /// it.
    pub fn new(id: impl Into<SharedString>, body: impl Into<MessageBody>) -> Self {
        Self {
            id: id.into(),
            author: None,
            time: EntryTime::Unknown,
            body: body.into(),
            delivery: DeliveryState::default(),
            streaming: false,
            attachments: Vec::new(),
            reactions: Vec::new(),
        }
    }

    pub fn markdown(id: impl Into<SharedString>, source: impl Into<SharedString>) -> Self {
        Self::new(id, MessageBody::Markdown(source.into()))
    }

    pub fn author(mut self, author: impl Into<SharedString>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// The time, in the words the host chose.
    pub fn time(mut self, time: impl Into<EntryTime>) -> Self {
        self.time = time.into();
        self
    }

    pub fn delivery(mut self, delivery: DeliveryState) -> Self {
        self.delivery = delivery;
        self
    }

    pub fn failed(self, reason: impl Into<SharedString>) -> Self {
        self.delivery(DeliveryState::Failed {
            reason: reason.into(),
        })
    }

    /// Marks the message as still arriving. The body keeps whatever has
    /// arrived so far.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    pub fn attachment(mut self, attachment: Attachment) -> Self {
        self.attachments.push(attachment);
        self
    }

    pub fn reaction(mut self, reaction: Reaction) -> Self {
        self.reactions.push(reaction);
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

type RetryHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type MarkdownHandler = Rc<dyn Fn(SharedString, &MarkdownEvent, &mut Window, &mut App)>;

/// A conversation.
#[derive(IntoElement)]
pub struct MessageList {
    ident: Ident,
    messages: Vec<Message>,
    visible_rows: Option<usize>,
    /// How many lines of body a slot holds, or `None` for a conversation whose
    /// messages are as tall as what they say.
    body_lines: Option<usize>,
    group_consecutive: bool,
    on_retry: Option<RetryHandler>,
    on_markdown: Option<MarkdownHandler>,
}

impl std::fmt::Debug for MessageList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MessageList")
            .field("ident", &self.ident)
            .field("messages", &self.messages.len())
            .field("visible_rows", &self.visible_rows)
            .field("group_consecutive", &self.group_consecutive)
            .finish()
    }
}

impl MessageList {
    pub fn new(ident: impl Into<Ident>, messages: impl IntoIterator<Item = Message>) -> Self {
        Self {
            ident: ident.into(),
            messages: messages.into_iter().collect(),
            visible_rows: None,
            body_lines: Some(3),
            group_consecutive: false,
            on_retry: None,
            on_markdown: None,
        }
    }

    /// Bounds the viewport to `rows` messages, which is what lets the list
    /// skip the ones it does not show.
    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = Some(rows);
        self
    }

    /// How many lines of body each message's slot holds.
    pub fn body_lines(mut self, lines: usize) -> Self {
        self.body_lines = Some(lines.max(1));
        self
    }

    /// Lets each message be as tall as what it says.
    ///
    /// The default is a slot: every message is the same height, which is what
    /// makes a conversation of ten thousand cost the same as one of ten, and a
    /// body longer than the slot says how many lines it left out. This trades
    /// that for messages that are whole. The list then measures a message when
    /// it first comes into view, so it can only estimate how far a conversation
    /// it has not scrolled through reaches, and a scrollbar over one settles as
    /// the reader moves down it.
    ///
    /// This and [`MessageList::body_lines`] are the same setting: the last one
    /// named wins.
    pub fn grows_to_fit(mut self) -> Self {
        self.body_lines = None;
        self
    }

    /// Whether consecutive messages from one author are drawn as one turn.
    ///
    /// Declared, not inferred, for the same reason
    /// [`Toolbar::overflow_after`](crate::layout::Toolbar::overflow_after) is:
    /// whether two messages a minute apart are one turn or two is a product
    /// judgement about the conversation, and this component knows nothing
    /// about the conversation.
    pub fn group_consecutive(mut self, group: bool) -> Self {
        self.group_consecutive = group;
        self
    }

    /// Reports that a failed message should be tried again. Nothing is resent
    /// and nothing is removed.
    pub fn on_retry(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_retry = Some(Rc::new(handler));
        self
    }

    /// Forwards what a Markdown body reported, stamped with the message it
    /// came from.
    pub fn on_markdown(
        mut self,
        handler: impl Fn(SharedString, &MarkdownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_markdown = Some(Rc::new(handler));
        self
    }

    /// The height of one slot: a header line, some body, and a footer line,
    /// inside the row's own padding.
    ///
    /// For a conversation that grows to fit, this is not a height but an
    /// estimate: what a message nobody has laid out yet is assumed to be, so a
    /// scrollbar starts roughly right rather than starting wrong.
    fn row_height(&self, theme: &Theme) -> f32 {
        // The gap between two message surfaces, then the surface's own
        // padding: a slot that counted only one of them would cut the last
        // line of every message it held.
        theme.space(Space::Xs) * 2.0
            + theme.space(Space::Sm) * 2.0
            + theme.typography.caption.line_height
            + theme.space(Space::Xs) * 2.0
            + theme.typography.body.line_height * self.body_lines.unwrap_or(3) as f32
            + theme.control.get(ControlSize::Sm).height
    }
}

impl RenderOnce for MessageList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();
        let count = self.messages.len();
        let row_height = self.row_height(&theme);
        let body_lines = self.body_lines;
        let group = self.group_consecutive;
        let on_retry = self.on_retry.clone();
        let on_markdown = self.on_markdown.clone();

        // A conversation whose rows are as tall as their content is measured
        // in pixels, so following it is a physical question — how far the view
        // sits above the end, and how fast that end is receding. A conversation
        // cut to a fixed number of lines per message has no such measurements,
        // and follows by naming the row to bring into view.
        let flowing = body_lines.is_none();
        let held = flowing && follows_end(&ident, window, cx);

        let follow = self.follow(count, held, window, cx);
        if follow.stick {
            if flowing {
                engage_end(&ident, window, cx);
            } else {
                scroll_to_row(&ident, count.saturating_sub(1), window, cx);
            }
        }
        if flowing {
            // Runs every frame, not only when something arrived: the end keeps
            // moving while a reply streams into the last message, and the
            // travel toward it is spread over frames rather than taken at once.
            follow_end(&ident, window, cx);
        }
        let pending = self.pending(&follow, &theme, cx);

        let messages = Rc::new(self.messages);
        let seen = follow.cell.clone();
        let list_ident = ident.clone();
        // A conversation that grows to fit is measured row by row, so it names
        // its rows: a message arriving anywhere but the end then re-measures
        // itself instead of every message after it.
        let row_keys: Vec<SharedString> =
            messages.iter().map(|message| message.id.clone()).collect();
        // The same names say which messages the conversation still holds, so
        // per-message state goes when its message does rather than when its
        // row happens to stop being drawn.
        keyed::retain_scope::<Stream>(
            &ident.semantic_id(),
            &row_keys,
            window.window_handle().window_id(),
            cx,
        );
        let rows = List::new(ident.clone(), count, move |index, window, cx| {
            if let Some(highest) = seen.borrow_mut().current.as_mut() {
                *highest = (*highest).max(index);
            } else {
                seen.borrow_mut().current = Some(index);
            }
            let Some(message) = messages.get(index) else {
                return ListItem::new("unknown", div());
            };
            let continues = group
                && index > 0
                && messages
                    .get(index - 1)
                    .is_some_and(|previous| previous.author == message.author);
            ListItem::new(
                message.id.clone(),
                row(
                    &list_ident,
                    message,
                    continues,
                    body_lines,
                    on_retry.as_ref(),
                    on_markdown.as_ref(),
                    window,
                    cx,
                ),
            )
            .text(shown_author(message.author.as_ref()))
        })
        .row_height(row_height)
        .when(body_lines.is_none(), |list| list.flowing().keys(row_keys))
        .when_some(self.visible_rows, List::visible_rows);

        div()
            .column()
            .w_full()
            .relative()
            .child(rows)
            .children(pending)
    }
}

/// What the list knows about where the reader is and what has arrived.
#[derive(Debug, Default)]
struct Following {
    /// Whether this identity has rendered before. Nothing is "new" on the
    /// frame a conversation first appears.
    started: bool,
    /// How many messages there were last time.
    count: usize,
    /// The highest index rendered in the frame being built.
    current: Option<usize>,
    /// The highest index rendered in the previous frame, which is where the
    /// reader was.
    previous: Option<usize>,
    /// The highest index the reader has ever had on screen.
    ever: Option<usize>,
    /// How many messages arrived while the reader was somewhere else.
    arrived: usize,
}

/// What one frame decided about following.
struct Follow {
    cell: Rc<RefCell<Following>>,
    /// Whether this frame should bring the newest message into view.
    stick: bool,
    /// How many messages sit below the viewport that the reader has not seen.
    below: usize,
    /// How many of those arrived after the conversation was first drawn.
    arrived: usize,
}

impl std::fmt::Debug for Follow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Follow")
            .field("stick", &self.stick)
            .field("below", &self.below)
            .field("arrived", &self.arrived)
            .finish()
    }
}

impl MessageList {
    /// Decides, once per frame, whether the newest message is followed.
    ///
    /// Following is conditional on purpose: dragging the reader back down
    /// while they are reading something further up takes the conversation away
    /// from them. When it does not follow it says how many messages it did not
    /// follow, which is the same rule
    /// [`ScrollArea`](crate::layout::ScrollArea) keeps — content continuing
    /// past the view is published rather than left to be noticed.
    fn follow(&self, count: usize, held: bool, window: &Window, cx: &mut App) -> Follow {
        let cell = keyed::slot::<Following>(
            &self.ident.child("following").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let mut following = cell.borrow_mut();
        following.previous = following.current.take();
        if let Some(previous) = following.previous {
            following.ever = Some(following.ever.map_or(previous, |ever| ever.max(previous)));
        }

        // Before the first frame has drawn a row, where the reader is has to
        // come from the viewport the caller asked for: a list opens at its
        // top.
        let reach = |highest: Option<usize>| match highest {
            Some(highest) => highest + 1,
            None => self.visible_rows.unwrap_or(count),
        };
        // A conversation that is holding its end is at the end even mid-glide,
        // when the last row it has drawn is not yet the last row there is.
        // Reading the rows alone would call that being away and start counting
        // at a reader who has not gone anywhere.
        let at_bottom = held || reach(following.previous) >= following.count.max(1);

        let mut stick = false;
        if !following.started {
            following.started = true;
        } else if count > following.count {
            if at_bottom {
                stick = true;
            } else {
                following.arrived += count - following.count;
            }
        }
        following.count = count;

        let mut below = count.saturating_sub(reach(following.ever));
        if below == 0 {
            following.arrived = 0;
        }
        let arrived = following.arrived.min(below);
        // A frame that is following the newest message is not behind it, even
        // though the row it is scrolling to has not been drawn yet.
        if stick || held {
            below = 0;
        }
        drop(following);

        Follow {
            cell,
            stick,
            below,
            arrived,
        }
    }

    /// The affordance naming what is below the view, when anything is.
    fn pending(&self, follow: &Follow, theme: &Theme, cx: &mut App) -> Option<AnyElement> {
        if follow.below == 0 {
            return None;
        }
        // Messages that arrived while the reader was away are a different fact
        // from messages that have simply always been further down, so they are
        // worded differently rather than counted together.
        let counted = if follow.arrived > 0 {
            follow.arrived
        } else {
            follow.below
        };
        let strings = cx.strings();
        let digits = cx.numbers().count(counted);
        let (one, other) = if follow.arrived == 0 {
            (StringKey::MessageMoreOne, StringKey::MessageMoreMany)
        } else {
            (StringKey::MessageNewOne, StringKey::MessageNewMany)
        };
        let label =
            strings.format_plural(one, other, cx.numbers().plural(counted), &[digits.as_ref()]);
        let ident = self.ident.child("pending");
        let list = self.ident.clone();
        let last = self.messages.len().saturating_sub(1);
        let flowing = self.body_lines.is_none();
        let cell = follow.cell.clone();

        Some(
            div()
                .absolute()
                .bottom(px(theme.space(Space::Sm)))
                .right(px(theme.space(Space::Sm)))
                .child(
                    Button::new(ident.child("follow"))
                        .label(label.clone())
                        .secondary()
                        .control_size(ControlSize::Sm)
                        .on_click(move |window, cx| {
                            cell.borrow_mut().arrived = 0;
                            // Going back to the newest message is a journey
                            // with a direction, so it is travelled rather than
                            // cut to: the reader sees what they are passing
                            // and where they land.
                            if flowing {
                                engage_end(&list, window, cx);
                            } else {
                                scroll_to_row(&list, last, window, cx);
                            }
                            window.refresh();
                        }),
                )
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Status)
                        .parent(self.ident.semantic_id())
                        .text(label)
                        .value(digits),
                )
                .into_any_element(),
        )
    }
}

/// When the indicator for a streaming message started.
#[derive(Debug, Default)]
struct Stream(Option<Instant>);

/// When the streaming indicator on `message` in `list` began.
///
/// Keyed by the message's identity and nothing else, so text arriving into a
/// message that is already streaming does not restart the indicator: a
/// conversation whose "still writing" mark flickered on every token would be
/// reporting the token, not the stream. Answers `None` once the message stops
/// streaming.
pub fn streaming_since(list: &Ident, message: &str, window: &Window, cx: &App) -> Option<Instant> {
    keyed::scoped_peek::<Stream>(
        &list.semantic_id(),
        &stream_key(message),
        window.window_handle().window_id(),
        cx,
    )
    .and_then(|stream| stream.borrow().0)
}

fn stream_key(message: &str) -> SharedString {
    SharedString::from(message.to_string())
}

/// The instant this message's stream began, started on first sight.
///
/// The clock belongs to the conversation rather than to the row, because a
/// message can stop streaming while it is scrolled out of view: a clock tied
/// to the element would be left behind by the frame that would have cleared
/// it, and a conversation that ran long enough would accumulate one entry per
/// message it had ever streamed. Tied to the list, it goes when the list does.
fn stream_began(
    list: &Ident,
    message: &str,
    streaming: bool,
    window: &Window,
    cx: &mut App,
) -> Option<Instant> {
    let cell = keyed::scoped_slot::<Stream>(
        &list.semantic_id(),
        &stream_key(message),
        window.window_handle().window_id(),
        cx,
    );
    if !streaming {
        cell.borrow_mut().0 = None;
        return None;
    }
    let now = cx.background_executor().now();
    let mut stream = cell.borrow_mut();
    Some(*stream.0.get_or_insert(now))
}

fn shown_author(author: Option<&SharedString>) -> SharedString {
    // An author nobody named is named `unknown`, because a blank byline reads
    // as a message with no author rather than one whose author was not
    // recorded.
    match author {
        Some(author) if !author.trim().is_empty() => author.clone(),
        _ => SharedString::new_static("unknown"),
    }
}

#[allow(clippy::too_many_arguments)]
fn row(
    list: &Ident,
    message: &Message,
    continues: bool,
    body_lines: Option<usize>,
    on_retry: Option<&RetryHandler>,
    on_markdown: Option<&MarkdownHandler>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = cx.theme().clone();
    let ident = list.child(message.id.as_ref());
    let author = shown_author(message.author.as_ref());
    let began = stream_began(list, message.id.as_ref(), message.streaming, window, cx);

    let header = div()
        .row()
        .w_full()
        .gap_token(&theme, Space::Sm)
        .type_scale(&theme, TypeScale::Caption)
        .h(px(theme.typography.caption.line_height))
        // A continued turn keeps the space its byline would occupy, because
        // the slot is one height and a hole where a name was is not a saving.
        .when(!continues, |element| {
            element
                .child(Avatar::new(author.clone()).size(theme.typography.caption.line_height))
                .child(
                    div()
                        .text_color(theme.colors.text)
                        .child(author.clone())
                        .semantic_in(
                            cx,
                            NodeSpec::new(ident.child("author").semantic_id(), Role::Text)
                                .parent(ident.semantic_id())
                                .text(author.clone()),
                        ),
                )
        })
        .child(
            div()
                .text_color(if message.time == EntryTime::Unknown {
                    theme.colors.warning
                } else {
                    theme.colors.text_faint
                })
                .child(message.time.shown(cx))
                .semantic_in(
                    cx,
                    NodeSpec::new(ident.child("time").semantic_id(), Role::Text)
                        .parent(ident.semantic_id())
                        .text(message.time.shown(cx))
                        .value(match &message.time {
                            EntryTime::At(time) => time.clone(),
                            EntryTime::Unknown => SharedString::new_static("time unknown"),
                        }),
                ),
        )
        .children(began.map(|_| streaming_mark(&ident, &theme, cx)));

    let body = body_element(&ident, message, body_lines, on_markdown, &theme, cx);

    let mut footer = div()
        .row()
        .w_full()
        .gap_token(&theme, Space::Sm)
        .h(px(theme.control.get(ControlSize::Sm).height))
        .child(delivery_mark(
            &ident,
            &message.delivery,
            message.streaming,
            &theme,
            cx,
        ));

    // What the slot cut off is reported next to the delivery mark rather than
    // under the body: the body is clipped to the slot, so a note drawn after
    // its last line is a note nobody can read.
    if let Some(hidden) = lines_left_out(message, body_lines) {
        footer = footer.child(truncation_mark(&ident, hidden, &theme, cx));
    }

    for attachment in &message.attachments {
        footer = footer.child(attachment_chip(&ident, attachment, &theme, cx));
    }
    for reaction in &message.reactions {
        footer = footer.child(reaction_chip(&ident, reaction, &theme, cx));
    }

    // A failed message keeps everything it had and gains one thing: a way to
    // ask for it to be tried again. It is never removed, and nothing here
    // retries it.
    if let (DeliveryState::Failed { .. }, Some(handler)) = (&message.delivery, on_retry) {
        let id = message.id.clone();
        let handler = Rc::clone(handler);
        footer = footer.child(
            Button::new(ident.child("retry"))
                .label(cx.strings().text(StringKey::TryAgain))
                .secondary()
                .control_size(ControlSize::Xs)
                .on_click(move |window, cx| handler(id.clone(), window, cx)),
        );
    }

    let _ = window;
    // Every message is a surface of its own. Without one the byline, the body
    // and the delivery mark are three runs of text at the same left edge, and
    // which message a status line belongs to is a question about vertical
    // distance: a reader counting gaps had already been told the wrong thing.
    div()
        .w_full()
        .py_token(&theme, Space::Xs)
        .child(
            div()
                .column()
                .w_full()
                .gap_token(&theme, Space::Xs)
                .p_token(&theme, Space::Sm)
                .radius(&theme, Radius::Card)
                .surface(&theme, Surface::Raised)
                .child(header)
                .child(body)
                .child(footer),
        )
        .into_any_element()
}

/// How many lines of a text body the slot did not have room for, or `None`
/// when it had room for all of them.
fn lines_left_out(message: &Message, body_lines: Option<usize>) -> Option<usize> {
    // A Markdown body is clipped by rendered height rather than by lines, so
    // nothing here knows a number to report and it does not invent one.
    let MessageBody::Text(text) = &message.body else {
        return None;
    };
    let limit = body_lines?;
    let hidden = text.lines().count().saturating_sub(limit);
    (hidden > 0).then_some(hidden)
}

fn truncation_mark(ident: &Ident, hidden: usize, theme: &Theme, cx: &mut App) -> AnyElement {
    let digits = cx.numbers().count(hidden);
    let label = cx.strings().format_plural(
        StringKey::MessageShowMoreOne,
        StringKey::MessageShowMoreMany,
        cx.numbers().plural(hidden),
        &[digits.as_ref()],
    );
    // Lines the slot cut off are the one thing in this footer a reader may
    // want to act on, so the note is a chip like the others rather than the
    // faintest text on the row — a mark quieter than an attachment name was
    // announcing the missing content less loudly than the content present.
    div()
        .flex_none()
        .row()
        .items_center()
        .gap_token(theme, Space::Xs)
        .px_token(theme, Space::Xs)
        .radius(theme, Radius::Small)
        .surface(theme, Surface::Sunken)
        .hairline(theme)
        .type_scale(theme, TypeScale::Caption)
        .text_color(theme.colors.text_muted)
        .child(
            icon(Icon::AltArrowDown)
                .size(px(theme.typography.caption.line_height))
                .text_color(theme.colors.text_muted),
        )
        .child(label.clone())
        .semantic_in(
            cx,
            NodeSpec::new(ident.child("truncated").semantic_id(), Role::Status)
                .parent(ident.semantic_id())
                .text(label)
                .value(digits),
        )
        .into_any_element()
}

fn body_element(
    ident: &Ident,
    message: &Message,
    body_lines: Option<usize>,
    on_markdown: Option<&MarkdownHandler>,
    theme: &Theme,
    _cx: &mut App,
) -> AnyElement {
    let height = body_lines.map(|lines| theme.typography.body.line_height * lines as f32);
    match &message.body {
        MessageBody::Markdown(source) => {
            // A message the host says is still being written is read as one:
            // parsed from its tail, its half-finished markers held steady, and
            // its newest words fading in rather than snapping into place.
            let mut markdown =
                Markdown::new(ident.child("body"), source.clone()).streaming(message.streaming);
            if let Some(lines) = body_lines {
                markdown = markdown.max_lines(lines);
            }
            if let Some(handler) = on_markdown {
                let handler = Rc::clone(handler);
                let id = message.id.clone();
                markdown = markdown
                    .on_event(move |event, window, cx| handler(id.clone(), event, window, cx));
            }
            div()
                .w_full()
                .when_some(height, |element, height| {
                    element.h(px(height)).overflow_hidden()
                })
                .child(markdown)
                .into_any_element()
        }
        MessageBody::Text(text) => {
            let lines: Vec<&str> = text.lines().collect();
            let shown = match body_lines {
                Some(limit) => lines
                    .iter()
                    .take(limit)
                    .copied()
                    .collect::<Vec<_>>()
                    .join("\n"),
                None => text.to_string(),
            };
            div()
                .column()
                .w_full()
                .when_some(height, |element, height| {
                    element.h(px(height)).overflow_hidden()
                })
                .type_scale(theme, TypeScale::Body)
                .text_color(theme.colors.text)
                .child(SharedString::from(shown))
                .into_any_element()
        }
    }
}

fn streaming_mark(ident: &Ident, theme: &Theme, cx: &mut App) -> AnyElement {
    let label = cx.strings().text(StringKey::MessageStreaming);
    div()
        .row()
        .gap(px(4.0))
        .text_color(theme.colors.accent)
        // The mark already published `busy`; a still dot meant a reader
        // watching the screen had to take the word for it.
        .child(StatusDot::new(Tone::Accent).busy(ident.child("streaming.mark")))
        .child(label.clone())
        .semantic_in(
            cx,
            NodeSpec::new(ident.child("streaming").semantic_id(), Role::Status)
                .parent(ident.semantic_id())
                .text(label)
                .value("streaming")
                .busy(true),
        )
        .into_any_element()
}

/// The mark that says where a message got to.
///
/// A message still being written has been handed over exactly as a queued one
/// has, so the state is the same and the wording is not: two messages in
/// visibly different situations reading the same word told the reader less
/// than the host knew. The node keeps the state's own name, so an assertion
/// reads the delivery rather than the sentence drawn for it.
fn delivery_mark(
    ident: &Ident,
    state: &DeliveryState,
    writing: bool,
    theme: &Theme,
    cx: &mut App,
) -> AnyElement {
    let label = match (state, writing) {
        (DeliveryState::Sending, true) => cx.strings().text(StringKey::MessageWriting),
        _ => state.label(cx.strings()),
    };
    let tone = state.tone();
    div()
        .row()
        .gap(px(4.0))
        .min_w_0()
        .type_scale(theme, TypeScale::Caption)
        .text_color(tone.color(theme))
        .child(StatusDot::new(tone))
        .child(div().min_w_0().child(label.clone()))
        .semantic_in(
            cx,
            NodeSpec::new(ident.child("delivery").semantic_id(), Role::Status)
                .parent(ident.semantic_id())
                .text(label)
                .value(state.name())
                .invalid(state.failed())
                .busy(matches!(state, DeliveryState::Sending)),
        )
        .into_any_element()
}

fn attachment_chip(
    ident: &Ident,
    attachment: &Attachment,
    theme: &Theme,
    cx: &mut App,
) -> AnyElement {
    let text = match &attachment.detail {
        Some(detail) => SharedString::from(format!("{} — {detail}", attachment.name)),
        None => attachment.name.clone(),
    };
    div()
        .flex_none()
        .px_token(theme, Space::Xs)
        .radius(theme, Radius::Small)
        .surface(theme, Surface::Sunken)
        .hairline(theme)
        .type_scale(theme, TypeScale::Caption)
        .text_color(theme.colors.text_muted)
        .child(text.clone())
        .semantic_in(
            cx,
            NodeSpec::new(
                ident
                    .child("attachment")
                    .child(attachment.id.as_ref())
                    .semantic_id(),
                Role::Text,
            )
            .parent(ident.semantic_id())
            .text(text),
        )
        .into_any_element()
}

fn reaction_chip(ident: &Ident, reaction: &Reaction, theme: &Theme, cx: &mut App) -> AnyElement {
    let digits = cx.numbers().count(reaction.count);
    let text = cx
        .numbers()
        .labelled_count(reaction.label.as_ref(), reaction.count);
    div()
        .flex_none()
        .px_token(theme, Space::Xs)
        .radius(theme, Radius::Pill)
        .surface(theme, Surface::Sunken)
        .hairline(theme)
        .type_scale(theme, TypeScale::Caption)
        .text_color(theme.colors.text_muted)
        .child(text.clone())
        .semantic_in(
            cx,
            NodeSpec::new(
                ident
                    .child("reaction")
                    .child(reaction.key.as_ref())
                    .semantic_id(),
                Role::Status,
            )
            .parent(ident.semantic_id())
            .text(text)
            .value(digits),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_delivery_state_has_its_own_name() {
        let states = [
            DeliveryState::Sending,
            DeliveryState::Sent,
            DeliveryState::Delivered,
            DeliveryState::Read,
            DeliveryState::Failed {
                reason: "The host refused it.".into(),
            },
        ];
        let mut names: Vec<&str> = states.iter().map(DeliveryState::name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), states.len());
    }

    #[test]
    fn a_failure_states_the_hosts_reason_rather_than_a_word_of_its_own() {
        let state = DeliveryState::Failed {
            reason: "The workspace is read only.".into(),
        };
        // A host's own reason is passed through untouched, so it needs no
        // catalogue to read it back.
        // A host's own reason is passed through untouched, so an empty
        // catalogue is enough to read it back.
        assert_eq!(
            state.label(&Strings::new()).as_ref(),
            "The workspace is read only."
        );
        assert!(state.failed());
    }

    #[test]
    fn an_unnamed_author_is_named_unknown() {
        assert_eq!(shown_author(None).as_ref(), "unknown");
        assert_eq!(shown_author(Some(&"  ".into())).as_ref(), "unknown");
        assert_eq!(shown_author(Some(&"Ada".into())).as_ref(), "Ada");
    }
}
