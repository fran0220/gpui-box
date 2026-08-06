//! One canonical rendering per component.
//!
//! A scene is the single description of a component's states that both the
//! gallery and the audit tests consume, so a component cannot be reviewed
//! visually in one arrangement and tested in another.

use gpui::{AnyElement, App, IntoElement, Window, div, prelude::*, px};
use gpui_kit_theme::Theme;

use crate::display::badge::Tone;
use crate::foundation::ActiveTheme;
use crate::overlay::{Kbd, Overlay, Placement};
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
            name: "kbd",
            build: kbd,
        },
        Scene {
            name: "overlay",
            build: overlay,
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
