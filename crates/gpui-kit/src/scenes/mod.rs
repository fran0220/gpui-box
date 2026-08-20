//! One canonical rendering per component, grouped the way the components are.
//!
//! A scene is the single description of a component's states that both the
//! gallery and the audit tests consume, so a component cannot be reviewed
//! visually in one arrangement and tested in another.
//!
//! Almost every scene is an *exhibit*: it is about one component, it lives in
//! the file named after that component's family, and it is where a reader is
//! sent to review it. `xtask api check` fails when a public component has no
//! exhibit, so a component cannot be added and left unseen.
//!
//! The rest are *compositions*, and there are three of them. A composition is
//! built the way a product would build one, because components interact in
//! ways none of them shows alone. It is nobody's coverage, and [`Shows`]
//! records the difference so the catalog does not claim a component was
//! reviewed when it was only recognised inside a shell.
//!
//! What is deliberately not here is a scene per screenshot. The files below
//! are the component families, not a taxonomy invented for this module, so a
//! component and the rendering that reviews it move together.

mod support;

mod agent;
mod canvas;
mod compositions;
mod content;
mod controls;
mod data;
#[cfg(feature = "fixtures")]
mod datetime;
mod display;
mod effects;
mod game;
mod layout;
mod media;
mod motion;
mod navigation;
mod overlay;
mod structured;

use gpui::{AnyElement, App, Window};

use crate::foundation::direction::LayoutDirection;

use agent::{
    agent_avatar, agent_roster, agent_run_canvas, agent_run_issues, approval, artifact_preview,
    cost_meter, feedback_rating, offering_catalog, permission_matrix, persona, prompt_builder,
    server_list, step_list, thinking, tool_call,
};
use canvas::{canvas_tools, node_graph};
use compositions::{motion_flip, motion_state, reading_direction};
#[cfg(all(feature = "terminal", not(target_family = "wasm")))]
use content::terminal;
use content::{
    agent_document, browser_panel, code_view, conversation, conversation_growing, diff_view,
    image_viewer, log_stream, markdown, transport,
};
use controls::{
    actions, auth_sign_in, auth_verification, button, cascader, choice, color_picker, copy_button,
    dropzone, filter_bar, find_replace, form, inline_edit, input, keybinding, keymap_editor,
    search_field, settings, textarea, toggle, upload_list,
};
use data::{
    data_grid, data_grid_editing, diagnostics_list, drag_list, drag_tree, kanban, list, table,
    tree, tree_grid,
};
#[cfg(feature = "fixtures")]
use datetime::{calendar, date_range, date_time};
use display::{
    animated_number, avatar, badge, card, chart, detail, divider, empty_state, failure_panel,
    heatmap, icon, loading, metric_card, progress_bar, progress_circle, sparkline, status, tag,
    trace,
};
use effects::{cinematic_effects, visual_effects};
use game::game_ui;
use layout::{
    aspect_ratio, ide_shell, responsive, scroll_area, scroll_fade, scroll_shadow, split_pane,
    split_tree, toolbar,
};
use media::{audio_player, audio_waveform, model_viewer, video_player};
use motion::micro;
use navigation::{
    accordion, anchor_list, breadcrumb, collapsible, document_tabs, pagination, sidebar, tabs,
    undo_history, wizard,
};
use overlay::{
    command_palette, context_menu, dialog, drawer, frost, glass, hover_card, kbd, menu, menubar,
    notification_center, overlay, popover, toast, tooltip,
};
use structured::{json_view, schema_form};

/// What a rendering exists to show.
///
/// The distinction is the difference between reviewing a component and
/// recognising it. A component that has only ever been seen inside a shell has
/// been recognised; nobody has looked at its states, and the catalog should
/// not claim otherwise, which is why only [`Shows::Subjects`] counts as
/// coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shows {
    /// The components this rendering is *about*. It arranges them so their
    /// states can be compared, and it is where a reader is sent to review
    /// them. Everything else it draws is scaffolding.
    Subjects(&'static [&'static str]),
    /// An arrangement the way a product would build one, kept because
    /// components interact in ways none of them shows alone. It is nobody's
    /// coverage: every component it draws is reviewed somewhere else.
    Composition(&'static [&'static str]),
}

impl Shows {
    /// Every component named, whichever kind this is.
    pub fn components(&self) -> &'static [&'static str] {
        match self {
            Self::Subjects(names) | Self::Composition(names) => names,
        }
    }

    /// The components this rendering is the review of, which is empty for a
    /// composition.
    pub fn subjects(&self) -> &'static [&'static str] {
        match self {
            Self::Subjects(names) => names,
            Self::Composition(_) => &[],
        }
    }
}

/// One canonical rendering, addressed by name.
pub struct Scene {
    pub name: &'static str,
    pub build: fn(&mut Window, &mut App) -> AnyElement,
    /// What this rendering is for, declared rather than inferred.
    ///
    /// It used to be read back out of the source by following every helper a
    /// scene called, which answered a different question — *what types does
    /// this code path touch* — and answered it as an upper bound. Three
    /// unrelated scenes that shared one fixture helper reported the same seven
    /// components, so "which scene shows me" was a claim no reader could rely
    /// on. Declaring it makes it exact, and `xtask api check` holds the
    /// declaration to what the source can actually reach.
    pub shows: Shows,
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Scene")
            .field("name", &self.name)
            .field("shows", &self.shows)
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
            shows: Shows::Subjects(&["Button"]),
        },
        Scene {
            name: "badge",
            build: badge,
            shows: Shows::Subjects(&["Badge"]),
        },
        Scene {
            name: "card",
            build: card,
            shows: Shows::Subjects(&["Card", "ListRow"]),
        },
        Scene {
            name: "status",
            build: status,
            shows: Shows::Subjects(&["Callout", "StatusDot", "StatusLine"]),
        },
        Scene {
            name: "loading",
            build: loading,
            shows: Shows::Subjects(&["GradientSpinner", "PulseLoader", "Skeleton"]),
        },
        Scene {
            name: "visual-effects",
            build: visual_effects,
            shows: Shows::Subjects(&["EffectParticles"]),
        },
        Scene {
            name: "cinematic-effects",
            build: cinematic_effects,
            shows: Shows::Subjects(&["CinematicEffect"]),
        },
        Scene {
            name: "choice",
            build: choice,
            shows: Shows::Subjects(&["Checkbox", "Radio", "Slider", "Switch"]),
        },
        Scene {
            name: "input",
            build: input,
            shows: Shows::Subjects(&["Select", "TextInput"]),
        },
        Scene {
            name: "textarea",
            build: textarea,
            shows: Shows::Subjects(&["TextArea"]),
        },
        Scene {
            name: "form",
            build: form,
            shows: Shows::Subjects(&[
                "Combobox",
                "FormField",
                "NumberInput",
                "SegmentedControl",
                "TagInput",
            ]),
        },
        Scene {
            name: "auth-sign-in",
            build: auth_sign_in,
            shows: Shows::Subjects(&["PasswordInput"]),
        },
        Scene {
            name: "auth-verification",
            build: auth_verification,
            shows: Shows::Subjects(&["OneTimeCodeInput"]),
        },
        Scene {
            name: "actions",
            build: actions,
            shows: Shows::Subjects(&["ButtonGroup", "IconButton", "SplitButton"]),
        },
        Scene {
            name: "progress-bar",
            build: progress_bar,
            shows: Shows::Subjects(&["ProgressBar"]),
        },
        Scene {
            name: "divider",
            build: divider,
            shows: Shows::Subjects(&["Divider"]),
        },
        Scene {
            name: "tag",
            build: tag,
            shows: Shows::Subjects(&["Tag"]),
        },
        Scene {
            name: "avatar",
            build: avatar,
            shows: Shows::Subjects(&["Avatar"]),
        },
        Scene {
            name: "empty-state",
            build: empty_state,
            shows: Shows::Subjects(&["EmptyState"]),
        },
        Scene {
            name: "kbd",
            build: kbd,
            shows: Shows::Subjects(&["Kbd"]),
        },
        Scene {
            name: "overlay",
            build: overlay,
            shows: Shows::Subjects(&["Overlay"]),
        },
        Scene {
            name: "dialog",
            build: dialog,
            shows: Shows::Subjects(&["Dialog"]),
        },
        Scene {
            name: "tooltip",
            build: tooltip,
            shows: Shows::Subjects(&["Tooltip"]),
        },
        Scene {
            name: "menu",
            build: menu,
            shows: Shows::Subjects(&["Menu"]),
        },
        Scene {
            name: "context-menu",
            build: context_menu,
            shows: Shows::Subjects(&["ContextMenu"]),
        },
        Scene {
            name: "popover",
            build: popover,
            shows: Shows::Subjects(&["Popover"]),
        },
        Scene {
            name: "command-palette",
            build: command_palette,
            shows: Shows::Subjects(&["CommandPalette"]),
        },
        Scene {
            name: "toast",
            build: toast,
            shows: Shows::Subjects(&["ToastLayer"]),
        },
        Scene {
            name: "tabs",
            build: tabs,
            shows: Shows::Subjects(&["Tabs"]),
        },
        Scene {
            name: "accordion",
            build: accordion,
            shows: Shows::Subjects(&["Accordion"]),
        },
        Scene {
            name: "breadcrumb",
            build: breadcrumb,
            shows: Shows::Subjects(&["Breadcrumb"]),
        },
        Scene {
            name: "list",
            build: list,
            shows: Shows::Subjects(&["List"]),
        },
        Scene {
            name: "table",
            build: table,
            shows: Shows::Subjects(&["Table"]),
        },
        Scene {
            name: "data-grid",
            build: data_grid,
            shows: Shows::Subjects(&["BulkBar", "DataGrid"]),
        },
        Scene {
            name: "data-grid-editing",
            build: data_grid_editing,
            shows: Shows::Subjects(&["DataGrid"]),
        },
        Scene {
            name: "tree-grid",
            build: tree_grid,
            shows: Shows::Subjects(&["TreeGrid"]),
        },
        Scene {
            name: "tree",
            build: tree,
            shows: Shows::Subjects(&["Tree"]),
        },
        Scene {
            name: "split-pane",
            build: split_pane,
            shows: Shows::Subjects(&["SplitPane"]),
        },
        Scene {
            name: "scroll-area",
            build: scroll_area,
            shows: Shows::Subjects(&["ScrollArea"]),
        },
        Scene {
            name: "scroll-shadow",
            build: scroll_shadow,
            shows: Shows::Subjects(&["ScrollArea"]),
        },
        Scene {
            name: "scroll-fade",
            build: scroll_fade,
            shows: Shows::Subjects(&["ScrollFade"]),
        },
        Scene {
            name: "frost",
            build: frost,
            shows: Shows::Subjects(&["Frost"]),
        },
        Scene {
            name: "glass",
            build: glass,
            shows: Shows::Subjects(&["Glass"]),
        },
        Scene {
            name: "toolbar",
            build: toolbar,
            shows: Shows::Subjects(&["Toolbar"]),
        },
        Scene {
            name: "sidebar",
            build: sidebar,
            shows: Shows::Subjects(&["Sidebar"]),
        },
        Scene {
            name: "pagination",
            build: pagination,
            shows: Shows::Subjects(&["Pagination"]),
        },
        Scene {
            name: "drawer",
            build: drawer,
            shows: Shows::Subjects(&["Drawer"]),
        },
        Scene {
            name: "motion-flip",
            build: motion_flip,
            shows: Shows::Composition(&["Badge", "Button", "Card", "ListRow"]),
        },
        Scene {
            name: "motion-state",
            build: motion_state,
            shows: Shows::Composition(&[
                "Accordion",
                "Button",
                "Checkbox",
                "ProgressBar",
                "Radio",
                "SegmentedControl",
                "Switch",
                "Tabs",
            ]),
        },
        Scene {
            name: "animated-number",
            build: animated_number,
            shows: Shows::Subjects(&["AnimatedNumber"]),
        },
        Scene {
            name: "drag-list",
            build: drag_list,
            shows: Shows::Subjects(&["List"]),
        },
        Scene {
            name: "drag-tree",
            build: drag_tree,
            shows: Shows::Subjects(&["Tree"]),
        },
        Scene {
            name: "dropzone",
            build: dropzone,
            shows: Shows::Subjects(&["Dropzone"]),
        },
        Scene {
            name: "wizard",
            build: wizard,
            shows: Shows::Subjects(&["Wizard"]),
        },
        Scene {
            name: "undo-history",
            build: undo_history,
            shows: Shows::Subjects(&["UndoHistory"]),
        },
        Scene {
            name: "settings",
            build: settings,
            shows: Shows::Subjects(&["SettingsRow", "SettingsSection"]),
        },
        Scene {
            name: "detail",
            build: detail,
            shows: Shows::Subjects(&["DescriptionList", "Timeline"]),
        },
        Scene {
            name: "filter-bar",
            build: filter_bar,
            shows: Shows::Subjects(&["FilterBar"]),
        },
        Scene {
            name: "inline-edit",
            build: inline_edit,
            shows: Shows::Subjects(&["InlineEdit"]),
        },
        Scene {
            name: "progress-circle",
            build: progress_circle,
            shows: Shows::Subjects(&["ProgressCircle"]),
        },
        Scene {
            name: "split-tree",
            build: split_tree,
            shows: Shows::Subjects(&["SplitTree"]),
        },
        Scene {
            name: "ide-shell",
            build: ide_shell,
            shows: Shows::Subjects(&["Dock", "StatusBar"]),
        },
        Scene {
            name: "keybinding",
            build: keybinding,
            shows: Shows::Subjects(&["KeybindingRecorder"]),
        },
        Scene {
            name: "keymap-editor",
            build: keymap_editor,
            shows: Shows::Subjects(&["KeymapEditor"]),
        },
        Scene {
            name: "markdown",
            build: markdown,
            shows: Shows::Subjects(&["Markdown"]),
        },
        Scene {
            name: "agent-document",
            build: agent_document,
            shows: Shows::Subjects(&["AgentDocument"]),
        },
        Scene {
            name: "agent-roster",
            build: agent_roster,
            shows: Shows::Subjects(&["AgentCard", "AgentGroup", "AgentRoster", "SubagentTree"]),
        },
        Scene {
            name: "persona",
            build: persona,
            shows: Shows::Subjects(&["PersonaDialogue", "PersonaPortrait", "VoiceReactive"]),
        },
        Scene {
            name: "game-ui",
            build: game_ui,
            shows: Shows::Subjects(&[
                "AbilityBar",
                "ObjectiveTracker",
                "PartyRoster",
                "RewardReveal",
            ]),
        },
        Scene {
            name: "agent-run-canvas",
            build: agent_run_canvas,
            shows: Shows::Subjects(&["AgentRunCanvas"]),
        },
        Scene {
            name: "agent-run-issues",
            build: agent_run_issues,
            shows: Shows::Subjects(&["AgentRunIssues"]),
        },
        Scene {
            name: "agent-avatar",
            build: agent_avatar,
            shows: Shows::Subjects(&["AgentActivityLine", "AgentAvatar"]),
        },
        Scene {
            name: "conversation",
            build: conversation,
            shows: Shows::Subjects(&["MessageList"]),
        },
        Scene {
            name: "conversation-growing",
            build: conversation_growing,
            shows: Shows::Subjects(&["MessageList"]),
        },
        Scene {
            name: "image-viewer",
            build: image_viewer,
            shows: Shows::Subjects(&["ImageViewer"]),
        },
        Scene {
            name: "transport",
            build: transport,
            shows: Shows::Subjects(&["TransportBar"]),
        },
        Scene {
            name: "audio-player",
            build: audio_player,
            shows: Shows::Subjects(&["AudioPlayer"]),
        },
        Scene {
            name: "audio-waveform",
            build: audio_waveform,
            shows: Shows::Subjects(&["AudioWaveform"]),
        },
        Scene {
            name: "video-player",
            build: video_player,
            shows: Shows::Subjects(&["VideoPlayer"]),
        },
        Scene {
            name: "model-viewer",
            build: model_viewer,
            shows: Shows::Subjects(&["ModelViewer"]),
        },
        Scene {
            name: "approval",
            build: approval,
            shows: Shows::Subjects(&["ApprovalPrompt"]),
        },
        Scene {
            name: "permission-matrix",
            build: permission_matrix,
            shows: Shows::Subjects(&["PermissionMatrix"]),
        },
        Scene {
            name: "cost-meter",
            build: cost_meter,
            shows: Shows::Subjects(&["ContextGauge", "CostMeter"]),
        },
        Scene {
            name: "prompt-builder",
            build: prompt_builder,
            shows: Shows::Subjects(&["PromptBuilder"]),
        },
        Scene {
            name: "feedback-rating",
            build: feedback_rating,
            shows: Shows::Subjects(&["FeedbackRating"]),
        },
        Scene {
            name: "artifact-preview",
            build: artifact_preview,
            shows: Shows::Subjects(&["ArtifactPreview"]),
        },
        Scene {
            name: "tool-call",
            build: tool_call,
            shows: Shows::Subjects(&["ToolCallCard"]),
        },
        Scene {
            name: "step-list",
            build: step_list,
            shows: Shows::Subjects(&["StepList"]),
        },
        Scene {
            name: "node-graph",
            build: node_graph,
            shows: Shows::Subjects(&["GraphNode", "NodeGraph"]),
        },
        Scene {
            name: "canvas-tools",
            build: canvas_tools,
            shows: Shows::Subjects(&["CanvasToolbar", "Minimap", "NodeGroup"]),
        },
        Scene {
            name: "browser-panel",
            build: browser_panel,
            shows: Shows::Subjects(&["BrowserPanel"]),
        },
        Scene {
            name: "thinking",
            build: thinking,
            shows: Shows::Subjects(&["ThinkingBlock"]),
        },
        Scene {
            name: "json-view",
            build: json_view,
            shows: Shows::Subjects(&["JsonView"]),
        },
        Scene {
            name: "schema-form",
            build: schema_form,
            shows: Shows::Subjects(&["SchemaForm"]),
        },
        Scene {
            name: "server-list",
            build: server_list,
            shows: Shows::Subjects(&["ServerList"]),
        },
        Scene {
            name: "offering-catalog",
            build: offering_catalog,
            shows: Shows::Subjects(&["OfferingCatalog"]),
        },
        Scene {
            name: "reading-direction",
            build: reading_direction,
            shows: Shows::Composition(&[
                "Accordion",
                "Breadcrumb",
                "Button",
                "Card",
                "Icon",
                "JsonView",
                "Tabs",
                "Tree",
            ]),
        },
        Scene {
            name: "toggle",
            build: toggle,
            shows: Shows::Subjects(&["Toggle", "ToggleGroup"]),
        },
        Scene {
            name: "collapsible",
            build: collapsible,
            shows: Shows::Subjects(&["Collapsible"]),
        },
        Scene {
            name: "hover-card",
            build: hover_card,
            shows: Shows::Subjects(&["HoverCard"]),
        },
        Scene {
            name: "menubar",
            build: menubar,
            shows: Shows::Subjects(&["Menubar"]),
        },
        Scene {
            name: "copy-button",
            build: copy_button,
            shows: Shows::Subjects(&["CopyButton"]),
        },
        Scene {
            name: "aspect-ratio",
            build: aspect_ratio,
            shows: Shows::Subjects(&["AspectRatio"]),
        },
        Scene {
            name: "responsive",
            build: responsive,
            shows: Shows::Subjects(&["Responsive"]),
        },
        Scene {
            name: "icon",
            build: icon,
            shows: Shows::Subjects(&["Icon"]),
        },
        Scene {
            name: "document-tabs",
            build: document_tabs,
            shows: Shows::Subjects(&["Tabs"]),
        },
        Scene {
            name: "search-field",
            build: search_field,
            shows: Shows::Subjects(&["HighlightedText", "SearchField"]),
        },
        Scene {
            name: "find-replace",
            build: find_replace,
            shows: Shows::Subjects(&["FindReplace"]),
        },
        Scene {
            name: "notification-center",
            build: notification_center,
            shows: Shows::Subjects(&["NotificationCenter"]),
        },
        Scene {
            name: "failure-panel",
            build: failure_panel,
            shows: Shows::Subjects(&["FailurePanel"]),
        },
        Scene {
            name: "log-stream",
            build: log_stream,
            shows: Shows::Subjects(&["LogStream"]),
        },
        Scene {
            name: "diff-view",
            build: diff_view,
            shows: Shows::Subjects(&["DiffView"]),
        },
        Scene {
            name: "sparkline",
            build: sparkline,
            shows: Shows::Subjects(&["Sparkline"]),
        },
        Scene {
            name: "chart",
            build: chart,
            shows: Shows::Subjects(&[
                "AreaChart",
                "BarChart",
                "ChartLegend",
                "GaugeChart",
                "LineChart",
                "PieChart",
                "RadarChart",
                "ScatterChart",
                "StackedBarChart",
            ]),
        },
        Scene {
            name: "metric-card",
            build: metric_card,
            shows: Shows::Subjects(&["MetricCard"]),
        },
        Scene {
            name: "kanban",
            build: kanban,
            shows: Shows::Subjects(&["KanbanBoard"]),
        },
        Scene {
            name: "micro",
            build: micro,
            shows: Shows::Subjects(&["MicroMark"]),
        },
        Scene {
            name: "trace",
            build: trace,
            shows: Shows::Subjects(&["SpanTimeline", "TraceView"]),
        },
        Scene {
            name: "heatmap",
            build: heatmap,
            shows: Shows::Subjects(&["Heatmap"]),
        },
        Scene {
            name: "color-picker",
            build: color_picker,
            shows: Shows::Subjects(&["ColorPicker", "ColorSwatch"]),
        },
        Scene {
            name: "code-view",
            build: code_view,
            shows: Shows::Subjects(&["CodeView"]),
        },
        Scene {
            name: "upload-list",
            build: upload_list,
            shows: Shows::Subjects(&["UploadList"]),
        },
        Scene {
            name: "cascader",
            build: cascader,
            shows: Shows::Subjects(&["Cascader"]),
        },
        Scene {
            name: "anchor-list",
            build: anchor_list,
            shows: Shows::Subjects(&["AnchorList"]),
        },
        Scene {
            name: "diagnostics-list",
            build: diagnostics_list,
            shows: Shows::Subjects(&["DiagnosticsList"]),
        },
    ];
    #[cfg(all(feature = "terminal", not(target_family = "wasm")))]
    scenes.push(Scene {
        name: "terminal",
        build: terminal,
        shows: Shows::Subjects(&["Terminal"]),
    });
    #[cfg(feature = "fixtures")]
    scenes.extend([
        Scene {
            name: "calendar",
            build: calendar,
            shows: Shows::Subjects(&["Calendar"]),
        },
        Scene {
            name: "date-range",
            build: date_range,
            shows: Shows::Subjects(&["RangePicker"]),
        },
        Scene {
            name: "date-time",
            build: date_time,
            shows: Shows::Subjects(&["DateInput", "TimeInput"]),
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
