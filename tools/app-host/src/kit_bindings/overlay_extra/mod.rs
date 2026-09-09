//! Retained overlay entities and fresh native surface builders. Slot factories
//! and event routes are weakly captured; no entity can retain its host state.
use super::{Emit, KitSlots, Node};
use anyhow::{Result, bail, ensure};
use gpui::{
    AnyElement, App, AppContext as _, Edges, Entity, Focusable, IntoElement, SharedString,
    Subscription, Window, div, point, px,
};
use gpui_kit::prelude::*;
use gpui_kit_theme::{ActiveTheme, ControlSize, Elevation, Layer, Radius, Surface};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

type SlotFactory = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// Recursive identity validation after the shared closed MenuItem grammar.
pub(super) fn validate_menu_items(value: &Value) -> Result<()> {
    fn visit(value: &Value, ids: &mut std::collections::HashSet<String>) -> Result<()> {
        for item in array(value) {
            ensure!(ids.insert(s(item, "id")), "duplicate menu identity");
            visit(&item["items"], ids)?;
        }
        Ok(())
    }
    visit(value, &mut Default::default())
}
pub(super) fn validate_props(node: &Node) -> Result<()> {
    let p = Value::Object(node.props.clone());
    match node.component.as_deref() {
        Some("Menu" | "ContextMenu") => validate_menu_items(&p["items"]),
        Some("Menubar") => {
            for menu in array(&p["menus"]) {
                validate_menu_items(&menu["items"])?;
            }
            Ok(())
        }
        Some("Overlay") => {
            ensure!(
                p["constructor"] != "edge" || p.get("edge").is_some(),
                "edge constructor requires edge"
            );
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) const COMPONENTS: &[&str] = &[
    "CommandPalette",
    "ContextMenu",
    "Drawer",
    "Frost",
    "Glass",
    "HoverCard",
    "Kbd",
    "Menu",
    "Menubar",
    "NotificationCenter",
    "Overlay",
    "ToastLayer",
    "Tooltip",
];
fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or_default().to_owned()
}
fn opt(v: &Value, k: &str) -> Option<SharedString> {
    v[k].as_str().map(|s| s.to_owned().into())
}
fn n(v: &Value, k: &str, d: f32) -> f32 {
    v[k].as_f64().map_or(d, |n| n as f32)
}
fn b(v: &Value, k: &str, d: bool) -> bool {
    v[k].as_bool().unwrap_or(d)
}
fn u(v: &Value, k: &str, d: u64) -> u64 {
    v[k].as_u64().unwrap_or(d)
}
fn array(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}
fn edge(v: &Value) -> Edge {
    match v.as_str() {
        Some("top") => Edge::Top,
        Some("bottom") => Edge::Bottom,
        Some("left") => Edge::Left,
        _ => Edge::Right,
    }
}
fn placement(v: &Value) -> Placement {
    match v.as_str() {
        Some("above") => Placement::Above,
        Some("center") => Placement::Center,
        _ => {
            if v.get("at").is_some() {
                Placement::At(point(px(n(&v["at"], "x", 0.)), px(n(&v["at"], "y", 0.))))
            } else if v.get("edge").is_some() {
                Placement::Edge(edge(&v["edge"]))
            } else {
                Placement::Below
            }
        }
    }
}
fn hang(v: &Value) -> Hang {
    if v == "end" { Hang::End } else { Hang::Start }
}
fn size(v: &Value) -> ControlSize {
    match v.as_str() {
        Some("xs") => ControlSize::Xs,
        Some("md") => ControlSize::Md,
        Some("lg") => ControlSize::Lg,
        _ => ControlSize::Sm,
    }
}
fn radius(v: &Value) -> Radius {
    match v.as_str() {
        Some("small") => Radius::Small,
        Some("control") => Radius::Control,
        Some("dialog") => Radius::Dialog,
        Some("bubble") => Radius::Bubble,
        Some("pill") => Radius::Pill,
        _ => Radius::Card,
    }
}
fn surface(v: &Value) -> Surface {
    match v.as_str() {
        Some("backdrop") => Surface::Backdrop,
        Some("canvas") => Surface::Canvas,
        Some("sunken") => Surface::Sunken,
        Some("panel") => Surface::Panel,
        Some("raised") => Surface::Raised,
        _ => Surface::Overlay,
    }
}
fn corner(v: &Value) -> ToastCorner {
    match v.as_str() {
        Some("top_left") => ToastCorner::TopLeft,
        Some("top_right") => ToastCorner::TopRight,
        Some("bottom_left") => ToastCorner::BottomLeft,
        _ => ToastCorner::BottomRight,
    }
}
fn tone(v: &Value) -> Tone {
    match v.as_str() {
        Some("accent") => Tone::Accent,
        Some("success") => Tone::Success,
        Some("warning") => Tone::Warning,
        Some("danger") => Tone::Danger,
        Some("info") => Tone::Info,
        _ => Tone::Neutral,
    }
}
fn variant(v: &Value) -> ButtonVariant {
    match v.as_str() {
        Some("primary") => ButtonVariant::Primary,
        Some("secondary") => ButtonVariant::Secondary,
        Some("ghost") => ButtonVariant::Ghost,
        Some("danger") => ButtonVariant::Danger,
        Some("link") => ButtonVariant::Link,
        _ => ButtonVariant::Secondary,
    }
}
fn join(v: &Value) -> ButtonJoin {
    match v.as_str() {
        Some("leading") => ButtonJoin::Leading,
        Some("middle") => ButtonJoin::Middle,
        Some("trailing") => ButtonJoin::Trailing,
        _ => ButtonJoin::Alone,
    }
}
fn reserved(v: &Value) -> Edges<gpui::Pixels> {
    Edges {
        top: px(n(v, "top", 0.)),
        right: px(n(v, "right", 0.)),
        bottom: px(n(v, "bottom", 0.)),
        left: px(n(v, "left", 0.)),
    }
}
/// Builds native records after closed-schema and recursive identity validation.
/// Shared by menu-bearing bindings; never substitutes records for an `Entity<Menu>`.
pub(super) fn menu_items(v: &Value) -> Vec<MenuItem> {
    array(v)
        .map(|v| {
            let mut item = match v["kind"].as_str() {
                Some("check") => MenuItem::check(s(v, "id"), s(v, "label"), b(v, "checked", false)),
                Some("separator") => MenuItem::separator(s(v, "id")),
                Some("section") => MenuItem::section(s(v, "id"), s(v, "label")),
                Some("submenu") => {
                    MenuItem::submenu(s(v, "id"), s(v, "label"), menu_items(&v["items"]))
                }
                _ => MenuItem::command(s(v, "id"), s(v, "label")),
            }
            .disabled(b(v, "disabled", false))
            .destructive(b(v, "destructive", false));
            if let Some(v) = v.get("icon") {
                item = item.icon(super::icon::resolve(v).expect("validated icon"));
            }
            if v.get("shortcut").is_some() {
                item = item.shortcut(s(v, "shortcut"));
            }
            item
        })
        .collect()
}

/// The one native Menu data dispatcher, shared by retained nodes and entity refs.
/// Validates arguments, recursive item identities, and bounded results here.
/// The host/registry must additionally enforce source lifetime and parent disabled
/// policy; a Menu cannot infer whether its owning SplitButton is disabled.
pub(super) fn menu_value(
    target: &Entity<Menu>,
    method: &str,
    args: &Value,
    query: bool,
    window: &mut Window,
    cx: &mut App,
) -> Result<Value> {
    let schema = super::validation::invocation("Menu", method, args, query)?;
    let result = if query {
        let menu = target.read(cx);
        match method {
            "is_open" => json!(menu.is_open()),
            "offered" => json!(menu.offered().iter().map(|item| json!({"id":item.id().as_ref(),"label":item.label().as_ref(),"disabled":item.is_disabled(),"destructive":item.is_destructive()})).collect::<Vec<_>>()),
            _ => bail!("unsupported Menu query"),
        }
    } else {
        if method == "set_items" {
            validate_menu_items(&args["items"])?;
        }
        target.update(cx, |menu, cx| -> Result<Value> {
            match method {
                "open" => menu.open(window, cx),
                "close" => menu.close(window, cx),
                "toggle" => menu.toggle(window, cx),
                "dismiss" => menu.dismiss(window, cx),
                "open_submenu" => return Ok(json!(menu.open_submenu(&s(args, "id"), window, cx))),
                "set_items" => menu.set_items(menu_items(&args["items"]), cx),
                "set_trigger" => menu.set_trigger(s(args, "label"), cx),
                "set_trigger_name" => menu.set_trigger_name(s(args, "name"), cx),
                "set_trigger_icon" => menu.set_trigger_icon(
                    args.get("icon")
                        .filter(|v| !v.is_null())
                        .map(|v| super::icon::resolve(v).expect("validated icon")),
                    cx,
                ),
                "set_placement" => menu.set_placement(placement(&args["placement"]), cx),
                "set_hang" => menu.set_hang(hang(&args["hang"]), cx),
                "set_trigger_style" => {
                    menu.set_trigger_style(variant(&args["variant"]), size(&args["size"]), cx)
                }
                "set_trigger_join" => menu.set_trigger_join(join(&args["join"]), cx),
                _ => bail!("unsupported Menu command"),
            }
            Ok(Value::Null)
        })?
    };
    super::validation::validate(&result, schema)?;
    Ok(result)
}

fn menus(v: &Value) -> Vec<MenubarMenu> {
    array(v)
        .map(|v| {
            MenubarMenu::new(s(v, "id"), s(v, "label"), menu_items(&v["items"]))
                .disabled(b(v, "disabled", false))
        })
        .collect()
}
fn commands(v: &Value) -> Vec<Command> {
    array(v)
        .map(|v| {
            let mut c = Command::new(s(v, "id"), s(v, "label"));
            if v.get("section").is_some() {
                c = c.section(s(v, "section"));
            }
            if v.get("shortcut").is_some() {
                c = c.shortcut(s(v, "shortcut"));
            }
            if v.get("unavailable").is_some() {
                c = c.unavailable(s(v, "unavailable"));
            }
            c
        })
        .collect()
}
struct Route {
    events: std::collections::BTreeMap<String, String>,
    emit: Emit,
}
fn send(route: &Rc<RefCell<Route>>, name: &str, value: Value) {
    let (action, emit) = {
        let route = route.borrow();
        (route.events.get(name).cloned(), route.emit.clone())
    };
    if let Some(action) = action {
        emit(&action, value);
    }
}
enum Control {
    Drawer(Entity<Drawer>),
    Hover(Entity<HoverCard>),
    Menu(Entity<Menu>),
    Context(Entity<ContextMenu>),
    Menubar(Entity<Menubar>),
    Palette(Entity<CommandPalette>),
    Toast(Entity<ToastLayer>),
    Notifications(Entity<NotificationCenter>),
}
struct Entry {
    component: String,
    control: Control,
    props: RefCell<Value>,
    reference_props: RefCell<Value>,
    focus_stop_refs: RefCell<Value>,
    slot_data: RefCell<std::collections::BTreeMap<String, Vec<Node>>>,
    slots: Rc<RefCell<KitSlots>>,
    route: Rc<RefCell<Route>>,
    _subscriptions: Vec<Subscription>,
}
#[derive(Default)]
pub(super) struct State {
    entries: RefCell<HashMap<(u64, String), Rc<Entry>>>,
}
fn factory(store: &Rc<RefCell<KitSlots>>, name: &'static str) -> SlotFactory {
    let weak = Rc::downgrade(store);
    Rc::new(move |window, cx| {
        let f = weak
            .upgrade()
            .and_then(|store| store.borrow().get(name).cloned());
        f.map_or_else(|| div().into_any_element(), |f| f(window, cx))
    })
}

impl State {
    /// The input binding owner supplies its one fixed native TextInput dispatcher.
    /// No JS callback or family-local entity lookup is accepted here.
    pub(super) fn query_input(
        &self,
        node: &Node,
        cx: &App,
        refs: &crate::references::Registration<'_>,
        dispatch: crate::references::EntityDispatch<TextInput>,
    ) -> Result<Value> {
        let entry = self
            .entries
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("command palette is not mounted"))?;
        ensure!(
            node.component.as_deref() == Some("CommandPalette"),
            "native component mismatch"
        );
        let Control::Palette(palette) = &entry.control else {
            bail!("native component mismatch");
        };
        let input = palette.read(cx).query_input().clone();
        refs.entity(
            "TextInput",
            palette,
            &input,
            |palette, _| Some(palette.query_input().clone()),
            |_, _| true,
            dispatch,
        )
    }

    /// Registry anchors use the actual retained entity, not a serialized handle.
    pub(super) fn native_entity_id(&self, node: &Node) -> Option<gpui::EntityId> {
        let entry = self
            .entries
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .cloned()?;
        Some(match &entry.control {
            Control::Drawer(v) => v.entity_id(),
            Control::Hover(v) => v.entity_id(),
            Control::Menu(v) => v.entity_id(),
            Control::Context(v) => v.entity_id(),
            Control::Menubar(v) => v.entity_id(),
            Control::Palette(v) => v.entity_id(),
            Control::Toast(v) => v.entity_id(),
            Control::Notifications(v) => v.entity_id(),
        })
    }

    /// Called by the host after render under the mounted node's registration.
    /// Removing the option clears stops; unrelated renders preserve imperative changes.
    pub(super) fn apply_reference_props(
        &self,
        node: &Node,
        cx: &mut App,
        refs: &crate::references::Registration<'_>,
    ) -> Result<()> {
        if node.component.as_deref() != Some("Drawer") {
            return Ok(());
        }
        let entry = self
            .entries
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("drawer is not mounted"))?;
        let value = node
            .props
            .get("focus_stops")
            .cloned()
            .unwrap_or(Value::Null);
        let Control::Drawer(drawer) = &entry.control else {
            bail!("native component mismatch");
        };
        let changed = *entry.reference_props.borrow() != value;
        let effective = if changed {
            value.clone()
        } else {
            entry.focus_stop_refs.borrow().clone()
        };
        // Revalidate unchanged references too: another mounted source can disappear.
        let stops = match array(&effective)
            .map(|value| refs.borrow_focus(value, cx))
            .collect::<Result<Vec<_>>>()
        {
            Ok(stops) => stops,
            Err(error) => {
                drawer.update(cx, |drawer, cx| drawer.set_focus_stops([], cx));
                *entry.reference_props.borrow_mut() = Value::Null;
                *entry.focus_stop_refs.borrow_mut() = Value::Null;
                return Err(error);
            }
        };
        if changed {
            drawer.update(cx, |drawer, cx| drawer.set_focus_stops(stops, cx));
            *entry.reference_props.borrow_mut() = value;
            *entry.focus_stop_refs.borrow_mut() = effective;
        }
        Ok(())
    }

    pub(super) fn invoke_reference(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        cx: &mut App,
        refs: &crate::references::Registration<'_>,
    ) -> Result<Value> {
        let entry = self
            .entries
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("native overlay is not mounted"))?;
        ensure!(
            node.component.as_deref() == Some(entry.component.as_str()),
            "native component mismatch"
        );
        if !query && method == "set_focus_stops" {
            let Control::Drawer(drawer) = &entry.control else {
                bail!("focus stops require Drawer");
            };
            let stops = array(&args["stops"])
                .map(|value| refs.borrow_focus(value, cx))
                .collect::<Result<Vec<_>>>()?;
            drawer.update(cx, |drawer, cx| drawer.set_focus_stops(stops, cx));
            *entry.focus_stop_refs.borrow_mut() = args["stops"].clone();
            return Ok(Value::Null);
        }
        ensure!(
            query && method == "focus_handle",
            "unsupported overlay reference method"
        );
        fn current<T: Focusable>(value: &T, handle: &gpui::FocusHandle, cx: &App) -> bool {
            value.focus_handle(cx) == *handle
        }
        macro_rules! focus {
            ($entity:expr) => {
                refs.focus(
                    $entity,
                    &$entity.read(cx).focus_handle(cx),
                    current,
                    |_, _| true,
                )
            };
        }
        match &entry.control {
            Control::Drawer(v) => focus!(v),
            Control::Hover(v) => focus!(v),
            Control::Menu(v) => focus!(v),
            Control::Context(v) => focus!(v),
            Control::Palette(v) => focus!(v),
            Control::Notifications(v) => focus!(v),
            _ => bail!("overlay has no Focusable contract"),
        }
    }

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
        self.entries
            .borrow_mut()
            .retain(|key, entry| live.get(key) == Some(&entry.component));
    }
    pub(super) fn render(
        &self,
        node: &Node,
        slots: KitSlots,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let component = node.component.as_deref().unwrap_or_default();
        if matches!(component, "Frost" | "Glass" | "Kbd" | "Overlay" | "Tooltip") {
            return render(node, slots, window, cx, emit);
        }
        let key = (node.instance, node.id.clone());
        let existing = { self.entries.borrow().get(&key).cloned() };
        let entry = existing.unwrap_or_else(|| {
            let store = Rc::new(RefCell::new(KitSlots::new()));
            let route = Rc::new(RefCell::new(Route {
                events: node.events.clone(),
                emit: emit.clone(),
            }));
            let mut subscriptions = Vec::new();
            macro_rules! subscribe {
                ($entity:expr,$event:ty,$body:expr) => {{
                    let weak = Rc::downgrade(&route);
                    subscriptions.push(cx.subscribe(&$entity, move |_, event: &$event, _| {
                        if let Some(route) = weak.upgrade() {
                            let (name, value) = $body(event);
                            send(&route, name, value);
                        }
                    }));
                }};
            }
            let control = match component {
                "Drawer" => {
                    let body = factory(&store, "content");
                    let footer = factory(&store, "footer");
                    let e = cx.new(|cx| {
                        Drawer::new(node.id.clone(), window, cx)
                            .content(move |w, c| body(w, c))
                            .footer(move |w, c| footer(w, c))
                    });
                    subscribe!(e, DrawerEvent, |e: &DrawerEvent| match e {
                        DrawerEvent::Opened => ("open", Value::Null),
                        DrawerEvent::Closed => ("close", Value::Null),
                        DrawerEvent::Dismissed => ("dismiss", Value::Null),
                        DrawerEvent::ResizeRequested(v) => ("resize_requested", json!(v)),
                    });
                    Control::Drawer(e)
                }
                "HoverCard" => {
                    let body = factory(&store, "content");
                    let trigger = factory(&store, "trigger");
                    let e = cx.new(|cx| {
                        HoverCard::new(node.id.clone(), window, cx)
                            .content(move |w, c| body(w, c))
                            .trigger(move |w, c| trigger(w, c))
                    });
                    subscribe!(e, HoverCardEvent, |e: &HoverCardEvent| match e {
                        HoverCardEvent::Opened => ("open", Value::Null),
                        HoverCardEvent::Closed => ("close", Value::Null),
                    });
                    Control::Hover(e)
                }
                "Menu" => {
                    let e = cx.new(|cx| Menu::new(node.id.clone(), window, cx));
                    subscribe!(e, MenuEvent, |e: &MenuEvent| match e {
                        MenuEvent::Opened => ("open", Value::Null),
                        MenuEvent::Closed => ("close", Value::Null),
                        MenuEvent::Dismissed => ("dismiss", Value::Null),
                        MenuEvent::Invoked(v) => ("invoked", json!(v.as_ref())),
                    });
                    Control::Menu(e)
                }
                "ContextMenu" => {
                    let body = factory(&store, "content");
                    let e = cx.new(|cx| {
                        ContextMenu::new(node.id.clone(), window, cx)
                            .content(move |w, c| body(w, c))
                    });
                    subscribe!(e, ContextMenuEvent, |e: &ContextMenuEvent| match e {
                        ContextMenuEvent::Opened(v) => ("open", json!(v.as_ref())),
                        ContextMenuEvent::Closed => ("close", Value::Null),
                        ContextMenuEvent::Dismissed => ("dismiss", Value::Null),
                        ContextMenuEvent::Invoked(v) => ("invoked", json!(v.as_ref())),
                        ContextMenuEvent::Unavailable(v) => ("unavailable", json!(v.as_ref())),
                    });
                    Control::Context(e)
                }
                "Menubar" => {
                    let e = cx.new(|cx| Menubar::new(node.id.clone(), [], window, cx));
                    subscribe!(e, MenubarEvent, |e: &MenubarEvent| match e {
                        MenubarEvent::Opened(v) => ("open", json!(v.as_ref())),
                        MenubarEvent::Closed(v) => ("close", json!(v.as_ref())),
                        MenubarEvent::Invoked { menu, item } => (
                            "invoked",
                            json!({"menu":menu.as_ref(),"item":item.as_ref()})
                        ),
                    });
                    Control::Menubar(e)
                }
                "CommandPalette" => {
                    let empty = factory(&store, "empty");
                    let e = cx.new(|cx| {
                        CommandPalette::new(node.id.clone(), window, cx)
                            .slot("empty", move |w, c| empty(w, c))
                    });
                    subscribe!(e, CommandPaletteEvent, |e: &CommandPaletteEvent| match e {
                        CommandPaletteEvent::QueryChanged(v) =>
                            ("query_changed", json!(v.as_ref())),
                        CommandPaletteEvent::Invoked(v) => ("invoked", json!(v.as_ref())),
                        CommandPaletteEvent::Dismissed => ("dismiss", Value::Null),
                    });
                    Control::Palette(e)
                }
                "ToastLayer" => Control::Toast(cx.new(|cx| ToastLayer::new(window, cx))),
                "NotificationCenter" => {
                    let empty = factory(&store, "empty");
                    let e = cx.new(|cx| {
                        NotificationCenter::new(node.id.clone(), cx)
                            .slot("empty", move |w, c| empty(w, c))
                    });
                    subscribe!(
                        e,
                        NotificationCenterEvent,
                        |e: &NotificationCenterEvent| match e {
                            NotificationCenterEvent::Read(v) => ("read", json!(v.as_ref())),
                            NotificationCenterEvent::Dismissed(v) =>
                                ("dismissed", json!(v.as_ref())),
                            NotificationCenterEvent::Cleared => ("cleared", Value::Null),
                            NotificationCenterEvent::ActionTaken(v) =>
                                ("action_taken", json!(v.as_ref())),
                        }
                    );
                    Control::Notifications(e)
                }
                _ => unreachable!("validated overlay entity"),
            };
            let entry = Rc::new(Entry {
                component: component.to_owned(),
                control,
                props: RefCell::new(Value::Null),
                reference_props: RefCell::new(Value::Null),
                focus_stop_refs: RefCell::new(Value::Null),
                slot_data: Default::default(),
                slots: store,
                route,
                _subscriptions: subscriptions,
            });
            self.entries.borrow_mut().insert(key, entry.clone());
            entry
        });
        *entry.slots.borrow_mut() = slots;
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
        };
        let p = Value::Object(node.props.clone());
        let old = entry.props.borrow().clone();
        let slots_changed = *entry.slot_data.borrow() != node.slots;
        let mut applied = true;
        let changed = |key: &str| old.is_null() || old.get(key) != p.get(key);
        macro_rules! update {($e:expr,|$v:ident,$c:ident|$body:block)=>{{$e.update(cx,|$v,$c|{ $body if slots_changed { $c.notify(); }});$e.clone().into_any_element()}};}
        let result = match &entry.control {
            Control::Drawer(e) => update!(e, |v, c| {
                if changed("title") {
                    v.set_title(s(&p, "title"), c);
                }
                if changed("description") {
                    v.set_description(opt(&p, "description"), c);
                }
                if changed("edge") {
                    v.set_edge(edge(&p["edge"]), c);
                }
                if changed("size") {
                    v.set_size(n(&p, "size", 360.), c);
                }
                if changed("dismissable") {
                    v.set_dismissable(b(&p, "dismissable", true), c);
                }
                if changed("resizable") {
                    v.set_resizable(b(&p, "resizable", false), c);
                }
            }),
            Control::Hover(e) => update!(e, |v, c| {
                if changed("name") {
                    v.set_name(opt(&p, "name"), c);
                }
                if changed("placement") {
                    v.set_placement(placement(&p["placement"]), c);
                }
                if changed("hang") {
                    v.set_hang(hang(&p["hang"]), c);
                }
                if changed("open_delay") {
                    v.set_open_delay(
                        Duration::from_millis(u(
                            &p,
                            "open_delay",
                            c.theme().motion.hover_card_open_ms,
                        )),
                        c,
                    );
                }
                if changed("grace") {
                    v.set_grace(
                        Duration::from_millis(u(&p, "grace", c.theme().motion.hover_card_grace_ms)),
                        c,
                    );
                }
            }),
            Control::Menu(e) => update!(e, |v, c| {
                if changed("trigger") {
                    v.set_trigger(s(&p, "trigger"), c);
                }
                if changed("trigger_name")
                    || (changed("trigger") && p.get("trigger_name").is_none())
                {
                    v.set_trigger_name(
                        s(
                            &p,
                            if p.get("trigger_name").is_some() {
                                "trigger_name"
                            } else {
                                "trigger"
                            },
                        ),
                        c,
                    );
                }
                if changed("trigger_icon") {
                    v.set_trigger_icon(
                        p.get("trigger_icon")
                            .map(|v| super::icon::resolve(v).expect("validated icon")),
                        c,
                    );
                }
                if changed("items") {
                    v.set_items(menu_items(&p["items"]), c);
                }
                if changed("placement") {
                    v.set_placement(placement(&p["placement"]), c);
                }
                if changed("hang") {
                    v.set_hang(hang(&p["hang"]), c);
                }
                if changed("trigger_variant") || changed("control_size") {
                    v.set_trigger_style(
                        variant(&p["trigger_variant"]),
                        p.get("control_size").map_or(ControlSize::Md, size),
                        c,
                    );
                }
                if changed("trigger_join") {
                    v.set_trigger_join(join(&p["trigger_join"]), c);
                }
            }),
            Control::Context(e) => update!(e, |v, c| {
                let mut apply = |result: std::result::Result<(), gpui::NativeMenuError>| {
                    if let Err(error) = result {
                        applied = false;
                        send(&entry.route, "unavailable", json!(error.to_string()));
                    }
                };
                if changed("name") {
                    apply(v.set_name(s(&p, "name"), window, c));
                }
                if changed("target") {
                    apply(v.set_target(opt(&p, "target"), window, c));
                }
                if changed("items") {
                    apply(v.set_items(menu_items(&p["items"]), window, c));
                }
                if slots_changed {
                    apply(v.set_content(Some(factory(&entry.slots, "content")), window, c));
                }
                if changed("presentation") {
                    apply(v.set_presentation(
                        if p["presentation"] == "in_window" {
                            ContextMenuPresentation::InWindow
                        } else {
                            ContextMenuPresentation::Native
                        },
                        window,
                        c,
                    ));
                }
            }),
            Control::Menubar(e) => update!(e, |v, c| {
                if changed("menus") {
                    v.set_menus(menus(&p["menus"]), window, c);
                }
                if changed("control_size") {
                    v.set_control_size(size(&p["control_size"]), c);
                }
            }),
            Control::Palette(e) => update!(e, |v, c| {
                if changed("commands") {
                    v.set_commands(commands(&p["commands"]), c);
                }
                if changed("query") {
                    v.set_query(s(&p, "query"), c);
                }
            }),
            Control::Toast(e) => update!(e, |v, c| {
                if changed("corner") {
                    v.set_corner(corner(&p["corner"]), c);
                }
                if changed("capacity") {
                    v.set_capacity(u(&p, "capacity", 3) as usize, c);
                }
                if changed("reserved_edges") {
                    v.set_reserved_edges(reserved(&p["reserved_edges"]), c);
                }
            }),
            Control::Notifications(e) => update!(e, |v, c| {
                if changed("capacity") {
                    v.set_capacity(u(&p, "capacity", 50) as usize, c);
                }
                if changed("control_size") {
                    v.set_control_size(size(&p["control_size"]), c);
                }
            }),
        };
        if applied {
            *entry.props.borrow_mut() = p;
            *entry.slot_data.borrow_mut() = node.slots.clone();
        }
        result
    }
    pub(super) fn invoke(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Value> {
        if !query && method == "set_items" && node.component.as_deref() != Some("Menu") {
            validate_menu_items(&args["items"])?;
        }
        if !query && method == "set_menus" {
            for menu in array(&args["menus"]) {
                validate_menu_items(&menu["items"])?;
            }
        }
        if node.component.as_deref() == Some("Kbd") && query && method == "caps" {
            return Ok(json!(
                Kbd::new(super::text(node, "keystroke"))
                    .caps(cx)
                    .iter()
                    .map(|s| s.as_ref())
                    .collect::<Vec<_>>()
            ));
        }
        let entry = self
            .entries
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("native overlay is not mounted"))?;
        ensure!(
            node.component.as_deref() == Some(entry.component.as_str()),
            "native component mismatch"
        );
        let slot = || -> Result<Option<SlotFactory>> {
            if args["slot"].is_null() {
                return Ok(None);
            }
            let name = match args["slot"].as_str() {
                Some("content") => "content",
                Some("footer") => "footer",
                Some("trigger") => "trigger",
                Some("empty") => "empty",
                _ => bail!("unknown slot"),
            };
            ensure!(
                entry.slots.borrow().contains_key(name),
                "slot is not currently mounted on this node"
            );
            Ok(Some(factory(&entry.slots, name)))
        };
        macro_rules! command {
            ($e:expr,|$v:ident,$c:ident|$body:block) => {{ $e.update(cx, |$v, $c| -> Result<Value> { $body }) }};
        }
        match (&entry.control, query) {
            (Control::Drawer(e), true) => Ok(match method {
                "is_open" => json!(e.read(cx).is_open()),
                "is_dismissable" => json!(e.read(cx).is_dismissable()),
                "is_rendered" => json!(e.read(cx).is_rendered()),
                _ => bail!("unsupported Drawer query"),
            }),
            (Control::Drawer(e), false) => command!(e, |v, c| {
                match method {
                    "open" => v.open(window, c),
                    "close" => v.close(window, c),
                    "dismiss" => {
                        ensure!(v.is_dismissable(), "drawer refuses dismissal");
                        v.dismiss(window, c)
                    }
                    "settle" => v.settle(c),
                    "set_title" => v.set_title(s(args, "title"), c),
                    "set_description" => v.set_description(opt(args, "description"), c),
                    "set_size" => v.set_size(n(args, "size", 0.), c),
                    "set_edge" => v.set_edge(edge(&args["edge"]), c),
                    "set_dismissable" => v.set_dismissable(b(args, "dismissable", true), c),
                    "set_resizable" => v.set_resizable(b(args, "resizable", false), c),
                    "set_content" => v.set_content(slot()?, c),
                    "set_footer" => v.set_footer(slot()?, c),
                    _ => bail!("unsupported Drawer command"),
                };
                Ok(Value::Null)
            }),
            (Control::Hover(e), true) => Ok(match method {
                "is_open" => json!(e.read(cx).is_open()),
                "is_leaving" => json!(e.read(cx).is_leaving()),
                "grace_period" => json!(e.read(cx).grace_period().as_millis() as u64),
                _ => bail!("unsupported HoverCard query"),
            }),
            (Control::Hover(e), false) => command!(e, |v, c| {
                match method {
                    "open" => v.open(c),
                    "close" => v.close(c),
                    "dismiss" => v.dismiss(window, c),
                    "set_name" => v.set_name(opt(args, "name"), c),
                    "set_placement" => v.set_placement(placement(&args["placement"]), c),
                    "set_hang" => v.set_hang(hang(&args["hang"]), c),
                    "set_open_delay" => {
                        v.set_open_delay(Duration::from_millis(u(args, "delay", 0)), c)
                    }
                    "set_grace" => v.set_grace(Duration::from_millis(u(args, "grace", 0)), c),
                    "set_content" => v.set_content(slot()?, c),
                    "set_trigger" => v.set_trigger(slot()?, c),
                    _ => bail!("unsupported HoverCard command"),
                };
                Ok(Value::Null)
            }),
            (Control::Menu(e), query) => menu_value(e,method,args,query,window,cx),
            (Control::Context(e), true) => Ok(match method {
                "is_open" => json!(e.read(cx).is_open()),
                "position" => {
                    let p = e.read(cx).position();
                    json!({"x":f32::from(p.x),"y":f32::from(p.y)})
                }
                _ => bail!("unsupported ContextMenu query"),
            }),
            (Control::Context(e), false) => command!(e, |v, c| {
                match method {
                    "open_at" => v.open_at(
                        point(
                            px(n(&args["position"], "x", 0.)),
                            px(n(&args["position"], "y", 0.)),
                        ),
                        window,
                        c,
                    ),
                    "close" => v.close(window, c),
                    "dismiss" => v.dismiss(window, c),
                    "set_items" => v.set_items(menu_items(&args["items"]), window, c),
                    "set_name" => v.set_name(s(args, "name"), window, c),
                    "set_target" => v.set_target(opt(args, "target"), window, c),
                    "set_content" => v.set_content(slot()?, window, c),
                    "set_presentation" => v.set_presentation(
                        if args["presentation"] == "native" {
                            ContextMenuPresentation::Native
                        } else {
                            ContextMenuPresentation::InWindow
                        },
                        window,
                        c,
                    ),
                    _ => bail!("unsupported ContextMenu command"),
                }?;
                Ok(Value::Null)
            }),
            (Control::Menubar(e), true) => Ok(match method {
                "open_menu" => json!(e.read(cx).open_menu().map(|s| s.as_ref())),
                "menus" => json!(e.read(cx).menus().iter().map(|item|json!({"id":item.id().as_ref(),"label":item.label().as_ref(),"disabled":item.is_disabled()})).collect::<Vec<_>>()),
                _ => bail!("unsupported Menubar query"),
            }),
            (Control::Menubar(e), false) => command!(e, |v, c| {
                match method {
                    "open" => {
                        let id = s(args, "id");
                        ensure!(
                            v.menus()
                                .iter()
                                .any(|m| m.id().as_ref() == id && !m.is_disabled()),
                            "menu is unavailable"
                        );
                        v.open(&id, window, c)
                    }
                    "close" => v.close(window, c),
                    "set_menus" => v.set_menus(menus(&args["menus"]), window, c),
                    "set_control_size" => v.set_control_size(size(&args["size"]), c),
                    _ => bail!("unsupported Menubar command"),
                };
                Ok(Value::Null)
            }),
            (Control::Palette(e), true) => Ok(match method {
                "query" => json!(e.read(cx).query(cx).as_ref()),
                "active_id" => json!(e.read(cx).active_id(cx).map(|s| s.to_string())),
                _ => bail!("unsupported CommandPalette query"),
            }),
            (Control::Palette(e), false) => command!(e, |v, c| {
                match method {
                    "set_commands" => v.set_commands(commands(&args["commands"]), c),
                    "set_query" => v.set_query(s(args, "query"), c),
                    "focus_query" => v.focus_query(window, c),
                    _ => bail!("unsupported CommandPalette command"),
                };
                Ok(Value::Null)
            }),
            (Control::Toast(e), true) => Ok(match method {
                "len" => json!(e.read(cx).len()),
                "is_empty" => json!(e.read(cx).is_empty()),
                "phase" => json!(
                    e.read(cx)
                        .phase(&s(args, "ident"))
                        .map(|p| format!("{p:?}").to_lowercase())
                ),
                _ => bail!("unsupported ToastLayer query"),
            }),
            (Control::Toast(e), false) => command!(e, |v, c| {
                match method {
                    "push" => v.push(toast(&args["toast"], &entry.route), c),
                    "dismiss" => return Ok(json!(v.dismiss(&s(args, "ident"), c))),
                    "clear" => v.clear(c),
                    "set_corner" => v.set_corner(corner(&args["corner"]), c),
                    "set_capacity" => v.set_capacity(u(args, "capacity", 1) as usize, c),
                    "set_reserved_edges" => v.set_reserved_edges(reserved(&args["edges"]), c),
                    _ => bail!("unsupported ToastLayer command"),
                };
                Ok(Value::Null)
            }),
            (Control::Notifications(e), true) => Ok(match method {
                "len" => json!(e.read(cx).len()),
                "is_empty" => json!(e.read(cx).is_empty()),
                "holds" => json!(e.read(cx).holds(&s(args, "id"))),
                "is_read" => json!(e.read(cx).is_read(&s(args, "id"))),
                "unread" => {
                    let unread = e.read(cx).unread();
                    json!({"kind":unread.name(),"value":unread.value()})
                }
                _ => bail!("unsupported NotificationCenter query"),
            }),
            (Control::Notifications(e), false) => command!(e, |v, c| {
                match method {
                    "record" => v.record(notification(&args["notification"], &entry.route), c),
                    "show" => {
                        return Ok(json!(v.show(
                            notification(&args["notification"], &entry.route),
                            window,
                            c
                        )));
                    }
                    "mark_read" => return Ok(json!(v.mark_read(&s(args, "id"), c))),
                    "mark_all_read" => v.mark_all_read(c),
                    "dismiss" => return Ok(json!(v.dismiss(&s(args, "id"), c))),
                    "clear" => v.clear(c),
                    "set_capacity" => v.set_capacity(u(args, "capacity", 1) as usize, c),
                    "set_control_size" => v.set_control_size(size(&args["size"]), c),
                    _ => bail!("unsupported NotificationCenter command"),
                };
                Ok(Value::Null)
            }),
        }
    }
}
fn toast(p: &Value, route: &Rc<RefCell<Route>>) -> Toast {
    let mut v = Toast::new(s(p, "id"), s(p, "message"))
        .tone(tone(&p["tone"]))
        .dismissable(b(p, "dismissable", true));
    if p.get("detail").is_some() {
        v = v.detail(s(p, "detail"));
    }
    if let Some(ms) = p["timeout"].as_u64() {
        v = v.timeout(Duration::from_millis(ms));
    }
    if b(p, "persistent", false) {
        v = v.persistent();
    }
    if p.get("action").is_some() {
        let weak = Rc::downgrade(route);
        let id = s(p, "id");
        v = v.action(s(p, "action"), move |_, _| {
            if let Some(route) = weak.upgrade() {
                send(&route, "action", json!(id));
            }
        });
    }
    v
}
fn notification(p: &Value, route: &Rc<RefCell<Route>>) -> Notification {
    let mut v = Notification::new(s(p, "id"), s(p, "message"))
        .tone(tone(&p["tone"]))
        .read(b(p, "read", false));
    if p.get("detail").is_some() {
        v = v.detail(s(p, "detail"));
    }
    if p.get("at").is_some() {
        v = v.at(s(p, "at"));
    }
    if p.get("action").is_some() {
        let weak = Rc::downgrade(route);
        let id = s(p, "id");
        v = v.action(s(p, "action"), move |_, _| {
            if let Some(route) = weak.upgrade() {
                send(&route, "action", json!(id));
            }
        });
    }
    v
}

pub(super) fn render(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
) -> AnyElement {
    let p = Value::Object(node.props.clone());
    let child = || slots.get("content").cloned();
    match node.component.as_deref().unwrap_or_default() {
        "Kbd" => Kbd::new(s(&p, "keystroke"))
            .id(node.id.clone())
            .into_any_element(),
        "Tooltip" => {
            let mut v = Tooltip::new(node.id.clone(), s(&p, "text"));
            if p.get("describes").is_some() {
                v = v.describes(s(&p, "describes"));
            }
            v.into_any_element()
        }
        "Frost" => {
            let mut v = Frost::new(node.id.clone());
            if p.get("surface").is_some() {
                v = v.surface(surface(&p["surface"]));
            }
            if p.get("radius").is_some() {
                v = v.radius(radius(&p["radius"]));
            }
            if p.get("blur").is_some() {
                v = v.blur(n(&p, "blur", 0.));
            }
            if let Some(f) = child() {
                v = v.child(f(window, cx));
            }
            v.into_any_element()
        }
        "Glass" => {
            let mut v = Glass::new(node.id.clone());
            if p.get("surface").is_some() {
                v = v.surface(surface(&p["surface"]));
            }
            if p.get("radius").is_some() {
                v = v.radius(radius(&p["radius"]));
            }
            if p.get("preset").is_some() {
                v = v.preset(super::canvas::glass(&p["preset"]));
            }
            if p.get("elevation").is_some() {
                v = v.elevation(match p["elevation"].as_str() {
                    Some("raised") => Elevation::Raised,
                    Some("overlay") => Elevation::Overlay,
                    Some("modal") => Elevation::Modal,
                    _ => Elevation::Flat,
                });
            }
            macro_rules! numeric{($($key:ident),*)=>{$(if p.get(stringify!($key)).is_some(){v=v.$key(n(&p,stringify!($key),0.));})*};}
            numeric!(
                radius_px,
                blur,
                refraction,
                dispersion,
                specular,
                light_angle
            );
            macro_rules! boolean{($($key:ident),*)=>{$(if p.get(stringify!($key)).is_some(){v=v.$key(b(&p,stringify!($key),false));})*};}
            boolean!(
                focused,
                track_pointer,
                pressable,
                adaptive,
                adaptive_appearance,
                dimmed
            );
            if let Some(t) = p.get("tint") {
                v = v.tint(gpui::hsla(
                    n(t, "h", 0.),
                    n(t, "s", 0.),
                    n(t, "l", 0.),
                    n(t, "a", 1.),
                ));
            }
            if let Some(e) = p.get("edge_mask") {
                v = v.edge_mask(
                    match e["edge"].as_str() {
                        Some("top") => gpui::GlassEdge::Top,
                        Some("bottom") => gpui::GlassEdge::Bottom,
                        Some("left") => gpui::GlassEdge::Left,
                        Some("right") => gpui::GlassEdge::Right,
                        _ => gpui::GlassEdge::None,
                    },
                    n(e, "band", 0.),
                );
            }
            if let Some(f) = child() {
                v = v.child(f(window, cx));
            }
            v.into_any_element()
        }
        "Overlay" => {
            let mut v = match p["constructor"].as_str() {
                Some("modal") => Overlay::modal(node.id.clone()),
                Some("edge") => Overlay::edge(node.id.clone(), edge(&p["edge"])),
                _ => Overlay::new(node.id.clone()),
            };
            if p.get("placement").is_some() {
                v = v.placement(placement(&p["placement"]));
            }
            if p.get("hang").is_some() {
                v = v.hang(hang(&p["hang"]));
            }
            if p.get("scrim").is_some() {
                v = v.scrim(b(&p, "scrim", false));
            }
            if p.get("progress").is_some() {
                v = v.progress(n(&p, "progress", 1.));
            }
            if p.get("stack").is_some() {
                v = v.stack(u(&p, "stack", 0) as usize);
            }
            if p.get("layer").is_some() {
                v = v.layer(match p["layer"].as_str() {
                    Some("content") => Layer::Content,
                    Some("sticky") => Layer::Sticky,
                    Some("dock") => Layer::Dock,
                    Some("tooltip") => Layer::Tooltip,
                    Some("modal") => Layer::Modal,
                    Some("toast") => Layer::Toast,
                    _ => Layer::Popover,
                });
            }
            if let Some(f) = child() {
                v = v.child(f(window, cx));
            }
            if let Some(action) = node.events.get("dismiss").cloned() {
                v = v.on_dismiss(move |_, _| emit(&action, Value::Null));
            }
            v.into_any_element()
        }
        _ => unreachable!("retained overlays require State::render"),
    }
}

#[cfg(all(test, feature = "capture"))]
mod tests;
