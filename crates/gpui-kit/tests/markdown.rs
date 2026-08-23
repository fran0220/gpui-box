//! A rendered document does nothing that the document asks for.
//!
//! Raw HTML is drawn as the characters somebody wrote rather than run or
//! dropped, a link states where it goes and is only reported, an image is
//! named rather than fetched, and a code block is never coloured by guessing.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{IntoElement, ParentElement, Styled, TestAppContext};
use gpui_kit::prelude::*;
use gpui_kit_semantics::Role;
use gpui_kit_testkit::harness::Harness;

type Sink<T> = Rc<RefCell<Vec<T>>>;

fn sink<T: 'static>() -> (Sink<T>, Sink<T>) {
    let calls: Sink<T> = Rc::new(RefCell::new(Vec::new()));
    (calls.clone(), calls)
}

/// One document carrying every rule this component keeps.
const DOCUMENT: &str = r#"# Release notes

## What changed

A paragraph with **strong**, *emphasis*, ~~struck~~ and `inline` text, and
a link to [the run log](https://example.test/runs/4821 "the failing run") with
some <b>inline markup</b> beside it.

- one
  - nested
- [x] Bounded retries
- [ ] Bounded backoff

> A refusal is displayed as a refusal.

```rust
fn main() {}
```

```
plain fence
```

| Stage | Result |
|:------|-------:|
| Build | passed |
| Test  | passed |

<div onclick="steal()">This was written as HTML.</div>

![The run graph](runs/graph.png)

---
"#;

fn markdown(
    cx: &mut TestAppContext,
    source: &'static str,
    max_lines: Option<usize>,
) -> (Harness, Sink<MarkdownEvent>) {
    let (calls, into) = sink::<MarkdownEvent>();
    let harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let into = into.clone();
        let mut document = Markdown::new("doc", source)
            .on_event(move |event, _, _| into.borrow_mut().push(event.clone()));
        if let Some(max) = max_lines {
            document = document.max_lines(max);
        }
        document.into_any_element()
    });
    (harness, calls)
}

#[gpui::test]
fn raw_html_is_rendered_as_literal_text_and_marked_unrendered(cx: &mut TestAppContext) {
    let (mut harness, _events) = markdown(cx, DOCUMENT, None);

    let block = harness.node("doc.html-block").expect("published");
    assert_eq!(block.value.as_deref(), Some("unrendered html"));
    assert!(
        block
            .text
            .as_deref()
            .is_some_and(|text| text.contains("<div onclick=") && text.contains("written as HTML")),
        "the tag and the text inside it must both survive: {:?}",
        block.text
    );
    assert!(block.visible, "unrendered HTML must still occupy the page");
}

#[gpui::test]
fn inline_html_stays_in_its_line_rather_than_being_dropped(cx: &mut TestAppContext) {
    let (mut harness, _events) = markdown(cx, DOCUMENT, None);

    let inline = harness.node("doc.html-inline").expect("published");
    assert_eq!(inline.value.as_deref(), Some("unrendered html"));
    assert_eq!(inline.text.as_deref(), Some("<b>"));
}

#[gpui::test]
fn a_links_destination_is_published_before_it_is_taken(cx: &mut TestAppContext) {
    let (mut harness, _events) = markdown(cx, DOCUMENT, None);

    let link = harness
        .node("doc.link-https-example-test-runs-4821")
        .expect("published");
    assert_eq!(link.role, Role::Link);
    assert_eq!(link.text.as_deref(), Some("the run log"));
    assert_eq!(
        link.value.as_deref(),
        Some("https://example.test/runs/4821")
    );
}

#[gpui::test]
fn taking_a_link_reports_it_and_opens_nothing(cx: &mut TestAppContext) {
    let (mut harness, events) = markdown(cx, DOCUMENT, None);

    harness.click("doc.link-https-example-test-runs-4821");

    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| matches!(
                event,
                MarkdownEvent::LinkClicked { href } if href.as_ref() == "https://example.test/runs/4821"
            ))
            .count(),
        1
    );
    assert!(
        harness
            .node("doc.link-https-example-test-runs-4821")
            .is_some(),
        "the document must be unchanged by a link nobody here opened"
    );
}

#[gpui::test]
fn an_image_is_not_fetched_and_names_what_is_missing(cx: &mut TestAppContext) {
    let (mut harness, events) = markdown(cx, DOCUMENT, None);

    let image = harness.node("doc.image-runs-graph-png").expect("published");
    assert_eq!(image.role, Role::Image);
    assert_eq!(image.text.as_deref(), Some("The run graph"));
    assert_eq!(image.value.as_deref(), Some("not fetched"));

    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| matches!(
                event,
                MarkdownEvent::ImageRequested { src, alt }
                    if src.as_ref() == "runs/graph.png" && alt.as_ref() == "The run graph"
            ))
            .count(),
        1
    );
}

#[gpui::test]
fn an_image_is_requested_once_rather_than_once_a_frame(cx: &mut TestAppContext) {
    let (mut harness, events) = markdown(cx, DOCUMENT, None);
    harness.frame();
    harness.frame();

    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| matches!(event, MarkdownEvent::ImageRequested { .. }))
            .count(),
        1,
        "a host that answers a request must not be asked again every frame"
    );
}

#[gpui::test]
fn a_host_supplied_image_replaces_the_placeholder(cx: &mut TestAppContext) {
    let (calls, into) = sink::<MarkdownEvent>();
    let mut harness = Harness::new(cx, gpui_kit::install, move |_, _| {
        let into = into.clone();
        Markdown::new("doc", "![The run graph](runs/graph.png)")
            .on_event(move |event, _, _| into.borrow_mut().push(event.clone()))
            .image(|request, _, _| {
                Some(
                    gpui::div()
                        .w(gpui::px(40.0))
                        .h(gpui::px(20.0))
                        .child(request.alt.clone())
                        .into_any_element(),
                )
            })
            .into_any_element()
    });

    assert_eq!(
        harness
            .node("doc.image-runs-graph-png")
            .expect("published")
            .value
            .as_deref(),
        Some("supplied")
    );
    assert!(
        calls.borrow().is_empty(),
        "an image the host already supplied is not a request"
    );
}

#[gpui::test]
fn a_code_block_publishes_the_info_string_it_was_given(cx: &mut TestAppContext) {
    let (mut harness, _events) = markdown(cx, DOCUMENT, None);

    let fenced = harness.node("doc.code-rust").expect("published");
    assert_eq!(fenced.text.as_deref(), Some("rust"));
    assert_eq!(fenced.value.as_deref(), Some("1 line"));

    let bare = harness.node("doc.code-plain-text").expect("published");
    assert_eq!(
        bare.text.as_deref(),
        Some("plain text"),
        "a fence with no info string must not be given a language"
    );
}

#[gpui::test]
fn every_heading_publishes_its_level(cx: &mut TestAppContext) {
    let (mut harness, _events) = markdown(cx, DOCUMENT, None);

    let title = harness
        .node("doc.heading-release-notes")
        .expect("published");
    assert_eq!(title.role, Role::Heading);
    assert_eq!(title.level, Some(1));

    let section = harness.node("doc.heading-what-changed").expect("published");
    assert_eq!(section.level, Some(2));
}

#[gpui::test]
fn a_task_item_publishes_its_state_and_installs_no_handler(cx: &mut TestAppContext) {
    let (mut harness, _events) = markdown(cx, DOCUMENT, None);

    let done = harness.node("doc.task-bounded-retries").expect("published");
    assert_eq!(done.checked, Some(true));
    assert!(
        done.disabled,
        "a rendered document ticks nothing, and says so"
    );
    assert_eq!(
        harness
            .node("doc.task-bounded-backoff")
            .expect("published")
            .checked,
        Some(false)
    );
}

#[gpui::test]
fn a_table_keeps_its_header_and_counts_its_rows(cx: &mut TestAppContext) {
    let (mut harness, _events) = markdown(cx, DOCUMENT, None);

    let table = harness.node("doc.table-stage").expect("published");
    assert_eq!(table.role, Role::Table);
    assert_eq!(table.value.as_deref(), Some("2 rows"));
}

#[gpui::test]
fn truncation_says_how_many_lines_it_left_out_and_reports_a_request(cx: &mut TestAppContext) {
    let (mut harness, events) = markdown(cx, DOCUMENT, Some(3));

    let cut = harness.node("doc.truncated").expect("published");
    let hidden: usize = cut
        .value
        .as_deref()
        .expect("a count")
        .parse()
        .expect("a number");
    assert!(hidden > 0);
    assert_eq!(
        cut.text.as_deref(),
        Some(format!("Show {hidden} more lines").as_str()),
        "the affordance states the count rather than fading it out"
    );
    assert!(
        harness.node("doc.table-stage").is_none(),
        "what was cut must actually be gone"
    );

    harness.click("doc.truncated.more");
    assert_eq!(
        events.borrow().as_slice(),
        [MarkdownEvent::MoreRequested { lines: hidden }]
    );
}

#[gpui::test]
fn two_links_to_one_destination_are_two_addressable_nodes(cx: &mut TestAppContext) {
    let (mut harness, _events) = markdown(
        cx,
        "[first](https://example.test/a) and [second](https://example.test/a)",
        None,
    );

    assert_eq!(
        harness
            .node("doc.link-https-example-test-a")
            .expect("published")
            .text
            .as_deref(),
        Some("first")
    );
    assert_eq!(
        harness
            .node("doc.link-https-example-test-a-2")
            .expect("published")
            .text
            .as_deref(),
        Some("second")
    );
}

/// A document whose source the test controls, drawn as one that is arriving.
fn arriving(cx: &mut TestAppContext, source: Rc<RefCell<String>>) -> Harness {
    Harness::new(cx, gpui_kit::install, move |_, _| {
        Markdown::new("doc", source.borrow().clone())
            .streaming(true)
            .into_any_element()
    })
}

#[gpui::test]
fn a_marker_that_has_not_closed_yet_is_read_as_though_it_had(cx: &mut TestAppContext) {
    // The alternative is four literal asterisks that vanish when the closer
    // lands, taking every wrap point after them along.
    let source = Rc::new(RefCell::new("A reply with **emphasis".to_string()));
    let mut harness = arriving(cx, source.clone());

    let hanging = harness.snapshot().ids().join(" ");
    assert!(
        !hanging.contains("**"),
        "the markers should not be showing as text: {hanging}"
    );

    *source.borrow_mut() = "A reply with **emphasis** and more.".to_string();
    harness.frame();
    assert!(
        harness.node("doc").is_some(),
        "the document still reads once the marker closes"
    );
}

#[gpui::test]
fn a_document_that_is_still_arriving_reads_the_same_as_one_that_arrived(cx: &mut TestAppContext) {
    // Every prefix is drawn, and the settled result has to be what a document
    // handed over whole would have been. Being fast is worth nothing if the
    // reader ends up with something else.
    let whole = "# Notes\n\nA paragraph with `code`.\n\n- one\n- two\n";
    let source = Rc::new(RefCell::new(String::new()));
    let mut streamed = arriving(cx, source.clone());
    for end in 1..=whole.len() {
        if !whole.is_char_boundary(end) {
            continue;
        }
        *source.borrow_mut() = whole[..end].to_string();
        streamed.frame();
    }
    let streamed: Vec<String> = streamed
        .snapshot()
        .ids()
        .iter()
        .map(|id| id.to_string())
        .collect();

    let mut settled = Harness::new(cx, gpui_kit::install, move |_, _| {
        Markdown::new("doc", whole).into_any_element()
    });
    let settled: Vec<String> = settled
        .snapshot()
        .ids()
        .iter()
        .map(|id| id.to_string())
        .collect();

    assert_eq!(streamed, settled, "watching it arrive changed what it says");
}

#[gpui::test]
fn a_link_whose_address_is_still_arriving_leads_nowhere(cx: &mut TestAppContext) {
    // Its words are readable straight away, because that is what a reader
    // reads. Its destination is not, because nobody has said one yet, and a
    // link to half an address is a link to somewhere else.
    let (calls, into) = sink::<MarkdownEvent>();
    let source = Rc::new(RefCell::new("Read [the notes](https://exa".to_string()));
    let mut harness = Harness::new(cx, gpui_kit::install, {
        let source = source.clone();
        move |_, _| {
            let into = into.clone();
            Markdown::new("doc", source.borrow().clone())
                .streaming(true)
                .on_event(move |event, _, _| into.borrow_mut().push(event.clone()))
                .into_any_element()
        }
    });

    let ids = harness.snapshot().ids().join(" ");
    assert!(
        !ids.contains("pending"),
        "the placeholder address must not reach the reader: {ids}"
    );
    assert!(
        calls.borrow().is_empty(),
        "an address that has not arrived cannot have been taken"
    );

    *source.borrow_mut() = "Read [the notes](https://example.com)".to_string();
    harness.frame();
    assert!(
        harness
            .snapshot()
            .ids()
            .iter()
            .any(|id| id.contains("link")),
        "the finished link is addressable"
    );
}

/// A paragraph written as one long run belongs to the column it was given,
/// not to its own unwrapped width. GPUI answers a min-content probe for text
/// with the width the whole run would take on one line, so a run that cannot
/// shrink takes the column with it and the reader loses the end of every
/// sentence off the edge of the pane.
#[gpui::test]
fn a_long_paragraph_wraps_inside_the_column_it_was_given(cx: &mut TestAppContext) {
    const PARAGRAPH: &str = "Colour contrast is the luminance difference between the text and \
the background behind it, and it has to be large enough that a person can read the words without \
leaning into the screen.";

    fn document(cx: &mut TestAppContext, width: f32) -> Harness {
        Harness::new(cx, gpui_kit::install, move |_, _| {
            gpui::div()
                .w(gpui::px(width))
                .child(Markdown::new("doc", PARAGRAPH).on_event(|_, _, _| {}))
                .into_any_element()
        })
    }

    let mut narrow = document(cx, 220.0);
    let narrow = narrow.node("doc").expect("the document publishes itself");
    let mut wide = document(cx, 1_600.0);
    let wide = wide.node("doc").expect("the document publishes itself");

    assert!(
        narrow.bounds.width <= 220.0,
        "the document grew past the column it was given: {}",
        narrow.bounds.width
    );
    assert!(
        narrow.bounds.height > wide.bounds.height,
        "the same paragraph took {} in a 220px column and {} in a 1600px one, \
so it never wrapped",
        narrow.bounds.height,
        wide.bounds.height
    );
}
