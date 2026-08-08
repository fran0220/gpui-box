//! What a connection list is allowed to claim about a server and about what
//! that server offers.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Calls = Rc<RefCell<Vec<String>>>;

fn every_state() -> Vec<ServerEntry> {
    vec![
        ServerEntry::new("workspace", "Workspace tools").state(ServerState::Connected),
        ServerEntry::new("build", "Build runner").state(ServerState::Connecting),
        ServerEntry::new("notes", "Notes").state(ServerState::Disconnected),
        ServerEntry::new("deploy", "Deployment").state(ServerState::Failed {
            reason: "The connection was refused after three attempts.".into(),
        }),
        ServerEntry::new("telemetry", "Telemetry").state(ServerState::Disabled {
            reason: Some("You turned this one off.".into()),
        }),
    ]
}

#[gpui::test]
fn all_five_states_present_differently(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ServerList::new("servers")
            .servers(every_state())
            .on_select(|_, _, _| {})
            .into_any_element()
    });

    let published: Vec<String> = ["workspace", "build", "notes", "deploy", "telemetry"]
        .iter()
        .map(|id| {
            harness
                .node(&format!("servers.{id}"))
                .and_then(|node| node.value)
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(
        published,
        vec![
            "connected",
            "connecting",
            "disconnected",
            "failed",
            "disabled"
        ],
        "five states, five sentences"
    );

    let list = harness.node("servers").expect("published");
    assert_eq!(list.role, Role::List);
    assert_eq!(list.value.as_deref(), Some("5"));

    let tree = harness.accessibility_tree();
    assert!(tree["nodes"].as_object().is_some_and(|nodes| {
        nodes.values().any(|node| {
            node["element_id"] == "Name(\"servers.workspace\")" && node["aria"]["role"] == "Row"
        })
    }));
}

#[gpui::test]
fn a_failure_keeps_its_reason_and_offers_one_control(cx: &mut TestAppContext) {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        ServerList::new("servers")
            .servers(every_state())
            .on_retry(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .into_any_element()
    });

    let reason = harness.node("servers.deploy.reason").expect("published");
    assert_eq!(
        reason.text.as_deref(),
        Some("The connection was refused after three attempts."),
        "the host's reason is shown word for word"
    );
    assert_eq!(reason.value.as_deref(), Some("failed"));

    harness.click("servers.deploy.retry");
    assert_eq!(calls.borrow().as_slice(), ["deploy"]);
    assert_eq!(
        harness
            .node("servers.deploy")
            .and_then(|node| node.value)
            .as_deref(),
        Some("failed"),
        "the list retries nothing, so the state that still holds stays on screen"
    );

    assert!(
        harness.node("servers.notes.retry").is_none(),
        "only a failure offers a retry"
    );
}

#[gpui::test]
fn a_connection_the_reader_turned_off_installs_no_handler(cx: &mut TestAppContext) {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        let retry = Rc::clone(&sink);
        ServerList::new("servers")
            .servers(every_state())
            .on_select(move |id, _, _| sink.borrow_mut().push(id.to_string()))
            .on_retry(move |id, _, _| retry.borrow_mut().push(id.to_string()))
            .into_any_element()
    });

    let off = harness.node("servers.telemetry").expect("published");
    assert!(off.disabled, "a refusal is published, not only drawn dim");
    assert_eq!(
        harness
            .node("servers.telemetry.reason")
            .and_then(|node| node.text)
            .as_deref(),
        Some("You turned this one off."),
        "a refusal the host explained keeps the explanation"
    );

    harness.click("servers.telemetry");
    assert!(calls.borrow().is_empty());
}

#[gpui::test]
fn offering_nothing_is_not_the_same_as_not_having_been_asked(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ServerList::new("servers")
            .expanded_ids(&["index", "archive", "build", "broken"])
            .servers([
                ServerEntry::new("index", "Search index")
                    .state(ServerState::Connected)
                    .offers([]),
                ServerEntry::new("archive", "Archive").state(ServerState::Connected),
                ServerEntry::new("build", "Build runner")
                    .state(ServerState::Connecting)
                    .catalog(Catalog::Asking),
                ServerEntry::new("broken", "Reports")
                    .state(ServerState::Connected)
                    .catalog(Catalog::Unavailable("The list endpoint refused.".into())),
            ])
            .into_any_element()
    });

    assert_eq!(
        harness
            .node("servers.index.offerings")
            .and_then(|node| node.value)
            .as_deref(),
        Some("empty"),
        "it answered, and the answer was empty"
    );
    assert_eq!(
        harness
            .node("servers.archive.offerings")
            .and_then(|node| node.value)
            .as_deref(),
        Some("unstarted"),
        "nobody asked"
    );

    let asking = harness.node("servers.build.offerings").expect("published");
    assert_eq!(asking.value.as_deref(), Some("asking"));
    assert!(asking.busy);

    assert_eq!(
        harness
            .node("servers.broken.offerings")
            .and_then(|node| node.value)
            .as_deref(),
        Some("unavailable"),
        "a refusal is not an emptiness"
    );
}

#[gpui::test]
fn an_offering_is_named_under_the_server_that_offers_it(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ServerList::new("servers")
            .expanded_ids(&["left", "right"])
            .servers([
                // Two servers offering the same name is the ordinary case, not
                // the exotic one, which is why attribution is in the id.
                ServerEntry::new("left", "Workspace tools")
                    .state(ServerState::Connected)
                    .offers([Offering::tool("read", "Read a file")]),
                ServerEntry::new("right", "Archive tools")
                    .state(ServerState::Connected)
                    .offers([
                        Offering::tool("read", "Read a file"),
                        Offering::skill("review", "Review a change"),
                        Offering::resource("changelog", "Changelog").qualifier("archive:/CHANGES"),
                    ]),
            ])
            .into_any_element()
    });

    let left = harness
        .node("servers.left.offering.read")
        .expect("published");
    let right = harness
        .node("servers.right.offering.read")
        .expect("published");
    assert_eq!(left.text, right.text);
    assert_ne!(left.id, right.id, "two servers may offer the same name");
    assert_eq!(left.value.as_deref(), Some("tool"));

    assert_eq!(
        harness
            .node("servers.right.offering.review")
            .and_then(|node| node.value)
            .as_deref(),
        Some("skill")
    );
    assert_eq!(
        harness
            .node("servers.right.offering.changelog")
            .and_then(|node| node.value)
            .as_deref(),
        Some("resource")
    );
    assert_eq!(
        harness
            .node("servers.right.offerings")
            .and_then(|node| node.value)
            .as_deref(),
        Some("3"),
        "the container states how much it holds"
    );
}

#[gpui::test]
fn a_folded_server_publishes_none_of_what_it_offers(cx: &mut TestAppContext) {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&calls);
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let sink = Rc::clone(&sink);
        ServerList::new("servers")
            .servers([ServerEntry::new("left", "Workspace tools")
                .state(ServerState::Connected)
                .offers([Offering::tool("read", "Read a file")])])
            .on_toggle(move |id, open, _, _| sink.borrow_mut().push(format!("{id}:{open}")))
            .into_any_element()
    });

    assert!(harness.node("servers.left.offering.read").is_none());
    assert_eq!(
        harness.node("servers.left").and_then(|node| node.expanded),
        Some(false)
    );

    harness.click("servers.left.toggle");
    assert_eq!(calls.borrow().as_slice(), ["left:true"]);
    assert!(
        harness.node("servers.left.offering.read").is_none(),
        "the list folds nothing itself: the caller owns the disclosure"
    );
}

#[gpui::test]
fn a_list_with_no_connections_says_so(cx: &mut TestAppContext) {
    let mut harness = Harness::new(cx, gpui_kit::install, |_, _| {
        ServerList::new("servers").into_any_element()
    });

    assert_eq!(
        harness
            .node("servers.empty")
            .and_then(|node| node.value)
            .as_deref(),
        Some("empty")
    );
    assert_eq!(
        harness
            .node("servers")
            .and_then(|node| node.value)
            .as_deref(),
        Some("0")
    );
}
