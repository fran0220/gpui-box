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

pub mod parse;

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, ClipboardItem, FontWeight, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, StyledText, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{ActiveTheme, Elevation, Radius, Space, Surface, Theme, TypeScale};

use crate::content::code_view::styled_code;
use crate::controls::button::Button;
use crate::display::badge::Tone;
use crate::foundation::{Ident, Sizable, StyledExt};
use crate::motion::keyed;
use crate::overlay::Tooltipped;
use crate::strings::{ActiveStrings, StringKey};

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
/// The tone vocabulary is the library's existing one rather than a set of
/// syntax categories, because deciding that a word is a keyword is the
/// grammar's judgement and this crate has no grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSpan {
    pub range: Range<usize>,
    pub tone: Tone,
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
    on_event: Option<EventHandler>,
    image: Option<ImageSource>,
    highlighter: Option<Highlighter>,
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
            .finish()
    }
}

impl Markdown {
    pub fn new(ident: impl Into<Ident>, source: impl Into<SharedString>) -> Self {
        Self {
            ident: ident.into(),
            source: source.into(),
            max_lines: None,
            on_event: None,
            image: None,
            highlighter: None,
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

    /// Colours code blocks from spans the host computed.
    ///
    /// Without this every fence renders plain. Guessing a language from its
    /// text, or a grammar from its name, would put colour on a claim nobody
    /// made.
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
        let parsed = Document::parse(self.source.as_ref());
        let (document, hidden) = match self.max_lines {
            Some(max) => parsed.truncate(max),
            None => (parsed, 0),
        };

        let mut painter = Painter {
            ident: ident.clone(),
            theme: theme.clone(),
            on_event: self.on_event.clone(),
            image: self.image.clone(),
            highlighter: self.highlighter.clone(),
            used: HashMap::new(),
            requested: Vec::new(),
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

        column.semantic_in(
            cx,
            NodeSpec::new(ident.semantic_id(), Role::Region)
                .value(format!("{} blocks", document.blocks.len())),
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
    let cell = keyed::slot::<Requested>(&ident.child("images").semantic_id(), cx);
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
}

/// Walks the tree once, drawing it and minting one id per addressable part.
struct Painter {
    ident: Ident,
    theme: Theme,
    on_event: Option<EventHandler>,
    image: Option<ImageSource>,
    highlighter: Option<Highlighter>,
    /// How many parts have already claimed each id stem, so a document that
    /// links to the same place twice still publishes two distinct nodes.
    used: HashMap<String, usize>,
    requested: Vec<ImageRequest>,
}

impl Painter {
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

    fn report(&self, event: MarkdownEvent) -> Option<impl Fn(&mut Window, &mut App) + use<>> {
        let handler = self.on_event.clone()?;
        Some(move |window: &mut Window, cx: &mut App| handler(&event, window, cx))
    }

    fn block(&mut self, block: &Block, window: &mut Window, cx: &mut App) -> AnyElement {
        match block {
            Block::Heading { level, content } => self.heading(*level, content, window, cx),
            Block::Paragraph(inlines) => self.paragraph(inlines, window, cx),
            Block::Code { language, text } => self.code(language.clone(), text.clone(), cx),
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
                    elements.push(run(&theme, ident.element_id(), text.clone(), style));
                }
                Inline::Code(text) => {
                    let ident = self.ident_for("code", text.as_ref());
                    elements.push(
                        div()
                            .px(px(theme.space(Space::Xs)))
                            .radius(&theme, Radius::Small)
                            .bg(theme.colors.raised)
                            .font_family(theme.typography.mono.clone())
                            .text_size(px(theme.typography.code.size))
                            .child(StyledText::new(text.clone()).selectable(ident.element_id()))
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
                    elements.push(run(&theme, ident.element_id(), " ".into(), style));
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
        let ident = self.ident_for("link", href.as_ref());
        // Hover help states the destination before the reader commits, and the
        // title the author wrote is shown beside it rather than instead of it.
        let help = match title {
            Some(title) => SharedString::from(format!("{title} — {href}")),
            None => href.clone(),
        };
        let runs = self.inlines(content, style, window, cx);
        let taken = self.report(MarkdownEvent::LinkClicked { href: href.clone() });

        div()
            .id(ident.element_id())
            .flex()
            .flex_row()
            .flex_wrap()
            .cursor_pointer()
            .text_color(theme.colors.accent)
            .underline()
            .tip(ident.clone(), help)
            .children(if runs.is_empty() {
                vec![run(
                    &theme,
                    ident.child("text").element_id(),
                    label.clone(),
                    style,
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
                .gap(px(2.0))
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
        cx: &mut App,
    ) -> AnyElement {
        let theme = self.theme.clone();
        // The id seed stays English on purpose: a semantic id that moved when
        // the host installed a translation would not be a stable id.
        let ident = self.ident_for("code", language.as_deref().unwrap_or(PLAIN_TEXT_ID));
        let label = language
            .clone()
            .unwrap_or_else(|| cx.strings().text(StringKey::MarkdownPlainText));
        let block = CodeBlock {
            language: language.clone(),
            text: text.clone(),
        };
        let spans = self
            .highlighter
            .as_ref()
            .map(|highlight| highlight(&block))
            .unwrap_or_default();
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
            .font_family(theme.typography.mono.clone())
            .text_size(px(theme.typography.code.size))
            .line_height(px(theme.typography.code.line_height))
            .text_color(theme.colors.text)
            .child(
                styled_code(&theme, text.clone(), &spans)
                    .selectable(ident.child("text").element_id()),
            );

        div()
            .column()
            .w_full()
            .gap_token(&theme, Space::Xs)
            .p_token(&theme, Space::Sm)
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Raised, Elevation::Raised)
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
            .child(body)
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Text)
                    .parent(self.ident.semantic_id())
                    .text(label)
                    .value(format!("{lines} lines")),
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
        div()
            .row()
            .items_stretch()
            .w_full()
            .gap_token(&theme, Space::Sm)
            .child(
                div()
                    .w(px(theme.borders.thick))
                    .flex_none()
                    .bg(theme.colors.hairline_strong),
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
        let marker = match (entry.task, ordered) {
            (Some(true), _) => SharedString::new_static("☑"),
            (Some(false), _) => SharedString::new_static("☐"),
            (None, true) => SharedString::from(format!("{}.", start + offset as u64)),
            (None, false) => SharedString::new_static("•"),
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
                    .min_w(px(18.0))
                    .type_scale(&theme, TypeScale::Body)
                    .text_color(theme.colors.text_faint)
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
        div()
            .w_full()
            .h(px(theme.borders.hairline))
            .bg(theme.colors.hairline)
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

        let mut frame = div()
            .column()
            .w_full()
            .radius(&theme, Radius::Card)
            .frame(&theme, Surface::Panel, Elevation::Raised)
            .overflow_hidden();

        if !head.is_empty() {
            let mut header = div()
                .row()
                .items_stretch()
                .w_full()
                .bg(theme.colors.raised)
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
                    .value(format!("{} rows", rows.len())),
            )
            .into_any_element()
    }

    fn html_block(&mut self, html: SharedString, cx: &mut App) -> AnyElement {
        let theme = self.theme.clone();
        let ident = self.ident_for("html", "block");
        div()
            .column()
            .w_full()
            .gap(px(2.0))
            .px_token(&theme, Space::Sm)
            .py_token(&theme, Space::Xs)
            .radius(&theme, Radius::Small)
            .border(px(theme.borders.hairline))
            .border_color(theme.colors.warning.opacity(0.4))
            .bg(theme.colors.warning.opacity(0.06))
            .child(
                div()
                    .type_scale(&theme, TypeScale::Caption)
                    .text_color(theme.colors.warning)
                    .child(cx.strings().text(StringKey::MarkdownUnrenderedHtml)),
            )
            .child(
                div()
                    .font_family(theme.typography.mono.clone())
                    .text_size(px(theme.typography.code.size))
                    .line_height(px(theme.typography.code.line_height))
                    .text_color(theme.colors.text_muted)
                    .children(
                        html.lines()
                            .map(|line| div().child(SharedString::from(line.to_string()))),
                    ),
            )
            .semantic_in(
                cx,
                NodeSpec::new(ident.semantic_id(), Role::Text)
                    .parent(self.ident.semantic_id())
                    .text(html.clone())
                    .value(UNRENDERED),
            )
            .into_any_element()
    }

    fn html_inline(&mut self, html: SharedString, cx: &mut App) -> AnyElement {
        let theme = self.theme.clone();
        let ident = self.ident_for("html", "inline");
        div()
            .px(px(2.0))
            .radius(&theme, Radius::Small)
            .bg(theme.colors.warning.opacity(0.1))
            .font_family(theme.typography.mono.clone())
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
        let label = if hidden == 1 {
            cx.strings().text(StringKey::MarkdownShowMoreOne)
        } else {
            cx.strings()
                .format(StringKey::MarkdownShowMoreMany, &[&hidden.to_string()])
        };
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
                    .value(hidden.to_string()),
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

fn run(theme: &Theme, id: gpui::ElementId, text: SharedString, style: RunStyle) -> AnyElement {
    div()
        .when(style.strong, |element| {
            element.font_weight(FontWeight::BOLD)
        })
        .when(style.emphasis, gpui::Styled::italic)
        .when(style.struck, |element| {
            element.line_through().text_color(theme.colors.text_muted)
        })
        .child(StyledText::new(text).selectable(id))
        .into_any_element()
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
}
