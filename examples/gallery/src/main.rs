mod recipes;

use std::env;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Result, bail};
use gpui::{
    App, Bounds, Context, Entity, IntoElement, Render, SharedString, Window, WindowBounds,
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
    confirm: Entity<Dialog>,
    toasts: Entity<ToastLayer>,
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
                    .child(recipes::section_title(&theme, "Popover primitives"))
                    .child(menu_sample(&theme, 320.0))
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
                    .child(self.confirm.clone())
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
