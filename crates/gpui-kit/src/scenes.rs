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
use gpui_kit_theme::{Radius, Space, Theme};

use crate::controls::combobox::Combobox;
use crate::controls::input::TextInput;
use crate::controls::number_input::NumberInput;
use crate::controls::select::{Select, SelectOption};
use crate::controls::split_button::SplitButton;
use crate::controls::tag_input::TagInput;
use crate::controls::textarea::TextArea;
use crate::display::badge::Tone;
use crate::foundation::ActiveTheme;
use crate::overlay::toast::push as toast_push;
use crate::overlay::{Edge, Kbd, Overlay, Placement, Tooltip, Tooltipped};
use crate::prelude::*;

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
    vec![
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
    ]
}

pub fn find(name: &str) -> Option<Scene> {
    catalog().into_iter().find(|scene| scene.name == name)
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
                        .first(true)
                        .child(div().flex_1().child("Native runtime"))
                        .child(Badge::new("Ready").success()),
                )
                .child(
                    ListRow::new()
                        .id("scene.card.catalog")
                        .child(div().flex_1().child("Model catalog"))
                        .child(Badge::new("Stale").warning()),
                ),
        )
        .into_any_element()
}

fn status(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
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
    stack(&theme)
        .child(
            row(&theme)
                .child(PulseLoader::new("scene.loading.pulse").label("Loading providers"))
                .child(GradientSpinner::new("scene.loading.spinner").label("Contacting host")),
        )
        .child(
            Skeleton::new("scene.loading.skeleton")
                .rows(3)
                .label("Loading list"),
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
        .child(div().child("Content behind the dialog"))
        .child(
            Overlay::modal("scene.overlay.dialog")
                .placement(Placement::Center)
                .child(
                    crate::overlay::surface(&theme, gpui_kit_theme::Elevation::Modal)
                        .w(px(320.0))
                        .p(px(theme.spacing.lg))
                        .gap(px(theme.spacing.sm))
                        .child(div().child("Delete this workspace?"))
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
        .child(div().child("Content behind the dialog"))
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
            .target("run-a04")
            .menu(menu_items())
            .content(|_, cx| {
                let theme = cx.theme().clone();
                div()
                    .w(px(320.0))
                    .p(px(theme.spacing.md))
                    .hairline(&theme)
                    .radius(&theme, Radius::Card)
                    .child("Right-click this fixture row")
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

    cx.set_global(SceneMenus {
        menu,
        context,
        palette,
    });
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
        .child(div().child("The right-click reports the row. Nothing is selected by it."))
        .child(context)
        .into_any_element()
}

fn command_palette(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_menus(window, cx);
    let palette = cx.global::<SceneMenus>().palette.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .h(px(420.0))
        .items_center()
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
            Toast::new(
                "scene.toast.refused",
                "The host refused to publish this run",
            )
            .tone(Tone::Danger)
            .detail("Approval is required for this workspace.")
            .action("Try again", |_, _| {}),
        );
    }
    let layer = cx.global::<SceneToasts>().layer.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(560.0))
        .h(px(360.0))
        .child(div().child("Content behind the notifications"))
        // A failure keeps its report on screen; only the success times out.
        .child(div().child("A danger or warning toast stays until it is dismissed."))
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
        .child(div().child("Runs are rendered by the caller, not by the strip."))
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
                        .body(div().child("Requests go out over the system proxy.")),
                )
                .section(
                    AccordionSection::new("storage", "Storage")
                        .description("Where verified results are kept")
                        .body(div().child("Nothing is written outside the workspace.")),
                )
                .section(
                    AccordionSection::new("policy", "Managed by policy")
                        .description("This machine cannot change these")
                        .disabled(true)
                        .body(div().child("Set by the administrator.")),
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
            div()
                .type_scale(&theme, gpui_kit_theme::TypeScale::Caption)
                .text_color(theme.colors.text_muted)
                .child(SharedString::from(format!(
                    "{FIXTURE_RECORDS} fixture records; only the rendered ones publish"
                ))),
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
        Combobox::new("scene.form.region", window, cx)
            .options([
                SelectOption::new("eu-west", "Europe (Ireland)"),
                SelectOption::new("eu-north", "Europe (Stockholm)"),
                SelectOption::new("us-east", "United States (Virginia)"),
                SelectOption::new("ap-south", "Asia Pacific (Mumbai)").disabled(true),
            ])
            .selected("eu-west")
            .placeholder("Choose a region")
    });
    let labels = cx.new(|cx| {
        TagInput::new("scene.form.labels", window, cx)
            .tags(["indexing", "nightly", "verified"])
            .placeholder("Add a label")
            .max(5)
    });
    region.update(cx, |combobox, cx| {
        combobox.set_query("eu", cx);
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
        .h(px(720.0))
        .child(
            FormField::new("scene.form.name.field", "Workspace name")
                .control("scene.form.name")
                .required(true)
                // The description says what the field is for and the error
                // says what went wrong; neither answers for the other.
                .description("Shown wherever this workspace appears.")
                .error("A workspace with this name already exists.")
                .child(name),
        )
        .child(
            FormField::new("scene.form.retention.field", "Retention")
                .control("scene.form.retention")
                .required(true)
                .description("How long a finished run is kept.")
                .error("This workspace allows at most 60 days.")
                .child(retention),
        )
        .child(
            FormField::new("scene.form.visibility.field", "Visibility")
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
            FormField::new("scene.form.labels.field", "Labels")
                .control("scene.form.labels")
                .description("Enter or comma adds one; backspace targets the last.")
                .hint("enter")
                .child(labels),
        )
        .child(
            FormField::new("scene.form.region.field", "Region")
                .control("scene.form.region")
                .description("Where runs in this workspace are executed.")
                .child(region),
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
                    .placeholder("sk-...")
                    .secret(true)
            }),
            disabled: cx.new(|cx| {
                TextInput::new("scene.input.disabled", window, cx)
                    .text("read only")
                    .disabled(true)
            }),
            invalid: cx.new(|cx| {
                TextInput::new("scene.input.invalid", window, cx)
                    .text("not an email")
                    .invalid(true)
                    .required(true)
            }),
            provider: cx.new(|cx| {
                Select::new("scene.input.provider", window, cx)
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
        .child(
            div()
                .type_scale(theme, gpui_kit_theme::TypeScale::Label)
                .child(SharedString::from(title)),
        );
    for line in 1..=lines {
        pane = pane.child(
            div()
                .type_scale(theme, gpui_kit_theme::TypeScale::Caption)
                .text_color(theme.colors.text_muted)
                .child(SharedString::from(format!("Fixture line {line:02}"))),
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
        .child(div().child("The last two actions moved into the overflow menu."))
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
                div()
                    .type_scale(&theme, gpui_kit_theme::TypeScale::Caption)
                    .text_color(theme.colors.text_faint)
                    .child(if collapsed {
                        SharedString::new_static("v0")
                    } else {
                        SharedString::new_static("Fixture workspace")
                    }),
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
        .child(div().child("Content behind the drawer"))
        .child(filters)
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
