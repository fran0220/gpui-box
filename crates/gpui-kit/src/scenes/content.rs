//! Documents, code, and streams a reader reads.

use super::support::*;
#[cfg(all(feature = "terminal", not(target_family = "wasm")))]
use crate::content::{CellSide, Emulator, GridSnapshot, SelectionKind, Terminal, TerminalState};

/// The complete shell contract: every state remains visibly distinct, and
/// the narrow panels prove long addresses and page content stay clipped to
/// the caller-owned viewport.
pub(super) fn browser_panel(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(
            row(&theme)
                .items_start()
                .child(
                    div().w(px(232.0)).h(px(190.0)).child(
                        BrowserPanel::new("scene.browser.loading")
                            .url("https://docs.example.com/a/long/path/that-must-stay-inside-the-address-well")
                            .state(ViewportState::Loading)
                            .on_reload(|_, _| {}),
                    ),
                )
                .child(
                    div().w(px(232.0)).h(px(190.0)).child(
                        BrowserPanel::new("scene.browser.empty")
                            .url("https://docs.example.com/empty")
                            .state(ViewportState::Empty)
                            .on_back(|_, _| {})
                            .on_reload(|_, _| {}),
                    ),
                )
                .child(
                    div().w(px(232.0)).h(px(190.0)).child(
                        BrowserPanel::new("scene.browser.unavailable")
                            .url("https://internal.example.com/admin")
                            .state(ViewportState::Unavailable(
                                "The workspace policy does not allow this host.".into(),
                            ))
                            .on_reload(|_, _| {}),
                    ),
                ),
        )
        .child(
            row(&theme)
                .items_start()
                .child(
                    div().w(px(352.0)).h(px(210.0)).child(
                        BrowserPanel::new("scene.browser.error")
                            .url("https://status.example.com/incidents/current")
                            .state(ViewportState::Error(
                                "The host could not resolve this address.".into(),
                            ))
                            .on_back(|_, _| {})
                            .on_reload(|_, _| {}),
                    ),
                )
                .child(
                    div().w(px(352.0)).h(px(210.0)).child(
                        BrowserPanel::new("scene.browser.ready")
                            .url("https://docs.example.com/ready")
                            .state(ViewportState::Ready)
                            .viewport(
                                div()
                                    .size_full()
                                    .p_token(&theme, Space::Md)
                                    .child(crate::foundation::text(
                                        &theme,
                                        TypeScale::Subtitle,
                                        "Host-owned page surface",
                                    ))
                                    .child(
                                        div()
                                            .mt_token(&theme, Space::Sm)
                                            .child(crate::foundation::text(&theme, TypeScale::Body, "Long page content remains clipped by the BrowserPanel viewport even when it cannot wrap naturally.").text_tone(&theme, TextTone::Muted)),
                                    ),
                            )
                            .on_back(|_, _| {})
                            .on_forward(|_, _| {})
                            .on_reload(|_, _| {}),
                    ),
                ),
        )
        .into_any_element()
}

pub(super) fn log_stream(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let entries = [
        LogEntry::new("boot", "Fixture worker entered the queue")
            .timestamp("09:41:02")
            .level("INFO", Tone::Info)
            .source("scheduler"),
        LogEntry::new("claim", "Fixture worker claimed run demo-42")
            .timestamp("09:41:03")
            .level("INFO", Tone::Info)
            .source("worker")
            .search_hits(std::iter::once(23..26))
            .current_hit(0),
        LogEntry::new("cache", "Fixture cache was not warm")
            .timestamp("09:41:04")
            .level("WARN", Tone::Warning)
            .source("cache"),
        LogEntry::new("ansi", "\u{1b}[31mfailed\u{1b}[0m after retry")
            .timestamp("09:41:04")
            .level("ERR", Tone::Danger)
            .source("worker"),
        LogEntry::new("retry", "Fixture request completed after one retry")
            .timestamp("09:41:05")
            .level("INFO", Tone::Info)
            .source("worker"),
        LogEntry::new("finish", "Fixture run demo-42 completed")
            .timestamp("09:41:06")
            .level("DONE", Tone::Success)
            .source("scheduler")
            .search_hits(std::iter::once(12..19)),
    ];
    stack(&theme)
        .w(px(780.0))
        .child(caption(
            &theme,
            "fixed virtual rows; metadata and search ranges are caller inputs",
        ))
        .child(
            LogStream::new("scene.log", entries)
                .ansi(true)
                .state(LogStreamState::Stale(
                    "The latest refresh did not complete; verified entries remain.".into(),
                ))
                .visible_rows(5)
                .selected("retry")
                .on_select(|_, _, _| {})
                .on_copy(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn scene_diff() -> Vec<DiffFile> {
    vec![DiffFile::new(
        "report",
        "src/report.rs",
        [DiffHunk::new(
            "summary",
            "@@ -40,3 +40,4 @@ fn report",
            [
                DiffLine::new("signature", "fn report() -> Outcome {")
                    .old_number(40)
                    .new_number(40)
                    .spans([CodeSpan {
                        range: 0..2,
                        role: SyntaxColor::Keyword,
                    }]),
                DiffLine::paired("cache", "    old_cache.read()", "    verified_cache.read()")
                    .old_number(41)
                    .new_number(41)
                    .old_spans({
                        let (old, _) =
                            word_spans("    old_cache.read()", "    verified_cache.read()");
                        old
                    })
                    .new_spans({
                        let (_, new) =
                            word_spans("    old_cache.read()", "    verified_cache.read()");
                        new
                    }),
                DiffLine::added("audit", "    audit.record()").new_number(42),
                DiffLine::new("close", "}").old_number(42).new_number(43),
            ],
        )],
    )]
}

pub(super) fn diff_view(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(840.0))
        .child(caption(
            &theme,
            "the same caller-owned rows arranged as unified and split",
        ))
        .child(
            div()
                .column()
                .items_start()
                .gap_token(&theme, Space::Md)
                .child(
                    div().w(px(840.0)).child(
                        DiffView::new("scene.diff.unified", scene_diff())
                            .visible_rows(7)
                            .on_event(|_, _, _| {}),
                    ),
                )
                .child(
                    div().w(px(840.0)).child(
                        DiffView::new("scene.diff.split", scene_diff())
                            .presentation(DiffPresentation::Split)
                            .visible_rows(7)
                            .on_event(|_, _, _| {}),
                    ),
                ),
        )
        .child(caption(
            &theme,
            "a review of several files: two folded behind headers that say what \
             they hold, one open and coloured by its named language",
        ))
        .child(
            div().w(px(840.0)).child(
                DiffView::new("scene.diff.review", scene_review())
                    .language("rust")
                    .visible_rows(9)
                    .on_event(|_, _, _| {}),
            ),
        )
        .child(caption(
            &theme,
            "the same rows measured rather than slotted, so a line longer than \
             the frame wraps instead of being cut off",
        ))
        .child(
            div().w(px(420.0)).child(
                DiffView::new("scene.diff.wrapping", scene_long_line())
                    .wrapping(true)
                    .visible_rows(5)
                    .on_event(|_, _, _| {}),
            ),
        )
        .into_any_element()
}

/// Three files, two of them folded: the shape the top of a real review has.
fn scene_review() -> Vec<DiffFile> {
    let mut files = scene_diff();
    files.push(
        DiffFile::new(
            "adapter",
            "src/adapter.rs",
            [DiffHunk::new(
                "open",
                "@@ -12,2 +12,3 @@ fn open",
                [
                    DiffLine::added("guard", "    let held = registry.lock();").new_number(12),
                    DiffLine::removed("stale", "    let held = registry.take();").old_number(12),
                ],
            )],
        )
        .folded(true),
    );
    files.push(
        DiffFile::new(
            "notes",
            "docs/notes.md",
            [DiffHunk::new(
                "intro",
                "@@ -1,1 +1,1 @@",
                [DiffLine::added("line", "The cache is verified now.").new_number(1)],
            )],
        )
        .folded(true),
    );
    files
}

/// One line far wider than the frame it is drawn in, which is the case fixed
/// rows cannot show at all.
fn scene_long_line() -> Vec<DiffFile> {
    vec![DiffFile::new(
        "config",
        "config/build.json",
        [DiffHunk::new(
            "one",
            "@@ -1,1 +1,1 @@",
            [DiffLine::paired(
                "targets",
                "{\"targets\":[\"macos-arm64\",\"macos-x86\",\"windows-x86\"],\"strip\":false}",
                "{\"targets\":[\"macos-arm64\",\"macos-x86\",\"windows-x86\",\"linux-arm64\"],\"strip\":true}",
            )
            .old_number(1)
            .new_number(1)],
        )],
    )]
}

pub(super) fn code_view(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    // Spans are the caller's, in byte offsets into each line, and they
    // outrank what the scanner would have found for the same line.
    let lines = [
        CodeLine::new(40, "fn report(&self) -> Outcome {").spans([CodeSpan {
            range: 0..2,
            role: SyntaxColor::Keyword,
        }]),
        CodeLine::new(41, "    let verified = self.check();")
            .spans([CodeSpan {
                range: 4..7,
                role: SyntaxColor::Keyword,
            }])
            .mark(LineMark::Added),
        CodeLine::new(42, "    let stale = self.cached();").mark(LineMark::Removed),
        CodeLine::new(43, "    Outcome::from(verified)").mark(LineMark::Changed),
        CodeLine::new(
            44,
            "    // this line runs off the edge rather than wrapping, because a column carries meaning in code",
        ),
        CodeLine::new(45, "}").mark(LineMark::Error),
    ];
    stack(&theme)
        .w(px(620.0))
        .child(caption(
            &theme,
            "line numbers are the file's, marks are the host's, colour is the caller's",
        ))
        .child(CodeView::new("scene.code.report", lines).language("rust"))
        .child(caption(
            &theme,
            "and where the caller says only what the language is, the four classes it can find",
        ))
        .child(
            CodeView::new(
                "scene.code.scanned",
                [
                    CodeLine::new(1, "def summarize(runs):  # every class in one line"),
                    CodeLine::new(2, "    total = 0"),
                    CodeLine::new(3, "    for run in runs:"),
                    CodeLine::new(4, "        if run.status == \"passed\":"),
                    CodeLine::new(5, "            total += 1"),
                    CodeLine::new(6, "    return total"),
                ],
            )
            .language("python"),
        )
        .child(caption(
            &theme,
            "a language with no table is shown and left plain, never guessed at",
        ))
        .child(
            CodeView::new(
                "scene.code.unknown",
                [
                    CodeLine::new(1, "REPORT SECTION."),
                    CodeLine::new(2, "    MOVE 0 TO TOTAL."),
                ],
            )
            .language("cobol"),
        )
        .into_any_element()
}

/// A document with one of every block kind in it, including a tag nobody ran.
pub(super) const SCENE_DOCUMENT: &str = r#"# Release notes

The build is **green** again, with *one* caveat and a ~~withdrawn~~ fix.
Details are in [the run log](https://example.test/runs/4821 "the failing run").

## What changed

- Retries are bounded
  - and the bound is stated
- Refusals keep their reason

1. Verify the workspace
2. Publish the artifacts

- [x] Bounded retries
- [ ] Bounded backoff

> A refused request is displayed as a refusal.

```rust
fn main() {
    println!("still green");
}
```

| Stage | Result |
|:------|-------:|
| Build | passed |
| Test  | passed |

<div onclick="steal()">This was written as HTML.</div>

![The run graph](runs/graph.png "yesterday's run")

---

Everything below this line is what truncation leaves out."#;

pub(super) fn markdown(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(640.0))
        .child(Markdown::new("scene.markdown.document", SCENE_DOCUMENT).on_event(|_, _, _| {}))
        .child(Divider::new().id("scene.markdown.rule").label("Truncated"))
        .child(
            Markdown::new("scene.markdown.short", SCENE_DOCUMENT)
                .max_lines(4)
                .on_event(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn agent_document(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(680.0))
        .child(
            AgentDocument::new("scene.agent-document")
                .block(
                    AgentDocumentBlock::markdown(
                        "summary",
                        "## Review complete\n\nThe run kept its **verified result** while the refresh failed.",
                    )
                    .revision(3),
                )
                .block(
                    AgentDocumentBlock::code("patch", |_, _| {
                        CodeView::from_text(
                            "scene.agent-document.patch",
                            "pub fn bounded_retry(attempt: usize) -> bool {\n    attempt < 3\n}",
                        )
                        .language("rust")
                        .into_any_element()
                    })
                    .label("Proposed code")
                    .revision(2),
                )
                .block(
                    AgentDocumentBlock::tool_call("verification", |_, _| {
                        ToolCallCard::new("scene.agent-document.tool", "cargo test")
                            .state(ToolCallState::Succeeded {
                                output: ToolOutput::Silent,
                            })
                            .elapsed("1.4 s")
                            .into_any_element()
                    })
                    .revision(1),
                )
                .block(AgentDocumentBlock::notice(
                    "refresh",
                    "The last refresh failed; the result above is the last verified value.",
                    Tone::Warning,
                )),
        )
        .child(
            Divider::new()
                .id("scene.agent-document.long")
                .label("The same document, virtualized"),
        )
        .child(caption(
            &theme,
            "a conversation that has been running all day: only the blocks on \
             screen are built and laid out, and the ones above are still there. \
             A long answer is drawn as a row per block of its own, so a token \
             arriving in its last paragraph relays out that paragraph",
        ))
        .child(
            AgentDocument::new("scene.agent-thread")
                .virtualized(5)
                .block(
                    AgentDocumentBlock::markdown(
                        "answer",
                        "**The answer.** It kept the verified result, and the run \
                         below says why.\n\n\
                         - the refresh failed\n- the last verified value stands\n\n\
                         Nothing after that point was rewritten.",
                    )
                    .revision(1),
                )
                .blocks((0..120).map(|turn| {
                    AgentDocumentBlock::markdown(
                        format!("turn-{turn:03}"),
                        format!(
                            "**Turn {turn}.** The run kept its verified result while the \
                             refresh failed, and the answer below says why."
                        ),
                    )
                    .revision(turn as u64)
                })),
        )
        .child(
            Divider::new()
                .id("scene.agent-document.rule")
                .label("Document states"),
        )
        .child(
            div()
                .row_reading(cx.layout_direction())
                .gap_token(&theme, Space::Lg)
                .child(
                    div().flex_1().child(
                        AgentDocument::new("scene.agent-document.empty")
                            .state(AgentDocumentState::Empty("No results were returned.".into())),
                    ),
                )
                .child(
                    div().flex_1().child(
                        AgentDocument::new("scene.agent-document.unavailable").state(
                            AgentDocumentState::Unavailable(
                                "The host refused to provide this document.".into(),
                            ),
                        ),
                    ),
                ),
        )
        .into_any_element()
}

/// The thread the conversation scene shows, one message per delivery state.
pub(super) fn scene_thread() -> Vec<Message> {
    vec![
        Message::new("msg-open", "Is the release still blocked?")
            .author("Ada")
            .time("09:14")
            .delivery(DeliveryState::Read),
        Message::markdown(
            "msg-answer",
            "It is not. The **retry bound** landed, so the last failure is gone.",
        )
        .author("Grace")
        .time("09:15")
        .delivery(DeliveryState::Delivered)
        .reaction(Reaction::new("thumbs", "👍", 2)),
        Message::new("msg-log", "Attaching the run log.")
            .author("Grace")
            .time("09:15")
            .delivery(DeliveryState::Sent)
            .attachment(Attachment::new("run-4821", "run-4821.log").detail("12 KB")),
        Message::new("msg-queued", "Then I will publish the artifacts.")
            .author("Ada")
            .delivery(DeliveryState::Sending),
        Message::new("msg-refused", "Publishing the artifacts now.")
            .author("Ada")
            .time("09:16")
            .failed("The workspace is frozen for the release."),
        Message::markdown("msg-stream", "Checking the freeze window")
            .author("Assistant")
            .time("09:16")
            .streaming(true)
            .delivery(DeliveryState::Sending),
    ]
}

pub(super) fn conversation(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .child(
            MessageList::new("scene.conversation.thread", scene_thread())
                .group_consecutive(true)
                .body_lines(2)
                .on_retry(|_, _, _| {})
                .on_markdown(|_, _, _, _| {}),
        )
        .child(
            Divider::new()
                .id("scene.conversation.rule")
                .label("Scrolled away from the newest message"),
        )
        .child(
            // A viewport shorter than the thread opens at its top, so the
            // messages below it have never been on screen and the list says
            // how many there are rather than letting them be discovered.
            MessageList::new("scene.conversation.behind", scene_thread())
                .visible_rows(2)
                .body_lines(2)
                .on_retry(|_, _, _| {}),
        )
        .into_any_element()
}

/// An outline beside the surface it maps, at both lengths that matter: short
/// enough for a mark per place, and long enough that the marks have to
/// condense into even ranges.
///
/// Both are drawn because the whole design claim is that the footprint does
/// not change between them.
pub(super) fn outline(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let mapped = |ident: &'static str, turns: usize| {
        let rows = turns * 2;
        div()
            .flex()
            .gap(px(theme.space(Space::Sm)))
            .h(px(220.0))
            .child(
                Outline::new(format!("{ident}.outline"))
                    .over(format!("{ident}.rows"))
                    .marks((0..turns).map(|turn| {
                        Mark::new(format!("ask-{turn}"), turn * 2, format!("Question {turn}"))
                            .detail(format!("The answer to question {turn} began here."))
                    })),
            )
            .child(
                div().w(px(440.0)).child(
                    List::new(ident, rows, move |row, _, _| {
                        let (who, what) = match row % 2 {
                            0 => ("Ada", format!("Question {}", row / 2)),
                            _ => ("Assistant", format!("Answer {}", row / 2)),
                        };
                        ListItem::new(format!("row-{row}"), div().child(format!("{who}: {what}")))
                            .text(format!("{who}: {what}"))
                    })
                    .flowing()
                    // Bounded, or the list draws every row it has and the
                    // outline has no viewport to be an outline of.
                    .visible_rows(6),
                ),
            )
    };
    stack(&theme)
        .w(px(560.0))
        .child(caption(
            &theme,
            "five exchanges: a mark for each, and the one being read is brighter",
        ))
        .child(mapped("scene.outline.short.rows", 5))
        .child(caption(
            &theme,
            "eighty exchanges: the same footprint, each mark now standing for a range",
        ))
        .child(mapped("scene.outline.long.rows", 80))
        .into_any_element()
}

/// The same thread drawn both ways, so the trade is visible rather than
/// described: a slot leaves the long message's last lines out and says how
/// many, and a message that grows to fit is whole.
pub(super) fn conversation_growing(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let long = || {
        vec![
            Message::new("msg-ask", "What did the freeze window actually cover?")
                .author("Ada")
                .time("09:14")
                .delivery(DeliveryState::Read),
            Message::new(
                "msg-long",
                "Everything published after the tag.\nThe artifacts, the changelog.\nThe signed manifest.\nThe release notes.\nNothing before it moved.",
            )
            .author("Grace")
            .time("09:15")
            .delivery(DeliveryState::Delivered),
        ]
    };
    stack(&theme)
        .w(px(560.0))
        .child(
            Divider::new()
                .id("scene.growing.slotted.rule")
                .label("A slot, two lines of body"),
        )
        .child(
            MessageList::new("scene.growing.slotted", long())
                .body_lines(2)
                .on_retry(|_, _, _| {}),
        )
        .child(
            Divider::new()
                .id("scene.growing.grown.rule")
                .label("As tall as what it says"),
        )
        .child(
            MessageList::new("scene.growing.grown", long())
                .grows_to_fit()
                .on_retry(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn image_viewer(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(860.0))
        .child(
            div()
                .row()
                .items_start()
                .gap(px(theme.spacing.lg))
                .child(
                    div().w(px(396.0)).child(
                        ImageViewer::new(
                            "scene.image.ready",
                            [
                                ImageFrame::new("graph", "The run graph")
                                    .source("runs/graph.png")
                                    .natural(1600, 900),
                                ImageFrame::new("trace", "The failing trace")
                                    .source("runs/trace.png")
                                    .natural(1200, 1200),
                            ],
                        )
                        .showing("graph")
                        .fit(FitMode::Contain)
                        .height(200.0)
                        .image(|_, _, cx| Some(scene_picture("Supplied by the host", cx)))
                        .on_event(|_, _, _| {}),
                    ),
                )
                .child(
                    div().w(px(396.0)).child(
                        ImageViewer::new(
                            "scene.image.refused",
                            [ImageFrame::new("scan", "Page 4 of the scan")
                                .source("scans/page-4.tiff")
                                .natural(2480, 3508)
                                .unavailable("The workspace is frozen for the release.")],
                        )
                        .height(200.0)
                        .image(|_, _, cx| Some(scene_picture("Supplied by the host", cx)))
                        .on_event(|_, _, _| {}),
                    ),
                ),
        )
        .child(
            Divider::new()
                .id("scene.image.rule.unmeasured")
                .label("A size the host never stated"),
        )
        .child(
            div().w(px(396.0)).child(
                ImageViewer::new(
                    "scene.image.unmeasured",
                    [ImageFrame::new("sketch", "A pasted sketch").source("clipboard")],
                )
                .height(200.0)
                .image(|_, _, cx| Some(scene_picture("Size never stated", cx)))
                .on_event(|_, _, _| {}),
            ),
        )
        .into_any_element()
}

pub(super) fn transport(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(640.0))
        .child(
            TransportBar::new("scene.transport.playing")
                .label("Release walkthrough")
                .state(TransportState::Playing)
                .position(72.0)
                .duration(240.0)
                .elapsed("01:12")
                .remaining("-02:48")
                .buffered([BufferedRange::new(0.0, 156.0)])
                .volume(0.7)
                .speeds([1.0, 1.5, 2.0], 1.0)
                .step_seconds(10.0)
                .has_next(true)
                .on_event(|_, _, _| {}),
        )
        .child(
            Divider::new()
                .id("scene.transport.rule.live")
                .label("A stream nobody measured"),
        )
        .child(
            TransportBar::new("scene.transport.live")
                .label("Incident bridge")
                .state(TransportState::Playing)
                .position(1543.0)
                .unknown_duration()
                .elapsed("25:43")
                .volume(0.4)
                .on_event(|_, _, _| {}),
        )
        .child(
            Divider::new()
                .id("scene.transport.rule.stalled")
                .label("Playing and waiting"),
        )
        .child(
            TransportBar::new("scene.transport.stalled")
                .label("Release walkthrough")
                .state(TransportState::Buffering)
                .position(158.0)
                .duration(240.0)
                .elapsed("02:38")
                .remaining("-01:22")
                .buffered([BufferedRange::new(0.0, 160.0)])
                .volume(0.7)
                .muted(true)
                .on_event(|_, _, _| {}),
        )
        .into_any_element()
}

/// Every state a session can be in, and a grid drawing the output a screenshot
/// can actually check: the sixteen ANSI slots against their own background, an
/// explicit cell background, bold and dim and underline and inverse, a
/// selection wash over sixteen hues at once, and both colours a program picks
/// by number.
///
/// The script is ASCII throughout, which is a property of the harness rather
/// than of the component: the headless text system shapes from one bundled
/// font with no fallback, so a box-drawing character or a CJK glyph captures
/// as a missing-glyph box and the baseline would record the font's gap as the
/// terminal's. Wide characters and the column pinning that carries them are
/// held by the emulator's own tests, where the assertion is on cells rather
/// than on pixels.
///
/// The emulator is rebuilt from a fixed byte script each frame, so the picture
/// is a function of the bytes and the measured grid and of nothing else.
#[cfg(all(feature = "terminal", not(target_family = "wasm")))]
pub(super) fn terminal(_window: &mut Window, cx: &mut App) -> AnyElement {
    /// A session's worth of output, chosen so every branch of the painter has
    /// something to paint.
    const SESSION: &[u8] = concat!(
        "\x1b[32m>\x1b[0m cargo run -p xtask -- gate\r\n",
        "\x1b[1mCompiling\x1b[0m gpui-box-kit v0.1.2\r\n",
        "\x1b[2m    dim text stays legible and stays dim\x1b[0m\r\n",
        "\x1b[4munderlined\x1b[0m \x1b[7minverse\x1b[0m \x1b[44;97m on a background \x1b[0m\r\n",
        "\x1b[30m0\x1b[31m1\x1b[32m2\x1b[33m3\x1b[34m4\x1b[35m5\x1b[36m6\x1b[37m7",
        "\x1b[90m8\x1b[91m9\x1b[92ma\x1b[93mb\x1b[94mc\x1b[95md\x1b[96me\x1b[97mf\x1b[0m\r\n",
        "\x1b[38;5;208m256-colour cube\x1b[0m and \x1b[38;2;120;200;255mtruecolour\x1b[0m\r\n",
        "\x1b[32m>\x1b[0m ",
    )
    .as_bytes();

    let theme = cx.theme().clone();
    let panel = |ident: &'static str, state: TerminalState| {
        div()
            .w(px(400.0))
            .h(px(180.0))
            .child(Terminal::new(ident).state(state))
    };

    stack(&theme)
        .child(
            div().w(px(400.0)).h(px(180.0)).child(
                Terminal::new("scene.terminal.ready")
                    .focused(true)
                    .grid(|geometry, _| {
                        let mut emulator = Emulator::new(geometry.cols, geometry.rows);
                        emulator.feed(SESSION);
                        // A selection over the middle of a coloured row, so the
                        // achromatic wash is reviewable against sixteen hues at
                        // once rather than against one.
                        emulator.start_selection(
                            SelectionKind::Drag,
                            emulator.grid_point(4, 0),
                            CellSide::Left,
                        );
                        emulator.update_selection(emulator.grid_point(4, 15), CellSide::Right);
                        Some(GridSnapshot {
                            lines: emulator.lines(),
                            cursor: emulator.cursor(),
                        })
                    })
                    .on_event(|_, _, _| {}),
            ),
        )
        .child(
            row(&theme)
                .items_start()
                .child(panel("scene.terminal.starting", TerminalState::Loading))
                .child(panel(
                    "scene.terminal.unavailable",
                    TerminalState::Unavailable("No shell is configured for this workspace.".into()),
                )),
        )
        .child(
            div().w(px(400.0)).h(px(180.0)).child(
                Terminal::new("scene.terminal.ended")
                    .state(TerminalState::Error("The process exited with status 130.".into()))
                    .grid(|geometry, _| {
                        let mut emulator = Emulator::new(geometry.cols, geometry.rows);
                        emulator.feed(b"\x1b[32m>\x1b[0m tail -f build.log\r\n\x1b[31merror\x1b[0m: interrupted\r\n");
                        Some(GridSnapshot {
                            lines: emulator.lines(),
                            cursor: emulator.cursor(),
                        })
                    }),
            ),
        )
        .into_any_element()
}
