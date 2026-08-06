mod recipes;

use std::env;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Result, bail};
use gpui::{
    App, Bounds, Context, Entity, Global, IntoElement, Render, SharedString, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, size,
};
use gpui_kit::assets::{Icon, icon};
use gpui_kit::overlay::{popover, toast};
use gpui_kit::prelude::*;
use gpui_kit_semantics::{NodeSpec, Role, Semantic, SemanticRegistry};
use gpui_kit_theme::{Theme, ThemeRegistry, activate_theme, set_density};
use serde::Deserialize;

const SETTINGS_FIXTURE: &str = include_str!("../../../fixtures/settings/states.json");

#[derive(Debug, Deserialize)]
struct SettingsFixture {
    fixture: bool,
    rows: Vec<SettingsRow>,
}

#[derive(Debug, Deserialize)]
struct SettingsRow {
    id: String,
    title: String,
    detail: String,
    state: FixtureState,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum FixtureState {
    Ready,
    Neutral,
    Stale,
}

struct Gallery {
    lower_scene: bool,
    /// A single component scene from the library catalog, for review and
    /// capture in isolation.
    scene: Option<&'static str>,
    search: Entity<TextInput>,
    provider: Entity<Select>,
    token: Entity<TextInput>,
    rejected: Entity<TextInput>,
    notes: Entity<TextArea>,
    workspace: Entity<TextInput>,
    retention: Entity<NumberInput>,
    region: Entity<Combobox>,
    labels: Entity<TagInput>,
    publish: Entity<SplitButton>,
    confirm: Entity<Dialog>,
    toasts: Entity<ToastLayer>,
    filters: Entity<Popover>,
    menu: Entity<Menu>,
    records: Entity<ContextMenu>,
    palette: Entity<CommandPalette>,
    overflow: Entity<Menu>,
    page_size: Entity<Select>,
    filter_drawer: Entity<Drawer>,
    /// The order the host holds. A reorder only shows once this changes,
    /// because the library reports the intent and moves nothing itself.
    queue: Vec<(SharedString, SharedString)>,
    /// What the host attached, in the order it accepted the drops.
    attached: Vec<SharedString>,
}

fn gallery_queue() -> Vec<(SharedString, SharedString)> {
    [
        ("step-clone", "Clone repository"),
        ("step-restore", "Restore dependencies"),
        ("step-build", "Build workspace"),
        ("step-test", "Run tests"),
        ("step-publish", "Publish artifacts"),
    ]
    .into_iter()
    .map(|(id, label)| {
        (
            SharedString::new_static(id),
            SharedString::new_static(label),
        )
    })
    .collect()
}

impl Gallery {
    /// Applies a reported move to the order the host owns.
    ///
    /// This is the host's half of the contract, written out so the gallery
    /// shows a real reorder rather than a component pretending to do one.
    fn apply_move(&mut self, intent: &DropIntent) {
        let Some(from) = self.queue.iter().position(|(id, _)| id == &intent.item.id) else {
            return;
        };
        let carried = self.queue.remove(from);
        let anchor = self
            .queue
            .iter()
            .position(|(id, _)| id == intent.position.anchor());
        let at = match (&intent.position, anchor) {
            (DropPosition::Before(_), Some(anchor)) => anchor,
            (DropPosition::After(_), Some(anchor)) => anchor + 1,
            _ => self.queue.len(),
        };
        self.queue.insert(at, carried);
    }

    fn interaction_section(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        let queue = self.queue.clone();
        let handle = cx.entity();
        let attaching = handle.clone();
        let attached = if self.attached.is_empty() {
            SharedString::new_static("Nothing attached yet.")
        } else {
            SharedString::from(format!("Attached: {}", self.attached.join(", ")))
        };

        div()
            .flex()
            .flex_col()
            .gap(px(theme.spacing.md))
            .w(px(620.0))
            .child(recipes::footnote(
                theme,
                "A drop reports where the item should go. The order below only changes \
                 because this window applied the report to the data it owns; escape \
                 abandons a drag and reports nothing.",
            ))
            .child(
                div()
                    .border(px(theme.borders.hairline))
                    .border_color(theme.colors.hairline)
                    .rounded(px(theme.radii.card))
                    .overflow_hidden()
                    .child(
                        List::new("gallery.queue", queue.len(), {
                            let queue = queue.clone();
                            move |index, _, _| {
                                let (id, label) = queue[index].clone();
                                ListItem::new(id, label.clone()).text(label)
                            }
                        })
                        .reorderable(true)
                        .on_select(|_, _, _| {})
                        .on_reorder(move |intent, _, cx| {
                            let intent = intent.clone();
                            handle.update(cx, |gallery, cx| {
                                gallery.apply_move(&intent);
                                cx.notify();
                            });
                        }),
                    ),
            )
            .child(
                Dropzone::new("gallery.attachments", "Drop a step to attach it")
                    .hint("Steps only")
                    .refusal("Only a build step can be attached.")
                    .accepts([gpui_kit::interaction::ROW_KIND])
                    .icon(Icon::Paperclip)
                    .on_drop(move |item, _, cx| {
                        let label = item.label.clone();
                        attaching.update(cx, |gallery, cx| {
                            gallery.attached.push(label);
                            cx.notify();
                        });
                    }),
            )
            .child(recipes::footnote(theme, attached))
            .into_any_element()
    }
}

fn gallery_menu_items() -> Vec<MenuItem> {
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

fn gallery_commands() -> Vec<Command> {
    vec![
        Command::new("workspace.open", "Open workspace")
            .section("Workspace")
            .shortcut("cmd-o"),
        Command::new("workspace.close", "Close workspace").section("Workspace"),
        Command::new("workspace.publish", "Publish workspace")
            .section("Workspace")
            .unavailable("Approval is required"),
        Command::new("editor.wrap", "Toggle word wrap").section("Editor"),
        Command::new("editor.split", "Split editor")
            .section("Editor")
            .shortcut("cmd-\\"),
    ]
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SemanticRegistry::global(cx).begin_frame();
        let theme = Theme::get(cx).clone();
        if let Some(name) = self.scene {
            let scene = gpui_kit::scenes::find(name).expect("scene is registered");
            return div()
                .size_full()
                .bg(theme.colors.canvas)
                .child((scene.build)(window, cx))
                .into_any_element();
        }
        if self.lower_scene {
            return lower_gallery(&theme, cx).into_any_element();
        }

        div()
            .id("gallery-root")
            .size_full()
            .overflow_y_scroll()
            .bg(theme.colors.canvas)
            .font_family(theme.typography.sans.clone())
            .text_color(theme.colors.text)
            .semantic_in(
                cx,
                NodeSpec::new("gallery.root", Role::Window).text("gpui-kit gallery"),
            )
            .child(
                recipes::page(&theme)
                    .child(recipes::page_header(&theme, "gpui-kit", Some(24)))
                    .child(recipes::subtitle(
                        &theme,
                        "A truthful desktop UI design system, semantic tree, and test kit.",
                    ))
                    .child(theme_switcher(&theme, cx))
                    .child(recipes::section_title(&theme, "Actions"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(theme.spacing.sm))
                            .child(
                                Button::new("gallery.primary")
                                    .label("Primary action")
                                    .primary()
                                    .on_click(|_, _| {}),
                            )
                            .child(
                                Button::new("gallery.secondary")
                                    .label("Secondary")
                                    .secondary()
                                    .icon(Icon::Copy)
                                    .on_click(|_, _| {}),
                            )
                            .child(
                                Button::new("gallery.ghost")
                                    .label("Ghost action")
                                    .ghost()
                                    .on_click(|_, _| {}),
                            )
                            .child(
                                Button::new("gallery.danger")
                                    .label("Destructive")
                                    .danger()
                                    .on_click(|_, _| {}),
                            )
                            .child(
                                Button::new("gallery.disabled")
                                    .label("Unavailable")
                                    .primary()
                                    .disabled(true)
                                    .on_click(|_, _| {}),
                            )
                            .child(
                                Button::new("gallery.loading")
                                    .label("Saving")
                                    .primary()
                                    .loading(true)
                                    .on_click(|_, _| {}),
                            ),
                    )
                    .child(recipes::section_title(&theme, "Icons, groups, and split actions"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .items_start()
                            .gap(px(theme.spacing.sm))
                            .child(
                                IconButton::new("gallery.copy", Icon::Copy, "Copy run id")
                                    .on_click(|_, _| {}),
                            )
                            .child(
                                IconButton::new("gallery.rename", Icon::Pen, "Rename run")
                                    .secondary()
                                    .on_click(|_, _| {}),
                            )
                            .child(
                                IconButton::new("gallery.remove", Icon::Trash, "Delete run")
                                    .danger()
                                    .on_click(|_, _| {}),
                            )
                            .child(
                                IconButton::new("gallery.archive", Icon::Archive, "Archive run")
                                    .secondary()
                                    .disabled(true)
                                    .on_click(|_, _| {}),
                            )
                            .child(
                                ButtonGroup::new("gallery.range")
                                    .children([
                                        Button::new("gallery.range.day")
                                            .label("Day")
                                            .secondary()
                                            .on_click(|_, _| {}),
                                        Button::new("gallery.range.week")
                                            .label("Week")
                                            .secondary()
                                            .selected(true)
                                            .on_click(|_, _| {}),
                                        Button::new("gallery.range.month")
                                            .label("Month")
                                            .secondary()
                                            .on_click(|_, _| {}),
                                    ]),
                            )
                            .child(self.publish.clone()),
                    )
                    .child(recipes::section_title(&theme, "Control sizes"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(theme.spacing.sm))
                            .child(Button::new("gallery.size.xs").label("Extra small").xs())
                            .child(Button::new("gallery.size.sm").label("Small").small())
                            .child(Button::new("gallery.size.md").label("Medium").medium())
                            .child(Button::new("gallery.size.lg").label("Large").large()),
                    )
                    .child(recipes::section_title(&theme, "Editing"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(theme.spacing.sm))
                            .w(px(360.0))
                            .child(self.search.clone())
                            .child(self.token.clone())
                            .child(self.rejected.clone())
                            .child(self.provider.clone())
                            .child(self.notes.clone()),
                    )
                    .child(recipes::section_title(&theme, "Form fields"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(theme.spacing.md))
                            .w(px(360.0))
                            .child(
                                FormField::new("gallery.workspace.field", "Workspace name")
                                    .control("gallery.workspace")
                                    .required(true)
                                    .description("Shown wherever this workspace appears.")
                                    .error("A workspace with this name already exists.")
                                    .child(self.workspace.clone()),
                            )
                            .child(
                                FormField::new("gallery.retention.field", "Retention")
                                    .control("gallery.retention")
                                    .description("How long a finished run is kept.")
                                    .child(self.retention.clone()),
                            )
                            .child(
                                FormField::new("gallery.visibility.field", "Visibility")
                                    .control("gallery.visibility")
                                    .description("Who can open the runs in this workspace.")
                                    .child(
                                        SegmentedControl::new("gallery.visibility")
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
                                FormField::new("gallery.labels.field", "Labels")
                                    .control("gallery.labels")
                                    .description("Enter or comma adds one.")
                                    .hint("enter")
                                    .child(self.labels.clone()),
                            )
                            .child(
                                FormField::new("gallery.region.field", "Region")
                                    .control("gallery.region")
                                    .description("Where runs in this workspace are executed.")
                                    .child(self.region.clone()),
                            ),
                    )
                    .child(recipes::section_title(&theme, "Choices"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(theme.spacing.sm))
                            .w(px(360.0))
                            .child(
                                Checkbox::new("gallery.telemetry")
                                    .label("Send anonymous usage data")
                                    .description("Counts only, never file contents")
                                    .checked(true)
                                    .on_change(|_, _, _| {}),
                            )
                            .child(
                                Checkbox::new("gallery.partial")
                                    .label("Some providers enabled")
                                    .mixed()
                                    .on_change(|_, _, _| {}),
                            )
                            .child(
                                Radio::new("gallery.mode.ask")
                                    .label("Ask before every action")
                                    .selected(true)
                                    .on_select(|_, _| {}),
                            )
                            .child(
                                Radio::new("gallery.mode.auto")
                                    .label("Run without asking")
                                    .on_select(|_, _| {}),
                            )
                            .child(
                                Switch::new("gallery.preview")
                                    .label("Preview releases")
                                    .on(true)
                                    .on_change(|_, _, _| {}),
                            )
                            .child(
                                Slider::new("gallery.temperature")
                                    .label("Temperature")
                                    .range(0.0, 2.0)
                                    .step(0.1)
                                    .value(0.7)
                                    .display("0.7")
                                    .on_change(|_, _, _| {}),
                            ),
                    )
                    .child(recipes::section_title(&theme, "Navigation"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(theme.spacing.md))
                            .w(px(520.0))
                            .child(
                                Breadcrumb::new("gallery.trail")
                                    .crumbs([
                                        Crumb::new("workspace", "Workspace"),
                                        Crumb::new("projects", "Projects"),
                                        Crumb::new("gpui-kit", "gpui-kit"),
                                        Crumb::new("runs", "Runs"),
                                        Crumb::new("indexing", "Indexing"),
                                    ])
                                    .max_visible(3)
                                    .on_select(|_, _, _| {})
                                    .on_reveal(|_, _, _| {}),
                            )
                            .child(
                                Tabs::new("gallery.tabs")
                                    .tabs([
                                        TabItem::new("overview", "Overview").icon(Icon::Widget),
                                        TabItem::new("runs", "Runs").badge("12"),
                                        TabItem::new("logs", "Logs"),
                                        TabItem::new("billing", "Billing").disabled(true),
                                    ])
                                    .selected("runs")
                                    .on_select(|_, _, _| {}),
                            )
                            // The body is the caller's: the strip reports the
                            // tab that was picked and nothing else.
                            .child(div().child("Runs are rendered by the caller."))
                            .child(
                                Accordion::new("gallery.sections")
                                    .expanded_ids(&["network"])
                                    .on_toggle(|_, _, _, _| {})
                                    .section(
                                        AccordionSection::new("network", "Network")
                                            .description("How this machine reaches a host")
                                            .body(div().child(
                                                "Requests go out over the system proxy.",
                                            )),
                                    )
                                    .section(
                                        AccordionSection::new("storage", "Storage")
                                            .description("Where verified results are kept")
                                            .body(div().child(
                                                "Nothing is written outside the workspace.",
                                            )),
                                    )
                                    .section(
                                        AccordionSection::new("policy", "Managed by policy")
                                            .description("This machine cannot change these")
                                            .disabled(true)
                                            .body(div().child("Set by the administrator.")),
                                    ),
                            ),
                    )
                    .child(recipes::section_title(&theme, "Data"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(theme.spacing.md))
                            .w(px(620.0))
                            .child(
                                Table::new("gallery.runs")
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
                                    .rows(fixture_runs())
                                    .visible_rows(6)
                                    .on_sort(|_, _, _, _| {})
                                    .on_select(|_, _, _| {}),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(theme.spacing.md))
                                    .child(
                                        div()
                                            .w(px(300.0))
                                            .rounded(px(theme.radii.card))
                                            .border(px(theme.borders.hairline))
                                            .border_color(theme.colors.hairline)
                                            .overflow_hidden()
                                            .child(
                                                List::new(
                                                    "gallery.records",
                                                    FIXTURE_RECORDS,
                                                    |index, _, _| {
                                                        let label = SharedString::from(format!(
                                                            "Fixture record {index:04}"
                                                        ));
                                                        ListItem::new(
                                                            format!("record-{index:04}"),
                                                            label.clone(),
                                                        )
                                                        .text(label)
                                                    },
                                                )
                                                .selected("record-0002")
                                                .visible_rows(8)
                                                .on_select(|_, _, _| {}),
                                            ),
                                    )
                                    .child(
                                        div().w(px(300.0)).child(
                                            Tree::new("gallery.tree")
                                                .expanded_ids(&["workspace", "crates"])
                                                .selected("tokens")
                                                .nodes(fixture_tree())
                                                .on_toggle(|_, _, _, _| {})
                                                .on_select(|_, _, _| {}),
                                        ),
                                    ),
                            )
                            .child(recipes::footnote(
                                &theme,
                                "The list holds a viewport, not a data set: it publishes the total \
                                 and only the rows it drew.",
                            )),
                    )
                    .child(recipes::section_title(&theme, "Data grid"))
                    .child(data_grid_section(&theme, window, cx))
                    .child(recipes::section_title(&theme, "Status"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(theme.spacing.sm))
                            .child(Badge::new("Neutral").neutral())
                            .child(Badge::new("Accent").accent())
                            .child(Badge::new("Success").success())
                            .child(Badge::new("Warning").warning())
                            .child(Badge::new("Danger").danger())
                            .child(Badge::new("Info").info()),
                    )
                    .child(recipes::section_title(&theme, "Content"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(theme.spacing.sm))
                            .w(px(420.0))
                            .child(
                                ProgressBar::new("gallery.index")
                                    .label("Indexing workspace")
                                    .count(3, 12),
                            )
                            .child(
                                ProgressBar::new("gallery.contact").label("Contacting host"),
                            )
                            .child(Divider::new().id("gallery.rule").label("Filters"))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .flex_wrap()
                                    .gap(px(theme.spacing.xs))
                                    .child(
                                        Tag::new("gallery.tag.rust", "rust")
                                            .on_remove(|_, _| {}),
                                    )
                                    .child(
                                        Tag::new("gallery.tag.failing", "failing")
                                            .tone(Tone::Danger)
                                            .on_remove(|_, _| {}),
                                    )
                                    .child(
                                        Tag::new("gallery.tag.pinned", "pinned").disabled(true),
                                    )
                                    .child(Avatar::new("Ada Lovelace").id("gallery.avatar")),
                            )
                            .child(
                                EmptyState::new("gallery.refused", "The host refused the request")
                                    .kind(EmptyKind::Unavailable)
                                    .detail("Approval is required for this workspace.")
                                    .action(
                                        Button::new("gallery.retry")
                                            .label("Try again")
                                            .secondary()
                                            .on_click(|_, _| {}),
                                    ),
                            ),
                    )
                    .child(recipes::section_title(&theme, "Settings pattern"))
                    .child(fixture_settings_card(&theme))
                    .child(recipes::section_title(&theme, "Truthful states"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(theme.spacing.xl))
                            .mb(px(theme.spacing.sm))
                            .child(StatusLine::new("Connected", Tone::Success))
                            .child(StatusLine::new("Reconnecting", Tone::Warning))
                            .child(StatusLine::new("Refused", Tone::Danger))
                            .child(StatusDot::new(Tone::Neutral))
                            .child(Kbd::new("cmd-shift-p"))
                            .child(Kbd::new("enter")),
                    )
                    .child(Callout::new(
                        "The host refused this action. Preserve its exact reason instead of showing an empty state.",
                        Tone::Danger,
                    ).id("gallery.callout.refusal"))
                    .child(
                        div()
                            .mt(px(theme.spacing.sm))
                            .child(Callout::new(
                                "Refreshing failed. The last verified model catalog remains visible.",
                                Tone::Warning,
                            ).id("gallery.callout.stale")),
                    )
                    .child(recipes::section_title(&theme, "Motion and loading"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(theme.spacing.xl))
                            .child(PulseLoader::new("gallery.pulse"))
                            .child(GradientSpinner::new("gallery.gradient"))
                            .child(
                                div()
                                    .w(px(220.0))
                                    .child(Skeleton::new("gallery.skeleton").rows(3)),
                            ),
                    )
                    .child(recipes::section_title(&theme, "Interaction"))
                    .child(self.interaction_section(&theme, cx))
                    .child(recipes::section_title(&theme, "Motion"))
                    .child(motion_section(&theme, window, cx))
                    .child(recipes::section_title(&theme, "Popover primitives"))
                    .child(menu_sample(&theme, 320.0))
                    .child(recipes::section_title(&theme, "Menus"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap(px(theme.spacing.sm))
                            .child(self.filters.clone())
                            .child(self.menu.clone()),
                    )
                    .child(recipes::footnote(
                        &theme,
                        "A checkable row reports the intent to change it. The host owns the \
                         answer, so the check only moves when the host moves it.",
                    ))
                    .child(self.records.clone())
                    .child(recipes::section_title(&theme, "Command palette"))
                    .child(self.palette.clone())
                    .child(recipes::footnote(
                        &theme,
                        "A command the host refused stays listed with its reason: hiding a \
                         command a typist knows exists is a lie about the application.",
                    ))
                    .child(recipes::section_title(&theme, "Dialog and hover help"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(theme.spacing.sm))
                            .child({
                                let confirm = self.confirm.clone();
                                Button::new("gallery.confirm.open")
                                    .label("Delete workspace")
                                    .danger()
                                    .on_click(move |window, cx| {
                                        confirm.update(cx, |dialog, cx| dialog.open(window, cx));
                                    })
                            })
                            .child(
                                div()
                                    .id("gallery.export.host")
                                    .tip("gallery.export", "Writes the theme to a file on disk")
                                    .child(
                                        Button::new("gallery.export")
                                            .label("Export theme")
                                            .secondary()
                                            .on_click(|_, _| {}),
                                    ),
                            )
                            .child(
                                Tooltip::new(
                                    "gallery.export.sample",
                                    "Writes the theme to a file on disk",
                                )
                                .describes("gallery.export"),
                            ),
                    )
                    .child(recipes::section_title(&theme, "Notifications"))
                    .child(toast_buttons(&theme))
                    .child(recipes::footnote(
                        &theme,
                        "A success times out. A warning or a danger notification stays until it \
                         is dismissed, and a pointer resting on one pauses its timer.",
                    ))
                    .child(recipes::section_title(&theme, "Toolbar"))
                    .child(
                        Toolbar::new("gallery.toolbar")
                            .label("Editor actions")
                            .group(
                                "history",
                                [
                                    ToolbarItem::new(
                                        "editor.undo",
                                        "Undo",
                                        IconButton::new(
                                            "gallery.toolbar.undo",
                                            Icon::ArrowLeft,
                                            "Undo",
                                        )
                                        .ghost()
                                        .small()
                                        .on_click(|_, _| {}),
                                    )
                                    .icon(Icon::ArrowLeft)
                                    .shortcut("cmd-z"),
                                    ToolbarItem::new(
                                        "editor.redo",
                                        "Redo",
                                        IconButton::new(
                                            "gallery.toolbar.redo",
                                            Icon::ArrowRight,
                                            "Redo",
                                        )
                                        .ghost()
                                        .small()
                                        .on_click(|_, _| {}),
                                    )
                                    .icon(Icon::ArrowRight),
                                ],
                            )
                            .spacer()
                            .group(
                                "publish",
                                [
                                    ToolbarItem::new(
                                        "editor.share",
                                        "Share",
                                        Button::new("gallery.toolbar.share")
                                            .label("Share")
                                            .secondary()
                                            .small()
                                            .on_click(|_, _| {}),
                                    )
                                    .icon(Icon::Copy),
                                    ToolbarItem::new(
                                        "editor.publish",
                                        "Publish",
                                        Button::new("gallery.toolbar.publish")
                                            .label("Publish")
                                            .primary()
                                            .small()
                                            .on_click(|_, _| {}),
                                    )
                                    .icon(Icon::ArchiveUp),
                                    ToolbarItem::new(
                                        "editor.archive",
                                        "Archive",
                                        Button::new("gallery.toolbar.archive")
                                            .label("Archive")
                                            .secondary()
                                            .small()
                                            .on_click(|_, _| {}),
                                    )
                                    .icon(Icon::Archive)
                                    .disabled(true),
                                ],
                            )
                            .overflow_after(3)
                            .overflow_menu(self.overflow.clone()),
                    )
                    .child(recipes::footnote(
                        &theme,
                        "An action past the declared cut is moved into the overflow menu, never \
                         dropped: it keeps its identity, its label, and its refusal.",
                    ))
                    .child(recipes::section_title(&theme, "Split and scroll"))
                    .child(
                        div()
                            .h(px(240.0))
                            .rounded(px(theme.radii.card))
                            .border(px(theme.borders.hairline))
                            .border_color(theme.colors.hairline)
                            .overflow_hidden()
                            .child(
                                SplitPane::new("gallery.split")
                                    .horizontal()
                                    .ratio(0.35)
                                    .min_sizes(140.0, 220.0)
                                    .collapsible(true)
                                    .handle_label("Resize the run list")
                                    .start(
                                        ScrollArea::new("gallery.split.runs")
                                            .label("Runs")
                                            .vertical()
                                            .child(fixture_lines(&theme, "Run", 24)),
                                    )
                                    .end(
                                        ScrollArea::new("gallery.split.detail")
                                            .label("Run detail")
                                            .vertical()
                                            .child(fixture_lines(&theme, "Detail line", 6)),
                                    )
                                    .on_resize(|_, _, _| {})
                                    .on_collapse(|_, _, _| {}),
                            ),
                    )
                    .child(recipes::footnote(
                        &theme,
                        "The right pane fits its content, so it publishes no scrollbar at all. \
                         An absent scrollbar is how a reader learns there is nothing more.",
                    ))
                    .child(recipes::section_title(&theme, "Sidebar"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .h(px(320.0))
                            .gap(px(theme.spacing.lg))
                            .child(
                                Sidebar::new("gallery.rail")
                                    .sections(gallery_places())
                                    .active("runs.active")
                                    .footer(recipes::footnote(&theme, "Fixture workspace"))
                                    .on_select(|_, _, _| {}),
                            )
                            .child(
                                Sidebar::new("gallery.rail.collapsed")
                                    .sections(gallery_places())
                                    .active("runs.active")
                                    .collapsed(true)
                                    .on_select(|_, _, _| {}),
                            ),
                    )
                    .child(recipes::section_title(&theme, "Pagination"))
                    .child(
                        Pagination::new("gallery.pages")
                            .page(9)
                            .total_pages(20)
                            .page_size(self.page_size.clone())
                            .on_select(|_, _, _| {}),
                    )
                    .child(
                        Pagination::new("gallery.pages.unknown")
                            .page(3)
                            .unknown_total(true)
                            .on_select(|_, _, _| {}),
                    )
                    .child(recipes::footnote(
                        &theme,
                        "A host that can only say whether one more page exists says exactly \
                         that: no last page, no numbers, and no total in the copy.",
                    ))
                    .child(recipes::section_title(&theme, "Drawer"))
                    .child({
                        let drawer = self.filter_drawer.clone();
                        Button::new("gallery.drawer.open")
                            .label("Filter runs")
                            .secondary()
                            .on_click(move |window, cx| {
                                drawer.update(cx, |drawer, cx| drawer.open(window, cx));
                            })
                    })
                    .child(self.confirm.clone())
                    .child(self.filter_drawer.clone())
                    .child(self.toasts.clone())
                    .child(recipes::footnote(
                        &theme,
                        "Fixture data is explicitly labeled. Product applications must render host-backed facts.",
                    )),
            )
            .into_any_element()
    }
}

/// Pushes one of each kind of notification, so the timing rules can be watched
/// rather than read about.
fn toast_buttons(theme: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(theme.spacing.sm))
        .child(
            Button::new("gallery.toast.saved")
                .label("Report a success")
                .secondary()
                .on_click(|_, cx| {
                    toast::push(
                        cx,
                        Toast::new("gallery.toast.saved.note", "Theme exported to disk")
                            .tone(Tone::Success),
                    );
                }),
        )
        .child(
            Button::new("gallery.toast.stale")
                .label("Report a stale refresh")
                .secondary()
                .on_click(|_, cx| {
                    toast::push(
                        cx,
                        Toast::new(
                            "gallery.toast.stale.note",
                            "Refreshing the model catalog failed",
                        )
                        .tone(Tone::Warning)
                        .detail("The last verified catalog is still shown."),
                    );
                }),
        )
        .child(
            Button::new("gallery.toast.refused")
                .label("Report a refusal")
                .danger()
                .on_click(|_, cx| {
                    toast::push(
                        cx,
                        Toast::new(
                            "gallery.toast.refused.note",
                            "The host refused to publish this run",
                        )
                        .tone(Tone::Danger)
                        .detail("Approval is required for this workspace.")
                        .action("Try again", |_, _| {}),
                    );
                }),
        )
        .child(
            Button::new("gallery.toast.pinned")
                .label("Report something that stays")
                .ghost()
                .on_click(|_, cx| {
                    toast::push(
                        cx,
                        Toast::new("gallery.toast.pinned.note", "Indexing is still running")
                            .tone(Tone::Info)
                            .persistent()
                            .dismissable(false),
                    );
                }),
        )
        .child(
            Button::new("gallery.toast.clear")
                .label("Clear")
                .ghost()
                .on_click(|_, cx| toast::clear(cx)),
        )
}

fn menu_sample(theme: &Theme, width: f32) -> gpui::Div {
    popover::card(theme)
        .w(px(width))
        .child(popover::heading(theme, "Recent"))
        .child(
            popover::menu_row(theme, true, false)
                .child(icon(Icon::Folder).size(px(15.0)))
                .child(SharedString::from("gpui-kit"))
                .child(div().flex_1())
                .child(popover::key_cap(theme, "↵")),
        )
        .child(
            popover::menu_row(theme, false, true)
                .child(icon(Icon::GitBranch).size(px(15.0)))
                .child(SharedString::from("feature/design-system")),
        )
        .child(popover::separator(theme))
        .child(
            popover::menu_row(theme, false, false)
                .child(icon(Icon::Settings).size(px(15.0)))
                .child(SharedString::from("Settings")),
        )
}

/// How many records the list fixture claims to hold.
const FIXTURE_RECORDS: usize = 240;

/// Synthetic rows. Nothing here stands for a product; the identities are
/// fixture keys.
/// Fixture copy, long enough for a scroll area to have something to scroll.
fn fixture_lines(theme: &Theme, title: &str, lines: usize) -> gpui::Div {
    let mut pane = div()
        .flex()
        .flex_col()
        .gap(px(theme.spacing.xs))
        .p(px(theme.spacing.md));
    for line in 1..=lines {
        pane = pane.child(
            div()
                .text_color(theme.colors.text_muted)
                .child(SharedString::from(format!("{title} {line:02}"))),
        );
    }
    pane
}

fn gallery_places() -> Vec<SidebarSection> {
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

fn fixture_runs() -> Vec<Row> {
    [
        ("run-a04", "Indexing", "Ready", Tone::Success, "4m 12s"),
        ("run-b12", "Verifying", "Stale", Tone::Warning, "2m 08s"),
        ("run-c31", "Publishing", "Refused", Tone::Danger, "1m 44s"),
        ("run-d02", "Archiving", "Managed", Tone::Neutral, "0m 51s"),
    ]
    .into_iter()
    .map(|(id, name, state, tone, duration)| {
        Row::new(id)
            .text(name)
            .cell("name", name)
            .cell(
                "state",
                Cell::new(Badge::new(state).tone(tone))
                    .text(state)
                    .published(true),
            )
            .cell("duration", duration)
    })
    .collect()
}

fn fixture_tree() -> Vec<TreeNode> {
    vec![
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
                    .children([TreeNode::new("components", "components.md").icon(Icon::Document)]),
            ]),
        TreeNode::new("target", "target")
            .icon(Icon::Archive)
            .disabled(true)
            .children([TreeNode::new("debug", "debug").icon(Icon::Folder)]),
    ]
}

fn fixture_settings_card(theme: &Theme) -> Card {
    let fixture = settings_fixture();
    assert!(
        fixture.fixture,
        "gallery data must identify itself as fixture"
    );
    let mut card = Card::new();
    for (index, row) in fixture.rows.iter().enumerate() {
        let (glyph, label, tone) = match row.state {
            FixtureState::Ready => (Icon::Monitor, "Ready", Tone::Success),
            FixtureState::Stale => (Icon::Global, "Stale", Tone::Warning),
            FixtureState::Neutral => (Icon::Key, "Approval", Tone::Neutral),
        };
        card = card.child(
            ListRow::new()
                .id(format!("fixture.settings.{}", row.id))
                .first(index == 0)
                .child(recipes::identity_tile(theme, glyph))
                .child(recipes::row_content(
                    theme,
                    row.title.clone(),
                    row.detail.clone(),
                ))
                .child(Badge::new(label).tone(tone)),
        );
    }
    card
}

fn settings_fixture() -> &'static SettingsFixture {
    static FIXTURE: OnceLock<SettingsFixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        serde_json::from_str(SETTINGS_FIXTURE).expect("settings fixture must be valid JSON")
    })
}

/// Exercises runtime theme and density switching, which only a live window can
/// demonstrate.
fn theme_switcher(theme: &Theme, cx: &App) -> impl IntoElement {
    let active = theme.id.clone();
    let density = theme.density;
    div()
        .flex()
        .flex_row()
        .items_center()
        .flex_wrap()
        .gap(px(theme.spacing.sm))
        .children(ThemeRegistry::global(cx).ids().into_iter().map(|id| {
            let selected = id == active;
            let target = id.clone();
            Button::new(Ident::new("gallery.theme").child(id.as_ref()))
                .label(id.clone())
                .secondary()
                .selected(selected)
                .on_click(move |_, cx| {
                    activate_theme(target.as_ref(), cx);
                })
        }))
        .children(
            [
                ("Compact", Density::Compact),
                ("Comfortable", Density::Comfortable),
            ]
            .map(|(label, option)| {
                Button::new(Ident::new("gallery.density").child(label))
                    .label(label)
                    .ghost()
                    .selected(density == option)
                    .on_click(move |_, cx| set_density(option, cx))
            }),
        )
}

/// The order the reorder demonstration is currently showing.
#[derive(Debug)]
struct GalleryQueue {
    steps: Vec<(&'static str, &'static str)>,
}

impl Global for GalleryQueue {}

/// The counts the animated readouts are currently showing.
#[derive(Debug)]
struct GalleryCounts {
    runs: f64,
    seconds: f64,
}

impl Global for GalleryCounts {}

/// Which way every state in the state-transition row is currently pointing.
///
/// One flag drives all of them, so the row can be flipped at once and every
/// transition watched against the same frame.
#[derive(Debug)]
struct GalleryStates {
    forward: bool,
}

impl Global for GalleryStates {}

/// Reordering, the two pointer responses, and a counting readout.
/// The grid, shown through the scenes the capture task and the audit use, so
/// the arrangement reviewed here is the arrangement that is tested.
fn data_grid_section(theme: &Theme, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
    let scene = |name: &str, window: &mut Window, cx: &mut App| {
        gpui_kit::scenes::find(name).map(|scene| (scene.build)(window, cx))
    };
    div()
        .flex()
        .flex_col()
        .gap(px(theme.spacing.md))
        .children(scene("data-grid", window, cx))
        .children(scene("data-grid-editing", window, cx))
        .child(recipes::footnote(
            theme,
            "Widths, order, sort, selection, expansion, and the value in an open cell are all \
             host state. The grid reports what was operated and draws what it was handed back.",
        ))
        .into_any_element()
}

fn motion_section(theme: &Theme, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
    if !cx.has_global::<GalleryQueue>() {
        cx.set_global(GalleryQueue {
            steps: vec![
                ("render", "Render frames"),
                ("upload", "Upload artifacts"),
                ("verify", "Verify checksums"),
                ("publish", "Publish release"),
            ],
        });
    }
    if !cx.has_global::<GalleryCounts>() {
        cx.set_global(GalleryCounts {
            runs: 1204.0,
            seconds: 18.4,
        });
    }
    if !cx.has_global::<GalleryStates>() {
        cx.set_global(GalleryStates { forward: false });
    }
    let forward = cx.global::<GalleryStates>().forward;
    let steps = cx.global::<GalleryQueue>().steps.clone();
    let counts = cx.global::<GalleryCounts>();
    let (runs, seconds) = (counts.runs, counts.seconds);

    let mut queue = Card::new().id("gallery.motion.queue");
    for (index, (id, label)) in steps.iter().enumerate() {
        let ident = format!("gallery.motion.{id}");
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

    let tile = |id: &'static str, label: &'static str| {
        div()
            .id(id)
            .w(px(150.0))
            .p(px(theme.spacing.md))
            .rounded(px(theme.radii.card))
            .border(px(theme.borders.hairline))
            .border_color(theme.colors.hairline)
            .bg(theme.colors.raised)
            .cursor_pointer()
            .hover_lift(cx)
            .pressable(cx)
            .child(label)
    };

    div()
        .flex()
        .flex_col()
        .gap(px(theme.spacing.lg))
        .child(
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(theme.spacing.lg))
                .child(div().w(px(320.0)).child(queue))
                .child(
                    Button::new("gallery.motion.reorder")
                        .label("Move the last step first")
                        .secondary()
                        .on_click(|_, cx| {
                            cx.update_global::<GalleryQueue, ()>(|queue, _| {
                                queue.steps.rotate_right(1)
                            });
                            cx.refresh_windows();
                        }),
                ),
        )
        .child(state_transitions(theme, forward))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(theme.spacing.md))
                .child(tile("gallery.motion.lift", "Hover to lift"))
                .child(tile("gallery.motion.press", "Hold to press"))
                .child(
                    Button::new("gallery.motion.pressable")
                        .label("Buttons sink when held")
                        .primary()
                        .on_click(|_, _| {}),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_end()
                .gap(px(theme.spacing.xl))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(theme.spacing.xs))
                        .child(
                            div()
                                .text_size(px(theme.typography.caption.size))
                                .text_color(theme.colors.text_muted)
                                .child("Runs this week"),
                        )
                        .child(AnimatedNumber::new("gallery.motion.runs", runs).format(grouped)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(theme.spacing.xs))
                        .child(
                            div()
                                .text_size(px(theme.typography.caption.size))
                                .text_color(theme.colors.text_muted)
                                .child("Median duration"),
                        )
                        .child(
                            AnimatedNumber::new("gallery.motion.seconds", seconds)
                                .format(|value| format!("{value:.1}s")),
                        ),
                )
                .child(
                    Button::new("gallery.motion.recount")
                        .label("Recount")
                        .secondary()
                        .on_click(|_, cx| {
                            cx.update_global::<GalleryCounts, ()>(|counts, _| {
                                counts.runs += 318.0;
                                counts.seconds += 4.7;
                            });
                            cx.refresh_windows();
                        }),
                ),
        )
        .into_any_element()
}

/// The state transitions, all driven by one switch so they can be compared.
fn state_transitions(theme: &Theme, forward: bool) -> gpui::AnyElement {
    let column = || div().flex().flex_col().gap(px(theme.spacing.md));

    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(theme.spacing.xl))
        .child(
            column()
                .w(px(280.0))
                .child({
                    let terms = Checkbox::new("gallery.state.terms").label("Accept the terms");
                    if forward {
                        terms.checked(true)
                    } else {
                        terms.mixed()
                    }
                    .on_change(|_, _, _| {})
                })
                .child(
                    Radio::new("gallery.state.plan")
                        .label("Bill monthly")
                        .selected(forward)
                        .on_select(|_, _| {}),
                )
                .child(
                    Switch::new("gallery.state.notify")
                        .label("Send run notifications")
                        .on(forward)
                        .on_change(|_, _, _| {}),
                ),
        )
        .child(
            column()
                .w(px(320.0))
                .child(
                    SegmentedControl::new("gallery.state.view")
                        .segments(vec![
                            Segment::new("list", "List"),
                            Segment::new("grid", "Grid"),
                        ])
                        .selected(if forward { "grid" } else { "list" })
                        .on_select(|_, _, _| {}),
                )
                .child(
                    Tabs::new("gallery.state.tabs")
                        .tabs(vec![
                            TabItem::new("overview", "Overview"),
                            TabItem::new("runs", "Runs"),
                        ])
                        .selected(if forward { "runs" } else { "overview" })
                        .on_select(|_, _, _| {}),
                )
                .child(
                    ProgressBar::new("gallery.state.progress")
                        .label("Uploading artifacts")
                        .fraction(if forward { 0.86 } else { 0.12 }),
                ),
        )
        .child(
            column().w(px(320.0)).child(
                Accordion::new("gallery.state.sections")
                    .expanded_ids(if forward { &["retention"][..] } else { &[][..] })
                    .on_toggle(|_, _, _, _| {})
                    .section(
                        AccordionSection::new("retention", "Retention")
                            .description("How long verified results are kept")
                            .body(div().child("Results are kept in the workspace for 30 days.")),
                    ),
            ),
        )
        .child(
            Button::new("gallery.state.flip")
                .label("Flip every state")
                .secondary()
                .on_click(|_, cx| {
                    cx.update_global::<GalleryStates, ()>(|state, _| {
                        state.forward = !state.forward
                    });
                    cx.refresh_windows();
                }),
        )
        .into_any_element()
}

fn lower_gallery(theme: &Theme, cx: &mut App) -> gpui::AnyElement {
    div()
        .id("gallery-lower-root")
        .size_full()
        .overflow_y_scroll()
        .bg(theme.colors.canvas)
        .font_family(theme.typography.sans.clone())
        .text_color(theme.colors.text)
        .semantic_in(
            cx,
            NodeSpec::new("gallery.lower", Role::Window).text("gpui-kit gallery lower scene"),
        )
        .child(
            recipes::page(theme)
                .child(recipes::page_header(theme, "Patterns and effects", None))
                .child(recipes::subtitle(
                    theme,
                    "Deterministic fixture scene for floating surfaces and loading states.",
                ))
                .child(recipes::section_title(theme, "Motion and loading"))
                .child(
                    Card::new().child(
                        div()
                            .p(px(theme.spacing.xl))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(theme.spacing.xl))
                            .child(PulseLoader::new("lower.pulse"))
                            .child(GradientSpinner::new("lower.gradient"))
                            .child(
                                div()
                                    .w(px(260.0))
                                    .child(Skeleton::new("lower.skeleton").rows(4)),
                            ),
                    ),
                )
                .child(recipes::section_title(theme, "Floating surfaces"))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap(px(theme.spacing.lg))
                        .child(menu_sample(theme, 360.0))
                        .child(
                            popover::dialog_card(theme)
                                .child(popover::dialog_title(theme, "Replace existing theme?"))
                                .child(popover::dialog_body(
                                    theme,
                                    "The application owns this decision. The component only presents the choice and emits an action.",
                                ))
                                .child(
                                    div()
                                        .mt(px(theme.spacing.lg))
                                        .flex()
                                        .flex_row()
                                        .justify_end()
                                        .gap(px(theme.spacing.sm))
                                        .child(
                                            Button::new("gallery.dialog.cancel")
                                                .label("Cancel")
                                                .ghost()
                                                .on_click(|_, _| {}),
                                        )
                                        .child(
                                            Button::new("gallery.dialog.replace")
                                                .label("Replace")
                                                .primary()
                                                .on_click(|_, _| {}),
                                        ),
                                ),
                        ),
                )
                .child(recipes::footnote(
                    theme,
                    "Frost uses the pinned BackdropBlur patch on macOS and an opaque fallback elsewhere.",
                )),
        )
        .into_any_element()
}

fn main() {
    let capture_path = capture_path();
    let lower_scene = env::args().any(|argument| argument == "--scene=lower");
    if env::args().any(|argument| argument == "--list-scenes") {
        for scene in gpui_kit::scenes::catalog() {
            println!("{}", scene.name);
        }
        return;
    }
    let scene =
        flag("--scene").and_then(|name| gpui_kit::scenes::find(&name).map(|scene| scene.name));
    let theme_id = flag("--theme");
    let compact = env::args().any(|argument| argument == "--density=compact");
    let app = gpui_platform::application().with_assets(gpui_kit::assets::Assets);
    app.run(move |cx: &mut App| {
        gpui_kit::install(cx);
        if let Some(id) = theme_id.as_deref()
            && !activate_theme(id, cx)
        {
            eprintln!("unknown theme `{id}`");
        }
        if compact {
            set_density(Density::Compact, cx);
        }
        let bounds = Bounds::centered(None, size(px(920.0), px(1000.0)), cx);
        let _window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| Gallery {
                        lower_scene,
                        scene,
                        queue: gallery_queue(),
                        attached: Vec::new(),
                        provider: cx.new(|cx| {
                            Select::new("gallery.provider", window, cx)
                                .options([
                                    SelectOption::new("anthropic", "Anthropic"),
                                    SelectOption::new("openai", "OpenAI")
                                        .description("Requires a key"),
                                    SelectOption::new("local", "Local runtime").disabled(true),
                                ])
                                .selected("anthropic")
                                .placeholder("Choose a provider")
                        }),
                        search: cx.new(|cx| {
                            TextInput::new("gallery.search", window, cx).placeholder("Search")
                        }),
                        token: cx.new(|cx| {
                            TextInput::new("gallery.token", window, cx)
                                .placeholder("sk-...")
                                .secret(true)
                        }),
                        rejected: cx.new(|cx| {
                            TextInput::new("gallery.rejected", window, cx)
                                .text("not an email")
                                .invalid(true)
                                .required(true)
                        }),
                        notes: cx.new(|cx| {
                            TextArea::new("gallery.notes", window, cx)
                                .placeholder("What changed, and why")
                                .rows(3)
                                .max_rows(8)
                        }),
                        workspace: cx.new(|cx| {
                            TextInput::new("gallery.workspace", window, cx).text("Runs 2024")
                        }),
                        retention: cx.new(|cx| {
                            NumberInput::new("gallery.retention", window, cx)
                                .value(30.0)
                                .range(1.0, 60.0)
                                .step(5.0)
                                .unit("days")
                        }),
                        region: cx.new(|cx| {
                            Combobox::new("gallery.region", window, cx)
                                .options([
                                    SelectOption::new("eu-west", "Europe (Ireland)"),
                                    SelectOption::new("eu-north", "Europe (Stockholm)"),
                                    SelectOption::new("us-east", "United States (Virginia)"),
                                    SelectOption::new("ap-south", "Asia Pacific (Mumbai)")
                                        .disabled(true),
                                ])
                                .selected("eu-west")
                                .placeholder("Choose a region")
                        }),
                        labels: cx.new(|cx| {
                            TagInput::new("gallery.labels", window, cx)
                                .tags(["indexing", "nightly"])
                                .placeholder("Add a label")
                                .max(5)
                        }),
                        publish: cx.new(|cx| {
                            SplitButton::new("gallery.publish", window, cx)
                                .label("Publish")
                                .primary()
                                .on_click(|_, _| {})
                                .items(
                                    [
                                        MenuItem::command("publish.draft", "Save as draft"),
                                        MenuItem::command("publish.schedule", "Schedule\u{2026}")
                                            .shortcut("cmd-shift-s"),
                                        MenuItem::separator("publish.rule"),
                                        MenuItem::command(
                                            "publish.export",
                                            "Export without publishing",
                                        ),
                                    ],
                                    cx,
                                )
                        }),
                        confirm: cx.new(|cx| {
                            Dialog::new("gallery.confirm", window, cx)
                                .title("Delete this workspace?")
                                .description(
                                    "Everything in it is removed from this machine. \
                                     The application owns the decision; the dialog reports it.",
                                )
                                .destructive(true)
                                .cancel_label("Keep")
                                .confirm_label("Delete")
                        }),
                        toasts: cx.new(ToastLayer::new),
                        filters: cx.new(|cx| {
                            Popover::new("gallery.filters", window, cx)
                                .trigger("Filters")
                                .content(|_, cx| {
                                    let theme = Theme::get(cx).clone();
                                    div()
                                        .flex()
                                        .flex_col()
                                        .w(px(220.0))
                                        .gap(px(theme.spacing.sm))
                                        .child("Anything can live in a popover.")
                                        .child(
                                            Checkbox::new("gallery.filters.failing")
                                                .label("Failing runs only")
                                                .on_change(|_, _, _| {}),
                                        )
                                        .into_any_element()
                                })
                        }),
                        menu: cx.new(|cx| {
                            Menu::new("gallery.run", window, cx)
                                .trigger("Run actions")
                                .items(gallery_menu_items())
                        }),
                        records: cx.new(|cx| {
                            ContextMenu::new("gallery.record", window, cx)
                                .target("run-a04")
                                .menu(gallery_menu_items())
                                .content(|_, cx| {
                                    let theme = Theme::get(cx).clone();
                                    div()
                                        .w(px(360.0))
                                        .p(px(theme.spacing.md))
                                        .rounded(px(theme.radii.card))
                                        .border(px(theme.borders.hairline))
                                        .border_color(theme.colors.hairline)
                                        .child("Right-click this fixture row")
                                        .into_any_element()
                                })
                        }),
                        palette: cx.new(|cx| {
                            CommandPalette::new("gallery.palette", window, cx)
                                .commands(gallery_commands())
                        }),
                        overflow: cx.new(|cx| {
                            Menu::new("gallery.toolbar.overflow", window, cx)
                                .trigger_icon(Icon::List)
                                .trigger_name("More actions")
                        }),
                        page_size: cx.new(|cx| {
                            Select::new("gallery.pages.size", window, cx)
                                .options([
                                    SelectOption::new("25", "25 per page"),
                                    SelectOption::new("50", "50 per page"),
                                    SelectOption::new("100", "100 per page"),
                                ])
                                .selected("50")
                        }),
                        filter_drawer: cx.new(|cx| {
                            Drawer::new("gallery.drawer", window, cx)
                                .edge(Edge::Right)
                                .size(340.0)
                                .title("Filter runs")
                                .description(
                                    "The drawer reports what was chosen. \
                                     The application decides what it means.",
                                )
                                .content(|_, cx| {
                                    let theme = Theme::get(cx).clone();
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(theme.spacing.sm))
                                        .child(
                                            Checkbox::new("gallery.drawer.failed")
                                                .label("Failed runs only")
                                                .checked(true)
                                                .on_change(|_, _, _| {}),
                                        )
                                        .child(
                                            Checkbox::new("gallery.drawer.mine")
                                                .label("Started by me")
                                                .on_change(|_, _, _| {}),
                                        )
                                        .into_any_element()
                                })
                                .footer(|_, _| {
                                    Button::new("gallery.drawer.apply")
                                        .label("Apply")
                                        .primary()
                                        .full_width(true)
                                        .on_click(|_, _| {})
                                        .into_any_element()
                                })
                        }),
                    })
                },
            )
            .expect("open gallery window");

        if let Some(path) = capture_path.clone() {
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(1200))
                    .await;
                match capture(&path, cx) {
                    Ok(()) => cx.update(|cx| cx.quit()),
                    Err(error) => {
                        eprintln!("gallery capture failed: {error:#}");
                        cx.update(|cx| cx.quit())
                    }
                }
            })
            .detach();
        } else {
            cx.activate(true);
        }
    });
}

fn capture_path() -> Option<PathBuf> {
    flag("--capture").map(PathBuf::from)
}

fn flag(name: &str) -> Option<String> {
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == name {
            return args.next();
        }
    }
    None
}

fn capture(path: &Path, cx: &mut gpui::AsyncApp) -> Result<()> {
    let frame = cx.update(|cx| {
        let Some(handle) = cx.windows().first().cloned() else {
            bail!("gallery window is gone");
        };
        let frame = handle.update(cx, |_, window, _| {
            gpui_kit_testkit::capture::capture_window(window)
        })??;
        Ok(frame)
    })?;
    let bytes = frame.write_png(path)?;
    if bytes == 0 {
        bail!("capture produced an empty file");
    }
    println!(
        "{} {}×{} ({} bytes)",
        path.display(),
        frame.width,
        frame.height,
        bytes
    );
    Ok(())
}
