use super::*;
use gpui_kit::controls::rich_text_editor::{RichTextEditor, RichTextEditorEvent};
use gpui_kit::controls::textarea::Frame;

#[derive(Default)]
struct BlockIds {
    issued: std::collections::HashSet<String>,
    next: u64,
}
impl BlockIds {
    fn reserve(&mut self, document: &RichTextDocument) {
        self.issued
            .extend(document.blocks().iter().map(|b| b.id().as_str().to_owned()));
    }
    fn mint(&mut self) -> RichTextBlockId {
        loop {
            self.next += 1;
            let id = format!("host-block-{}", self.next);
            if self.issued.insert(id.clone()) {
                return RichTextBlockId::new(id);
            }
        }
    }
    fn intent(&mut self, intent: &RichTextIntent) {
        match intent {
            RichTextIntent::InsertHardBreak { new_block } => {
                self.issued.insert(new_block.as_str().to_owned());
            }
            RichTextIntent::ReplaceMultiline { new_blocks, .. } => self
                .issued
                .extend(new_blocks.iter().map(|id| id.as_str().to_owned())),
            _ => {}
        }
    }
}
pub(super) struct RichEntry {
    pub(super) entry: Entry<RichTextEditor>,
    ids: Rc<RefCell<BlockIds>>,
}
fn position(v: &Value) -> RichTextPosition {
    RichTextPosition::new(
        RichTextBlockId::new(v["block"].as_str().unwrap_or_default().to_owned()),
        v["offset"].as_u64().unwrap_or(0) as usize,
    )
}
fn position_value(p: &RichTextPosition) -> Value {
    json!({"block":p.block.as_str(),"offset":p.offset})
}
fn selection(v: &Value) -> RichTextSelection {
    RichTextSelection::new(position(&v["anchor"]), position(&v["head"]))
}
fn selection_value(s: &RichTextSelection) -> Value {
    json!({"anchor":position_value(&s.anchor),"head":position_value(&s.head)})
}
fn format(v: &Value) -> RichTextFormat {
    match v.as_str() {
        Some("italic") => RichTextFormat::Italic,
        Some("underline") => RichTextFormat::Underline,
        Some("strike") => RichTextFormat::Strike,
        Some("code") => RichTextFormat::Code,
        _ => RichTextFormat::Bold,
    }
}
fn format_value(v: RichTextFormat) -> &'static str {
    match v {
        RichTextFormat::Bold => "bold",
        RichTextFormat::Italic => "italic",
        RichTextFormat::Underline => "underline",
        RichTextFormat::Strike => "strike",
        RichTextFormat::Code => "code",
    }
}
fn alignment(v: &Value) -> RichTextAlignment {
    match v.as_str() {
        Some("center") => RichTextAlignment::Center,
        Some("end") => RichTextAlignment::End,
        _ => RichTextAlignment::Start,
    }
}
fn alignment_value(v: RichTextAlignment) -> &'static str {
    match v {
        RichTextAlignment::Start => "start",
        RichTextAlignment::Center => "center",
        RichTextAlignment::End => "end",
    }
}
fn list_kind(v: &Value) -> Option<RichTextListKind> {
    match v.as_str() {
        Some("ordered") => Some(RichTextListKind::Ordered),
        Some("unordered") => Some(RichTextListKind::Unordered),
        _ => None,
    }
}
fn list_value(v: Option<RichTextListKind>) -> Value {
    v.map_or(Value::Null, |v| {
        json!(match v {
            RichTextListKind::Ordered => "ordered",
            RichTextListKind::Unordered => "unordered",
        })
    })
}
fn style(v: &Value) -> RichTextInlineStyle {
    let mut style = RichTextInlineStyle::default();
    for kind in [
        RichTextFormat::Bold,
        RichTextFormat::Italic,
        RichTextFormat::Underline,
        RichTextFormat::Strike,
        RichTextFormat::Code,
    ] {
        style = style.with_format(kind, v[format_value(kind)].as_bool().unwrap_or(false));
    }
    style.with_link(v["link"].as_str().map(|s| s.to_owned().into()))
}
fn style_value(v: &RichTextInlineStyle) -> Value {
    json!({"bold":v.format(RichTextFormat::Bold),"italic":v.format(RichTextFormat::Italic),"underline":v.format(RichTextFormat::Underline),"strike":v.format(RichTextFormat::Strike),"code":v.format(RichTextFormat::Code),"link":v.link().map(|s|s.as_ref())})
}
fn document(v: &Value) -> anyhow::Result<RichTextDocument> {
    Ok(RichTextDocument::new(
        v["blocks"].as_array().into_iter().flatten().map(|v| {
            let mut block = RichTextBlock::new(
                RichTextBlockId::new(v["id"].as_str().unwrap_or_default().to_owned()),
                v["text"].as_str().unwrap_or_default().to_owned(),
            );
            let paragraph = RichTextParagraphStyle::default()
                .with_alignment(alignment(&v["paragraph"]["alignment"]))
                .with_list(list_kind(&v["paragraph"]["list"]["kind"]).map(|kind| {
                    RichTextListItem::new(kind)
                        .depth(v["paragraph"]["list"]["depth"].as_u64().unwrap_or(0) as u8)
                }));
            block = block.with_paragraph(paragraph);
            for run in v["styles"].as_array().into_iter().flatten() {
                block = block.with_style(text_area::range(&run["range"]), style(&run["style"]));
            }
            block
        }),
    )?)
}
fn document_value(v: &RichTextDocument) -> Value {
    let blocks=v.blocks().iter().map(|block|{
        let mut offset=0;
        let mut runs=block.styles().runs().iter().map(|run|{
            let start=offset;offset+=run.len;
            json!({"range":{"start":start,"end":offset},"style":style_value(&run.style)})
        }).collect::<Vec<_>>();
        if block.text().is_empty(){runs.push(json!({"range":{"start":0,"end":0},"style":style_value(block.styles().style_at(0))}));}
        let paragraph=block.paragraph();
        json!({"id":block.id().as_str(),"text":block.text().as_ref(),"styles":runs,"paragraph":{"alignment":alignment_value(paragraph.alignment()),"list":paragraph.list().map(|item|json!({"kind":list_value(Some(item.kind)),"depth":item.depth}))}})
    }).collect::<Vec<_>>();
    json!({"blocks":blocks})
}
fn input_kind(v: &Value) -> RichTextInputKind {
    match v.as_str() {
        Some("deleting") => RichTextInputKind::Deleting,
        Some("paste") => RichTextInputKind::Paste,
        Some("cut") => RichTextInputKind::Cut,
        _ => RichTextInputKind::Typing,
    }
}
fn input_value(v: RichTextInputKind) -> &'static str {
    match v {
        RichTextInputKind::Typing => "typing",
        RichTextInputKind::Deleting => "deleting",
        RichTextInputKind::Paste => "paste",
        RichTextInputKind::Cut => "cut",
    }
}
fn intent(v: &Value) -> RichTextIntent {
    match v["kind"].as_str() {
        Some("select") => RichTextIntent::Select(selection(&v["selection"])),
        Some("replace") => RichTextIntent::Replace {
            text: v["text"].as_str().unwrap_or_default().to_owned().into(),
            kind: input_kind(&v["input"]),
        },
        Some("replaceMultiline") => RichTextIntent::ReplaceMultiline {
            text: v["text"].as_str().unwrap_or_default().to_owned().into(),
            new_blocks: v["newBlocks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|s| RichTextBlockId::new(s.to_owned()))
                .collect(),
            kind: input_kind(&v["input"]),
        },
        Some("hardBreak") => RichTextIntent::InsertHardBreak {
            new_block: RichTextBlockId::new(v["newBlock"].as_str().unwrap_or_default().to_owned()),
        },
        Some("softBreak") => RichTextIntent::InsertSoftBreak,
        Some("backspaceAtStart") => RichTextIntent::BackspaceAtStart,
        Some("toggleFormat") => RichTextIntent::ToggleFormat(format(&v["format"])),
        Some("setLink") => {
            RichTextIntent::SetLink(v["destination"].as_str().map(|s| s.to_owned().into()))
        }
        Some("setAlignment") => RichTextIntent::SetAlignment(alignment(&v["alignment"])),
        Some("setList") => RichTextIntent::SetList(list_kind(&v["list"])),
        Some("changeListDepth") => {
            RichTextIntent::ChangeListDepth(v["delta"].as_i64().unwrap_or(0) as i8)
        }
        Some("compose") => RichTextIntent::Compose {
            text: v["text"].as_str().unwrap_or_default().to_owned().into(),
            selection_in_text: v
                .get("selection")
                .filter(|v| !v.is_null())
                .map(text_area::range),
        },
        Some("endComposition") => RichTextIntent::EndComposition,
        Some("undo") => RichTextIntent::Undo,
        _ => RichTextIntent::Redo,
    }
}
fn intent_value(v: &RichTextIntent) -> Value {
    match v {
        RichTextIntent::Select(s) => json!({"kind":"select","selection":selection_value(s)}),
        RichTextIntent::Replace { text, kind } => {
            json!({"kind":"replace","text":text.as_ref(),"input":input_value(*kind)})
        }
        RichTextIntent::ReplaceMultiline {
            text,
            new_blocks,
            kind,
        } => {
            json!({"kind":"replaceMultiline","text":text.as_ref(),"newBlocks":new_blocks.iter().map(|id|id.as_str()).collect::<Vec<_>>(),"input":input_value(*kind)})
        }
        RichTextIntent::InsertHardBreak { new_block } => {
            json!({"kind":"hardBreak","newBlock":new_block.as_str()})
        }
        RichTextIntent::InsertSoftBreak => json!({"kind":"softBreak"}),
        RichTextIntent::BackspaceAtStart => json!({"kind":"backspaceAtStart"}),
        RichTextIntent::ToggleFormat(f) => json!({"kind":"toggleFormat","format":format_value(*f)}),
        RichTextIntent::SetLink(s) => {
            json!({"kind":"setLink","destination":s.as_ref().map(|s|s.as_ref())})
        }
        RichTextIntent::SetAlignment(a) => {
            json!({"kind":"setAlignment","alignment":alignment_value(*a)})
        }
        RichTextIntent::SetList(l) => json!({"kind":"setList","list":list_value(*l)}),
        RichTextIntent::ChangeListDepth(d) => json!({"kind":"changeListDepth","delta":d}),
        RichTextIntent::Compose {
            text,
            selection_in_text,
        } => {
            json!({"kind":"compose","text":text.as_ref(),"selection":selection_in_text.clone().map(text_area::range_value)})
        }
        RichTextIntent::EndComposition => json!({"kind":"endComposition"}),
        RichTextIntent::Undo => json!({"kind":"undo"}),
        RichTextIntent::Redo => json!({"kind":"redo"}),
    }
}
fn session_query(session: &RichTextEditSession, method: &str) -> anyhow::Result<Value> {
    Ok(match method {
        "document" => document_value(session.document()),
        "selection" => selection_value(session.selection()),
        "pending_style" => style_value(session.pending_style()),
        "marked_range" => session.marked_range().map_or(
            Value::Null,
            |r| json!({"start":position_value(&r.start),"end":position_value(&r.end)}),
        ),
        "can_undo" => json!(session.can_undo()),
        "can_redo" => json!(session.can_redo()),
        _ => anyhow::bail!("unsupported rich session query"),
    })
}

impl State {
    pub(super) fn rich_reference_query(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        cx: &App,
        refs: &crate::references::Registration<'_>,
    ) -> Option<anyhow::Result<Value>> {
        if !matches!(method, "session" | "focus_handle") {
            return None;
        }
        Some((|| {
            super::super::validation::invocation("RichTextEditor", method, args, true)?;
            let retained = self
                .rich_editors
                .borrow()
                .get(&(node.instance, node.id.clone()))
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
            let entity = &retained.entry.entity;
            if method == "session" {
                refs.entity(
                    "RichTextEditSession",
                    entity,
                    entity.read(cx).session(),
                    |editor, _| Some(editor.session().clone()),
                    |_, _| false,
                    session_dispatch,
                )
            } else {
                let focus = gpui::Focusable::focus_handle(entity.read(cx), cx);
                refs.focus(
                    entity,
                    &focus,
                    |editor, focus, cx| gpui::Focusable::focus_handle(editor, cx) == *focus,
                    |editor, _| !editor.is_disabled(),
                )
            }
        })())
    }
    pub(super) fn render_rich(
        &self,
        node: &Node,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.rich_editors.borrow().get(&key).cloned();
        let retained=existing.unwrap_or_else(||{
            let document=document(node.props.get("document").unwrap_or(&Value::Null)).expect("validated nonempty rich document");let ids=Rc::new(RefCell::new(BlockIds::default()));ids.borrow_mut().reserve(&document);
            let session=cx.new(|_|RichTextEditSession::new(document));let allocate=ids.clone();let entity=cx.new(|cx|RichTextEditor::new(node.id.clone(),session,move||allocate.borrow_mut().mint(),window,cx));
            let route=Rc::new(RefCell::new(Route{events:node.events.clone(),emit:emit.clone(),disabled:flag(node,"disabled")}));let callback=Rc::downgrade(&route);
            let subscription=cx.subscribe(&entity,move|entity,event:&RichTextEditorEvent,cx|{if entity.read(cx).is_disabled(){return;}let name=match event{RichTextEditorEvent::IntentApplied{..}=>"intentApplied",RichTextEditorEvent::IntentRefused{..}=>"intentRefused",RichTextEditorEvent::LinkRequested(_)=>"linkRequested",RichTextEditorEvent::Focus=>"focus",RichTextEditorEvent::Blur=>"blur"};let target=callback.upgrade().and_then(|r|{let r=r.borrow();if r.disabled{None}else{r.events.get(name).map(|a|(a.clone(),r.emit.clone()))}});if let Some((action,emit))=target{emit(&action,match event{RichTextEditorEvent::IntentApplied{intent,result}=>json!({"intent":intent_value(intent),"result":{"documentChanged":result.document_changed,"selectionChanged":result.selection_changed,"pendingStyleChanged":result.pending_style_changed}}),RichTextEditorEvent::IntentRefused{intent,error}=>json!({"intent":intent_value(intent),"reason":error.to_string()}),RichTextEditorEvent::LinkRequested(s)=>selection_value(s),_=>Value::Null});}});
            let retained=Rc::new(RichEntry{entry:Entry{entity,route,props:Default::default(),_subscription:subscription},ids});self.rich_editors.borrow_mut().insert(key,retained.clone());retained
        });
        let entry = &retained.entry;
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled: flag(node, "disabled"),
        };
        if *entry.props.borrow() != node.props {
            if !entry.props.borrow().is_empty()
                && entry.props.borrow().get("document") != node.props.get("document")
            {
                let doc = document(&node.props["document"]).expect("validated rich document");
                retained.ids.borrow_mut().reserve(&doc);
                let selection = doc.selection_at_start();
                let session = entry.entity.read(cx).session().clone();
                session.update(cx, |s, cx| {
                    s.replace_document(doc, selection)
                        .expect("document start selection");
                    cx.notify();
                });
            }
            entry.entity.update(cx, |e, cx| {
                e.set_name(text(node, "name"), cx);
                e.set_placeholder(text(node, "placeholder"), cx);
                e.set_frame(
                    if node.props.get("frame").and_then(Value::as_str) == Some("host") {
                        Frame::Host
                    } else {
                        Frame::Own
                    },
                    cx,
                );
                e.set_toolbar(
                    node.props
                        .get("toolbar")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                    cx,
                );
                e.set_rows(
                    node.props.get("rows").and_then(Value::as_u64).unwrap_or(5) as usize,
                    cx,
                );
                e.set_max_rows(
                    node.props
                        .get("maxRows")
                        .and_then(Value::as_u64)
                        .map(|n| n as usize),
                    cx,
                );
                e.set_required(flag(node, "required"), cx);
                e.set_invalid(flag(node, "invalid"), cx);
                e.set_read_only(flag(node, "readOnly"), cx);
                e.set_disabled(flag(node, "disabled"), cx);
            });
            *entry.props.borrow_mut() = node.props.clone();
        }
        entry.entity.clone().into_any_element()
    }
    pub(super) fn invoke_rich(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        cx: &mut App,
    ) -> anyhow::Result<Value> {
        let retained = self
            .rich_editors
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("native target is not mounted"))?;
        let entity = &retained.entry.entity;
        let session = entity.read(cx).session().clone();
        if query {
            return match method {
                "is_disabled" => Ok(json!(entity.read(cx).is_disabled())),
                "is_read_only" => Ok(json!(entity.read(cx).is_read_only())),
                _ => session_query(session.read(cx), method),
            };
        }
        anyhow::ensure!(
            !flag(node, "disabled") && !entity.read(cx).is_disabled(),
            "disabled rich editor refuses invocation"
        );
        match method {
            "apply_intent" => {
                let intent = intent(&args["intent"]);
                anyhow::ensure!(
                    !entity.read(cx).is_read_only() || matches!(intent, RichTextIntent::Select(_)),
                    "read-only rich editor refuses mutation"
                );
                retained.ids.borrow_mut().intent(&intent);
                entity.update(cx, |e, cx| e.apply_intent(intent, cx));
            }
            "replace_document" => {
                anyhow::ensure!(
                    !entity.read(cx).is_read_only(),
                    "read-only rich editor refuses document replacement"
                );
                let doc = document(&args["document"])?;
                retained.ids.borrow_mut().reserve(&doc);
                session.update(cx, |s, cx| {
                    s.replace_document(doc, selection(&args["selection"]))?;
                    cx.notify();
                    Ok::<_, anyhow::Error>(())
                })?;
            }
            "forbid_history" => session.update(cx, |s, cx| {
                s.forbid_history();
                cx.notify();
            }),
            "set_diagnostics" => entity.update(cx,|editor,cx|editor.set_diagnostics(args["diagnostics"].as_array().into_iter().flatten().map(|v|gpui_kit::controls::rich_text_editor::RichTextDiagnostic::new(RichTextRange{start:position(&v["range"]["start"]),end:position(&v["range"]["end"])},match v["severity"].as_str(){Some("error")=>gpui_kit::controls::rich_text_editor::RichTextDiagnosticSeverity::Error,Some("warning")=>gpui_kit::controls::rich_text_editor::RichTextDiagnosticSeverity::Warning,_=>gpui_kit::controls::rich_text_editor::RichTextDiagnosticSeverity::Info})),cx)),
            _ => entity.update(cx, |e, cx| {
                match method {
                    "set_name" => {
                        e.set_name(args["name"].as_str().unwrap_or_default().to_owned(), cx)
                    }
                    "set_placeholder" => e.set_placeholder(
                        args["placeholder"].as_str().unwrap_or_default().to_owned(),
                        cx,
                    ),
                    "set_frame" => e.set_frame(
                        if args["frame"] == "host" {
                            Frame::Host
                        } else {
                            Frame::Own
                        },
                        cx,
                    ),
                    "set_toolbar" => e.set_toolbar(args["visible"].as_bool().unwrap_or(false), cx),
                    "set_rows" => e.set_rows(args["rows"].as_u64().unwrap_or(1) as usize, cx),
                    "set_max_rows" => {
                        e.set_max_rows(args["max_rows"].as_u64().map(|n| n as usize), cx)
                    }
                    "set_disabled" => {
                        e.set_disabled(args["disabled"].as_bool().unwrap_or(false), cx)
                    }
                    "set_read_only" => {
                        e.set_read_only(args["read_only"].as_bool().unwrap_or(false), cx)
                    }
                    "set_required" => {
                        e.set_required(args["required"].as_bool().unwrap_or(false), cx)
                    }
                    "set_invalid" => e.set_invalid(args["invalid"].as_bool().unwrap_or(false), cx),
                    _ => anyhow::bail!("unsupported RichTextEditor command"),
                };
                Ok(())
            })?,
        }
        Ok(Value::Null)
    }
}

fn session_dispatch(
    target: &Entity<RichTextEditSession>,
    method: &str,
    args: &Value,
    query: bool,
    _window: &mut Window,
    cx: &mut App,
    _refs: &crate::references::Registration<'_>,
) -> anyhow::Result<Value> {
    anyhow::ensure!(
        query,
        "rich session reference is read-only; mutate through its editor"
    );
    let schema = super::super::validation::invocation("RichTextEditor", method, args, true)?;
    let value = session_query(target.read(cx), method)?;
    super::super::validation::validate(&value, schema)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use gpui_kit_testkit::harness::Harness;
    #[test]
    fn rich_document_retains_styles_paragraphs_and_empty_insertion_style() {
        let source = json!({"blocks":[{"id":"first","text":"AλZ","styles":[{"range":{"start":1,"end":3},"style":{"bold":true,"link":"caller:destination"}}],"paragraph":{"alignment":"center","list":{"kind":"ordered","depth":2}}},{"id":"empty","text":"","styles":[{"range":{"start":0,"end":0},"style":{"italic":true}}]}]});
        let native = document(&source).expect("valid");
        let value = document_value(&native);
        assert_eq!(value["blocks"][0]["text"], "AλZ");
        assert_eq!(
            value["blocks"][0]["styles"][1]["range"],
            json!({"start":1,"end":3})
        );
        assert_eq!(
            value["blocks"][0]["styles"][1]["style"]["link"],
            "caller:destination"
        );
        assert_eq!(
            value["blocks"][0]["paragraph"],
            source["blocks"][0]["paragraph"]
        );
        assert_eq!(value["blocks"][1]["styles"][0]["style"]["italic"], true);
        assert_eq!(document_value(&document(&value).expect("roundtrip")), value);
        assert!(document(&json!({"blocks":[]})).is_err());
        assert!(
            document(&json!({"blocks":[{"id":"same","text":"a"},{"id":"same","text":"b"}]}))
                .is_err()
        );
    }
    #[gpui::test]
    fn rich_native_editing_keeps_session_history_and_issued_block_ids(cx: &mut TestAppContext) {
        let owner = gpui::EffectOwner::new();
        let state = Rc::new(State::default());
        let node=Rc::new(RefCell::new(serde_json::from_value::<Node>(json!({"kind":"kit","component":"RichTextEditor","id":"rich","props":{"document":{"blocks":[{"id":"host-block-1","text":"AλZ"},{"id":"host-block-3","text":"Last"}]},"rows":4,"maxRows":8},"events":{"intentApplied":"applied","intentRefused":"refused"}})).expect("fixture")));
        let events = Rc::new(RefCell::new(Vec::new()));
        let (s, n, e) = (state.clone(), node.clone(), events.clone());
        let mut harness = Harness::new(
            cx,
            move |cx| {
                gpui_kit::install(cx);
                gpui_kit::foundation::register_owner_state(owner, cx);
            },
            move |window, cx| {
                let e = e.clone();
                let element = cx.with_effect_owner(Some(owner), |cx| {
                    s.render_rich(
                        &n.borrow(),
                        window,
                        cx,
                        Rc::new(move |name, value| e.borrow_mut().push((name.to_owned(), value))),
                    )
                });
                gpui::effect_owner(owner, element).into_any_element()
            },
        );
        let session = harness.update(|_, cx| {
            state.rich_editors.borrow()[&(0, "rich".into())]
                .entry
                .entity
                .read(cx)
                .session()
                .clone()
        });
        harness.click("rich");
        harness.update(|_,cx|{state.invoke_rich(&node.borrow(),"apply_intent",&json!({"intent":{"kind":"select","selection":{"anchor":{"block":"host-block-1","offset":4},"head":{"block":"host-block-1","offset":4}}}}),false,cx).expect("select");});
        harness.keystrokes("enter x");
        harness.update(|_, cx| {
            let doc = state
                .invoke_rich(&node.borrow(), "document", &json!({}), true, cx)
                .expect("document");
            assert_eq!(doc["blocks"][1]["id"], "host-block-2");
            assert_eq!(doc["blocks"][1]["text"], "x");
        });
        node.borrow_mut().props.remove("maxRows");
        node.borrow_mut()
            .props
            .insert("toolbar".into(), json!(false));
        harness.update(|_, cx| cx.refresh_windows());
        harness.keystrokes("ctrl-z ctrl-z enter");
        harness.update(|_, cx| {
            assert_eq!(
                state.rich_editors.borrow()[&(0, "rich".into())]
                    .entry
                    .entity
                    .read(cx)
                    .session(),
                &session
            );
            let doc = state
                .invoke_rich(&node.borrow(), "document", &json!({}), true, cx)
                .expect("doc");
            assert_eq!(doc["blocks"][1]["id"], "host-block-4");
            state
                .invoke_rich(
                    &node.borrow(),
                    "apply_intent",
                    &json!({"intent":{"kind":"hardBreak","newBlock":"host-block-3"}}),
                    false,
                    cx,
                )
                .expect("core refusal via event");
            assert_eq!(
                state
                    .invoke_rich(&node.borrow(), "document", &json!({}), true, cx)
                    .expect("unchanged"),
                doc
            );
            state
                .invoke_rich(
                    &node.borrow(),
                    "set_read_only",
                    &json!({"read_only":true}),
                    false,
                    cx,
                )
                .expect("readonly");
            assert!(
                state
                    .invoke_rich(
                        &node.borrow(),
                        "apply_intent",
                        &json!({"intent":{"kind":"replace","text":"bad","input":"typing"}}),
                        false,
                        cx
                    )
                    .is_err()
            );
        });
        assert!(events.borrow().iter().any(
            |(name, value)| name == "refused" && value["intent"]["newBlock"] == "host-block-3"
        ));
        let registry = crate::references::Registry::new();
        harness.update(|window, cx| {
            let descriptor = node.borrow();
            let refs = registry.registration(&descriptor, owner);
            let borrowed = state
                .rich_reference_query(&descriptor, "session", &json!({}), cx, &refs)
                .expect("getter")
                .expect("session ref");
            let focus = state
                .rich_reference_query(&descriptor, "focus_handle", &json!({}), cx, &refs)
                .expect("focus getter")
                .expect("focus ref");
            assert_eq!(
                registry
                    .invoke(owner, &borrowed, "document", &json!({}), true, window, cx)
                    .expect("real session"),
                state
                    .invoke_rich(&descriptor, "document", &json!({}), true, cx)
                    .expect("same document")
            );
            assert!(
                registry
                    .invoke(
                        owner,
                        &borrowed,
                        "apply_intent",
                        &json!({"intent":{"kind":"undo"}}),
                        false,
                        window,
                        cx
                    )
                    .is_err()
            );
            state
                .invoke_rich(
                    &descriptor,
                    "set_disabled",
                    &json!({"disabled":true}),
                    false,
                    cx,
                )
                .expect("disable parent");
            assert!(
                registry
                    .invoke(owner, &focus, "focus", &json!({}), false, window, cx)
                    .is_err()
            );
            assert!(
                registry
                    .invoke(owner, &borrowed, "document", &json!({}), true, window, cx)
                    .is_ok()
            );
            let replacement: Node = serde_json::from_value(
                json!({"kind":"text","id":"replacement","text":"Unmounted"}),
            )
            .expect("replacement");
            state.reconcile(&replacement, cx);
            registry.reconcile(&replacement, |_| Some(owner), |_| None, cx);
            assert!(
                registry
                    .invoke(owner, &borrowed, "document", &json!({}), true, window, cx)
                    .is_err()
            );
        });
    }
}
