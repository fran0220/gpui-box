//! The date components own no calendar. Everything they draw comes from the
//! adapter, and everything the adapter refuses to answer is shown as a
//! refusal rather than guessed at.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, IntoElement, SharedString, TestAppContext};
use gpui_kit::datetime::fixture::FixtureDateAdapter;
use gpui_kit::prelude::*;
use gpui_kit_testkit::harness::Harness;

/// The pinned calendar every test here runs on.
fn pinned() -> FixtureDateAdapter {
    FixtureDateAdapter::pinned(2024, 3, 14)
        .blocking(2024, 3, 8, "The workspace is frozen for the release.")
        .blocking(2024, 3, 20, "Nobody is on call that day.")
}

fn shared(adapter: FixtureDateAdapter) -> SharedDateAdapter {
    Rc::new(adapter)
}

fn day(year: i32, month: u32, date: u32) -> Day {
    FixtureDateAdapter::without_today().day(year, month, date)
}

fn day_id(prefix: &str, day: Day) -> String {
    format!("{prefix}.day-{}", day.0)
}

fn calendar(
    cx: &mut TestAppContext,
    adapter: FixtureDateAdapter,
    selection: Vec<Day>,
) -> (Harness, Entity<Calendar>) {
    let slot: Rc<RefCell<Option<Entity<Calendar>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let adapter = shared(adapter);
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    Calendar::new("runs.calendar", adapter.clone(), window, cx)
                        .selected(selection.clone())
                })
            })
            .clone()
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("calendar was built");
    (harness, entity)
}

fn picked(harness: &mut Harness, calendar: &Entity<Calendar>) -> Rc<RefCell<Vec<Day>>> {
    let sink: Rc<RefCell<Vec<Day>>> = Rc::new(RefCell::new(Vec::new()));
    let into = sink.clone();
    let entity = calendar.clone();
    harness.update(move |_, cx| {
        cx.subscribe(&entity, move |_, event: &CalendarEvent, _| {
            if let CalendarEvent::Picked(day) = event {
                into.borrow_mut().push(*day);
            }
        })
        .detach();
    });
    sink
}

#[gpui::test]
fn without_a_today_no_ring_is_drawn_and_no_month_is_guessed(cx: &mut TestAppContext) {
    let (mut harness, _entity) = calendar(cx, FixtureDateAdapter::without_today(), Vec::new());

    let node = harness.node("runs.calendar").expect("published");
    assert_eq!(node.value.as_deref(), Some("month unknown"));
    assert!(
        harness.node("runs.calendar.today").is_none(),
        "a calendar with no today must not mark one"
    );
    assert!(
        harness
            .snapshot()
            .ids()
            .iter()
            .all(|id| !id.contains(".day-")),
        "a calendar that does not know its month must not render one"
    );
    assert!(
        harness
            .node("runs.calendar.unknown-month")
            .expect("published")
            .value
            .as_deref()
            == Some("unavailable")
    );
}

#[gpui::test]
fn a_known_today_is_marked_and_opens_its_own_month(cx: &mut TestAppContext) {
    let (mut harness, _entity) = calendar(cx, pinned(), Vec::new());

    assert_eq!(
        harness
            .node("runs.calendar")
            .expect("published")
            .text
            .as_deref(),
        Some("March 2024")
    );
    assert_eq!(
        harness
            .node("runs.calendar.today")
            .expect("published")
            .text
            .as_deref(),
        Some("2024-03-14")
    );
}

#[gpui::test]
fn the_weekday_headings_are_the_adapters_own(cx: &mut TestAppContext) {
    let (mut harness, entity) = calendar(cx, pinned(), Vec::new());
    let labels = harness.update(move |_, cx| entity.read(cx).adapter().weekday_labels());

    assert_eq!(
        labels
            .iter()
            .map(SharedString::to_string)
            .collect::<Vec<_>>(),
        vec!["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
    );
}

#[gpui::test]
fn a_blocked_day_installs_no_handler_and_states_its_reason(cx: &mut TestAppContext) {
    let (mut harness, entity) = calendar(cx, pinned(), Vec::new());
    let sink = picked(&mut harness, &entity);
    let blocked = day_id("runs.calendar", day(2024, 3, 8));

    let node = harness.node(&blocked).expect("published");
    assert!(node.disabled, "a blocked day publishes itself refused");
    assert_eq!(
        node.value.as_deref(),
        Some("The workspace is frozen for the release."),
        "the host's reason is published word for word"
    );

    harness.click(&blocked);
    assert!(
        sink.borrow().is_empty(),
        "a blocked day must install no click handler"
    );
}

#[gpui::test]
fn a_selectable_day_reports_itself_without_moving_the_selection(cx: &mut TestAppContext) {
    let (mut harness, entity) = calendar(cx, pinned(), Vec::new());
    let sink = picked(&mut harness, &entity);
    let target = day(2024, 3, 12);

    harness.click(&day_id("runs.calendar", target));

    assert_eq!(*sink.borrow(), vec![target]);
    assert_eq!(
        harness
            .node(&day_id("runs.calendar", target))
            .expect("published")
            .checked,
        Some(false),
        "the selection is the caller's, so the calendar has not moved it"
    );
}

#[gpui::test]
fn month_navigation_goes_through_the_adapter_and_stops_where_it_refuses(cx: &mut TestAppContext) {
    let adapter = FixtureDateAdapter::pinned(2024, 3, 14).month_bounds(
        FixtureDateAdapter::without_today().month(2024, 3),
        FixtureDateAdapter::without_today().month(2024, 4),
    );
    let (mut harness, _entity) = calendar(cx, adapter, Vec::new());

    harness.click("runs.calendar.next");
    assert_eq!(
        harness
            .node("runs.calendar")
            .expect("published")
            .text
            .as_deref(),
        Some("April 2024")
    );

    harness.click("runs.calendar.next");
    assert_eq!(
        harness
            .node("runs.calendar")
            .expect("published")
            .text
            .as_deref(),
        Some("April 2024"),
        "a month the adapter refuses is a month the calendar does not travel to"
    );
}

#[gpui::test]
fn the_keyboard_walks_the_grid_the_adapter_produced(cx: &mut TestAppContext) {
    let (mut harness, entity) = calendar(cx, pinned(), vec![day(2024, 3, 14)]);
    let sink = picked(&mut harness, &entity);

    harness.click(&day_id("runs.calendar", day(2024, 3, 14)));
    sink.borrow_mut().clear();
    harness.keystrokes("right enter");
    assert_eq!(*sink.borrow(), vec![day(2024, 3, 15)]);

    sink.borrow_mut().clear();
    harness.keystrokes("down enter");
    assert_eq!(*sink.borrow(), vec![day(2024, 3, 22)]);

    sink.borrow_mut().clear();
    harness.keystrokes("home enter");
    assert_eq!(*sink.borrow(), vec![day(2024, 3, 18)]);
}

#[gpui::test]
fn a_host_overlay_marks_the_days_it_chose(cx: &mut TestAppContext) {
    let slot: Rc<RefCell<Option<Entity<Calendar>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let adapter = shared(pinned());
    let marked = day(2024, 3, 12);
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    Calendar::new("runs.calendar", adapter.clone(), window, cx).overlay(
                        move |day| (day == marked).then(|| DayMark::new("Two runs finished here")),
                    )
                })
            })
            .clone()
            .into_any_element()
    });

    let mark = harness
        .node(&format!("{}.mark", day_id("runs.calendar", marked)))
        .expect("the host's mark is published");
    assert_eq!(mark.text.as_deref(), Some("Two runs finished here"));
    assert!(
        harness
            .node(&format!(
                "{}.mark",
                day_id("runs.calendar", day(2024, 3, 13))
            ))
            .is_none(),
        "only the days the host marked carry a mark"
    );
}

fn date_input(cx: &mut TestAppContext) -> (Harness, Entity<DateInput>) {
    let slot: Rc<RefCell<Option<Entity<DateInput>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let adapter = shared(pinned());
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    DateInput::new("filters.from", adapter.clone(), window, cx)
                        .value(day(2024, 3, 14))
                })
            })
            .clone()
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("date input was built");
    (harness, entity)
}

#[gpui::test]
fn an_unreadable_entry_stays_in_the_field_and_publishes_invalid(cx: &mut TestAppContext) {
    let (mut harness, entity) = date_input(cx);
    let reports: Rc<RefCell<Vec<(String, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = reports.clone();
    harness.update({
        let entity = entity.clone();
        move |_, cx| {
            cx.subscribe(&entity, move |_, event: &DateInputEvent, _| {
                if let DateInputEvent::Unparsable { text, message } = event {
                    sink.borrow_mut()
                        .push((text.to_string(), message.to_string()));
                }
            })
            .detach();
        }
    });

    harness.update({
        let entity = entity.clone();
        move |_, cx| {
            let field = entity.read(cx).field().clone();
            field.update(cx, |field, cx| field.set_value("the fifth", cx));
        }
    });

    let node = harness.node("filters.from").expect("published");
    assert!(node.invalid, "unreadable text publishes the field invalid");
    assert_eq!(
        node.value.as_deref(),
        Some("the fifth"),
        "the field keeps exactly what was typed"
    );
    assert_eq!(
        reports.borrow().first().map(|(text, _)| text.as_str()),
        Some("the fifth")
    );
}

#[gpui::test]
fn the_adapters_parse_message_is_shown_verbatim(cx: &mut TestAppContext) {
    let (mut harness, entity) = date_input(cx);
    harness.update({
        let entity = entity.clone();
        move |_, cx| {
            let field = entity.read(cx).field().clone();
            field.update(cx, |field, cx| field.set_value("2024-02-30", cx));
        }
    });

    assert_eq!(
        harness
            .node("filters.from.message")
            .expect("published")
            .text
            .as_deref(),
        Some("The fixture calendar reads dates as 2024-03-05 and nothing else.")
    );
}

#[gpui::test]
fn a_readable_entry_clears_the_refusal_and_reports_the_day(cx: &mut TestAppContext) {
    let (mut harness, entity) = date_input(cx);
    let days: Rc<RefCell<Vec<Day>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = days.clone();
    harness.update({
        let entity = entity.clone();
        move |_, cx| {
            cx.subscribe(&entity, move |_, event: &DateInputEvent, _| {
                if let DateInputEvent::Changed(day) = event {
                    sink.borrow_mut().push(*day);
                }
            })
            .detach();
        }
    });

    harness.update({
        let entity = entity.clone();
        move |_, cx| {
            let field = entity.read(cx).field().clone();
            field.update(cx, |field, cx| field.set_value("2024-03-21", cx));
        }
    });

    assert_eq!(*days.borrow(), vec![day(2024, 3, 21)]);
    assert!(harness.node("filters.from.message").is_none());
    assert!(!harness.node("filters.from").expect("published").invalid);
}

fn range_picker(
    cx: &mut TestAppContext,
    range: Option<DayRange>,
) -> (Harness, Entity<RangePicker>) {
    let slot: Rc<RefCell<Option<Entity<RangePicker>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let adapter = shared(pinned());
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    let picker = RangePicker::new("filters.window", adapter.clone(), window, cx);
                    match range {
                        Some(range) => picker.range(range),
                        None => picker,
                    }
                })
            })
            .clone()
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("range picker was built");
    (harness, entity)
}

#[gpui::test]
fn an_incomplete_range_is_a_state_of_its_own(cx: &mut TestAppContext) {
    let (mut harness, _entity) = range_picker(cx, Some(DayRange::starting(day(2024, 3, 11))));

    let node = harness.node("filters.window").expect("published");
    assert_eq!(node.value.as_deref(), Some("incomplete"));
    assert!(
        !node.invalid,
        "a range that is not finished yet is not a failure"
    );
}

#[gpui::test]
fn an_end_before_the_start_is_reported_rather_than_swapped(cx: &mut TestAppContext) {
    let (mut harness, _entity) =
        range_picker(cx, Some(DayRange::new(day(2024, 3, 18), day(2024, 3, 11))));

    assert_eq!(
        harness
            .node("filters.window")
            .expect("published")
            .value
            .as_deref(),
        Some("end before start")
    );
    let summary = harness.node("filters.window.summary").expect("published");
    assert_eq!(
        summary.text.as_deref(),
        Some("The end, 2024-03-11, comes before the start, 2024-03-18.")
    );
}

#[gpui::test]
fn a_blocked_day_inside_a_range_is_named(cx: &mut TestAppContext) {
    let (mut harness, _entity) =
        range_picker(cx, Some(DayRange::new(day(2024, 3, 6), day(2024, 3, 9))));

    let named = harness
        .node(&format!("filters.window.blocked-{}", day(2024, 3, 8).0))
        .expect("the blocked day inside the range is published");
    assert_eq!(
        named.value.as_deref(),
        Some("The workspace is frozen for the release.")
    );
    assert_eq!(
        harness
            .node("filters.window")
            .expect("published")
            .value
            .as_deref(),
        Some("complete"),
        "naming a blocked day is a report, not a judgement about the range"
    );
}

#[gpui::test]
fn picking_fills_the_start_then_the_end(cx: &mut TestAppContext) {
    let (mut harness, entity) = range_picker(cx, None);
    let reports: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = reports.clone();
    harness.update({
        let entity = entity.clone();
        move |_, cx| {
            cx.subscribe(&entity, move |_, event: &RangePickerEvent, _| {
                sink.borrow_mut().push(match event {
                    RangePickerEvent::StartPicked(day) => format!("start {}", day.0),
                    RangePickerEvent::EndPicked(day) => format!("end {}", day.0),
                });
            })
            .detach();
        }
    });

    harness.click(&day_id("filters.window.calendar", day(2024, 3, 11)));
    assert_eq!(
        *reports.borrow(),
        vec![format!("start {}", day(2024, 3, 11).0)]
    );

    // The host applies what it accepted; only then is the next pick an end.
    harness.update({
        let entity = entity.clone();
        move |_, cx| {
            entity.update(cx, |picker, cx| {
                picker.set_range(Some(DayRange::starting(day(2024, 3, 11))), cx);
            });
        }
    });
    harness.click(&day_id("filters.window.calendar", day(2024, 3, 15)));
    assert_eq!(
        reports.borrow().last().cloned(),
        Some(format!("end {}", day(2024, 3, 15).0))
    );
}

fn time_input(
    cx: &mut TestAppContext,
    adapter: FixtureDateAdapter,
    value: TimeOfDay,
) -> (Harness, Entity<TimeInput>) {
    let slot: Rc<RefCell<Option<Entity<TimeInput>>>> = Rc::new(RefCell::new(None));
    let build_slot = slot.clone();
    let adapter = shared(adapter);
    let mut harness = Harness::new(cx, gpui_kit::install, move |window, cx| {
        build_slot
            .borrow_mut()
            .get_or_insert_with(|| {
                cx.new(|cx| TimeInput::new("schedule.at", adapter.clone(), window, cx).value(value))
            })
            .clone()
            .into_any_element()
    });
    harness.snapshot();
    let entity = slot.borrow().clone().expect("time input was built");
    (harness, entity)
}

#[gpui::test]
fn a_time_segment_steps_within_the_bounds_the_clock_gave_it(cx: &mut TestAppContext) {
    let (mut harness, entity) = time_input(cx, pinned(), TimeOfDay::new(23, 59));

    harness.click("schedule.at.hour");
    harness.keystrokes("up");
    assert_eq!(
        harness.update({
            let entity = entity.clone();
            move |_, cx| entity.read(cx).current()
        }),
        TimeOfDay::new(23, 59),
        "the last hour has no next hour, so the step reports nothing"
    );

    harness.keystrokes("down");
    assert_eq!(
        harness.update({
            let entity = entity.clone();
            move |_, cx| entity.read(cx).current()
        }),
        TimeOfDay::new(22, 59)
    );

    harness.keystrokes("right up");
    assert_eq!(
        harness.update(move |_, cx| entity.read(cx).current()),
        TimeOfDay::new(22, 59),
        "the minute segment has ends too"
    );
}

#[gpui::test]
fn the_meridiem_labels_come_from_the_adapter(cx: &mut TestAppContext) {
    let (mut harness, _entity) = time_input(
        cx,
        FixtureDateAdapter::pinned(2024, 3, 14).twelve_hour(true),
        TimeOfDay::new(9, 30).with_meridiem(1),
    );

    assert_eq!(
        harness
            .node("schedule.at.meridiem")
            .expect("published")
            .value
            .as_deref(),
        Some("PM")
    );
    assert_eq!(
        harness
            .node("schedule.at")
            .expect("published")
            .value
            .as_deref(),
        Some("09:30 PM")
    );
}

#[gpui::test]
fn typing_overwrites_the_segment_the_keyboard_is_on(cx: &mut TestAppContext) {
    let (mut harness, entity) = time_input(cx, pinned(), TimeOfDay::new(9, 30));

    harness.click("schedule.at.hour");
    harness.keystrokes("1 4");
    assert_eq!(
        harness.update({
            let entity = entity.clone();
            move |_, cx| entity.read(cx).current().hour
        }),
        14
    );
    assert_eq!(
        harness.update(move |_, cx| entity.read(cx).active_segment()),
        TimeSegment::Minute,
        "two digits complete an hour and move on"
    );
}
