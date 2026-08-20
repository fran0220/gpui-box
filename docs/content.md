# Content

`Markdown`, `AgentDocument`, and `MessageList` are the surfaces in this library
that draw text nobody in the application wrote. A button's label is authored;
a document's contents, a typed model result, and a message's body are not. They
arrive from a file, a model, or another person, and they may contain a tag, a
destination, or an image reference that would act on the reader if anything
here acted on it.

So nothing here acts. This file is the posture written down.

`AgentDocument` keeps Markdown in that prose role instead of making it the
protocol for every model result. Its stable, revisioned blocks carry existing
code, diff, schema, chart, image, tool-call, notice, choice, and artifact
components. A reconnect updates the caller-owned block id; duplicate ids are
reported and never silently merged. Markdown events include the block id that
produced them, so a host applies the same link and image policy it already uses
for a standalone document.

`PersonaDialogue` keeps the same boundary while adding an expressive speaker,
localized execution status, streaming treatment, and typed choices. Its
`PersonaDialogueEvent::Markdown` carries the turn id alongside the unchanged
`MarkdownEvent`; `markdown_image` and `markdown_highlighter` are the same
host-resolution seams as the standalone renderer. A disabled choice shows the
host's refusal and installs no callback. The component never opens a link,
fetches portrait or document art, samples a microphone, recognizes speech, or
advances the dialogue.

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

Code blocks and prose runs use GPUI's document selection. Pointer dragging can
cross separately mounted runs and blocks in reading order; reverse selection,
Copy, Select All, wrapped and bidirectional hit testing, and AccessKit
selection all operate on grapheme boundaries. Every code block still carries a
whole-value copy action that puts its exact bytes on the clipboard and reports
`MarkdownEvent::CodeCopied`; that operation also includes text outside a
mounted or visible range. Nothing is reflowed, retyped, or re-indented on the
way.

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

### One message, one slot — unless the caller buys measurement

The default `List` is uniform, so every row is the same height. That is what
makes a conversation of ten thousand messages cost what one of ten costs, and
the price is stated rather than hidden: `MessageList::body_lines(n)` decides
how tall a slot is, and a plain body longer than that reports how many lines it
left out beside the delivery state.

`MessageList::grows_to_fit()` chooses the other truthful trade. Rows remain
virtualized, but GPUI measures each one when it first approaches the viewport
and keeps that height, so every message is whole. The list can only estimate
the extent of rows it has not measured, and its scrollbar settles as the reader
moves through them. The last call to `body_lines` or `grows_to_fit` wins.

## Media

`ImageViewer` and `TransportBar` are the same posture from the other side. The
two surfaces above draw content nobody in the application wrote; these two
frame content this crate cannot produce at all. **Nothing is fetched and
nothing is played.**

### An image arrives from the host or not at all

There is no network here and no asset resolution, which is exactly why
`Markdown` names an image instead of drawing it. `ImageViewer` keeps that rule
over a whole frame: the picture comes back from `ImageViewer::image`, and a
host that answers `None` gets a frame naming the image and its source, plus one
`ImageViewerEvent::ImageRequested`. The request is made once per image for as
long as the viewer is on screen, so answering it does not provoke another.

Four states are four renderings, and each fails differently:

| State | Means | Drawn as |
|---|---|---|
| `Loading` | the host is still fetching | the name, `Loading`, published `busy` |
| `Unavailable(reason)` | the host could not supply it, and here is why | the name and the host's own sentence, in the warning tone |
| `Failed(reason)` | the bytes arrived and could not be read | the name and the host's own sentence, published `invalid` |
| `Ready` | the host has it | the picture, or a frame naming what was not supplied |

A refusal is never an empty frame. A reader shown a blank rectangle cannot tell
a refusal from an image of nothing, which is the same failure `Markdown`'s
placeholder exists to avoid.

### A size nobody stated is not the size of the box it was drawn in

Natural dimensions are a caller input. `Contain`, `Cover` and `Actual` are all
ratios between the source and the frame, and `Actual` in particular claims one
image pixel per frame pixel — a claim nobody can make about an image whose
pixel count the host never gave. So a viewer with no dimensions reads
`Size unknown`, refuses the fit and zoom controls, and hands the picture the
frame without saying how much of the source that shows. Reporting the rendered
size as though it were the source's would invent the one fact the host declined
to give.

The frame's own extent comes from the measurement taken during prepaint, the
same `layout::measure` cell the slider and the split divider read, because zoom
at a point and a clamped pan are both arithmetic against a box only layout
knows. A frame that has not been measured yet states no scale rather than
stating zero.

Zoom is anchored at the pointer by recording the *image* point that was under
it and the *frame* point it was under, both normalized. That pair survives a
zoom the caller has not applied yet: a host that refuses the new scale renders
at the old one, and the same arithmetic puts the picture back exactly where it
was. A pan is clamped so the picture cannot be dragged past its own edge, and
is offered at all only while the picture is larger than the frame.

Stepping never wraps. Past the last image the control is refused — it installs
no handler at all — and the position is published as `2 of 2`, because a reader
returned silently to the first image has been told the gallery is endless.

### A duration nobody knows is a state

This is `PageTotal::Unknown` for a timeline, and for the same reason: a page
count nobody counted is a number nobody can trust, and a live stream has a
position and no total. `TransportDuration::Unknown` therefore draws **no track
fraction at all** — no fill, no head, no buffered band — states
`Duration unknown` where a remaining time would be, and publishes the scrubber
as a status rather than a slider, because there is no range to slide along. A
half-filled bar over an unknown total would be reporting a number nobody has.

Scrubbing is refused there too. The forward end of a stream nobody measured is
the host's to clamp, so an arrow key still reports the position it was asked
for and the host decides whether that is past the live edge.

### Everything reports, and nothing is applied

`TransportEvent` is the whole vocabulary: `PlayRequested`, `PauseRequested`,
`SeekPreview`, `SeekRequested`, `VolumeRequested`, `MuteToggled`,
`SpeedRequested`, `Stepped`. The head is drawn where the caller says it is, so
a refused seek keeps the position that still holds — the rule every value in
this library follows.

A scrub reports `SeekPreview` on every move and `SeekRequested` once, on
release. That split is what lets a host show a preview frame without seeking
once per pixel. It is the press-move-release gesture the slider and the split
divider already use, with the track's measured bounds turning a pointer
position into seconds; the drag is followed on the whole bar rather than on the
few pixels of the track, so letting go outside it still commits once.

Buffered ranges are drawn as their own band, never folded into the played fill:
a reader who cannot tell them apart has been told the media is further along
than it is. A host that supplies no ranges gets no band and no node, so "no
buffer reported" stays distinguishable from "buffered to the start".

### Waiting is not paused

`TransportState::Buffering` is playing and stalled. It says `Waiting for data`,
publishes `busy`, and still offers the control that would *stop* playback,
because nothing has stopped. A stalled transport drawn as paused sends the
reader to the control that would resume something that never stopped.

### Times come from the host

The crate formats no durations. `elapsed` and `remaining` are finished strings,
exactly as `Timeline` and `MessageList` take theirs, because turning seconds
into words is locale work and whoever owns the clock owns the wording. A
transport given no strings says `Time unknown` rather than counting for itself.
The numeric position is a separate input, and it is the one that drives the
track geometry — the picture and the words never come from the same place, so
neither can quietly stand in for the other.
