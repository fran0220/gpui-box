//! One canonical rendering per component.
//!
//! A scene is the single description of a component's states that both the
//! gallery and the audit tests consume, so a component cannot be reviewed
//! visually in one arrangement and tested in another.

use gpui::{AnyElement, App, Entity, Global, IntoElement, Window, div, prelude::*, px};
use gpui_kit_assets::Icon;
use gpui_kit_theme::{Space, Theme};

use crate::controls::input::TextInput;
use crate::controls::select::{Select, SelectOption};
use crate::display::badge::Tone;
use crate::foundation::ActiveTheme;
use crate::overlay::{Kbd, Overlay, Placement, Tooltip, Tooltipped};
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
}

impl Global for SceneInputs {}

fn input(window: &mut Window, cx: &mut App) -> AnyElement {
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
        };
        cx.set_global(inputs);
    }
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
