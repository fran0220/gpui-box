//! Caller-owned content. Strings are content, never native IO authority.
use super::{Emit, KitSlots, Node, flag, number, text};
use crate::resources::{ResourceRef, Resources};
use anyhow::{Result, bail, ensure};
use gpui::{AnyElement, App, IntoElement, SharedString, Styled, StyledImage, Window};
use gpui_kit::{
    content::*,
    foundation::{Disableable, slot::Slotted},
};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

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

pub(super) fn validate_descriptor(node: &Node) -> Result<()> {
    let data = props(node);
    for image in items(&data, "images")
        .chain(items(&data["markdownOptions"], "images"))
        .chain(items(&data, "frames"))
    {
        if let Some(reference) = image.get("resource") {
            validate_resource(reference)?;
        }
    }
    if node.component.as_deref() == Some("CodeView") {
        ensure!(
            !(node.props.contains_key("text") && node.props.contains_key("lines")),
            "choose code text or lines"
        );
        let mut numbers = std::collections::HashSet::new();
        for line in items(&data, "lines") {
            ensure!(
                numbers.insert(n(line, "number")),
                "duplicate code line number"
            );
        }
    }
    if node.component.as_deref() == Some("AgentDocument") {
        for block in items(&data, "blocks") {
            match block["kind"].as_str().unwrap_or_default() {
                "text" | "markdown" => {
                    ensure!(block.get("text").is_some(), "document block text required")
                }
                "notice" => ensure!(
                    block.get("text").is_some() || node.slots.contains_key(&s(block, "id")),
                    "notice text or slot required"
                ),
                _ => ensure!(
                    node.slots.contains_key(&s(block, "id")),
                    "typed document block requires slot"
                ),
            }
        }
    }
    if node.component.as_deref() == Some("ImageViewer") {
        ensure!(
            number(node, "minZoom", 0.1) <= number(node, "maxZoom", 10.),
            "invalid image zoom range"
        );
    }
    Ok(())
}

pub(super) fn validate_resource(value: &Value) -> Result<()> {
    let reference: ResourceRef = serde_json::from_value(value.clone())?;
    ensure!(
        !reference.key.is_empty()
            && reference.key.len() <= 128
            && reference
                .key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "invalid resource key"
    );
    Ok(())
}

type TerminalStates = HashMap<(u64, String), Rc<RefCell<TerminalData>>>;

/// Builders retain native visual state through their stable Ident. The runtime
/// namespaces it per mount. This state owns query targets and pure ANSI emulators;
/// dropping it does not claim immediate destruction of framework keyed caches.
#[derive(Default)]
pub(super) struct State {
    mounted: RefCell<HashMap<(u64, String), Node>>,
    terminals: RefCell<TerminalStates>,
}

struct TerminalData {
    source: Option<String>,
    emulator: Emulator,
}
impl Default for TerminalData {
    fn default() -> Self {
        Self {
            source: None,
            emulator: Emulator::new(80, 24),
        }
    }
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
        self.terminals.borrow_mut().retain(|key, _| {
            live.get(key)
                .is_some_and(|component| component == "Terminal")
        });
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
        if node.component.as_deref() == Some("Terminal") {
            let retained = self
                .terminals
                .borrow_mut()
                .entry((node.instance, node.id.clone()))
                .or_default()
                .clone();
            return terminal(node, slots, retained, emit);
        }
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
pub(super) fn spans(value: &Value, key: &str) -> Vec<CodeSpan> {
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

pub(super) fn markdown_event(event: &MarkdownEvent) -> Value {
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
    configure_markdown(
        Markdown::new(node.id.clone(), text(node, "source")).streaming(flag(node, "streaming")),
        &props(node),
    )
}

pub(super) fn configure_markdown(mut control: Markdown, options: &Value) -> Markdown {
    if options.get("codePresentation").is_some() {
        control = control.code_presentation(if s(options, "codePresentation") == "card" {
            MarkdownCodePresentation::Card
        } else {
            MarkdownCodePresentation::Flat
        });
    }
    if let Some(lines) = options.get("maxLines").and_then(Value::as_u64) {
        control = control.max_lines(lines as usize);
    }
    if let Some(order) = options.get("selectionOrderStart").and_then(Value::as_u64) {
        control = control.selection_order_start(order);
    }
    if options.get("highlights").is_some() {
        let highlights = items(options, "highlights").cloned().collect::<Vec<_>>();
        control = control.highlight(move |code| {
            highlights
                .iter()
                .find(|item| {
                    item["text"].as_str() == Some(code.text.as_ref())
                        && item["language"].as_str() == code.language.as_deref()
                })
                .map_or_else(Vec::new, |item| spans(item, "spans"))
        });
    }
    if options.get("images").is_some() {
        let images = items(options, "images").cloned().collect::<Vec<_>>();
        control = control.image(move |request, _, cx| {
            images
                .iter()
                .find(|image| image["src"].as_str() == Some(request.src.as_ref()))
                .and_then(|image| image_resource(&image["resource"], cx).ok())
                .map(IntoElement::into_any_element)
        });
    }
    control
}

/// Resolve at the point of use; the Custom loader also checks revocation during
/// native image layout/paint. Never cache the resulting pixels in adapter state.
pub(super) fn image_resource(reference: &Value, cx: &App) -> Result<gpui::Img> {
    let reference: ResourceRef = serde_json::from_value(reference.clone())?;
    Ok(gpui::img(Resources::image(&reference, cx)?).object_fit(gpui::ObjectFit::Contain))
}

pub(super) fn message_body(value: &Value) -> MessageBody {
    if b(value, "markdown") {
        MessageBody::Markdown(s(value, "text").into())
    } else {
        MessageBody::Text(s(value, "text").into())
    }
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
    if let Some(options) = node.props.get("markdownOptions").cloned() {
        document =
            document.configure_markdown(move |_, markdown| configure_markdown(markdown, &options));
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
                    Some("ready")
                        if slots.contains_key(&s(frame, "id"))
                            || frame
                                .get("resource")
                                .is_some_and(|reference| image_resource(reference, cx).is_ok()) =>
                    {
                        ImageState::Ready
                    }
                    _ => ImageState::Unavailable("Image resource refused or unavailable".into()),
                })
            });
            let mut control = ImageViewer::new(node.id.clone(), frames)
                .disabled(flag(node, "disabled"))
                .fit(match text(node, "fit").as_str() {
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
                let slot = slots
                    .get(frame.id().as_ref())
                    .map(|build| build(window, cx));
                slot.or_else(|| {
                    items(&data, "frames")
                        .find(|item| item["id"].as_str() == Some(frame.id().as_ref()))
                        .and_then(|item| image_resource(&item["resource"], cx).ok())
                        .map(|image| image.size_full().into_any_element())
                })
            });
            if !flag(node, "disabled") && node.events.contains_key("event") {
                control=control.on_event(move |event,_,_|send(&callback,&emit,"event",match event {
                ImageViewerEvent::FitChanged(fit)=>json!({"kind":"fitChanged","fit":fit.name(),"zoom":if let FitMode::Zoom(value)=fit{Some(*value)}else{None}}),
                ImageViewerEvent::Stepped{id}=>json!({"kind":"stepped","id":id.as_ref()}),
                ImageViewerEvent::ImageRequested(request)=>json!({"kind":"imageRequested","id":request.id.as_ref(),"label":request.label.as_ref()}),
            }));
            }
            control.into_any_element()
        }
        "Terminal" => terminal(node, slots, Rc::default(), emit),
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
                let body = message_body(message);
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
                .disabled(flag(node, "disabled"))
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
            if !flag(node, "disabled") && node.events.contains_key("event") {
                control = control.on_event(move |event, _, _| {
                    let value = match event {
                        TransportEvent::PlayRequested => json!({"kind":"playRequested"}),
                        TransportEvent::PauseRequested => json!({"kind":"pauseRequested"}),
                        TransportEvent::MuteToggled => json!({"kind":"muteToggled"}),
                        TransportEvent::SeekPreview(value) => {
                            json!({"kind":"seekPreview","value":value})
                        }
                        TransportEvent::SeekCancelled => json!({"kind":"seekCancelled"}),
                        TransportEvent::PresentationRequested(state) => {
                            json!({"kind":"presentationRequested","state":state.name()})
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
                });
            }
            control.into_any_element()
        }
        _ => unreachable!("validated content registration"),
    }
}

fn terminal(
    node: &Node,
    slots: KitSlots,
    retained: Rc<RefCell<TerminalData>>,
    emit: Emit,
) -> AnyElement {
    let has_data = node.props.contains_key("text") || retained.borrow().source.is_some();
    let state = match text(node, "state").as_str() {
        "loading" => TerminalState::Loading,
        "error" => TerminalState::Error(text(node, "reason").into()),
        "unavailable" => TerminalState::Unavailable(text(node, "reason").into()),
        _ if has_data => TerminalState::Ready,
        _ => TerminalState::Unavailable("Native terminal process authority unavailable".into()),
    };
    let mut control = Terminal::new(node.id.clone())
        .state(state)
        .focused(flag(node, "focused"))
        .scrollback(flag(node, "scrollback"));
    if has_data {
        let source = node
            .props
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let held = retained.clone();
        control = control.grid(move |geometry, _| {
            let mut data = held.borrow_mut();
            let cols = geometry.cols.clamp(1, 512);
            let rows = geometry.rows.clamp(1, 256);
            if data.emulator.cols() != usize::from(cols)
                || data.emulator.rows() != usize::from(rows)
            {
                data.emulator.resize(cols, rows);
            }
            if let Some(source) = &source
                && data.source.as_ref() != Some(source)
            {
                let prefix = data
                    .source
                    .as_ref()
                    .filter(|previous| source.starts_with(previous.as_str()))
                    .map(String::len);
                if let Some(prefix) = prefix {
                    data.emulator.feed(&source.as_bytes()[prefix..]);
                } else {
                    data.emulator = Emulator::new(cols, rows);
                    data.emulator.feed(source.as_bytes());
                }
                data.source = Some(source.clone());
            }
            Some(GridSnapshot {
                lines: data.emulator.lines(),
                cursor: data.emulator.cursor(),
            })
        });
        let node = node.clone();
        control=control.on_event(move |event,window,_|{
            let hit=|hit:CellHit|json!({"row":hit.row,"col":hit.col,"side":match hit.side{CellSide::Left=>"left",CellSide::Right=>"right"}});
            let payload={
                let mut data=retained.borrow_mut();
                match event {
                    TerminalEvent::SelectionStarted{hit:cell,kind}=>{
                        let point=data.emulator.grid_point(cell.row,cell.col);
                        data.emulator.start_selection(kind,point,cell.side);
                        json!({"kind":"selectionStarted","hit":hit(cell),"selectionKind":match kind{SelectionKind::Drag=>"drag",SelectionKind::Word=>"word",SelectionKind::Line=>"line"}})
                    }
                    TerminalEvent::SelectionUpdated{hit:cell}=>{
                        let point=data.emulator.grid_point(cell.row,cell.col);
                        data.emulator.update_selection(point,cell.side);
                        json!({"kind":"selectionUpdated","hit":hit(cell)})
                    }
                    TerminalEvent::SelectionCleared=>{data.emulator.clear_selection();json!({"kind":"selectionCleared"})}
                    TerminalEvent::Scrolled{lines}=>{data.emulator.scroll(lines);json!({"kind":"scrolled","lines":lines})}
                }
            };
            send(&node,&emit,"event",payload);
            window.refresh();
        });
    }
    slotted(control, slots).into_any_element()
}
