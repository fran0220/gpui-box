//! Tabs, accordion, and breadcrumb report where the typist wants to go. None
//! of them decides that they got there.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, SharedString, TestAppContext, div, prelude::*};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Calls<T> = Rc<RefCell<Vec<T>>>;

/// One list, held by the test and by the component's handler.
fn recorder<T: 'static>() -> (Calls<T>, Calls<T>) {
    let calls: Calls<T> = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

fn workspace_tabs() -> Vec<TabItem> {
    vec![
        TabItem::new("overview", "Overview"),
        TabItem::new("runs", "Runs").badge("12"),
        TabItem::new("logs", "Logs").disabled(true),
        TabItem::new("billing", "Billing"),
    ]
}

fn tabs(cx: &mut TestAppContext, selected: &'static str) -> (Harness, Calls<String>) {
    let (calls, sink) = recorder::<String>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Tabs::new("workspace.tabs")
            .tabs(workspace_tabs())
            .selected(selected)
            .on_select(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn clicking_a_tab_reports_it_without_moving_the_selection(cx: &mut TestAppContext) {
    let (mut harness, calls) = tabs(cx, "runs");

    harness.click("workspace.tabs.overview");

    assert_eq!(*calls.borrow(), vec!["overview".to_string()]);
    // The caller owns the selection, so nothing moved.
    assert_eq!(
        harness
            .node("workspace.tabs.runs")
            .expect("published")
            .checked,
        Some(true)
    );
    assert_eq!(
        harness
            .node("workspace.tabs.overview")
            .expect("published")
            .checked,
        Some(false)
    );
}

#[gpui::test]
fn a_tab_publishes_its_label_under_the_strip(cx: &mut TestAppContext) {
    let (mut harness, _calls) = tabs(cx, "runs");
    let node = harness.node("workspace.tabs.runs").expect("published");

    assert_eq!(node.role, Role::Tab);
    assert_eq!(node.text.as_deref(), Some("Runs"));
    assert_eq!(node.parent.as_deref(), Some("workspace.tabs"));
}

#[gpui::test]
fn a_disabled_tab_reports_nothing(cx: &mut TestAppContext) {
    let (mut harness, calls) = tabs(cx, "runs");

    harness.click("workspace.tabs.logs");

    assert!(calls.borrow().is_empty());
    assert!(
        harness
            .node("workspace.tabs.logs")
            .expect("published")
            .disabled
    );
}

#[gpui::test]
fn a_disabled_strip_reports_nothing_at_all(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<String>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Tabs::new("workspace.tabs")
            .tabs(workspace_tabs())
            .selected("runs")
            .disabled(true)
            .on_select(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });

    harness.click("workspace.tabs.overview");
    harness.keystrokes("right");

    assert!(calls.borrow().is_empty());
}

#[gpui::test]
fn the_keyboard_moves_across_tabs_and_skips_refusals(cx: &mut TestAppContext) {
    let (mut harness, calls) = tabs(cx, "runs");

    harness.click("workspace.tabs.runs");
    calls.borrow_mut().clear();
    harness.keystrokes("right");

    // `logs` refuses selection, so the move lands past it.
    assert_eq!(*calls.borrow(), vec!["billing".to_string()]);

    calls.borrow_mut().clear();
    harness.keystrokes("left");
    assert_eq!(*calls.borrow(), vec!["overview".to_string()]);
}

#[gpui::test]
fn home_and_end_move_to_the_ends_of_the_strip(cx: &mut TestAppContext) {
    let (mut harness, calls) = tabs(cx, "runs");

    harness.click("workspace.tabs.runs");
    calls.borrow_mut().clear();
    harness.keystrokes("end");
    assert_eq!(*calls.borrow(), vec!["billing".to_string()]);

    calls.borrow_mut().clear();
    harness.keystrokes("home");
    assert_eq!(*calls.borrow(), vec!["overview".to_string()]);
}

#[gpui::test]
fn a_move_past_the_last_tab_reports_nothing(cx: &mut TestAppContext) {
    let (mut harness, calls) = tabs(cx, "billing");

    harness.click("workspace.tabs.billing");
    calls.borrow_mut().clear();
    harness.keystrokes("right");

    assert!(calls.borrow().is_empty());
}

fn accordion(
    cx: &mut TestAppContext,
    open: &'static [&'static str],
    exclusive: bool,
) -> (Harness, Calls<(String, bool)>) {
    let (calls, sink) = recorder::<(String, bool)>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Accordion::new("settings.sections")
            .expanded_ids(open)
            .exclusive(exclusive)
            .on_toggle(move |id, next, _, _| sink.borrow_mut().push((id.to_string(), next)))
            .section(
                AccordionSection::new("network", "Network").body(
                    Button::new("settings.network.probe")
                        .label("Probe host")
                        .secondary()
                        .on_click(|_, _| {}),
                ),
            )
            .section(
                AccordionSection::new("storage", "Storage")
                    .description("Where verified results are kept")
                    .body(div().child("Nothing leaves the workspace.")),
            )
            .section(
                AccordionSection::new("policy", "Managed by policy")
                    .disabled(true)
                    .body(div().child("Set by the administrator.")),
            )
            .into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn a_collapsed_section_does_not_render_its_body(cx: &mut TestAppContext) {
    let (mut harness, _calls) = accordion(cx, &["storage"], false);

    assert!(
        harness.node("settings.network.probe").is_none(),
        "a closed section must not render its body"
    );
    let header = harness
        .node("settings.sections.network")
        .expect("published");
    assert_eq!(header.expanded, Some(false));
    assert_eq!(header.role, Role::Button);
    assert_eq!(header.text.as_deref(), Some("Network"));
}

#[gpui::test]
fn an_open_section_renders_its_body(cx: &mut TestAppContext) {
    let (mut harness, _calls) = accordion(cx, &["network"], false);

    assert!(harness.node("settings.network.probe").is_some());
    assert_eq!(
        harness
            .node("settings.sections.network")
            .expect("published")
            .expanded,
        Some(true)
    );
}

#[gpui::test]
fn toggling_reports_the_section_and_the_state_it_should_take(cx: &mut TestAppContext) {
    let (mut harness, calls) = accordion(cx, &["network"], false);

    harness.click("settings.sections.storage");
    assert_eq!(*calls.borrow(), vec![("storage".to_string(), true)]);

    calls.borrow_mut().clear();
    harness.click("settings.sections.network");
    assert_eq!(*calls.borrow(), vec![("network".to_string(), false)]);

    // Nothing was applied, so nothing moved.
    assert_eq!(
        harness
            .node("settings.sections.network")
            .expect("published")
            .expanded,
        Some(true)
    );
}

#[gpui::test]
fn an_exclusive_accordion_reports_the_closes_without_showing_them(cx: &mut TestAppContext) {
    let (mut harness, calls) = accordion(cx, &["network"], true);

    harness.click("settings.sections.storage");

    assert_eq!(
        *calls.borrow(),
        vec![
            ("storage".to_string(), true),
            ("network".to_string(), false)
        ]
    );
    // Exclusivity changes the report, never the rendering.
    assert!(harness.node("settings.network.probe").is_some());
}

#[gpui::test]
fn a_disabled_section_installs_no_handler(cx: &mut TestAppContext) {
    let (mut harness, calls) = accordion(cx, &[], false);

    harness.click("settings.sections.policy");

    assert!(calls.borrow().is_empty());
    assert!(
        harness
            .node("settings.sections.policy")
            .expect("published")
            .disabled
    );
}

fn trail() -> Vec<Crumb> {
    vec![
        Crumb::new("workspace", "Workspace"),
        Crumb::new("projects", "Projects"),
        Crumb::new("gpui-kit", "gpui-kit"),
        Crumb::new("runs", "Runs"),
        Crumb::new("indexing", "Indexing"),
    ]
}

#[gpui::test]
fn the_current_place_is_text_rather_than_a_way_to_go_there(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<String>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Breadcrumb::new("nav.trail")
            .crumbs(trail())
            .on_select(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });

    harness.click("nav.trail.indexing");
    assert!(calls.borrow().is_empty());
    assert_eq!(
        harness.node("nav.trail.indexing").expect("published").role,
        Role::Text
    );

    harness.click("nav.trail.projects");
    assert_eq!(*calls.borrow(), vec!["projects".to_string()]);
    assert_eq!(
        harness.node("nav.trail.projects").expect("published").role,
        Role::Link
    );
}

#[gpui::test]
fn a_collapsed_trail_reports_how_many_crumbs_it_hides(cx: &mut TestAppContext) {
    let (calls, sink) = recorder::<Vec<SharedString>>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = sink.clone();
        Breadcrumb::new("nav.trail")
            .crumbs(trail())
            .max_visible(3)
            .on_select(|_, _, _| {})
            .on_reveal(move |hidden, _, _| sink.borrow_mut().push(hidden))
            .into_any_element()
    });

    let collapsed = harness.node("nav.trail.collapsed").expect("published");
    assert_eq!(collapsed.value.as_deref(), Some("2"));
    assert_eq!(collapsed.text.as_deref(), Some("2 hidden levels"));

    assert!(harness.node("nav.trail.workspace").is_some());
    assert!(harness.node("nav.trail.runs").is_some());
    assert!(harness.node("nav.trail.indexing").is_some());
    assert!(
        harness.node("nav.trail.projects").is_none(),
        "a hidden crumb must not stay addressable"
    );

    harness.click("nav.trail.collapsed");
    assert_eq!(
        *calls.borrow(),
        vec![vec![
            SharedString::from("projects"),
            SharedString::from("gpui-kit")
        ]]
    );
}

#[gpui::test]
fn a_short_trail_is_not_collapsed(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        Breadcrumb::new("nav.trail")
            .crumbs(trail().into_iter().take(3))
            .max_visible(3)
            .on_select(|_, _, _| {})
            .into_any_element()
    });

    assert!(harness.node("nav.trail.collapsed").is_none());
    assert!(harness.node("nav.trail.projects").is_some());
}

/// Eight tabs in a frame with room for about three, so the strip genuinely has
/// somewhere to scroll. The selection is held by the test the way a host would
/// hold it, because that is what makes changing it observable.
fn scrolling_strip(cx: &mut TestAppContext) -> (Harness, Rc<RefCell<String>>) {
    let selected = Rc::new(RefCell::new("space-1".to_string()));
    let current = selected.clone();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let selected = current.borrow().clone();
        gpui::div()
            .w(gpui::px(240.0))
            .child(
                Tabs::new("shell.spaces")
                    .tabs(
                        (1..=8)
                            .map(|n| TabItem::new(format!("space-{n}"), format!("Workspace {n}"))),
                    )
                    .selected(selected)
                    .scrolling()
                    .on_select(|_, _, _| {}),
            )
            .into_any_element()
    });
    (harness, selected)
}

#[gpui::test]
fn a_scrolling_strip_still_publishes_every_tab_it_was_given(cx: &mut TestAppContext) {
    // The keyboard steps over all of them, so all of them are named. A tab
    // that is off screen is scrolled away, not withheld.
    let (mut harness, _) = scrolling_strip(cx);

    for n in 1..=8 {
        assert!(
            harness.node(&format!("shell.spaces.space-{n}")).is_some(),
            "workspace {n} is unreachable"
        );
    }
    assert_eq!(
        harness
            .node("shell.spaces")
            .expect("the strip publishes itself")
            .value
            .as_deref(),
        Some("8")
    );
}

#[gpui::test]
fn choosing_a_tab_that_is_off_screen_brings_it_into_view(cx: &mut TestAppContext) {
    let (mut harness, selected) = scrolling_strip(cx);
    let frame = harness.bounds("shell.spaces").expect("the strip is drawn");
    let last = harness
        .bounds("shell.spaces.space-8")
        .expect("the last tab is drawn");
    assert!(
        last.right() > frame.right(),
        "the strip has to actually overflow for this to mean anything: \
         last tab ends at {:?}, frame at {:?}",
        last.right(),
        frame.right()
    );

    *selected.borrow_mut() = "space-8".to_string();
    harness.frame();
    // The scroll is applied in the prepaint after the one that asked for it,
    // so the tab has bounds to be scrolled to.
    harness.frame();

    let shown = harness
        .bounds("shell.spaces.space-8")
        .expect("the last tab is still drawn");
    let frame = harness.bounds("shell.spaces").expect("the strip is drawn");
    assert!(
        shown.right() <= frame.right() + gpui::px(1.0) && shown.left() >= frame.left(),
        "the chosen tab is still outside the strip: tab {shown:?}, strip {frame:?}"
    );
}

#[gpui::test]
fn a_strip_the_reader_scrolled_does_not_haul_itself_back(cx: &mut TestAppContext) {
    // Scrolling on the selection's *change* rather than on the selection is
    // the whole of this: once a tab has been shown, looking somewhere else has
    // to be allowed to stick, or the strip fights every attempt to read it.
    let (mut harness, selected) = scrolling_strip(cx);
    *selected.borrow_mut() = "space-8".to_string();
    harness.frame();
    harness.frame();
    let shown = harness.bounds("shell.spaces.space-8").expect("drawn");

    // Several frames with nothing changing: no drift, and no second scroll.
    for _ in 0..3 {
        harness.frame();
    }

    assert_eq!(
        harness.bounds("shell.spaces.space-8").expect("drawn"),
        shown,
        "the strip moved on its own while the selection stood still"
    );
}

#[gpui::test]
fn the_edge_that_has_more_behind_it_is_the_edge_that_fades(cx: &mut TestAppContext) {
    // "There is more this way" has to be on screen, or the only way to find
    // out is to try scrolling and see whether anything happens.
    let (mut harness, selected) = scrolling_strip(cx);
    assert_eq!(
        harness
            .node("shell.spaces.fade")
            .expect("a scrolling strip fades")
            .value
            .as_deref(),
        Some("right"),
        "at the start there is nothing behind the left edge"
    );

    *selected.borrow_mut() = "space-8".to_string();
    harness.frame();
    harness.frame();
    // The fade is read from the offset the previous frame measured, so the
    // frame that scrolls is not yet the frame that knows it did.
    harness.frame();

    assert_eq!(
        harness
            .node("shell.spaces.fade")
            .expect("a scrolling strip fades")
            .value
            .as_deref(),
        Some("left"),
        "at the far end there is nothing further to reach"
    );
}
