//! Getting to a place, and knowing which place you are in.

use super::support::*;

pub(super) fn bottom_navigation(_window: &mut Window, cx: &mut App) -> AnyElement {
    use crate::navigation::{BottomNavigation, NavigationItem};
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(390.0))
        .child(caption(
            &theme,
            "Fixture destinations; selected, enabled and disabled actions",
        ))
        .child(
            BottomNavigation::new("scene.bottom-navigation.ready")
                .items([
                    NavigationItem::new("library", "Library"),
                    NavigationItem::new("search", "Search"),
                    NavigationItem::new("settings", "Settings").disabled(true),
                ])
                .selected("search")
                .on_select(|_, _, _| {}),
        )
        .child(caption(
            &theme,
            "No host handler: destinations are unavailable",
        ))
        .child(
            BottomNavigation::new("scene.bottom-navigation.unavailable")
                .items([
                    NavigationItem::new("library", "Library"),
                    NavigationItem::new("search", "Search"),
                ])
                .selected("library"),
        )
        .into_any_element()
}

pub(super) fn adaptive_navigation(_window: &mut Window, cx: &mut App) -> AnyElement {
    use crate::layout::{AppBar, PageLayout, Responsive};
    use crate::navigation::{BottomNavigation, NavigationItem};
    let theme = cx.theme().clone();
    let make = |id: &'static str, width: f32| {
        div().w(px(width)).h(px(320.0)).child(
            Responsive::new(id, move |size, _, _| {
                let wide = size.at_least(600.0);
                let body = div()
                    .flex()
                    .size_full()
                    .when(wide, |body| {
                        body.child(
                            Sidebar::new(format!("{id}.rail"))
                                .section(SidebarSection::new("destinations").items([
                                    SidebarItem::new("library", "Library"),
                                    SidebarItem::new("search", "Search"),
                                ]))
                                .active("search")
                                .on_select(|_, _, _| {}),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .p(px(16.0))
                            .child("Fixture content · caller owns route selection"),
                    );
                PageLayout::new(format!("{id}.page"), body)
                    .insets(gpui::WindowInsets::default())
                    .header(AppBar::new(
                        format!("{id}.bar"),
                        if wide {
                            "Wide container"
                        } else {
                            "Compact container"
                        },
                    ))
                    .hide_footer(wide)
                    .footer(
                        BottomNavigation::new(format!("{id}.bottom"))
                            .items([
                                NavigationItem::new("library", "Library"),
                                NavigationItem::new("search", "Search"),
                            ])
                            .selected("search")
                            .on_select(|_, _, _| {}),
                    )
                    .into_any_element()
            })
            .fill(),
        )
    };
    stack(&theme)
        .child(caption(&theme, "Same caller data; measured container width selects rail or bottom destinations. No platform guessing."))
        .child(make("scene.adaptive-navigation.wide", 640.0))
        .child(make("scene.adaptive-navigation.compact", 350.0))
        .into_any_element()
}

pub(super) fn nav_back_preview(window: &mut Window, cx: &mut App) -> AnyElement {
    use crate::navigation::{BackTransition, NavHistory, NavStack};
    let theme = cx.theme().clone();
    let state = window.use_keyed_state("scene.nav-back-preview.state", cx, |_, cx| {
        let mut history = NavHistory::new("overview");
        history.push("details");
        (history, None::<BackTransition>, cx.focus_handle())
    });
    let (history, preview, focus) = state.read(cx).clone();
    let progress = state.clone();
    let cancel = state.clone();
    let commit = state.clone();
    let reset = state.clone();
    let mut page = NavStack::new(
        "scene.nav-back-preview",
        &history,
        history.current().clone(),
        focus,
        Card::new()
            .padding(Space::Xl)
            .child(history.current().clone()),
    );
    if let Some(preview) = &preview {
        page = page.back_transition(preview);
    }
    stack(&theme).w(px(550.0))
        .child(caption(&theme, "Fixture host-fed progress; cancel retains the page, accept mutates caller history. Reduced motion holds full opacity."))
        .child(Slider::new("scene.nav-back-preview.progress").label("Back progress")
            .range(0.0, 1.0).value(preview.as_ref().map_or(0.0, BackTransition::progress))
            .disabled(!history.can_pop()).on_change(move |value, _, cx| progress.update(cx, |state, cx| {
                if state.1.is_none() { state.1 = BackTransition::begin(&state.0); }
                if let Some(preview) = &mut state.1 { preview.update(value); }
                cx.notify();
            })))
        .child(row(&theme)
            .child(Button::new("scene.nav-back-preview.cancel").label("Cancel").disabled(preview.is_none())
                .on_click(move |_, cx| cancel.update(cx, |state, cx| {
                    if let Some(preview) = state.1.take() { preview.cancel(); } cx.notify();
                })))
            .child(Button::new("scene.nav-back-preview.accept").label("Accept back").disabled(preview.is_none())
                .on_click(move |_, cx| commit.update(cx, |state, cx| {
                    if let Some(preview) = state.1.take() { preview.commit(&mut state.0); } cx.notify();
                })))
            .child(Button::new("scene.nav-back-preview.reset").label("Forward").disabled(!history.can_forward())
                .on_click(move |_, cx| reset.update(cx, |state, cx| { state.0.forward(); cx.notify(); }))))
        .child(page).into_any_element()
}

pub(super) fn nav_stack(window: &mut Window, cx: &mut App) -> AnyElement {
    use crate::navigation::nav_stack::{NavHistory, NavStack};
    let theme = cx.theme().clone();
    let state = window.use_keyed_state("scene.nav-stack.state", cx, |_, cx| {
        (
            NavHistory::new("overview"),
            cx.focus_handle(),
            cx.focus_handle(),
            cx.focus_handle(),
        )
    });
    let (history, overview, details, replacement) = state.read(cx).clone();
    let focus = match history.current().as_ref() {
        "details" => details,
        "replacement" => replacement,
        _ => overview,
    };
    let back = state.clone();
    let forward = state.clone();
    let push = state.clone();
    let replace = state.clone();
    stack(&theme).w(px(620.0))
        .child(caption(&theme, "Caller-owned visit history; back retains forward visits, push forks, replace preserves branches"))
        .child(row(&theme)
            .child(Button::new("scene.nav-stack.back").label("Back").disabled(!history.can_pop())
                .on_click(move |_, cx| back.update(cx, |state, cx| { state.0.pop(); cx.notify(); })))
            .child(Button::new("scene.nav-stack.forward").label("Forward").disabled(!history.can_forward())
                .on_click(move |_, cx| forward.update(cx, |state, cx| { state.0.forward(); cx.notify(); })))
            .child(Button::new("scene.nav-stack.push").label("Push details")
                .disabled(history.entries().iter().any(|id| id.as_ref() == "details"))
                .on_click(move |_, cx| push.update(cx, |state, cx| { state.0.push("details"); cx.notify(); })))
            .child(Button::new("scene.nav-stack.replace").label("Replace")
                .disabled(history.entries().iter().any(|id| id.as_ref() == "replacement"))
                .on_click(move |_, cx| replace.update(cx, |state, cx| { state.0.replace("replacement"); cx.notify(); }))))
        .child(NavStack::new("scene.nav-stack", &history, history.current().clone(), focus,
            Card::new().padding(Space::Xl)
                .child(crate::foundation::text(&theme, TypeScale::Title, history.current().clone()))
                .child(crate::foundation::text(&theme, TypeScale::Body,
                    "Only the active page is mounted. Return visits restore focus without keeping inactive controls live."))))
        .into_any_element()
}

pub(super) fn carousel(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let page = |id: &'static str, label: &'static str, text: &'static str| {
        CarouselItem::new(
            id,
            label,
            Card::new()
                .id(format!("scene.carousel.card.{id}"))
                .padding(Space::Xl)
                .child(crate::foundation::text(&theme, TypeScale::Title, label))
                .child(crate::foundation::text(&theme, TypeScale::Body, text)),
        )
    };
    stack(&theme)
        .w(px(620.0))
        .child(caption(
            &theme,
            "A controlled, focusable page track with direct selection and reduced-motion-aware transitions",
        ))
        .child(
            Carousel::new("scene.carousel.ready")
                .items([
                    page("overview", "Overview", "The first page is active."),
                    page("details", "Details", "The caller owns the page content."),
                    page("history", "History", "The track reports page intents."),
                ])
                .active("details")
                .looped(true)
                .on_event(|_, _, _| {}),
        )
        .child(
            Carousel::new("scene.carousel.empty")
                .phase(Phase::Empty)
                .on_event(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn tabs(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(caption(
            &theme,
            "In-content tabs: raised control, quiet track, tabular count",
        ))
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
        .child(caption(
            &theme,
            "places, not documents: Regular Liquid capsules, \
             with a translucent selected fill on the material",
        ))
        .child(
            Tabs::new("scene.tabs.capsules")
                .capsules()
                .control_size(ControlSize::Sm)
                .tabs([
                    TabItem::new("code", "Portal")
                        .icon(Icon::Terminal)
                        .tint(theme.colors.accent),
                    TabItem::new("game", "Harbour")
                        .icon(Icon::Play)
                        .tint(theme.colors.warning),
                    TabItem::new("design", "notes")
                        .icon(Icon::Pen)
                        .tint(theme.colors.info),
                ])
                .selected("code")
                .on_select(|_, _, _| {}),
        )
        // The body belongs to the caller: tabs render the strip only.
        .child(crate::foundation::text(
            &theme,
            TypeScale::Body,
            "Runs are rendered by the caller, not by the strip.",
        ))
        .child(caption(
            &theme,
            "the other answer to a strip with no room: it scrolls, and the \
             edge with more behind it fades",
        ))
        // Deliberately narrower than its tabs, because a strip that fits shows
        // nothing about what a strip that does not fit does.
        .child(
            div().w(px(300.0)).child(
                Tabs::new("scene.tabs.spaces")
                    .tabs((1..=9).map(|n| {
                        TabItem::new(format!("space-{n}"), format!("Workspace {n}"))
                            .icon(Icon::Widget)
                    }))
                    .selected("space-5")
                    .scrolling()
                    .on_select(|_, _, _| {}),
            ),
        )
        .into_any_element()
}

/// The overflow menu the document-tab scene hangs off the strip.
///
/// The strip does not own it: a menu is an entity with an open state that
/// outlives a frame, so the caller builds it and the strip fills it.
pub(super) struct SceneDocumentTabs {
    overflow: Entity<Menu>,
}

impl Global for SceneDocumentTabs {}

pub(super) fn document_tabs(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneDocumentTabs>() {
        // An icon-only trigger still has to be nameable, so it carries the
        // strip's own word for what moved into it.
        let name = cx.strings().text(StringKey::TabMoreTabs);
        let overflow = cx.new(|cx| {
            Menu::new("scene.document-tabs.overflow", window, cx)
                .trigger_icon(Icon::AltArrowDown)
                .trigger_name(name)
        });
        cx.set_global(SceneDocumentTabs { overflow });
    }
    let overflow = cx.global::<SceneDocumentTabs>().overflow.clone();
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(620.0))
        .child(caption(
            &theme,
            "clean, unsaved, saving, save failed — three marks, and silence for the fourth",
        ))
        .child(
            Tabs::new("scene.document-tabs.editor")
                .tabs([
                    TabItem::new("readme", "README.md").closable(true),
                    TabItem::new("main", "main.rs").dirty().closable(true),
                    TabItem::new("theme", "theme.json").saving().closable(true),
                    TabItem::new("notes", "notes.md")
                        .save_failed("The workspace is read-only.")
                        .closable(true),
                ])
                .selected("main")
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {}),
        )
        .child(caption(
            &theme,
            "past the declared limit the rest go to a menu, which stays reachable from the keyboard",
        ))
        .child(
            Tabs::new("scene.document-tabs.overflowing")
                .tabs([
                    TabItem::new("one", "adapter.rs").closable(true),
                    TabItem::new("two", "catalog.rs").dirty().closable(true),
                    TabItem::new("three", "harness.rs").closable(true),
                    TabItem::new("four", "registry.rs").closable(true),
                    TabItem::new("five", "transport.rs").dirty().closable(true),
                ])
                .selected("two")
                .overflow_after(3)
                .overflow_menu(overflow)
                .on_select(|_, _, _| {})
                .on_close(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) struct SceneAnchorList {
    overflow: Entity<Menu>,
}

impl Global for SceneAnchorList {}

pub(super) fn anchor_list(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneAnchorList>() {
        let label = cx.strings().text(StringKey::AnchorMoreSections);
        let overflow = cx.new(|cx| Menu::new("scene.anchor-list.menu", window, cx).trigger(label));
        cx.set_global(SceneAnchorList { overflow });
    }
    let theme = cx.theme().clone();
    let overflow = cx.global::<SceneAnchorList>().overflow.clone();
    stack(&theme)
        .w(px(700.0))
        .child(caption(
            &theme,
            "section intents only; the declared overflow moves anchors into a menu",
        ))
        .child(
            AnchorList::new("scene.anchor-list")
                .anchors([
                    Anchor::new("summary", "Summary"),
                    Anchor::new("inputs", "Inputs"),
                    Anchor::new("constraints", "Constraints"),
                    Anchor::new("verification", "Verification"),
                    Anchor::new("history", "History").disabled(true),
                ])
                .active("inputs")
                .overflow_after(3)
                .overflow_menu(overflow)
                .on_navigate(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn accordion(_window: &mut Window, cx: &mut App) -> AnyElement {
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
                        .body(crate::foundation::text(
                            &theme,
                            TypeScale::Body,
                            "Requests go out over the system proxy.",
                        )),
                )
                .section(
                    AccordionSection::new("storage", "Storage")
                        .description("Where verified results are kept")
                        .body(crate::foundation::text(
                            &theme,
                            TypeScale::Body,
                            "Nothing is written outside the workspace.",
                        )),
                )
                .section(
                    AccordionSection::new("policy", "Managed by policy")
                        .description("This machine cannot change these")
                        .disabled(true)
                        .body(crate::foundation::text(
                            &theme,
                            TypeScale::Body,
                            "Set by the administrator.",
                        )),
                ),
        )
        // The open section is the last one, which is the case where the body
        // meets the card's rounded corners.
        .child(
            Accordion::new("scene.accordion.release")
                .expanded_ids(&["publish"])
                .on_toggle(|_, _, _, _| {})
                .section(
                    AccordionSection::new("build", "Build")
                        .description("What the last run produced"),
                )
                .section(
                    AccordionSection::new("publish", "Publish")
                        .description("Where the signed bundle goes")
                        .body(crate::foundation::text(
                            &theme,
                            TypeScale::Body,
                            "The bundle is published to the workspace channel.",
                        )),
                ),
        )
        .into_any_element()
}

pub(super) fn breadcrumb(_window: &mut Window, cx: &mut App) -> AnyElement {
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

pub(super) fn navigation_sections() -> Vec<SidebarSection> {
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

pub(super) fn sidebar(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let rail = |ident: &'static str, collapsed: bool| {
        Sidebar::new(ident)
            .sections(navigation_sections())
            .active("runs.active")
            .collapsed(collapsed)
            .footer(
                crate::foundation::text(
                    &theme,
                    TypeScale::Caption,
                    if collapsed {
                        SharedString::new_static("v0")
                    } else {
                        SharedString::new_static("Fixture workspace")
                    },
                )
                .text_tone(&theme, TextTone::Faint),
            )
            .on_select(|_, _, _| {})
    };

    stack(&theme)
        .h(px(420.0))
        .child(
            div()
                .flex()
                .flex_row()
                .h(px(360.0))
                .gap(px(theme.space(Space::Lg)))
                .child(rail("scene.sidebar.expanded", false))
                .child(rail("scene.sidebar.collapsed", true)),
        )
        .into_any_element()
}

/// The page-size control of the pagination scene, kept across frames.
pub(super) struct ScenePagination {
    page_size: Entity<Select>,
}

impl Global for ScenePagination {}

pub(super) fn pagination(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<ScenePagination>() {
        let page_size = cx.new(|cx| {
            Select::new("scene.pagination.size", window, cx)
                .name("Rows per page")
                .options([
                    SelectOption::new("25", "25 per page"),
                    SelectOption::new("50", "50 per page"),
                    SelectOption::new("100", "100 per page"),
                ])
                .selected("50")
        });
        cx.set_global(ScenePagination { page_size });
    }
    let page_size = cx.global::<ScenePagination>().page_size.clone();
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(620.0))
        .child(caption(
            &theme,
            "a known total, with the page size the host offered",
        ))
        .child(
            Pagination::new("scene.pagination.known")
                .page(9)
                .total_pages(20)
                .page_size(page_size)
                .on_select(|_, _, _| {}),
        )
        // A host that only knows there is another page says exactly that: no
        // last-page control, no numbers, and no total.
        .child(caption(
            &theme,
            "an unknown total: no numbers, no last page, and no count invented",
        ))
        .child(
            Pagination::new("scene.pagination.unknown")
                .page(3)
                .unknown_total(true)
                .on_select(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn wizard(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(720.0))
        .child(caption(
            &theme,
            "horizontal, with a blocked step and a failed one",
        ))
        .child(
            Wizard::new("scene.wizard.release")
                .steps([
                    WizardStep::new("prepare", "Prepare")
                        .description("Check the workspace is clean")
                        .complete(),
                    WizardStep::new("build", "Build")
                        .description("Compile every target")
                        .failed("The build failed on the test target."),
                    WizardStep::new("sign", "Sign").current(),
                    WizardStep::new("publish", "Publish")
                        .blocked("Approval is required for this workspace."),
                ])
                .body(crate::foundation::text(
                    &theme,
                    TypeScale::Body,
                    SharedString::new_static("The body of the current step belongs to the caller."),
                ))
                .back_to("build")
                .on_navigate(|_, _, _| {}),
        )
        .child(caption(&theme, "vertical, finishing"))
        .child(
            Wizard::new("scene.wizard.setup")
                .vertical()
                .steps([
                    WizardStep::new("account", "Account").complete(),
                    WizardStep::new("workspace", "Workspace").complete(),
                    WizardStep::new("review", "Review").current(),
                ])
                .back_to("workspace")
                .finish(true)
                .on_navigate(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn undo_history(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(caption(
            &theme,
            "the caller owns every revision and whether it can be restored",
        ))
        .child(
            UndoHistory::new("scene.undo-history", "Document undo history")
                .entries([
                    HistoryEntry::new("opened", "Opened the fixture")
                        .description("The initial verified document")
                        .source("Fixture host")
                        .time("10:14"),
                    HistoryEntry::new("renamed", "Renamed the title")
                        .source("Alex")
                        .time("10:16"),
                    HistoryEntry::new("imported", "Imported archived blocks")
                        .description("The archive is no longer available")
                        .source("Archive")
                        .time("10:18")
                        .unavailable("This revision cannot be restored without the archive."),
                    HistoryEntry::new("current", "Reordered the summary")
                        .description("Current document")
                        .source("Alex")
                        .time("10:21"),
                    HistoryEntry::new("draft", "Drafted a new conclusion")
                        .source("Fixture host")
                        .time("10:23"),
                ])
                .current("current")
                .on_jump(|_, _, _| {}),
        )
        .into_any_element()
}

pub(super) fn collapsible(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(520.0))
        .child(
            Collapsible::new("scene.collapsible.open", "Advanced")
                .description("Settings most runs never touch")
                .open(true)
                .body(crate::foundation::text(
                    &theme,
                    TypeScale::Body,
                    "Requests go out over the system proxy.",
                ))
                .on_toggle(|_, _, _| {}),
        )
        .child(
            Collapsible::new("scene.collapsible.shut", "Diagnostics")
                .description("Nothing is collected until this is opened")
                .body(crate::foundation::text(
                    &theme,
                    TypeScale::Body,
                    "This body is absent from the tree while it is shut.",
                ))
                .on_toggle(|_, _, _| {}),
        )
        .child(
            Collapsible::new("scene.collapsible.refused", "Managed by policy")
                .description("This machine cannot change these")
                .disabled(true)
                .body(crate::foundation::text(
                    &theme,
                    TypeScale::Body,
                    "Set by the administrator.",
                )),
        )
        .into_any_element()
}
