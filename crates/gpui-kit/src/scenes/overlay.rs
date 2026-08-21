//! Surfaces that appear above the page and take a decision.

use super::support::*;

pub(super) fn kbd(_window: &mut Window, cx: &mut App) -> AnyElement {
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

pub(super) fn overlay(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .h(px(320.0))
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "Content behind the dialog",
        ))
        .child(
            Overlay::modal("scene.overlay.dialog")
                .placement(Placement::Center)
                .child(
                    crate::overlay::surface(&theme, gpui_kit_theme::Elevation::Modal)
                        .w(px(320.0))
                        .p(px(theme.spacing.lg))
                        .gap(px(theme.spacing.sm))
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Subtitle,
                            "Delete this workspace?",
                        ))
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
pub(super) struct SceneDialog {
    replace: Entity<Dialog>,
}

impl Global for SceneDialog {}

pub(super) fn dialog(window: &mut Window, cx: &mut App) -> AnyElement {
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
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "Content behind the dialog",
        ))
        .child(replace)
        .into_any_element()
}

pub(super) fn tooltip(_window: &mut Window, cx: &mut App) -> AnyElement {
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
                            .accessible_description("Writes the theme to a file on disk")
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
pub(super) struct SceneMenus {
    menu: Entity<Menu>,
    hung: Entity<Menu>,
    context: Entity<ContextMenu>,
    palette: Entity<CommandPalette>,
    popover: Entity<Popover>,
}

impl Global for SceneMenus {}

pub(super) fn menu_items() -> Vec<MenuItem> {
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
        MenuItem::separator("destroy"),
        MenuItem::command("delete", "Delete this run")
            .icon(Icon::Trash)
            .destructive(true),
    ]
}

pub(super) fn scene_commands() -> Vec<Command> {
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

pub(super) fn ensure_menus(window: &mut Window, cx: &mut App) {
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

    let hung = cx.new(|cx| {
        Menu::new("scene.menu.hung", window, cx)
            .trigger("Model")
            .items(menu_items())
            .hang(Hang::End)
    });
    hung.update(cx, |menu, cx| menu.open(window, cx));

    let context = cx.new(|cx| {
        ContextMenu::new("scene.context.run", window, cx)
            .name("Run actions")
            .target("run-a04")
            .menu(menu_items())
            .content(|_, cx| {
                let theme = cx.theme().clone();
                div()
                    .w(px(320.0))
                    .p(px(theme.spacing.md))
                    .surface(&theme, Surface::Panel)
                    .radius(&theme, Radius::Card)
                    .child(crate::foundation::text(
                        &theme,
                        TypeScale::Body,
                        "Right-click this fixture row",
                    ))
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
                    .child(crate::foundation::text(
                        &theme,
                        TypeScale::Body,
                        "Anything can live in a popover.",
                    ))
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
        hung,
        context,
        palette,
        popover,
    });
}

pub(super) fn popover(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_menus(window, cx);
    let popover = cx.global::<SceneMenus>().popover.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .h(px(320.0))
        // The trigger keeps its place while the surface is open, because the
        // surface is anchored to it rather than laid out beside it.
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "The trigger owns whether the surface is open.",
        ))
        .child(popover)
        .into_any_element()
}

pub(super) fn menu(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_menus(window, cx);
    let menus = cx.global::<SceneMenus>();
    let menu = menus.menu.clone();
    let hung = menus.hung.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .h(px(560.0))
        // An open surface is an overlay and takes no room in the flow, so the
        // scene reserves what the first menu occupies. Without it the caption
        // below is drawn under that menu and cannot be read.
        .child(div().h(px(230.0)).child(menu))
        .child(caption(
            &theme,
            "a trigger against the trailing edge hangs its surface from that edge instead",
        ))
        .child(div().w_full().flex().justify_end().child(hung))
        .into_any_element()
}

pub(super) fn context_menu(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_menus(window, cx);
    let context = cx.global::<SceneMenus>().context.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .h(px(400.0))
        // Opening a context menu reports the row that was pointed at; what is
        // selected stays the host's answer.
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "The right-click reports the row. Nothing is selected by it.",
        ))
        .child(context)
        .into_any_element()
}

pub(super) fn command_palette(window: &mut Window, cx: &mut App) -> AnyElement {
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
pub(super) struct SceneToasts {
    layer: Entity<ToastLayer>,
}

impl Global for SceneToasts {}

pub(super) fn toast(_window: &mut Window, cx: &mut App) -> AnyElement {
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
            // A refusal, so the offer is the thing that could change the
            // answer. Retrying an unapproved call only gets refused again.
            Toast::new(
                "scene.toast.refused",
                "The host refused to publish this run",
            )
            .tone(Tone::Warning)
            .detail("Approval is required for this workspace.")
            .action("Request approval", |_, _| {}),
        );
        // The failure beside the refusal, so the two tones are on screen
        // together and the difference between them is a picture rather than a
        // claim.
        toast_push(
            cx,
            Toast::new("scene.toast.failed", "Publishing this run failed")
                .tone(Tone::Danger)
                .detail("The publish service did not respond.")
                .action("Try again", |_, _| {}),
        );
    }
    let layer = cx.global::<SceneToasts>().layer.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(560.0))
        .h(px(360.0))
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "Content behind the notifications",
        ))
        // A failure keeps its report on screen; only the success times out.
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "A danger or warning toast stays until it is dismissed.",
        ))
        .child(layer)
        .into_any_element()
}

/// The notification centre the scene shows, kept across frames.
pub(super) struct SceneNotifications {
    centre: Entity<NotificationCenter>,
}

impl Global for SceneNotifications {}

pub(super) fn notification_center(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneNotifications>() {
        let centre = cx.new(|cx| NotificationCenter::new("scene.notifications", cx));
        centre.update(cx, |centre, cx| {
            centre.record(
                Notification::new("scene.notify.exported", "Theme exported to disk")
                    .tone(Tone::Success)
                    .at("9:41")
                    .read(true),
                cx,
            );
            centre.record(
                Notification::new("scene.notify.stale", "Refreshing the model catalog failed")
                    .tone(Tone::Warning)
                    .detail("The last verified catalog is still shown.")
                    .at("9:44"),
                cx,
            );
            centre.record(
                Notification::new(
                    "scene.notify.refused",
                    "The host refused to publish this run",
                )
                .tone(Tone::Warning)
                .detail("Approval is required for this workspace.")
                .at("9:46")
                .action("Request approval", |_, _| {}),
                cx,
            );
        });
        cx.set_global(SceneNotifications { centre });
    }
    let centre = cx.global::<SceneNotifications>().centre.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        // The same three reports the toast scene shows, after their toasts
        // have gone.
        .child(caption(
            &theme,
            "what the toasts said, still here once they timed out",
        ))
        .child(centre)
        .into_any_element()
}

/// Glass over the page it covers, which is the only way to see that the
/// backdrop is blurred rather than merely tinted.
pub(super) fn frost(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let card = |title: &'static str, body: &'static str| {
        div()
            .column()
            .gap(px(theme.space(Space::Xs)))
            .p(px(theme.space(Space::Md)))
            .child(crate::foundation::text(
                &theme,
                TypeScale::Label,
                SharedString::from(title),
            ))
            .child(caption(&theme, body))
    };
    let stripes = || {
        div()
            .absolute()
            .top(px(56.0))
            .left(px(100.0))
            .w(px(330.0))
            .h(px(64.0))
            .flex()
            .overflow_hidden()
            .children((0..22).map(|index| {
                div()
                    .flex_none()
                    .w(px(15.0))
                    .h(px(64.0))
                    .bg(if index % 2 == 0 {
                        theme.colors.accent
                    } else {
                        theme.colors.canvas
                    })
            }))
    };
    stack(&theme)
        .w(px(480.0))
        .child(caption(&theme, "A floating surface on glass"))
        .child(
            div()
                .relative()
                .h(px(200.0))
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(stripes())
                .child(filler(&theme, "Document", 8))
                .child(
                    div()
                        .absolute()
                        .top(px(48.0))
                        .left(px(120.0))
                        .w(px(240.0))
                        .child(
                            Frost::new("scene.frost.popover")
                                .radius(Radius::Card)
                                .child(card(
                                    "Rename",
                                    "The page behind stays visible, out of focus",
                                )),
                        ),
                ),
        )
        .child(caption(&theme, "The same glass over a panel surface"))
        .child(
            div()
                .relative()
                .h(px(160.0))
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(stripes())
                .child(filler(&theme, "Files", 6))
                .child(
                    div()
                        .absolute()
                        .top(px(32.0))
                        .left(px(120.0))
                        .w(px(280.0))
                        .child(
                            Frost::new("scene.frost.rail")
                                .surface(gpui_kit_theme::Surface::Panel)
                                .radius(Radius::Dialog)
                                .blur(32.0)
                                .child(card("Rail", "The striped backdrop stays out of focus")),
                        ),
                ),
        )
        .into_any_element()
}

/// The optics, one at a time and then together, over a backdrop with enough
/// structure in it that a bend is visible as a bend rather than as a smudge.
///
/// The presets sit side by side deliberately: `Frosted` is the control, and
/// what separates it from `Lens` is exactly the refraction, so a reviewer
/// looking at the pair is looking at the thing that changed.
pub(super) fn glass(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let label = |title: &'static str, body: &'static str| {
        div()
            .column()
            .gap(px(theme.space(Space::Xs)))
            .p(px(theme.space(Space::Md)))
            .child(crate::foundation::text(
                &theme,
                TypeScale::Label,
                SharedString::from(title),
            ))
            .child(caption(&theme, body))
    };

    // A checkerboard bends far more legibly than a flat fill: a straight edge
    // running under the rim is what makes refraction readable in a still.
    let checkerboard =
        |width: f32, height: f32| {
            let columns = (width / 48.0).ceil() as usize;
            let rows = (height / 48.0).ceil() as usize;
            div()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .w(px(width))
                .h(px(height))
                .column()
                .overflow_hidden()
                .children((0..rows).map(|row| {
                    div()
                        .flex()
                        .flex_none()
                        .h(px(48.0))
                        .children((0..columns).map(move |column| {
                            div().flex_none().w(px(48.0)).h(px(48.0)).bg(
                                if (row + column) % 2 == 0 {
                                    theme.colors.accent
                                } else {
                                    theme.colors.canvas
                                },
                            )
                        }))
                }))
        };

    let plate =
        |ident: &'static str, preset: GlassPreset, title: &'static str, body: &'static str| {
            div()
                .relative()
                .h(px(150.0))
                .w(px(460.0))
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(checkerboard(460.0, 150.0))
                .child(
                    div()
                        .absolute()
                        .top(px(28.0))
                        .left(px(90.0))
                        .w(px(280.0))
                        .child(
                            Glass::new(ident)
                                .preset(preset)
                                .radius(Radius::Dialog)
                                .child(label(title, body)),
                        ),
                )
        };

    stack(&theme)
        .w(px(900.0))
        .child(caption(&theme, "Frosted: blurred, and nothing bent"))
        .child(plate(
            "scene.glass.frosted",
            GlassPreset::Frosted,
            "Frosted",
            "The control the others are read against",
        ))
        .child(caption(&theme, "Lens: the edge bends the backdrop"))
        .child(plate(
            "scene.glass.lens",
            GlassPreset::Lens,
            "Lens",
            "The squares bend at the rim and run straight inside",
        ))
        .child(caption(
            &theme,
            "Liquid: the bend, its colour, and the light",
        ))
        .child(plate(
            "scene.glass.liquid",
            GlassPreset::Liquid,
            "Liquid",
            "Dispersion at the rim and a highlight from the top right",
        ))
        // The last two demonstrations sit side by side so the whole scene
        // stays inside the window a real display can give the gallery, which
        // is where the DirectX renderer gets looked at.
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(theme.space(Space::Md)))
                .child(
                    div()
                        .column()
                        .gap(px(theme.space(Space::Sm)))
                        .child(caption(&theme, "Fused: two panes joined into one body"))
                        .child(
                            div()
                                .relative()
                                .h(px(150.0))
                                .w(px(440.0))
                                .surface(&theme, Surface::Panel)
                                .radius(&theme, Radius::Card)
                                .overflow_hidden()
                                .child(checkerboard(440.0, 150.0))
                                .child(
                                    div().absolute().top(px(40.0)).left(px(40.0)).child(
                                        GlassGroup::new("scene.glass.fused")
                                            .preset(GlassPreset::Liquid)
                                            .radius(Radius::Dialog)
                                            .gap(12.0)
                                            .pane(
                                                "scene.glass.fused.left",
                                                label("Left", "One lobe of the body"),
                                            )
                                            .pane(
                                                "scene.glass.fused.right",
                                                label("Right", "Joined across the gap"),
                                            ),
                                    ),
                                ),
                        ),
                )
                .child(
                    div()
                        .column()
                        .gap(px(theme.space(Space::Sm)))
                        .child(caption(&theme, "Adaptive: the tint deepens when opposed"))
                        .child(
                            div()
                                .relative()
                                .h(px(150.0))
                                .w(px(440.0))
                                .surface(&theme, Surface::Panel)
                                .radius(&theme, Radius::Card)
                                .overflow_hidden()
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(0.0))
                                        .left(px(0.0))
                                        .w(px(220.0))
                                        .h(px(150.0))
                                        .bg(gpui::white()),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(0.0))
                                        .left(px(220.0))
                                        .w(px(220.0))
                                        .h(px(150.0))
                                        .bg(gpui::black()),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(28.0))
                                        .left(px(15.0))
                                        .w(px(190.0))
                                        .child(
                                            Glass::new("scene.glass.adaptive.bright")
                                                .preset(GlassPreset::Liquid)
                                                .radius(Radius::Dialog)
                                                .adaptive(true)
                                                .child(label(
                                                    "Bright",
                                                    "The reading lands next frame",
                                                )),
                                        ),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(28.0))
                                        .left(px(235.0))
                                        .w(px(190.0))
                                        .child(
                                            Glass::new("scene.glass.adaptive.dark")
                                                .preset(GlassPreset::Liquid)
                                                .radius(Radius::Dialog)
                                                .adaptive(true)
                                                .child(label("Dark", "The same glass, other side")),
                                        ),
                                ),
                        ),
                ),
        )
        .into_any_element()
}

/// The drawer the scene shows, kept across frames and settled so the capture
/// photographs the panel where it comes to rest rather than mid-slide.
pub(super) struct SceneDrawer {
    filters: Entity<Drawer>,
}

impl Global for SceneDrawer {}

pub(super) fn drawer(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneDrawer>() {
        let filters = cx.new(|cx| {
            Drawer::new("scene.drawer.filters", window, cx)
                .edge(Edge::Right)
                .size(320.0)
                .resizable(true)
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
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "Content behind the drawer",
        ))
        .child(filters)
        .into_any_element()
}

pub(super) fn hover_card(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_ordinary(window, cx);
    let card = cx.global::<SceneOrdinary>().hover_card.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(460.0))
        .h(px(300.0))
        .child(caption(&theme, "A preview the pointer can travel into"))
        .child(
            row(&theme)
                .child(crate::foundation::text(
                    &theme,
                    TypeScale::Label,
                    "Reported by",
                ))
                .child(card),
        )
        .into_any_element()
}

pub(super) fn menubar(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure_ordinary(window, cx);
    let bar = cx.global::<SceneOrdinary>().menubar.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .h(px(360.0))
        .child(bar)
        .into_any_element()
}
