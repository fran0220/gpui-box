//! Dates and times, on a fixed fixture clock.

use std::rc::Rc;

use gpui::{AnyElement, App, Entity, Global, IntoElement, Window, div, prelude::*, px};
use gpui_kit_theme::{Space, TextTone, TypeScale};

use crate::datetime::fixture::FixtureDateAdapter;
use crate::datetime::{
    Calendar, DateInput, DayMark, DayRange, RangePicker, SharedDateAdapter, TimeInput, TimeOfDay,
};
use crate::display::badge::Tone;
use crate::foundation::{ActiveTheme, StyledExt};

use super::support::{row, stack};

struct TouchDates(Vec<gpui::AnyView>);
impl Global for TouchDates {}

pub(super) fn touch_dates(window: &mut Window, cx: &mut App) -> AnyElement {
    use crate::foundation::{Disableable, Sizable};
    use gpui_kit_theme::ControlSize;
    if !cx.has_global::<TouchDates>() {
        let adapter = adapter();
        let field = cx.new(|cx| {
            DateInput::new("scene.touch.date", adapter.clone(), window, cx)
                .name("Appointment date")
                .control_size(ControlSize::Touch)
                .presentation(crate::overlay::popover::PickerPresentation::Bottom)
        });
        let disabled = cx.new(|cx| {
            DateInput::new("scene.touch.date-disabled", adapter.clone(), window, cx)
                .name("Managed appointment date")
                .value(adapter.today().expect("fixture today"))
                .control_size(ControlSize::Touch)
                .disabled(true)
        });
        let invalid = cx.new(|cx| {
            DateInput::new("scene.touch.date-invalid", adapter.clone(), window, cx)
                .name("Invalid appointment date")
                .value(adapter.today().expect("fixture today"))
                .control_size(ControlSize::Touch)
                .invalid(true)
        });
        let time = cx.new(|cx| {
            TimeInput::new("scene.touch.time", adapter.clone(), window, cx)
                .value(TimeOfDay::new(9, 37))
                .control_size(ControlSize::Touch)
        });
        let range = cx.new(|cx| {
            let mut picker = RangePicker::new("scene.touch.range", adapter.clone(), window, cx);
            picker.set_control_size(ControlSize::Touch, cx);
            picker
        });
        cx.set_global(TouchDates(vec![
            field.into(),
            disabled.into(),
            invalid.into(),
            time.into(),
            range.into(),
        ]));
    }
    let theme = cx.theme().clone();
    stack(&theme)
        .w(px(390.0))
        .max_w_full()
        // Seven 48px day targets plus card padding require at least352px.
        // This exhibit deliberately grants the calendar the full narrow width.
        .px_0()
        .child(crate::foundation::text(
            &theme,
            TypeScale::Caption,
            "Touch dates · empty, disabled, invalid · full-width calendar (minimum352px)",
        ))
        .children(cx.global::<TouchDates>().0.clone())
        .into_any_element()
}

/// The pinned calendar every date scene runs on, so two captures of the
/// same scene are the same picture.
fn adapter() -> SharedDateAdapter {
    Rc::new(
        FixtureDateAdapter::pinned(2024, 3, 14)
            .blocking(2024, 3, 8, "The workspace is frozen for the release.")
            .blocking(2024, 3, 20, "Nobody is on call that day."),
    )
}

fn marks(day: crate::datetime::Day) -> Option<DayMark> {
    match day.0.rem_euclid(7) {
        0 => Some(DayMark::new("Two runs finished here").tone(Tone::Success)),
        3 => Some(DayMark::new("One run failed here").tone(Tone::Danger)),
        _ => None,
    }
}

struct SceneDates {
    month: Entity<Calendar>,
    unknown: Entity<Calendar>,
    incomplete: Entity<RangePicker>,
    preview: Entity<RangePicker>,
    blocked: Entity<RangePicker>,
    field: Entity<DateInput>,
    refused: Entity<DateInput>,
    clock: Entity<TimeInput>,
    twelve: Entity<TimeInput>,
}

impl Global for SceneDates {}

pub(super) fn ensure(window: &mut Window, cx: &mut App) {
    if cx.has_global::<SceneDates>() {
        return;
    }
    let pinned = adapter();
    let unknown_adapter: SharedDateAdapter = Rc::new(FixtureDateAdapter::without_today());
    let march = FixtureDateAdapter::pinned(2024, 3, 14);

    let month = cx.new(|cx| {
        Calendar::new("scene.calendar", pinned.clone(), window, cx)
            .selected([march.day(2024, 3, 14)])
            .overlay(marks)
    });
    let unknown = cx.new(|cx| {
        Calendar::new(
            "scene.calendar.unknown",
            unknown_adapter.clone(),
            window,
            cx,
        )
    });
    let incomplete = cx.new(|cx| {
        RangePicker::new("scene.range.incomplete", pinned.clone(), window, cx)
            .range(DayRange::starting(march.day(2024, 3, 11)))
    });
    let preview = cx.new(|cx| {
        RangePicker::new("scene.range.preview", pinned.clone(), window, cx)
            .range(DayRange::starting(march.day(2024, 3, 11)))
    });
    let blocked = cx.new(|cx| {
        RangePicker::new("scene.range.blocked", pinned.clone(), window, cx)
            .range(DayRange::new(march.day(2024, 3, 6), march.day(2024, 3, 9)))
    });
    let field = cx.new(|cx| {
        DateInput::new("scene.date.field", pinned.clone(), window, cx).value(march.day(2024, 3, 14))
    });
    let refused = cx.new(|cx| DateInput::new("scene.date.refused", pinned.clone(), window, cx));
    let clock = cx.new(|cx| {
        TimeInput::new("scene.time.clock", pinned.clone(), window, cx)
            .value(TimeOfDay::new(9, 30).with_second(0))
            .seconds(true)
    });
    let twelve_adapter: SharedDateAdapter =
        Rc::new(FixtureDateAdapter::pinned(2024, 3, 14).twelve_hour(true));
    let twelve = cx.new(|cx| {
        TimeInput::new("scene.time.twelve", twelve_adapter, window, cx)
            .value(TimeOfDay::new(9, 30).with_meridiem(1))
    });

    let hovered = march.day(2024, 3, 15);
    preview.update(cx, |picker, cx| {
        picker.calendar().update(cx, |calendar, cx| {
            calendar.set_hovered_day(Some(hovered), cx);
        });
    });
    let refused_field = refused.read(cx).field().clone();
    refused_field.update(cx, |input, cx| input.set_value("the fifth", cx));

    cx.set_global(SceneDates {
        month,
        unknown,
        incomplete,
        preview,
        blocked,
        field,
        refused,
        clock,
        twelve,
    });
}

pub(super) fn calendar(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure(window, cx);
    let theme = cx.theme().clone();
    let dates = cx.global::<SceneDates>();
    let (month, unknown) = (dates.month.clone(), dates.unknown.clone());
    stack(&theme)
        .child(
            row(&theme)
                .items_start()
                .gap(px(theme.space(Space::Lg)))
                .child(month)
                .child(unknown),
        )
        .child(
            crate::foundation::text(
                &theme,
                TypeScale::Body,
                "Every weekday name, month name, and blocked reason above came from the \
                     host. The calendar on the right has no today, so it draws no ring and \
                     guesses no month.",
            )
            .max_w(px(560.0))
            .text_tone(&theme, TextTone::Muted),
        )
        .into_any_element()
}

pub(super) fn date_range(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure(window, cx);
    let theme = cx.theme().clone();
    let dates = cx.global::<SceneDates>();
    let (incomplete, preview, blocked) = (
        dates.incomplete.clone(),
        dates.preview.clone(),
        dates.blocked.clone(),
    );
    // The first two ranges report the same thing, because they are the same
    // range; what differs is that the pointer is over a day in the second, so
    // the caption has to say which picture is which.
    let column = |caption: &'static str, picker: AnyElement| {
        div()
            .column()
            .w(px(320.0))
            .gap(px(theme.space(Space::Xs)))
            .child(
                crate::foundation::text(&theme, TypeScale::Caption, caption)
                    .text_tone(&theme, TextTone::Muted),
            )
            .child(picker)
    };
    stack(&theme)
        .child(
            div()
                .row()
                .items_start()
                .gap(px(theme.space(Space::Lg)))
                .child(column(
                    "a start with no end yet",
                    incomplete.into_any_element(),
                ))
                .child(column(
                    "the same start, previewing the end under the pointer",
                    preview.into_any_element(),
                )),
        )
        // The third picker used to overhang the fixed review frame and looked
        // narrower only because its right edge was clipped. Give every state
        // the same full-width review column instead of squeezing three across.
        .child(column(
            "a finished range with a blocked day inside it",
            blocked.into_any_element(),
        ))
        .into_any_element()
}

pub(super) fn date_time(window: &mut Window, cx: &mut App) -> AnyElement {
    ensure(window, cx);
    let theme = cx.theme().clone();
    let dates = cx.global::<SceneDates>();
    let (field, refused, clock, twelve) = (
        dates.field.clone(),
        dates.refused.clone(),
        dates.clock.clone(),
        dates.twelve.clone(),
    );
    stack(&theme)
        .child(div().w(px(280.0)).child(field))
        .child(div().w(px(280.0)).child(refused))
        .child(
            row(&theme)
                .gap(px(theme.space(Space::Lg)))
                .child(clock)
                .child(twelve),
        )
        .child(
            crate::foundation::text(
                &theme,
                TypeScale::Body,
                "What the field could not read is still in it, and the refusal is the \
                     adapter's own sentence.",
            )
            .max_w(px(560.0))
            .text_tone(&theme, TextTone::Muted),
        )
        .into_any_element()
}

#[cfg(test)]
mod touch_tests {
    use super::*;

    #[gpui::test]
    fn touch_calendar_days_fit_full_bleed_narrow_card(cx: &mut gpui::TestAppContext) {
        let mut harness = gpui_kit_testkit::harness::Harness::new(cx, crate::install, touch_dates);
        harness
            .context()
            .simulate_resize(gpui::size(px(361.0), px(701.0)));
        let snapshot = harness.snapshot();
        let card = snapshot
            .find("scene.touch.range.calendar")
            .expect("calendar card")
            .bounds;
        for (id, name) in [
            ("scene.touch.date", "Appointment date"),
            ("scene.touch.date-disabled", "Managed appointment date"),
            ("scene.touch.date-invalid", "Invalid appointment date"),
        ] {
            assert_eq!(
                snapshot.find(id).expect("date field").text.as_deref(),
                Some(name)
            );
            assert_eq!(
                snapshot
                    .find(&format!("{id}.field"))
                    .expect("date editor")
                    .text
                    .as_deref(),
                Some(name)
            );
        }
        assert!(card.x >= 0.0 && card.x + card.width <= 361.0);
        let days: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.id.starts_with("scene.touch.range.calendar.day-"))
            .collect();
        assert!(days.len() >= 28);
        for day in days {
            assert!(day.bounds.width >= 48.0 && day.bounds.height >= 48.0);
            assert!(
                day.bounds.x >= card.x && day.bounds.x + day.bounds.width <= card.x + card.width,
                "{} lies outside card: {:?} vs {:?}",
                day.id,
                day.bounds,
                card
            );
        }
    }
}
