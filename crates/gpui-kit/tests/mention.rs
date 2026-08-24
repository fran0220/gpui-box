//! Mention completion through the same editor input and semantic paths a host uses.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{App, AppContext as _, Entity, SharedString, TestAppContext, div, prelude::*, px};
use gpui_kit::prelude::*;
use gpui_kit::strings::{SearchMatcher, set_search};
use gpui_kit_testkit::harness::Harness;

type Views = Rc<RefCell<Option<(Entity<MentionInput>, Entity<TextArea>)>>>;

fn mention(
    cx: &mut TestAppContext,
    suggestions: AsyncValue<Vec<MentionCandidate>, SharedString>,
    install: impl Fn(&mut App) + 'static,
) -> (Harness, Views) {
    let views: Views = Rc::new(RefCell::new(None));
    let build = views.clone();
    let harness = Harness::new(cx, install, move |window, cx| {
        let pair = build
            .borrow_mut()
            .get_or_insert_with(|| {
                let editor = cx.new(|cx| {
                    TextArea::new("composer.text", window, cx)
                        .placeholder("Message the team")
                        .enter(Enter::Submits)
                        .rows(2)
                });
                let input = cx.new(|cx| {
                    MentionInput::new("composer.mentions", editor.clone(), cx)
                        .suggestions(suggestions.clone())
                });
                (input, editor)
            })
            .clone();
        div()
            .w(px(420.0))
            .p(px(24.0))
            .child(pair.0)
            .into_any_element()
    });
    (harness, views)
}

fn ready() -> AsyncValue<Vec<MentionCandidate>, SharedString> {
    AsyncValue::ready(vec![
        MentionCandidate::new("ada", "Ada Lovelace")
            .description("Compiler group")
            .replacement("@Ada"),
        MentionCandidate::new("grace", "Grace Hopper")
            .description("Runtime group")
            .replacement("@Grace"),
    ])
}

fn entities(views: &Views) -> (Entity<MentionInput>, Entity<TextArea>) {
    views.borrow().clone().expect("views were built")
}

fn events(
    harness: &mut Harness,
    input: &Entity<MentionInput>,
) -> Rc<RefCell<Vec<MentionInputEvent>>> {
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let input = input.clone();
    harness.update(move |_, cx| {
        cx.subscribe(&input, move |_, event: &MentionInputEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    events
}

fn value(harness: &mut Harness, editor: &Entity<TextArea>) -> String {
    harness.update(|_, cx| editor.read(cx).value().to_string())
}

#[gpui::test]
fn typing_a_trigger_reports_its_range_and_opens_stable_candidate_rows(cx: &mut TestAppContext) {
    let (mut harness, views) = mention(cx, ready(), gpui_kit::install);
    let (input, _) = entities(&views);
    let reported = events(&mut harness, &input);

    harness.click("composer.text");
    harness.keystrokes("@ a d");
    harness.frame();

    assert!(harness.node("composer.mentions.menu").is_some());
    assert!(harness.node("composer.mentions.option.ada").is_some());
    assert!(reported.borrow().contains(&MentionInputEvent::Focused));
    assert!(
        reported
            .borrow()
            .contains(&MentionInputEvent::QueryChanged(Some(MentionQuery {
                text: "ad".into(),
                range: 0..3,
            })))
    );

    let tree = harness.accessibility_tree();
    let nodes = tree["nodes"].as_object().expect("native nodes");
    let editor_key = tree["gpui_focus"].as_str().expect("GPUI focus");
    let active_key = tree["active_descendant_focus"]
        .as_str()
        .expect("active descendant focus");
    assert_eq!(nodes[editor_key]["element_id"], "Name(\"composer.text\")");
    assert_eq!(
        nodes[active_key]["element_id"],
        "Name(\"composer.mentions.option.ada\")"
    );
}

#[gpui::test]
fn enter_replaces_the_unicode_token_once_and_reports_identity_with_the_new_range(
    cx: &mut TestAppContext,
) {
    let (mut harness, views) = mention(cx, ready(), gpui_kit::install);
    let (input, editor) = entities(&views);
    let reported = events(&mut harness, &input);

    harness.click("composer.text");
    harness.keystrokes("@ a d enter");

    assert_eq!(value(&mut harness, &editor), "@Ada");
    assert!(reported.borrow().contains(&MentionInputEvent::Accepted {
        id: "ada".into(),
        range: 0..4,
    }));
    let undo = if cfg!(target_os = "macos") {
        "cmd-z"
    } else {
        "ctrl-z"
    };
    harness.keystrokes(undo);
    assert_eq!(value(&mut harness, &editor), "@ad");
}

#[gpui::test]
fn email_text_and_a_caret_inside_a_token_do_not_claim_completion(cx: &mut TestAppContext) {
    let (mut harness, views) = mention(cx, ready(), gpui_kit::install);
    let (_, editor) = entities(&views);

    harness.click("composer.text");
    harness.keystrokes("m a i l @ e x a m p l e");
    harness.frame();
    assert!(harness.node("composer.mentions.menu").is_none());

    let set = editor.clone();
    harness.update(move |_, cx| {
        set.update(cx, |editor, cx| {
            editor.set_value("@ada", cx);
            editor.set_selected_range(2..2, cx);
        });
    });
    harness.frame();
    assert!(
        harness.node("composer.mentions.menu").is_none(),
        "a caret before the end does not replace only half of the token"
    );
}

#[gpui::test]
fn disabled_and_read_only_editors_release_an_open_completion(cx: &mut TestAppContext) {
    let (mut harness, views) = mention(cx, ready(), gpui_kit::install);
    let (input, editor) = entities(&views);
    let reported = events(&mut harness, &input);

    harness.click("composer.text");
    harness.keystrokes("@");
    assert!(harness.node("composer.mentions.menu").is_some());

    let disabled = editor.clone();
    harness.update(move |_, cx| {
        disabled.update(cx, |editor, cx| editor.set_disabled(true, cx));
    });
    assert!(harness.node("composer.mentions.menu").is_none());
    assert!(
        reported
            .borrow()
            .contains(&MentionInputEvent::QueryChanged(None))
    );

    let read_only = editor.clone();
    harness.update(move |_, cx| {
        read_only.update(cx, |editor, cx| {
            editor.set_disabled(false, cx);
            editor.set_read_only(true, cx);
        });
    });
    harness.frame();
    assert!(harness.node("composer.mentions.menu").is_none());
}

#[gpui::test]
fn escape_dismisses_without_cancelling_the_editor_and_a_changed_query_reopens(
    cx: &mut TestAppContext,
) {
    let (mut harness, views) = mention(cx, ready(), gpui_kit::install);
    let (input, _) = entities(&views);
    let reported = events(&mut harness, &input);

    harness.click("composer.text");
    harness.keystrokes("@ a d escape");
    harness.frame();
    assert!(harness.node("composer.mentions.menu").is_none());
    assert!(!reported.borrow().contains(&MentionInputEvent::Cancelled));
    assert!(
        reported
            .borrow()
            .contains(&MentionInputEvent::QueryChanged(None))
    );

    harness.keystrokes("a");
    harness.frame();
    assert!(harness.node("composer.mentions.menu").is_some());
}

#[derive(Debug)]
struct LocaleSearch;

impl SearchMatcher for LocaleSearch {
    fn rank(&self, query: &str, label: &str) -> Option<usize> {
        (query == "机器" && label == "engine").then_some(0)
    }
}

#[gpui::test]
fn aliases_use_the_installed_matcher_without_reaching_semantics_or_debug(cx: &mut TestAppContext) {
    let candidate = MentionCandidate::new("runtime", "Native runtime")
        .replacement("@runtime")
        .search_terms(["engine"]);
    let debug = format!("{candidate:?}");
    assert!(!debug.contains("engine") && !debug.contains("@runtime"));
    let install = |cx: &mut App| {
        gpui_kit::install(cx);
        set_search(LocaleSearch, cx);
    };
    let (mut harness, views) = mention(cx, AsyncValue::ready(vec![candidate]), install);
    let (_, editor) = entities(&views);
    let seed = editor.clone();
    harness.update(move |_, cx| seed.update(cx, |editor, cx| editor.set_value("@机器", cx)));
    harness.click("composer.text");
    harness.frame();

    assert!(harness.node("composer.mentions.option.runtime").is_some());
    assert!(!harness.snapshot().contains("engine"));
}

#[gpui::test]
fn loading_failure_and_stale_failure_remain_three_different_truths(cx: &mut TestAppContext) {
    let (mut harness, views) = mention(cx, AsyncValue::loading(), gpui_kit::install);
    let (input, _) = entities(&views);
    harness.click("composer.text");
    harness.keystrokes("@");
    harness.frame();
    assert_eq!(
        harness
            .node("composer.mentions.status.loading")
            .expect("loading status")
            .text
            .as_deref(),
        Some("Loading mention suggestions")
    );

    let failed = input.clone();
    harness.update(move |_, cx| {
        failed.update(cx, |input, cx| {
            input.set_suggestions(AsyncValue::error("Directory offline".into()), cx)
        });
    });
    assert_eq!(
        harness
            .node("composer.mentions.status.error")
            .expect("error status")
            .description
            .as_deref(),
        Some("Directory offline")
    );

    let mut stale = ready();
    stale.refresh();
    stale.fail_refresh("Refresh timed out".into());
    let stale_input = input.clone();
    harness.update(move |_, cx| {
        stale_input.update(cx, |input, cx| input.set_suggestions(stale, cx));
    });
    assert!(harness.node("composer.mentions.option.ada").is_some());
    assert_eq!(
        harness
            .node("composer.mentions.status.stale")
            .expect("stale status")
            .description
            .as_deref(),
        Some("Refresh timed out")
    );
}

#[gpui::test]
fn refused_candidates_are_visible_inert_and_skipped_by_the_keyboard(cx: &mut TestAppContext) {
    let candidates = AsyncValue::ready(vec![
        MentionCandidate::new("ada", "Ada")
            .replacement("@Ada")
            .unavailable("Outside this workspace"),
        MentionCandidate::new("grace", "Grace").replacement("@Grace"),
    ]);
    let (mut harness, views) = mention(cx, candidates, gpui_kit::install);
    let (input, editor) = entities(&views);
    let reported = events(&mut harness, &input);
    harness.click("composer.text");
    harness.keystrokes("@");
    harness.frame();

    assert!(
        harness
            .node("composer.mentions.option.ada")
            .expect("refused candidate")
            .disabled
    );
    harness.click("composer.mentions.option.ada");
    assert_eq!(value(&mut harness, &editor), "@");
    harness.keystrokes("enter");
    assert_eq!(value(&mut harness, &editor), "@Grace");
    assert!(reported.borrow().contains(&MentionInputEvent::Accepted {
        id: "grace".into(),
        range: 0..6,
    }));
}

#[gpui::test]
fn arrows_move_by_stable_candidate_identity(cx: &mut TestAppContext) {
    let (mut harness, views) = mention(cx, ready(), gpui_kit::install);
    let (_, editor) = entities(&views);
    harness.click("composer.text");
    harness.keystrokes("@ down enter");
    assert_eq!(value(&mut harness, &editor), "@Grace");
}

#[gpui::test]
fn empty_no_match_and_unavailable_publish_distinct_statuses(cx: &mut TestAppContext) {
    let (mut harness, views) = mention(cx, AsyncValue::empty(), gpui_kit::install);
    let (input, editor) = entities(&views);
    harness.click("composer.text");
    harness.keystrokes("@");
    assert_eq!(
        harness
            .node("composer.mentions.status.empty")
            .expect("empty status")
            .text
            .as_deref(),
        Some("No mention suggestions")
    );

    let no_match = input.clone();
    harness.update(move |_, cx| {
        no_match.update(cx, |input, cx| input.set_suggestions(ready(), cx));
        editor.update(cx, |editor, cx| editor.set_value("@zzz", cx));
    });
    assert_eq!(
        harness
            .node("composer.mentions.status.no-match")
            .expect("no-match status")
            .text
            .as_deref(),
        Some("No mentions match this search")
    );

    let unavailable = input.clone();
    harness.update(move |_, cx| {
        unavailable.update(cx, |input, cx| {
            input.set_suggestions(AsyncValue::refused("Directory disabled"), cx)
        });
    });
    let status = harness
        .node("composer.mentions.status.unavailable")
        .expect("unavailable status");
    assert_eq!(status.text.as_deref(), Some("Mentions unavailable"));
    assert_eq!(status.description.as_deref(), Some("Directory disabled"));
}
