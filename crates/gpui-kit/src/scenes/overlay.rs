//! Surfaces that appear above the page and take a decision.

use super::support::*;

pub(super) fn kbd(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    // A keystroke is only ever read beside the thing it performs, so the
    // exhibit shows it where it is used: at the end of a menu row, at the end
    // of a sentence, and in the run of special keys whose glyphs are the part
    // that goes wrong.
    let named = |label: &'static str, keystroke: &'static str, id: &'static str| {
        div()
            .row()
            .w_full()
            .items_center()
            .justify_between()
            .gap_token(&theme, Space::Md)
            .py_token(&theme, Space::Xs)
            .child(crate::foundation::text(&theme, TypeScale::Body, label))
            .child(Kbd::new(keystroke).id(id))
    };

    stack(&theme)
        .w(px(420.0))
        .child(caption(&theme, "the shortcut sits with the thing it does"))
        .child(
            div()
                .column()
                .w_full()
                .card_surface(&theme, CardVariant::Elevated)
                .px_token(&theme, Space::Md)
                .py_token(&theme, Space::Sm)
                .child(named(
                    "Open the command palette",
                    "cmd-shift-p",
                    "scene.kbd.palette",
                ))
                .child(div().w_full().h(px(theme.space(Space::Sm))))
                .child(named("Copy the selection", "ctrl-c", "scene.kbd.copy"))
                .child(div().w_full().h(px(theme.space(Space::Sm))))
                .child(named("Rename in place", "cmd-alt-r", "scene.kbd.rename")),
        )
        .child(caption(
            &theme,
            "the keys that need the bundled symbol face",
        ))
        .child(
            row(&theme)
                .gap_token(&theme, Space::Sm)
                .child(Kbd::new("enter").id("scene.kbd.confirm"))
                .child(Kbd::new("escape").id("scene.kbd.dismiss"))
                .child(Kbd::new("backspace").id("scene.kbd.erase"))
                .child(Kbd::new("delete").id("scene.kbd.delete"))
                .child(Kbd::new("tab").id("scene.kbd.advance"))
                .child(Kbd::new("space").id("scene.kbd.space"))
                .child(Kbd::new("up").id("scene.kbd.up"))
                .child(Kbd::new("down").id("scene.kbd.down")),
        )
        .child(
            div()
                .row()
                .items_center()
                .gap_token(&theme, Space::Xs)
                .child(crate::foundation::text(&theme, TypeScale::Body, "Press"))
                .child(Kbd::new("cmd-enter").id("scene.kbd.inline"))
                .child(crate::foundation::text(
                    &theme,
                    TypeScale::Body,
                    "to send the message.",
                ))
                .text_color(theme.colors.text_muted),
        )
        .into_any_element()
}

/// A page for a modal layer to sit over.
///
/// A scrim drawn over an empty canvas is indistinguishable from a fill: the
/// only thing that shows it is translucent is what stays legible underneath
/// it. So every scene in this file that raises a modal layer puts a page worth
/// obscuring behind it rather than one line of text.
fn page_behind(theme: &Theme, title: &'static str) -> gpui::Div {
    let card = |heading: &'static str, lines: usize| {
        div()
            .flex_1()
            .min_w_0()
            .card_surface(theme, CardVariant::Elevated)
            .overflow_hidden()
            .child(filler(theme, heading, lines))
    };
    div()
        .column()
        .w_full()
        .gap(px(theme.space(Space::Md)))
        .child(crate::foundation::text(theme, TypeScale::Subtitle, title))
        .child(
            div()
                .row()
                .w_full()
                .gap(px(theme.space(Space::Md)))
                .child(card("Runs", 5))
                .child(card("Details", 5)),
        )
}

pub(super) fn overlay(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .h(px(320.0))
        .child(page_behind(&theme, "Workspace settings"))
        .child(
            Overlay::modal("scene.overlay.dialog")
                .placement(Placement::Center)
                .child(
                    crate::overlay::surface(&theme, crate::overlay::OverlaySurface::MODAL)
                        .w(px(320.0))
                        .p(px(theme.spacing.lg))
                        .gap(px(theme.spacing.sm))
                        .child(crate::foundation::text(
                            &theme,
                            TypeScale::Subtitle,
                            "Delete this workspace?",
                        ))
                        .child(
                            crate::foundation::text(
                                &theme,
                                TypeScale::Body,
                                "Its runs, filters and saved views are removed for everyone. This \
                                 cannot be undone.",
                            )
                            .text_tone(&theme, TextTone::Muted),
                        )
                        .child(
                            div()
                                .row()
                                .justify_end()
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
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(560.0))
        .h(px(320.0))
        // An open surface is an overlay and takes no room in the flow, so the
        // scene reserves the panel's review area explicitly.
        .child(div().h(px(300.0)).child(menu))
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

pub(super) fn toast(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneToasts>() {
        let layer = cx.new(|cx| ToastLayer::new(window, cx).capacity(4));
        cx.set_global(SceneToasts { layer });
        toast_push(
            window,
            cx,
            Toast::new("scene.toast.saved", "Theme exported to disk").tone(Tone::Success),
        );
        toast_push(
            window,
            cx,
            Toast::new("scene.toast.stale", "Refreshing the model catalog failed")
                .tone(Tone::Warning)
                .detail("The last verified catalog is still shown."),
        );
        toast_push(
            window,
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
            window,
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
    // What is behind the glass is a page, not a test pattern. A block of
    // accent stripes dropped into a panel reads as a hole cut in it, and it
    // answers the wrong question anyway: the thing a reader has to be able to
    // judge is whether text they can otherwise read has gone out of focus.
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
                .child(filler(&theme, "Document", 8))
                .child(
                    div()
                        .absolute()
                        .top(px(48.0))
                        // Far enough left to cross the lines. Placed clear of
                        // them the glass had nothing behind it, so the picture
                        // could not answer the one question it is here for:
                        // whether text a reader can otherwise read has gone
                        // out of focus.
                        .left(px(64.0))
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
        .child(caption(
            &theme,
            "The same glass, blurred further and raised above the panel",
        ))
        .child(
            div()
                .relative()
                .h(px(160.0))
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(filler(&theme, "Files", 6))
                .child(
                    div()
                        .absolute()
                        .top(px(32.0))
                        .left(px(64.0))
                        .w(px(280.0))
                        .child(
                            Frost::new("scene.frost.rail")
                                // A surface a step above the page it covers.
                                // Tinted with the page's own colour the glass
                                // had no surface of its own: at any alpha, a
                                // fill the colour of what is behind it adds
                                // nothing, and the rail's words landed
                                // directly on the words of the file list.
                                .surface(gpui_kit_theme::Surface::Raised)
                                .radius(Radius::Dialog)
                                .blur(32.0)
                                .child(card(
                                    "Rail",
                                    "The lines behind stay legible and out of focus",
                                )),
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
/// One square of the backdrop every optics plate is read against, and the two
/// plate footprints built from it. Both are whole numbers of squares.
const TILE: f32 = 48.0;
const PLATE_WIDTH: f32 = TILE * 10.0;
const PLATE_HEIGHT: f32 = TILE * 3.0;
const JOIN_WIDTH: f32 = TILE * 9.0;

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
            .child(crate::foundation::text(
                &theme,
                TypeScale::Caption,
                SharedString::from(body),
            ))
    };

    // A ruled checkerboard bends far more legibly than a flat fill: its
    // one-pixel lines reveal whether the refracted rim retained the sharp
    // snapshot while Frosted scattered the interior. It is drawn in neutrals,
    // and every plate that carries one is an exact
    // number of squares across and down: a board cut through the middle of a
    // square at the plate edge reads as a broken pattern rather than as the
    // ruled backdrop the optics are being measured against.
    let checker_light = theme.colors.text_faint;
    let checker_dark = theme.colors.canvas;
    let checker_rule = theme.colors.divider.opacity(0.55);
    let checkerboard =
        |width: f32, height: f32| {
            let columns = (width / TILE).ceil() as usize;
            let rows = (height / TILE).ceil() as usize;
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
                        .h(px(TILE))
                        .children((0..columns).map(move |column| {
                            div().flex_none().w(px(TILE)).h(px(TILE)).bg(
                                if (row + column) % 2 == 0 {
                                    checker_light
                                } else {
                                    checker_dark
                                },
                            )
                        }))
                }))
                .children((0..=(width / 12.0) as usize).map(|column| {
                    div()
                        .absolute()
                        .top(px(0.0))
                        .left(px(column as f32 * 12.0))
                        .w(px(1.0))
                        .h(px(height))
                        .bg(checker_rule)
                }))
                .children((0..=(height / 12.0) as usize).map(|row| {
                    div()
                        .absolute()
                        .top(px(row as f32 * 12.0))
                        .left(px(0.0))
                        .w(px(width))
                        .h(px(1.0))
                        .bg(checker_rule)
                }))
        };

    let plate =
        |ident: &'static str, preset: GlassPreset, title: &'static str, body: &'static str| {
            div()
                .relative()
                .h(px(PLATE_HEIGHT))
                .w(px(PLATE_WIDTH))
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(checkerboard(PLATE_WIDTH, PLATE_HEIGHT))
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
        .child(caption(
            &theme,
            "Lens: clear inside, the edge bends the sharp backdrop",
        ))
        .child(plate(
            "scene.glass.lens",
            GlassPreset::Lens,
            "Lens",
            "The one-pixel rules bend at the rim and stay sharp",
        ))
        .child(caption(
            &theme,
            "Liquid: clear refraction, additive lift, and a lit hairline",
        ))
        .child(plate(
            "scene.glass.liquid",
            GlassPreset::Liquid,
            "Liquid",
            "Subtle dispersion without turning the backdrop into a smear",
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
                                .h(px(PLATE_HEIGHT))
                                .w(px(JOIN_WIDTH))
                                .surface(&theme, Surface::Panel)
                                .radius(&theme, Radius::Card)
                                .overflow_hidden()
                                .child(checkerboard(JOIN_WIDTH, PLATE_HEIGHT))
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
                                .h(px(PLATE_HEIGHT))
                                .w(px(JOIN_WIDTH))
                                .surface(&theme, Surface::Panel)
                                .radius(&theme, Radius::Card)
                                .overflow_hidden()
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(0.0))
                                        .left(px(0.0))
                                        .w(px(JOIN_WIDTH / 2.0))
                                        .h(px(PLATE_HEIGHT))
                                        .bg(gpui::white()),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(0.0))
                                        .left(px(JOIN_WIDTH / 2.0))
                                        .w(px(JOIN_WIDTH / 2.0))
                                        .h(px(PLATE_HEIGHT))
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
                                        .left(px(JOIN_WIDTH / 2.0 + 15.0))
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
                    let group = |heading: &'static str, rows: gpui::Div| {
                        div()
                            .column()
                            .gap(px(theme.space(Space::Sm)))
                            .child(
                                crate::foundation::text(&theme, TypeScale::Label, heading)
                                    .text_tone(&theme, TextTone::Muted),
                            )
                            .child(rows.column().gap(px(theme.space(Space::Sm))))
                    };
                    div()
                        .column()
                        .gap(px(theme.space(Space::Lg)))
                        .child(group(
                            "Outcome",
                            div()
                                .child(
                                    Checkbox::new("scene.drawer.failed")
                                        .label("Failed runs only")
                                        .checked(true)
                                        .on_change(|_, _, _| {}),
                                )
                                .child(
                                    Checkbox::new("scene.drawer.cancelled")
                                        .label("Include cancelled")
                                        .on_change(|_, _, _| {}),
                                ),
                        ))
                        .child(group(
                            "Ownership",
                            div()
                                .child(
                                    Checkbox::new("scene.drawer.mine")
                                        .label("Started by me")
                                        .on_change(|_, _, _| {}),
                                )
                                .child(
                                    Checkbox::new("scene.drawer.watching")
                                        .label("Repositories I watch")
                                        .checked(true)
                                        .on_change(|_, _, _| {}),
                                ),
                        ))
                        .child(group(
                            "Window",
                            div()
                                .child(
                                    Checkbox::new("scene.drawer.today")
                                        .label("Today")
                                        .checked(true)
                                        .on_change(|_, _, _| {}),
                                )
                                .child(
                                    Checkbox::new("scene.drawer.week")
                                        .label("This week")
                                        .on_change(|_, _, _| {}),
                                ),
                        ))
                        .into_any_element()
                })
                // Two controls, sized to their labels and pinned to the
                // reading edge: a full-width primary slab is the loudest
                // thing on the page and says the drawer has one exit.
                .footer(|_, cx| {
                    let theme = cx.theme().clone();
                    div()
                        .row()
                        .justify_end()
                        .gap(px(theme.space(Space::Sm)))
                        .child(
                            Button::new("scene.drawer.cancel")
                                .label("Cancel")
                                .secondary()
                                .on_click(|_, _| {}),
                        )
                        .child(
                            Button::new("scene.drawer.apply")
                                .label("Apply")
                                .primary()
                                .on_click(|_, _| {}),
                        )
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
        .child(page_behind(&theme, "Runs"))
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
