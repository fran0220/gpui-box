//! Native layout builders over validated caller-owned records and fresh slots.
use super::navigation_extra::Overflow;
use super::*;
use anyhow::Result;
use gpui::{Bounds, div, point, size as extent};
use gpui_kit::layout::*;
use gpui_kit_theme::Space;

#[derive(Default)]
pub(super) struct State {
    overflow: RefCell<HashMap<Key, Rc<Overflow>>>,
}
impl State {
    pub(super) fn reconcile(&self, root: &Node, _: &mut App) {
        fn visit(node: &Node, live: &mut std::collections::HashSet<Key>) {
            if node.component.as_deref() == Some("Toolbar") && flag(node, "overflow") {
                live.insert((node.instance, node.id.clone()));
            }
            for child in node.children.iter().chain(node.slots.values().flatten()) {
                visit(child, live);
            }
        }
        let mut live = std::collections::HashSet::new();
        visit(root, &mut live);
        self.overflow
            .borrow_mut()
            .retain(|key, _| live.contains(key));
    }
    pub(super) fn render(
        &self,
        node: &Node,
        slots: KitSlots,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        if node.component.as_deref() != Some("Toolbar") || !flag(node, "overflow") {
            return render(node, slots, window, cx, emit);
        }
        let existing = self
            .overflow
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .cloned();
        let entry = existing.unwrap_or_else(|| {
            let entry = Overflow::new(node, "overflowSelect", window, cx, emit.clone());
            self.overflow
                .borrow_mut()
                .insert((node.instance, node.id.clone()), entry.clone());
            entry
        });
        entry.update(node, emit.clone());
        render_with_menu(node, slots, window, cx, emit, Some(entry.menu.clone()))
    }
    pub(super) fn invoke(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Result<Value> {
        anyhow::ensure!(
            query && args.as_object().is_some_and(|a| a.is_empty()),
            "layout builders expose only argument-free native queries"
        );
        match (node.component.as_deref(), method) {
            (Some("AspectRatio"), "ratio") => Ok(json!(
                AspectRatio::new(node.id.clone(), number(node, "ratio", 1.)).ratio()
            )),
            (Some("Toolbar"), "item_count") => {
                let v = Value::Object(node.props.clone());
                let mut toolbar = Toolbar::new(node.id.clone());
                for group in rows(&v, "groups") {
                    toolbar = toolbar.group(
                        s(group, "id"),
                        rows(&v, "items")
                            .filter(|i| i["group"] == group["id"])
                            .map(|i| ToolbarItem::new(s(i, "id"), s(i, "label"), div())),
                    );
                }
                Ok(json!(toolbar.item_count()))
            }
            _ => anyhow::bail!("unsupported native layout query"),
        }
    }
}

pub(super) const COMPONENTS: &[&str] = &[
    "AspectRatio",
    "Container",
    "DesktopTitlebar",
    "Dock",
    "DockTree",
    "Grid",
    "Responsive",
    "ScrollEdgeEffect",
    "ScrollFade",
    "SplitTree",
    "StatusBar",
    "Toolbar",
];
fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or_default().to_owned()
}
fn b(v: &Value, k: &str) -> bool {
    v[k].as_bool().unwrap_or(false)
}
fn n(v: &Value, k: &str, d: f32) -> f32 {
    v[k].as_f64().map_or(d, |n| n as f32)
}
fn rows<'a>(v: &'a Value, k: &str) -> impl Iterator<Item = &'a Value> {
    v[k].as_array().into_iter().flatten()
}
fn optional(v: &Value, k: &str) -> Option<SharedString> {
    v[k].as_str().map(|s| s.to_owned().into())
}
fn slot(slots: &KitSlots, k: &str, w: &mut Window, cx: &mut App) -> AnyElement {
    slots
        .get(k)
        .map_or_else(|| div().into_any_element(), |f| f(w, cx))
}
fn space(v: &str) -> Space {
    match v {
        "xxs" => Space::Xxs,
        "xs" => Space::Xs,
        "sm" => Space::Sm,
        "lg" => Space::Lg,
        "xl" => Space::Xl,
        "xxl" => Space::Xxl,
        _ => Space::Md,
    }
}
fn breakpoint(v: &str) -> Breakpoint {
    match v {
        "small" => Breakpoint::Small,
        "large" => Breakpoint::Large,
        "extraLarge" => Breakpoint::ExtraLarge,
        _ => Breakpoint::Medium,
    }
}
fn region(v: &str) -> DockRegion {
    match v {
        "left" => DockRegion::Left,
        "right" => DockRegion::Right,
        "bottom" => DockRegion::Bottom,
        _ => DockRegion::Centre,
    }
}
fn dock_record(v: &Value) -> DockRecord {
    DockRecord {
        id: s(v, "id").into(),
        parent: optional(v, "parent"),
        kind: match v["kind"].as_str() {
            Some("horizontal") => DockRecordKind::Horizontal,
            Some("vertical") => DockRecordKind::Vertical,
            _ => DockRecordKind::Stack,
        },
        ratio: n(v, "ratio", 0.5),
        panels: rows(v, "panels")
            .map(|p| p.as_str().unwrap_or_default().to_owned().into())
            .collect(),
        active: optional(v, "active"),
        min_width: n(v, "minWidth", 0.),
        min_height: n(v, "minHeight", 0.),
        rail: n(v, "rail", 0.),
        collapsed: b(v, "collapsed"),
    }
}
fn dock_topology(v: &Value) -> Result<(DockTopology, Vec<FloatingDock>)> {
    let records = rows(v, "records").map(dock_record).collect::<Vec<_>>();
    let floating = rows(v, "floating")
        .map(|tile| FloatingDockRecord {
            stack: dock_record(&tile["stack"]),
            bounds: Bounds::new(
                point(n(&tile["bounds"], "x", 0.), n(&tile["bounds"], "y", 0.)),
                extent(
                    n(&tile["bounds"], "width", 0.),
                    n(&tile["bounds"], "height", 0.),
                ),
            ),
        })
        .collect::<Vec<_>>();
    Ok(DockTopology::restore_with_floating(&records, &floating)?)
}
fn split_layout(v: &Value) -> Result<SplitLayout> {
    let records = rows(v, "records")
        .map(|v| SplitRecord {
            id: s(v, "id").into(),
            parent: optional(v, "parent"),
            kind: match v["kind"].as_str() {
                Some("horizontal") => SplitKind::Horizontal,
                Some("vertical") => SplitKind::Vertical,
                _ => SplitKind::Pane,
            },
            ratio: n(v, "ratio", 0.5),
            min_width: n(v, "minWidth", 0.),
            min_height: n(v, "minHeight", 0.),
            rail: n(v, "rail", 0.),
            collapsed: b(v, "collapsed"),
        })
        .collect::<Vec<_>>();
    Ok(SplitLayout::from_records(&records)?)
}
pub(super) fn validate(node: &Node) -> Result<()> {
    static SCHEMAS: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!("fixture/schemas.json"))
            .expect("generated layout schemas")
    });
    let v = Value::Object(node.props.clone());
    super::validation::validate(
        &v,
        &SCHEMAS[node.component.as_deref().unwrap_or_default()]["props"],
    )?;
    if node.component.as_deref() == Some("Container") && text(node, "width") == "custom" {
        anyhow::ensure!(
            node.props.contains_key("customWidth"),
            "custom width required"
        );
    }
    if node.component.as_deref() == Some("Toolbar") {
        for item in rows(&v, "items") {
            anyhow::ensure!(
                rows(&v, "groups").any(|g| g["id"] == item["group"]),
                "unknown toolbar group"
            );
        }
    }
    if node.component.as_deref() == Some("DesktopTitlebar") {
        let mut seen = std::collections::HashSet::new();
        for button in rows(&v["buttons"], "left").chain(rows(&v["buttons"], "right")) {
            anyhow::ensure!(seen.insert(button.as_str()), "duplicate window button");
        }
    }
    match node.component.as_deref() {
        Some("DockTree") => {
            dock_topology(&v)?;
        }
        Some("SplitTree") => {
            split_layout(&v)?;
        }
        _ => {}
    }
    Ok(())
}
fn dock_event(event: DockTreeEvent) -> Value {
    match event {
        DockTreeEvent::FloatingRaised { stack } => {
            json!({"kind":"floatingRaised","stack":stack.as_ref()})
        }
        DockTreeEvent::FloatingCancelled { stack } => {
            json!({"kind":"floatingCancelled","stack":stack.as_ref()})
        }
        DockTreeEvent::FloatingChanged {
            stack,
            bounds,
            finished,
        } => {
            json!({"kind":"floatingChanged","stack":stack.as_ref(),"bounds":{"x":bounds.origin.x,"y":bounds.origin.y,"width":bounds.size.width,"height":bounds.size.height},"finished":finished})
        }
        DockTreeEvent::PanelSelected { stack, panel } => {
            json!({"kind":"panelSelected","stack":stack.as_ref(),"panel":panel.as_ref()})
        }
        DockTreeEvent::PanelMoved {
            panel,
            to_stack,
            before,
        } => {
            json!({"kind":"panelMoved","panel":panel.as_ref(),"toStack":to_stack.as_ref(),"before":before.as_deref()})
        }
        DockTreeEvent::PanelSplit {
            panel,
            target_stack,
            placement,
        } => {
            json!({"kind":"panelSplit","panel":panel.as_ref(),"targetStack":target_stack.as_ref(),"placement":match placement {DockPlacement::Left=>"left",DockPlacement::Right=>"right",DockPlacement::Top=>"top",DockPlacement::Bottom=>"bottom"}})
        }
        DockTreeEvent::SplitResized { split, ratio } => {
            json!({"kind":"splitResized","split":split.as_ref(),"ratio":ratio})
        }
        DockTreeEvent::StackCollapsed { stack, collapsed } => {
            json!({"kind":"stackCollapsed","stack":stack.as_ref(),"collapsed":collapsed})
        }
    }
}
fn panel(v: &Value, slots: &KitSlots, w: &mut Window, cx: &mut App) -> DockPanel {
    let mut panel = DockPanel::new(s(v, "id"), s(v, "title"));
    if v.get("badge").is_some() {
        panel = panel.badge(s(v, "badge"));
    }
    if v.get("unavailable").is_some() {
        panel = panel.unavailable(s(v, "unavailable"));
    }
    if let Some(icon) = v.get("icon") {
        panel = panel.icon(super::icon::resolve(icon).expect("validated built-in icon"));
    }
    if slots.contains_key(&s(v, "id")) {
        panel = panel.content(slot(slots, &s(v, "id"), w, cx));
    }
    panel
}
pub(super) fn render(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
) -> AnyElement {
    render_with_menu(node, slots, window, cx, emit, None)
}
fn render_with_menu(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
    menu: Option<Entity<Menu>>,
) -> AnyElement {
    let v = Value::Object(node.props.clone());
    let event = |name: &str| {
        if flag(node, "disabled") {
            None
        } else {
            node.events.get(name).cloned()
        }
    };
    match node.component.as_deref().unwrap_or_default() {
        "AspectRatio" => AspectRatio::new(node.id.clone(), number(node, "ratio", 1.))
            .fit(if text(node, "fit") == "height" {
                AspectFit::Height
            } else {
                AspectFit::Width
            })
            .child(slot(&slots, "content", window, cx))
            .into_any_element(),
        "Container" => {
            let width = match text(node, "width").as_str() {
                "full" => ContainerWidth::Full,
                "dialog" => ContainerWidth::Dialog,
                "custom" => ContainerWidth::Custom(number(node, "customWidth", 0.)),
                _ => ContainerWidth::Readable,
            };
            Container::new(node.id.clone())
                .width(width)
                .padding(space(&text(node, "padding")))
                .child(slot(&slots, "content", window, cx))
                .into_any_element()
        }
        "Grid" => {
            let items = rows(&v, "items")
                .map(|v| {
                    let mut item = GridItem::new(s(v, "id"), slot(&slots, &s(v, "id"), window, cx))
                        .span(n(v, "span", 1.) as u16);
                    for at in rows(v, "spanAt") {
                        item = item
                            .span_at(breakpoint(&s(at, "breakpoint")), n(at, "span", 1.) as u16);
                    }
                    item
                })
                .collect::<Vec<_>>();
            let mut control = Grid::new(node.id.clone())
                .columns(number(node, "columns", 1.) as u16)
                .gap(space(&text(node, "gap")))
                .items(items);
            for at in rows(&v, "columnsAt") {
                control = control.columns_at(
                    breakpoint(&s(at, "breakpoint")),
                    n(at, "columns", 1.) as u16,
                );
            }
            control.into_any_element()
        }
        "Responsive" => {
            let threshold = number(node, "threshold", 0.);
            let mut control = Responsive::new(node.id.clone(), move |size, w, cx| {
                slot(
                    &slots,
                    match size {
                        ContainerSize::Unmeasured => "unmeasured",
                        _ if size.at_least(threshold) => "wide",
                        _ => "narrow",
                    },
                    w,
                    cx,
                )
            });
            if flag(node, "fill") {
                control = control.fill();
            }
            control.into_any_element()
        }
        "ScrollFade" => {
            let mut control = ScrollFade::new(node.id.clone());
            for key in ["top", "bottom", "left", "right"] {
                if let Some(value) = v[key].as_bool() {
                    control = match key {
                        "top" => control.top(value),
                        "bottom" => control.bottom(value),
                        "left" => control.left(value),
                        _ => control.right(value),
                    };
                }
            }
            if node.props.contains_key("band") {
                control = control.band(number(node, "band", 0.));
            }
            if flag(node, "fitHeight") {
                control = control.fit_height();
            }
            control
                .child(slot(&slots, "content", window, cx))
                .into_any_element()
        }
        "ScrollEdgeEffect" => {
            let mut control = ScrollEdgeEffect::new(node.id.clone());
            for key in ["top", "bottom", "left", "right"] {
                if let Some(value) = v[key].as_bool() {
                    control = match key {
                        "top" => control.top(value),
                        "bottom" => control.bottom(value),
                        "left" => control.left(value),
                        _ => control.right(value),
                    };
                }
            }
            if node.props.contains_key("band") {
                control = control.band(number(node, "band", 0.));
            }
            if node.props.contains_key("blur") {
                control = control.blur(number(node, "blur", 0.));
            }
            if text(node, "kind") == "hard" {
                control = control.hard();
            }
            control
                .child(slot(&slots, "content", window, cx))
                .into_any_element()
        }
        "SplitTree" => {
            let layout = split_layout(&v).expect("validated split topology");
            let mut control = SplitTree::new(node.id.clone()).disabled(flag(node, "disabled"));
            for pane in layout.panes() {
                control = control.pane(pane.id().clone(), slot(&slots, pane.id(), window, cx));
            }
            control = control.layout(layout);
            if let Some(action) = event("change") {
                control=control.on_change(move |change,_,_|emit(&action,match change{SplitChange::Ratio{split,ratio}=>json!({"kind":"ratio","split":split.as_ref(),"ratio":ratio}),SplitChange::Collapsed{split,side,pane}=>json!({"kind":"collapsed","split":split.as_ref(),"side":side.name(),"pane":pane.as_ref()})}));
            }
            control.into_any_element()
        }
        "DockTree" => {
            let (topology, floating) = dock_topology(&v).expect("validated dock topology");
            let mut control = DockTree::new(node.id.clone(), topology)
                .floating(floating)
                .expect("validated floating topology")
                .panels(rows(&v, "panels").map(|v| panel(v, &slots, window, cx)))
                .disabled(flag(node, "disabled"));
            if let Some(action) = event("event") {
                control = control.on_event(move |event, _, _| emit(&action, dock_event(event)));
            }
            control.into_any_element()
        }
        "Dock" => {
            let mut control = Dock::new(node.id.clone()).disabled(flag(node, "disabled"));
            for p in rows(&v, "panels") {
                control = control.panel(region(&s(p, "region")), panel(p, &slots, window, cx));
            }
            for r in rows(&v, "regions") {
                let key = region(&s(r, "id"));
                if let Some(active) = optional(r, "active") {
                    control = control.active(key, active);
                }
                if r.get("collapsed").is_some() {
                    control = control.collapsed(key, b(r, "collapsed"));
                }
                if r.get("share").is_some() {
                    control = control.share(key, n(r, "share", 0.));
                }
                if r.get("minSize").is_some() {
                    control = control.min_size(key, n(r, "minSize", 0.));
                }
            }
            if let Some(action) = event("event") {
                control=control.on_event(move |event,_,_|emit(&action,match event{
                DockEvent::PanelSelected{region,panel}=>json!({"kind":"panelSelected","region":region.name(),"panel":panel.as_ref()}),
                DockEvent::PanelMoved{panel,to_region,before}=>json!({"kind":"panelMoved","panel":panel.as_ref(),"toRegion":to_region.name(),"before":before.as_deref()}),
                DockEvent::RegionCollapsed{region,collapsed}=>json!({"kind":"regionCollapsed","region":region.name(),"collapsed":collapsed}),
                DockEvent::RegionResized{region,ratio}=>json!({"kind":"regionResized","region":region.name(),"ratio":ratio}),
            }));
            }
            control.into_any_element()
        }
        "Toolbar" => {
            let mut control = Toolbar::new(node.id.clone()).control_size(size(node));
            if node.props.contains_key("label") {
                control = control.label(text(node, "label"));
            }
            for group in rows(&v, "groups") {
                if b(group, "spacer") {
                    control = control.spacer();
                }
                let items = rows(&v, "items")
                    .filter(|item| item["group"] == group["id"])
                    .map(|v| {
                        let mut item = ToolbarItem::new(
                            s(v, "id"),
                            s(v, "label"),
                            slot(&slots, &s(v, "id"), window, cx),
                        )
                        .disabled(b(v, "disabled"));
                        if v.get("shortcut").is_some() {
                            item = item.shortcut(s(v, "shortcut"));
                        }
                        if let Some(icon) = v.get("icon") {
                            item = item
                                .icon(super::icon::resolve(icon).expect("validated built-in icon"));
                        }
                        item
                    })
                    .collect::<Vec<_>>();
                control = control.group(s(group, "id"), items);
            }
            if let Some(count) = v["overflowAfter"].as_u64() {
                control = control.overflow_after(count as usize);
            }
            if let Some(menu) = menu {
                control = control.overflow_menu(menu);
            }
            control.into_any_element()
        }
        "DesktopTitlebar" => {
            let mut control = DesktopTitlebar::new(node.id.clone(), text(node, "title"));
            if node.props.contains_key("subtitle") {
                control = control.subtitle(text(node, "subtitle"));
            }
            if slots.contains_key("left") {
                control = control.left(slot(&slots, "left", window, cx));
            }
            if slots.contains_key("right") {
                control = control.right(slot(&slots, "right", window, cx));
            }
            if let Some(buttons) = v.get("buttons") {
                let side = |key: &str| {
                    let mut side = [None; 3];
                    for (i, button) in rows(buttons, key).enumerate() {
                        side[i] = Some(match button.as_str() {
                            Some("minimize") => gpui::WindowButton::Minimize,
                            Some("maximize") => gpui::WindowButton::Maximize,
                            _ => gpui::WindowButton::Close,
                        });
                    }
                    side
                };
                control = control.button_layout(gpui::WindowButtonLayout {
                    left: side("left"),
                    right: side("right"),
                });
            }
            if let Some(c) = v.get("controls") {
                control = control.controls(gpui::WindowControls {
                    fullscreen: b(c, "fullscreen"),
                    maximize: b(c, "maximize"),
                    minimize: b(c, "minimize"),
                    window_menu: b(c, "windowMenu"),
                });
            }
            if let Some(action) = event("event") {
                control = control.on_event(move |event, _, _| {
                    emit(
                        &action,
                        json!(match event {
                            DesktopTitlebarEvent::Minimize => "minimize",
                            DesktopTitlebarEvent::ToggleMaximize => "toggleMaximize",
                            DesktopTitlebarEvent::Close => "close",
                        }),
                    )
                });
            }
            control.into_any_element()
        }
        "StatusBar" => {
            let mut control = StatusBar::new(node.id.clone());
            if node.props.contains_key("label") {
                control = control.label(text(node, "label"));
            }
            for row in rows(&v, "items") {
                let id = s(row, "id");
                let label = s(row, "label");
                let mut item = match row["kind"].as_str() {
                    Some("action") => StatusItem::action(id.clone(), label),
                    Some("element") => {
                        StatusItem::element(id.clone(), label, slot(&slots, &id, window, cx))
                    }
                    Some("progress") => StatusItem::progress(id.clone(), label),
                    Some("state") => StatusItem::state(
                        id.clone(),
                        label,
                        match row["tone"].as_str() {
                            Some("success") => Tone::Success,
                            Some("warning") => Tone::Warning,
                            Some("danger") => Tone::Danger,
                            Some("info") => Tone::Info,
                            Some("accent") => Tone::Accent,
                            _ => Tone::Neutral,
                        },
                    ),
                    _ => StatusItem::text(id.clone(), label),
                };
                item = item.disabled(b(row, "disabled")).stale(b(row, "stale"));
                if let Some(icon) = row.get("icon") {
                    item = item.icon(super::icon::resolve(icon).expect("validated built-in icon"));
                }
                if row.get("fraction").is_some() {
                    item = item.fraction(n(row, "fraction", 0.));
                }
                if let Some(count) = row.get("count") {
                    item = item.count(
                        n(count, "done", 0.) as usize,
                        n(count, "total", 0.) as usize,
                    );
                }
                if row.get("stateName").is_some() {
                    item = item.state_name(s(row, "stateName"));
                }
                if !b(row, "disabled")
                    && let Some(action) = event("click")
                {
                    let emit = emit.clone();
                    item = item.on_click(move |_, _| emit(&action, json!(id)));
                }
                control = control.item(
                    match row["group"].as_str() {
                        Some("centre") => StatusGroup::Centre,
                        Some("end") => StatusGroup::End,
                        _ => StatusGroup::Start,
                    },
                    item,
                );
            }
            control.into_any_element()
        }
        _ => unreachable!("layout family dispatch"),
    }
}

#[cfg(test)]
mod tests;
