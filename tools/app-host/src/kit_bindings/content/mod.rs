//! Caller-owned content. Strings are content, never native IO authority.
use super::{Emit, KitSlots, Node, flag, number, text};
use anyhow::{Result, bail, ensure};
use gpui::{AnyElement, App, IntoElement, SharedString, Window};
use gpui_kit::{content::*, foundation::slot::Slotted};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::HashMap};

#[cfg(all(test, feature = "capture"))]
mod tests;

pub(super) const COMPONENTS: &[&str] = &[
    "Markdown",
    "CodeView",
    "AgentDocument",
    "LogStream",
    "MessageList",
    "Outline",
    "BrowserPanel",
    "TransportBar",
    "DiffView",
    "ImageViewer",
    "Terminal",
];

/// Builders retain native visual state through their stable Ident. This map holds
/// only mounted query targets; it does not duplicate the parser or selection state.
#[derive(Default)]
pub(super) struct State {
    mounted: RefCell<HashMap<(u64, String), Node>>,
}

impl State {
    pub(super) fn reconcile(&self, root: &Node, _cx: &mut App) {
        fn visit(node: &Node, live: &mut HashMap<(u64, String), String>) {
            if let Some(component) = &node.component {
                live.insert((node.instance, node.id.clone()), component.clone());
            }
            for child in node.children.iter().chain(node.slots.values().flatten()) {
                visit(child, live);
            }
        }
        let mut live = HashMap::new();
        visit(root, &mut live);
        self.mounted
            .borrow_mut()
            .retain(|key, node| live.get(key) == node.component.as_ref());
    }

    pub(super) fn render(
        &self,
        node: &Node,
        slots: KitSlots,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        self.mounted
            .borrow_mut()
            .insert((node.instance, node.id.clone()), node.clone());
        render(node, slots, window, cx, emit)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn invoke(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Value> {
        let mounted = self
            .mounted
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        ensure!(
            mounted.component == node.component,
            "native component mismatch"
        );
        if !query {
            ensure!(
                !flag(node, "disabled"),
                "disabled target refuses invocation"
            );
            ensure!(
                mounted.component.as_deref() == Some("AgentDocument")
                    && method == "remeasure_block",
                "unsupported content command"
            );
            ensure!(
                args.as_object().is_some_and(|args| args.len() == 1),
                "expected block argument"
            );
            let block = args["block"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("expected block identity"))?;
            ensure!(
                items(&props(&mounted), "blocks").any(|item| item["id"] == block),
                "unknown block identity"
            );
            AgentDocument::remeasure_block(
                &gpui_kit::foundation::Ident::new(mounted.id),
                block,
                window,
                cx,
            );
            return Ok(Value::Null);
        }
        ensure!(
            args.as_object().is_some_and(|args| args.is_empty()),
            "expected empty named arguments"
        );
        match (mounted.component.as_deref(), method) {
            (Some("CodeView"), "text") => Ok(json!(code_view(&mounted).text())),
            (Some("AgentDocument"), "duplicate_ids") => Ok(json!(
                document(&mounted, &KitSlots::new())
                    .duplicate_ids()
                    .iter()
                    .map(|id| id.as_ref())
                    .collect::<Vec<_>>()
            )),
            (Some("AgentDocument"), "work") => {
                let work =
                    AgentDocument::work(&gpui_kit::foundation::Ident::new(mounted.id), window, cx);
                Ok(
                    json!({"input_checks":work.input_checks,"planned_rows":work.planned_rows,"parser":{"parser_passes":work.parser.parser_passes,"parsed_bytes":work.parser.parsed_bytes,"copied_bytes":work.parser.copied_bytes}}),
                )
            }
            _ => bail!("unsupported content method"),
        }
    }
}

pub(super) fn slotted<T: Slotted>(mut control: T, slots: KitSlots) -> T {
    for &name in T::SLOTS {
        if let Some(build) = slots.get(name).cloned() {
            control = control.slot(name, move |window, cx| build(window, cx));
        }
    }
    control
}

fn items<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}
fn s(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}
fn b(value: &Value, key: &str) -> bool {
    value[key].as_bool().unwrap_or(false)
}
fn n(value: &Value, key: &str) -> usize {
    value[key].as_u64().unwrap_or_default() as usize
}
fn props(node: &Node) -> Value {
    Value::Object(node.props.clone())
}
fn tone(value: &Value) -> gpui_kit::display::badge::Tone {
    use gpui_kit::display::badge::Tone;
    match value.as_str().unwrap_or_default() {
        "accent" => Tone::Accent,
        "success" => Tone::Success,
        "warning" => Tone::Warning,
        "danger" => Tone::Danger,
        "info" => Tone::Info,
        _ => Tone::Neutral,
    }
}
fn spans(value: &Value, key: &str) -> Vec<CodeSpan> {
    use gpui_kit_theme::SyntaxColor;
    items(value, key)
        .map(|span| CodeSpan {
            range: n(span, "start")..n(span, "end"),
            role: match span["role"].as_str().unwrap_or_default() {
                "keyword" => SyntaxColor::Keyword,
                "string" => SyntaxColor::StringLiteral,
                "comment" => SyntaxColor::Comment,
                "number" => SyntaxColor::Number,
                "inlineWash" => SyntaxColor::InlineWash,
                "added" => SyntaxColor::Added,
                "addedWash" => SyntaxColor::AddedWash,
                "removed" => SyntaxColor::Removed,
                "removedWash" => SyntaxColor::RemovedWash,
                _ => SyntaxColor::Inline,
            },
        })
        .collect()
}
fn send(node: &Node, emit: &Emit, event: &str, value: Value) {
    if !flag(node, "disabled")
        && let Some(action) = node.events.get(event)
    {
        emit(action, value);
    }
}

fn markdown_event(event: &MarkdownEvent) -> Value {
    match event {
        MarkdownEvent::LinkClicked { href } => json!({"kind":"linkClicked","href":href.as_ref()}),
        MarkdownEvent::ImageRequested { src, alt } => {
            json!({"kind":"imageRequested","src":src.as_ref(),"alt":alt.as_ref()})
        }
        MarkdownEvent::CodeCopied { language, text } => {
            json!({"kind":"codeCopied","language":language.as_ref().map(|s|s.as_ref()),"text":text.as_ref()})
        }
        MarkdownEvent::CodeCopyRefused { language, reason } => {
            json!({"kind":"codeCopyRefused","language":language.as_ref().map(|s|s.as_ref()),"reason":match reason { gpui::ClipboardDenied::MissingOwner => "missingOwner", gpui::ClipboardDenied::Denied => "denied" }})
        }
        MarkdownEvent::MoreRequested { lines } => json!({"kind":"moreRequested","lines":lines}),
    }
}

fn markdown(node: &Node) -> Markdown {
    let mut control = Markdown::new(node.id.clone(), text(node, "source"))
        .streaming(flag(node, "streaming"))
        .code_presentation(if text(node, "codePresentation") == "card" {
            MarkdownCodePresentation::Card
        } else {
            MarkdownCodePresentation::Flat
        });
    if let Some(lines) = node.props.get("maxLines").and_then(Value::as_u64) {
        control = control.max_lines(lines as usize);
    }
    if let Some(order) = node
        .props
        .get("selectionOrderStart")
        .and_then(Value::as_u64)
    {
        control = control.selection_order_start(order);
    }
    control
}

fn code_view(node: &Node) -> CodeView {
    let mut control = if let Some(lines) = node.props.get("lines").and_then(Value::as_array) {
        CodeView::new(
            node.id.clone(),
            lines.iter().map(|line| {
                let mut result =
                    CodeLine::new(n(line, "number"), s(line, "text")).spans(spans(line, "spans"));
                if let Some(mark) = line["mark"].as_str() {
                    result = result.mark(match mark {
                        "added" => LineMark::Added,
                        "removed" => LineMark::Removed,
                        "changed" => LineMark::Changed,
                        "error" => LineMark::Error,
                        _ => LineMark::Highlighted,
                    });
                }
                result
            }),
        )
    } else {
        CodeView::from_text(node.id.clone(), &text(node, "text"))
    };
    control = control
        .line_numbers(node.props.get("lineNumbers") != Some(&Value::Bool(false)))
        .copyable(node.props.get("copyable") != Some(&Value::Bool(false)));
    if node.props.contains_key("language") {
        control = control.language(text(node, "language"));
    }
    if let Some(lines) = node.props.get("visibleLines").and_then(Value::as_u64) {
        control = control.visible_lines(lines as usize);
    }
    control
}

fn document(node: &Node, slots: &KitSlots) -> AgentDocument {
    let data = props(node);
    let reason = text(node, "reason").into();
    let mut document =
        AgentDocument::new(node.id.clone()).state(match text(node, "state").as_str() {
            "idle" => AgentDocumentState::Idle(reason),
            "loading" => AgentDocumentState::Loading(reason),
            "empty" => AgentDocumentState::Empty(reason),
            "unavailable" => AgentDocumentState::Unavailable(reason),
            "failed" => AgentDocumentState::Failed(reason),
            _ => AgentDocumentState::Ready,
        });
    for block in items(&data, "blocks") {
        let id = s(block, "id");
        let mut entry = match block["kind"].as_str() {
            Some("markdown") => AgentDocumentBlock::markdown(id, s(block, "text")),
            Some("text") => AgentDocumentBlock::text(id, s(block, "text")),
            Some("notice") if !slots.contains_key(&id) => {
                AgentDocumentBlock::notice(id, s(block, "text"), tone(&block["tone"]))
            }
            _ => {
                let Some(build) = slots.get(&id).cloned() else {
                    continue;
                };
                let kind = match block["kind"].as_str().unwrap_or_default() {
                    "code" => AgentBlockKind::Code,
                    "tool-call" => AgentBlockKind::ToolCall,
                    "diff" => AgentBlockKind::Diff,
                    "artifact" => AgentBlockKind::Artifact,
                    "schema" => AgentBlockKind::Schema,
                    "chart" => AgentBlockKind::Chart,
                    "image" => AgentBlockKind::Image,
                    "notice" => AgentBlockKind::Notice,
                    "choice" => AgentBlockKind::Choice,
                    _ => AgentBlockKind::Custom,
                };
                AgentDocumentBlock::element(id, kind, move |window, cx| build(window, cx))
            }
        }
        .revision(n(block, "revision") as u64)
        .streaming(b(block, "streaming"));
        if block.get("label").is_some() {
            entry = entry.label(s(block, "label"));
        }
        document = document.block(entry);
    }
    if let Some(rows) = node.props.get("virtualized").and_then(Value::as_u64) {
        document = document.virtualized(rows as usize);
    }
    document
}

pub(super) fn render(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
) -> AnyElement {
    let callback = node.clone();
    let data = props(node);
    match node.component.as_deref().unwrap_or_default() {
        "Markdown" => markdown(node)
            .on_event(move |event, _, _| send(&callback, &emit, "event", markdown_event(event)))
            .into_any_element(),
        "CodeView" => slotted(code_view(node), slots).into_any_element(),
        "DiffView" => {
            let files = items(&data, "files").map(|file| {
                let hunks = items(file, "hunks").map(|hunk| {
                    DiffHunk::new(
                        s(hunk, "id"),
                        s(hunk, "header"),
                        items(hunk, "lines").map(|line| {
                            let mut result = match line["kind"].as_str() {
                                Some("added") => DiffLine::added(s(line, "id"), s(line, "text")),
                                Some("removed") => {
                                    DiffLine::removed(s(line, "id"), s(line, "text"))
                                }
                                Some("paired") => {
                                    DiffLine::paired(s(line, "id"), s(line, "old"), s(line, "text"))
                                }
                                _ => DiffLine::new(s(line, "id"), s(line, "text")),
                            };
                            if line.get("oldNumber").is_some() {
                                result = result.old_number(n(line, "oldNumber"));
                            }
                            if line.get("newNumber").is_some() {
                                result = result.new_number(n(line, "newNumber"));
                            }
                            if line.get("spans").is_some() {
                                result = result.spans(spans(line, "spans"));
                            }
                            if line.get("oldSpans").is_some() {
                                result = result.old_spans(spans(line, "oldSpans"));
                            }
                            if line.get("newSpans").is_some() {
                                result = result.new_spans(spans(line, "newSpans"));
                            }
                            result
                        }),
                    )
                    .collapsed(b(hunk, "collapsed"))
                });
                let mut result =
                    DiffFile::new(s(file, "id"), s(file, "label"), hunks).folded(b(file, "folded"));
                if file.get("language").is_some() {
                    result = result.language(s(file, "language"));
                }
                result = result.notes(items(file, "notes").map(
                    |note| match note["kind"].as_str() {
                        Some("removed") => DiffNote::Removed,
                        Some("binary") => DiffNote::Binary,
                        Some("renamed") => DiffNote::Renamed {
                            from: s(note, "from").into(),
                        },
                        Some("mode") => DiffNote::Mode {
                            to: s(note, "to").into(),
                        },
                        _ => DiffNote::Added,
                    },
                ));
                result
            });
            let mut control = DiffView::new(node.id.clone(), files)
                .wrapping(flag(node, "wrapping"))
                .presentation(if text(node, "presentation") == "split" {
                    DiffPresentation::Split
                } else {
                    DiffPresentation::Unified
                });
            if let Some(cursor) = data.get("cursor") {
                control = control.cursor(if cursor.get("lineId").is_some() {
                    DiffCursor::Line {
                        file_id: s(cursor, "fileId").into(),
                        hunk_id: s(cursor, "hunkId").into(),
                        line_id: s(cursor, "lineId").into(),
                    }
                } else if cursor.get("hunkId").is_some() {
                    DiffCursor::Hunk {
                        file_id: s(cursor, "fileId").into(),
                        hunk_id: s(cursor, "hunkId").into(),
                    }
                } else {
                    DiffCursor::File {
                        file_id: s(cursor, "fileId").into(),
                    }
                });
            }
            if flag(node, "fills") {
                control = control.fills();
            }
            if node.props.contains_key("language") {
                control = control.language(text(node, "language"));
            }
            if let Some(rows) = node.props.get("visibleRows").and_then(Value::as_u64) {
                control = control.visible_rows(rows as usize);
            }
            if node.events.contains_key("event") {
                control=control.on_event(move |event,_,_|send(&callback,&emit,"event",match event {
                DiffViewEvent::FileActivated{file_id}=>json!({"kind":"fileActivated","fileId":file_id.as_ref()}),
                DiffViewEvent::UnfoldFile{file_id}=>json!({"kind":"unfoldFile","fileId":file_id.as_ref()}),
                DiffViewEvent::HunkActivated{file_id,hunk_id}=>json!({"kind":"hunkActivated","fileId":file_id.as_ref(),"hunkId":hunk_id.as_ref()}),
                DiffViewEvent::ExpandHunk{file_id,hunk_id}=>json!({"kind":"expandHunk","fileId":file_id.as_ref(),"hunkId":hunk_id.as_ref()}),
                DiffViewEvent::LineActivated{file_id,hunk_id,line_id}=>json!({"kind":"lineActivated","fileId":file_id.as_ref(),"hunkId":hunk_id.as_ref(),"lineId":line_id.as_ref()}),
            }));
            }
            slotted(control, slots).into_any_element()
        }
        "ImageViewer" => {
            let frames = items(&data, "frames").map(|frame| {
                let mut result = ImageFrame::new(s(frame, "id"), s(frame, "label"));
                if frame.get("width").is_some() {
                    result = result.natural(n(frame, "width") as u32, n(frame, "height") as u32);
                }
                result.state(match frame["state"].as_str() {
                    Some("loading") => ImageState::Loading,
                    Some("error") => ImageState::Failed(s(frame, "reason").into()),
                    Some("ready") if slots.contains_key(&s(frame, "id")) => ImageState::Ready,
                    _ => ImageState::Unavailable(
                        "Native image resource authority unavailable".into(),
                    ),
                })
            });
            let mut control =
                ImageViewer::new(node.id.clone(), frames).fit(match text(node, "fit").as_str() {
                    "cover" => FitMode::Cover,
                    "actual" => FitMode::Actual,
                    "zoom" => FitMode::Zoom(number(node, "zoom", 1.)),
                    _ => FitMode::Contain,
                });
            if node.props.contains_key("showing") {
                control = control.showing(text(node, "showing"));
            }
            if node.props.contains_key("height") {
                control = control.height(number(node, "height", 240.));
            }
            if node.props.contains_key("minZoom") || node.props.contains_key("maxZoom") {
                control =
                    control.zoom_range(number(node, "minZoom", 0.1), number(node, "maxZoom", 10.));
            }
            control = control.image(move |frame, window, cx| {
                slots
                    .get(frame.id().as_ref())
                    .map(|build| build(window, cx))
            });
            if node.events.contains_key("event") {
                control=control.on_event(move |event,_,_|send(&callback,&emit,"event",match event {
                ImageViewerEvent::FitChanged(fit)=>json!({"kind":"fitChanged","fit":fit.name(),"zoom":if let FitMode::Zoom(value)=fit{Some(*value)}else{None}}),
                ImageViewerEvent::Stepped{id}=>json!({"kind":"stepped","id":id.as_ref()}),
                ImageViewerEvent::ImageRequested(request)=>json!({"kind":"imageRequested","id":request.id.as_ref(),"label":request.label.as_ref()}),
            }));
            }
            control.into_any_element()
        }
        "Terminal" => {
            let mut control = Terminal::new(node.id.clone())
                .focused(flag(node, "focused"))
                .scrollback(flag(node, "scrollback"));
            let has_data = node.props.contains_key("text");
            let state = match text(node, "state").as_str() {
                "loading" => TerminalState::Loading,
                "error" => TerminalState::Error(text(node, "reason").into()),
                _ if has_data => TerminalState::Ready,
                _ => TerminalState::Unavailable(
                    "Native terminal process authority unavailable".into(),
                ),
            };
            control = control.state(state);
            if has_data {
                let text = text(node, "text");
                // The emulator is a pure ANSI fold. Replies are intentionally not sent
                // to a process; this adapter has no process or PTY handle.
                control = control.grid(move |geometry, _| {
                    let mut emulator =
                        Emulator::new(geometry.cols.min(512), geometry.rows.min(256));
                    emulator.feed(text.as_bytes());
                    Some(GridSnapshot {
                        lines: emulator.lines(),
                        cursor: emulator.cursor(),
                    })
                });
            }
            if node.events.contains_key("event") {
                control=control.on_event(move |event,_,_| {
                let hit=|hit:CellHit|json!({"row":hit.row,"col":hit.col,"side":match hit.side{CellSide::Left=>"left",CellSide::Right=>"right"}});
                send(&callback,&emit,"event",match event {
                    TerminalEvent::SelectionStarted{hit:cell,kind}=>json!({"kind":"selectionStarted","hit":hit(cell),"selectionKind":match kind{SelectionKind::Drag=>"drag",SelectionKind::Word=>"word",SelectionKind::Line=>"line"}}),
                    TerminalEvent::SelectionUpdated{hit:cell}=>json!({"kind":"selectionUpdated","hit":hit(cell)}),
                    TerminalEvent::SelectionCleared=>json!({"kind":"selectionCleared"}),
                    TerminalEvent::Scrolled{lines}=>json!({"kind":"scrolled","lines":lines}),
                });
            });
            }
            slotted(control, slots).into_any_element()
        }
        "AgentDocument" => {
            let control = document(node, &slots).on_event(move |event, _, _| {
                let AgentDocumentEvent::Markdown { block, event } = event;
                send(
                    &callback,
                    &emit,
                    "markdown",
                    json!({"blockId":block.as_ref(),"event":markdown_event(event)}),
                );
            });
            slotted(control, slots).into_any_element()
        }
        "Outline" => {
            let mut control =
                Outline::new(node.id.clone()).marks(items(&data, "marks").map(|mark| {
                    let mut mark_value = Mark::new(s(mark, "id"), n(mark, "row"), s(mark, "title"));
                    if mark.get("detail").is_some() {
                        mark_value = mark_value.detail(s(mark, "detail"));
                    }
                    mark_value
                }));
            if node.props.contains_key("over") {
                control = control.over(text(node, "over"));
            }
            if let Some(count) = node.props.get("slots").and_then(Value::as_u64) {
                control = control.slots(count as usize);
            }
            if node.events.contains_key("select") {
                control = control.on_select(move |id, _, _| {
                    send(&callback, &emit, "select", json!(id.as_ref()))
                });
            }
            control.into_any_element()
        }
        "BrowserPanel" => {
            // URL is address-bar text only. No WebView is constructed here.
            let reason: SharedString = text(node, "reason").into();
            let state = match text(node, "state").as_str() {
                "loading" => ViewportState::Loading,
                "empty" => ViewportState::Empty,
                "error" => ViewportState::Error(reason),
                "ready" if slots.contains_key("viewport") => ViewportState::Ready,
                _ => ViewportState::Unavailable(
                    "Native browser resource authority unavailable".into(),
                ),
            };
            let mut control = BrowserPanel::new(node.id.clone())
                .url(text(node, "url"))
                .state(state);
            if let Some(build) = slots.get("viewport") {
                control = control.viewport(build(window, cx));
            }
            if node.events.contains_key("back") {
                let node = node.clone();
                let emit = emit.clone();
                control = control.on_back(move |_, _| send(&node, &emit, "back", Value::Null));
            }
            if node.events.contains_key("forward") {
                let node = node.clone();
                let emit = emit.clone();
                control =
                    control.on_forward(move |_, _| send(&node, &emit, "forward", Value::Null));
            }
            if node.events.contains_key("reload") {
                control =
                    control.on_reload(move |_, _| send(&callback, &emit, "reload", Value::Null));
            }
            slotted(control, slots).into_any_element()
        }
        "LogStream" => {
            let entries = items(&data, "entries").map(|entry| {
                let mut result = LogEntry::new(s(entry, "id"), s(entry, "message"))
                    .timestamp(s(entry, "timestamp"))
                    .source(s(entry, "source"))
                    .level(s(entry, "level"), tone(&entry["tone"]))
                    .search_hits(
                        items(entry, "searchHits").map(|hit| n(hit, "start")..n(hit, "end")),
                    );
                if entry.get("currentHit").is_some() {
                    result = result.current_hit(n(entry, "currentHit"));
                }
                result
            });
            let reason = text(node, "reason").into();
            let mut control = LogStream::new(node.id.clone(), entries)
                .ansi(flag(node, "ansi"))
                .state(match text(node, "state").as_str() {
                    "loading" => LogStreamState::Loading,
                    "empty" => LogStreamState::Empty,
                    "unavailable" => LogStreamState::Unavailable(reason),
                    "error" => LogStreamState::Error(reason),
                    "stale" => LogStreamState::Stale(reason),
                    _ => LogStreamState::Ready,
                });
            if let Some(rows) = node.props.get("visibleRows").and_then(Value::as_u64) {
                control = control.visible_rows(rows as usize);
            }
            if node.props.contains_key("selected") {
                control = control.selected(text(node, "selected"));
            }
            if node.events.contains_key("select") {
                let node = node.clone();
                let emit = emit.clone();
                control = control
                    .on_select(move |id, _, _| send(&node, &emit, "select", json!(id.as_ref())));
            }
            if node.events.contains_key("copy") {
                control = control
                    .on_copy(move |id, _, _| send(&callback, &emit, "copy", json!(id.as_ref())));
            }
            slotted(control, slots).into_any_element()
        }
        "MessageList" => {
            let messages = items(&data, "messages").map(|message| {
                let body = if b(message, "markdown") {
                    MessageBody::Markdown(s(message, "text").into())
                } else {
                    MessageBody::Text(s(message, "text").into())
                };
                let mut result =
                    Message::new(s(message, "id"), body).streaming(b(message, "streaming"));
                if message.get("author").is_some() {
                    result = result.author(s(message, "author"));
                }
                if message.get("time").is_some() {
                    result = result.time(s(message, "time"));
                }
                for attachment in items(message, "attachments") {
                    let mut value = Attachment::new(s(attachment, "id"), s(attachment, "name"));
                    if attachment.get("detail").is_some() {
                        value = value.detail(s(attachment, "detail"));
                    }
                    result = result.attachment(value);
                }
                for reaction in items(message, "reactions") {
                    result = result.reaction(Reaction::new(
                        s(reaction, "id"),
                        s(reaction, "label"),
                        n(reaction, "count"),
                    ));
                }
                result.delivery(match message["delivery"].as_str().unwrap_or_default() {
                    "sending" => DeliveryState::Sending,
                    "delivered" => DeliveryState::Delivered,
                    "read" => DeliveryState::Read,
                    "failed" => DeliveryState::Failed {
                        reason: s(message, "reason").into(),
                    },
                    _ => DeliveryState::Sent,
                })
            });
            let mut control = MessageList::new(node.id.clone(), messages)
                .group_consecutive(flag(node, "groupConsecutive"));
            if let Some(rows) = node.props.get("visibleRows").and_then(Value::as_u64) {
                control = control.visible_rows(rows as usize);
            }
            if let Some(lines) = node.props.get("bodyLines").and_then(Value::as_u64) {
                control = control.body_lines(lines as usize);
            }
            if flag(node, "growsToFit") {
                control = control.grows_to_fit();
            }
            if node.events.contains_key("retry") {
                let node = node.clone();
                let emit = emit.clone();
                control = control
                    .on_retry(move |id, _, _| send(&node, &emit, "retry", json!(id.as_ref())));
            }
            control
                .on_markdown(move |id, event, _, _| {
                    send(
                        &callback,
                        &emit,
                        "markdown",
                        json!({"messageId":id.as_ref(),"event":markdown_event(event)}),
                    )
                })
                .into_any_element()
        }
        "TransportBar" => {
            let mut control = TransportBar::new(node.id.clone())
                .label(text(node, "label"))
                .state(match text(node, "state").as_str() {
                    "playing" => TransportState::Playing,
                    "buffering" => TransportState::Buffering,
                    _ => TransportState::Paused,
                })
                .position(number(node, "position", 0.))
                .volume(number(node, "volume", 1.))
                .muted(flag(node, "muted"))
                .seekable(node.props.get("seekable") != Some(&Value::Bool(false)))
                .volume_control(node.props.get("volumeControl") != Some(&Value::Bool(false)))
                .has_previous(flag(node, "hasPrevious"))
                .has_next(flag(node, "hasNext"));
            if let Some(duration) = node.props.get("duration").and_then(Value::as_f64) {
                control = control.duration(duration as f32);
            } else {
                control = control.unknown_duration();
            }
            if node.props.contains_key("elapsed") {
                control = control.elapsed(text(node, "elapsed"));
            }
            if node.props.contains_key("remaining") {
                control = control.remaining(text(node, "remaining"));
            }
            if node.props.contains_key("stepSeconds") {
                control = control.step_seconds(number(node, "stepSeconds", 5.));
            }
            if node.props.contains_key("speeds") {
                control = control.speeds(
                    items(&data, "speeds")
                        .filter_map(Value::as_f64)
                        .map(|v| v as f32),
                    number(node, "speed", 1.),
                );
            }
            control = control.buffered(items(&data, "buffered").map(|range| {
                BufferedRange::new(
                    range["start"].as_f64().unwrap_or_default() as f32,
                    range["end"].as_f64().unwrap_or_default() as f32,
                )
            }));
            control
                .on_event(move |event, _, _| {
                    let value = match event {
                        TransportEvent::PlayRequested => json!({"kind":"playRequested"}),
                        TransportEvent::PauseRequested => json!({"kind":"pauseRequested"}),
                        TransportEvent::MuteToggled => json!({"kind":"muteToggled"}),
                        TransportEvent::SeekPreview(value) => {
                            json!({"kind":"seekPreview","value":value})
                        }
                        TransportEvent::SeekRequested(value) => {
                            json!({"kind":"seekRequested","value":value})
                        }
                        TransportEvent::VolumeRequested(value) => {
                            json!({"kind":"volumeRequested","value":value})
                        }
                        TransportEvent::SpeedRequested(value) => {
                            json!({"kind":"speedRequested","value":value})
                        }
                        TransportEvent::Stepped(step) => {
                            json!({"kind":"stepped","step":step.name()})
                        }
                    };
                    send(&callback, &emit, "event", value);
                })
                .into_any_element()
        }
        _ => unreachable!("validated content registration"),
    }
}
