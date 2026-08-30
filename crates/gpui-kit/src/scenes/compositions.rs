//! Arrangements, not exhibits.
//!
//! Each of these is built the way a product would build it, and is kept
//! because components interact in ways none of them shows alone. None of
//! them is anybody's coverage: every component they draw is reviewed in
//! its own family.

use super::support::*;

/// The same vocabulary an interface uses everywhere, rendered right to left.
///
/// The point of capturing this is that support for another reading direction
/// is a claim about pixels, and a claim about pixels is only checkable in a
/// picture. Everything here is a component that already existed; nothing was
/// drawn specially for the scene. What should have moved: the trail runs from
/// the right, the tree indents from the right and its shut chevrons point
/// left, the accordion header reads from the right, and the magnifier's
/// handle changed corners. What should not have moved: the checkmark, the
/// gear, and the downward chevron, because none of them mean "forward".
pub(super) fn reading_direction(_window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let direction = cx.layout_direction();
    stack(&theme)
        .w(px(560.0))
        .child(
            Breadcrumb::new("scene.rtl.trail")
                .crumbs([
                    Crumb::new("workspace", "Workspace"),
                    Crumb::new("runs", "Runs"),
                    Crumb::new("run-4821", "Indexing"),
                ])
                .on_select(|_, _, _| {}),
        )
        .child(
            Tabs::new("scene.rtl.tabs")
                .tabs([
                    TabItem::new("overview", "Overview").icon(Icon::Widget),
                    TabItem::new("runs", "Runs").badge("12"),
                    TabItem::new("logs", "Logs"),
                ])
                .selected("runs")
                .on_select(|_, _, _| {}),
        )
        .child(
            Card::new()
                .id("scene.rtl.mixed")
                .child(
                    crate::foundation::text(
                        &theme,
                        TypeScale::Body,
                        "الإصدار v2.4 — build 17 — גרסה יציבה",
                    )
                    .text_start(direction),
                )
                .child(
                    crate::foundation::text(
                        &theme,
                        TypeScale::Caption,
                        "المسار /workspace/run-4821، 64% مكتمل",
                    )
                    .text_start(direction)
                    .text_tone(&theme, TextTone::Muted),
                ),
        )
        .child(
            div()
                .row_reading(direction)
                .gap(px(theme.space(Space::Md)))
                .items_start()
                .child(
                    div().w(px(240.0)).child(
                        Tree::new("scene.rtl.tree")
                            .expanded_ids(&["workspace"])
                            .selected("tokens")
                            .nodes([TreeNode::new("workspace", "workspace")
                                .icon(Icon::Folder)
                                .children([
                                    TreeNode::new("crates", "crates")
                                        .icon(Icon::Folder)
                                        .children([
                                            TreeNode::new("kit", "gpui-kit").icon(Icon::Document)
                                        ]),
                                    TreeNode::new("tokens", "tokens").icon(Icon::Document),
                                ])])
                            .on_toggle(|_, _, _, _| {})
                            .on_select(|_, _, _| {}),
                    ),
                )
                .child(
                    div().flex_1().child(
                        Accordion::new("scene.rtl.sections")
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
                                    .description("Where verified results are kept"),
                            ),
                    ),
                ),
        )
        .child(
            JsonView::new(
                "scene.rtl.json",
                JsonValue::object([
                    ("الحالة", JsonValue::string("جاهز build-17")),
                    ("גרסה", JsonValue::string("v2.4 مستقرة")),
                ]),
            )
            .root_label("تفاصيل التشغيل")
            .selected("الحالة")
            .on_select(|_, _, _| {}),
        )
        .child(
            div()
                .row_reading(direction)
                .gap(px(theme.space(Space::Md)))
                // Directional glyphs on the top row, fixed ones below the
                // names, so a reviewer can see which turned around.
                .child(IconView::new(Icon::Magnifier).large().muted())
                .child(IconView::new(Icon::AltArrowRight).large().muted())
                .child(IconView::new(Icon::Return).large().muted())
                .child(IconView::new(Icon::Check).large().tone(IconTone::Success))
                .child(IconView::new(Icon::Settings).large().muted())
                .child(IconView::new(Icon::AltArrowDown).large().muted())
                .child(
                    IconView::named("scene.rtl.alone", Icon::Danger, "Run failed")
                        .large()
                        .tone(IconTone::Danger),
                ),
        )
        .child(
            div()
                .row_reading(direction)
                .gap(px(theme.space(Space::Sm)))
                .child(
                    Button::new("scene.rtl.save")
                        .label("Save")
                        .primary()
                        .icon(Icon::Check)
                        .on_click(|_, _| {}),
                )
                .child(
                    Button::new("scene.rtl.cancel")
                        .label("Cancel")
                        .secondary()
                        .on_click(|_, _| {}),
                ),
        )
        .into_any_element()
}

/// The order the reorder scene is currently showing.
///
/// A still frame cannot show a slide, so the capture is the settled list and
/// the button is what a reviewer presses to watch a row travel.
#[derive(Debug)]
pub(super) struct SceneQueue {
    steps: Vec<(&'static str, &'static str)>,
}

impl Global for SceneQueue {}

pub(super) fn motion_flip(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneQueue>() {
        cx.set_global(SceneQueue {
            steps: vec![
                ("render", "Render frames"),
                ("upload", "Upload artifacts"),
                ("verify", "Verify checksums"),
                ("publish", "Publish release"),
            ],
        });
    }
    let steps = cx.global::<SceneQueue>().steps.clone();
    let theme = cx.theme().clone();

    let mut queue = Card::new().id("scene.motion.queue").divided(true);
    for (index, (id, label)) in steps.iter().enumerate() {
        let ident = format!("scene.motion.{id}");
        let handle = flip(ident.clone(), window, cx);
        // The position is what this scene is about, so it leads the row in a
        // column of its own rather than trailing as a badge: a number that
        // moves is only readable against the numbers above and below it.
        let position = div()
            .flex_none()
            .size(px(theme
                .control
                .get(gpui_kit_theme::ControlSize::Xs)
                .height))
            .flex()
            .items_center()
            .justify_center()
            .radius(&theme, Radius::Small)
            .well(&theme)
            .mono(&theme)
            .type_scale(&theme, TypeScale::Caption)
            .text_color(theme.colors.text)
            .child(format!("{}", index + 1));
        queue = queue.child(
            ListRow::new()
                .id(ident)
                .leading(position)
                .child(div().flex_1().child(*label))
                .flip(&handle, window, cx),
        );
    }

    stack(&theme)
        .w(px(420.0))
        .child(
            row(&theme).child(
                Button::new("scene.motion.reorder")
                    .label("Move the last step first")
                    .secondary()
                    .on_click(|_, cx| {
                        cx.update_global::<SceneQueue, ()>(|queue, _| queue.steps.rotate_right(1));
                        cx.refresh_windows();
                    }),
            ),
        )
        .child(queue)
        .child(
            crate::foundation::text(
                &theme,
                TypeScale::Body,
                "Rows land in their new slot at once and slide into it.",
            )
            .text_tone(&theme, TextTone::Muted),
        )
        .into_any_element()
}

/// Which way every state in the state-transition scene is currently pointing.
///
/// One flag drives all of them, so a reviewer flips the whole row at once and
/// watches a check draw, a knob slide, an indicator travel and a section open
/// on the same frame.
#[derive(Debug)]
pub(super) struct SceneStates {
    forward: bool,
}

impl Global for SceneStates {}

pub(super) fn motion_state(_window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<SceneStates>() {
        cx.set_global(SceneStates { forward: false });
    }
    let forward = cx.global::<SceneStates>().forward;
    let theme = cx.theme().clone();

    stack(&theme)
        .w(px(460.0))
        .child(
            row(&theme).child(
                Button::new("scene.state.flip")
                    .label("Flip every state")
                    .secondary()
                    .on_click(|_, cx| {
                        cx.update_global::<SceneStates, ()>(|state, _| {
                            state.forward = !state.forward
                        });
                        cx.refresh_windows();
                    }),
            ),
        )
        .child({
            let terms = Checkbox::new("scene.state.terms").label("Accept the terms");
            if forward {
                terms.checked(true)
            } else {
                terms.mixed()
            }
            .on_change(|_, _, _| {})
        })
        .child(
            Radio::new("scene.state.plan")
                .label("Bill monthly")
                .selected(forward)
                .on_select(|_, _| {}),
        )
        .child(
            Switch::new("scene.state.notify")
                .label("Send run notifications")
                .on(forward)
                .on_change(|_, _, _| {}),
        )
        .child(
            SegmentedControl::new("scene.state.view")
                .segments(vec![
                    Segment::new("list", "List"),
                    Segment::new("grid", "Grid"),
                ])
                .selected(if forward { "grid" } else { "list" })
                .on_select(|_, _, _| {}),
        )
        .child(
            Tabs::new("scene.state.tabs")
                .tabs(vec![
                    TabItem::new("overview", "Overview"),
                    TabItem::new("runs", "Runs"),
                ])
                .selected(if forward { "runs" } else { "overview" })
                .on_select(|_, _, _| {}),
        )
        .child(
            ProgressBar::new("scene.state.progress")
                .label("Uploading artifacts")
                .fraction(if forward { 0.86 } else { 0.12 }),
        )
        .child(
            Accordion::new("scene.state.sections")
                .expanded_ids(if forward { &["retention"][..] } else { &[][..] })
                .on_toggle(|_, _, _, _| {})
                .section(
                    AccordionSection::new("retention", "Retention")
                        .description("How long verified results are kept")
                        .body(crate::foundation::text(&theme, TypeScale::Body, "Results are kept in the workspace for 30 days.")),
                ),
        )
        .child(crate::foundation::text(&theme, TypeScale::Body, "Every state settles within a fifth of a second; the values are published the moment they change.").text_tone(&theme, TextTone::Muted))
        .into_any_element()
}
