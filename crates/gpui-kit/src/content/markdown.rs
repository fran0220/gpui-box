//! Read-only Markdown, rendered to GPUI elements.
//!
//! # What this component will not do
//!
//! **It does not execute HTML, and it does not swallow it.** A fragment of raw
//! HTML is rendered as the literal characters that were written, marked as
//! unrendered. Interpreting it would let a document reach outside the reader's
//! text; dropping it would let a document delete its own content from view by
//! wrapping it in a tag, which is worse than showing the tag.
//!
//! **It does not open links.** A link shows its destination before it is
//! taken — in the node it publishes and in hover help — and reports
//! [`MarkdownEvent::LinkClicked`]. Whether that destination may be opened, and
//! by what, is the host's to decide.
//!
//! **It does not fetch images.** This crate has no network and no asset
//! resolution. An image is drawn as a placeholder naming its alt text and its
//! source, and reported once as [`MarkdownEvent::ImageRequested`]; a host that
//! has the bytes hands an element back through [`Markdown::image`].
//!
//! **It does not guess at syntax.** A code block is set in the mono face and
//! publishes the fence's info string exactly as it was written. Colour comes
//! from host-supplied [`CodeSpan`]s or from nowhere.

pub mod doc;
mod mend;
pub mod parse;
mod stream;
mod veil;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, ClipboardItem, FontWeight, HighlightStyle, Hsla, InteractiveElement,
    IntoElement, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    StyledText, Window, div, prelude::FluentBuilder, px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{
    ActiveTheme, Elevation, Radius, SemanticColor, SemanticWash, Space, Surface, SyntaxColor,
    Theme, TypeScale,
};
use web_time::Instant;

use crate::content::code_view::styled_code;
use crate::content::highlight::{Cache, Language};
use crate::controls::button::Button;
use crate::foundation::{FocusRing, Ident, Sizable, StyledExt};
use crate::motion::{MotionPolicy, MotionRole, keyed};
use crate::overlay::Tooltipped;
use crate::strings::{ActiveNumbers, ActiveStrings, StringKey};
use stream::Stream;
use veil::Veil;

pub use parse::{Block, CellAlign, Document, Inline, ListEntry};

/// What a rendered document reports. It applies none of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownEvent {
    /// A link was taken. The crate opens nothing.
    LinkClicked { href: SharedString },
    /// An image the host has not supplied was drawn as a placeholder.
    ///
    /// Reported once per source per rendered document, not once per frame, so
    /// a host may answer it by supplying the image without the answer
    /// provoking another request.
    ImageRequested {
        src: SharedString,
        alt: SharedString,
    },
    /// A code block's exact text was put on the clipboard.
    CodeCopied {
        language: Option<SharedString>,
        text: SharedString,
    },
    /// The reader asked for the lines [`Markdown::max_lines`] left out.
    MoreRequested { lines: usize },
}

/// An image the document referred to and this crate did not fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRequest {
    pub src: SharedString,
    pub alt: SharedString,
    pub title: Option<SharedString>,
}

/// A fenced block, as handed to a host that colours code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    /// The fence's info string, exactly as written. Nothing here parses it.
    pub language: Option<SharedString>,
    pub text: SharedString,
}

/// One coloured run of a code block, in byte offsets into its text.
///
/// The role is a syntax class, not a general tone, so the same span means the
/// same thing whether [`crate::content::highlight`] found it or a host with a
/// real grammar did, and both land on the theme's syntax colours rather than
/// on whatever the caller thought a keyword should look like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSpan {
    pub range: Range<usize>,
    pub role: SyntaxColor,
}

/// How fenced code blocks are presented inside a Markdown document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarkdownCodePresentation {
    /// A raised card that separates code from the surrounding document.
    Card,
    /// An inline block without a raised surface, elevation, or card frame.
    #[default]
    Flat,
}

type EventHandler = Rc<dyn Fn(&MarkdownEvent, &mut Window, &mut App)>;
type ImageSource = Rc<dyn Fn(&ImageRequest, &mut Window, &mut App) -> Option<AnyElement>>;
type Highlighter = Rc<dyn Fn(&CodeBlock) -> Vec<CodeSpan>>;

/// A rendered Markdown document.
#[derive(IntoElement)]
pub struct Markdown {
    ident: Ident,
    source: SharedString,
    max_lines: Option<usize>,
    /// The first reading-order value this document may claim when it is
    /// embedded in a larger document.
    selection_order_start: u64,
    on_event: Option<EventHandler>,
    image: Option<ImageSource>,
    highlighter: Option<Highlighter>,
    code_presentation: MarkdownCodePresentation,
    streaming: bool,
}

impl std::fmt::Debug for Markdown {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Markdown")
            .field("ident", &self.ident)
            .field("bytes", &self.source.len())
            .field("max_lines", &self.max_lines)
            .field("has_images", &self.image.is_some())
            .field("has_highlighter", &self.highlighter.is_some())
            .field("code_presentation", &self.code_presentation)
            .field("streaming", &self.streaming)
            .finish()
    }
}

impl Markdown {
    pub fn new(ident: impl Into<Ident>, source: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            source: source.into(),
            max_lines: None,
            selection_order_start: 0,
            on_event: None,
            image: None,
            highlighter: None,
            code_presentation: MarkdownCodePresentation::Flat,
            streaming: false,
        }
    }

    /// Shows at most `lines` of the document's own structure, and says how
    /// many were left out.
    ///
    /// A line is a heading, a paragraph, one line of a code fence, one list
    /// entry, or one table row — not a line of wrapped text, whose count only
    /// layout knows.
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.max_lines = Some(lines);
        self
    }

    /// Places this document inside a caller-owned reading-order partition.
    ///
    /// A window's document selection joins what it selected in reading order,
    /// and a document that draws its own runs from zero claims the same order
    /// as every other one. That is invisible while a Markdown is a whole
    /// surface, and wrong the moment a caller draws several of them — one per
    /// message, or one per block of a long answer split across rows — because
    /// a drag over three of them would then read them back interleaved.
    ///
    /// Give each document a start far enough apart that its own runs cannot
    /// reach the next one's: the count is emissions, so a stride per document
    /// bounds how many selectable runs one document may contain.
    pub fn selection_order_start(mut self, order: u64) -> Self {
        self.selection_order_start = order;
        self
    }

    pub fn on_event(
        mut self,
        handler: impl Fn(&MarkdownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }

    /// Supplies the element to draw for an image the host already holds.
    ///
    /// Answering `None` leaves the placeholder, which names the source, so a
    /// host that cannot supply one has said so rather than left a gap.
    pub fn image(
        mut self,
        source: impl Fn(&ImageRequest, &mut Window, &mut App) -> Option<AnyElement> + 'static,
    ) -> Self {
        self.image = Some(Rc::new(source));
        self
    }

    /// Reads this document as one that is still being written.
    ///
    /// Three things change. The source is reparsed from the last block that
    /// could still be affected rather than from the beginning, so a long reply
    /// does not get slower as it gets longer. Inline markers that have opened
    /// and not yet closed are read as though they had, so a paragraph does not
    /// twitch every time a `**` finishes. And text that has just arrived fades
    /// in over a fraction of a second, so a fast stream reads as a soft
    /// leading edge instead of a stutter.
    ///
    /// None of it delays anything: every character is laid out the frame it
    /// arrives, at the position it will keep.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// Chooses how fenced code blocks are presented inside this document.
    ///
    /// The default is [`MarkdownCodePresentation::Flat`]. Card presentation
    /// changes only the fence's container treatment; its header, Copy action,
    /// highlighting, selection, line count, and overflow behavior are the
    /// same as the default presentation.
    pub fn code_presentation(mut self, presentation: MarkdownCodePresentation) -> Self {
        self.code_presentation = presentation;
        self
    }

    /// Colours code blocks from spans the host computed.
    ///
    /// A fence whose info string names a language
    /// [`crate::content::highlight`] knows is coloured without this. Install a
    /// highlighter to override that with a real grammar's judgement, or to
    /// reach a language the built-in scanner has no table for.
    pub fn highlight(
        mut self,
        highlighter: impl Fn(&CodeBlock) -> Vec<CodeSpan> + 'static,
    ) -> Self {
        self.highlighter = Some(Rc::new(highlighter));
        self
    }
}

impl RenderOnce for Markdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ident = self.ident.clone();
        let now = cx.background_executor().now();

        // Parsing goes through the incremental reader whether the document is
        // streaming or not: a source that did not change costs nothing to read
        // again, which is every settled document on screen.
        let reader = keyed::slot::<Stream>(
            &ident.child("source").semantic_id(),
            window.window_handle().window_id(),
            cx,
        );
        let parsed = {
            let mut reader = reader.borrow_mut();
            reader.read(self.source.as_ref());
            if self.streaming {
                reader.mended().unwrap_or_else(|| reader.document().clone())
            } else {
                reader.document().clone()
            }
        };
        let (document, hidden) = match self.max_lines {
            Some(max) => parsed.truncate(max),
            None => (parsed, 0),
        };

        let veil = self.streaming.then(|| {
            keyed::slot::<Veil>(
                &ident.child("arriving").semantic_id(),
                window.window_handle().window_id(),
                cx,
            )
        });

        let mut painter = Painter {
            ident: ident.clone(),
            theme: theme.clone(),
            on_event: self.on_event.clone(),
            image: self.image.clone(),
            highlighter: self.highlighter.clone(),
            code_presentation: self.code_presentation,
            used: HashMap::new(),
            reading_order: self.selection_order_start,
            requested: Vec::new(),
            veil: veil.clone(),
            now,
            drawn: Vec::new(),
        };

        let mut column = div().column().w_full().gap_token(&theme, Space::Md);
        for block in &document.blocks {
            column = column.child(painter.block(block, window, cx));
        }

        if hidden > 0 {
            column = column.child(painter.more(hidden, cx));
        }

        let requests = std::mem::take(&mut painter.requested);
        report_images(&ident, requests, self.on_event.as_ref(), window, cx);

        // The fade learns from the frame that has just been built, and is read
        // by the next one. A frame of lag is a sixtieth of a second against a
        // fade of a fifth, and it is what lets the record be of what was drawn
        // rather than of what a second walk of the tree guessed was drawn.
        if let Some(veil) = veil {
            let mut veil = veil.borrow_mut();
            veil.observe(
                std::mem::take(&mut painter.drawn),
                now,
                &theme,
                MotionPolicy::resolve(MotionRole::Streaming, cx),
            );
            if veil.is_fading(now) {
                window.request_animation_frame();
            }
        }

        let blocks = document.blocks.len();
        column.semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Region).value(cx.strings().format_plural(
                StringKey::MarkdownBlockOne,
                StringKey::MarkdownBlocks,
                cx.numbers().plural(blocks),
                &[cx.numbers().count(blocks).as_ref()],
            )),
        )
    }
}

/// The image sources already reported for one document.
#[derive(Debug, Default)]
struct Requested(HashSet<SharedString>);

/// Reports each unfetched image once, keyed by the document's identity.
///
/// The report happens while the document renders, because that is when the
/// crate learns an image was asked for. Reporting it again every frame would
/// hand the host a request it has already answered, so the sources already
/// reported are remembered for as long as the document is on screen.
fn report_images(
    ident: &Ident,
    requests: Vec<ImageRequest>,
    handler: Option<&EventHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    if requests.is_empty() {
        return;
    }
    let Some(handler) = handler.cloned() else {
        return;
    };
    let cell = keyed::slot::<Requested>(
        &ident.child("images").semantic_id(),
        window.window_handle().window_id(),
        cx,
    );
    let fresh: Vec<ImageRequest> = {
        let mut seen = cell.borrow_mut();
        requests
            .into_iter()
            .filter(|request| seen.0.insert(request.src.clone()))
            .collect()
    };
    for request in fresh {
        handler(
            &MarkdownEvent::ImageRequested {
                src: request.src,
                alt: request.alt,
            },
            window,
            cx,
        );
    }
}

/// How a run of prose is set.
#[derive(Debug, Clone, Copy, Default)]
struct RunStyle {
    strong: bool,
    emphasis: bool,
    struck: bool,
    /// The colour this run inherits, when something above it set one.
    ///
    /// Only a fading run needs to know. An opacity is applied by naming a
    /// colour, so a run arriving inside a link has to name the link's colour
    /// rather than the page's, or it would fade in the wrong colour and then
    /// correct itself.
    color: Option<Hsla>,
}

/// Walks the tree once, drawing it and minting one id per addressable part.
struct Painter {
    ident: Ident,
    theme: Theme,
    on_event: Option<EventHandler>,
    image: Option<ImageSource>,
    highlighter: Option<Highlighter>,
    code_presentation: MarkdownCodePresentation,
    /// How many parts have already claimed each id stem, so a document that
    /// links to the same place twice still publishes two distinct nodes.
    used: HashMap<String, usize>,
    /// How many selectable runs have been emitted, which is the reading order
    /// the document selection joins them in. It counts emissions rather than
    /// positions in the source, so a run's order matches the order a reader
    /// meets it.
    reading_order: u64,
    requested: Vec<ImageRequest>,
    /// The fade over what has most recently arrived, while this document is
    /// still arriving.
    veil: Option<Rc<RefCell<Veil>>>,
    /// One clock for the whole frame, so every fading run is at the same point
    /// in its fade rather than at the point it was reached.
    now: Instant,
    /// The text of every run this frame drew, in the order it drew them.
    ///
    /// Recorded rather than derived, so what the fade believes is on screen is
    /// exactly what was put there.
    drawn: Vec<String>,
}

impl Painter {
    /// Records that a run of text was drawn, and answers how much of it is
    /// still arriving.
    ///
    /// Every run passes through here whether the document is streaming or not,
    /// because the record is what the next frame compares against. A settled
    /// document simply always gets an empty answer.
    fn arriving(&mut self, text: &str) -> Vec<(Range<usize>, f32)> {
        let index = self.drawn.len();
        self.drawn.push(text.to_string());
        let Some(veil) = &self.veil else {
            return Vec::new();
        };
        // A span is recorded at the end of one frame and read by the next, so
        // it describes the text as that frame drew it. A document that
        // reflowed in between — a link resolved, a code span closed, a block
        // split — leaves run `index` holding a different string, and the
        // recorded byte range then belongs to nothing on screen.
        //
        // Applying it anyway aborts the process: `StyledText::with_highlights`
        // asserts that both ends land on a character boundary, and an offset
        // taken from one string lands mid-character in another as soon as the
        // text is not ASCII. A Chinese transcript hits it almost immediately,
        // where an English one runs for a long time before an end offset
        // happens to overrun.
        //
        // The veil is a fade. A fade whose range no longer describes the text
        // is not drawn, because the alternative is a renderer that stops the
        // application to insist on decorating it.
        veil.borrow()
            .spans(index, self.now)
            .into_iter()
            .filter(|(range, _)| fits(text, range))
            .collect()
    }

    /// An identity derived from what a part *is*, never from where it sits.
    ///
    /// Two links to the same destination are the same thing said twice, so the
    /// repeat is numbered rather than the position.
    fn ident_for(&mut self, kind: &str, name: &str) -> Ident {
        let stem = format!("{kind}-{}", slug(name));
        let count = self.used.entry(stem.clone()).or_insert(0);
        *count += 1;
        match *count {
            1 => self.ident.child(stem),
            repeat => self.ident.child(format!("{stem}-{repeat}")),
        }
    }

    /// The next reading order, consumed by one selectable run.
    fn next_reading_order(&mut self) -> u64 {
        let order = self.reading_order;
        self.reading_order += 1;
        order
    }

    fn report(&self, event: MarkdownEvent) -> Option<impl Fn(&mut Window, &mut App) + use<>> {
        let handler = self.on_event.clone()?;
        Some(move |window: &mut Window, cx: &mut App| handler(&event, window, cx))
    }

    fn block(&mut self, block: &Block, window: &mut Window, cx: &mut App) -> AnyElement {
        match block {
            Block::Heading { level, content } => self.heading(*level, content, window, cx),
            Block::Paragraph(inlines) => self.paragraph(inlines, window, cx),
            Block::Code { language, text } => self.code(language.clone(), text.clone(), window, cx),
            Block::Quote(blocks) => self.quote(blocks, window, cx),
            Block::List {
                ordered,
                start,
                entries,
            } => self.list(*ordered, *start, entries, window, cx),
            Block::Rule => self.rule(cx),
            Block::Table {
                alignment,
                head,
                rows,
            } => self.table(alignment, head, rows, window, cx),
            Block::Html(text) => self.html_block(text.clone(), cx),
        }
    }

    fn heading(
        &mut self,
        level: u8,
        content: &[Inline],
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let text = flatten(content);
        let ident = self.ident_for("heading", &text);
        let theme = self.theme.clone();
        // Only the first two steps get their own size; deeper headings are
        // distinguished by weight, because the type scale has five steps and a
        // document may have six levels.
        let scale = match level {
            1 => TypeScale::Title,
            2 => TypeScale::Body,
            _ => TypeScale::Label,
        };
        let runs = self.inline_row(
            content,
            RunStyle {
                strong: true,
                ..RunStyle::default()
            },
            window,
            cx,
        );

        div()
            .w_full()
            .type_scale(&theme, scale)
            .text_color(theme.colors.text)
            .child(runs)
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Heading)
                    .parent(self.ident.semantic_id())
                    .text(text)
                    // The outline is what assistive technology reads a
                    // document's shape from, so the level is published rather
                    // than left to be inferred from the type size.
                    .level(u32::from(level)),
            )
            .into_any_element()
    }

    fn paragraph(&mut self, inlines: &[Inline], window: &mut Window, cx: &mut App) -> AnyElement {
        let theme = self.theme.clone();
        div()
            .w_full()
            .type_scale(&theme, TypeScale::Body)
            .text_color(theme.colors.text)
            .child(self.inline_row(inlines, RunStyle::default(), window, cx))
            .into_any_element()
    }

    /// One line of prose, as a wrapping row of separately addressable runs.
    ///
    /// A link and an image each need their own bounds so a test and a pointer
    /// can find them, and GPUI has no way to hang a probe on a byte range
    /// inside a shaped line. The cost is stated in `docs/content.md`: a line
    /// breaks between runs rather than inside one that spans two styles.
    fn inline_row(
        &mut self,
        inlines: &[Inline],
        style: RunStyle,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let mut row = div().flex().flex_row().flex_wrap().w_full();
        for element in self.inlines(inlines, style, window, cx) {
            row = row.child(element);
        }
        row.into_any_element()
    }

    fn inlines(
        &mut self,
        inlines: &[Inline],
        style: RunStyle,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        let theme = self.theme.clone();
        let mut elements = Vec::new();
        for inline in inlines {
            match inline {
                Inline::Text(text) => {
                    let ident = self.ident_for("text", text.as_ref());
                    let order = self.next_reading_order();
                    let fading = self.arriving(text.as_ref());
                    elements.push(run(&theme, &ident, order, text.clone(), style, &fading));
                }
                Inline::Code(text) => {
                    let ident = self.ident_for("code", text.as_ref());
                    let order = self.next_reading_order();
                    let fading = self.arriving(text.as_ref());
                    elements.push(
                        div()
                            .px(px(theme.space(Space::Xs)))
                            .radius(&theme, Radius::Small)
                            .bg(theme.colors.raised)
                            .mono(&theme)
                            .text_size(px(theme.typography.code.size))
                            .child(
                                StyledText::new(text.clone())
                                    .with_highlights(fade(&fading, theme.colors.text))
                                    .selectable_in_document(
                                        ident.element_id(),
                                        ident.semantic_id(),
                                        order,
                                    ),
                            )
                            .into_any_element(),
                    );
                }
                Inline::Emphasis(inner) => elements.extend(self.inlines(
                    inner,
                    RunStyle {
                        emphasis: true,
                        ..style
                    },
                    window,
                    cx,
                )),
                Inline::Strong(inner) => elements.extend(self.inlines(
                    inner,
                    RunStyle {
                        strong: true,
                        ..style
                    },
                    window,
                    cx,
                )),
                Inline::Struck(inner) => elements.extend(self.inlines(
                    inner,
                    RunStyle {
                        struck: true,
                        ..style
                    },
                    window,
                    cx,
                )),
                Inline::Link {
                    href,
                    title,
                    content,
                } => elements.push(self.link(href, title.as_ref(), content, style, window, cx)),
                Inline::Image { src, alt, title } => {
                    elements.push(self.image(src, alt, title.as_ref(), window, cx))
                }
                Inline::Html(html) => elements.push(self.html_inline(html.clone(), cx)),
                Inline::SoftBreak => {
                    let ident = self.ident_for("text", "soft break");
                    let order = self.next_reading_order();
                    let fading = self.arriving(" ");
                    elements.push(run(&theme, &ident, order, " ".into(), style, &fading));
                }
                Inline::HardBreak => {
                    elements.push(div().w_full().h(px(0.0)).flex_none().into_any_element())
                }
            }
        }
        elements
    }

    fn link(
        &mut self,
        href: &SharedString,
        title: Option<&SharedString>,
        content: &[Inline],
        style: RunStyle,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let theme = self.theme.clone();
        let label = {
            let text = flatten(content);
            if text.trim().is_empty() {
                href.clone()
            } else {
                text
            }
        };
        // A link whose address has not finished arriving is styled as a link
        // and is nothing else: it says no destination, because none has been
        // said, and it cannot be taken to one nobody named. It is named after
        // its words, so the placeholder never reaches a reader or a report.
        let unfinished = href.as_ref() == mend::PENDING_LINK;
        let ident = self.ident_for(
            "link",
            if unfinished {
                label.as_ref()
            } else {
                href.as_ref()
            },
        );
        // Hover help states the destination before the reader commits, and the
        // title the author wrote is shown beside it rather than instead of it.
        let help = match (unfinished, title) {
            (true, Some(title)) => title.clone(),
            (true, None) => SharedString::default(),
            (false, Some(title)) => SharedString::from(format!("{title} — {href}")),
            (false, None) => href.clone(),
        };
        // Everything inside a link takes the link's colour, which a run that
        // is still arriving has to name in order to fade in it.
        let style = RunStyle {
            color: Some(theme.colors.accent),
            ..style
        };
        let runs = self.inlines(content, style, window, cx);
        // A link whose content produced no runs still reads at one place in
        // the document, so it claims an order of its own.
        let label_order = self.next_reading_order();
        let taken = (!unfinished)
            .then(|| self.report(MarkdownEvent::LinkClicked { href: href.clone() }))
            .flatten();

        div()
            .id(ident.element_id())
            .flex()
            .flex_row()
            .flex_wrap()
            // A link that can be followed can be reached. It publishes
            // `Role::Link` whether or not anybody can get to it, so without a
            // tab stop the document told a reader there were links in it and
            // then handed them to the pointer alone. The underline stays: on a
            // link it is not decoration but the second channel, so colour is
            // not the only thing separating a link from the prose around it.
            .when(!unfinished, |element| {
                element.cursor_pointer().tab_index(0).focus_ring(&theme)
            })
            .text_color(theme.colors.accent)
            .underline()
            .when(!help.is_empty(), |element| {
                element.tip(ident.clone(), help.clone())
            })
            .children(if runs.is_empty() {
                let fading = self.arriving(label.as_ref());
                vec![run(
                    &theme,
                    &ident.child("text"),
                    label_order,
                    label.clone(),
                    style,
                    &fading,
                )]
            } else {
                runs
            })
            .when_some(taken, |element, taken| {
                element.on_click(move |_, window, cx| taken(window, cx))
            })
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Link)
                    .parent(self.ident.semantic_id())
                    .text(label)
                    // The destination is the fact a reader would otherwise
                    // have to take on trust, so it is published, not hinted.
                    .value(href.clone()),
            )
            .into_any_element()
    }

    fn image(
        &mut self,
        src: &SharedString,
        alt: &SharedString,
        title: Option<&SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let theme = self.theme.clone();
        let ident = self.ident_for("image", src.as_ref());
        let request = ImageRequest {
            src: src.clone(),
            alt: alt.clone(),
            title: title.cloned(),
        };
        let supplied = self
            .image
            .as_ref()
            .and_then(|source| source(&request, window, cx));
        let named = if alt.trim().is_empty() {
            cx.strings().text(StringKey::MarkdownImageAlt)
        } else {
            alt.clone()
        };

        if supplied.is_none() {
            self.requested.push(request);
        }

        let spec = NodeSpec::new(ident.semantic_id(), Role::Image)
            .parent(self.ident.semantic_id())
            .text(named.clone())
            .value(if supplied.is_some() {
                SharedString::new_static("supplied")
            } else {
                SharedString::new_static("not fetched")
            });

        match supplied {
            Some(element) => div()
                .child(element)
                .semantic_in(cx, spec)
                .into_any_element(),
            // The placeholder names both the alt text and the source, because
            // an image nobody fetched is a fact about the document, and a grey
            // rectangle would hide which image is missing.
            None => div()
                .column()
                .gap(px(theme.space(Space::Xxs)))
                .px_token(&theme, Space::Sm)
                .py_token(&theme, Space::Xs)
                .radius(&theme, Radius::Small)
                .frame(&theme, Surface::Raised, Elevation::Raised)
                .child(
                    div()
                        .type_scale(&theme, TypeScale::Label)
                        .text_color(theme.colors.text)
                        .child(named),
                )
                .child(
                    div()
                        .type_scale(&theme, TypeScale::Caption)
                        .text_color(theme.colors.text_faint)
                        .child(
                            cx.strings()
                                .format(StringKey::MarkdownImageNotFetched, &[src.as_ref()]),
                        ),
                )
                .semantic_in(cx, spec)
                .into_any_element(),
        }
    }

    fn code(
        &mut self,
        language: Option<SharedString>,
        text: SharedString,
        window: &Window,
        cx: &mut App,
    ) -> AnyElement {
        let theme = self.theme.clone();
        // The id seed stays English on purpose: a semantic id that moved when
        // the host installed a translation would not be a stable id.
        let ident = self.ident_for("code", language.as_deref().unwrap_or(PLAIN_TEXT_ID));
        let code_order = self.next_reading_order();
        let label = language
            .clone()
            .unwrap_or_else(|| cx.strings().text(StringKey::MarkdownPlainText));
        // A host highlighter wins where there is one: it has a grammar and
        // this crate has a scanner. Where there is not, a fence that named a
        // language kit can read is coloured rather than left plain, and one
        // that named anything else stays exactly as it was written.
        let spans = match self.highlighter.as_ref() {
            Some(highlight) => Rc::new(highlight(&CodeBlock {
                language: language.clone(),
                text: text.clone(),
            })),
            None => match language.as_deref().and_then(Language::named) {
                Some(known) => keyed::slot::<Cache>(
                    &ident.child("colour").semantic_id(),
                    window.window_handle().window_id(),
                    cx,
                )
                .borrow_mut()
                .block(known, &text),
                None => Rc::new(Vec::new()),
            },
        };
        let lines = text.lines().count().max(1);

        let copied = self.report(MarkdownEvent::CodeCopied {
            language: language.clone(),
            text: text.clone(),
        });
        let copy_ident = ident.child("copy");
        let clipboard = text.clone();

        let body = div()
            .column()
            .w_full()
            .min_h(px(theme.typography.code.line_height))
            .mono(&theme)
            .text_size(px(theme.typography.code.size))
            .line_height(px(theme.typography.code.line_height))
            .text_color(theme.colors.text)
            .child({
                let text_ident = ident.child("text");
                styled_code(&theme, text.clone(), &spans).selectable_in_document(
                    text_ident.element_id(),
                    text_ident.semantic_id(),
                    code_order,
                )
            });

        let block = div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .child(
                div()
                    .row()
                    .w_full()
                    .justify_between()
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.text_faint)
                    .child(label.clone())
                    .child(
                        Button::new(copy_ident)
                            .label(cx.strings().text(StringKey::Copy))
                            .ghost()
                            .control_size(gpui_kit_theme::ControlSize::Xs)
                            .on_click(move |window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    clipboard.to_string(),
                                ));
                                if let Some(copied) = &copied {
                                    copied(window, cx);
                                }
                            }),
                    ),
            )
            .child(body);

        block
            .when(
                self.code_presentation == MarkdownCodePresentation::Card,
                |block| {
                    block
                        .p_token(&theme, Space::Sm)
                        .radius(&theme, Radius::Card)
                        .frame(&theme, Surface::Raised, Elevation::Raised)
                },
            )
            .when(
                self.code_presentation == MarkdownCodePresentation::Flat,
                // A fence is part of the document's reading flow, not a
                // second surface asking to become its visual focus.
                |block| {
                    block
                        .py_token(&theme, Space::Xs)
                        .border_b(px(theme.borders.hairline))
                        .border_color(theme.colors.divider.opacity(theme.opacity.muted))
                },
            )
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Text)
                    .parent(self.ident.semantic_id())
                    .text(label)
                    .value(cx.strings().format_plural(
                        StringKey::MarkdownLineOne,
                        StringKey::MarkdownLines,
                        cx.numbers().plural(lines),
                        &[cx.numbers().count(lines).as_ref()],
                    )),
            )
            .into_any_element()
    }

    fn quote(&mut self, blocks: &[Block], window: &mut Window, cx: &mut App) -> AnyElement {
        let theme = self.theme.clone();
        let mut column = div()
            .column()
            .flex_1()
            .min_w_0()
            .gap_token(&theme, Space::Sm);
        for block in blocks {
            column = column.child(self.block(block, window, cx));
        }
        // A quote stays on the document plane. Its rail states the hierarchy;
        // a recessed rounded surface would repeat that fact and make ordinary
        // quoted prose compete with media for attention.
        div()
            .row()
            .items_stretch()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .py_token(&theme, Space::Xs)
            .child(
                div()
                    .w(px(theme.effects.rail_width))
                    .flex_none()
                    .radius(&theme, Radius::Pill)
                    .bg(theme.colors.hairline_strong.opacity(theme.opacity.muted)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(theme.colors.text_muted)
                    .child(column),
            )
            .into_any_element()
    }

    fn list(
        &mut self,
        ordered: bool,
        start: u64,
        entries: &[ListEntry],
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let theme = self.theme.clone();
        let mut column = div().column().w_full().gap_token(&theme, Space::Xs);
        for (offset, entry) in entries.iter().enumerate() {
            column = column.child(self.entry(ordered, start, offset, entry, window, cx));
        }
        column.into_any_element()
    }

    fn entry(
        &mut self,
        ordered: bool,
        start: u64,
        offset: usize,
        entry: &ListEntry,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let theme = self.theme.clone();
        // A tick is drawn, not typed. The box characters CommonMark suggests
        // are outside every face this library bundles, so a document that used
        // them rendered its task list as two missing-glyph boxes on any
        // machine without a font that happened to cover them.
        let marker: AnyElement = match (entry.task, ordered) {
            (Some(checked), _) => gpui_kit_assets::icon(if checked {
                Icon::CheckboxChecked
            } else {
                Icon::CheckboxEmpty
            })
            .size(px(theme.typography.body.line_height))
            .text_color(if checked {
                theme.colors.text_muted
            } else {
                theme.colors.text_faint
            })
            .into_any_element(),
            (None, true) => div()
                .type_scale(&theme, TypeScale::Body)
                .text_color(theme.colors.text_faint)
                .child(cx.numbers().ordinal(start + offset as u64))
                .into_any_element(),
            (None, false) => div()
                .type_scale(&theme, TypeScale::Body)
                .text_color(theme.colors.text_faint)
                .child(SharedString::new_static("•"))
                .into_any_element(),
        };

        let mut body = div()
            .column()
            .flex_1()
            .min_w_0()
            .gap_token(&theme, Space::Xs);
        for block in &entry.blocks {
            body = body.child(self.block(block, window, cx));
        }

        let row = div()
            .row()
            .items_start()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .min_w(px(18.0))
                    .h(px(theme.typography.body.line_height))
                    .child(marker),
            )
            .child(body);

        // A task in a rendered document is a fact about the document, not a
        // control: this component ticks nothing, so the box publishes its
        // state and says it cannot be operated.
        match entry.task {
            Some(checked) => {
                let stated = entry.blocks.iter().find_map(|block| match block {
                    Block::Paragraph(inlines) => Some(flatten(inlines)),
                    _ => None,
                });
                // The id is seeded from the task's own words, or from an
                // English constant when it has none: a semantic id that moved
                // when the host installed a translation would not be stable.
                let ident = self.ident_for("task", stated.as_deref().unwrap_or(TASK_ID));
                let text = stated.unwrap_or_else(|| cx.strings().text(StringKey::MarkdownTask));
                row.semantic_in(
                    cx,
                    NodeSpec::new(ident.semantic_id(), Role::Checkbox)
                        .parent(self.ident.semantic_id())
                        .text(text)
                        .checked(checked)
                        .disabled(true),
                )
                .into_any_element()
            }
            None => row.into_any_element(),
        }
    }

    fn rule(&mut self, cx: &mut App) -> AnyElement {
        let theme = self.theme.clone();
        let ident = self.ident_for("rule", "break");
        crate::foundation::rule(&theme)
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Separator)
                    .parent(self.ident.semantic_id()),
            )
            .into_any_element()
    }

    fn table(
        &mut self,
        alignment: &[CellAlign],
        head: &[Vec<Inline>],
        rows: &[Vec<Vec<Inline>>],
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let theme = self.theme.clone();
        let name = flatten(head.first().map(Vec::as_slice).unwrap_or_default());
        let ident = self.ident_for("table", name.as_ref());

        // A table is structured information in this document, not a durable
        // result or a media object. Row rules carry its structure directly on
        // the reading plane instead of enclosing it in another raised card.
        let mut frame = div().column().w_full();

        if !head.is_empty() {
            let mut header = div()
                .row()
                .items_stretch()
                .w_full()
                .border_b(px(theme.borders.hairline))
                .border_color(theme.colors.divider)
                .type_scale(&theme, TypeScale::Caption)
                .text_color(theme.colors.text_muted);
            for (column, cell) in head.iter().enumerate() {
                let content = self.inline_row(cell, RunStyle::default(), window, cx);
                header =
                    header.child(cell_frame(&theme, aligned(alignment, column)).child(content));
            }
            frame = frame.child(header);
        }

        for row in rows {
            let mut line = div()
                .row()
                .items_stretch()
                .w_full()
                .border_b(px(theme.borders.hairline))
                .border_color(theme.colors.divider.opacity(theme.opacity.muted))
                .type_scale(&theme, TypeScale::Label)
                .text_color(theme.colors.text);
            for (column, cell) in row.iter().enumerate() {
                let content = self.inline_row(cell, RunStyle::default(), window, cx);
                line = line.child(cell_frame(&theme, aligned(alignment, column)).child(content));
            }
            frame = frame.child(line);
        }

        frame
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Table)
                    .parent(self.ident.semantic_id())
                    .text(name)
                    .value(cx.strings().format_plural(
                        StringKey::MarkdownRowOne,
                        StringKey::MarkdownRows,
                        cx.numbers().plural(rows.len()),
                        &[cx.numbers().count(rows.len()).as_ref()],
                    )),
            )
            .into_any_element()
    }

    fn html_block(&mut self, html: SharedString, cx: &mut App) -> AnyElement {
        let theme = self.theme.clone();
        let ident = self.ident_for("html", "block");
        let explanation = cx.strings().text(StringKey::MarkdownUnrenderedHtml);
        div()
            .id(ident.element_id())
            .column()
            .w_full()
            .px_token(&theme, Space::Sm)
            .py_token(&theme, Space::Xs)
            .radius(&theme, Radius::Small)
            .bg(theme.semantic_wash(SemanticColor::Warning, SemanticWash::Faint))
            .child(
                div()
                    .mono(&theme)
                    .text_size(px(theme.typography.code.size))
                    .line_height(px(theme.typography.code.line_height))
                    .text_color(theme.colors.text_muted)
                    .children(
                        html.lines()
                            .map(|line| div().child(SharedString::from(line.to_string()))),
                    ),
            )
            .tip(ident.clone(), explanation.clone())
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Text)
                    .parent(self.ident.semantic_id())
                    .text(html.clone())
                    .description(explanation)
                    .value(UNRENDERED),
            )
            .into_any_element()
    }

    fn html_inline(&mut self, html: SharedString, cx: &mut App) -> AnyElement {
        let theme = self.theme.clone();
        let ident = self.ident_for("html", "inline");
        div()
            .px(px(theme.space(Space::Xxs)))
            .radius(&theme, Radius::Small)
            .bg(theme.semantic_wash(SemanticColor::Warning, SemanticWash::Standard))
            .mono(&theme)
            .text_size(px(theme.typography.code.size))
            .text_color(theme.colors.warning)
            .child(html.clone())
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Text)
                    .parent(self.ident.semantic_id())
                    .text(html)
                    .value(UNRENDERED),
            )
            .into_any_element()
    }

    /// What a truncated document left out, and a way to ask for it.
    ///
    /// A count and a named action, rather than a fade: a gradient over the
    /// last line says something was cut without saying how much, which is a
    /// decoration standing where a fact belongs.
    fn more(&mut self, hidden: usize, cx: &mut App) -> AnyElement {
        let theme = self.theme.clone();
        let ident = self.ident.child("truncated");
        let digits = cx.numbers().count(hidden);
        let label = cx.strings().format_plural(
            StringKey::MarkdownShowMoreOne,
            StringKey::MarkdownShowMoreMany,
            cx.numbers().plural(hidden),
            &[digits.as_ref()],
        );
        let asked = self.report(MarkdownEvent::MoreRequested { lines: hidden });

        div()
            .row()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .child(
                Button::new(ident.child("more"))
                    .label(label.clone())
                    .link()
                    .on_click(move |window, cx| {
                        if let Some(asked) = &asked {
                            asked(window, cx);
                        }
                    }),
            )
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Status)
                    .parent(self.ident.semantic_id())
                    .text(label)
                    .value(digits),
            )
            .into_any_element()
    }
}

/// The published value for a fragment of HTML that was neither run nor
/// dropped. It is a state name a test asserts, not a word a reader reads;
/// [`StringKey::MarkdownUnrenderedHtml`](crate::strings::StringKey) is the
/// reader's copy.
const UNRENDERED: SharedString = SharedString::new_static("unrendered html");

/// Id seeds for the two blocks whose id would otherwise come from a
/// translated word.
const PLAIN_TEXT_ID: &str = "plain text";
const TASK_ID: &str = "task";

fn run(
    theme: &Theme,
    ident: &Ident,
    order: u64,
    text: SharedString,
    style: RunStyle,
    fading: &[(Range<usize>, f32)],
) -> AnyElement {
    let color = if style.struck {
        theme.colors.text_muted
    } else {
        style.color.unwrap_or(theme.colors.text)
    };
    div()
        // A run is a flex item in a wrapping row, and gpui answers a
        // min-content probe for text with the width the whole run would take
        // unwrapped. Without this the item's automatic minimum is that width,
        // so a paragraph written as one long run cannot shrink to the column
        // it sits in and walks out past it instead of wrapping inside it.
        .min_w_0()
        .when(style.strong, |element| {
            element.font_weight(FontWeight::BOLD)
        })
        .when(style.emphasis, gpui::Styled::italic)
        .when(style.struck, |element| {
            element.line_through().text_color(theme.colors.text_muted)
        })
        .child(
            StyledText::new(text)
                .with_highlights(fade(fading, color))
                .selectable_in_document(ident.element_id(), ident.semantic_id(), order),
        )
        .into_any_element()
}

/// Whether a byte range still names a slice of `text`.
///
/// A highlight range that does not is not a cosmetic problem: `StyledText`
/// asserts both ends are on a character boundary, so handing it a stale offset
/// aborts the process. Both ends have to be checked and so does the length,
/// because `is_char_boundary` is false for an index past the end as well as
/// for one inside a character.
///
/// This is the same guard [`code_highlights`](crate::content::code_view) has
/// always applied to syntax spans. The veil is the one place that trusted an
/// offset it did not compute against the string in front of it.
fn fits(text: &str, range: &Range<usize>) -> bool {
    range.end <= text.len()
        && text.is_char_boundary(range.start)
        && text.is_char_boundary(range.end)
}

/// Turns opacities into the colours that express them.
///
/// A fade is applied by naming the run's own colour at a lower alpha, so it
/// touches nothing but transparency: the glyphs are already laid out, already
/// where they will stay, and already the size they will be.
fn fade(fading: &[(Range<usize>, f32)], color: Hsla) -> Vec<(Range<usize>, HighlightStyle)> {
    fading
        .iter()
        .map(|(range, opacity)| {
            (
                range.clone(),
                HighlightStyle {
                    color: Some(color.opacity(*opacity)),
                    ..Default::default()
                },
            )
        })
        .collect()
}

fn cell_frame(theme: &Theme, align: CellAlign) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .px_token(theme, Space::Sm)
        .py_token(theme, Space::Xs)
        .when(align == CellAlign::Center, |element| element.items_center())
        .when(align == CellAlign::End, |element| element.items_end())
}

fn aligned(alignment: &[CellAlign], column: usize) -> CellAlign {
    alignment.get(column).copied().unwrap_or_default()
}

/// The text a run of inlines reads as, for a name and for an id stem.
fn flatten(inlines: &[Inline]) -> SharedString {
    fn walk(inlines: &[Inline], into: &mut String) {
        for inline in inlines {
            match inline {
                Inline::Text(text) | Inline::Code(text) | Inline::Html(text) => into.push_str(text),
                Inline::Emphasis(inner) | Inline::Strong(inner) | Inline::Struck(inner) => {
                    walk(inner, into)
                }
                Inline::Link { content, .. } => walk(content, into),
                Inline::Image { alt, .. } => into.push_str(alt),
                Inline::SoftBreak | Inline::HardBreak => into.push(' '),
            }
        }
    }
    let mut text = String::new();
    walk(inlines, &mut text);
    SharedString::from(text.trim().to_string())
}

/// An id-safe stem for a name, bounded so one long heading cannot mint an
/// unreadable identity.
fn slug(name: &str) -> String {
    let mut slug = String::new();
    for character in name.chars().take(64) {
        if character.is_ascii_alphanumeric() {
            slug.extend(character.to_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "item".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slug_never_carries_punctuation_or_an_empty_stem() {
        assert_eq!(
            slug("https://example.test/a?b=1"),
            "https-example-test-a-b-1"
        );
        assert_eq!(slug("   "), "item");
        assert_eq!(slug("Getting started!"), "getting-started");
    }

    #[test]
    fn flattening_reads_a_line_the_way_it_is_spoken() {
        let document = Document::parse("**bold** and `code` and [a link](x)");
        let Some(Block::Paragraph(inlines)) = document.blocks.first() else {
            panic!("expected a paragraph");
        };
        assert_eq!(flatten(inlines).as_ref(), "bold and code and a link");
    }

    /// The crash this guard exists for: a span recorded against one frame's
    /// text, read by the next after the document reflowed. `跟随镜` is nine
    /// bytes, so an offset taken from it lands inside a character of anything
    /// shorter — and `StyledText::with_highlights` aborts rather than skips.
    ///
    /// An English transcript survives this for a long time because every byte
    /// of ASCII is a boundary; a Chinese one hits it on the first reflow.
    #[test]
    fn a_span_from_a_frame_that_reflowed_is_not_applied_to_the_new_text() {
        let settled = "跟随镜头 · src/camera/follow.ts";
        let recorded = 0..9;
        assert!(fits(settled, &recorded), "a live range still applies");

        // The run now holds something shorter: the range overruns it.
        assert!(!fits("·", &recorded));
        // And something the same length in characters but not in bytes.
        assert!(!fits("abc", &recorded));

        // Mid-character is the case ASCII never produces. Byte 1 is inside
        // `跟`, and asking StyledText to start a highlight there aborts.
        assert!(!fits(settled, &(1..9)));
        assert!(!fits(settled, &(0..4)));

        // The boundaries either side of one CJK character are fine.
        assert!(fits(settled, &(3..6)));
        // An empty range at the very end is a boundary, and harmless.
        assert!(fits(settled, &(settled.len()..settled.len())));
    }
}
