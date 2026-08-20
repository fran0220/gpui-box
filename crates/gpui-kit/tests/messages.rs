//! A conversation never removes a failure, never retries by itself, and never
//! drags the reader back down to a message that has just arrived.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{IntoElement, TestAppContext};
use gpui_kit::content::message_list::streaming_since;
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

type Sink<T> = Rc<RefCell<Vec<T>>>;

fn sink<T: 'static>() -> (Sink<T>, Sink<T>) {
    let calls: Sink<T> = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

type Thread = Rc<RefCell<Vec<Message>>>;

fn message(id: &'static str, author: &'static str, body: &'static str) -> Message {
    Message::new(id, body).author(author).time("09:15")
}

fn conversation(
    cx: &mut TestAppContext,
    thread: Thread,
    visible_rows: Option<usize>,
    group: bool,
) -> (Harness, Sink<String>) {
    let (calls, into) = sink::<String>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let into = into.clone();
        let mut list = MessageList::new("chat", thread.borrow().iter().cloned())
            .body_lines(2)
            .group_consecutive(group)
            .on_retry(move |id, _, _| into.borrow_mut().push(id.to_string()));
        if let Some(rows) = visible_rows {
            list = list.visible_rows(rows);
        }
        list.into_any_element()
    });
    (harness, calls)
}

fn states() -> Vec<Message> {
    vec![
        message("msg-sending", "Ada", "Publishing").delivery(DeliveryState::Sending),
        message("msg-sent", "Ada", "Published").delivery(DeliveryState::Sent),
        message("msg-delivered", "Grace", "Seen it").delivery(DeliveryState::Delivered),
        message("msg-read", "Grace", "Thanks").delivery(DeliveryState::Read),
        message("msg-failed", "Ada", "Publishing the artifacts")
            .failed("The workspace is frozen for the release."),
    ]
}

#[gpui::test]
fn every_delivery_state_is_drawn_as_itself(cx: &mut TestAppContext) {
    let thread: Thread = Rc::new(RefCell::new(states()));
    let (mut harness, _retries) = conversation(cx, thread, None, false);

    let published: Vec<Option<String>> = [
        "msg-sending",
        "msg-sent",
        "msg-delivered",
        "msg-read",
        "msg-failed",
    ]
    .into_iter()
    .map(|id| {
        harness
            .node(&format!("chat.{id}.delivery"))
            .expect("published")
            .value
    })
    .collect();

    assert_eq!(
        published,
        vec![
            Some("sending".to_string()),
            Some("sent".to_string()),
            Some("delivered".to_string()),
            Some("read".to_string()),
            Some("failed".to_string()),
        ]
    );
}

#[gpui::test]
fn a_failed_message_keeps_its_text_and_states_the_hosts_reason(cx: &mut TestAppContext) {
    let thread: Thread = Rc::new(RefCell::new(states()));
    let (mut harness, _retries) = conversation(cx, thread, None, false);

    assert!(
        harness.node("chat.msg-failed").is_some(),
        "a failure is never removed from the conversation"
    );
    let delivery = harness.node("chat.msg-failed.delivery").expect("published");
    assert_eq!(
        delivery.text.as_deref(),
        Some("The workspace is frozen for the release."),
        "the reason is the host's own sentence"
    );
    assert!(delivery.invalid);
}

#[gpui::test]
fn a_retry_reports_and_resends_nothing(cx: &mut TestAppContext) {
    let thread: Thread = Rc::new(RefCell::new(states()));
    let (mut harness, retries) = conversation(cx, thread, None, false);

    harness.click("chat.msg-failed.retry");

    assert_eq!(retries.borrow().as_slice(), ["msg-failed"]);
    assert_eq!(
        harness
            .node("chat.msg-failed.delivery")
            .expect("published")
            .value
            .as_deref(),
        Some("failed"),
        "the state stays failed until the host says otherwise"
    );
}

#[gpui::test]
fn only_a_failed_message_offers_a_retry(cx: &mut TestAppContext) {
    let thread: Thread = Rc::new(RefCell::new(states()));
    let (mut harness, _retries) = conversation(cx, thread, None, false);

    assert!(harness.node("chat.msg-failed.retry").is_some());
    assert!(harness.node("chat.msg-sent.retry").is_none());
}

#[gpui::test]
fn appending_to_a_streaming_message_does_not_restart_its_indicator(cx: &mut TestAppContext) {
    let thread: Thread = Rc::new(RefCell::new(vec![
        Message::new("msg-stream", "Chec")
            .author("Assistant")
            .time("09:16")
            .streaming(true),
    ]));
    let (mut harness, _retries) = conversation(cx, thread.clone(), None, false);

    let list = Ident::new("chat");
    let began = harness
        .update({
            let list = list.clone();
            move |_, cx| streaming_since(&list, "msg-stream", cx)
        })
        .expect("a streaming message starts a clock");
    assert!(harness.node("chat.msg-stream.streaming").is_some());

    harness.advance(Duration::from_millis(500));
    thread.borrow_mut()[0] = Message::new("msg-stream", "Checking the freeze window")
        .author("Assistant")
        .time("09:16")
        .streaming(true);
    harness.frame();

    let after = harness
        .update({
            let list = list.clone();
            move |_, cx| streaming_since(&list, "msg-stream", cx)
        })
        .expect("still streaming");
    assert_eq!(
        began, after,
        "text arriving into a stream must not restart the indicator"
    );
    assert!(harness.node("chat.msg-stream.streaming").is_some());
}

#[gpui::test]
fn a_message_that_stops_streaming_drops_its_indicator_and_its_clock(cx: &mut TestAppContext) {
    let thread: Thread = Rc::new(RefCell::new(vec![
        Message::new("msg-stream", "Done")
            .author("Assistant")
            .streaming(true),
    ]));
    let (mut harness, _retries) = conversation(cx, thread.clone(), None, false);
    assert!(harness.node("chat.msg-stream.streaming").is_some());

    thread.borrow_mut()[0] = Message::new("msg-stream", "Done")
        .author("Assistant")
        .streaming(false)
        .delivery(DeliveryState::Delivered);
    harness.frame();

    assert!(harness.node("chat.msg-stream.streaming").is_none());
    let list = Ident::new("chat");
    assert_eq!(
        harness.update(move |_, cx| streaming_since(&list, "msg-stream", cx)),
        None
    );
}

#[gpui::test]
fn grouping_is_the_callers_decision(cx: &mut TestAppContext) {
    let thread = || {
        Rc::new(RefCell::new(vec![
            message("msg-one", "Ada", "First"),
            message("msg-two", "Ada", "Second"),
        ]))
    };

    let (mut ungrouped, _) = conversation(cx, thread(), None, false);
    assert!(
        ungrouped.node("chat.msg-two.author").is_some(),
        "without grouping every message carries its byline"
    );

    let (mut grouped, _) = conversation(cx, thread(), None, true);
    assert!(grouped.node("chat.msg-one.author").is_some());
    assert!(
        grouped.node("chat.msg-two.author").is_none(),
        "a continued turn does not repeat the name"
    );
}

#[gpui::test]
fn an_unknown_author_is_named_and_an_unknown_time_says_so(cx: &mut TestAppContext) {
    let thread: Thread = Rc::new(RefCell::new(vec![Message::new(
        "msg-anon",
        "Who sent this?",
    )]));
    let (mut harness, _retries) = conversation(cx, thread, None, false);

    assert_eq!(
        harness
            .node("chat.msg-anon.author")
            .expect("published")
            .text
            .as_deref(),
        Some("unknown")
    );
    assert_eq!(
        harness
            .node("chat.msg-anon.time")
            .expect("published")
            .value
            .as_deref(),
        Some("time unknown")
    );
}

#[gpui::test]
fn only_the_messages_in_the_viewport_are_published(cx: &mut TestAppContext) {
    let thread: Thread = Rc::new(RefCell::new(states()));
    let (mut harness, _retries) = conversation(cx, thread, Some(2), false);

    assert_eq!(
        harness.node("chat").expect("published").value.as_deref(),
        Some("5"),
        "the conversation reports how many messages it holds"
    );
    assert!(harness.node("chat.msg-sending").is_some());
    assert!(
        harness.node("chat.msg-failed").is_none(),
        "a message below the viewport is not laid out and publishes nothing"
    );
}

#[gpui::test]
fn a_reader_who_is_away_is_told_how_many_arrived(cx: &mut TestAppContext) {
    let thread: Thread = Rc::new(RefCell::new(states()));
    let (mut harness, _retries) = conversation(cx, thread.clone(), Some(2), false);

    // Opening at the top of a longer thread is not an arrival, so what is
    // below the fold is reported as being below the fold.
    let waiting = harness.node("chat.pending").expect("published");
    assert_eq!(waiting.text.as_deref(), Some("3 more messages"));

    thread
        .borrow_mut()
        .push(message("msg-new", "Grace", "One more thing"));
    harness.frame();

    let arrived = harness.node("chat.pending").expect("published");
    assert_eq!(
        arrived.text.as_deref(),
        Some("1 new message"),
        "a message that arrived while the reader was away is counted as new"
    );
    assert_eq!(arrived.value.as_deref(), Some("1"));
    assert!(
        harness.node("chat.msg-new").is_none(),
        "the list must not drag the reader down to it"
    );
}

#[gpui::test]
fn a_reader_at_the_bottom_follows_without_being_told_anything(cx: &mut TestAppContext) {
    let thread: Thread = Rc::new(RefCell::new(vec![
        message("msg-one", "Ada", "First"),
        message("msg-two", "Ada", "Second"),
    ]));
    let (mut harness, _retries) = conversation(cx, thread.clone(), None, false);
    assert!(
        harness.node("chat.pending").is_none(),
        "a conversation that fits has nothing below it"
    );

    thread
        .borrow_mut()
        .push(message("msg-three", "Grace", "Third"));
    harness.frame();

    assert!(harness.node("chat.msg-three").is_some());
    assert!(
        harness.node("chat.pending").is_none(),
        "a reader already at the bottom is followed, not counted at"
    );
}

/// A conversation drawn either way, with one long message in it.
fn long_message(cx: &mut TestAppContext, grows: bool) -> Harness {
    Harness::new(cx, gpui_kit::install, move |_, _| {
        let list = MessageList::new(
            "chat",
            [Message::new("msg-long", "one\ntwo\nthree\nfour\nfive")
                .author("Ada")
                .time("09:15")],
        )
        .visible_rows(4);
        if grows {
            list.grows_to_fit().into_any_element()
        } else {
            list.body_lines(2).into_any_element()
        }
    })
}

#[gpui::test]
fn a_slot_says_how_much_of_a_message_it_left_out(cx: &mut TestAppContext) {
    let mut harness = long_message(cx, false);
    let left_out = harness
        .node("chat.msg-long.truncated")
        .expect("a slot that cut a message off says so");

    assert_eq!(
        left_out.value.as_deref(),
        Some("3"),
        "five lines in a two-line slot leaves three"
    );
}

#[gpui::test]
fn a_message_that_grows_to_fit_leaves_nothing_out(cx: &mut TestAppContext) {
    let mut harness = long_message(cx, true);

    assert!(
        harness.node("chat.msg-long").is_some(),
        "the message is still published"
    );
    assert!(
        harness.node("chat.msg-long.truncated").is_none(),
        "nothing was left out, so there is nothing to report"
    );
}

#[gpui::test]
fn a_growing_conversation_still_only_builds_its_viewport(cx: &mut TestAppContext) {
    let messages: Vec<Message> = (0..2_000)
        .map(|index| {
            Message::new(format!("msg-{index}"), format!("Message {index}"))
                .author("Ada")
                .time("09:15")
        })
        .collect();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        MessageList::new("chat", messages.clone())
            .grows_to_fit()
            .visible_rows(6)
            .into_any_element()
    });

    assert!(
        harness.node("chat.msg-0").is_some(),
        "the messages at the top are the ones on screen"
    );
    assert!(
        harness.node("chat.msg-1999").is_none(),
        "growing to fit is still virtualization: a message nobody can see is not laid out"
    );
}
