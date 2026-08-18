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
    stack(&theme)
        .child(
            row(&theme)
                .items_start()
                .gap(px(theme.space(Space::Lg)))
                .child(incomplete)
                .child(preview)
                .child(blocked),
        )
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
