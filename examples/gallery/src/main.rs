mod recipes;

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
use gpui_kit::overlay::popover;
use gpui_kit::prelude::*;
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
    lower_scene: bool,
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        SemanticRegistry::global(cx).begin_frame();
        let theme = Theme::get(cx).clone();
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
                    .child(recipes::footnote(
                        &theme,
                        "Fixture data is explicitly labeled. Product applications must render host-backed facts.",
                    )),
            )
            .into_any_element()
    }
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
    let app = gpui_platform::application().with_assets(gpui_kit::assets::Assets);
    app.run(move |cx: &mut App| {
        gpui_kit::install(cx);
        let bounds = Bounds::centered(None, size(px(920.0), px(1000.0)), cx);
        let _window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| Gallery { lower_scene }),
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
