//! Native retained app host. The JS engine never owns a GPUI object or pointer.
use std::{
    collections::{BTreeMap, HashSet},
    io::{BufRead, BufReader, Read, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender},
    time::Duration,
};

use anyhow::{Context as _, Result, bail, ensure};
use gpui::{
    App, Bounds, Context, FocusHandle, IntoElement, Render, SharedString, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, size,
};
use gpui_kit::prelude::*;
use gpui_kit_semantics::{NodeSpec, Role, Semantic, SemanticCoordinator};
use gpui_kit_theme::Theme;
use serde::Deserialize;
use serde_json::{Value, json};

const MAX_FRAME: u64 = 256 * 1024;

#[cfg(feature = "capture")]
mod capture;
mod clipboard;
mod construction;
mod kit_bindings;
mod native;
mod references;
mod resource_bridge;
pub mod resources;
#[cfg(feature = "capture")]
mod review_fonts;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Node {
    kind: Kind,
    id: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    children: Vec<Node>,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    component: Option<String>,
    #[serde(default)]
    props: serde_json::Map<String, Value>,
    #[serde(default)]
    slots: BTreeMap<String, Vec<Node>>,
    #[serde(default)]
    events: BTreeMap<String, String>,
    #[serde(default)]
    predicates: BTreeMap<String, String>,
    #[serde(default)]
    instance: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Column,
    Row,
    Text,
    Button,
    Kit,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    kind: String,
    generation: u64,
    revision: u64,
    tree: Node,
    #[serde(default)]
    clipboard: BTreeMap<u64, clipboard::Grants>,
    #[serde(default)]
    resources: BTreeMap<u64, bool>,
}

impl Node {
    fn validate(&self, depth: usize, ids: &mut HashSet<String>) -> Result<()> {
        ensure!(
            depth <= 40 && ids.len() < 4096,
            "native tree limit exceeded"
        );
        ensure!(
            !self.id.is_empty()
                && self.id.len() <= 512
                && self
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                && ids.insert(self.id.clone()),
            "invalid or duplicate semantic id"
        );
        ensure!(self.text.len() <= 65536, "text exceeds limit");
        ensure!(
            self.action
                .as_ref()
                .is_none_or(|action| self.kind == Kind::Button && action.len() <= 512),
            "invalid action"
        );
        ensure!(
            self.children.is_empty() || matches!(self.kind, Kind::Column | Kind::Row),
            "leaf has children"
        );
        if self.kind == Kind::Kit {
            ensure!(
                self.component
                    .as_deref()
                    .is_some_and(|name| kit_bindings::COMPONENTS.contains(&name)),
                "unsupported Kit component"
            );
            ensure!(
                self.events.len() <= 16 && self.events.values().all(|action| action.len() <= 512),
                "invalid Kit events"
            );
            kit_bindings::validate_descriptor(self)?;
        } else {
            ensure!(
                self.component.is_none()
                    && self.props.is_empty()
                    && self.slots.is_empty()
                    && self.events.is_empty(),
                "Kit fields on primitive"
            );
        }
        for child in self.children.iter().chain(self.slots.values().flatten()) {
            child.validate(depth + 1, ids)?;
        }
        Ok(())
    }
}

enum Incoming {
    Frame(Frame),
    Request(HostRequest),
}

enum HostRequest {
    Invoke(native::Invocation),
    Resource(resource_bridge::ResourceRequest),
}

fn read_incoming(reader: &mut impl BufRead) -> Result<Option<Incoming>> {
    let mut bytes = Vec::new();
    let count = reader.take(MAX_FRAME + 1).read_until(b'\n', &mut bytes)?;
    if count == 0 {
        return Ok(None);
    }
    ensure!(
        count as u64 <= MAX_FRAME && bytes.last() == Some(&b'\n'),
        "protocol frame exceeds limit or is truncated"
    );
    let value: Value = serde_json::from_slice(&bytes)?;
    if value.get("kind").and_then(Value::as_str) == Some("invoke") {
        let request: native::Invocation = serde_json::from_value(value)?;
        request.validate()?;
        return Ok(Some(Incoming::Request(HostRequest::Invoke(request))));
    }
    if value.get("kind").and_then(Value::as_str) == Some("register-resource") {
        let request: resource_bridge::ResourceRequest = serde_json::from_value(value)?;
        request.validate()?;
        return Ok(Some(Incoming::Request(HostRequest::Resource(request))));
    }
    let frame: Frame = serde_json::from_value(value)?;
    ensure!(
        frame.kind == "render" && frame.generation == 0,
        "unsupported host protocol"
    );
    frame.tree.validate(0, &mut HashSet::new())?;
    Ok(Some(Incoming::Frame(frame)))
}

#[cfg(test)]
fn read_frame(reader: &mut impl BufRead) -> Result<Option<Frame>> {
    match read_incoming(reader)? {
        Some(Incoming::Frame(frame)) => Ok(Some(frame)),
        None => Ok(None),
        Some(Incoming::Request(_)) => bail!("expected render frame"),
    }
}

struct Bridge {
    child: Child,
    outgoing: SyncSender<Value>,
    incoming: Receiver<Result<Frame>>,
    requests: Receiver<HostRequest>,
}

impl Bridge {
    fn start(arguments: Vec<String>) -> Result<Self> {
        let mut child =
            Command::new(std::env::var_os("GPUI_NODE").unwrap_or_else(|| "node".into()))
                .args(arguments)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .context("start external Node runtime (>=26.5.1 required)")?;
        let mut input = child.stdin.take().context("runtime stdin")?;
        let output = child.stdout.take().context("runtime stdout")?;
        let (outgoing, write_queue) = mpsc::sync_channel::<Value>(32);
        std::thread::spawn(move || {
            while let Ok(message) = write_queue.recv() {
                if writeln!(input, "{message}").is_err() {
                    break;
                }
            }
        });
        let (read_queue, incoming) = mpsc::sync_channel(16);
        let (request_queue, requests) = mpsc::sync_channel(32);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                match read_incoming(&mut reader) {
                    Ok(Some(Incoming::Request(request))) => {
                        if request_queue.send(request).is_err() {
                            break;
                        }
                    }
                    Ok(Some(Incoming::Frame(frame))) => {
                        if read_queue.send(Ok(frame)).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {
                        let _ = read_queue.send(Err(anyhow::anyhow!(
                            "Runtime disconnected; last verified view retained"
                        )));
                        break;
                    }
                    Err(error) => {
                        let _ = read_queue.send(Err(error));
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child,
            outgoing,
            incoming,
            requests,
        })
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.outgoing.try_send(json!({ "kind": "close" }));
        for _ in 0..50 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Host {
    bridge: Bridge,
    frame: Option<Frame>,
    rendered_revision: std::rc::Rc<std::cell::Cell<u64>>,
    error: Option<String>,
    focus: FocusHandle,
    kit: std::rc::Rc<kit_bindings::KitState>,
    clipboard: clipboard::Policy,
    resource_store: resources::ResourceStore,
    references: references::Registry,
}

impl Host {
    fn handle_request(&mut self, request: HostRequest, window: &mut Window, cx: &mut App) {
        match request {
            HostRequest::Invoke(request) => self.invoke_request(request, window, cx),
            HostRequest::Resource(request) => self.register_resource(request, cx),
        }
    }
}

#[derive(Clone)]
struct NodeRenderer {
    outgoing: std::sync::mpsc::SyncSender<Value>,
    kit: std::rc::Weak<kit_bindings::KitState>,
    rendered_revision: std::rc::Rc<std::cell::Cell<u64>>,
    clipboard: clipboard::Policy,
}

impl NodeRenderer {
    fn build_context(&self, node: &Node, revision: u64) -> construction::NativeBuildContext {
        construction::NativeBuildContext {
            deferred: None,
            typed: construction::TypedSlots::new(self.clone(), node, revision),
            slots: self.slots(node, revision),
        }
    }

    fn slots(&self, node: &Node, revision: u64) -> kit_bindings::KitSlots {
        node.slots
            .iter()
            .map(|(name, children)| {
                let renderer = self.clone();
                let children = children.clone();
                let render: gpui_kit::foundation::SlotRender =
                    std::rc::Rc::new(move |window, cx| {
                        div()
                            .flex()
                            .flex_col()
                            .children(
                                children
                                    .iter()
                                    .map(|child| renderer.node(child, revision, window, cx)),
                            )
                            .into_any_element()
                    });
                (name.clone(), render)
            })
            .collect()
    }

    fn emitter(&self, revision: u64) -> construction::Emit {
        let outgoing = self.outgoing.clone();
        std::rc::Rc::new(move |action: &str, payload: Value| {
            if serde_json::to_vec(&payload).is_ok_and(|bytes| bytes.len() <= 16384) {
                let _ = outgoing.try_send(json!({"kind":"event", "generation":0, "revision":revision, "action":action, "payload":payload}));
            }
        })
    }

    fn node(
        &self,
        node: &Node,
        revision: u64,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::AnyElement {
        let kit = self.kit.upgrade();
        let owner = self.clipboard.owner_of(node);
        let Some(kit) = kit.filter(|_| {
            self.rendered_revision.get() == revision && (node.instance == 0 || owner.is_some())
        }) else {
            return div()
                .id(SharedString::from(node.id.clone()))
                .semantic_in(cx, NodeSpec::new(node.id.clone(), Role::Text))
                .child("Unavailable: native view expired")
                .into_any_element();
        };
        cx.with_effect_owner(owner, |cx| {
            let element = self.node_inner(node, revision, &kit, window, cx);
            match owner {
                Some(owner) => gpui::effect_owner(owner, element).into_any_element(),
                None => element,
            }
        })
    }

    fn node_inner(
        &self,
        node: &Node,
        revision: u64,
        kit: &kit_bindings::KitState,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::AnyElement {
        let theme = Theme::get(cx).clone();
        let id = SharedString::from(node.id.clone());
        match node.kind {
            Kind::Kit => {
                let slots = self.slots(node, revision);
                let emit = self.emitter(revision);
                kit.render(node, slots, window, cx, emit)
            }
            Kind::Button => {
                let mut button = Button::new(id)
                    .label(node.text.clone())
                    .disabled(node.disabled);
                if !node.disabled
                    && let Some(action) = node.action.clone()
                {
                    let outgoing = self.outgoing.clone();
                    button = button.on_click(move |_, _| {
                        if outgoing.try_send(json!({ "kind": "event", "generation": 0, "revision": revision, "action": action })).is_err() {
                            eprintln!("Runtime event queue unavailable; action refused");
                        }
                    });
                }
                button.into_any_element()
            }
            Kind::Text => div()
                .id(id.clone())
                // User text stays in rendered UI, never in semantic snapshots.
                .semantic_in(cx, NodeSpec::new(id, Role::Text))
                .child(node.text.clone())
                .into_any_element(),
            Kind::Column | Kind::Row => {
                let mut container = div().id(id.clone()).flex().gap(px(theme.spacing.md));
                if node.kind == Kind::Column {
                    container = container.flex_col();
                }
                container
                    .semantic_in(cx, NodeSpec::new(id, Role::Group))
                    .children(
                        node.children
                            .iter()
                            .map(|child| self.node(child, revision, window, cx)),
                    )
                    .into_any_element()
            }
        }
    }
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SemanticCoordinator::global(cx).begin_frame(window);
        let theme = Theme::get(cx).clone();
        let outgoing = self.bridge.outgoing.clone();
        let mut root = div()
            .id("native.host")
            .track_focus(&self.focus)
            .size_full()
            .overflow_y_scroll()
            .p(px(theme.spacing.xl))
            .bg(theme.colors.canvas)
            .text_color(theme.colors.text)
            .font_family(theme.typography.sans.clone())
            .semantic_in(cx, NodeSpec::new("native.host", Role::Window))
            .on_key_down(move |event, _, _| {
                let modifiers = event.keystroke.modifiers;
                let prefix = if modifiers.control {
                    "ctrl"
                } else if modifiers.alt {
                    "alt"
                } else if modifiers.platform {
                    "cmd"
                } else {
                    return;
                };
                let _ = outgoing.try_send(
                    json!({ "kind": "key", "key": format!("{prefix}-{}", event.keystroke.key) }),
                );
            });
        if let Some(error) = &self.error {
            root = root.child(format!("Host error: {error}"));
        }
        if let Some(frame) = self.frame.clone() {
            self.clipboard.reconcile(&frame.tree, &frame.clipboard, cx);
            self.resource_store
                .reconcile(&self.clipboard.resource_aliases(&frame.resources), cx);
            self.kit.reconcile(&frame.tree, cx);
            self.references.reconcile(
                &frame.tree,
                |node| self.clipboard.owner_of(node),
                |id| self.kit.native_entity_id(id),
                cx,
            );
            let renderer = NodeRenderer {
                outgoing: self.bridge.outgoing.clone(),
                kit: std::rc::Rc::downgrade(&self.kit),
                rendered_revision: self.rendered_revision.clone(),
                clipboard: self.clipboard.clone(),
            };
            self.rendered_revision.set(frame.revision);
            root = root.child(renderer.node(&frame.tree, frame.revision, window, cx));
        } else {
            root = root.child("Loading native JavaScript app…");
        }
        root
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.rendered_revision.set(0);
        self.clipboard.revoke();
        self.references.revoke();
    }
}

fn main() -> Result<()> {
    env_logger::init();
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    #[cfg(feature = "capture")]
    let capture = arguments
        .windows(2)
        .find(|pair| pair[0] == "--capture")
        .map(|pair| pair[1].clone());
    if arguments.len() < 2 {
        bail!(
            "usage: gpui-box-app-host RUNNER.mjs APP_DIRECTORY [--dev] [--trust-local] [--data-dir DIR]"
        );
    }
    let bridge = Bridge::start(arguments)?;
    #[cfg(feature = "capture")]
    if let Some(path) = capture {
        return capture::run(bridge, &path);
    }
    gpui_platform::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx: &mut App| {
            gpui_kit::install(cx);
            let bounds = Bounds::centered(None, size(px(980.0), px(760.0)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let focus = cx.focus_handle();
                        window.focus(&focus, cx);
                        cx.spawn_in(window, async move |host, cx| {
                            loop {
                                cx.background_executor()
                                    .timer(Duration::from_millis(16))
                                    .await;
                                if host
                                    .update_in(cx, |host: &mut Host, window, cx| {
                                        let mut changed = false;
                                        while let Ok(frame) = host.bridge.incoming.try_recv() {
                                            changed = true;
                                            match frame {
                                                Ok(frame)
                                                    if host.frame.as_ref().is_none_or(|old| {
                                                        frame.revision > old.revision
                                                    }) =>
                                                {
                                                    host.frame = Some(frame);
                                                    host.error = None;
                                                }
                                                Ok(_) => {}
                                                Err(error) => {
                                                    host.error = Some(error.to_string());
                                                }
                                            }
                                        }
                                        if host.frame.as_ref().is_some_and(|frame| {
                                            frame.revision == host.rendered_revision.get()
                                        }) {
                                            while let Ok(request) = host.bridge.requests.try_recv()
                                            {
                                                host.handle_request(request, window, cx);
                                            }
                                        }
                                        if changed {
                                            cx.notify();
                                        }
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        })
                        .detach();
                        let clipboard = clipboard::Policy::install(bridge.outgoing.clone(), cx);
                        let resource_store = resources::Resources::install(cx);
                        cx.on_release(|host: &mut Host, cx| {
                            host.clipboard.release(cx);
                            host.resource_store.clear(cx);
                        })
                        .detach();
                        Host {
                            bridge,
                            frame: None,
                            rendered_revision: Default::default(),
                            error: None,
                            focus,
                            kit: Default::default(),
                            clipboard,
                            resource_store,
                            references: references::Registry::new(),
                        }
                    })
                },
            );
            match opened {
                Err(error) => {
                    eprintln!("Native window unavailable: {error}");
                    cx.quit();
                }
                Ok(_window) => {}
            }
            cx.activate(true);
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "capture")]
    #[gpui::test]
    fn native_mount_release_preserves_siblings_and_retires_actual_scroll_cache(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui_kit::layout::{scroll_offset, scroll_to};
        use gpui_kit_testkit::harness::Harness;
        let mut tree: Node = serde_json::from_value(json!({
            "kind":"column", "id":"root", "instance":7,
            "children":[
                {"kind":"text", "id":"first", "instance":7},
                {"kind":"text", "id":"second", "instance":7}
            ]
        }))
        .expect("mount fixture");
        let mut policy = clipboard::Policy::default();
        let mut harness = Harness::new(cx, gpui_kit::install, |_, _| div().into_any_element());
        harness.update(|window, cx| {
            policy.reconcile(&tree, &BTreeMap::new(), cx);
            let first_node = tree.children[0].clone();
            let first = policy.owner_of(&first_node).expect("first mount");
            let second = policy.owner_of(&tree.children[1]).expect("second mount");
            assert_ne!(first, second);
            let aliases = policy.resource_aliases(&BTreeMap::from([(7, true)]));
            assert_eq!(aliases[&first], aliases[&second]);
            assert_ne!(first, aliases[&first]);
            // Deliberately use the SAME native cache key: scope, not strings,
            // must isolate these actual window_state-backed scroll handles.
            for (owner, offset) in [(first, 37.0), (second, 19.0)] {
                cx.with_effect_owner(Some(owner), |cx| {
                    scroll_to("shared-cache", gpui::point(px(0.0), px(offset)), window, cx);
                });
            }
            policy.reconcile(&tree, &BTreeMap::new(), cx);
            assert_eq!(policy.owner_of(&first_node), Some(first));
            cx.with_effect_owner(Some(first), |cx| {
                assert_eq!(scroll_offset("shared-cache", window, cx).y, px(37.0));
            });
            tree.children.remove(0);
            policy.reconcile(&tree, &BTreeMap::new(), cx);
            assert!(!gpui_kit::foundation::owner_state_is_live(first, cx));
            cx.with_effect_owner(Some(first), |cx| {
                scroll_to("shared-cache", gpui::point(px(0.0), px(99.0)), window, cx);
                assert_eq!(scroll_offset("shared-cache", window, cx).y, px(0.0));
            });
            cx.with_effect_owner(Some(second), |cx| {
                assert_eq!(scroll_offset("shared-cache", window, cx).y, px(19.0));
            });
            tree.children.insert(0, first_node.clone());
            policy.reconcile(&tree, &BTreeMap::new(), cx);
            let replacement = policy.owner_of(&first_node).expect("replacement mount");
            assert_ne!(first, replacement);
            assert_eq!(policy.owner_of(&tree.children[1]), Some(second));
            cx.with_effect_owner(Some(replacement), |cx| {
                assert_eq!(scroll_offset("shared-cache", window, cx).y, px(0.0));
            });
            policy.release(cx);
            assert!(!gpui_kit::foundation::owner_state_is_live(second, cx));
            assert!(!gpui_kit::foundation::owner_state_is_live(replacement, cx));
        });
    }

    #[cfg(feature = "capture")]
    #[gpui::test]
    fn expired_factory_cannot_resurrect_native_state_or_retain_its_owner(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui_kit_testkit::harness::Harness;
        use std::{cell::Cell, rc::Rc};
        let kit = Rc::new(kit_bindings::KitState::default());
        let revision = Rc::new(Cell::new(1));
        let (outgoing, _events) = mpsc::sync_channel(8);
        let node: Node = serde_json::from_value(json!({"kind":"kit","component":"TextInput","id":"input","instance":7,"props":{"text":"fresh"}})).expect("input fixture");
        let mut clipboard = clipboard::Policy::default();
        cx.update(|cx| clipboard.reconcile(&node, &BTreeMap::new(), cx));
        let renderer = NodeRenderer {
            outgoing,
            kit: Rc::downgrade(&kit),
            rendered_revision: revision.clone(),
            clipboard,
        };
        let captured = node.clone();
        let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
            renderer.node(&captured, 1, window, cx)
        });
        harness.update(|window, cx| {
            assert_eq!(
                kit.invoke(&node, "value", &json!({}), true, window, cx)
                    .expect("mounted input"),
                json!("fresh")
            );
            let empty: Node = serde_json::from_value(json!({"kind":"column","id":"empty"}))
                .expect("empty fixture");
            kit.reconcile(&empty, cx);
        });
        revision.set(2);
        harness.frame();
        harness.update(|window, cx| {
            assert!(
                kit.invoke(&node, "value", &json!({}), true, window, cx)
                    .is_err()
            )
        });
        assert_eq!(
            harness.node("input").expect("expired view").role,
            Role::Text
        );
        let weak = Rc::downgrade(&kit);
        drop(kit);
        assert!(
            weak.upgrade().is_none(),
            "a retained renderer must not own KitState"
        );
    }

    #[test]
    fn native_protocol_validates_before_retaining() {
        let frame = b"{\"kind\":\"render\",\"generation\":0,\"revision\":3,\"tree\":{\"kind\":\"text\",\"id\":\"counter\",\"text\":\"seven\"}}\n";
        let parsed = read_frame(&mut &frame[..])
            .expect("valid frame")
            .expect("one frame");
        assert_eq!(parsed.tree.text, "seven");
        assert_eq!(parsed.revision, 3);
        assert!(read_frame(&mut &frame[..frame.len() - 1]).is_err());
        assert!(read_frame(&mut &vec![b'x'; MAX_FRAME as usize + 2][..]).is_err());
        let duplicate = br#"{"kind":"column","id":"x","children":[{"kind":"text","id":"x"}]}"#;
        let tree: Node = serde_json::from_slice(duplicate).expect("parse test tree");
        assert!(tree.validate(0, &mut HashSet::new()).is_err());
        assert!(serde_json::from_str::<Node>(r#"{"kind":"webview","id":"unsupported"}"#).is_err());
    }
}
