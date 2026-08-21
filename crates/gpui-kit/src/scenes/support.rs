//! What more than one family of scenes needs.
//!
//! Everything here is used by scenes in at least two families. A fixture
//! that only one family builds lives in that family, next to the scenes
//! that read it, so that moving a component moves its fixture with it.

pub(super) use gpui::{
    AnyElement, App, Corners, DevicePixels, Entity, Focusable, Global, IntoElement, Pixels,
    RenderImage, SharedString, SpriteBlendMode, SpriteColorMode, SpriteInstance, SpriteTransform,
    Window, bounds, canvas, conic_gradient_stops, div, linear_color_stop, point, prelude::*, px,
    radial_gradient_stops, radians, size,
};
pub(super) use gpui_kit_assets::Icon;
pub(super) use gpui_kit_semantics::{NodeSpec, Role, Semantic};
pub(super) use gpui_kit_theme::{Radius, Space, Surface, SyntaxColor, TextTone, Theme, TypeScale};
pub(super) use std::rc::Rc;
pub(super) use std::sync::{Arc, OnceLock};
pub(super) use std::time::Duration;

pub(super) use crate::controls::combobox::Combobox;
pub(super) use crate::controls::input::TextInput;
pub(super) use crate::controls::keybinding_recorder::KeybindingRecorder;
pub(super) use crate::controls::number_input::NumberInput;
pub(super) use crate::controls::select::{Select, SelectOption};
pub(super) use crate::controls::split_button::SplitButton;
pub(super) use crate::controls::tag_input::TagInput;
pub(super) use crate::controls::textarea::{Frame, TextArea};
pub(super) use crate::display::badge::Tone;
pub(super) use crate::display::icon::{Icon as IconView, IconTone};
pub(super) use crate::foundation::ActiveTheme;
pub(super) use crate::foundation::direction::{ActiveDirection, DirectionalExt};
pub(super) use crate::interaction::dnd;
pub(super) use crate::overlay::toast::push as toast_push;
pub(super) use crate::overlay::{Edge, Kbd, Overlay, Placement, Tooltip, Tooltipped};
pub(super) use crate::prelude::*;
pub(super) use crate::strings::{ActiveStrings, StringKey};

pub(super) fn stack(theme: &Theme) -> gpui::Div {
    div()
        .column()
        .gap(px(theme.spacing.md))
        .p(px(theme.spacing.lg))
        .bg(theme.colors.canvas)
        .text_color(theme.colors.text)
        .font_family(theme.typography.sans.clone())
}

pub(super) fn row(theme: &Theme) -> gpui::Div {
    div()
        .row()
        .flex_wrap()
        .gap(px(theme.spacing.sm))
        .items_center()
}

/// A pane of fixture copy, for a scene that needs something on both sides of a
/// divider or below a viewport.
pub(super) fn filler(theme: &Theme, title: &'static str, lines: usize) -> gpui::Div {
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

/// A caption naming the state a drag scene was staged in.
pub(super) fn caption(theme: &Theme, text: impl Into<SharedString>) -> gpui::Div {
    crate::foundation::text(theme, TypeScale::Caption, text.into())
        .text_tone(theme, TextTone::Muted)
}

/// The element a host hands back for an image this crate did not fetch.
///
/// A flat fill rather than a picture, because the crate ships no photographs
/// and a scene has to photograph the same pixels every run.
pub(super) fn scene_picture(label: &'static str, cx: &App) -> AnyElement {
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

/// The ordinary-application views that own state across frames.
pub(super) struct SceneOrdinary {
    pub(super) menubar: Entity<Menubar>,
    pub(super) hover_card: Entity<HoverCard>,
    pub(super) copy_idle: Entity<CopyButton>,
    pub(super) copy: Entity<CopyButton>,
    pub(super) copy_refused: Entity<CopyButton>,
}

impl Global for SceneOrdinary {}

pub(super) fn ensure_ordinary(window: &mut Window, cx: &mut App) {
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
