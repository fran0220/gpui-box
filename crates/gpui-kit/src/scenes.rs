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
use crate::controls::keybinding_recorder::KeybindingRecorder;
use crate::controls::number_input::NumberInput;
use crate::controls::select::{Select, SelectOption};
use crate::controls::split_button::SplitButton;
use crate::controls::tag_input::TagInput;
use crate::controls::textarea::TextArea;
use crate::display::badge::Tone;
use crate::foundation::ActiveTheme;
use crate::interaction::dnd;
use crate::overlay::toast::push as toast_push;
use crate::overlay::{Edge, Kbd, Overlay, Placement, Tooltip, Tooltipped};
use crate::prelude::*;

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

    let popover = cx.new(|cx| {
        Popover::new("scene.popover.filters", window, cx)
            .trigger("Filters")
            .content(|_, cx| {
                let theme = cx.theme().clone();
                div()
                    .column()
                    .w(px(260.0))
                    .gap(px(theme.spacing.sm))
                    .child("Anything can live in a popover.")
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
        .child(div().child("The trigger owns whether the surface is open."))
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
        .child(div().child("The right-click reports the row. Nothing is selected by it."))
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
        .type_scale(theme, gpui_kit_theme::TypeScale::Caption)
        .text_color(theme.colors.text_muted)
        .child(SharedString::from(format!("Fixture detail for {id}")))
        .child(SharedString::from(
            "Only an opened row builds this region; the rest never ask for it.",
        ))
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
            div()
                .type_scale(&theme, gpui_kit_theme::TypeScale::Caption)
                .text_color(theme.colors.text_muted)
                .child(SharedString::from(format!(
                    "{FIXTURE_JOBS_LOADED} rows loaded of {FIXTURE_JOBS_TOTAL}; only the drawn \
                     ones publish"
                ))),
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
            div()
                .type_scale(&theme, gpui_kit_theme::TypeScale::Caption)
                .text_color(theme.colors.text_muted)
                .child(SharedString::from(
                    "Escape reverts, enter commits, tab commits and moves on. The grid never \
                     writes the value.",
                )),
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
                // The keystroke lives in the hint, so the description does not
                // spend a second line repeating it.
                .description("At most five, and each one only once.")
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
                .first(index == 0)
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
            div()
                .text_size(px(theme.typography.caption.size))
                .text_color(theme.colors.text_muted)
                .child("Rows land in their new slot at once and slide into it."),
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
                        .body(div().child("Results are kept in the workspace for 30 days.")),
                ),
        )
        .child(
            div()
                .text_size(px(theme.typography.caption.size))
                .text_color(theme.colors.text_muted)
                .child("Every state settles within a fifth of a second; the values are published the moment they change."),
        )
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
                div()
                    .text_size(px(theme.typography.caption.size))
                    .text_color(theme.colors.text_muted)
                    .child(label),
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
            div()
                .text_size(px(theme.typography.caption.size))
                .text_color(theme.colors.text_muted)
                .child("The published value is the target, from the frame it changes."),
        )
        .into_any_element()
}

/// A caption naming the state a drag scene was staged in.
fn caption(theme: &Theme, text: impl Into<SharedString>) -> gpui::Div {
    div()
        .type_scale(theme, gpui_kit_theme::TypeScale::Caption)
        .text_color(theme.colors.text_muted)
        .child(text.into())
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
                        .child(SharedString::new_static(
                            "The body of the current step belongs to the caller.",
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
                                .detail(div().child(SharedString::new_static(
                                    "The host refused the request. The refusal is shown as it \
                                     arrived.",
                                ))),
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

#[cfg(feature = "fixtures")]
mod dates {
    use std::rc::Rc;

    use gpui::{AnyElement, App, Entity, Global, IntoElement, Window, div, prelude::*, px};
    use gpui_kit_theme::Space;

    use crate::datetime::fixture::FixtureDateAdapter;
    use crate::datetime::{
        Calendar, DateInput, DayMark, DayRange, RangePicker, SharedDateAdapter, TimeInput,
        TimeOfDay,
    };
    use crate::display::badge::Tone;
    use crate::foundation::ActiveTheme;

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
                div()
                    .max_w(px(560.0))
                    .text_color(theme.colors.text_muted)
                    .child(
                        "Every weekday name, month name, and blocked reason above came from the \
                         host. The calendar on the right has no today, so it draws no ring and \
                         guesses no month.",
                    ),
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
                div()
                    .max_w(px(560.0))
                    .text_color(theme.colors.text_muted)
                    .child(
                        "What the field could not read is still in it, and the refusal is the \
                         adapter's own sentence.",
                    ),
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
        .text_color(theme.colors.text)
        .text_size(px(theme.typography.label.size))
        .child(SharedString::new_static(label))
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
