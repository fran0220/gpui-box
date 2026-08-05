use std::env;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Result, bail};
use gpui::{
    App, Bounds, Context, IntoElement, Render, SharedString, Window, WindowBounds, WindowOptions,
    div, prelude::*, px, size,
};
use gpui_kit::assets::{Icon, icon};
use gpui_kit::badge::{BadgeTone, badge};
use gpui_kit::button::{ButtonSize, ButtonVariant, action_button};
use gpui_kit::{card, loaders, popover, settings, status};
use gpui_kit_semantics::{NodeSpec, Role, Semantic, SemanticRegistry};
use gpui_kit_theme::Theme;
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
    semantics: SemanticRegistry,
    lower_scene: bool,
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.semantics.begin_frame();
        let theme = Theme::get(cx).clone();
        if self.lower_scene {
            return lower_gallery(&theme, &self.semantics).into_any_element();
        }
        let action = |id: &'static str, label: &'static str, variant, disabled| {
            action_button(
                id,
                &theme,
                label,
                variant,
                ButtonSize::Medium,
                disabled,
                |_, _| {},
            )
            .semantic(
                &self.semantics,
                NodeSpec::new(id, Role::Button)
                    .text(label)
                    .disabled(disabled),
            )
        };

        let settings_card = fixture_settings_card(&theme);

        div()
            .id("gallery-root")
            .size_full()
            .overflow_y_scroll()
            .bg(theme.colors.canvas)
            .font_family(theme.typography.sans.clone())
            .text_color(theme.colors.text)
            .semantic(
                &self.semantics,
                NodeSpec::new("gallery.root", Role::Window).text("gpui-kit gallery"),
            )
            .child(
                settings::page(&theme)
                    .child(settings::page_header(&theme, "gpui-kit", Some(24)))
                    .child(settings::subtitle(
                        &theme,
                        "A truthful desktop UI design system, semantic tree, and test kit.",
                    ))
                    .child(settings::section_title(&theme, "Actions"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(theme.spacing.sm))
                            .child(action(
                                "gallery.primary",
                                "Primary action",
                                ButtonVariant::Primary,
                                false,
                            ))
                            .child(action(
                                "gallery.ghost",
                                "Ghost action",
                                ButtonVariant::Ghost,
                                false,
                            ))
                            .child(action(
                                "gallery.danger",
                                "Destructive",
                                ButtonVariant::Danger,
                                false,
                            ))
                            .child(action(
                                "gallery.disabled",
                                "Unavailable",
                                ButtonVariant::Primary,
                                true,
                            )),
                    )
                    .child(settings::section_title(&theme, "Status"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(theme.spacing.sm))
                            .child(badge(&theme, "Neutral", BadgeTone::Neutral))
                            .child(badge(&theme, "Accent", BadgeTone::Accent))
                            .child(badge(&theme, "Success", BadgeTone::Success))
                            .child(badge(&theme, "Warning", BadgeTone::Warning))
                            .child(badge(&theme, "Danger", BadgeTone::Danger))
                            .child(badge(&theme, "Info", BadgeTone::Info)),
                    )
                    .child(settings::section_title(&theme, "Settings pattern"))
                    .child(settings_card)
                    .child(settings::section_title(&theme, "Truthful states"))
                    .child(status::callout(
                        &theme,
                        "The host refused this action. Preserve its exact reason instead of showing an empty state.",
                        status::StatusTone::Danger,
                    ))
                    .child(
                        div()
                            .mt(px(theme.spacing.sm))
                            .child(status::callout(
                                &theme,
                                "Refreshing failed. The last verified model catalog remains visible.",
                                status::StatusTone::Warning,
                            )),
                    )
                    .child(settings::section_title(&theme, "Motion and loading"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(theme.spacing.xl))
                            .child(loaders::pulse_loader(
                                "gallery-pulse",
                                &theme,
                                8.0,
                            ))
                            .child(loaders::gradient_spinner(
                                "gallery-gradient",
                                &theme,
                                5.0,
                            ))
                            .child(
                                div()
                                    .w(px(220.0))
                                    .child(loaders::skeleton_rows(
                                        "gallery-skeleton",
                                        &theme,
                                        3,
                                    )),
                            ),
                    )
                    .child(settings::section_title(&theme, "Popover primitives"))
                    .child(
                        popover::card(&theme)
                            .w(px(320.0))
                            .child(popover::heading(&theme, "Recent"))
                            .child(
                                popover::menu_row(&theme, true, false)
                                    .child(icon(Icon::Folder).size(px(15.0)))
                                    .child(SharedString::from("gpui-kit"))
                                    .child(div().flex_1())
                                    .child(popover::key_cap(&theme, "↵")),
                            )
                            .child(
                                popover::menu_row(&theme, false, true)
                                    .child(icon(Icon::GitBranch).size(px(15.0)))
                                    .child(SharedString::from("feature/design-system")),
                            )
                            .child(popover::separator(&theme))
                            .child(
                                popover::menu_row(&theme, false, false)
                                    .child(icon(Icon::Settings).size(px(15.0)))
                                    .child(SharedString::from("Settings")),
                            ),
                    )
                    .child(settings::footnote(
                        &theme,
                        "Fixture data is explicitly labeled. Product applications must render host-backed facts.",
                    )),
            )
            .into_any_element()
    }
}

fn fixture_settings_card(theme: &Theme) -> gpui::Div {
    let fixture = settings_fixture();
    assert!(
        fixture.fixture,
        "gallery data must identify itself as fixture"
    );
    let mut result = card::section_card(theme);
    for (index, row) in fixture.rows.iter().enumerate() {
        let (glyph, label, tone) = match row.state {
            FixtureState::Ready => (Icon::Monitor, "Ready", BadgeTone::Success),
            FixtureState::Stale => (Icon::Global, "Stale", BadgeTone::Warning),
            FixtureState::Neutral => (Icon::Key, "Approval", BadgeTone::Neutral),
        };
        result = result.child(
            card::card_row(theme, index == 0)
                .id(SharedString::from(format!("fixture.settings.{}", row.id)))
                .child(card::identity_tile(theme, glyph))
                .child(card::row_content(
                    theme,
                    row.title.clone(),
                    row.detail.clone(),
                ))
                .child(badge(theme, label, tone)),
        );
    }
    result
}

fn settings_fixture() -> &'static SettingsFixture {
    static FIXTURE: OnceLock<SettingsFixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        serde_json::from_str(SETTINGS_FIXTURE).expect("settings fixture must be valid JSON")
    })
}

fn lower_gallery(theme: &Theme, semantics: &SemanticRegistry) -> gpui::AnyElement {
    div()
        .id("gallery-lower-root")
        .size_full()
        .overflow_y_scroll()
        .bg(theme.colors.canvas)
        .font_family(theme.typography.sans.clone())
        .text_color(theme.colors.text)
        .semantic(
            semantics,
            NodeSpec::new("gallery.lower", Role::Window).text("gpui-kit gallery lower scene"),
        )
        .child(
            settings::page(theme)
                .child(settings::page_header(theme, "Patterns and effects", None))
                .child(settings::subtitle(
                    theme,
                    "Deterministic fixture scene for floating surfaces and loading states.",
                ))
                .child(settings::section_title(theme, "Motion and loading"))
                .child(
                    card::section_card(theme).child(
                        div()
                            .p(px(theme.spacing.xl))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(theme.spacing.xl))
                            .child(loaders::pulse_loader("lower-pulse", theme, 8.0))
                            .child(loaders::gradient_spinner("lower-gradient", theme, 5.0))
                            .child(
                                div()
                                    .w(px(260.0))
                                    .child(loaders::skeleton_rows("lower-skeleton", theme, 4)),
                            ),
                    ),
                )
                .child(settings::section_title(theme, "Floating surfaces"))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap(px(theme.spacing.lg))
                        .child(
                            popover::card(theme)
                                .w(px(360.0))
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
                                ),
                        )
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
                                        .child(action_button(
                                            "gallery.dialog.cancel",
                                            theme,
                                            "Cancel",
                                            ButtonVariant::Ghost,
                                            ButtonSize::Medium,
                                            false,
                                            |_, _| {},
                                        ))
                                        .child(action_button(
                                            "gallery.dialog.replace",
                                            theme,
                                            "Replace",
                                            ButtonVariant::Primary,
                                            ButtonSize::Medium,
                                            false,
                                            |_, _| {},
                                        )),
                                ),
                        ),
                )
                .child(settings::footnote(
                    theme,
                    "Frost uses the pinned BackdropBlur patch on macOS and an opaque fallback elsewhere.",
                )),
        )
        .into_any_element()
}

fn main() {
    let capture_path = capture_path();
    let lower_scene = env::args().any(|argument| argument == "--scene=lower");
    let app = gpui_platform::application().with_assets(gpui_kit::assets::Assets);
    app.run(move |cx: &mut App| {
        gpui_kit::install(cx);
        let bounds = Bounds::centered(None, size(px(920.0), px(900.0)), cx);
        let _window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| Gallery {
                        semantics: SemanticRegistry::new(),
                        lower_scene,
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
                        cx.update(|cx| cx.quit());
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
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--capture" {
            return args.next().map(PathBuf::from);
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
