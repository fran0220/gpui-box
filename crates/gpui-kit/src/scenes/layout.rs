//! Frames that decide where their contents go.

use super::support::*;

pub(super) fn split_pane(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(620.0))
        .h(px(380.0))
        .child(
            div()
                .h(px(320.0))
                .surface(&theme, Surface::Panel)
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

/// The same region, already scrolled, so the shadow that says there is content
/// above the fold is in a captured image rather than only in a test.
pub(super) fn scroll_shadow(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    crate::layout::scroll_to("scene.scroll.scrolled", gpui::point(px(0.0), px(120.0)), cx);
    stack(&theme)
        .w(px(480.0))
        .child(
            div()
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    ScrollArea::new("scene.scroll.scrolled")
                        .label("Run output")
                        .vertical()
                        .height(200.0)
                        .child(filler(&theme, "Output", 20)),
                ),
        )
        .into_any_element()
}

pub(super) fn scroll_area(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(480.0))
        .child(
            div()
                .surface(&theme, Surface::Panel)
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
                .surface(&theme, Surface::Panel)
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

/// A region scrolled off both ends, so the fade that says there is more in
/// either direction is in a captured image, beside one that hides nothing and
/// therefore fades at neither edge.
pub(super) fn scroll_fade(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    crate::layout::scroll_to("scene.fade.output", gpui::point(px(0.0), px(120.0)), cx);
    stack(&theme)
        .w(px(480.0))
        .child(caption(&theme, "Scrolled: content runs past both ends"))
        .child(
            div()
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    ScrollFade::new("scene.fade.scrolled")
                        .edges(FadeEdges::vertical())
                        .fit_height()
                        .child(
                            ScrollArea::new("scene.fade.output")
                                .label("Run output")
                                .vertical()
                                .height(200.0)
                                .child(filler(&theme, "Output", 20)),
                        ),
                ),
        )
        // Nothing is hidden here, so no edge fades and the caller says so.
        .child(caption(&theme, "Nothing hidden: no edge fades"))
        .child(
            div()
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    ScrollFade::new("scene.fade.settled").fit_height().child(
                        ScrollArea::new("scene.fade.summary")
                            .label("Summary")
                            .vertical()
                            .height(120.0)
                            .child(filler(&theme, "Summary", 2)),
                    ),
                ),
        )
        .into_any_element()
}

/// A complete client titlebar: product content remains clickable inside the
/// drag strip, and platform controls keep their native hit-test identities.
pub(super) fn desktop_titlebar(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(720.0))
        .child(
            div()
                .surface(&theme, Surface::Panel)
                .radius(&theme, Radius::Card)
                .overflow_hidden()
                .child(
                    DesktopTitlebar::new("scene.desktop-titlebar", "Workspace")
                        .subtitle("main.rs")
                        .left(
                            Button::new("scene.desktop-titlebar.workspace")
                                .label("Workspace menu")
                                .ghost()
                                .small()
                                .on_click(|_, _| {}),
                        )
                        .right(
                            Badge::new("Connected")
                                .success()
                                .id("scene.desktop-titlebar.status"),
                        )
                        .on_event(|_, _, _| {}),
                )
                .child(div().h(px(112.0)).p_token(&theme, Space::Lg).child(caption(
                    &theme,
                    "Host content is client input; maximize remains native Snap chrome.",
                ))),
        )
        .into_any_element()
}

/// The overflow menu of the toolbar scene, kept across frames.
pub(super) struct SceneToolbar {
    overflow: Entity<Menu>,
}

impl Global for SceneToolbar {}

pub(super) fn toolbar(window: &mut Window, cx: &mut App) -> AnyElement {
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
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "The last two actions moved into the overflow menu.",
        ))
        .into_any_element()
}

/// A layout nested three deep, with one leaf collapsed to its rail. The tree
/// is the caller's: every divider reports the ratio it was asked for and moves
/// nothing here.
pub(super) fn split_tree(_window: &mut Window, cx: &mut App) -> AnyElement {
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
                .surface(&theme, Surface::Panel)
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
pub(super) fn ide_shell(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let mut branch = AsyncValue::<SharedString, String>::ready("main@a1b2c3".into());
    branch.refresh();
    branch.fail_refresh("the host is unreachable".into());

    div()
        .column()
        .w(px(900.0))
        .h(px(940.0))
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
                        // No badge: a panel that cannot list problems cannot
                        // count them either, and one unavailable reason is the
                        // whole claim. Gluing "there is nothing here" onto it
                        // would report Empty and Unavailable at once.
                        DockPanel::new("problems", "Problems")
                            .icon(Icon::Danger)
                            .unavailable(
                                "The language server is not running, so problems cannot be listed.",
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

pub(super) fn aspect_ratio(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let filled = |label: &'static str| {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.colors.hover)
            .child(
                crate::foundation::text(&theme, TypeScale::Label, label)
                    .text_tone(&theme, TextTone::Muted),
            )
    };
    stack(&theme)
        .child(caption(&theme, "Width given, height from the ratio"))
        .child(
            div().w(px(320.0)).child(
                AspectRatio::of("scene.aspect.wide", 16.0, 9.0)
                    .width_driven()
                    .child(filled("16 by 9")),
            ),
        )
        .child(caption(&theme, "Height given, width from the ratio"))
        .child(
            div().h(px(120.0)).child(
                AspectRatio::new("scene.aspect.square", 1.0)
                    .height_driven()
                    .child(filled("square")),
            ),
        )
        .into_any_element()
}

/// A container that arranges itself from its own measured width.
///
/// Both arrangements are shown at once, because the point of the component is
/// that the caller decides them and neither is a fallback for the other. The
/// unmeasured first frame is a state a still image cannot hold, so it is named
/// rather than drawn.
pub(super) fn responsive(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let arrangement = move |id: &'static str| {
        let theme = theme.clone();
        move |size: ContainerSize, _: &mut Window, cx: &mut App| {
            let wide = size.width().is_some_and(|measured| measured >= 480.0);
            // The unmeasured frame is a real state and it is named rather than
            // guessed, which is the whole argument the component makes.
            let heading = match size.width() {
                None => "Not laid out yet, so neither arrangement is chosen".to_string(),
                Some(measured) if wide => format!("{measured:.0}px wide, so two columns"),
                Some(measured) => format!("{measured:.0}px wide, so one column"),
            };
            let block = |label: &'static str, cx: &mut App| {
                div()
                    .flex_1()
                    .p_token(&theme, Space::Md)
                    .radius(&theme, Radius::Card)
                    .surface(&theme, Surface::Panel)
                    .child(
                        crate::foundation::text(&theme, TypeScale::Label, label)
                            .text_tone(&theme, TextTone::Muted),
                    )
                    .semantic_in(
                        cx,
                        NodeSpec::new(format!("{id}.{}", label.to_lowercase()), Role::Group)
                            .text(label),
                    )
            };
            let panes = div()
                .gap_token(&theme, Space::Sm)
                .child(block("Settings", cx))
                .child(block("Detail", cx));
            div()
                .column()
                .gap_token(&theme, Space::Sm)
                .child(
                    crate::foundation::text(&theme, TypeScale::Caption, heading.clone())
                        .text_tone(&theme, TextTone::Faint),
                )
                .child(if wide { panes.row() } else { panes.column() })
                .semantic_in(cx, NodeSpec::new(id, Role::Group).value(heading))
                .into_any_element()
        }
    };

    let theme = cx.theme().clone();
    stack(&theme)
        .child(caption(&theme, "Wide enough for two columns"))
        .child(div().w(px(620.0)).child(Responsive::new(
            "scene.responsive.wide",
            arrangement("scene.responsive.wide.body"),
        )))
        .child(caption(&theme, "The same content, narrow"))
        .child(div().w(px(320.0)).child(Responsive::new(
            "scene.responsive.narrow",
            arrangement("scene.responsive.narrow.body"),
        )))
        .into_any_element()
}
