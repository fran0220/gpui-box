# Content

`Markdown` and `MessageList` are the two surfaces in this library that draw
text nobody in the application wrote. A button's label is authored; a
document's contents, and a message's body, are not. They arrive from a file, a
model, or another person, and they may contain a tag, a destination, or an
image reference that would act on the reader if anything here acted on it.

So nothing here acts. This file is the posture written down.

## The Markdown security posture

### Raw HTML is shown, never run, and never dropped

A fragment of HTML — `<div onclick="…">`, `<script>`, `<b>` — reaches the tree
as `Block::Html` or `Inline::Html` and is drawn as the literal characters that
were written, in the mono face, in a frame that says `unrendered html`. The
node it publishes carries the same words in `value`.

Both halves of that matter, and they fail differently:

- **Interpreting it** would let a document reach outside its own text: into the
  layout around it, into the pointer, into whatever the host's element tree can
  be persuaded to do. This crate has no HTML renderer and will not acquire one;
  `pulldown-cmark` is compiled with its `html` feature off.
- **Dropping it** would let a document delete its own content from the reader's
  view by wrapping it in a tag. A reader who is shown a tag knows something was
  written that this component would not draw. A reader who is shown nothing
  believes nothing was written. Silence is the worse failure of the two, so
  the tag is visible and marked.

### A link states where it goes, and this crate opens nothing

Before a link is taken, its destination is in two places the reader can reach:
hover help, through the same `Tooltipped` trait every other control uses, and
the `Link` node's `value`. A title the author wrote is shown *beside* the
destination, never instead of it, because a friendly label over a hostile
destination is exactly the shape of the attack.

Taking it reports `MarkdownEvent::LinkClicked { href }` and does nothing else.
Whether an `https` link may be opened, whether a `file` link may be, and what
happens to a scheme nobody expected are all host policy over host capability,
and this crate has neither.

### An image is named, not fetched

This crate has no network and no asset resolution, so an image reference is
drawn as a placeholder naming its alt text *and* its source, and reported as
`MarkdownEvent::ImageRequested { src, alt }`. A grey rectangle would hide which
image is missing; the placeholder says.

The request is made once per source per rendered document, not once per frame,
so a host that answers by supplying the image does not provoke another request
by answering. A host that has the bytes returns an element from
`Markdown::image`; a host that returns `None` has said it cannot, and the
placeholder stays.

### Code is never coloured by guessing

A fenced block publishes its info string exactly as it was written — `rust`,
`rust,no_run`, whatever it says — and a block with no info string publishes
`plain text` rather than a language somebody inferred. Colour comes from
`Markdown::highlight`, which hands the host the block and takes back byte
ranges tagged with the library's existing `Tone` vocabulary. There are no
syntax categories in this crate, because deciding a word is a keyword is a
grammar's judgement and this crate has no grammar.

Selecting text is not something GPUI offers, so "selectable" is delivered as
the capability underneath it: every block carries a copy action that puts its
exact bytes on the clipboard and reports `MarkdownEvent::CodeCopied`. Nothing
is reflowed, retyped, or re-indented on the way.

### Truncation says how much it cut

`Markdown::max_lines(n)` keeps the first `n` lines and offers the rest by name:
`Show 12 more lines`, which reports `MarkdownEvent::MoreRequested`. A fade over
the last line says something was cut without saying how much, which is a
decoration standing where a fact belongs.

A line here is a line of the document's own structure — a heading, a paragraph,
one line of a code fence, one list entry, one table row — not a line of wrapped
text, whose count only layout knows. A block that cannot be cut without lying,
such as a table split from its header, is left out whole rather than split.

### What the renderer gives up

A line of prose is drawn as a wrapping row of separately addressable runs, so
a link and an image each have their own bounds for a pointer and for a test to
find. GPUI has no way to hang a probe on a byte range inside a shaped line, and
an unaddressable link is one no test can prove reports its destination. The
cost is that a line breaks between runs rather than inside a run that spans two
styles.

## The message list's delivery vocabulary

`DeliveryState` has five members and five renderings:

| State | Means | Drawn as |
|---|---|---|
| `Sending` | handed over, not yet acknowledged | neutral, published `busy` |
| `Sent` | the host accepted it | info |
| `Delivered` | it reached the other side | accent |
| `Read` | the other side opened it | success |
| `Failed { reason }` | it did not arrive, and here is why | danger, published `invalid` |

Collapsing sent, delivered, and read into one tick tells the reader less than
the host knows. Folding a failure into any of them tells them something untrue.

**A failure is never removed and never retried by itself.** A failed message
keeps its position, its author, its time, and its full text, states the host's
reason word for word rather than a friendlier sentence, and gains exactly one
thing: a `Try again` control that reports the message id. Whether to resend,
and what a resend even means, belongs to the transport this crate does not
have.

### Streaming

A message may be `streaming`, which draws a live mark beside its byline and
publishes a `streaming` node. The mark is keyed to the message's identity and
nothing else, so text arriving into a stream does not restart it:
`content::message_list::streaming_since` answers with the instant a stream
began, and it is the same instant after the body grows. A "still writing" mark
that flickered on every token would be reporting the token, not the stream.

### Grouping is declared, not inferred

`MessageList::group_consecutive(bool)` decides whether consecutive messages
from one author are drawn as one turn, for the same reason
`Toolbar::overflow_after` is declared: whether two messages a minute apart are
one turn or two is a judgement about the conversation, and this component knows
nothing about the conversation. A continued turn drops the repeated byline and
keeps the space, because the slot is one height and a hole where a name was is
not a saving.

### Following the newest message is conditional, and says when it does not

The list follows a new message **only while the reader is already at the
bottom**. Dragging somebody back down while they are reading something further
up takes the conversation away from them.

When it does not follow, it says so, with the count: `3 new messages` for
messages that arrived while the reader was away, `3 more messages` for messages
that have simply always been further down. This is `ScrollArea`'s rule — content
continuing past the view is published rather than left to be noticed —
specialized for a surface that grows from the bottom.

### Times and authors nobody recorded

A message's time is an `EntryTime`, the same type `Timeline` takes and for the
same reason: turning an instant into words is calendar, time-zone and locale
work, and whoever owns the clock owns the wording. A message whose time nobody
knows says `Time unknown` and publishes `time unknown`; it is not floated to an
end that would claim when it happened. An author nobody named is `unknown`,
not blank, because a blank byline reads as a message with no author rather than
one whose author was not recorded.

### One message, one slot

Virtualization is `List`, which is `uniform_list`, so every row is the same
height. That is what makes a conversation of ten thousand messages cost what
one of ten costs, and the price is stated rather than hidden:
`MessageList::body_lines(n)` decides how tall a slot is, and a body longer than
that says how many lines it left out — through `Markdown::max_lines` for a
Markdown body, and through the same wording for a plain one. There is no second
virtualization in this library and there will not be one.
