//! Native display builders. Values and action identity remain caller-owned.
use super::{Emit, KitSlots, Node, flag, number, size as control_size, text};
use anyhow::{Result, bail, ensure};
use gpui::{AnyElement, App, Hsla, IntoElement, ParentElement, Window, div};
use gpui_kit::{display::attachment::*, foundation::slot::Slotted, prelude::*};
use gpui_kit_theme::{ActiveTheme, ColorChoice, SemanticColor, Space, Surface, Variant};
use serde_json::{Value, json};

pub(super) const COMPONENTS: &[&str] = &[
    "AnimatedNumber",
    "AttachmentTile",
    "Avatar",
    "AvatarGroup",
    "Badge",
    "Banner",
    "BarLoader",
    "Bubble",
    "Callout",
    "Card",
    "DescriptionList",
    "EmptyState",
    "FailurePanel",
    "Heatmap",
    "HighlightedText",
    "Icon",
    "ListRow",
    "LoadMore",
    "MetricCard",
    "OutcomePanel",
    "PerformanceHud",
    "ProgressBar",
    "ProgressCircle",
    "PulseLoader",
    "Rating",
    "RefreshVeil",
    "Skeleton",
    "SpanTimeline",
    "Spinner",
    "StageProgress",
    "StaleMark",
    "StateView",
    "StatusDot",
    "StatusLine",
    "Tag",
    "Timeline",
    "TraceView",
];

fn string(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_owned()
}
fn scalar(v: &Value, key: &str) -> f32 {
    v[key].as_f64().unwrap_or_default() as f32
}
fn integer(v: &Value, key: &str) -> usize {
    v[key].as_u64().unwrap_or_default() as usize
}
fn boolean(v: &Value, key: &str) -> bool {
    v[key].as_bool().unwrap_or(false)
}
fn list(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}
fn tint(v: &Value) -> Hsla {
    gpui::hsla(
        scalar(v, "h"),
        scalar(v, "s"),
        scalar(v, "l"),
        scalar(v, "a"),
    )
}
fn tone(v: &str) -> Tone {
    match v {
        "accent" => Tone::Accent,
        "success" => Tone::Success,
        "warning" => Tone::Warning,
        "danger" => Tone::Danger,
        "info" => Tone::Info,
        _ => Tone::Neutral,
    }
}
fn variant(v: &str) -> Variant {
    match v {
        "filled" => Variant::Filled,
        "light" => Variant::Light,
        "subtle" => Variant::Subtle,
        "transparent" => Variant::Transparent,
        "white" => Variant::White,
        _ => Variant::Default,
    }
}
fn color_choice(v: &Value) -> ColorChoice {
    match v["kind"].as_str() {
        Some("palette") => ColorChoice::Palette(string(v, "name").into()),
        Some("custom") => ColorChoice::Custom(tint(&v["tint"])),
        _ => ColorChoice::Semantic(match v["name"].as_str() {
            Some("accentStrong") => SemanticColor::AccentStrong,
            Some("danger") => SemanticColor::Danger,
            Some("warning") => SemanticColor::Warning,
            Some("success") => SemanticColor::Success,
            Some("info") => SemanticColor::Info,
            _ => SemanticColor::Accent,
        }),
    }
}
fn slotted<T: Slotted>(mut b: T, slots: &KitSlots) -> T {
    for &name in T::SLOTS {
        if let Some(f) = slots.get(name).cloned() {
            b = b.slot(name, move |w, c| f(w, c));
        }
    }
    b
}
fn avatar(v: &Value, id: String, cx: &App) -> Result<Avatar> {
    let presence = match v["presence"].as_str() {
        Some("online") => AvatarPresence::Online,
        Some("away") => AvatarPresence::Away,
        Some("busy") => AvatarPresence::Busy,
        Some("offline") => AvatarPresence::Offline,
        _ => AvatarPresence::Unknown,
    };
    let mut b = Avatar::new(string(v, "name")).id(id).presence(presence);
    if v.get("size").is_some() {
        b = b.size(scalar(v, "size"));
    }
    if let Some(color) = v.get("tint") {
        b = b.tint(tint(color));
    }
    if let Some(image) = v.get("image") {
        let reference: crate::resources::ResourceRef = serde_json::from_value(image.clone())?;
        b = b.image_source(crate::resources::Resources::image(&reference, cx)?);
    }
    Ok(b)
}
fn icon(node: &Node) -> Icon {
    let glyph = super::icon::resolve(&node.props["glyph"]).expect("validated built-in icon");
    let mut b = if node.props.contains_key("name") {
        Icon::named(node.id.clone(), glyph, text(node, "name"))
    } else {
        Icon::new(glyph)
    };
    let tone = match text(node, "tone").as_str() {
        "muted" => IconTone::Muted,
        "faint" => IconTone::Faint,
        "onAccent" => IconTone::OnAccent,
        "accent" => IconTone::Accent,
        "accentStrong" => IconTone::AccentStrong,
        "danger" => IconTone::Danger,
        "warning" => IconTone::Warning,
        "success" => IconTone::Success,
        "info" => IconTone::Info,
        _ => IconTone::Primary,
    };
    b = b.tone(tone).control_size(control_size(node));
    if node.props.contains_key("followDirection") {
        b = b.follow_direction(flag(node, "followDirection"));
    }
    b = match text(node, "motion").as_str() {
        "spinning" => b.spinning(node.id.clone()),
        "breathing" => b.breathing(node.id.clone()),
        "heartbeat" => b.reacting(node.id.clone(), gpui_kit::motion::Micro::Heartbeat),
        "bounce" => b.reacting(node.id.clone(), gpui_kit::motion::Micro::Bounce),
        "wobble" => b.reacting(node.id.clone(), gpui_kit::motion::Micro::Wobble),
        "pop" => b.reacting(node.id.clone(), gpui_kit::motion::Micro::Pop),
        "sparkle" => b.reacting(node.id.clone(), gpui_kit::motion::Micro::Sparkle),
        _ => b,
    };
    b
}
fn highlighted(node: &Node) -> HighlightedText {
    let mut b = HighlightedText::new(text(node, "text"))
        .id(node.id.clone())
        .monospace(flag(node, "monospace"));
    if flag(node, "selectable") {
        b = b.selectable(node.id.clone());
    }
    if let Some(doc) = node.props.get("document") {
        b = b.in_document(
            doc["order"].as_u64().unwrap_or_default(),
            boolean(doc, "virtualized"),
        );
    }
    if let Some(hits) = node.props.get("hits") {
        b = b.hits(list(hits).map(|h| integer(h, "start")..integer(h, "end")));
    }
    if let Some(current) = node.props.get("current").and_then(Value::as_u64) {
        b = b.current(current as usize);
    }
    b
}

pub(super) fn render(
    node: &Node,
    slots: KitSlots,
    window: &mut Window,
    cx: &mut App,
    emit: Emit,
) -> AnyElement {
    let id = node.id.clone();
    let p = Value::Object(node.props.clone().into_iter().collect());
    let action = |name: &str| {
        if flag(node, "disabled") {
            None
        } else {
            node.events.get(name).cloned()
        }
    };
    let slot = |name: &str, w: &mut Window, c: &mut App| {
        slots
            .get(name)
            .map_or_else(|| div().into_any_element(), |f| f(w, c))
    };
    macro_rules! optional_text { ($b:ident,$($key:literal=>$method:ident),*$(,)?)=>{ $(if node.props.contains_key($key) {$b=$b.$method(text(node,$key));})* }; }
    macro_rules! cancel {
        ($b:ident) => {
            if let Some(a) = action("cancel") {
                $b = $b.on_cancel(move |_, _| emit(&a, Value::Null));
            }
        };
    }
    macro_rules! colored {
        ($b:ident) => {
            if let Some(c) = node.props.get("tint") {
                $b = $b.tint(tint(c));
            }
        };
    }
    macro_rules! progress { ($b:ident)=>{
        optional_text!($b,"label"=>label,"display"=>display);
        if let Some(count)=node.props.get("count") {$b=$b.count(integer(count,"done"),integer(count,"total"));}
        if node.props.contains_key("fraction") {$b=$b.fraction(number(node,"fraction",0.));}
        $b=$b.stalled(flag(node,"stalled")).paused(flag(node,"paused"));
        cancel!($b);
    }; }
    match node.component.as_deref().unwrap_or_default() {
        "Avatar" => match avatar(&p, id.clone(), cx) {
            Ok(avatar) => avatar.into_any_element(),
            Err(_) => EmptyState::new(id, "Image unavailable")
                .kind(EmptyKind::Unavailable)
                .into_any_element(),
        },
        "AvatarGroup" => {
            let members = list(&p["members"])
                .map(|v| avatar(v, string(v, "id"), cx))
                .collect::<Result<Vec<_>>>();
            let Ok(members) = members else {
                return EmptyState::new(id, "Image unavailable")
                    .kind(EmptyKind::Unavailable)
                    .into_any_element();
            };
            let mut b = AvatarGroup::new().id(id).members(members);
            if node.props.contains_key("size") {
                b = b.size(number(node, "size", 32.));
            }
            optional_text!(b,"overflow"=>overflow);
            b.into_any_element()
        }
        "Badge" => {
            let mut b = Badge::new(text(node, "label"))
                .id(id)
                .tone(tone(&text(node, "tone")))
                .variant(variant(&text(node, "variant")))
                .dot(flag(node, "dot"))
                .control_size(control_size(node));
            if flag(node, "count") {
                b = b.count();
            }
            if let Some(v) = p.get("icon") {
                b = b.icon(super::icon::resolve(v).expect("validated icon"));
            }
            colored!(b);
            if let Some(c) = p.get("color") {
                b = b.color(color_choice(c));
            }
            b.into_any_element()
        }
        "Tag" => {
            let mut b = Tag::new(id, text(node, "label"))
                .tone(tone(&text(node, "tone")))
                .variant(variant(&text(node, "variant")))
                .disabled(flag(node, "disabled"));
            colored!(b);
            if let Some(c) = p.get("color") {
                b = b.color(color_choice(c));
            }
            if let Some(a) = action("remove") {
                b = b.on_remove(move |_, _| emit(&a, Value::Null));
            }
            b.into_any_element()
        }
        "Icon" => icon(node).into_any_element(),
        "HighlightedText" => highlighted(node).into_any_element(),
        "Callout" => Callout::new(text(node, "message"), tone(&text(node, "tone")))
            .id(id)
            .into_any_element(),
        "Banner" => {
            let mut b = Banner::new(id, text(node, "message"), tone(&text(node, "tone")));
            optional_text!(b,"title"=>title);
            if slots.contains_key("action") {
                b = b.action(slot("action", window, cx));
            }
            if let Some(a) = action("dismiss") {
                b = b.on_dismiss(move |_, _| emit(&a, Value::Null));
            }
            b.into_any_element()
        }
        "StaleMark" => {
            let mut b = StaleMark::new(id, text(node, "reason"));
            optional_text!(b,"updated"=>updated);
            b.into_any_element()
        }
        "StatusDot" => {
            let mut b = StatusDot::new(tone(&text(node, "tone")));
            colored!(b);
            if flag(node, "busy") {
                b = b.busy(id);
            }
            if let Some(a) = p["activity"].as_str() {
                b = b.activity(activity(a));
            }
            b.into_any_element()
        }
        "StatusLine" => {
            let mut b =
                StatusLine::new(text(node, "label"), tone(&text(node, "tone"))).id(id.clone());
            colored!(b);
            if flag(node, "busy") {
                b = b.busy(id);
            }
            if let Some(a) = p["activity"].as_str() {
                b = b.activity(activity(a));
            }
            b.into_any_element()
        }
        "ProgressBar" => {
            let mut b = ProgressBar::new(id);
            progress!(b);
            b.into_any_element()
        }
        "ProgressCircle" => {
            let mut b = ProgressCircle::new(id);
            optional_text!(b,"centre"=>centre);
            progress!(b);
            b.into_any_element()
        }
        "PulseLoader" => {
            let mut b = PulseLoader::new(id);
            optional_text!(b,"label"=>label);
            colored!(b);
            b.into_any_element()
        }
        "Spinner" => {
            let mut b = Spinner::new(id);
            optional_text!(b,"label"=>label);
            colored!(b);
            b.into_any_element()
        }
        "BarLoader" => {
            let mut b = BarLoader::new(id);
            optional_text!(b,"label"=>label);
            colored!(b);
            b.into_any_element()
        }
        "LoadMore" => {
            let state = match text(node, "state").as_str() {
                "loading" => LoadMoreState::Loading,
                "exhausted" => LoadMoreState::Exhausted,
                _ => LoadMoreState::Idle,
            };
            let mut b = LoadMore::new(id).state(state);
            if let Some(a) = action("more") {
                b = b.on_more(move |_, _| emit(&a, Value::Null));
            }
            b.into_any_element()
        }
        "RefreshVeil" => {
            let mut b = RefreshVeil::new(id, slot("content", window, cx));
            optional_text!(b,"label"=>label);
            b.into_any_element()
        }
        "Skeleton" => {
            let mut b = Skeleton::new(id);
            optional_text!(b,"label"=>label);
            if p.get("rows").is_some() {
                b = b.rows(integer(&p, "rows"));
            }
            if p.get("rowHeight").is_some() {
                b = b.row_height(scalar(&p, "rowHeight"));
            }
            if p.get("widths").is_some() {
                b = b.widths(
                    list(&p["widths"])
                        .filter_map(Value::as_f64)
                        .map(|v| v as f32),
                );
            }
            if p.get("shapes").is_some() {
                b = b.shapes(list(&p["shapes"]).map(|s| match s["kind"].as_str() {
                    Some("row") => SkeletonShape::Row {
                        width: scalar(s, "width"),
                        height: scalar(s, "height"),
                    },
                    Some("rect") => SkeletonShape::Rect {
                        width: scalar(s, "width"),
                        height: scalar(s, "height"),
                    },
                    Some("circle") => SkeletonShape::Circle {
                        size: scalar(s, "size"),
                    },
                    Some("paragraph") => SkeletonShape::Paragraph {
                        lines: integer(s, "lines"),
                    },
                    _ => SkeletonShape::Card,
                }));
            }
            b.into_any_element()
        }
        "Rating" => {
            let mut b = Rating::new(id)
                .disabled(flag(node, "disabled"))
                .clearable(flag(node, "clearable"));
            optional_text!(b,"label"=>label);
            if p.get("value").is_some() {
                b = b.value(p["value"].as_f64().map(|v| v as f32));
            }
            if p.get("maximum").is_some() {
                b = b.maximum(integer(&p, "maximum"));
            }
            if p["precision"] == "half" {
                b = b.precision(RatingPrecision::Half);
            }
            if let Some(a) = action("change") {
                b = b.on_change(move |v, _, _| emit(&a, json!(v)));
            }
            b.into_any_element()
        }
        "EmptyState" => {
            let mut b =
                EmptyState::new(id, text(node, "title")).kind(empty_kind(&text(node, "kind")));
            optional_text!(b,"detail"=>detail);
            if let Some(v) = p.get("icon") {
                b = b.icon(super::icon::resolve(v).expect("validated icon"));
            }
            if slots.contains_key("action") {
                b = b.action(slot("action", window, cx));
            }
            b.into_any_element()
        }
        "FailurePanel" => {
            let mut b = if let Some(result) = p.get("result") {
                let result: std::result::Result<(), String> = if boolean(result, "ok") {
                    Ok(())
                } else {
                    Err(string(result, "error"))
                };
                let Some(panel) = FailurePanel::from_result(id, &result) else {
                    return div().into_any_element();
                };
                panel
            } else {
                FailurePanel::new(id, text(node, "reason"))
            };
            b = b.retrying(flag(node, "retrying"));
            optional_text!(b,"title"=>title,"detail"=>detail);
            if p.get("attempts").is_some() {
                b = b.attempts(integer(&p, "attempts"));
            }
            if let Some(a) = action("retry") {
                b = b.on_retry(move |_, _| emit(&a, Value::Null));
            }
            b.into_any_element()
        }
        "OutcomePanel" => {
            let kind = match text(node, "kind").as_str() {
                "partial" => OutcomeKind::Partial,
                "failed" => OutcomeKind::Failed,
                _ => OutcomeKind::Success,
            };
            let mut b = OutcomePanel::new(id, kind);
            optional_text!(b,"title"=>title,"detail"=>detail,"count"=>count);
            if slots.contains_key("action") {
                b = b.action(slot("action", window, cx));
            }
            b.into_any_element()
        }
        "AttachmentTile" => {
            let s = &p["state"];
            let state = match s["kind"].as_str() {
                Some("queued") => AttachmentState::Queued,
                Some("processing") => AttachmentState::Processing,
                Some("cancelled") => AttachmentState::Cancelled,
                Some("failed") => AttachmentState::Failed(string(s, "reason").into()),
                Some("unavailable") => AttachmentState::Unavailable(string(s, "reason").into()),
                Some("transferring") => AttachmentState::Transferring {
                    completed: integer(s, "completed"),
                    total: s["total"].as_u64().map(|n| n as usize),
                },
                Some("paused") => AttachmentState::Paused {
                    completed: integer(s, "completed"),
                    total: s["total"].as_u64().map(|n| n as usize),
                },
                _ => AttachmentState::Ready,
            };
            let mut b = AttachmentTile::new(id, text(node, "title")).state(state);
            optional_text!(b,"description"=>description);
            slotted(b, &slots).into_any_element()
        }
        "Bubble" => {
            let mut b = Bubble::new(id, text(node, "label")).grouped(flag(node, "grouped"));
            if p["placement"] == "end" {
                b = b.placement(BubblePlacement::End);
            }
            if p.get("maxWidth").is_some() {
                b = b.max_width(scalar(&p, "maxWidth"));
            }
            if slots.contains_key("content") {
                b = b.content(slot("content", window, cx));
            }
            if slots.contains_key("actions") {
                b = b.actions([slot("actions", window, cx)]);
            }
            b.into_any_element()
        }
        "Card" => {
            let mut b = Card::new().id(id).disabled(flag(node, "disabled"));
            optional_text!(b,"name"=>name);
            b = b.variant(match p["variant"].as_str() {
                Some("filled") => CardVariant::Filled,
                Some("ghost") => CardVariant::Ghost,
                _ => CardVariant::Elevated,
            });
            if p.get("header").is_some() {
                let mut h = CardHeader::new(string(&p["header"], "title"));
                if p["header"].get("subtitle").is_some() {
                    h = h.subtitle(string(&p["header"], "subtitle"));
                }
                if let Some(f) = slots.get("headerAction").cloned() {
                    h = h.action(move |w, c| f(w, c));
                }
                b = b.header(h);
            }
            if let Some(f) = slots.get("media").cloned() {
                b = b.media(move |w, c| f(w, c));
            }
            if let Some(f) = slots.get("footer").cloned() {
                b = b.footer(move |w, c| f(w, c));
            }
            if p.get("padded").is_some() {
                b = b.padded(boolean(&p, "padded"));
            }
            if let Some(s) = p.get("padding") {
                b = b.padding(s.as_str().map(|s| match s {
                    "xxs" => Space::Xxs,
                    "xs" => Space::Xs,
                    "sm" => Space::Sm,
                    "lg" => Space::Lg,
                    "xl" => Space::Xl,
                    "xxl" => Space::Xxl,
                    _ => Space::Md,
                }));
            }
            if let Some(s) = p["ground"].as_str() {
                b = b.ground(match s {
                    "backdrop" => Surface::Backdrop,
                    "canvas" => Surface::Canvas,
                    "sunken" => Surface::Sunken,
                    "raised" => Surface::Raised,
                    "overlay" => Surface::Overlay,
                    _ => Surface::Panel,
                });
            }
            if slots.contains_key("content") {
                b = b.child(slot("content", window, cx));
            }
            if let Some(a) = action("click") {
                b = b.on_click(move |_, _| emit(&a, Value::Null));
            }
            b.into_any_element()
        }
        "ListRow" => {
            let mut b = ListRow::new().id(id).disabled(flag(node, "disabled"));
            if slots.contains_key("leading") {
                b = b.leading(slot("leading", window, cx));
            }
            if slots.contains_key("trailing") {
                b = b.trailing(slot("trailing", window, cx));
            }
            if slots.contains_key("content") {
                b = b.child(slot("content", window, cx));
            }
            if let Some(a) = action("click") {
                b = b.on_click(move |_, _| emit(&a, Value::Null));
            }
            b.into_any_element()
        }
        _ => render_data(node, slots, window, cx, emit),
    }
}

fn activity(v: &str) -> gpui_kit::motion::Activity {
    use gpui_kit::motion::Activity;
    match v {
        "advancing" => Activity::Advancing,
        "deliberating" => Activity::Deliberating,
        "signaling" => Activity::Signaling,
        "transmitting" => Activity::Transmitting,
        _ => Activity::Working,
    }
}
fn empty_kind(v: &str) -> EmptyKind {
    match v {
        "unstarted" => EmptyKind::Unstarted,
        "queued" => EmptyKind::Queued,
        "blocked" => EmptyKind::Blocked,
        "cancelled" => EmptyKind::Cancelled,
        "unavailable" => EmptyKind::Unavailable,
        "failed" => EmptyKind::Failed,
        "unauthorized" => EmptyKind::Unauthorized,
        _ => EmptyKind::Empty,
    }
}

/// Conditional data invariants supplement the shared closed-schema validator.
pub(super) fn validate(node: &Node) -> Result<()> {
    let p = Value::Object(node.props.clone().into_iter().collect());
    for image in p
        .get("image")
        .into_iter()
        .chain(list(&p["members"]).filter_map(|m| m.get("image")))
    {
        let key = image["key"].as_str().unwrap_or_default();
        ensure!(
            !key.is_empty()
                && key.len() <= 128
                && key
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "invalid resource key"
        );
    }
    match node.component.as_deref().unwrap_or_default() {
        "AnimatedNumber" => {
            if let Some(s) = p.get("spec") {
                let spring = s.get("spring").is_some();
                ensure!(
                    s.get("curve").is_some() != spring && s.get("durationMs").is_some() != spring,
                    "choose spring or duration and curve"
                );
            }
        }
        "Timeline" => {
            let mut ids = std::collections::HashSet::new();
            for entry in
                list(&p["entries"]).chain(list(&p["groups"]).flat_map(|g| list(&g["entries"])))
            {
                ensure!(
                    ids.insert(string(entry, "id")),
                    "duplicate timeline entry identity"
                );
            }
        }
        "FailurePanel" => {
            ensure!(
                p.get("reason").is_some() != p.get("result").is_some(),
                "choose reason or result"
            );
            if let Some(r) = p.get("result") {
                ensure!(
                    r.get("error").is_some() != boolean(r, "ok"),
                    "result error mismatch"
                );
            }
        }
        "StateView" => {
            if boolean(&p, "fromAsync") {
                ensure!(
                    matches!(
                        p["state"]["kind"].as_str(),
                        Some(
                            "idle"
                                | "loading"
                                | "refreshing"
                                | "ready"
                                | "empty"
                                | "unavailable"
                                | "error"
                        )
                    ),
                    "phase not representable by AsyncValue"
                );
            }
        }
        "MetricCard" | "PerformanceHud" => {
            let s = &p["state"];
            let kind = s["kind"].as_str().unwrap_or_default();
            ensure!(
                s.get("data").is_some() == matches!(kind, "ready" | "stale"),
                "state data mismatch"
            );
            ensure!(
                s.get("reason").is_some() == matches!(kind, "unavailable" | "error" | "stale"),
                "state reason mismatch"
            );
        }
        "AttachmentTile" => {
            if let Some(s) = p.get("state") {
                let kind = s["kind"].as_str().unwrap_or_default();
                ensure!(
                    s.get("reason").is_some() == matches!(kind, "unavailable" | "failed"),
                    "attachment reason mismatch"
                );
                ensure!(
                    s.get("completed").is_some() == matches!(kind, "transferring" | "paused"),
                    "attachment count mismatch"
                );
                ensure!(
                    s.get("total").is_none() || matches!(kind, "transferring" | "paused"),
                    "attachment total mismatch"
                );
            }
        }
        "DescriptionList" => {
            for i in list(&p["items"]) {
                let v = &i["value"];
                ensure!(
                    v.get("text").is_some()
                        == matches!(v["kind"].as_str(), Some("text" | "redacted")),
                    "description value mismatch"
                );
            }
        }
        "Rating" => {
            if let Some(v) = p["value"].as_f64() {
                ensure!(
                    v <= p["maximum"].as_f64().unwrap_or(5.),
                    "rating exceeds maximum"
                );
            }
        }
        "ProgressBar" | "ProgressCircle" => ensure!(
            p.get("count").is_none() || p.get("fraction").is_none(),
            "choose count or fraction"
        ),
        "Skeleton" => {
            for s in list(&p["shapes"]) {
                let fields: &[&str] = match s["kind"].as_str() {
                    Some("row" | "rect") => &["width", "height"],
                    Some("circle") => &["size"],
                    Some("paragraph") => &["lines"],
                    _ => &[],
                };
                for key in ["width", "height", "size", "lines"] {
                    ensure!(
                        s.get(key).is_some() == fields.contains(&key),
                        "skeleton shape mismatch"
                    );
                }
            }
        }
        "Heatmap" => {
            if let Some(s) = p.get("state") {
                ensure!(
                    s.get("reason").is_some()
                        == matches!(s["kind"].as_str(), Some("unavailable" | "error")),
                    "heatmap reason mismatch"
                );
            }
            for c in list(&p["cells"]) {
                ensure!(
                    list(&p["rows"]).any(|r| r["id"] == c["row"])
                        && list(&p["columns"]).any(|r| r["id"] == c["column"]),
                    "unknown heatmap axis"
                );
            }
        }
        "TraceView" | "SpanTimeline" => {
            for s in list(&p["spans"]) {
                ensure!(scalar(s, "start") <= scalar(s, "end"), "reversed span");
            }
        }
        _ => {}
    }
    if let Some(c) = p.get("color") {
        if c["kind"] == "custom" {
            ensure!(
                c.get("tint").is_some() && c.get("name").is_none(),
                "custom color mismatch"
            );
        } else {
            ensure!(
                c.get("name").is_some() && c.get("tint").is_none(),
                "named color mismatch"
            );
            if c["kind"] == "semantic" {
                ensure!(
                    matches!(
                        c["name"].as_str(),
                        Some("accent" | "accentStrong" | "danger" | "warning" | "success" | "info")
                    ),
                    "unknown semantic color"
                );
            }
        }
    }
    Ok(())
}

pub(super) fn invoke(
    node: &Node,
    method: &str,
    args: &Value,
    query: bool,
    _window: &mut Window,
    cx: &mut App,
) -> Result<Value> {
    ensure!(query, "display builders have no native commands");
    match (node.component.as_deref(), method) {
        (Some("HighlightedText"), "published_hits") => {
            Ok(json!(highlighted(node).published_hits()))
        }
        (Some("Icon"), "resolved_size") => Ok(json!(icon(node).resolved_size(cx.theme()))),
        (Some("Icon"), "resolved_color") => {
            let c = icon(node).resolved_color(cx.theme());
            Ok(json!({"h":c.h,"s":c.s,"l":c.l,"a":c.a}))
        }
        (Some("Icon"), "flips_in") => {
            Ok(json!(icon(node).flips_in(if args["direction"] == "rtl" {
                gpui_kit::foundation::LayoutDirection::RightToLeft
            } else {
                gpui_kit::foundation::LayoutDirection::LeftToRight
            })))
        }
        _ => bail!("unsupported display query"),
    }
}

mod data;
use data::render_data;
#[cfg(all(test, feature = "capture"))]
mod tests;
