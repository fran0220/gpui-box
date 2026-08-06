//! The palette is the keyboard surface of an application: it filters what the
//! host gave it, reports what was taken, and never hides a command. A query
//! that answers nothing says so about that query, and a command the host
//! refused is shown as refused rather than dropped from the list.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, IntoElement, TestAppContext, div, prelude::*, px};
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

fn commands() -> Vec<Command> {
    vec![
        Command::new("editor.split", "Split editor").section("Editor"),
        Command::new("editor.save", "Save file")
            .section("Editor")
            .shortcut("cmd-s"),
        Command::new("workspace.save", "Save workspace").section("Workspace"),
        Command::new("workspace.publish", "Publish workspace")
            .section("Workspace")
            .unavailable("Approval is required for this workspace"),
    ]
}

fn palette(cx: &mut TestAppContext) -> (Harness, Entity<CommandPalette>) {
    let slot: Rc<RefCell<Option<Entity<CommandPalette>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        let palette = build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    CommandPalette::new("workspace.palette", window, cx).commands(commands())
                })
            })
            .clone();
        div()
            .w(px(600.0))
            .h(px(480.0))
            .child(palette)
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("palette was built");
    (harness, entity)
}

fn events(
    harness: &mut Harness,
    palette: &Entity<CommandPalette>,
) -> Rc<RefCell<Vec<CommandPaletteEvent>>> {
    let seen: Rc<RefCell<Vec<CommandPaletteEvent>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    harness.update({
        let palette = palette.clone();
        move |_, cx| {
            cx.subscribe(&palette, move |_, event: &CommandPaletteEvent, _| {
                sink.borrow_mut().push(event.clone());
            })
            .detach();
        }
    });
    seen
}

fn taken(seen: &Rc<RefCell<Vec<CommandPaletteEvent>>>) -> Vec<String> {
    seen.borrow()
        .iter()
        .filter_map(|event| match event {
            CommandPaletteEvent::Invoked(id) => Some(id.to_string()),
            _ => None,
        })
        .collect()
}

fn type_query(harness: &mut Harness, palette: &Entity<CommandPalette>, query: &str) {
    let palette = palette.clone();
    let query = query.to_string();
    harness.update(move |_, cx| {
        palette.update(cx, |palette, cx| palette.set_query(query, cx));
    });
}

fn focus_query(harness: &mut Harness, palette: &Entity<CommandPalette>) {
    let palette = palette.clone();
    harness.update(move |window, cx| {
        palette.update(cx, |palette, cx| palette.focus_query(window, cx));
    });
}

#[gpui::test]
fn an_untyped_palette_lists_every_command_it_was_given(cx: &mut TestAppContext) {
    let (mut harness, _palette) = palette(cx);

    for id in [
        "workspace.palette.editor.split",
        "workspace.palette.editor.save",
        "workspace.palette.workspace.save",
        "workspace.palette.workspace.publish",
    ] {
        assert!(harness.node(id).is_some(), "`{id}` must be listed");
    }
    assert!(
        harness.node("workspace.palette.empty").is_none(),
        "a palette with commands is not empty"
    );
}

#[gpui::test]
fn typing_filters_the_list_to_what_answers_the_query(cx: &mut TestAppContext) {
    let (mut harness, palette) = palette(cx);
    type_query(&mut harness, &palette, "save");

    assert!(harness.node("workspace.palette.editor.save").is_some());
    assert!(harness.node("workspace.palette.workspace.save").is_some());
    assert!(
        harness.node("workspace.palette.editor.split").is_none(),
        "a command that does not answer the query leaves the list"
    );
}

#[gpui::test]
fn results_stay_grouped_under_the_section_they_belong_to(cx: &mut TestAppContext) {
    let (mut harness, _palette) = palette(cx);

    let heading = harness
        .node("workspace.palette.section.Editor")
        .expect("published");
    assert_eq!(heading.text.as_deref(), Some("Editor"));
    assert!(
        harness
            .node("workspace.palette.section.Workspace")
            .is_some()
    );
}

#[gpui::test]
fn the_highlight_starts_on_the_best_answer_and_moves_with_the_keyboard(cx: &mut TestAppContext) {
    let (mut harness, palette) = palette(cx);
    type_query(&mut harness, &palette, "save");
    focus_query(&mut harness, &palette);

    assert!(
        harness
            .node("workspace.palette.editor.save")
            .expect("published")
            .hovered,
        "the closest match leads"
    );

    harness.keystrokes("down");
    assert!(
        harness
            .node("workspace.palette.workspace.save")
            .expect("published")
            .hovered
    );
}

#[gpui::test]
fn enter_reports_the_highlighted_command(cx: &mut TestAppContext) {
    let (mut harness, palette) = palette(cx);
    let seen = events(&mut harness, &palette);
    type_query(&mut harness, &palette, "save");
    focus_query(&mut harness, &palette);

    harness.keystrokes("down enter");

    assert_eq!(taken(&seen), vec!["workspace.save".to_string()]);
}

#[gpui::test]
fn escape_reports_a_dismissal_and_takes_nothing(cx: &mut TestAppContext) {
    let (mut harness, palette) = palette(cx);
    let seen = events(&mut harness, &palette);
    focus_query(&mut harness, &palette);

    harness.keystrokes("escape");

    assert!(taken(&seen).is_empty());
    assert!(
        seen.borrow().contains(&CommandPaletteEvent::Dismissed),
        "escape reports the intent and leaves the decision to the host"
    );
}

#[gpui::test]
fn a_query_nothing_answers_shows_what_was_asked(cx: &mut TestAppContext) {
    let (mut harness, palette) = palette(cx);
    type_query(&mut harness, &palette, "zzz");

    let empty = harness
        .node("workspace.palette.empty")
        .expect("an empty result is reported, never drawn as an empty list");
    assert_eq!(empty.value.as_deref(), Some("empty"));
    assert!(
        empty
            .text
            .as_deref()
            .is_some_and(|text| text.contains("zzz")),
        "the report names the query that answered nothing"
    );
    assert!(harness.node("workspace.palette.editor.save").is_none());
}

#[gpui::test]
fn a_command_the_host_refused_is_listed_with_its_reason(cx: &mut TestAppContext) {
    let (mut harness, palette) = palette(cx);
    let seen = events(&mut harness, &palette);
    type_query(&mut harness, &palette, "publish");

    let refused = harness
        .node("workspace.palette.workspace.publish")
        .expect("an unavailable command is still listed");
    assert!(refused.disabled);
    assert_eq!(
        refused.value.as_deref(),
        Some("Approval is required for this workspace")
    );

    harness.click("workspace.palette.workspace.publish");
    assert!(taken(&seen).is_empty(), "a refused row installs no handler");
}

#[gpui::test]
fn the_keyboard_never_lands_on_a_refused_command(cx: &mut TestAppContext) {
    let (mut harness, palette) = palette(cx);
    let seen = events(&mut harness, &palette);
    type_query(&mut harness, &palette, "workspace");
    focus_query(&mut harness, &palette);

    assert!(
        !harness
            .node("workspace.palette.workspace.publish")
            .expect("published")
            .hovered
    );
    harness.keystrokes("down enter");
    assert_eq!(taken(&seen), vec!["workspace.save".to_string()]);
}

#[gpui::test]
fn clicking_a_result_reports_it(cx: &mut TestAppContext) {
    let (mut harness, palette) = palette(cx);
    let seen = events(&mut harness, &palette);

    harness.click("workspace.palette.editor.split");

    assert_eq!(taken(&seen), vec!["editor.split".to_string()]);
}
