//! One canonical rendering per component.
//!
//! A scene is the single description of a component's states that both the
//! gallery and the audit tests consume, so a component cannot be reviewed
//! visually in one arrangement and tested in another.

use gpui::{
    AnyElement, App, Entity, Focusable, Global, IntoElement, SharedString, Window, div, prelude::*,
    px,
};
use gpui_kit_assets::Icon;
use gpui_kit_semantics::{NodeSpec, Role, Semantic};
use gpui_kit_theme::{Radius, Space, TextTone, Theme, TypeScale};

use crate::controls::combobox::Combobox;
use crate::controls::input::TextInput;
use crate::controls::keybinding_recorder::KeybindingRecorder;
use crate::controls::number_input::NumberInput;
use crate::controls::select::{Select, SelectOption};
use crate::controls::split_button::SplitButton;
use crate::controls::tag_input::TagInput;
use crate::controls::textarea::TextArea;
use crate::display::badge::Tone;
use crate::display::icon::{Icon as IconView, IconTone};
use crate::foundation::ActiveTheme;
use crate::foundation::direction::{ActiveDirection, DirectionalExt, LayoutDirection};
use crate::interaction::dnd;
use crate::overlay::toast::push as toast_push;
use crate::overlay::{Edge, Kbd, Overlay, Placement, Tooltip, Tooltipped};
use crate::prelude::*;
use crate::strings::{ActiveStrings, StringKey};

/// One canonical rendering, addressed by name.
pub struct Scene {
    pub name: &'static str,
    pub build: fn(&mut Window, &mut App) -> AnyElement,
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Scene")
            .field("name", &self.name)
            .finish()
    }
}

/// Every scene, in a stable order.
pub fn catalog() -> Vec<Scene> {
    #[allow(unused_mut)]
    let mut scenes = vec![
        Scene {
            name: "button",
            build: button,
        },
        Scene {
            name: "badge",
            build: badge,
        },
        Scene {
            name: "card",
            build: card,
        },
        Scene {
            name: "status",
            build: status,
        },
        Scene {
            name: "loading",
            build: loading,
        },
        Scene {
            name: "choice",
            build: choice,
        },
        Scene {
            name: "input",
            build: input,
        },
        Scene {
            name: "textarea",
            build: textarea,
        },
        Scene {
            name: "form",
            build: form,
        },
        Scene {
            name: "auth-sign-in",
            build: auth_sign_in,
        },
        Scene {
            name: "auth-verification",
            build: auth_verification,
        },
        Scene {
            name: "actions",
            build: actions,
        },
        Scene {
            name: "content",
            build: content,
        },
        Scene {
            name: "kbd",
            build: kbd,
        },
        Scene {
            name: "overlay",
            build: overlay,
        },
        Scene {
            name: "dialog",
            build: dialog,
        },
        Scene {
            name: "tooltip",
            build: tooltip,
        },
        Scene {
            name: "menu",
            build: menu,
        },
        Scene {
            name: "context-menu",
            build: context_menu,
        },
        Scene {
            name: "popover",
            build: popover,
        },
        Scene {
            name: "command-palette",
            build: command_palette,
        },
        Scene {
            name: "toast",
            build: toast,
        },
        Scene {
            name: "tabs",
            build: tabs,
        },
        Scene {
            name: "accordion",
            build: accordion,
        },
        Scene {
            name: "breadcrumb",
            build: breadcrumb,
        },
        Scene {
            name: "list",
            build: list,
        },
        Scene {
            name: "table",
            build: table,
        },
        Scene {
            name: "data-grid",
            build: data_grid,
        },
        Scene {
            name: "data-grid-editing",
            build: data_grid_editing,
        },
        Scene {
            name: "tree-grid",
            build: tree_grid,
        },
        Scene {
            name: "tree",
            build: tree,
        },
        Scene {
            name: "split-pane",
            build: split_pane,
        },
        Scene {
            name: "scroll-area",
            build: scroll_area,
        },
        Scene {
            name: "scroll-shadow",
            build: scroll_shadow,
        },
        Scene {
            name: "scroll-fade",
            build: scroll_fade,
        },
        Scene {
            name: "frost",
            build: frost,
        },
        Scene {
            name: "toolbar",
            build: toolbar,
        },
        Scene {
            name: "sidebar",
            build: sidebar,
        },
        Scene {
            name: "pagination",
            build: pagination,
        },
        Scene {
            name: "drawer",
            build: drawer,
        },
        Scene {
            name: "motion-flip",
            build: motion_flip,
        },
        Scene {
            name: "motion-state",
            build: motion_state,
        },
        Scene {
            name: "animated-number",
            build: animated_number,
        },
        Scene {
            name: "drag-list",
            build: drag_list,
        },
        Scene {
            name: "drag-tree",
            build: drag_tree,
        },
        Scene {
            name: "dropzone",
            build: dropzone,
        },
        Scene {
            name: "wizard",
            build: wizard,
        },
        Scene {
            name: "undo-history",
            build: undo_history,
        },
        Scene {
            name: "settings",
            build: settings,
        },
        Scene {
            name: "detail",
            build: detail,
        },
        Scene {
            name: "filter-bar",
            build: filter_bar,
        },
        Scene {
            name: "inline-edit",
            build: inline_edit,
        },
        Scene {
            name: "progress-circle",
            build: progress_circle,
        },
        Scene {
            name: "split-tree",
            build: split_tree,
        },
        Scene {
            name: "ide-shell",
            build: ide_shell,
        },
        Scene {
            name: "keybinding",
            build: keybinding,
        },
        Scene {
            name: "keymap-editor",
            build: keymap_editor,
        },
        Scene {
            name: "markdown",
            build: markdown,
        },
        Scene {
            name: "conversation",
            build: conversation,
        },
        Scene {
            name: "image-viewer",
            build: image_viewer,
        },
        Scene {
            name: "transport",
            build: transport,
        },
        Scene {
            name: "audio-player",
            build: audio_player,
        },
        Scene {
            name: "video-player",
            build: video_player,
        },
        Scene {
            name: "model-viewer",
            build: model_viewer,
        },
        Scene {
            name: "approval",
            build: approval,
        },
        Scene {
            name: "permission-matrix",
            build: permission_matrix,
        },
        Scene {
            name: "cost-meter",
            build: cost_meter,
        },
        Scene {
            name: "tool-call",
            build: tool_call,
        },
        Scene {
            name: "step-list",
            build: step_list,
        },
        Scene {
            name: "node-graph",
            build: node_graph,
        },
        Scene {
            name: "browser-panel",
            build: browser_panel,
        },
        Scene {
            name: "thinking",
            build: thinking,
        },
        Scene {
            name: "json-view",
            build: json_view,
        },
        Scene {
            name: "schema-form",
            build: schema_form,
        },
        Scene {
            name: "server-list",
            build: server_list,
        },
        Scene {
            name: "offering-catalog",
            build: offering_catalog,
        },
        Scene {
            name: "reading-direction",
            build: reading_direction,
        },
        Scene {
            name: "toggle",
            build: toggle,
        },
        Scene {
            name: "collapsible",
            build: collapsible,
        },
        Scene {
            name: "hover-card",
            build: hover_card,
        },
        Scene {
            name: "menubar",
            build: menubar,
        },
        Scene {
            name: "copy-button",
            build: copy_button,
        },
        Scene {
            name: "aspect-ratio",
            build: aspect_ratio,
        },
        Scene {
            name: "document-tabs",
            build: document_tabs,
        },
        Scene {
            name: "search-field",
            build: search_field,
        },
        Scene {
            name: "find-replace",
            build: find_replace,
        },
        Scene {
            name: "notification-center",
            build: notification_center,
        },
        Scene {
            name: "failure-panel",
            build: failure_panel,
        },
        Scene {
            name: "log-stream",
            build: log_stream,
        },
        Scene {
            name: "diff-view",
            build: diff_view,
        },
        Scene {
            name: "sparkline",
            build: sparkline,
        },
        Scene {
            name: "code-view",
            build: code_view,
        },
        Scene {
            name: "upload-list",
            build: upload_list,
        },
        Scene {
            name: "cascader",
            build: cascader,
        },
        Scene {
            name: "anchor-list",
            build: anchor_navigation,
        },
        Scene {
            name: "diagnostics-list",
            build: diagnostics_surface,
        },
    ];
    #[cfg(feature = "fixtures")]
    scenes.extend([
        Scene {
            name: "calendar",
            build: calendar,
        },
        Scene {
            name: "date-range",
            build: date_range,
        },
        Scene {
            name: "date-time",
            build: date_time,
        },
    ]);
    scenes
}

/// The scene registered under `name`, or `None` when nothing is.
pub fn find(name: &str) -> Option<Scene> {
    catalog().into_iter().find(|scene| scene.name == name)
}

/// The reading direction a scene expects.
///
/// The direction is a global, like the theme, so a capture run that renders
/// the whole catalog into one process has to set it per scene the same way it
/// activates a theme per scene. A scene that changed it while rendering would
/// leak into whichever scene came next, and the leak would show up as a
/// changed image somewhere else entirely.
pub fn direction(name: &str) -> LayoutDirection {
    match name {
        "reading-direction" => LayoutDirection::RightToLeft,
        _ => LayoutDirection::LeftToRight,
    }
}

fn stack(theme: &Theme) -> gpui::Div {
    div()
        .column()
        .gap(px(theme.spacing.md))
        .p(px(theme.spacing.lg))
        .bg(theme.colors.canvas)
        .text_color(theme.colors.text)
        .font_family(theme.typography.sans.clone())
}

fn row(theme: &Theme) -> gpui::Div {
    div()
        .row()
        .flex_wrap()
        .gap(px(theme.spacing.sm))
        .items_center()
}

fn button(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(
            row(&theme)
                .child(
                    Button::new("scene.button.primary")
                        .label("Primary")
                        .primary()
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.button.secondary")
                        .label("Secondary")
                        .secondary()
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.button.ghost")
                        .label("Ghost")
                        .ghost()
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.button.danger")
                        .label("Delete")
                        .danger()
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.button.link")
                        .label("Learn more")
                        .link()
                        .on_click(|_, _| {}),
                ),
        )
        .child(
            row(&theme)
                .child(
                    Button::new("scene.button.disabled")
                        .label("Unavailable")
                        .primary()
                        .disabled(true)
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.button.loading")
                        .label("Saving")
                        .primary()
                        .loading(true)
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.button.selected")
                        .label("Selected")
                        .secondary()
                        .selected(true)
                        .on_click(|_, _| {}),
                ),
        )
        .child(
            row(&theme)
                .child(Button::new("scene.button.xs").label("Extra small").xs())
                .child(Button::new("scene.button.sm").label("Small").small())
                .child(Button::new("scene.button.md").label("Medium").medium())
                .child(Button::new("scene.button.lg").label("Large").large()),
        )
        .into_any_element()
}

fn badge(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(
            row(&theme)
                .child(Badge::new("Neutral").neutral().id("scene.badge.neutral"))
                .child(Badge::new("Accent").accent().id("scene.badge.accent"))
                .child(Badge::new("Success").success().id("scene.badge.success"))
                .child(Badge::new("Warning").warning().id("scene.badge.warning"))
                .child(Badge::new("Danger").danger().id("scene.badge.danger"))
                .child(Badge::new("Info").info().id("scene.badge.info")),
        )
        .into_any_element()
}

fn card(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(
            Card::new()
                .id("scene.card")
                .child(
                    ListRow::new()
                        .id("scene.card.runtime")
                        .child(div().flex_1().child(crate::foundation::text(
                            &theme,
                            TypeScale::Label,
                            "Native runtime",
                        )))
                        .child(Badge::new("Ready").success()),
                )
                .child(
                    ListRow::new()
                        .id("scene.card.catalog")
                        .child(div().flex_1().child(crate::foundation::text(
                            &theme,
                            TypeScale::Label,
                            "Model catalog",
                        )))
                        .child(Badge::new("Stale").warning()),
                ),
        )
        .into_any_element()
}

#[derive(Debug)]
struct SceneGraph {
    viewport: GraphViewport,
    ingest: gpui::Point<f32>,
    validate: gpui::Point<f32>,
    persist: gpui::Point<f32>,
    observe: gpui::Point<f32>,
    publish: gpui::Point<f32>,
    edges: Vec<GraphEdge>,
}

impl Global for SceneGraph {}

/// A live processing graph with the visual states and editor gestures shown
/// together: traffic lanes, a running shockwave, explicit ports, labels,
/// feedback routing, pan, zoom, node movement, and connection creation.
fn node_graph(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneGraph>() {
        cx.set_global(SceneGraph {
            viewport: GraphViewport::default(),
            ingest: gpui::point(24.0, 200.0),
            validate: gpui::point(244.0, 56.0),
            persist: gpui::point(500.0, 56.0),
            observe: gpui::point(464.0, 330.0),
            publish: gpui::point(684.0, 200.0),
            edges: vec![
                GraphEdge::new("scene.graph.ingest", "scene.graph.validate")
                    .id("scene.graph.edge.rows")
                    .ports("rows", "records")
                    .label("12.4k rows")
                    .active(true),
                GraphEdge::new("scene.graph.validate", "scene.graph.persist")
                    .id("scene.graph.edge.valid")
                    .ports("valid", "records")
                    .active(true),
                GraphEdge::new("scene.graph.validate", "scene.graph.observe")
                    .id("scene.graph.edge.telemetry")
                    .ports("telemetry", "events")
                    .label("telemetry")
                    .lane(-1),
                GraphEdge::new("scene.graph.persist", "scene.graph.publish")
                    .id("scene.graph.edge.commit")
                    .ports("commit", "artifact")
                    .label("commit 8f72")
                    .active(true),
                GraphEdge::new("scene.graph.observe", "scene.graph.validate")
                    .id("scene.graph.edge.retry")
                    .ports("retry", "retry")
                    .label("retry policy")
                    .lane(1)
                    .feedback(),
            ],
        });
    }
    let scene = cx.global::<SceneGraph>();
    let viewport = scene.viewport;
    let ingest = scene.ingest;
    let validate = scene.validate;
    let persist = scene.persist;
    let observe = scene.observe;
    let publish = scene.publish;
    let edges = scene.edges.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .child(
            div().w(px(860.0)).h(px(500.0)).child(
                NodeGraph::new("scene.graph")
                    .viewport(viewport)
                    .zoom_range(0.55, 1.8)
                    .node(
                        GraphNode::new("scene.graph.ingest", "Stream ingest")
                            .width(176.0)
                            .state(NodeState::Succeeded)
                            .action("orders.v2 · partition 18")
                            .metric("rate", "3.2k/s")
                            .port(GraphPort::input("source", "Source").side(PortSide::Top))
                            .port(GraphPort::output("rows", "Rows"))
                            .port(GraphPort::output("errors", "Errors")),
                        ingest.x,
                        ingest.y,
                    )
                    .node(
                        GraphNode::new("scene.graph.validate", "Validate & enrich")
                            .width(176.0)
                            .state(NodeState::Running)
                            .action("schema + fraud signals")
                            .metric("p95", "18 ms")
                            .port(GraphPort::input("records", "Records"))
                            .port(GraphPort::input("retry", "Retry").side(PortSide::Bottom))
                            .port(GraphPort::output("valid", "Valid"))
                            .port(GraphPort::output("telemetry", "Events").side(PortSide::Bottom))
                            .selected(true),
                        validate.x,
                        validate.y,
                    )
                    .node(
                        GraphNode::new("scene.graph.persist", "Persist batch")
                            .width(176.0)
                            .state(NodeState::Succeeded)
                            .action("warehouse / orders")
                            .metric("written", "12.3k")
                            .port(GraphPort::input("records", "Records"))
                            .port(GraphPort::output("commit", "Commit")),
                        persist.x,
                        persist.y,
                    )
                    .node(
                        GraphNode::new("scene.graph.observe", "Observe quality")
                            .width(176.0)
                            .state(NodeState::Failed)
                            .action("drift threshold exceeded")
                            .metric("rejected", "94")
                            .port(GraphPort::input("events", "Events").side(PortSide::Top))
                            .port(GraphPort::output("retry", "Retry").side(PortSide::Top)),
                        observe.x,
                        observe.y,
                    )
                    .node(
                        GraphNode::new("scene.graph.publish", "Publish artifact")
                            .width(176.0)
                            .state(NodeState::Pending)
                            .action("waiting for commit")
                            .port(GraphPort::input("artifact", "Artifact").side(PortSide::Top))
                            .port(GraphPort::output("release", "Release").side(PortSide::Bottom)),
                        publish.x,
                        publish.y,
                    )
                    .edges(edges)
                    .on_event(|event, _, cx| {
                        cx.update_global::<SceneGraph, ()>(|scene, _| match event {
                            NodeGraphEvent::ViewportChanged(viewport) => {
                                scene.viewport = *viewport;
                            }
                            NodeGraphEvent::NodeMoved { id, position } => match id.as_ref() {
                                "scene.graph.ingest" => scene.ingest = *position,
                                "scene.graph.validate" => scene.validate = *position,
                                "scene.graph.persist" => scene.persist = *position,
                                "scene.graph.observe" => scene.observe = *position,
                                "scene.graph.publish" => scene.publish = *position,
                                _ => {}
                            },
                            NodeGraphEvent::ConnectionRequested { from, to } => {
                                let id = format!(
                                    "scene.graph.edge.user.{}.{}.{}.{}",
                                    from.node, from.port, to.node, to.port
                                );
                                if !scene
                                    .edges
                                    .iter()
                                    .any(|edge| edge.from() == &from.node && edge.to() == &to.node)
                                {
                                    scene.edges.push(
                                        GraphEdge::new(from.node.clone(), to.node.clone())
                                            .id(id)
                                            .ports(from.port.clone(), to.port.clone())
                                            .label("new connection"),
                                    );
                                }
                            }
                        });
                        cx.refresh_windows();
                    }),
            ),
        )
        .into_any_element()
}

/// The complete shell contract: every state remains visibly distinct, and
/// the narrow panels prove long addresses and page content stay clipped to
/// the caller-owned viewport.
fn browser_panel(_window: &mut Window, cx: &mut App) -> AnyElement {
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

fn status(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(
            row(&theme)
                .child(StatusDot::new(Tone::Success))
                .child(StatusDot::new(Tone::Warning))
                .child(StatusDot::new(Tone::Danger))
                .child(StatusDot::new(Tone::Neutral)),
        )
        .child(
            StatusLine::new("Connected", Tone::Success).id("scene.status.line"),
        )
        .child(
            Callout::new(
                "The host refused this action. The refusal is shown, not converted to an empty state.",
                Tone::Danger,
            )
            .id("scene.status.refusal"),
        )
        .child(
            Callout::new("Refreshing failed. The last verified value remains visible.", Tone::Warning)
                .id("scene.status.stale"),
        )
        .into_any_element()
}

fn loading(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let indicator = |title: &'static str, loader: AnyElement| {
        div()
            .column()
            .items_center()
            .justify_center()
            .gap_token(&theme, Space::Md)
            .w(px(220.0))
            .h(px(112.0))
            .p_token(&theme, Space::Md)
            .radius(&theme, Radius::Card)
            .surface(&theme, gpui_kit_theme::Surface::Panel)
            .child(crate::foundation::text(&theme, TypeScale::Label, title))
            .child(loader)
    };
    stack(&theme)
        .w_full()
        .max_w(px(520.0))
        .child(
            row(&theme)
                .gap_token(&theme, Space::Md)
                .child(indicator(
                    "Loading providers",
                    PulseLoader::new("scene.loading.pulse")
                        .label("Loading providers")
                        .into_any_element(),
                ))
                .child(indicator(
                    "Contacting host",
                    GradientSpinner::new("scene.loading.spinner")
                        .label("Contacting host")
                        .into_any_element(),
                )),
        )
        .child(
            div()
                .column()
                .gap_token(&theme, Space::Sm)
                .w_full()
                .child(crate::foundation::text(
                    &theme,
                    TypeScale::Label,
                    "Loading list",
                ))
                .child(
                    Skeleton::new("scene.loading.skeleton")
                        .rows(3)
                        .label("Loading list"),
                ),
        )
        .into_any_element()
}

fn kbd(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(
            row(&theme)
                .child(Kbd::new("cmd-shift-p").id("scene.kbd.palette"))
                .child(Kbd::new("ctrl-c").id("scene.kbd.copy"))
                .child(Kbd::new("enter").id("scene.kbd.confirm"))
                .child(Kbd::new("escape").id("scene.kbd.dismiss")),
        )
        .into_any_element()
}

fn overlay(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .h(px(320.0))
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "Content behind the dialog",
        ))
        .child(
            Overlay::modal("scene.overlay.dialog")
                .placement(Placement::Center)
                .child(
                    crate::overlay::surface(&theme, gpui_kit_theme::Elevation::Modal)
                        .w(px(320.0))
                        .p(px(theme.spacing.lg))
                        .gap(px(theme.spacing.sm))
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Subtitle,
                            "Delete this workspace?",
                        ))
                        .child(
                            div()
                                .row()
                                .gap(px(theme.spacing.sm))
                                .child(
                                    Button::new("scene.overlay.cancel")
                                        .label("Cancel")
                                        .secondary()
                                        .on_click(|_, _| {}),
                                )
                                .child(
                                    Button::new("scene.overlay.confirm")
                                        .label("Delete")
                                        .danger()
                                        .on_click(|_, _| {}),
                                ),
                        ),
                ),
        )
        .into_any_element()
}

/// The dialog the scene shows, kept across frames.
///
/// A dialog owns whether it is open and which element had the keyboard before
/// it opened, so rebuilding it every frame would reopen it every frame.
struct SceneDialog {
    replace: Entity<Dialog>,
}

impl Global for SceneDialog {}

fn dialog(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneDialog>() {
        let replace = cx.new(|cx| {
            Dialog::new("scene.dialog.replace", window, cx)
                .title("Replace the existing theme?")
                .description(
                    "The application owns this decision. The dialog presents it and reports what \
                     was chosen.",
                )
                .cancel_label("Cancel")
                .confirm_label("Replace")
        });
        replace.update(cx, |dialog, cx| dialog.open(window, cx));
        cx.set_global(SceneDialog { replace });
    }
    let replace = cx.global::<SceneDialog>().replace.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(560.0))
        .h(px(360.0))
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "Content behind the dialog",
        ))
        .child(replace)
        .into_any_element()
}

fn tooltip(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(
            row(&theme).child(
                div()
                    .id("scene.tooltip.host")
                    .tip("scene.tooltip.export", "Writes the theme to a file on disk")
                    .child(
                        Button::new("scene.tooltip.export")
                            .label("Export theme")
                            .accessible_description("Writes the theme to a file on disk")
                            .secondary()
                            .on_click(|_, _| {}),
                    ),
            ),
        )
        // Hover help only exists while a pointer rests on the control, so the
        // surface itself is also shown outright, where it can be reviewed.
        .child(
            row(&theme).child(
                Tooltip::new("scene.tooltip.help", "Writes the theme to a file on disk")
                    .describes("scene.tooltip.export"),
            ),
        )
        .into_any_element()
}

/// The menu family the scenes show, kept across frames.
///
/// Each of these owns whether it is open, where the keyboard is, and which
/// submenu stands expanded, so rebuilding them every frame would reopen them
/// every frame. Building them once is also what makes the capture static.
struct SceneMenus {
    menu: Entity<Menu>,
    context: Entity<ContextMenu>,
    palette: Entity<CommandPalette>,
    popover: Entity<Popover>,
}

impl Global for SceneMenus {}

fn menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::section("group", "This run"),
        MenuItem::command("copy", "Copy run id")
            .icon(Icon::Copy)
            .shortcut("cmd-c"),
        MenuItem::check("follow", "Follow output", true),
        MenuItem::separator("rule"),
        MenuItem::command("publish", "Publish").disabled(true),
        MenuItem::submenu(
            "share",
            "Share",
            [
                MenuItem::command("share.link", "Copy link").shortcut("cmd-shift-c"),
                MenuItem::command("share.export", "Export as file"),
            ],
        ),
    ]
}

fn scene_commands() -> Vec<Command> {
    vec![
        Command::new("workspace.open", "Open workspace")
            .section("Workspace")
            .shortcut("cmd-o"),
        Command::new("workspace.close", "Close workspace").section("Workspace"),
        Command::new("workspace.publish", "Publish workspace")
            .section("Workspace")
            .unavailable("Approval is required"),
        Command::new("editor.wrap", "Toggle word wrap").section("Editor"),
    ]
}

fn ensure_menus(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneMenus>() {
        return;
    }
    let menu = cx.new(|cx| {
        Menu::new("scene.menu.run", window, cx)
            .trigger("Run actions")
            .items(menu_items())
    });
    menu.update(cx, |menu, cx| {
        menu.open_submenu("share", window, cx);
    });

    let context = cx.new(|cx| {
        ContextMenu::new("scene.context.run", window, cx)
            .name("Run actions")
            .target("run-a04")
            .menu(menu_items())
            .content(|_, cx| {
                let theme = cx.theme().clone();
                div()
                    .w(px(320.0))
                    .p(px(theme.spacing.md))
                    .hairline(&theme)
                    .radius(&theme, Radius::Card)
                    .child(crate::foundation::text(
                        &theme,
                        TypeScale::Body,
                        "Right-click this fixture row",
                    ))
                    .into_any_element()
            })
    });
    context.update(cx, |context, cx| {
        context.open_at(gpui::point(px(180.0), px(150.0)), window, cx);
    });

    let palette = cx.new(|cx| {
        CommandPalette::new("scene.palette.commands", window, cx).commands(scene_commands())
    });
    palette.update(cx, |palette, cx| palette.set_query("work", cx));

    let popover = cx.new(|cx| {
        Popover::new("scene.popover.filters", window, cx)
            .trigger("Filters")
            .content(|_, cx| {
                let theme = cx.theme().clone();
                div()
                    .column()
                    .w(px(260.0))
                    .gap(px(theme.spacing.sm))
                    .child(crate::foundation::text(
                        &theme,
                        TypeScale::Body,
                        "Anything can live in a popover.",
                    ))
                    .child(
                        Checkbox::new("scene.popover.failing")
                            .label("Failing runs only")
                            .on_change(|_, _, _| {}),
                    )
                    .into_any_element()
            })
    });
    popover.update(cx, |popover, cx| popover.open(window, cx));

    cx.set_global(SceneMenus {
        menu,
        context,
        palette,
        popover,
    });
}

fn popover(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_menus(window, cx);
    let popover = cx.global::<SceneMenus>().popover.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .h(px(320.0))
        // The trigger keeps its place while the surface is open, because the
        // surface is anchored to it rather than laid out beside it.
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "The trigger owns whether the surface is open.",
        ))
        .child(popover)
        .into_any_element()
}

fn menu(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_menus(window, cx);
    let menu = cx.global::<SceneMenus>().menu.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .h(px(360.0))
        .child(menu)
        .into_any_element()
}

fn context_menu(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_menus(window, cx);
    let context = cx.global::<SceneMenus>().context.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .h(px(400.0))
        // Opening a context menu reports the row that was pointed at; what is
        // selected stays the host's answer.
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "The right-click reports the row. Nothing is selected by it.",
        ))
        .child(context)
        .into_any_element()
}

fn command_palette(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_menus(window, cx);
    let palette = cx.global::<SceneMenus>().palette.clone();
    let theme = cx.theme().clone();
    // A palette is summoned over whatever is on screen, so it sits centred
    // near the top of the surface rather than in a corner of it.
    stack(&theme)
        .w_full()
        .h(px(420.0))
        .items_center()
        .pt(px(theme.spacing.xxl))
        .child(palette)
        .into_any_element()
}

/// The notification layer the scene shows, kept across frames.
///
/// The stack, each timer, and each entry animation outlive a frame, so the
/// layer is built once and the toasts are pushed once with it.
struct SceneToasts {
    layer: Entity<ToastLayer>,
}

impl Global for SceneToasts {}

fn toast(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneToasts>() {
        let layer = cx.new(|cx| ToastLayer::new(cx).capacity(4));
        cx.set_global(SceneToasts { layer });
        toast_push(
            cx,
            Toast::new("scene.toast.saved", "Theme exported to disk").tone(Tone::Success),
        );
        toast_push(
            cx,
            Toast::new("scene.toast.stale", "Refreshing the model catalog failed")
                .tone(Tone::Warning)
                .detail("The last verified catalog is still shown."),
        );
        toast_push(
            cx,
            // A refusal, so the offer is the thing that could change the
            // answer. Retrying an unapproved call only gets refused again.
            Toast::new(
                "scene.toast.refused",
                "The host refused to publish this run",
            )
            .tone(Tone::Warning)
            .detail("Approval is required for this workspace.")
            .action("Request approval", |_, _| {}),
        );
        // The failure beside the refusal, so the two tones are on screen
        // together and the difference between them is a picture rather than a
        // claim.
        toast_push(
            cx,
            Toast::new("scene.toast.failed", "Publishing this run failed")
                .tone(Tone::Danger)
                .detail("The publish service did not respond.")
                .action("Try again", |_, _| {}),
        );
    }
    let layer = cx.global::<SceneToasts>().layer.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(560.0))
        .h(px(360.0))
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "Content behind the notifications",
        ))
        // A failure keeps its report on screen; only the success times out.
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "A danger or warning toast stays until it is dismissed.",
        ))
        .child(layer)
        .into_any_element()
}

fn tabs(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(
            Tabs::new("scene.tabs.workspace")
                .tabs([
                    TabItem::new("overview", "Overview").icon(Icon::Widget),
                    TabItem::new("runs", "Runs").badge("12"),
                    TabItem::new("logs", "Logs"),
                    TabItem::new("billing", "Billing").disabled(true),
                ])
                .selected("runs")
                .on_select(|_, _, _| {}),
        )
        // The body belongs to the caller: tabs render the strip only.
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "Runs are rendered by the caller, not by the strip.",
        ))
        .into_any_element()
}

/// The overflow menu the document-tab scene hangs off the strip.
///
/// The strip does not own it: a menu is an entity with an open state that
/// outlives a frame, so the caller builds it and the strip fills it.
struct SceneDocumentTabs {
    overflow: Entity<Menu>,
}

impl Global for SceneDocumentTabs {}

fn document_tabs(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneDocumentTabs>() {
        // An icon-only trigger still has to be nameable, so it carries the
        // strip's own word for what moved into it.
        let name = cx.strings().text(StringKey::TabMoreTabs);
        let overflow = cx.new(|cx| {
            Menu::new("scene.document-tabs.overflow", window, cx)
                .trigger_icon(Icon::AltArrowDown)
                .trigger_name(name)
        });
        cx.set_global(SceneDocumentTabs { overflow });
    }
    let overflow = cx.global::<SceneDocumentTabs>().overflow.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(620.0))
        .child(caption(
            &theme,
            "clean, unsaved, saving, save failed — three marks, and silence for the fourth",
        ))
        .child(
            Tabs::new("scene.document-tabs.editor")
                .tabs([
                    TabItem::new("readme", "README.md").closable(true),
                    TabItem::new("main", "main.rs").dirty().closable(true),
                    TabItem::new("theme", "theme.json").saving().closable(true),
                    TabItem::new("notes", "notes.md")
                        .save_failed("The workspace is read-only.")
                        .closable(true),
                ])
                .selected("main")
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {}),
        )
        .child(caption(
            &theme,
            "past the declared limit the rest go to a menu, which stays reachable from the keyboard",
        ))
        .child(
            Tabs::new("scene.document-tabs.overflowing")
                .tabs([
                    TabItem::new("one", "adapter.rs").closable(true),
                    TabItem::new("two", "catalog.rs").dirty().closable(true),
                    TabItem::new("three", "harness.rs").closable(true),
                    TabItem::new("four", "registry.rs").closable(true),
                    TabItem::new("five", "transport.rs").dirty().closable(true),
                ])
                .selected("two")
                .overflow_after(3)
                .overflow_menu(overflow)
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {}),
        )
        .into_any_element()
}

/// The search surfaces the scene shows, kept across frames.
///
/// Both hold a [`TextInput`], which owns a caret and a selection that outlive
/// a frame, so they are built once and driven once.
struct SceneSearch {
    field: Entity<SearchField>,
    replace: Entity<FindReplace>,
}

impl Global for SceneSearch {}

fn ensure_search(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneSearch>() {
        return;
    }
    let field = cx.new(|cx| SearchField::new("scene.search.field", window, cx));
    field.update(cx, |field, cx| {
        field.set_query("transport", cx);
        field.set_count(
            HitCount::Known {
                total: 12,
                current: Some(2),
            },
            cx,
        );
    });

    let replace = cx.new(|cx| FindReplace::new("scene.search.replace", window, cx));
    replace.update(cx, |replace, cx| {
        replace.search_field().update(cx, |field, cx| {
            field.set_query("transport", cx);
        });
        replace.replacement_input().update(cx, |input, cx| {
            input.set_value("delivery", cx);
        });
        replace.set_count(
            HitCount::Known {
                total: 12,
                current: Some(2),
            },
            cx,
        );
    });

    cx.set_global(SceneSearch { field, replace });
}

fn search_field(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_search(window, cx);
    let field = cx.global::<SceneSearch>().field.clone();
    let theme = cx.theme().clone();

    // The three counts a field must keep apart. No pointer position produces
    // them at once, so each is stated.
    stack(&theme)
        .w(px(620.0))
        .child(field)
        .child(caption(
            &theme,
            "counting is not none, and too many is not a total",
        ))
        .child(
            row(&theme)
                .child(hit_count_sample(&theme, "counting", HitCount::Counting))
                .child(hit_count_sample(&theme, "none", HitCount::None))
                .child(hit_count_sample(
                    &theme,
                    "too many",
                    HitCount::TooMany { counted: 500 },
                )),
        )
        .child(caption(&theme, "the current hit is not the other hits"))
        .child(
            div().child(
                HighlightedText::new(
                    "The transport reports what it did; the transport never decides.",
                )
                .id("scene.search.line")
                .hits([4..13, 39..48])
                .current(1),
            ),
        )
        .into_any_element()
}

/// One hit count, rendered on its own so the states can be seen side by side.
fn hit_count_sample(theme: &Theme, label: &'static str, count: HitCount) -> gpui::Div {
    div()
        .column()
        .gap(px(theme.spacing.xs))
        .child(caption(theme, label))
        .child(
            div()
                .px(px(theme.spacing.sm))
                .py(px(theme.spacing.xs))
                .hairline(theme)
                .radius(theme, Radius::Control)
                .child(crate::foundation::text(
                    theme,
                    TypeScale::Label,
                    count.name(),
                )),
        )
}

fn find_replace(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_search(window, cx);
    let replace = cx.global::<SceneSearch>().replace.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(620.0))
        // Replace all says how many before it does any, so nobody agrees to a
        // number they were never shown.
        .child(caption(
            &theme,
            "replace all names its count before it acts",
        ))
        .child(replace)
        .into_any_element()
}

/// The notification centre the scene shows, kept across frames.
struct SceneNotifications {
    centre: Entity<NotificationCenter>,
}

impl Global for SceneNotifications {}

fn notification_center(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneNotifications>() {
        let centre = cx.new(|cx| NotificationCenter::new("scene.notifications", cx));
        centre.update(cx, |centre, cx| {
            centre.record(
                Notification::new("scene.notify.exported", "Theme exported to disk")
                    .tone(Tone::Success)
                    .at("9:41")
                    .read(true),
                cx,
            );
            centre.record(
                Notification::new("scene.notify.stale", "Refreshing the model catalog failed")
                    .tone(Tone::Warning)
                    .detail("The last verified catalog is still shown.")
                    .at("9:44"),
                cx,
            );
            centre.record(
                Notification::new(
                    "scene.notify.refused",
                    "The host refused to publish this run",
                )
                .tone(Tone::Warning)
                .detail("Approval is required for this workspace.")
                .at("9:46")
                .action("Request approval", |_, _| {}),
                cx,
            );
        });
        cx.set_global(SceneNotifications { centre });
    }
    let centre = cx.global::<SceneNotifications>().centre.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        // The same three reports the toast scene shows, after their toasts
        // have gone.
        .child(caption(
            &theme,
            "what the toasts said, still here once they timed out",
        ))
        .child(centre)
        .into_any_element()
}

fn failure_panel(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    // A failure, not a refusal. Retrying a refusal would only be refused
    // again, so the retry here is offered against something that can succeed
    // on a second attempt; refusals are shown by ToolCallCard instead.
    let failed: Result<(), &str> = Err("The runs service did not respond.");
    stack(&theme)
        .w(px(560.0))
        .child(caption(
            &theme,
            "the host's own words, kept on screen; the retry belongs to the host",
        ))
        .children(
            FailurePanel::from_result("scene.failure.query", &failed).map(|panel| {
                panel
                    .title("Runs")
                    .detail("The connection timed out after 30 seconds.")
                    .attempts(3)
                    .on_retry(|_, _| {})
            }),
        )
        .into_any_element()
}

fn log_stream(_window: &mut Window, cx: &mut App) -> AnyElement {
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

fn scene_diff() -> Vec<DiffFile> {
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
                        tone: Tone::Accent,
                    }]),
                DiffLine::paired("cache", "    old_cache.read()", "    verified_cache.read()")
                    .old_number(41)
                    .new_number(41),
                DiffLine::added("audit", "    audit.record()").new_number(42),
                DiffLine::new("close", "}").old_number(42).new_number(43),
            ],
        )],
    )]
}

fn diff_view(_window: &mut Window, cx: &mut App) -> AnyElement {
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
        .into_any_element()
}

fn sparkline(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let points = [
        SparklinePoint::new(0.0, 0.20),
        SparklinePoint::new(0.14, 0.34),
        SparklinePoint::new(0.28, 0.29),
        SparklinePoint::new(0.42, 0.56),
        SparklinePoint::new(0.57, 0.48),
        SparklinePoint::new(0.71, 0.74),
        SparklinePoint::new(0.85, 0.66),
        SparklinePoint::new(1.0, 0.82),
    ];
    stack(&theme)
        .w(px(520.0))
        .child(caption(
            &theme,
            "normalized geometry with caller-formatted current, minimum and maximum",
        ))
        .child(Sparkline::new(
            "scene.sparkline.rate",
            "Fixture throughput",
            SparklineState::Ready(SparklineReading::new(
                points, "82 req/s", "20 req/s", "82 req/s",
            )),
        ))
        .child(Sparkline::new(
            "scene.sparkline.stale",
            "Fixture queue depth",
            SparklineState::Stale {
                reading: SparklineReading::new(points, "34 jobs", "8 jobs", "41 jobs"),
                reason: "The latest sample is unavailable; the verified reading remains.".into(),
            },
        ))
        .into_any_element()
}

fn code_view(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    // Spans are the caller's, in byte offsets into each line. Nothing here
    // parses anything.
    let lines = [
        CodeLine::new(40, "fn report(&self) -> Outcome {").spans([
            CodeSpan {
                range: 0..2,
                tone: Tone::Accent,
            },
            CodeSpan {
                range: 3..9,
                tone: Tone::Success,
            },
        ]),
        CodeLine::new(41, "    let verified = self.check();")
            .spans([CodeSpan {
                range: 4..7,
                tone: Tone::Accent,
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
        .into_any_element()
}

fn upload_list(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .child(caption(
            &theme,
            "a refusal is not a failure, and only a failure is offered a retry",
        ))
        .child(
            UploadList::new("scene.uploads")
                .dropzone(
                    Dropzone::new("scene.uploads.zone", "Drop files to attach")
                        .hint("PDF, PNG, or plain text")
                        .on_files(|_, _, _| {}),
                )
                .uploads([
                    Upload::new("brief", "brief.pdf").size("1.2 MB").done(),
                    Upload::new("capture", "capture.png")
                        .size("4.8 MB")
                        .uploading(0.4),
                    Upload::new("notes", "notes.txt").size("12 KB"),
                    Upload::new("archive", "archive.zip")
                        .size("240 MB")
                        .failed("The connection dropped."),
                    Upload::new("installer", "installer.exe")
                        .size("64 MB")
                        .refused("This zone does not take programs."),
                ])
                .on_retry(|_, _, _| {})
                .on_cancel(|_, _, _| {})
                .on_remove(|_, _, _| {}),
        )
        .into_any_element()
}

/// The cascader owns only the open surface and path, so its scene keeps one
/// view alive across capture frames.
struct SceneCascader {
    cascader: Entity<Cascader>,
}

impl Global for SceneCascader {}

fn cascader(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneCascader>() {
        let cascader = cx.new(|cx| {
            Cascader::new("scene.cascader", window, cx)
                .name("Fixture destination")
                .selected("release-notes")
                .options([
                    CascaderOption::new("guides", "Guides").children([
                        CascaderOption::new("getting-started", "Getting started"),
                        CascaderOption::new("configuration", "Configuration"),
                    ]),
                    CascaderOption::new("reference", "Reference").loading_children(),
                    CascaderOption::new("archive", "Archive").unavailable_children(
                        "The fixture host does not provide archived sections.",
                    ),
                    CascaderOption::new("release-notes", "Release notes"),
                    CascaderOption::new("managed", "Managed section").disabled(true),
                ])
        });
        cascader.update(cx, |cascader, cx| cascader.open(window, cx));
        cx.set_global(SceneCascader { cascader });
    }
    let theme = cx.theme().clone();
    let cascader = cx.global::<SceneCascader>().cascader.clone();
    stack(&theme)
        .w(px(680.0))
        .child(caption(
            &theme,
            "caller-owned hierarchy and value; the open path belongs only to the view",
        ))
        .child(cascader)
        .into_any_element()
}

struct SceneAnchorList {
    overflow: Entity<Menu>,
}

impl Global for SceneAnchorList {}

fn anchor_navigation(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneAnchorList>() {
        let label = cx.strings().text(StringKey::AnchorMoreSections);
        let overflow = cx.new(|cx| Menu::new("scene.anchor-list.menu", window, cx).trigger(label));
        cx.set_global(SceneAnchorList { overflow });
    }
    let theme = cx.theme().clone();
    let overflow = cx.global::<SceneAnchorList>().overflow.clone();
    stack(&theme)
        .w(px(700.0))
        .child(caption(
            &theme,
            "section intents only; the declared overflow moves anchors into a menu",
        ))
        .child(
            AnchorList::new("scene.anchor-list")
                .anchors([
                    Anchor::new("summary", "Summary"),
                    Anchor::new("inputs", "Inputs"),
                    Anchor::new("constraints", "Constraints"),
                    Anchor::new("verification", "Verification"),
                    Anchor::new("history", "History").disabled(true),
                ])
                .active("inputs")
                .overflow_after(3)
                .overflow_menu(overflow)
                .on_navigate(|_, _, _| {}),
        )
        .into_any_element()
}

fn diagnostics_surface(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let diagnostics = [
        Diagnostic::new(
            "fixture-error",
            DiagnosticSeverity::Error,
            DiagnosticLocation::new("fixture.rs:18"),
            "Fixture diagnostic: incompatible value",
        )
        .action(DiagnosticAction::new("inspect", "Inspect fixture")),
        Diagnostic::new(
            "fixture-warning",
            DiagnosticSeverity::Warning,
            DiagnosticLocation::new("fixture.rs:31"),
            "Fixture diagnostic: unused declaration",
        ),
        Diagnostic::new(
            "fixture-information",
            DiagnosticSeverity::Information,
            DiagnosticLocation::new("fixture.rs:47"),
            "Fixture diagnostic: a simpler form is available",
        ),
        Diagnostic::new(
            "fixture-hint",
            DiagnosticSeverity::Hint,
            DiagnosticLocation::new("fixture.rs:64"),
            "Fixture diagnostic: consider a descriptive name",
        ),
    ];
    stack(&theme)
        .w(px(760.0))
        .child(caption(
            &theme,
            "synthetic diagnostics; filters, selection, and actions are caller-owned intents",
        ))
        .child(
            DiagnosticsList::new("scene.diagnostics", Loadable::Ready(diagnostics.to_vec()))
                .selected("fixture-warning")
                .visible_rows(5)
                .on_filter(|_, _, _| {})
                .on_select(|_, _, _| {})
                .on_action(|_, _, _, _| {}),
        )
        .into_any_element()
}

fn accordion(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(
            Accordion::new("scene.accordion.settings")
                .expanded_ids(&["network"])
                .on_toggle(|_, _, _, _| {})
                .section(
                    AccordionSection::new("network", "Network")
                        .description("How this machine reaches a host")
                        .body(crate::foundation::text(
                            &theme,
                            TypeScale::Body,
                            "Requests go out over the system proxy.",
                        )),
                )
                .section(
                    AccordionSection::new("storage", "Storage")
                        .description("Where verified results are kept")
                        .body(crate::foundation::text(
                            &theme,
                            TypeScale::Body,
                            "Nothing is written outside the workspace.",
                        )),
                )
                .section(
                    AccordionSection::new("policy", "Managed by policy")
                        .description("This machine cannot change these")
                        .disabled(true)
                        .body(crate::foundation::text(
                            &theme,
                            TypeScale::Body,
                            "Set by the administrator.",
                        )),
                ),
        )
        .into_any_element()
}

fn breadcrumb(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(
            Breadcrumb::new("scene.breadcrumb.short")
                .crumbs([
                    Crumb::new("workspace", "Workspace"),
                    Crumb::new("runs", "Runs"),
                    Crumb::new("run-4821", "Indexing"),
                ])
                .on_select(|_, _, _| {}),
        )
        .child(
            Breadcrumb::new("scene.breadcrumb.long")
                .crumbs([
                    Crumb::new("workspace", "Workspace"),
                    Crumb::new("projects", "Projects"),
                    Crumb::new("gpui-kit", "gpui-kit"),
                    Crumb::new("runs", "Runs"),
                    Crumb::new("run-4821", "Indexing"),
                ])
                .max_visible(3)
                .on_select(|_, _, _| {})
                .on_reveal(|_, _, _| {}),
        )
        .into_any_element()
}

/// How many records the list fixture claims to hold.
///
/// The count exists to make the difference visible: the list publishes all of
/// it, and renders only the handful the viewport can show.
const FIXTURE_RECORDS: usize = 240;

/// One synthetic record. Nothing here stands for a product: the identity is a
/// fixture key and the label says so.
fn fixture_record(index: usize) -> (SharedString, SharedString) {
    (
        SharedString::from(format!("record-{index:04}")),
        SharedString::from(format!("Fixture record {index:04}")),
    )
}

fn list(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(420.0))
        .child(
            crate::foundation::text(
                &theme,
                TypeScale::Caption,
                SharedString::from(format!(
                    "{FIXTURE_RECORDS} fixture records; only the rendered ones publish"
                )),
            )
            .text_tone(&theme, TextTone::Muted),
        )
        .child(
            div()
                .hairline(&theme)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    List::new("scene.list.records", FIXTURE_RECORDS, |index, _, _| {
                        let (id, label) = fixture_record(index);
                        ListItem::new(id, label.clone()).text(label)
                    })
                    .selected(fixture_record(2).0)
                    .visible_rows(8)
                    .on_select(|_, _, _| {}),
                ),
        )
        .into_any_element()
}

fn table(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let state = |label: &'static str, tone: Tone| {
        Cell::new(Badge::new(label).tone(tone))
            .text(label)
            .published(true)
    };
    stack(&theme)
        .w(px(600.0))
        .child(
            Table::new("scene.table.runs")
                .columns([
                    Column::new("name", "Run").flex(2.0).sortable(true),
                    Column::new("state", "State").fixed(110.0),
                    Column::new("duration", "Duration")
                        .fixed(96.0)
                        .align(Align::End)
                        .sortable(true),
                ])
                .sorted_by("duration", SortDirection::Descending)
                .selected("run-b12")
                .rows([
                    Row::new("run-a04")
                        .text("Indexing")
                        .cell("name", "Indexing")
                        .cell("state", state("Ready", Tone::Success))
                        .cell("duration", "4m 12s"),
                    Row::new("run-b12")
                        .text("Verifying")
                        .cell("name", "Verifying")
                        .cell("state", state("Stale", Tone::Warning))
                        .cell("duration", "2m 08s"),
                    Row::new("run-c31")
                        .text("Publishing")
                        .cell("name", "Publishing")
                        .cell("state", state("Refused", Tone::Danger))
                        .cell("duration", "1m 44s"),
                    Row::new("run-d02")
                        .text("Archiving")
                        .disabled(true)
                        .cell("name", "Archiving")
                        .cell("state", state("Managed", Tone::Neutral))
                        .cell("duration", "0m 51s"),
                ])
                .visible_rows(6)
                .on_sort(|_, _, _, _| {})
                .on_select(|_, _, _| {}),
        )
        .into_any_element()
}

/// How many rows the fixture host has handed over, and how many exist behind
/// it. The two numbers differ on purpose: that gap is what the select-all box
/// and the bulk bar have to be honest about.
const FIXTURE_JOBS_LOADED: usize = 240;
const FIXTURE_JOBS_TOTAL: usize = 12_000;

/// One synthetic job. Nothing here stands for a product: the identity is a
/// fixture key and the label says so.
fn fixture_job(index: usize) -> (SharedString, SharedString, SharedString, SharedString) {
    const PHASES: [&str; 4] = ["Indexing", "Verifying", "Publishing", "Archiving"];
    const OWNERS: [&str; 3] = ["fixture-a", "fixture-b", "fixture-c"];
    (
        SharedString::from(format!("job-{index:04}")),
        SharedString::from(format!("{} {index:04}", PHASES[index % PHASES.len()])),
        SharedString::from(OWNERS[index % OWNERS.len()]),
        SharedString::from(format!("{}m {:02}s", index % 9 + 1, index * 7 % 60)),
    )
}

fn fixture_job_tone(index: usize) -> (&'static str, Tone) {
    match index % 4 {
        0 => ("Ready", Tone::Success),
        1 => ("Stale", Tone::Warning),
        2 => ("Refused", Tone::Danger),
        _ => ("Managed", Tone::Neutral),
    }
}

fn grid_columns() -> [GridColumn; 4] {
    [
        // Declared second, drawn first: a pinned column holds the left edge
        // whatever order the caller puts the columns in.
        GridColumn::new("owner", "Owner")
            .fixed(120.0)
            .reorderable(true)
            .editable(true),
        GridColumn::new("name", "Job")
            .flex(2.0)
            .min_width(140.0)
            .pinned(true)
            .sortable(true)
            .resizable(true),
        GridColumn::new("state", "State")
            .fixed(110.0)
            .reorderable(true),
        GridColumn::new("duration", "Duration")
            .fixed(104.0)
            .align(Align::End)
            .sortable(true)
            .resizable(true)
            .reorderable(true),
    ]
}

fn grid_row(index: usize) -> GridRow {
    let (id, name, owner, duration) = fixture_job(index);
    let (label, tone) = fixture_job_tone(index);
    GridRow::new(id)
        .text(name.clone())
        .cell("name", Cell::new(name.clone()).text(name).published(true))
        .cell("owner", Cell::new(owner.clone()).text(owner))
        .cell(
            "state",
            Cell::new(Badge::new(label).tone(tone))
                .text(label)
                .published(true),
        )
        .cell("duration", duration)
}

fn grid_detail(theme: &Theme, id: SharedString) -> AnyElement {
    div()
        .column()
        .gap(px(theme.spacing.xs))
        .child(
            crate::foundation::text(
                theme,
                TypeScale::Caption,
                SharedString::from(format!("Fixture detail for {id}")),
            )
            .text_tone(theme, TextTone::Muted),
        )
        .child(
            crate::foundation::text(
                theme,
                TypeScale::Body,
                SharedString::from(
                    "Only an opened row builds this region; the rest never ask for it.",
                ),
            )
            .text_tone(theme, TextTone::Muted),
        )
        .into_any_element()
}

fn data_grid(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let detail_theme = theme.clone();
    stack(&theme)
        .w(px(760.0))
        .child(
            BulkBar::new("scene.data-grid.bulk", 2)
                .total(FIXTURE_JOBS_TOTAL)
                .action(
                    Button::new("scene.data-grid.bulk.retry")
                        .label("Retry")
                        .secondary()
                        .small()
                        .on_click(|_, _| {}),
                )
                .action(
                    Button::new("scene.data-grid.bulk.archive")
                        .label("Archive")
                        .secondary()
                        .small()
                        .on_click(|_, _| {}),
                )
                .on_select_all(|_, _| {})
                .on_dismiss(|_, _| {}),
        )
        .child(
            DataGrid::new(
                "scene.data-grid.jobs",
                FIXTURE_JOBS_LOADED,
                |index, _, _| grid_row(index),
            )
            .total(FIXTURE_JOBS_TOTAL)
            .columns(grid_columns())
            .sorted_by("duration", SortDirection::Descending)
            .selection_mode(SelectionMode::Multiple)
            .selected(["job-0001", "job-0003"])
            .expanded([Expanded::new("job-0002", 2)])
            .detail_rows(2)
            .detail(move |id, _, _| grid_detail(&detail_theme, id))
            .visible_rows(9)
            .on_sort(|_, _, _, _| {})
            .on_select(|_, _, _| {})
            .on_resize(|_, _, _, _| {})
            .on_fit(|_, _, _| {})
            .on_reorder(|_, _, _| {})
            .on_expand(|_, _, _, _| {})
            .on_edit_request(|_, _, _, _| {})
            .on_edit(|_, _, _| {}),
        )
        .child(
            crate::foundation::text(
                &theme,
                TypeScale::Caption,
                SharedString::from(format!(
                    "{FIXTURE_JOBS_LOADED} rows loaded of {FIXTURE_JOBS_TOTAL}; only the drawn \
                     ones publish"
                )),
            )
            .text_tone(&theme, TextTone::Muted),
        )
        .into_any_element()
}

fn data_grid_editing(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(760.0))
        .child(
            DataGrid::new("scene.data-grid-editing.jobs", 6, |index, _, _| {
                grid_row(index)
            })
            .columns(grid_columns())
            .sorted_by("duration", SortDirection::Descending)
            .selection_mode(SelectionMode::Single)
            .selected(["job-0001"])
            .editing(Some(EditingCell::new("job-0001", "owner", "fixture-b")))
            .visible_rows(6)
            .on_sort(|_, _, _, _| {})
            .on_select(|_, _, _| {})
            .on_resize(|_, _, _, _| {})
            .on_edit_request(|_, _, _, _| {})
            .on_edit(|_, _, _| {}),
        )
        .child(
            crate::foundation::text(
                &theme,
                TypeScale::Caption,
                SharedString::from(
                    "Escape reverts, enter commits, tab commits and moves on. The grid never \
                     writes the value.",
                ),
            )
            .text_tone(&theme, TextTone::Muted),
        )
        .into_any_element()
}

fn tree_grid(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let rows = [
        ("workspace", "Workspace", 1, true, true, None),
        ("src", "src", 2, true, true, Some("workspace")),
        ("components", "components", 3, false, false, Some("src")),
        ("lib", "lib.rs", 3, false, false, Some("src")),
        ("docs", "docs", 2, true, false, Some("workspace")),
    ];
    stack(&theme)
        .w(px(720.0))
        .child(
            TreeGrid::new("scene.tree-grid.files", rows.len(), move |index, _, _| {
                let (id, name, level, branch, expanded, parent) = rows[index];
                let mut row = TreeGridRow::new(id, level)
                    .text(name)
                    .cell("name", Cell::new(name).text(name).published(true))
                    .cell("kind", if branch { "Folder" } else { "File" })
                    .cell("state", if expanded { "Expanded" } else { "Ready" });
                if branch {
                    row = row.branch(expanded);
                }
                if let Some(parent) = parent {
                    row = row.parent(parent);
                }
                row
            })
            .columns([
                GridColumn::new("name", "Name").flex(2.0),
                GridColumn::new("kind", "Kind").fixed(120.0),
                GridColumn::new("state", "State").flex(1.0),
            ])
            .selected("components")
            .visible_rows(5)
            .lines(GridLines::Rows)
            .on_select(|_, _, _| {})
            .on_expand(|_, _, _, _| {}),
        )
        .into_any_element()
}

fn tree(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(360.0))
        .child(
            Tree::new("scene.tree.workspace")
                .expanded_ids(&["workspace", "crates"])
                .selected("tokens")
                .nodes([
                    TreeNode::new("workspace", "workspace")
                        .icon(Icon::Folder)
                        .children([
                            TreeNode::new("crates", "crates")
                                .icon(Icon::Folder)
                                .children([
                                    TreeNode::new("kit", "gpui-kit").icon(Icon::Document),
                                    TreeNode::new("tokens", "gpui-kit-tokens").icon(Icon::Document),
                                ]),
                            TreeNode::new("docs", "docs")
                                .icon(Icon::Folder)
                                .children([TreeNode::new("components", "components.md")
                                    .icon(Icon::Document)]),
                        ]),
                    TreeNode::new("target", "target")
                        .icon(Icon::Archive)
                        .disabled(true)
                        .children([TreeNode::new("debug", "debug").icon(Icon::Folder)]),
                ])
                .on_toggle(|_, _, _, _| {})
                .on_select(|_, _, _| {}),
        )
        .into_any_element()
}

/// The same vocabulary an interface uses everywhere, rendered right to left.
///
/// The point of capturing this is that support for another reading direction
/// is a claim about pixels, and a claim about pixels is only checkable in a
/// picture. Everything here is a component that already existed; nothing was
/// drawn specially for the scene. What should have moved: the trail runs from
/// the right, the tree indents from the right and its shut chevrons point
/// left, the accordion header reads from the right, and the magnifier's
/// handle changed corners. What should not have moved: the checkmark, the
/// gear, and the downward chevron, because none of them mean "forward".
fn reading_direction(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let direction = cx.layout_direction();
    stack(&theme)
        .w(px(560.0))
        .child(
            Breadcrumb::new("scene.rtl.trail")
                .crumbs([
                    Crumb::new("workspace", "Workspace"),
                    Crumb::new("runs", "Runs"),
                    Crumb::new("run-4821", "Indexing"),
                ])
                .on_select(|_, _, _| {}),
        )
        .child(
            Tabs::new("scene.rtl.tabs")
                .tabs([
                    TabItem::new("overview", "Overview").icon(Icon::Widget),
                    TabItem::new("runs", "Runs").badge("12"),
                    TabItem::new("logs", "Logs"),
                ])
                .selected("runs")
                .on_select(|_, _, _| {}),
        )
        .child(
            div()
                .row_reading(direction)
                .gap(px(theme.space(Space::Md)))
                .items_start()
                .child(
                    div().w(px(240.0)).child(
                        Tree::new("scene.rtl.tree")
                            .expanded_ids(&["workspace"])
                            .selected("tokens")
                            .nodes([TreeNode::new("workspace", "workspace")
                                .icon(Icon::Folder)
                                .children([
                                    TreeNode::new("crates", "crates")
                                        .icon(Icon::Folder)
                                        .children([
                                            TreeNode::new("kit", "gpui-kit").icon(Icon::Document)
                                        ]),
                                    TreeNode::new("tokens", "tokens").icon(Icon::Document),
                                ])])
                            .on_toggle(|_, _, _, _| {})
                            .on_select(|_, _, _| {}),
                    ),
                )
                .child(
                    div().flex_1().child(
                        Accordion::new("scene.rtl.sections")
                            .expanded_ids(&["network"])
                            .on_toggle(|_, _, _, _| {})
                            .section(
                                AccordionSection::new("network", "Network")
                                    .description("How this machine reaches a host")
                                    .body(crate::foundation::text(
                                        &theme,
                                        TypeScale::Body,
                                        "Requests go out over the system proxy.",
                                    )),
                            )
                            .section(
                                AccordionSection::new("storage", "Storage")
                                    .description("Where verified results are kept"),
                            ),
                    ),
                ),
        )
        .child(
            div()
                .row_reading(direction)
                .gap(px(theme.space(Space::Md)))
                // Directional glyphs on the top row, fixed ones below the
                // names, so a reviewer can see which turned around.
                .child(IconView::new(Icon::Magnifier).large().muted())
                .child(IconView::new(Icon::AltArrowRight).large().muted())
                .child(IconView::new(Icon::Return).large().muted())
                .child(IconView::new(Icon::Check).large().tone(IconTone::Success))
                .child(IconView::new(Icon::Settings).large().muted())
                .child(IconView::new(Icon::AltArrowDown).large().muted())
                .child(
                    IconView::named("scene.rtl.alone", Icon::Danger, "Run failed")
                        .large()
                        .tone(IconTone::Danger),
                ),
        )
        .child(
            div()
                .row_reading(direction)
                .gap(px(theme.space(Space::Sm)))
                .child(
                    Button::new("scene.rtl.save")
                        .label("Save")
                        .primary()
                        .icon(Icon::Check)
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.rtl.cancel")
                        .label("Cancel")
                        .secondary()
                        .on_click(|_, _| {}),
                ),
        )
        .into_any_element()
}

fn choice(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    div()
        .flex()
        .flex_col()
        .gap(px(theme.space(Space::Md)))
        .p(px(theme.space(Space::Lg)))
        .w(px(360.0))
        .child(
            Checkbox::new("scene.choice.telemetry")
                .label("Send anonymous usage data")
                .description("Counts only, never file contents")
                .checked(true)
                .on_change(|_, _, _| {}),
        )
        .child(
            Checkbox::new("scene.choice.partial")
                .label("Some providers enabled")
                .mixed()
                .on_change(|_, _, _| {}),
        )
        .child(
            Checkbox::new("scene.choice.locked")
                .label("Managed by policy")
                .checked(true)
                .disabled(true),
        )
        .child(
            Radio::new("scene.choice.ask")
                .label("Ask before every action")
                .selected(true)
                .on_select(|_, _| {}),
        )
        .child(
            Radio::new("scene.choice.auto")
                .label("Run without asking")
                .description("Consequential actions still require approval")
                .on_select(|_, _| {}),
        )
        .child(
            Switch::new("scene.choice.preview")
                .label("Preview releases")
                .on(true)
                .on_change(|_, _, _| {}),
        )
        .child(
            Slider::new("scene.choice.temperature")
                .label("Temperature")
                .range(0.0, 2.0)
                .step(0.1)
                .value(0.7)
                .display("0.7")
                .on_change(|_, _, _| {}),
        )
        .into_any_element()
}

/// The form scene's controls, kept across frames.
///
/// Every one of these owns editing state — a caret, a query, an open list —
/// so they are built once. Building them once is also what makes the capture
/// static.
struct SceneForm {
    name: Entity<TextInput>,
    retention: Entity<NumberInput>,
    region: Entity<Combobox>,
    labels: Entity<TagInput>,
}

impl Global for SceneForm {}

fn ensure_form(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneForm>() {
        return;
    }
    let name = cx.new(|cx| {
        TextInput::new("scene.form.name", window, cx)
            .text("Runs 2024")
            .required(true)
            .invalid(true)
    });
    let retention = cx.new(|cx| {
        // The host holds ninety days while its own limit is sixty. The field
        // shows the number that is actually set and says it is out of range,
        // rather than quietly drawing a number nobody chose.
        NumberInput::new("scene.form.retention", window, cx)
            .value(90.0)
            .range(1.0, 60.0)
            .step(5.0)
            .unit("days")
            .required(true)
    });
    let region = cx.new(|cx| {
        let mut options = (0..14)
            .map(|index| {
                SelectOption::new(
                    format!("model-{index:02}"),
                    format!("Agent model {index:02}"),
                )
            })
            .collect::<Vec<_>>();
        options.push(SelectOption::new("unknown", "Unknown model").description(
            "This model may not support chat in direct mode.\nChoose a chat-capable model.",
        ));
        options.push(SelectOption::new("managed", "Managed model").disabled(true));
        Combobox::new("scene.form.region", window, cx)
            .name("All Agent models")
            .options(options)
            .selected("unknown")
            .placeholder("Choose an Agent model")
    });
    let labels = cx.new(|cx| {
        TagInput::new("scene.form.labels", window, cx)
            .tags(["indexing", "nightly", "verified"])
            .placeholder("Add a label")
            .max(5)
    });
    region.update(cx, |combobox, cx| {
        combobox.set_query("Unknown model", cx);
    });
    cx.set_global(SceneForm {
        name,
        retention,
        region,
        labels,
    });
}

fn form(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_form(window, cx);
    let form = cx.global::<SceneForm>();
    let (name, retention, region, labels) = (
        form.name.clone(),
        form.retention.clone(),
        form.region.clone(),
        form.labels.clone(),
    );
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(420.0))
        .h(px(920.0))
        .child(
            FormField::new("scene.form.name.form-field", "Workspace name")
                .control("scene.form.name")
                .required(true)
                // The description says what the field is for and the error
                // says what went wrong; neither answers for the other.
                .description("Shown wherever this workspace appears.")
                .error("A workspace with this name already exists.")
                .child(name),
        )
        .child(
            FormField::new("scene.form.retention.form-field", "Retention")
                .control("scene.form.retention")
                .required(true)
                .description("How long a finished run is kept.")
                .error("This workspace allows at most 60 days.")
                .child(retention),
        )
        .child(
            FormField::new("scene.form.visibility.form-field", "Visibility")
                .control("scene.form.visibility")
                .description("Who can open the runs in this workspace.")
                .child(
                    SegmentedControl::new("scene.form.visibility")
                        .label("Visibility")
                        .segments([
                            Segment::new("private", "Private"),
                            Segment::new("team", "Team"),
                            Segment::new("public", "Public").disabled(true),
                        ])
                        .selected("team")
                        .on_select(|_, _, _| {}),
                ),
        )
        .child(
            FormField::new("scene.form.labels.form-field", "Labels")
                .control("scene.form.labels")
                // The keystroke lives in the hint, so the description does not
                // spend a second line repeating it.
                .description("At most five, and each one only once.")
                .hint("enter")
                .child(labels),
        )
        .child(
            div().mt_auto().child(
                FormField::new("scene.form.region.form-field", "All Agent models")
                    .control("scene.form.region")
                    .description("Choose a chat-capable model for direct runs.")
                    .child(region),
            ),
        )
        .into_any_element()
}

/// The password in the canonical sign-in composition, kept across frames.
struct SceneAuthSignIn {
    password: Entity<PasswordInput>,
}

impl Global for SceneAuthSignIn {}

fn ensure_auth_sign_in(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneAuthSignIn>() {
        return;
    }
    let password = cx.new(|cx| {
        PasswordInput::new("scene.auth.sign-in.password", window, cx)
            .name("Password")
            .placeholder("Enter password")
            .required(true)
    });
    cx.set_global(SceneAuthSignIn { password });
}

fn auth_sign_in(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_auth_sign_in(window, cx);
    let password = cx.global::<SceneAuthSignIn>().password.clone();
    let theme = cx.theme().clone();
    let card_id = "scene.auth.sign-in.card";
    let title = crate::foundation::text(&theme, TypeScale::Title, "Sign in").semantic_in(
        cx,
        NodeSpec::new("scene.auth.sign-in.title", Role::Text)
            .parent(card_id)
            .text("Sign in"),
    );

    stack(&theme)
        .w(px(440.0))
        .child(
            Card::new().id(card_id).padded(true).child(
                div()
                    .column()
                    .gap_token(&theme, Space::Md)
                    .child(title)
                    .child(
                        Callout::new("Credentials are verified by the caller.", Tone::Neutral)
                            .id("scene.auth.sign-in.boundary"),
                    )
                    .child(
                        FormField::new("scene.auth.sign-in.password.field", "Password")
                            .control("scene.auth.sign-in.password")
                            .required(true)
                            .child(password),
                    )
                    .child(
                        Button::new("scene.auth.sign-in.submit")
                            .label("Sign in")
                            .full_width(true)
                            .on_click(|_, _| {}),
                    )
                    .child(
                        Button::new("scene.auth.sign-in.passkey")
                            .label("Continue with passkey")
                            .icon(Icon::Key)
                            .secondary()
                            .full_width(true)
                            .on_click(|_, _| {}),
                    )
                    .child(
                        Button::new("scene.auth.sign-in.organization")
                            .label("Continue with organization sign-on")
                            .icon(Icon::Global)
                            .secondary()
                            .full_width(true)
                            .on_click(|_, _| {}),
                    )
                    .child(
                        Button::new("scene.auth.sign-in.recovery")
                            .label("Use a recovery option")
                            .link()
                            .on_click(|_, _| {}),
                    ),
            ),
        )
        .into_any_element()
}

/// The one logical code input in the canonical verification composition.
struct SceneAuthVerification {
    code: Entity<OneTimeCodeInput>,
}

impl Global for SceneAuthVerification {}

fn ensure_auth_verification(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneAuthVerification>() {
        return;
    }
    let code = cx.new(|cx| {
        OneTimeCodeInput::new("scene.auth.verification.code", window, cx)
            .name("Verification code")
            .slots(6)
            .required(true)
    });
    cx.set_global(SceneAuthVerification { code });
}

fn auth_verification(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_auth_verification(window, cx);
    let code = cx.global::<SceneAuthVerification>().code.clone();
    let theme = cx.theme().clone();
    let card_id = "scene.auth.verification.card";
    let title = crate::foundation::text(&theme, TypeScale::Title, "Verify sign-in").semantic_in(
        cx,
        NodeSpec::new("scene.auth.verification.title", Role::Text)
            .parent(card_id)
            .text("Verify sign-in"),
    );

    stack(&theme)
        .w(px(440.0))
        .child(
            Card::new().id(card_id).padded(true).child(
                div()
                    .column()
                    .gap_token(&theme, Space::Md)
                    .child(title)
                    .child(
                        Callout::new(
                            "Enter the code from your authenticator or recovery method.",
                            Tone::Neutral,
                        )
                        .id("scene.auth.verification.guidance"),
                    )
                    .child(
                        FormField::new("scene.auth.verification.code.field", "Verification code")
                            .control("scene.auth.verification.code")
                            .required(true)
                            .child(code),
                    )
                    .child(
                        Button::new("scene.auth.verification.submit")
                            .label("Verify")
                            .full_width(true)
                            .on_click(|_, _| {}),
                    )
                    .child(
                        Button::new("scene.auth.verification.alternative")
                            .label("Use another method")
                            .secondary()
                            .full_width(true)
                            .on_click(|_, _| {}),
                    )
                    .child(
                        Button::new("scene.auth.verification.recovery")
                            .label("Use a recovery option")
                            .link()
                            .on_click(|_, _| {}),
                    ),
            ),
        )
        .into_any_element()
}

/// The split button the actions scene shows, kept across frames.
struct SceneActions {
    split: Entity<SplitButton>,
}

impl Global for SceneActions {}

fn ensure_actions(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneActions>() {
        return;
    }
    let split = cx.new(|cx| {
        SplitButton::new("scene.actions.publish", window, cx)
            .label("Publish")
            .primary()
            .on_click(|_, _| {})
            .items(
                [
                    MenuItem::command("publish.draft", "Save as draft"),
                    MenuItem::command("publish.schedule", "Schedule…").shortcut("cmd-shift-s"),
                    MenuItem::separator("publish.rule"),
                    MenuItem::command("publish.export", "Export without publishing"),
                ],
                cx,
            )
    });
    split.update(cx, |split, cx| split.open_menu(window, cx));
    cx.set_global(SceneActions { split });
}

fn actions(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_actions(window, cx);
    let split = cx.global::<SceneActions>().split.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(560.0))
        .h(px(360.0))
        .child(
            row(&theme)
                .child(
                    IconButton::new("scene.actions.copy", Icon::Copy, "Copy run id")
                        .on_click(|_, _| {}),
                )
                .child(
                    IconButton::new("scene.actions.rename", Icon::Pen, "Rename run")
                        .secondary()
                        .on_click(|_, _| {}),
                )
                .child(
                    IconButton::new("scene.actions.refresh", Icon::Refresh, "Refresh")
                        .secondary()
                        .loading(true)
                        .on_click(|_, _| {}),
                )
                .child(
                    IconButton::new("scene.actions.delete", Icon::Trash, "Delete run")
                        .danger()
                        .on_click(|_, _| {}),
                )
                .child(
                    IconButton::new("scene.actions.archive", Icon::Archive, "Archive run")
                        .secondary()
                        .disabled(true)
                        .on_click(|_, _| {}),
                ),
        )
        .child(
            row(&theme).child(
                ButtonGroup::new("scene.actions.range")
                    .children([
                        Button::new("scene.actions.range.day")
                            .label("Day")
                            .secondary()
                            .on_click(|_, _| {}),
                        Button::new("scene.actions.range.week")
                            .label("Week")
                            .secondary()
                            .selected(true)
                            .on_click(|_, _| {}),
                        Button::new("scene.actions.range.month")
                            .label("Month")
                            .secondary()
                            .on_click(|_, _| {}),
                    ])
                    .small(),
            ),
        )
        .child(row(&theme).child(split))
        .into_any_element()
}

fn content(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    div()
        .flex()
        .flex_col()
        .gap(px(theme.space(Space::Lg)))
        .p(px(theme.space(Space::Lg)))
        .w(px(420.0))
        .child(
            ProgressBar::new("scene.content.upload")
                .label("Indexing workspace")
                .count(3, 12),
        )
        .child(ProgressBar::new("scene.content.unknown").label("Contacting host"))
        .child(Divider::new().id("scene.content.rule").label("Filters"))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(theme.space(Space::Xs)))
                .child(Tag::new("scene.content.tag.rust", "rust").on_remove(|_, _| {}))
                .child(
                    Tag::new("scene.content.tag.failing", "failing")
                        .tone(Tone::Danger)
                        .on_remove(|_, _| {}),
                )
                .child(Tag::new("scene.content.tag.pinned", "pinned").disabled(true)),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(theme.space(Space::Sm)))
                .child(Avatar::new("Ada Lovelace").id("scene.content.avatar"))
                .child(Avatar::new("").size(24.0)),
        )
        .child(
            EmptyState::new("scene.content.empty", "No runs yet")
                .kind(EmptyKind::Unstarted)
                .detail("A run appears here once one has been started."),
        )
        .child(
            EmptyState::new("scene.content.refused", "The host refused the request")
                .kind(EmptyKind::Unavailable)
                .detail("Approval is required for this workspace.")
                .action(
                    Button::new("scene.content.retry")
                        .label("Try again")
                        .secondary()
                        .on_click(|_, _| {}),
                ),
        )
        .into_any_element()
}

/// The inputs the scene shows, kept across frames.
///
/// An editable control carries state, so the scene builds its entities once
/// rather than on every frame, which would discard whatever was typed.
struct SceneInputs {
    token: Entity<TextInput>,
    disabled: Entity<TextInput>,
    invalid: Entity<TextInput>,
    provider: Entity<Select>,
    notes: Entity<TextArea>,
    review: Entity<TextArea>,
    frozen: Entity<TextArea>,
}

impl Global for SceneInputs {}

fn ensure_inputs(window: &mut Window, cx: &mut App) {
    if !cx.has_global::<SceneInputs>() {
        let inputs = SceneInputs {
            token: cx.new(|cx| {
                TextInput::new("scene.input.token", window, cx)
                    .name("API token")
                    .placeholder("sk-...")
                    .secret(true)
            }),
            disabled: cx.new(|cx| {
                TextInput::new("scene.input.disabled", window, cx)
                    .name("Disabled")
                    .text("read only")
                    .disabled(true)
            }),
            invalid: cx.new(|cx| {
                TextInput::new("scene.input.invalid", window, cx)
                    .name("Email")
                    .text("not an email")
                    .invalid(true)
                    .required(true)
            }),
            provider: cx.new(|cx| {
                Select::new("scene.input.provider", window, cx)
                    .name("Provider")
                    .options([
                        SelectOption::new("anthropic", "Anthropic"),
                        SelectOption::new("openai", "OpenAI").description("Requires a key"),
                        SelectOption::new("local", "Local runtime").disabled(true),
                    ])
                    .selected("anthropic")
                    .placeholder("Choose a provider")
            }),
            notes: cx.new(|cx| {
                TextArea::new("scene.textarea.notes", window, cx)
                    .text(
                        "The refusal is shown exactly as the host worded it, and the last \
                         verified value stays on screen.",
                    )
                    .rows(3)
                    .max_rows(6)
            }),
            review: cx.new(|cx| {
                TextArea::new("scene.textarea.review", window, cx)
                    .placeholder("What changed, and why")
                    .rows(3)
            }),
            frozen: cx.new(|cx| {
                TextArea::new("scene.textarea.frozen", window, cx)
                    .text("Set by the administrator.\nThis machine cannot change it.")
                    .rows(2)
                    .disabled(true)
            }),
        };
        // A caret only paints where the keyboard is, so one area takes it:
        // otherwise a capture cannot show a caret at all.
        window.focus(&inputs.review.read(cx).focus_handle(cx), cx);
        cx.set_global(inputs);
    }
}

fn input(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_inputs(window, cx);
    let inputs = cx.global::<SceneInputs>();
    let (token, disabled, invalid, provider) = (
        inputs.token.clone(),
        inputs.disabled.clone(),
        inputs.invalid.clone(),
        inputs.provider.clone(),
    );
    let theme = cx.theme().clone();

    div()
        .flex()
        .flex_col()
        .gap(px(theme.space(Space::Md)))
        .p(px(theme.space(Space::Lg)))
        .w(px(360.0))
        .child(token)
        .child(disabled)
        .child(invalid)
        .child(provider)
        .into_any_element()
}

fn textarea(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_inputs(window, cx);
    let inputs = cx.global::<SceneInputs>();
    let (notes, review, frozen) = (
        inputs.notes.clone(),
        inputs.review.clone(),
        inputs.frozen.clone(),
    );
    let theme = cx.theme().clone();

    div()
        .flex()
        .flex_col()
        .gap(px(theme.space(Space::Md)))
        .p(px(theme.space(Space::Lg)))
        .w(px(360.0))
        .child(notes)
        .child(review)
        .child(frozen)
        .into_any_element()
}

/// A pane of fixture copy, for a scene that needs something on both sides of a
/// divider or below a viewport.
fn filler(theme: &Theme, title: &'static str, lines: usize) -> gpui::Div {
    let mut pane = div()
        .flex()
        .flex_col()
        .gap(px(theme.space(Space::Xs)))
        .p(px(theme.space(Space::Md)))
        .child(crate::foundation::text(
            theme,
            TypeScale::Label,
            SharedString::from(title),
        ));
    for line in 1..=lines {
        pane = pane.child(
            crate::foundation::text(
                theme,
                TypeScale::Code,
                SharedString::from(format!("Fixture line {line:02}")),
            )
            .text_tone(theme, TextTone::Muted),
        );
    }
    pane
}

fn split_pane(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(620.0))
        .h(px(380.0))
        .child(
            div()
                .h(px(320.0))
                .hairline(&theme)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    SplitPane::new("scene.split.workspace")
                        .horizontal()
                        .ratio(0.34)
                        .min_sizes(120.0, 200.0)
                        .collapsible(true)
                        .handle_label("Resize the file tree")
                        .start(filler(&theme, "Files", 6))
                        .end(filler(&theme, "Editor", 8))
                        .on_resize(|_, _, _| {})
                        .on_collapse(|_, _, _| {}),
                ),
        )
        .into_any_element()
}

/// The same region, already scrolled, so the shadow that says there is content
/// above the fold is in a captured image rather than only in a test.
fn scroll_shadow(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    crate::layout::scroll_to("scene.scroll.scrolled", gpui::point(px(0.0), px(120.0)), cx);
    stack(&theme)
        .w(px(480.0))
        .child(
            div()
                .hairline(&theme)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    ScrollArea::new("scene.scroll.scrolled")
                        .label("Run output")
                        .vertical()
                        .height(200.0)
                        .child(filler(&theme, "Output", 20)),
                ),
        )
        .into_any_element()
}

fn scroll_area(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(480.0))
        .child(
            div()
                .hairline(&theme)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    ScrollArea::new("scene.scroll.output")
                        .label("Run output")
                        .vertical()
                        .height(200.0)
                        .child(filler(&theme, "Output", 20)),
                ),
        )
        // Nothing overflows here, so no scrollbar is drawn or published.
        .child(
            div()
                .hairline(&theme)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    ScrollArea::new("scene.scroll.summary")
                        .label("Summary")
                        .vertical()
                        .height(120.0)
                        .child(filler(&theme, "Summary", 2)),
                ),
        )
        .into_any_element()
}

/// A region scrolled off both ends, so the fade that says there is more in
/// either direction is in a captured image, beside one that hides nothing and
/// therefore fades at neither edge.
fn scroll_fade(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    crate::layout::scroll_to("scene.fade.output", gpui::point(px(0.0), px(120.0)), cx);
    stack(&theme)
        .w(px(480.0))
        .child(caption(&theme, "Scrolled: content runs past both ends"))
        .child(
            div()
                .hairline(&theme)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    ScrollFade::new("scene.fade.scrolled")
                        .edges(FadeEdges::vertical())
                        .fit_height()
                        .child(
                            ScrollArea::new("scene.fade.output")
                                .label("Run output")
                                .vertical()
                                .height(200.0)
                                .child(filler(&theme, "Output", 20)),
                        ),
                ),
        )
        // Nothing is hidden here, so no edge fades and the caller says so.
        .child(caption(&theme, "Nothing hidden: no edge fades"))
        .child(
            div()
                .hairline(&theme)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    ScrollFade::new("scene.fade.settled").fit_height().child(
                        ScrollArea::new("scene.fade.summary")
                            .label("Summary")
                            .vertical()
                            .height(120.0)
                            .child(filler(&theme, "Summary", 2)),
                    ),
                ),
        )
        .into_any_element()
}

/// Glass over the page it covers, which is the only way to see that the
/// backdrop is blurred rather than merely tinted.
fn frost(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let card = |title: &'static str, body: &'static str| {
        div()
            .column()
            .gap(px(theme.space(Space::Xs)))
            .p(px(theme.space(Space::Md)))
            .child(crate::foundation::text(
                &theme,
                TypeScale::Label,
                SharedString::from(title),
            ))
            .child(caption(&theme, body))
    };
    let stripes = || {
        div()
            .absolute()
            .top(px(56.0))
            .left(px(100.0))
            .w(px(330.0))
            .h(px(64.0))
            .flex()
            .overflow_hidden()
            .children((0..22).map(|index| {
                div()
                    .flex_none()
                    .w(px(15.0))
                    .h(px(64.0))
                    .bg(if index % 2 == 0 {
                        theme.colors.accent
                    } else {
                        theme.colors.canvas
                    })
            }))
    };
    stack(&theme)
        .w(px(480.0))
        .child(caption(&theme, "A floating surface on glass"))
        .child(
            div()
                .relative()
                .h(px(200.0))
                .hairline(&theme)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(stripes())
                .child(filler(&theme, "Document", 8))
                .child(
                    div()
                        .absolute()
                        .top(px(48.0))
                        .left(px(120.0))
                        .w(px(240.0))
                        .child(
                            Frost::new("scene.frost.popover")
                                .radius(Radius::Card)
                                .child(card(
                                    "Rename",
                                    "The page behind stays visible, out of focus",
                                )),
                        ),
                ),
        )
        .child(caption(&theme, "The same glass over a panel surface"))
        .child(
            div()
                .relative()
                .h(px(160.0))
                .hairline(&theme)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(stripes())
                .child(filler(&theme, "Files", 6))
                .child(
                    div()
                        .absolute()
                        .top(px(32.0))
                        .left(px(120.0))
                        .w(px(280.0))
                        .child(
                            Frost::new("scene.frost.rail")
                                .surface(gpui_kit_theme::Surface::Panel)
                                .radius(Radius::Dialog)
                                .blur(32.0)
                                .child(card("Rail", "The striped backdrop stays out of focus")),
                        ),
                ),
        )
        .into_any_element()
}

/// The overflow menu of the toolbar scene, kept across frames.
struct SceneToolbar {
    overflow: Entity<Menu>,
}

impl Global for SceneToolbar {}

fn toolbar(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneToolbar>() {
        let overflow = cx.new(|cx| {
            Menu::new("scene.toolbar.overflow", window, cx)
                .trigger_icon(Icon::List)
                .trigger_name("More actions")
        });
        cx.set_global(SceneToolbar { overflow });
    }
    let overflow = cx.global::<SceneToolbar>().overflow.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(620.0))
        .child(
            Toolbar::new("scene.toolbar.editor")
                .label("Editor actions")
                .group(
                    "history",
                    [
                        ToolbarItem::new(
                            "editor.undo",
                            "Undo",
                            IconButton::new("scene.toolbar.undo", Icon::ArrowLeft, "Undo")
                                .ghost()
                                .small()
                                .on_click(|_, _| {}),
                        )
                        .icon(Icon::ArrowLeft)
                        .shortcut("cmd-z"),
                        ToolbarItem::new(
                            "editor.redo",
                            "Redo",
                            IconButton::new("scene.toolbar.redo", Icon::ArrowRight, "Redo")
                                .ghost()
                                .small()
                                .on_click(|_, _| {}),
                        )
                        .icon(Icon::ArrowRight),
                    ],
                )
                .group(
                    "view",
                    [ToolbarItem::new(
                        "editor.view",
                        "View",
                        SegmentedControl::new("scene.toolbar.view")
                            .label("View")
                            .segments([
                                Segment::new("code", "Code"),
                                Segment::new("split", "Split"),
                                Segment::new("preview", "Preview"),
                            ])
                            .selected("split")
                            .small()
                            .on_select(|_, _, _| {}),
                    )],
                )
                .spacer()
                .group(
                    "publish",
                    [
                        ToolbarItem::new(
                            "editor.share",
                            "Share",
                            Button::new("scene.toolbar.share")
                                .label("Share")
                                .secondary()
                                .small()
                                .on_click(|_, _| {}),
                        )
                        .icon(Icon::Copy),
                        ToolbarItem::new(
                            "editor.publish",
                            "Publish",
                            Button::new("scene.toolbar.publish")
                                .label("Publish")
                                .primary()
                                .small()
                                .on_click(|_, _| {}),
                        )
                        .icon(Icon::ArchiveUp),
                        ToolbarItem::new(
                            "editor.archive",
                            "Archive",
                            Button::new("scene.toolbar.archive")
                                .label("Archive")
                                .secondary()
                                .small()
                                .on_click(|_, _| {}),
                        )
                        .icon(Icon::Archive)
                        .disabled(true),
                    ],
                )
                .overflow_after(4)
                .overflow_menu(overflow),
        )
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "The last two actions moved into the overflow menu.",
        ))
        .into_any_element()
}

fn navigation_sections() -> Vec<SidebarSection> {
    vec![
        SidebarSection::new("work").title("Work").items([
            SidebarItem::new("runs", "Runs")
                .icon(Icon::List)
                .badge("12")
                .children([
                    SidebarItem::new("runs.active", "Active").icon(Icon::Refresh),
                    SidebarItem::new("runs.archived", "Archived").icon(Icon::Archive),
                ]),
            SidebarItem::new("files", "Files").icon(Icon::Folder),
        ]),
        SidebarSection::new("admin").title("Administration").items([
            SidebarItem::new("settings", "Settings").icon(Icon::Settings),
            SidebarItem::new("policy", "Managed by policy")
                .icon(Icon::Key)
                .disabled(true),
        ]),
    ]
}

fn sidebar(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let rail = |ident: &'static str, collapsed: bool| {
        Sidebar::new(ident)
            .sections(navigation_sections())
            .active("runs.active")
            .collapsed(collapsed)
            .footer(
                crate::foundation::text(
                    &theme,
                    TypeScale::Caption,
                    if collapsed {
                        SharedString::new_static("v0")
                    } else {
                        SharedString::new_static("Fixture workspace")
                    },
                )
                .text_tone(&theme, TextTone::Faint),
            )
            .on_select(|_, _, _| {})
    };

    stack(&theme)
        .h(px(420.0))
        .child(
            div()
                .flex()
                .flex_row()
                .h(px(360.0))
                .gap(px(theme.space(Space::Lg)))
                .child(rail("scene.sidebar.expanded", false))
                .child(rail("scene.sidebar.collapsed", true)),
        )
        .into_any_element()
}

/// The page-size control of the pagination scene, kept across frames.
struct ScenePagination {
    page_size: Entity<Select>,
}

impl Global for ScenePagination {}

fn pagination(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<ScenePagination>() {
        let page_size = cx.new(|cx| {
            Select::new("scene.pagination.size", window, cx)
                .name("Rows per page")
                .options([
                    SelectOption::new("25", "25 per page"),
                    SelectOption::new("50", "50 per page"),
                    SelectOption::new("100", "100 per page"),
                ])
                .selected("50")
        });
        cx.set_global(ScenePagination { page_size });
    }
    let page_size = cx.global::<ScenePagination>().page_size.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(620.0))
        .child(
            Pagination::new("scene.pagination.known")
                .page(9)
                .total_pages(20)
                .page_size(page_size)
                .on_select(|_, _, _| {}),
        )
        // A host that only knows there is another page says exactly that: no
        // last-page control, no numbers, and no total.
        .child(
            Pagination::new("scene.pagination.unknown")
                .page(3)
                .unknown_total(true)
                .on_select(|_, _, _| {}),
        )
        .into_any_element()
}

/// The drawer the scene shows, kept across frames and settled so the capture
/// photographs the panel where it comes to rest rather than mid-slide.
struct SceneDrawer {
    filters: Entity<Drawer>,
}

impl Global for SceneDrawer {}

fn drawer(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneDrawer>() {
        let filters = cx.new(|cx| {
            Drawer::new("scene.drawer.filters", window, cx)
                .edge(Edge::Right)
                .size(320.0)
                .title("Filter runs")
                .description("The drawer reports what was chosen. The host applies it.")
                .content(|_, cx| {
                    let theme = cx.theme().clone();
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(theme.space(Space::Sm)))
                        .child(
                            Checkbox::new("scene.drawer.failed")
                                .label("Failed runs only")
                                .checked(true)
                                .on_change(|_, _, _| {}),
                        )
                        .child(
                            Checkbox::new("scene.drawer.mine")
                                .label("Started by me")
                                .on_change(|_, _, _| {}),
                        )
                        .into_any_element()
                })
                .footer(|_, _| {
                    Button::new("scene.drawer.apply")
                        .label("Apply")
                        .primary()
                        .full_width(true)
                        .on_click(|_, _| {})
                        .into_any_element()
                })
        });
        filters.update(cx, |drawer, cx| {
            drawer.open(window, cx);
            drawer.settle(cx);
        });
        cx.set_global(SceneDrawer { filters });
    }
    let filters = cx.global::<SceneDrawer>().filters.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(620.0))
        .h(px(400.0))
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "Content behind the drawer",
        ))
        .child(filters)
        .into_any_element()
}

/// The order the reorder scene is currently showing.
///
/// A still frame cannot show a slide, so the capture is the settled list and
/// the button is what a reviewer presses to watch a row travel.
#[derive(Debug)]
struct SceneQueue {
    steps: Vec<(&'static str, &'static str)>,
}

impl Global for SceneQueue {}

fn motion_flip(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneQueue>() {
        cx.set_global(SceneQueue {
            steps: vec![
                ("render", "Render frames"),
                ("upload", "Upload artifacts"),
                ("verify", "Verify checksums"),
                ("publish", "Publish release"),
            ],
        });
    }
    let steps = cx.global::<SceneQueue>().steps.clone();
    let theme = cx.theme().clone();

    let mut queue = Card::new().id("scene.motion.queue");
    for (index, (id, label)) in steps.iter().enumerate() {
        let ident = format!("scene.motion.{id}");
        let handle = flip(ident.clone(), cx);
        queue = queue.child(
            ListRow::new()
                .id(ident)
                .child(div().flex_1().child(*label))
                .child(Badge::new(format!("{}", index + 1)).neutral())
                .flip(&handle, window, cx),
        );
    }

    stack(&theme)
        .w(px(420.0))
        .child(
            row(&theme).child(
                Button::new("scene.motion.reorder")
                    .label("Move the last step first")
                    .secondary()
                    .on_click(|_, cx| {
                        cx.update_global::<SceneQueue, ()>(|queue, _| queue.steps.rotate_right(1));
                        cx.refresh_windows();
                    }),
            ),
        )
        .child(queue)
        .child(
            crate::foundation::text(
                &theme,
                TypeScale::Body,
                "Rows land in their new slot at once and slide into it.",
            )
            .text_tone(&theme, TextTone::Muted),
        )
        .into_any_element()
}

/// Which way every state in the state-transition scene is currently pointing.
///
/// One flag drives all of them, so a reviewer flips the whole row at once and
/// watches a check draw, a knob slide, an indicator travel and a section open
/// on the same frame.
#[derive(Debug)]
struct SceneStates {
    forward: bool,
}

impl Global for SceneStates {}

fn motion_state(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneStates>() {
        cx.set_global(SceneStates { forward: false });
    }
    let forward = cx.global::<SceneStates>().forward;
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(460.0))
        .child(
            row(&theme).child(
                Button::new("scene.state.flip")
                    .label("Flip every state")
                    .secondary()
                    .on_click(|_, cx| {
                        cx.update_global::<SceneStates, ()>(|state, _| {
                            state.forward = !state.forward
                        });
                        cx.refresh_windows();
                    }),
            ),
        )
        .child({
            let terms = Checkbox::new("scene.state.terms").label("Accept the terms");
            if forward {
                terms.checked(true)
            } else {
                terms.mixed()
            }
            .on_change(|_, _, _| {})
        })
        .child(
            Radio::new("scene.state.plan")
                .label("Bill monthly")
                .selected(forward)
                .on_select(|_, _| {}),
        )
        .child(
            Switch::new("scene.state.notify")
                .label("Send run notifications")
                .on(forward)
                .on_change(|_, _, _| {}),
        )
        .child(
            SegmentedControl::new("scene.state.view")
                .segments(vec![
                    Segment::new("list", "List"),
                    Segment::new("grid", "Grid"),
                ])
                .selected(if forward { "grid" } else { "list" })
                .on_select(|_, _, _| {}),
        )
        .child(
            Tabs::new("scene.state.tabs")
                .tabs(vec![
                    TabItem::new("overview", "Overview"),
                    TabItem::new("runs", "Runs"),
                ])
                .selected(if forward { "runs" } else { "overview" })
                .on_select(|_, _, _| {}),
        )
        .child(
            ProgressBar::new("scene.state.progress")
                .label("Uploading artifacts")
                .fraction(if forward { 0.86 } else { 0.12 }),
        )
        .child(
            Accordion::new("scene.state.sections")
                .expanded_ids(if forward { &["retention"][..] } else { &[][..] })
                .on_toggle(|_, _, _, _| {})
                .section(
                    AccordionSection::new("retention", "Retention")
                        .description("How long verified results are kept")
                        .body(crate::foundation::text(&theme, TypeScale::Body, "Results are kept in the workspace for 30 days.")),
                ),
        )
        .child(crate::foundation::text(&theme, TypeScale::Body, "Every state settles within a fifth of a second; the values are published the moment they change.").text_tone(&theme, TextTone::Muted))
        .into_any_element()
}

/// The counts the animated readout scene is currently showing.
#[derive(Debug)]
struct SceneCounts {
    runs: f64,
    seconds: f64,
}

impl Global for SceneCounts {}

fn animated_number(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneCounts>() {
        cx.set_global(SceneCounts {
            runs: 1204.0,
            seconds: 18.4,
        });
    }
    let counts = cx.global::<SceneCounts>();
    let (runs, seconds) = (counts.runs, counts.seconds);
    let theme = cx.theme().clone();

    let readout = |label: &'static str, number: AnimatedNumber| {
        div()
            .column()
            .gap(px(theme.spacing.xs))
            .child(
                crate::foundation::text(&theme, TypeScale::Caption, label)
                    .text_tone(&theme, TextTone::Muted),
            )
            .child(number)
    };

    stack(&theme)
        .child(
            row(&theme)
                .gap(px(theme.spacing.xl))
                .items_start()
                .child(readout(
                    "Runs this week",
                    AnimatedNumber::new("scene.number.runs", runs).format(grouped),
                ))
                .child(readout(
                    "Median duration",
                    AnimatedNumber::new("scene.number.seconds", seconds)
                        .format(|value| format!("{value:.1}s")),
                )),
        )
        .child(
            row(&theme).child(
                Button::new("scene.number.recount")
                    .label("Recount")
                    .secondary()
                    .on_click(|_, cx| {
                        cx.update_global::<SceneCounts, ()>(|counts, _| {
                            counts.runs += 318.0;
                            counts.seconds += 4.7;
                        });
                        cx.refresh_windows();
                    }),
            ),
        )
        .child(
            crate::foundation::text(
                &theme,
                TypeScale::Body,
                "The published value is the target, from the frame it changes.",
            )
            .text_tone(&theme, TextTone::Muted),
        )
        .into_any_element()
}

/// A caption naming the state a drag scene was staged in.
fn caption(theme: &Theme, text: impl Into<SharedString>) -> gpui::Div {
    crate::foundation::text(theme, TypeScale::Caption, text.into())
        .text_tone(theme, TextTone::Muted)
}

fn drag_list(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let carried = fixture_record(4);
    let anchor = fixture_record(1);
    // A capture cannot photograph a gesture, so the drag is placed by hand.
    // Staging fixes the ghost, the indicator, and the open slot, and takes the
    // pointer and the spring out of the picture.
    dnd::stage(
        StagedDrag::new(DragItem::new(
            "scene.drag.records",
            carried.0.clone(),
            carried.1.clone(),
        ))
        .landing(
            "scene.drag.records",
            DropPosition::Before(anchor.0.clone()),
            Some(1),
            true,
        ),
        cx,
    );

    stack(&theme)
        .w(px(420.0))
        .child(caption(
            &theme,
            SharedString::from(format!("{} moving before {}", carried.1, anchor.1)),
        ))
        .child(
            div()
                .relative()
                .hairline(&theme)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    List::new("scene.drag.records", 6, |index, _, _| {
                        let (id, label) = fixture_record(index);
                        ListItem::new(id, label.clone()).text(label)
                    })
                    // A row slides without its layout slot moving, so the
                    // viewport is one row taller than the rows it holds and
                    // the open slot has somewhere to be.
                    .visible_rows(7)
                    .reorderable(true)
                    .on_select(|_, _, _| {})
                    .on_reorder(|_, _, _| {}),
                )
                .children(
                    dnd::staged_ghost(cx)
                        .map(|ghost| div().absolute().left(px(96.0)).top(px(18.0)).child(ghost)),
                ),
        )
        .into_any_element()
}

fn drag_tree(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    dnd::stage(
        StagedDrag::new(
            DragItem::new("scene.drag.workspace", "kit", "gpui-kit").icon(Icon::Document),
        )
        .landing(
            "scene.drag.workspace",
            DropPosition::Into(SharedString::new_static("docs")),
            None,
            true,
        ),
        cx,
    );

    stack(&theme)
        .w(px(360.0))
        .child(caption(&theme, "gpui-kit moving into docs"))
        .child(
            div()
                .relative()
                .child(
                    Tree::new("scene.drag.workspace")
                        .expanded_ids(&["workspace", "crates", "docs"])
                        .nodes([TreeNode::new("workspace", "workspace")
                            .icon(Icon::Folder)
                            .children([
                                TreeNode::new("crates", "crates")
                                    .icon(Icon::Folder)
                                    .children([
                                        TreeNode::new("kit", "gpui-kit").icon(Icon::Document),
                                        TreeNode::new("tokens", "gpui-kit-tokens")
                                            .icon(Icon::Document),
                                    ]),
                                TreeNode::new("docs", "docs")
                                    .icon(Icon::Folder)
                                    .children([TreeNode::new("components", "components.md")
                                        .icon(Icon::Document)]),
                            ])])
                        .reorderable(true)
                        .on_toggle(|_, _, _, _| {})
                        .on_select(|_, _, _| {})
                        .on_move(|_, _, _| {}),
                )
                .children(
                    dnd::staged_ghost(cx)
                        .map(|ghost| div().absolute().left(px(212.0)).top(px(116.0)).child(ghost)),
                ),
        )
        .into_any_element()
}

fn dropzone(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    // No single pointer position can produce all three states at once, so each
    // zone is pinned to the one it is here to show.
    stack(&theme)
        .w(px(560.0))
        .child(caption(&theme, "idle, accepting, refusing"))
        .child(
            row(&theme)
                .items_stretch()
                .child(
                    div().flex_1().child(
                        Dropzone::new("scene.dropzone.idle", "Drop files to attach")
                            .hint("PDF, PNG, or plain text")
                            .state(DropzoneState::Idle)
                            .on_files(|_, _, _| {}),
                    ),
                )
                .child(
                    div().flex_1().child(
                        Dropzone::new("scene.dropzone.accepting", "Drop files to attach")
                            .hint("PDF, PNG, or plain text")
                            .state(DropzoneState::Accepting)
                            .on_files(|_, _, _| {}),
                    ),
                )
                .child(
                    div().flex_1().child(
                        Dropzone::new("scene.dropzone.refusing", "Drop files to attach")
                            .refusal("A folder cannot be attached.")
                            .state(DropzoneState::Refusing)
                            .on_files(|_, _, _| {}),
                    ),
                ),
        )
        .into_any_element()
}

fn wizard(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(720.0))
        .child(caption(
            &theme,
            "horizontal, with a blocked step and a failed one",
        ))
        .child(
            Wizard::new("scene.wizard.release")
                .steps([
                    WizardStep::new("prepare", "Prepare")
                        .description("Check the workspace is clean")
                        .complete(),
                    WizardStep::new("build", "Build")
                        .description("Compile every target")
                        .failed("The build failed on the test target."),
                    WizardStep::new("sign", "Sign").current(),
                    WizardStep::new("publish", "Publish")
                        .blocked("Approval is required for this workspace."),
                ])
                .body(
                    div()
                        .p(px(theme.spacing.md))
                        .radius(&theme, Radius::Card)
                        .hairline(&theme)
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Body,
                            SharedString::new_static(
                                "The body of the current step belongs to the caller.",
                            ),
                        )),
                )
                .back_to("build")
                .on_navigate(|_, _, _| {}),
        )
        .child(caption(&theme, "vertical, finishing"))
        .child(
            Wizard::new("scene.wizard.setup")
                .vertical()
                .steps([
                    WizardStep::new("account", "Account").complete(),
                    WizardStep::new("workspace", "Workspace").complete(),
                    WizardStep::new("review", "Review").current(),
                ])
                .back_to("workspace")
                .finish(true)
                .on_navigate(|_, _, _| {}),
        )
        .into_any_element()
}

fn undo_history(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(caption(
            &theme,
            "the caller owns every revision and whether it can be restored",
        ))
        .child(
            UndoHistory::new("scene.undo-history", "Document undo history")
                .entries([
                    HistoryEntry::new("opened", "Opened the fixture")
                        .description("The initial verified document")
                        .source("Fixture host")
                        .time("10:14"),
                    HistoryEntry::new("renamed", "Renamed the title")
                        .source("Alex")
                        .time("10:16"),
                    HistoryEntry::new("imported", "Imported archived blocks")
                        .description("The archive is no longer available")
                        .source("Archive")
                        .time("10:18")
                        .unavailable("This revision cannot be restored without the archive."),
                    HistoryEntry::new("current", "Reordered the summary")
                        .description("Current document")
                        .source("Alex")
                        .time("10:21"),
                    HistoryEntry::new("draft", "Drafted a new conclusion")
                        .source("Fixture host")
                        .time("10:23"),
                ])
                .current("current")
                .on_jump(|_, _, _| {}),
        )
        .into_any_element()
}

fn settings(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(620.0))
        .child(
            SettingsSection::new("scene.settings.general", "General")
                .description("How this workspace behaves")
                .row(
                    SettingsRow::new("scene.settings.general.autosave", "Save automatically")
                        .description("Write changes as they happen")
                        .control(
                            Switch::new("scene.settings.general.autosave.switch")
                                .named("Save automatically")
                                .on(true)
                                .on_change(|_, _, _| {}),
                        ),
                )
                .row(
                    SettingsRow::new("scene.settings.general.runtime", "Native runtime")
                        .description("Runs work on this machine instead of a host")
                        .badge("Requires restart")
                        .control(
                            Switch::new("scene.settings.general.runtime.switch")
                                .named("Native runtime")
                                .on(false)
                                .on_change(|_, _, _| {}),
                        ),
                )
                .row(
                    SettingsRow::new("scene.settings.general.telemetry", "Usage reporting")
                        .description("Nobody on this machine can change this")
                        .value("Off")
                        .managed("your administrator"),
                ),
        )
        .child(
            SettingsSection::new("scene.settings.sync", "Synchronisation")
                .description("What travels between machines")
                .dimmed_by("This workspace is local, so nothing synchronises.")
                .row(
                    SettingsRow::new("scene.settings.sync.settings", "Sync settings")
                        .value("Off")
                        .control(
                            Switch::new("scene.settings.sync.settings.switch")
                                .named("Sync settings")
                                .on(false)
                                .on_change(|_, _, _| {}),
                        ),
                )
                .row(
                    SettingsRow::new("scene.settings.sync.history", "Sync history")
                        .value("Off")
                        .control(
                            Switch::new("scene.settings.sync.history.switch")
                                .named("Sync history")
                                .on(false)
                                .on_change(|_, _, _| {}),
                        ),
                ),
        )
        .into_any_element()
}

fn detail(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(640.0))
        .child(caption(
            &theme,
            "unknown, not applicable, and redacted are three facts",
        ))
        .child(
            DescriptionList::new("scene.detail.facts")
                .columns(2)
                .items([
                    DescriptionItem::new("id", "Run", "run-4821"),
                    DescriptionItem::new("owner", "Owner", "fixture-owner"),
                    DescriptionItem::new("finished", "Finished", DescriptionValue::Unknown),
                    DescriptionItem::new("artifact", "Artifact", DescriptionValue::NotApplicable),
                    DescriptionItem::new(
                        "token",
                        "Access token",
                        DescriptionValue::redacted("51 characters"),
                    )
                    .copyable(true),
                ])
                .on_copy(|_, _, _| {}),
        )
        .child(caption(
            &theme,
            "what happened, in the words the host chose",
        ))
        .child(
            Timeline::new("scene.detail.activity")
                .group(
                    TimelineGroup::new("today", "Today")
                        .entry(
                            TimelineEntry::new("queued", "Run queued")
                                .time("09:12")
                                .actor("fixture-owner")
                                .tone(Tone::Neutral),
                        )
                        .entry(
                            TimelineEntry::new("started", "Indexing started")
                                .time("09:13")
                                .actor("scheduler")
                                .tone(Tone::Info),
                        )
                        .entry(
                            TimelineEntry::new("failed", "Indexing failed")
                                .time("09:41")
                                .actor("scheduler")
                                .tone(Tone::Danger)
                                .detail(crate::foundation::text(
                                    &theme,
                                    TypeScale::Body,
                                    SharedString::new_static(
                                        "The host refused the request. The refusal is shown as it \
                                     arrived.",
                                    ),
                                ).text_tone(&theme, TextTone::Muted)),
                        ),
                )
                .group(
                    TimelineGroup::new("earlier", "Earlier").entry(
                        TimelineEntry::new("imported", "Workspace imported")
                            .time_unknown()
                            .actor("fixture-owner")
                            .tone(Tone::Neutral),
                    ),
                ),
        )
        .into_any_element()
}

fn filter_bar(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(720.0))
        .child(
            FilterBar::new("scene.filter-bar.runs")
                .conditions([
                    FilterCondition::new("status", "Status", "is", "failed"),
                    FilterCondition::new("owner", "Owner", "is", "fixture-owner"),
                    FilterCondition::new("started", "Started", "after", "09:00"),
                ])
                .count(ResultCount::Known(14))
                .noun("runs")
                .on_add(|_, _| {})
                .on_remove(|_, _, _| {})
                .on_clear(|_, _| {}),
        )
        .child(caption(&theme, "counting is not zero"))
        .child(
            FilterBar::new("scene.filter-bar.counting")
                .conditions([FilterCondition::new("status", "Status", "is", "queued")])
                .count(ResultCount::Counting)
                .on_add(|_, _| {})
                .on_remove(|_, _, _| {})
                .on_clear(|_, _| {}),
        )
        .into_any_element()
}

fn inline_edit(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(420.0))
        .child(caption(
            &theme,
            "reading, editing, and a save that did not take",
        ))
        .child(
            InlineEdit::new("scene.inline-edit.title", "Indexing the workspace")
                .on_edit(|_, _| {})
                .on_commit(|_, _, _| {})
                .on_cancel(|_, _| {}),
        )
        .child(
            InlineEdit::new("scene.inline-edit.owner", "fixture-owner")
                .editing(true)
                .on_edit(|_, _| {})
                .on_commit(|_, _, _| {})
                .on_cancel(|_, _| {}),
        )
        .child(
            InlineEdit::new(
                "scene.inline-edit.note",
                "Retry after the host is reachable",
            )
            .editing(true)
            .failure("The host refused this change. What you typed is still here.")
            .on_edit(|_, _| {})
            .on_commit(|_, _, _| {})
            .on_cancel(|_, _| {}),
        )
        .child(
            InlineEdit::new("scene.inline-edit.policy", "Set by the administrator")
                .disabled(true)
                .on_edit(|_, _| {}),
        )
        .into_any_element()
}

fn progress_circle(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(caption(
            &theme,
            "a position exists only when the extent is known",
        ))
        .child(
            row(&theme)
                .gap(px(theme.spacing.lg))
                .child(
                    ProgressCircle::new("scene.progress-circle.upload")
                        .count(3, 12)
                        .label("Uploading artifacts")
                        .centre("25%"),
                )
                .child(
                    ProgressCircle::new("scene.progress-circle.verify")
                        .fraction(0.72)
                        .label("Verifying checksums")
                        .display("72%")
                        .centre("72%"),
                )
                .child(
                    ProgressCircle::new("scene.progress-circle.contact")
                        .label("Contacting the host"),
                ),
        )
        .child(caption(&theme, "the size ramp"))
        .child(
            row(&theme)
                .gap(px(theme.spacing.lg))
                .child(
                    ProgressCircle::new("scene.progress-circle.xs")
                        .fraction(0.4)
                        .label("Extra small")
                        .xs(),
                )
                .child(
                    ProgressCircle::new("scene.progress-circle.sm")
                        .fraction(0.4)
                        .label("Small")
                        .small(),
                )
                .child(
                    ProgressCircle::new("scene.progress-circle.md")
                        .fraction(0.4)
                        .label("Medium")
                        .medium(),
                )
                .child(
                    ProgressCircle::new("scene.progress-circle.lg")
                        .fraction(0.4)
                        .label("Large")
                        .large(),
                ),
        )
        .into_any_element()
}

/// A layout nested three deep, with one leaf collapsed to its rail. The tree
/// is the caller's: every divider reports the ratio it was asked for and moves
/// nothing here.
fn split_tree(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let layout = SplitLayout::horizontal(
        "workspace",
        0.26,
        SplitLayout::leaf(SplitPaneSpec::new("files").min_width(140.0)),
        SplitLayout::horizontal(
            "body",
            0.74,
            SplitLayout::vertical(
                "editing",
                0.6,
                SplitLayout::leaf(SplitPaneSpec::new("editor").min_height(90.0)),
                SplitLayout::leaf(SplitPaneSpec::new("terminal").min_height(70.0)),
            ),
            // A collapsed leaf is drawn at its rail, and the divider beside it
            // is not offered: a fixed extent has no ratio to trade.
            SplitLayout::leaf(SplitPaneSpec::new("outline").rail(40.0).collapsed(true)),
        ),
    );

    stack(&theme)
        .w(px(680.0))
        .child(
            div()
                .h(px(340.0))
                .hairline(&theme)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    SplitTree::new("scene.tree.workspace")
                        .layout(layout)
                        .pane("files", filler(&theme, "Files", 6))
                        .pane("editor", filler(&theme, "main.rs", 6))
                        .pane("terminal", filler(&theme, "Terminal", 2))
                        // The rail is narrower than any label, so the collapsed
                        // leaf is drawn as the room it still holds.
                        .pane("outline", div())
                        .on_change(|_, _, _| {}),
                ),
        )
        .child(caption(
            &theme,
            "A divider high in the tree stops where a leaf far below it would \
             run out of room.",
        ))
        .into_any_element()
}

/// A whole application frame: panels in regions, one region collapsed to a
/// rail, one panel the host refuses, and a status bar under all of it.
fn ide_shell(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let mut branch = AsyncValue::<SharedString, String>::ready("main@a1b2c3".into());
    branch.refresh();
    branch.fail_refresh("the host is unreachable".into());

    div()
        .column()
        .w(px(900.0))
        .h(px(560.0))
        .bg(theme.colors.canvas)
        .text_color(theme.colors.text)
        .font_family(theme.typography.sans.clone())
        .child(
            div().flex_1().min_h(px(0.0)).child(
                Dock::new("scene.shell")
                    .share(DockRegion::Left, 0.24)
                    .share(DockRegion::Bottom, 0.3)
                    .panel(
                        DockRegion::Left,
                        DockPanel::new("files", "Files")
                            .icon(Icon::Folder)
                            .content(filler(&theme, "Workspace", 8)),
                    )
                    .panel(
                        DockRegion::Left,
                        DockPanel::new("search", "Search")
                            .icon(Icon::Magnifier)
                            .badge("12"),
                    )
                    .active(DockRegion::Left, "files")
                    .panel(
                        DockRegion::Centre,
                        DockPanel::new("editor", "main.rs")
                            .icon(Icon::Document)
                            .content(filler(&theme, "fn main()", 12)),
                    )
                    .panel(
                        DockRegion::Right,
                        DockPanel::new("outline", "Outline").icon(Icon::List),
                    )
                    .panel(
                        DockRegion::Right,
                        DockPanel::new("history", "History").icon(Icon::GitBranch),
                    )
                    .collapsed(DockRegion::Right, true)
                    .panel(
                        DockRegion::Bottom,
                        DockPanel::new("terminal", "Terminal")
                            .icon(Icon::Terminal)
                            .content(filler(&theme, "$ cargo test", 4)),
                    )
                    .panel(
                        DockRegion::Bottom,
                        DockPanel::new("problems", "Problems")
                            .icon(Icon::Danger)
                            .badge("3")
                            .unavailable(
                                "The language server is not running, so problems cannot be \
                                 listed. Nothing here is out of date; there is nothing here.",
                            ),
                    )
                    // The refused panel is the one on top, because a refusal
                    // nobody can see is a refusal nobody was told about.
                    .active(DockRegion::Bottom, "problems")
                    .on_event(|_, _, _| {}),
            ),
        )
        .child(
            StatusBar::new("scene.shell.status")
                .label("Workspace status")
                .start([
                    StatusItem::text("branch", "main")
                        .icon(Icon::GitBranch)
                        .tracking(&branch),
                    StatusItem::state("build", "Build passing", Tone::Success),
                ])
                .centre([StatusItem::progress("index", "Indexing the workspace")
                    .count(7, 12)
                    .state_name("loading")])
                .end([
                    StatusItem::text("position", "Ln 42, Col 7"),
                    StatusItem::action("encoding", "UTF-8").on_click(|_, _| {}),
                ]),
        )
        .into_any_element()
}

#[derive(Clone)]
struct SceneRecorders {
    idle: Entity<KeybindingRecorder>,
    recording: Entity<KeybindingRecorder>,
    captured: Entity<KeybindingRecorder>,
    conflicting: Entity<KeybindingRecorder>,
}

impl Global for SceneRecorders {}

fn keybinding(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneRecorders>() {
        let idle = cx.new(|cx| {
            KeybindingRecorder::new("scene.keybinding.idle", window, cx).label("Open workspace")
        });
        let recording = cx.new(|cx| {
            KeybindingRecorder::new("scene.keybinding.recording", window, cx)
                .label("Command palette")
        });
        let captured = cx.new(|cx| {
            KeybindingRecorder::new("scene.keybinding.captured", window, cx)
                .label("Toggle terminal")
                .binding("ctrl-`")
        });
        let conflicting = cx.new(|cx| {
            KeybindingRecorder::new("scene.keybinding.conflicting", window, cx)
                .label("Split editor")
                .binding("cmd-shift-p")
                // The host's words, not the recorder's: it has no keymap.
                .conflict(Some("Already opens the command palette"))
        });
        // Recording is a state, not a gesture, so the scene puts one recorder
        // into it by hand rather than waiting for a keystroke that a still
        // image could not photograph anyway.
        recording.update(cx, |recorder, cx| recorder.start(window, cx));
        cx.set_global(SceneRecorders {
            idle,
            recording,
            captured,
            conflicting,
        });
    }
    let recorders = cx.global::<SceneRecorders>().clone();
    let theme = cx.theme().clone();

    // A recorder carries its name in the tree rather than drawing one, the way
    // every other control here does, so the scene puts it where a keymap page
    // would: in a settings row that states what the binding is for.
    stack(&theme)
        .w(px(680.0))
        .child(
            SettingsSection::new("scene.keybinding.keymap", "Keyboard shortcuts")
                .description("Recording captures the next keystroke instead of acting on it.")
                .row(
                    SettingsRow::new("scene.keybinding.row.open", "Open workspace")
                        .description("Nothing is bound yet")
                        .control(recorders.idle),
                )
                .row(
                    SettingsRow::new("scene.keybinding.row.palette", "Command palette")
                        .description("Listening for a keystroke")
                        .control(recorders.recording),
                )
                .row(
                    SettingsRow::new("scene.keybinding.row.terminal", "Toggle terminal")
                        .control(recorders.captured),
                )
                .row(
                    SettingsRow::new("scene.keybinding.row.split", "Split editor")
                        .description("The host judged this one, and said so")
                        .control(recorders.conflicting),
                ),
        )
        .child(caption(
            &theme,
            "Escape ends recording without capturing, so escape cannot be bound \
             unless the caller turns allow_escape on.",
        ))
        .into_any_element()
}

#[derive(Clone)]
struct SceneKeymapEditor(Entity<KeymapEditor>);

impl Global for SceneKeymapEditor {}

fn keymap_editor(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneKeymapEditor>() {
        let editor = cx.new(|cx| {
            KeymapEditor::new("scene.keymap-editor", window, cx).commands([
                KeymapCommand::new("workspace.open", "Open workspace")
                    .context("Workspace")
                    .defaults(["cmd-o"])
                    .bindings([
                        KeymapBinding::new("user", "cmd-shift-o")
                            .conflict("Already opens recent workspaces")
                            .provenance("User keymap"),
                        KeymapBinding::new("workspace", "ctrl-o").provenance("Workspace keymap"),
                    ])
                    .searchable("Open a folder or project", ["folder", "project"]),
                KeymapCommand::new("terminal.toggle", "Toggle terminal")
                    .context("Terminal")
                    .defaults(["ctrl-`"])
                    .bindings([KeymapBinding::new("default", "ctrl-`")])
                    .searchable("Show the integrated terminal", ["panel", "console"]),
                KeymapCommand::new("policy.locked", "Managed shortcut")
                    .context("Workspace")
                    .defaults(["cmd-l"])
                    .bindings([KeymapBinding::new("managed", "cmd-l").provenance("Host policy")])
                    .refused("This binding is managed by the host."),
            ])
        });
        cx.set_global(SceneKeymapEditor(editor));
    }
    let editor = cx.global::<SceneKeymapEditor>().0.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(760.0))
        .child(editor)
        .child(caption(
            &theme,
            "Bindings remain caller-owned; add, remove, and reset are intents.",
        ))
        .into_any_element()
}

#[cfg(feature = "fixtures")]
mod dates {
    use std::rc::Rc;

    use gpui::{AnyElement, App, Entity, Global, IntoElement, Window, div, prelude::*, px};
    use gpui_kit_theme::{Space, TextTone, TypeScale};

    use crate::datetime::fixture::FixtureDateAdapter;
    use crate::datetime::{
        Calendar, DateInput, DayMark, DayRange, RangePicker, SharedDateAdapter, TimeInput,
        TimeOfDay,
    };
    use crate::display::badge::Tone;
    use crate::foundation::{ActiveTheme, StyledExt};

    use super::{row, stack};

    /// The pinned calendar every date scene runs on, so two captures of the
    /// same scene are the same picture.
    fn adapter() -> SharedDateAdapter {
        Rc::new(
            FixtureDateAdapter::pinned(2024, 3, 14)
                .blocking(2024, 3, 8, "The workspace is frozen for the release.")
                .blocking(2024, 3, 20, "Nobody is on call that day."),
        )
    }

    fn marks(day: crate::datetime::Day) -> Option<DayMark> {
        match day.0.rem_euclid(7) {
            0 => Some(DayMark::new("Two runs finished here").tone(Tone::Success)),
            3 => Some(DayMark::new("One run failed here").tone(Tone::Danger)),
            _ => None,
        }
    }

    struct SceneDates {
        month: Entity<Calendar>,
        unknown: Entity<Calendar>,
        incomplete: Entity<RangePicker>,
        preview: Entity<RangePicker>,
        blocked: Entity<RangePicker>,
        field: Entity<DateInput>,
        refused: Entity<DateInput>,
        clock: Entity<TimeInput>,
        twelve: Entity<TimeInput>,
    }

    impl Global for SceneDates {}

    fn ensure(window: &mut Window, cx: &mut App) {
        if cx.has_global::<SceneDates>() {
            return;
        }
        let pinned = adapter();
        let unknown_adapter: SharedDateAdapter = Rc::new(FixtureDateAdapter::without_today());
        let march = FixtureDateAdapter::pinned(2024, 3, 14);

        let month = cx.new(|cx| {
            Calendar::new("scene.calendar", pinned.clone(), window, cx)
                .selected([march.day(2024, 3, 14)])
                .overlay(marks)
        });
        let unknown = cx.new(|cx| {
            Calendar::new(
                "scene.calendar.unknown",
                unknown_adapter.clone(),
                window,
                cx,
            )
        });
        let incomplete = cx.new(|cx| {
            RangePicker::new("scene.range.incomplete", pinned.clone(), window, cx)
                .range(DayRange::starting(march.day(2024, 3, 11)))
        });
        let preview = cx.new(|cx| {
            RangePicker::new("scene.range.preview", pinned.clone(), window, cx)
                .range(DayRange::starting(march.day(2024, 3, 11)))
        });
        let blocked = cx.new(|cx| {
            RangePicker::new("scene.range.blocked", pinned.clone(), window, cx)
                .range(DayRange::new(march.day(2024, 3, 6), march.day(2024, 3, 9)))
        });
        let field = cx.new(|cx| {
            DateInput::new("scene.date.field", pinned.clone(), window, cx)
                .value(march.day(2024, 3, 14))
        });
        let refused = cx.new(|cx| DateInput::new("scene.date.refused", pinned.clone(), window, cx));
        let clock = cx.new(|cx| {
            TimeInput::new("scene.time.clock", pinned.clone(), window, cx)
                .value(TimeOfDay::new(9, 30).with_second(0))
                .seconds(true)
        });
        let twelve_adapter: SharedDateAdapter =
            Rc::new(FixtureDateAdapter::pinned(2024, 3, 14).twelve_hour(true));
        let twelve = cx.new(|cx| {
            TimeInput::new("scene.time.twelve", twelve_adapter, window, cx)
                .value(TimeOfDay::new(9, 30).with_meridiem(1))
        });

        let hovered = march.day(2024, 3, 15);
        preview.update(cx, |picker, cx| {
            picker.calendar().update(cx, |calendar, cx| {
                calendar.set_hovered_day(Some(hovered), cx);
            });
        });
        let refused_field = refused.read(cx).field().clone();
        refused_field.update(cx, |input, cx| input.set_value("the fifth", cx));

        cx.set_global(SceneDates {
            month,
            unknown,
            incomplete,
            preview,
            blocked,
            field,
            refused,
            clock,
            twelve,
        });
    }

    pub(super) fn calendar(window: &mut Window, cx: &mut App) -> AnyElement {
        ensure(window, cx);
        let theme = cx.theme().clone();
        let dates = cx.global::<SceneDates>();
        let (month, unknown) = (dates.month.clone(), dates.unknown.clone());
        stack(&theme)
            .child(
                row(&theme)
                    .items_start()
                    .gap(px(theme.space(Space::Lg)))
                    .child(month)
                    .child(unknown),
            )
            .child(
                crate::foundation::text(
                    &theme,
                    TypeScale::Body,
                    "Every weekday name, month name, and blocked reason above came from the \
                         host. The calendar on the right has no today, so it draws no ring and \
                         guesses no month.",
                )
                .max_w(px(560.0))
                .text_tone(&theme, TextTone::Muted),
            )
            .into_any_element()
    }

    pub(super) fn date_range(window: &mut Window, cx: &mut App) -> AnyElement {
        ensure(window, cx);
        let theme = cx.theme().clone();
        let dates = cx.global::<SceneDates>();
        let (incomplete, preview, blocked) = (
            dates.incomplete.clone(),
            dates.preview.clone(),
            dates.blocked.clone(),
        );
        stack(&theme)
            .child(
                row(&theme)
                    .items_start()
                    .gap(px(theme.space(Space::Lg)))
                    .child(incomplete)
                    .child(preview)
                    .child(blocked),
            )
            .into_any_element()
    }

    pub(super) fn date_time(window: &mut Window, cx: &mut App) -> AnyElement {
        ensure(window, cx);
        let theme = cx.theme().clone();
        let dates = cx.global::<SceneDates>();
        let (field, refused, clock, twelve) = (
            dates.field.clone(),
            dates.refused.clone(),
            dates.clock.clone(),
            dates.twelve.clone(),
        );
        stack(&theme)
            .child(div().w(px(280.0)).child(field))
            .child(div().w(px(280.0)).child(refused))
            .child(
                row(&theme)
                    .gap(px(theme.space(Space::Lg)))
                    .child(clock)
                    .child(twelve),
            )
            .child(
                crate::foundation::text(
                    &theme,
                    TypeScale::Body,
                    "What the field could not read is still in it, and the refusal is the \
                         adapter's own sentence.",
                )
                .max_w(px(560.0))
                .text_tone(&theme, TextTone::Muted),
            )
            .into_any_element()
    }
}

#[cfg(feature = "fixtures")]
use dates::{calendar, date_range, date_time};

/// A document with one of every block kind in it, including a tag nobody ran.
const SCENE_DOCUMENT: &str = r#"# Release notes

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

fn markdown(_window: &mut Window, cx: &mut App) -> AnyElement {
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

/// The thread the conversation scene shows, one message per delivery state.
fn scene_thread() -> Vec<Message> {
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

fn conversation(_window: &mut Window, cx: &mut App) -> AnyElement {
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

/// The element a host hands back for an image this crate did not fetch.
///
/// A flat fill rather than a picture, because the crate ships no photographs
/// and a scene has to photograph the same pixels every run.
fn scene_picture(label: &'static str, cx: &App) -> AnyElement {
    let theme = cx.theme().clone();
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.colors.accent.opacity(0.35))
        .border(px(theme.borders.thick))
        .border_color(theme.colors.accent)
        .child(crate::foundation::text(
            &theme,
            TypeScale::Label,
            SharedString::new_static(label),
        ))
        .into_any_element()
}

fn image_viewer(_window: &mut Window, cx: &mut App) -> AnyElement {
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

fn transport(_window: &mut Window, cx: &mut App) -> AnyElement {
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

/// The cube the model scenes read: a glTF document with its own buffer
/// inside it, so the reader is exercised on the path a host would take and
/// nothing is fetched to draw a scene.
const CUBE: &[u8] = include_bytes!("../assets/models/cube.gltf");

/// A measured envelope, in the shape a host that read the samples would hand
/// over. It is written down rather than sampled, because a scene that drew a
/// different waveform every run could not be a visual baseline.
fn scene_peaks(count: usize) -> Vec<f32> {
    (0..count)
        .map(|index| {
            let phase = index as f32 / count as f32;
            let swell = (phase * 11.0).sin() * 0.32 + (phase * 3.0).cos() * 0.28;
            (0.42 + swell).clamp(0.05, 1.0)
        })
        .collect()
}

fn audio_player(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(640.0))
        .child(
            AudioPlayer::new("scene.audio.ready")
                .title("Release walkthrough")
                .subtitle("Recorded 12 March")
                .transport(
                    FixtureTransport::ready(240.0)
                        .position(72.0)
                        .state(TransportState::Playing)
                        .volume(0.7)
                        .buffered([BufferedRange::new(0.0, 156.0)])
                        .shared(),
                )
                .elapsed("01:12")
                .remaining("-02:48")
                .peaks(scene_peaks(96))
                .speeds([1.0, 1.5, 2.0])
                .step_seconds(10.0)
                .on_event(|_, _, _| {}),
        )
        .child(
            Divider::new()
                .id("scene.audio.rule.backend")
                .label("Nothing here can decode it"),
        )
        .child(
            AudioPlayer::new("scene.audio.absent")
                .title("interview.opus")
                .transport(
                    FixtureTransport::new()
                        .no_backend("There is no Opus decoder on this machine.")
                        .shared(),
                )
                .on_event(|_, _, _| {}),
        )
        .child(
            Divider::new()
                .id("scene.audio.rule.none")
                .label("No player connected at all"),
        )
        .child(
            AudioPlayer::new("scene.audio.none")
                .title("Standup recording")
                .on_event(|_, _, _| {}),
        )
        .into_any_element()
}

fn video_player(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(440.0))
        .child(
            VideoPlayer::new("scene.video.frame")
                .title("Screen capture")
                .transport(
                    FixtureTransport::ready(96.0)
                        .position(24.0)
                        .state(TransportState::Playing)
                        .volume(0.6)
                        .shared(),
                )
                .frame(|_, cx| Some(scene_picture("Supplied frame", cx)))
                .elapsed("00:24")
                .remaining("-01:12")
                .on_event(|_, _, _| {}),
        )
        .child(
            Divider::new()
                .id("scene.video.rule.poster")
                .label("Ready, and no frames supplied"),
        )
        .child(
            VideoPlayer::new("scene.video.poster")
                .title("Release walkthrough")
                .transport(FixtureTransport::ready(240.0).shared())
                .poster(|_, cx| Some(scene_picture("Poster", cx)))
                .elapsed("00:00")
                .remaining("-04:00")
                .on_event(|_, _, _| {}),
        )
        .into_any_element()
}

fn model_viewer(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let read = || ModelState::read(CUBE, ModelBounds::default());
    stack(&theme)
        .w(px(720.0))
        .child(
            div()
                .row()
                .items_start()
                .gap(px(theme.spacing.lg))
                .child(
                    div().flex_1().min_w_0().child(
                        ModelViewer::new("scene.model.flat")
                            .title("cube.gltf")
                            .state(read())
                            .orbit(0.62, 0.42)
                            .height(240.0)
                            .on_event(|_, _, _| {}),
                    ),
                )
                .child(
                    div().flex_1().min_w_0().child(
                        ModelViewer::new("scene.model.wireframe")
                            .title("cube.gltf")
                            .state(read())
                            .shading(ModelShading::Wireframe)
                            .orbit(0.62, 0.42)
                            .height(240.0)
                            .on_event(|_, _, _| {}),
                    ),
                ),
        )
        .child(
            Divider::new()
                .id("scene.model.rule.refused")
                .label("A document past a bound"),
        )
        .child(
            ModelViewer::new("scene.model.refused")
                .title("city-block.glb")
                .rejected(ModelError::TooLarge {
                    limit: ModelLimit::Triangles,
                    found: 412_930,
                    allowed: 24_576,
                })
                .height(160.0)
                .on_event(|_, _, _| {}),
        )
        .into_any_element()
}

struct SceneApprovals {
    pending: Entity<ApprovalPrompt>,
    declined: Entity<ApprovalPrompt>,
    expired: Entity<ApprovalPrompt>,
    superseded: Entity<ApprovalPrompt>,
}

impl Global for SceneApprovals {}

fn approval(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneApprovals>() {
        let pending = cx.new(|cx| {
            ApprovalPrompt::new(
                "scene.approval.pending",
                "Write to /work/report/summary.md",
                window,
                cx,
            )
            .details([
                DescriptionItem::new("tool", "Tool", "write-file"),
                DescriptionItem::new("path", "Path", "/work/report/summary.md"),
                DescriptionItem::new("bytes", "Size", "4 KB"),
            ])
            .always(AlwaysScope::Session)
            .always(AlwaysScope::path("/work/report"))
            .always(AlwaysScope::tool("write-file"))
        });
        let declined = cx.new(|cx| {
            ApprovalPrompt::new(
                "scene.approval.declined",
                "Delete /work/report/draft.md",
                window,
                cx,
            )
            .status(ApprovalStatus::Declined)
        });
        let expired = cx.new(|cx| {
            ApprovalPrompt::new(
                "scene.approval.expired",
                "Open a connection to build.internal:8443",
                window,
                cx,
            )
            .status(ApprovalStatus::Expired)
        });
        let superseded = cx.new(|cx| {
            ApprovalPrompt::new(
                "scene.approval.superseded",
                "Run the test suite in /work",
                window,
                cx,
            )
            .status(ApprovalStatus::Superseded {
                by: "a later request covering the whole workspace".into(),
            })
        });
        cx.set_global(SceneApprovals {
            pending,
            declined,
            expired,
            superseded,
        });
    }
    let prompts = cx.global::<SceneApprovals>();
    let pending = prompts.pending.clone();
    let declined = prompts.declined.clone();
    let expired = prompts.expired.clone();
    let superseded = prompts.superseded.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(560.0))
        .child(pending)
        .child(
            Divider::new()
                .id("scene.approval.rule.resolved")
                .label("Answered, expired, and replaced are three different things"),
        )
        .child(declined)
        .child(expired)
        .child(superseded)
        .into_any_element()
}

fn permission_matrix(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let actions = [
        PermissionAction::new("read", "Read files"),
        PermissionAction::new("write", "Write files"),
        PermissionAction::new("network", "Reach the network"),
    ];
    let subjects = [
        PermissionSubject::new("workspace", "This workspace")
            .cell("read", PermissionEntry::new(PermissionState::Allowed))
            .cell("write", PermissionEntry::new(PermissionState::Ask))
            .cell(
                "network",
                PermissionEntry::inherited(PermissionState::Denied, "the organisation policy"),
            ),
        PermissionSubject::new("scratch", "The scratch directory")
            .cell(
                "read",
                PermissionEntry::inherited(PermissionState::Allowed, "this workspace"),
            )
            .cell("write", PermissionEntry::new(PermissionState::Allowed))
            .cell(
                "network",
                PermissionEntry::inherited(PermissionState::Denied, "the organisation policy"),
            ),
        // A calculator has no files to read, which is not the same as being
        // refused them.
        PermissionSubject::new("calculator", "The calculator tool")
            .cell("network", PermissionEntry::new(PermissionState::Denied)),
    ];

    stack(&theme)
        .w(px(720.0))
        .child(
            PermissionMatrix::new("scene.permission.editable")
                .actions(actions.clone())
                .subjects(subjects.clone())
                .on_change(|_change, _window, _cx| {}),
        )
        .child(
            Divider::new()
                .id("scene.permission.rule.read-only")
                .label("The same permissions, shown rather than offered"),
        )
        .child(
            PermissionMatrix::new("scene.permission.read-only")
                .actions(actions)
                .subjects(subjects),
        )
        .into_any_element()
}

fn cost_meter(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(
            CostMeter::new("scene.cost.meter")
                .label("This run")
                .line(CostLine::new(
                    "spend",
                    "Spend",
                    Reading::measured(1.24, "1.24 credits"),
                ))
                .line(CostLine::new(
                    "projected",
                    "Projected at this rate",
                    Reading::estimated(4.0, "4.00 credits"),
                ))
                .line(
                    CostLine::new(
                        "account",
                        "Account balance",
                        Reading::measured(112.0, "112.00 credits"),
                    )
                    .stale(LastVerified::at("09:41 today")),
                )
                .line(CostLine::new(
                    "storage",
                    "Storage",
                    Reading::unavailable_because("The billing host refused the request."),
                )),
        )
        .child(
            Divider::new()
                .id("scene.cost.rule.gauge")
                .label("A limit that is known, and one that is not"),
        )
        .child(
            ContextGauge::new(
                "scene.cost.context.known",
                Reading::measured(48_000.0, "48,000 tokens"),
            )
            .label("Context used")
            .limit(Limit::measured(128_000.0, "128,000 tokens")),
        )
        .child(
            ContextGauge::new(
                "scene.cost.context.unknown",
                Reading::estimated(48_000.0, "48,000 tokens"),
            )
            .label("Context used"),
        )
        .child(
            ContextGauge::new("scene.cost.context.unavailable", Reading::unavailable())
                .label("Context used")
                .limit(Limit::measured(128_000.0, "128,000 tokens")),
        )
        .into_any_element()
}

fn scene_arguments() -> ToolBody {
    ToolBody::new(
        "{\n  \"path\": \"docs/coverage.md\",\n  \"pattern\": \"unknown\",\n  \"limit\": 20\n}",
    )
    .max_lines(2)
}

/// Every state a call can be in, side by side.
///
/// Two columns because a captured image is one screen: stacked, the refusal
/// fell below the fold, and a state nobody can see in the snapshot is a state
/// the snapshot does not guard.
fn tool_call(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    div()
        .flex()
        .gap(px(theme.spacing.lg))
        .child(
            stack(&theme).w(px(460.0)).child(
            ToolCallCard::new("scene.tool.pending", "workspace.search")
                .arguments(scene_arguments())
                .state(ToolCallState::PendingApproval),
        )
        .child(
            ToolCallCard::new("scene.tool.running", "workspace.index")
                .arguments("{ \"root\": \"crates\" }")
                .state(ToolCallState::Running)
                .elapsed("4.2 s"),
        )
        .child(
            ToolCallCard::new("scene.tool.succeeded", "workspace.read")
                .arguments("{ \"path\": \"README.md\" }")
                .state(ToolCallState::succeeded(
                    ToolBody::new(
                        "# GPUI Box\n\nProduct-neutral components.\n\nEvery word is replaceable.",
                    )
                    .max_lines(2),
                ))
                .elapsed("0.3 s"),
        )
        .child(
            ToolCallCard::new("scene.tool.silent", "workspace.touch")
                .arguments("{ \"path\": \"notes.md\" }")
                .state(ToolCallState::succeeded_silently())
                .elapsed("0.1 s"),
        ),
        )
        .child(
            stack(&theme)
                .w(px(460.0))
                .child(
                    ToolCallCard::new("scene.tool.failed", "workspace.write")
                        .arguments("{ \"path\": \"/read-only/notes.md\" }")
                        .state(ToolCallState::failed(
                            "The file system reported that the path is read only.",
                        ))
                        .elapsed("0.2 s")
                        .on_retry(|_, _| {}),
                )
                // A refusal reads as a decision: it is not the failure above
                // it, and not the silent success in the other column.
                .child(
                    ToolCallCard::new("scene.tool.refused", "shell.run")
                        .arguments("{ \"command\": \"rm -rf build\" }")
                        .state(ToolCallState::refused(
                            "This workspace does not allow shell commands.",
                        )),
                ),
        )
        .into_any_element()
}

fn step_list(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(caption(&theme, "A run somebody counted"))
        .child(
            StepList::new("scene.steps.counted")
                .step(Step::new("read", "Read the brief").state(StepState::Done))
                .step(
                    Step::new("search", "Search the workspace")
                        .state(StepState::Running)
                        .body(
                            ToolCallCard::new("scene.steps.search.call", "workspace.search")
                                .arguments(scene_arguments())
                                .state(ToolCallState::Running)
                                .elapsed("1.1 s"),
                        ),
                )
                .step(Step::new("summarise", "Summarise what was found"))
                .step(
                    Step::new("publish", "Publish the summary").state(StepState::Skipped(
                        "Publishing is turned off for this workspace.".into(),
                    )),
                )
                .step(
                    // A failure, not a refusal: the skipped step above is what
                    // a refusal looks like, and the two must not be worded
                    // into each other.
                    Step::new("notify", "Notify the reviewers").state(StepState::Failed(
                        "The notification service did not respond.".into(),
                    )),
                ),
        )
        .child(caption(&theme, "A run still being decided"))
        .child(
            StepList::new("scene.steps.open")
                .length(RunLength::Unknown)
                .step(Step::new("read", "Read the brief").state(StepState::Done))
                .step(Step::new("plan", "Decide what to do next").state(StepState::Running)),
        )
        .into_any_element()
}

fn thinking(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(ThinkingBlock::new(
            "scene.thinking.collapsed",
            Reasoning::present("The brief asks for two files, so read both before answering."),
        ))
        // Reasoning that has finished and reasoning still arriving are the
        // same words in the same place, so the difference has to be shown.
        .child(
            ThinkingBlock::new(
                "scene.thinking.working",
                Reasoning::present("Reading the second file before answering."),
            )
            .thinking(true),
        )
        .child(
            ThinkingBlock::new(
                "scene.thinking.open",
                Reasoning::present(
                    "The brief asks for two files.\nRead both before answering.\nThen summarise.",
                ),
            )
            .expanded(true)
            .on_toggle(|_, _, _| {}),
        )
        // Withheld and absent are two different facts, and neither is the
        // collapsed block above.
        .child(ThinkingBlock::new(
            "scene.thinking.withheld",
            Reasoning::withheld("This connection does not hand over reasoning."),
        ))
        .child(ThinkingBlock::new(
            "scene.thinking.absent",
            Reasoning::Absent,
        ))
        .into_any_element()
}

// ------------------------------------------------- structured data and agents

/// A document with the three facts a viewer usually confuses: a key holding
/// `null`, a key holding an empty object, and a key that is simply not here.
fn scene_document() -> JsonValue {
    JsonValue::object([
        ("id", JsonValue::string("run-4812")),
        ("attempts", JsonValue::number("3")),
        ("streaming", JsonValue::Bool(true)),
        // Nothing has been recorded here yet, which is not the same as the
        // key being missing: `resumed_from` is missing, and says nothing.
        ("cursor", JsonValue::Null),
        ("labels", JsonValue::object(Vec::<(&str, JsonValue)>::new())),
        (
            "credentials",
            JsonValue::object([("token", JsonValue::redacted("51 characters"))]),
        ),
        (
            "request",
            JsonValue::object([
                ("method", JsonValue::string("POST")),
                (
                    "headers",
                    JsonValue::object([
                        ("content-type", JsonValue::string("application/json")),
                        ("authorization", JsonValue::redacted("a value")),
                    ]),
                ),
            ]),
        ),
        (
            "steps",
            JsonValue::array([
                JsonValue::string("plan"),
                JsonValue::string("apply"),
                JsonValue::object([("retries", JsonValue::number("0"))]),
            ]),
        ),
    ])
}

fn json_view(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(
            JsonView::new("scene.json", scene_document())
                .expanded_paths(&["credentials", "request", "request/headers", "steps"])
                .selected("request/method")
                .on_toggle(|_, _, _, _| {})
                .on_select(|_, _, _| {}),
        )
        .into_any_element()
}

/// The form kept across frames.
///
/// Its fields own carets, open menus and a selection, so rebuilding it every
/// frame would throw away whatever had been typed into it.
struct SceneSchemaForm {
    form: Entity<SchemaForm>,
}

impl Global for SceneSchemaForm {}

fn scene_schema() -> Schema {
    Schema::new()
        .field(
            SchemaField::new(
                "path",
                SchemaKind::Text {
                    placeholder: Some("relative to the workspace".into()),
                    secret: false,
                },
            )
            .label("File")
            .description("Which file the call reads")
            .required(true),
        )
        .field(
            SchemaField::new(
                "max_bytes",
                SchemaKind::Integer(NumberBounds::new().min(1.0).max(65_536.0).step(1024.0)),
            )
            .label("Maximum bytes"),
        )
        .field(
            SchemaField::new("follow_symlinks", SchemaKind::Boolean).label("Follow symbolic links"),
        )
        .field(
            SchemaField::new(
                "encoding",
                SchemaKind::Enum(vec![
                    SchemaChoice::new("utf-8", "UTF-8"),
                    SchemaChoice::new("latin-1", "Latin-1"),
                ]),
            )
            .label("Encoding")
            .required(true),
        )
        .field(
            SchemaField::new(
                "profile",
                SchemaKind::OpenEnum(vec![
                    SchemaChoice::new("fast", "Fast"),
                    SchemaChoice::new("thorough", "Thorough"),
                ]),
            )
            .label("Profile")
            .description("One of these, or whatever you type"),
        )
        .field(SchemaField::new("tags", SchemaKind::TextList { max: Some(4) }).label("Tags"))
        .field(
            SchemaField::new(
                "limits",
                SchemaKind::Object(vec![
                    SchemaField::new("timeout_ms", SchemaKind::Integer(NumberBounds::new()))
                        .label("Timeout in milliseconds"),
                ]),
            )
            .label("Limits"),
        )
        .field(
            SchemaField::new(
                "matcher",
                SchemaKind::Unrenderable(
                    "This argument is one of three shapes at once, and no single control \
                     stands for that."
                        .into(),
                ),
            )
            .label("Matcher")
            .required(true),
        )
}

fn schema_form(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneSchemaForm>() {
        let form = cx.new(|cx| SchemaForm::new("scene.schema", scene_schema(), window, cx));
        form.update(cx, |form, cx| {
            // An error the host returned rather than one the form derived:
            // only the host knows this path is outside the workspace.
            form.set_error("path", "That path is outside the workspace.", cx);
        });
        cx.set_global(SceneSchemaForm { form });
    }
    let form = cx.global::<SceneSchemaForm>().form.clone();
    let theme = cx.theme().clone();
    stack(&theme).w(px(520.0)).child(form).into_any_element()
}

fn server_list(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .child(
            // Only the one with a catalogue is opened. Expanding the two empty
            // ones as well pushed disconnected, failed, and disabled out of
            // the captured screen, and a state no image shows is a state no
            // image guards.
            ServerList::new("scene.servers")
                .expanded_ids(&["workspace"])
                .selected("workspace")
                .servers([
                    ServerEntry::new("workspace", "Workspace tools")
                        .detail("Running beside this window")
                        .state(ServerState::Connected)
                        .offers([
                            Offering::tool("read", "Read a file")
                                .summary("Returns the contents of one file")
                                .qualifier("path, max_bytes"),
                            Offering::tool("write", "Write a file")
                                .summary("Replaces the contents of one file"),
                            Offering::skill("review", "Review a change")
                                .summary("Reads a diff and reports what it finds"),
                            Offering::resource("changelog", "Changelog")
                                .qualifier("workspace:/CHANGELOG.md"),
                        ]),
                    ServerEntry::new("index", "Search index")
                        .detail("Answered, and the answer was empty")
                        .state(ServerState::Connected)
                        .offers([]),
                    ServerEntry::new("archive", "Archive")
                        .detail("Nobody has asked it anything yet")
                        .state(ServerState::Connected),
                    ServerEntry::new("build", "Build runner")
                        .state(ServerState::Connecting)
                        .catalog(Catalog::Asking),
                    ServerEntry::new("notes", "Notes").state(ServerState::Disconnected),
                    ServerEntry::new("deploy", "Deployment").state(ServerState::Failed {
                        reason: "The connection was refused after three attempts.".into(),
                    }),
                    ServerEntry::new("telemetry", "Telemetry").state(ServerState::Disabled {
                        reason: Some("You turned this one off.".into()),
                    }),
                ])
                .on_select(|_, _, _| {})
                .on_retry(|_, _, _| {})
                .on_toggle(|_, _, _, _| {}),
        )
        .into_any_element()
}

fn offering_catalog(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let workspace = OfferingSource::new(
        "workspace",
        "Workspace tools",
        OfferingSourceState::Ready(vec![
            SearchableOffering::new(
                Offering::tool("read", "Read a file")
                    .summary("Returns the contents of one file")
                    .qualifier("path, max_bytes"),
                "read file contents path workspace",
            ),
            SearchableOffering::new(
                Offering::skill("review", "Review a change")
                    .summary("Reads a diff and reports what it finds"),
                "review change diff findings",
            ),
        ]),
    );
    let archive = OfferingSource::new(
        "archive",
        "Archive tools",
        OfferingSourceState::Stale {
            offerings: vec![
                SearchableOffering::new(
                    Offering::tool("read", "Read a file").summary("Reads one archived file"),
                    "read archived file contents",
                ),
                SearchableOffering::new(
                    Offering::resource("changelog", "Changelog").qualifier("archive:/CHANGELOG.md"),
                    "changelog changes release history",
                ),
            ],
            reason: "Archive refresh failed; showing the last verified results.".into(),
        },
    );
    stack(&theme)
        .w(px(560.0))
        .child(
            OfferingCatalog::new("scene.offering-catalog")
                .sources([workspace, archive])
                .selected(OfferingIdentity::new("workspace", "read"))
                .on_activate(|_, _, _| {}),
        )
        .into_any_element()
}

/// The ordinary-application views that own state across frames.
struct SceneOrdinary {
    menubar: Entity<Menubar>,
    hover_card: Entity<HoverCard>,
    copy_idle: Entity<CopyButton>,
    copy: Entity<CopyButton>,
    copy_refused: Entity<CopyButton>,
}

impl Global for SceneOrdinary {}

fn ensure_ordinary(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneOrdinary>() {
        return;
    }
    let menubar = cx.new(|cx| {
        Menubar::new(
            "scene.menubar",
            [
                MenubarMenu::new(
                    "file",
                    "File",
                    [
                        MenuItem::command("file.new", "New run").shortcut("cmd-n"),
                        MenuItem::command("file.open", "Open workspace").shortcut("cmd-o"),
                        MenuItem::separator("file.rule"),
                        MenuItem::submenu(
                            "file.export",
                            "Export",
                            [
                                MenuItem::command("file.export.json", "As JSON"),
                                MenuItem::command("file.export.text", "As plain text"),
                            ],
                        ),
                    ],
                ),
                MenubarMenu::new(
                    "edit",
                    "Edit",
                    [
                        MenuItem::command("edit.undo", "Undo").shortcut("cmd-z"),
                        MenuItem::check("edit.wrap", "Wrap lines", true),
                    ],
                ),
                MenubarMenu::new("view", "View", [MenuItem::command("view.zoom", "Zoom in")]),
                MenubarMenu::new("policy", "Policy", []).disabled(true),
            ],
            window,
            cx,
        )
    });
    menubar.update(cx, |bar, cx| bar.open("file", window, cx));

    let hover_card = cx.new(|cx| {
        HoverCard::new("scene.hover-card", window, cx)
            .name("Run 4821")
            .trigger(|_, cx| {
                let theme = cx.theme().clone();
                crate::foundation::text(&theme, TypeScale::Caption, "run 4821")
                    .text_color(theme.colors.accent)
                    .into_any_element()
            })
            .content(|_, cx| {
                let theme = cx.theme().clone();
                div()
                    .column()
                    // A column stretches its children across, and a badge
                    // stretched that far stops reading as a label and starts
                    // reading as a filled bar.
                    .items_start()
                    .gap(px(theme.spacing.xs))
                    .child(crate::foundation::text(
                        &theme,
                        TypeScale::Strong,
                        "Nightly regression sweep",
                    ))
                    .child(
                        crate::foundation::text(
                            &theme,
                            TypeScale::Caption,
                            "Finished in 4 minutes, 12 checks, none failed.",
                        )
                        .text_tone(&theme, TextTone::Muted),
                    )
                    .child(Badge::new("Ready").success())
                    .into_any_element()
            })
    });
    hover_card.update(cx, |card, cx| card.open(cx));

    // Nobody has pressed this one. It is the state a reader sees almost all
    // of the time, and it was the one state of the three no image held.
    let copy_idle = cx.new(|cx| {
        CopyButton::new("scene.copy-idle", window, cx)
            .text("run-4821-9f3a")
            .copier(|_, _| Ok(()))
    });

    // The confirmation outlives the capture. At its ordinary length it expires
    // partway through a run, so which of the two states this scene showed
    // depended on how long the run took to reach it.
    let copy = cx.new(|cx| {
        CopyButton::new("scene.copy", window, cx)
            .text("run-4821-9f3a")
            .confirmation(std::time::Duration::from_secs(60 * 60))
            .copier(|_, _| Ok(()))
    });
    copy.update(cx, |button, cx| button.copy(cx));

    let copy_refused = cx.new(|cx| {
        CopyButton::new("scene.copy-refused", window, cx)
            .text("run-4821-9f3a")
            .copier(|_, _| Err("The clipboard did not take it.".into()))
    });
    copy_refused.update(cx, |button, cx| button.copy(cx));

    cx.set_global(SceneOrdinary {
        menubar,
        hover_card,
        copy_idle,
        copy,
        copy_refused,
    });
}

fn toggle(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .child(caption(&theme, "A button that stays in"))
        .child(
            row(&theme)
                .child(
                    Toggle::new("scene.toggle.bold")
                        .label("Bold")
                        .pressed(true)
                        .on_press(|_, _, _| {}),
                )
                .child(
                    Toggle::new("scene.toggle.italic")
                        .label("Italic")
                        .on_press(|_, _, _| {}),
                )
                .child(
                    Toggle::new("scene.toggle.review")
                        .label("Review mode")
                        .secondary()
                        .pressed(true)
                        .on_press(|_, _, _| {}),
                )
                .child(
                    Toggle::new("scene.toggle.locked")
                        .label("Locked")
                        .disabled(true),
                ),
        )
        .child(caption(&theme, "Any number in at once"))
        .child(
            ToggleGroup::new("scene.toggle-group.format")
                .label("Formatting")
                .selection(ToggleSelection::Any)
                .items([
                    ToggleItem::new("bold", "Bold"),
                    ToggleItem::new("italic", "Italic"),
                    ToggleItem::new("underline", "Underline").disabled(true),
                ])
                .pressed_ids(&["bold", "italic"])
                .on_change(|_, _, _, _| {}),
        )
        .child(caption(
            &theme,
            "One or none, which a segmented strip cannot say",
        ))
        .child(
            ToggleGroup::new("scene.toggle-group.density")
                .label("Density")
                .selection(ToggleSelection::AtMostOne)
                .items([
                    ToggleItem::new("compact", "Compact"),
                    ToggleItem::new("cosy", "Cosy"),
                    ToggleItem::new("roomy", "Roomy"),
                ])
                .pressed_ids(&["cosy"])
                .on_change(|_, _, _, _| {}),
        )
        .into_any_element()
}

fn collapsible(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(
            Collapsible::new("scene.collapsible.open", "Advanced")
                .description("Settings most runs never touch")
                .open(true)
                .body(crate::foundation::text(
                    &theme,
                    TypeScale::Body,
                    "Requests go out over the system proxy.",
                ))
                .on_toggle(|_, _, _| {}),
        )
        .child(
            Collapsible::new("scene.collapsible.shut", "Diagnostics")
                .description("Nothing is collected until this is opened")
                .body(crate::foundation::text(
                    &theme,
                    TypeScale::Body,
                    "This body is absent from the tree while it is shut.",
                ))
                .on_toggle(|_, _, _| {}),
        )
        .child(
            Collapsible::new("scene.collapsible.refused", "Managed by policy")
                .description("This machine cannot change these")
                .disabled(true)
                .body(crate::foundation::text(
                    &theme,
                    TypeScale::Body,
                    "Set by the administrator.",
                )),
        )
        .into_any_element()
}

fn hover_card(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_ordinary(window, cx);
    let card = cx.global::<SceneOrdinary>().hover_card.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(460.0))
        .h(px(300.0))
        .child(caption(&theme, "A preview the pointer can travel into"))
        .child(
            row(&theme)
                .child(crate::foundation::text(
                    &theme,
                    TypeScale::Label,
                    "Reported by",
                ))
                .child(card),
        )
        .into_any_element()
}

fn menubar(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_ordinary(window, cx);
    let bar = cx.global::<SceneOrdinary>().menubar.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .h(px(360.0))
        .child(bar)
        .into_any_element()
}

fn copy_button(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_ordinary(window, cx);
    let scene = cx.global::<SceneOrdinary>();
    let idle = scene.copy_idle.clone();
    let copied = scene.copy.clone();
    let refused = scene.copy_refused.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .child(caption(&theme, "Nobody has pressed it yet"))
        .child(idle)
        .child(caption(&theme, "The clipboard took it"))
        .child(copied)
        .child(caption(&theme, "It did not go through, and says so"))
        .child(refused)
        .into_any_element()
}

fn aspect_ratio(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let filled = |label: &'static str| {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.colors.hover)
            .child(
                crate::foundation::text(&theme, TypeScale::Label, label)
                    .text_tone(&theme, TextTone::Muted),
            )
    };
    stack(&theme)
        .child(caption(&theme, "Width given, height from the ratio"))
        .child(
            div().w(px(320.0)).child(
                AspectRatio::of("scene.aspect.wide", 16.0, 9.0)
                    .width_driven()
                    .child(filled("16 by 9")),
            ),
        )
        .child(caption(&theme, "Height given, width from the ratio"))
        .child(
            div().h(px(120.0)).child(
                AspectRatio::new("scene.aspect.square", 1.0)
                    .height_driven()
                    .child(filled("square")),
            ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_names_are_unique_and_addressable() {
        let mut names: Vec<&str> = catalog().iter().map(|scene| scene.name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
        assert!(find("button").is_some());
        assert!(find("nothing").is_none());
    }
}
