//! Retained native date controls. Calendar/locale facts come from a bounded
//! caller-owned table, never a fixture, process clock, locale guess or RPC.
use super::*;
use anyhow::{Result, bail, ensure};
use serde::Deserialize;

pub(super) const COMPONENTS: &[&str] = &["Calendar", "DateInput", "RangePicker", "TimeInput"];

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DayData {
    day: i64,
    month: i64,
    label: String,
    formatted: String,
    #[serde(default)]
    aliases: Vec<String>,
    blocked: Option<String>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    day: Option<i64>,
    #[serde(default)]
    adjacent: bool,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MonthData {
    month: i64,
    label: String,
    weeks: Vec<Vec<Cell>>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClockData {
    hour_min: u32,
    hour_max: u32,
    minute_max: u32,
    second_max: u32,
    meridiem: Option<[String; 2]>,
    hours: Vec<String>,
    minutes: Vec<String>,
    seconds: Vec<String>,
    separator: String,
    meridiem_separator: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Data {
    today: Option<i64>,
    weekdays: Vec<String>,
    months: Vec<MonthData>,
    days: Vec<DayData>,
    parse_error: String,
    unknown_day: String,
    #[serde(default)]
    complete_range: bool,
    clock: ClockData,
}

impl Data {
    fn parse(v: &Value) -> Result<Self> {
        let data: Self = serde_json::from_value(v.clone())?;
        ensure!(
            !data.weekdays.is_empty() && data.weekdays.len() <= 16,
            "invalid weekday count"
        );
        ensure!(
            data.months.len() <= 120 && data.days.len() <= 4096,
            "calendar table exceeds budget"
        );
        let months: std::collections::HashSet<_> = data.months.iter().map(|m| m.month).collect();
        let days: std::collections::HashSet<_> = data.days.iter().map(|d| d.day).collect();
        ensure!(
            months.len() == data.months.len() && days.len() == data.days.len(),
            "duplicate calendar identity"
        );
        ensure!(
            data.days.windows(2).all(|w| w[0].day < w[1].day),
            "days must be chronologically ordered"
        );
        let mut aliases = HashMap::new();
        for d in &data.days {
            ensure!(months.contains(&d.month), "unknown day month");
            for alias in std::iter::once(&d.formatted).chain(&d.aliases) {
                ensure!(
                    aliases
                        .insert(alias, d.day)
                        .is_none_or(|other| other == d.day),
                    "ambiguous date parse alias"
                );
            }
        }
        for m in &data.months {
            ensure!(m.weeks.len() <= 16, "month grid exceeds budget");
            let mut seen = std::collections::HashSet::new();
            for week in &m.weeks {
                ensure!(week.len() == data.weekdays.len(), "ragged month grid");
                for cell in week {
                    if let Some(day) = cell.day {
                        ensure!(
                            days.contains(&day) && seen.insert(day),
                            "unknown or repeated grid day"
                        );
                        let owner = data
                            .days
                            .iter()
                            .find(|d| d.day == day)
                            .expect("day membership checked above")
                            .month;
                        ensure!(
                            cell.adjacent == (owner != m.month),
                            "incorrect adjacent month marker"
                        );
                    }
                }
            }
        }
        ensure!(
            data.today.is_none_or(|d| days.contains(&d)),
            "unknown today"
        );
        let c = &data.clock;
        ensure!(
            c.hour_min <= c.hour_max
                && c.hour_max <= 99
                && c.minute_max <= 99
                && c.second_max <= 99,
            "invalid clock bounds"
        );
        ensure!(
            c.hours.len() == c.hour_max as usize + 1
                && c.minutes.len() == c.minute_max as usize + 1
                && c.seconds.len() == c.second_max as usize + 1,
            "incomplete localized clock labels"
        );
        Ok(data)
    }
    fn day(&self, day: Day) -> Option<&DayData> {
        self.days.iter().find(|d| d.day == day.0)
    }
    fn known(&self, day: Option<Day>) -> Result<()> {
        ensure!(
            day.is_none_or(|d| self.day(d).is_some()),
            "unknown day token"
        );
        Ok(())
    }
    fn time(&self, time: TimeOfDay) -> Result<()> {
        let c = &self.clock;
        ensure!(
            time.hour >= c.hour_min
                && time.hour <= c.hour_max
                && time.minute <= c.minute_max
                && time.second.is_none_or(|s| s <= c.second_max),
            "time outside caller clock"
        );
        ensure!(
            match (&c.meridiem, time.meridiem) {
                (Some(_), Some(i)) => i < 2,
                (None, None) => true,
                _ => false,
            },
            "invalid meridiem"
        );
        Ok(())
    }
}

struct Adapter(Rc<RefCell<Data>>);
impl DateAdapter for Adapter {
    fn today(&self) -> Option<Day> {
        self.0.borrow().today.map(Day)
    }
    fn month_of(&self, day: Day) -> MonthKey {
        MonthKey(self.0.borrow().day(day).expect("validated live day").month)
    }
    fn month_grid(&self, month: MonthKey) -> MonthGrid {
        self.0
            .borrow()
            .months
            .iter()
            .find(|m| m.month == month.0)
            .map_or_else(MonthGrid::default, |m| {
                MonthGrid::new(m.weeks.iter().map(|w| {
                    w.iter()
                        .map(|c| match c.day {
                            None => MonthCell::Empty,
                            Some(d) if c.adjacent => MonthCell::Adjacent(Day(d)),
                            Some(d) => MonthCell::Day(Day(d)),
                        })
                        .collect()
                }))
            })
    }
    fn month_label(&self, month: MonthKey) -> SharedString {
        let d = self.0.borrow();
        d.months
            .iter()
            .find(|m| m.month == month.0)
            .map_or_else(|| d.unknown_day.clone(), |m| m.label.clone())
            .into()
    }
    fn weekday_labels(&self) -> Vec<SharedString> {
        self.0
            .borrow()
            .weekdays
            .iter()
            .cloned()
            .map(Into::into)
            .collect()
    }
    fn format_day(&self, day: Day) -> SharedString {
        let d = self.0.borrow();
        d.day(day)
            .map_or_else(|| d.unknown_day.clone(), |d| d.formatted.clone())
            .into()
    }
    fn day_label(&self, day: Day) -> SharedString {
        let d = self.0.borrow();
        d.day(day)
            .map_or_else(|| d.unknown_day.clone(), |d| d.label.clone())
            .into()
    }
    fn parse_day(&self, text: &str) -> std::result::Result<Day, SharedString> {
        let d = self.0.borrow();
        d.days
            .iter()
            .find(|d| d.formatted == text || d.aliases.iter().any(|a| a == text))
            .map(|d| Day(d.day))
            .ok_or_else(|| d.parse_error.clone().into())
    }
    fn shift_month(&self, month: MonthKey, delta: i32) -> Option<MonthKey> {
        let d = self.0.borrow();
        let i = d.months.iter().position(|m| m.month == month.0)?;
        let next = (i as i64).checked_add(delta as i64)?;
        d.months
            .get(usize::try_from(next).ok()?)
            .map(|m| MonthKey(m.month))
    }
    fn is_selectable(&self, day: Day) -> Selectability {
        let d = self.0.borrow();
        match d.day(day) {
            Some(DayData { blocked: None, .. }) => Selectability::Selectable,
            Some(DayData {
                blocked: Some(reason),
                ..
            }) => Selectability::blocked(reason.clone()),
            None => Selectability::blocked(d.unknown_day.clone()),
        }
    }
    fn days_in(&self, start: Day, end: Day) -> Option<Vec<Day>> {
        let d = self.0.borrow();
        if !d.complete_range || d.day(start).is_none() || d.day(end).is_none() {
            return None;
        }
        Some(
            d.days
                .iter()
                .filter(|d| d.day >= start.0 && d.day <= end.0)
                .map(|d| Day(d.day))
                .collect(),
        )
    }
    fn clock(&self) -> Clock {
        let d = self.0.borrow();
        let c = &d.clock;
        Clock {
            hour_min: c.hour_min,
            hour_max: c.hour_max,
            minute_max: c.minute_max,
            second_max: c.second_max,
            meridiem: c
                .meridiem
                .as_ref()
                .map(|m| (m[0].clone().into(), m[1].clone().into())),
        }
    }
    fn format_time(&self, time: TimeOfDay) -> SharedString {
        let d = self.0.borrow();
        let c = &d.clock;
        let mut parts = vec![
            c.hours
                .get(time.hour as usize)
                .cloned()
                .unwrap_or_else(|| d.unknown_day.clone()),
            c.minutes
                .get(time.minute as usize)
                .cloned()
                .unwrap_or_else(|| d.unknown_day.clone()),
        ];
        if let Some(second) = time.second {
            parts.push(
                c.seconds
                    .get(second as usize)
                    .cloned()
                    .unwrap_or_else(|| d.unknown_day.clone()),
            );
        }
        let mut formatted = parts.join(&c.separator);
        if let (Some(labels), Some(i)) = (&c.meridiem, time.meridiem)
            && let Some(label) = labels.get(i)
        {
            formatted.push_str(&c.meridiem_separator);
            formatted.push_str(label);
        }
        formatted.into()
    }
}

enum Control {
    Calendar(Entity<Calendar>),
    Date(Entity<DateInput>),
    Range(Entity<RangePicker>),
    Time(Entity<TimeInput>),
}
struct Entry {
    control: Control,
    data: Rc<RefCell<Data>>,
    route: Rc<RefCell<Route>>,
    props: RefCell<serde_json::Map<String, Value>>,
    _subscriptions: Vec<Subscription>,
}
#[derive(Default)]
pub(super) struct State {
    entries: RefCell<HashMap<Key, Rc<Entry>>>,
}
fn day(v: &Value) -> Option<Day> {
    v.as_i64().map(Day)
}
fn range(v: &Value) -> Option<DayRange> {
    day(&v["start"]).map(|start| DayRange {
        start,
        end: day(&v["end"]),
    })
}
fn time(v: &Value) -> TimeOfDay {
    TimeOfDay {
        hour: v["hour"].as_u64().unwrap_or(0) as u32,
        minute: v["minute"].as_u64().unwrap_or(0) as u32,
        second: v["second"].as_u64().map(|n| n as u32),
        meridiem: v["meridiem"].as_u64().map(|n| n as usize),
    }
}
fn time_json(v: TimeOfDay) -> Value {
    json!({"hour":v.hour,"minute":v.minute,"second":v.second,"meridiem":v.meridiem})
}
fn range_json(v: Option<DayRange>) -> Value {
    v.map_or(
        Value::Null,
        |r| json!({"start":r.start.0,"end":r.end.map(|d|d.0)}),
    )
}
fn marks(v: &Value) -> impl Fn(Day) -> Option<DayMark> + 'static {
    let marks = v.as_array().cloned().unwrap_or_default();
    move |day| {
        marks
            .iter()
            .find(|m| m["day"].as_i64() == Some(day.0))
            .map(|m| {
                DayMark::new(m["label"].as_str().unwrap_or_default().to_owned()).tone(
                    match m["tone"].as_str() {
                        Some("neutral") => Tone::Neutral,
                        Some("success") => Tone::Success,
                        Some("warning") => Tone::Warning,
                        Some("danger") => Tone::Danger,
                        Some("info") => Tone::Info,
                        _ => Tone::Accent,
                    },
                )
            })
    }
}
pub(super) fn validate(node: &Node) -> Result<()> {
    static SCHEMAS: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!("fixture/schemas.json"))
            .expect("generated datetime schemas")
    });
    let v = Value::Object(node.props.clone());
    super::validation::validate(
        &v,
        &SCHEMAS[node.component.as_deref().unwrap_or_default()]["props"],
    )?;
    let data = Data::parse(&v["adapter"])?;
    for d in v["selected"].as_array().into_iter().flatten() {
        data.known(day(d))?;
    }
    if node.component.as_deref() == Some("DateInput") {
        data.known(day(&v["value"]))?;
    }
    if let Some(r) = range(&v["range"]) {
        data.known(Some(r.start))?;
        data.known(r.end)?;
    }
    if let Some(m) = v["month"].as_i64() {
        ensure!(
            data.months.iter().any(|month| month.month == m),
            "unknown requested month"
        );
    }
    if node.component.as_deref() == Some("TimeInput") && v.get("value").is_some() {
        data.time(time(&v["value"]))?;
    }
    Ok(())
}

impl State {
    pub(super) fn reconcile(&self, root: &Node, _: &mut App) {
        fn visit(n: &Node, live: &mut HashMap<Key, String>) {
            if let Some(c) = &n.component {
                live.insert((n.instance, n.id.clone()), c.clone());
            }
            for child in n.children.iter().chain(n.slots.values().flatten()) {
                visit(child, live);
            }
        }
        let mut live = HashMap::new();
        visit(root, &mut live);
        self.entries.borrow_mut().retain(|key, e| {
            live.get(key).is_some_and(|c| {
                matches!(
                    (&e.control, c.as_str()),
                    (Control::Calendar(_), "Calendar")
                        | (Control::Date(_), "DateInput")
                        | (Control::Range(_), "RangePicker")
                        | (Control::Time(_), "TimeInput")
                )
            })
        });
    }
    pub(super) fn render(
        &self,
        node: &Node,
        slots: KitSlots,
        window: &mut Window,
        cx: &mut App,
        emit: Emit,
    ) -> AnyElement {
        let key = (node.instance, node.id.clone());
        let existing = self.entries.borrow().get(&key).cloned();
        let entry = existing.unwrap_or_else(|| {
            let data = Rc::new(RefCell::new(
                Data::parse(&node.props["adapter"]).expect("validated date adapter"),
            ));
            let adapter: SharedDateAdapter = Rc::new(Adapter(data.clone()));
            let route = Rc::new(RefCell::new(Route {
                events: node.events.clone(),
                emit: emit.clone(),
                disabled: flag(node, "disabled"),
            }));
            let callback = Rc::downgrade(&route);
            let send = move |name: &str, value: Value| {
                if let Some(route) = callback.upgrade() {
                    route.borrow().send(name, value);
                }
            };
            let (control, subscription) = match node.component.as_deref().unwrap_or_default() {
                "Calendar" => {
                    let entity = cx.new(|cx| Calendar::new(node.id.clone(), adapter, window, cx));
                    let sub = cx.subscribe(&entity, move |_, e: &CalendarEvent, _| match e {
                        CalendarEvent::Picked(d) => send("pick", json!(d.0)),
                        CalendarEvent::MonthShown(m) => send("monthShown", json!(m.0)),
                        CalendarEvent::Hovered(d) => send("hover", json!(d.map(|d| d.0))),
                    });
                    (Control::Calendar(entity), sub)
                }
                "DateInput" => {
                    let entity = cx.new(|cx| DateInput::new(node.id.clone(), adapter, window, cx));
                    let sub = cx.subscribe(&entity, move |_, e: &DateInputEvent, _| match e {
                        DateInputEvent::Changed(d) => send("change", json!(d.0)),
                        DateInputEvent::Unparsable { text, message } => send(
                            "unparsable",
                            json!({"text":text.as_ref(),"message":message.as_ref()}),
                        ),
                        DateInputEvent::Opened => send("open", Value::Null),
                        DateInputEvent::Closed => send("close", Value::Null),
                        DateInputEvent::Submit => send("submit", Value::Null),
                    });
                    (Control::Date(entity), sub)
                }
                "RangePicker" => {
                    let entity =
                        cx.new(|cx| RangePicker::new(node.id.clone(), adapter, window, cx));
                    let sub = cx.subscribe(&entity, move |_, e: &RangePickerEvent, _| match e {
                        RangePickerEvent::StartPicked(d) => send("startPick", json!(d.0)),
                        RangePickerEvent::EndPicked(d) => send("endPick", json!(d.0)),
                    });
                    (Control::Range(entity), sub)
                }
                "TimeInput" => {
                    let entity = cx.new(|cx| TimeInput::new(node.id.clone(), adapter, window, cx));
                    let sub = cx.subscribe(&entity, move |_, e: &TimeInputEvent, _| {
                        let TimeInputEvent::Changed(t) = e;
                        send("change", time_json(*t));
                    });
                    (Control::Time(entity), sub)
                }
                _ => unreachable!("date family dispatch"),
            };
            let entry = Rc::new(Entry {
                control,
                data,
                route,
                props: Default::default(),
                _subscriptions: vec![subscription],
            });
            self.entries.borrow_mut().insert(key, entry.clone());
            entry
        });
        let previous = entry.props.borrow().clone();
        let v = Value::Object(node.props.clone());
        let changed = |key: &str| previous.get(key) != node.props.get(key);
        // Locale labels and parse aliases are live data, not a request to erase
        // an in-progress edit or move the calendar. Only identity changes make
        // retained navigation potentially invalid.
        let domain = |value: &Value| {
            (
                value["days"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|day| (day["day"].clone(), day["month"].clone()))
                    .collect::<Vec<_>>(),
                value["months"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|month| month["month"].clone())
                    .collect::<Vec<_>>(),
            )
        };
        let old_adapter = previous.get("adapter").unwrap_or(&Value::Null);
        let reset_dates = domain(old_adapter) != domain(&v["adapter"]);
        let reset_clock = old_adapter["clock"]["hourMin"] != v["adapter"]["clock"]["hourMin"]
            || old_adapter["clock"]["hourMax"] != v["adapter"]["clock"]["hourMax"]
            || old_adapter["clock"]["minuteMax"] != v["adapter"]["clock"]["minuteMax"]
            || old_adapter["clock"]["secondMax"] != v["adapter"]["clock"]["secondMax"]
            || old_adapter["clock"]["meridiem"].is_null()
                != v["adapter"]["clock"]["meridiem"].is_null();
        let disabled = if changed("disabled") {
            flag(node, "disabled")
        } else {
            entry.route.borrow().disabled
        };
        *entry.route.borrow_mut() = Route {
            events: node.events.clone(),
            emit,
            disabled,
        };
        if changed("adapter") {
            *entry.data.borrow_mut() = Data::parse(&v["adapter"]).expect("validated date adapter");
        }
        match &entry.control {
            Control::Calendar(e) => e.update(cx, |c, cx| {
                if reset_dates {
                    c.reset_navigation(cx);
                }
                if changed("selected") || reset_dates {
                    c.set_selection(
                        v["selected"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(day)
                            .collect(),
                        cx,
                    );
                }
                if changed("multi") {
                    c.set_multi(flag(node, "multi"), cx);
                }
                if changed("range") || reset_dates {
                    c.set_range(range(&v["range"]), cx);
                }
                if (changed("month") || reset_dates)
                    && let Some(m) = v["month"].as_i64()
                {
                    c.show_month(MonthKey(m), cx);
                }
                if changed("marks") {
                    c.set_overlay(marks(&v["marks"]), cx);
                }
                if changed("disabled") {
                    c.set_disabled(disabled, cx);
                }
                *c.slots_mut() = Default::default();
                if let Some(factory) = slots.get("empty").cloned() {
                    c.slots_mut().set("empty", move |w, cx| factory(w, cx));
                }
            }),
            Control::Date(e) => e.update(cx, |c, cx| {
                if reset_dates {
                    c.calendar()
                        .clone()
                        .update(cx, |c, cx| c.reset_navigation(cx));
                }
                if changed("value") || reset_dates {
                    c.set_value(day(&v["value"]), cx);
                }
                if changed("required") {
                    c.set_required(flag(node, "required"), cx);
                }
                if changed("size") {
                    c.set_control_size(size(node), cx);
                }
                if changed("invalid") {
                    c.set_invalid(flag(node, "invalid"), cx);
                }
                if changed("disabled") {
                    c.set_disabled(disabled, cx);
                }
            }),
            Control::Range(e) => e.update(cx, |c, cx| {
                if reset_dates {
                    c.calendar()
                        .clone()
                        .update(cx, |c, cx| c.reset_navigation(cx));
                }
                if changed("range") || reset_dates {
                    c.set_range(range(&v["range"]), cx);
                }
                if changed("marks") {
                    c.set_overlay(marks(&v["marks"]), cx);
                }
                if changed("invalid") {
                    c.set_invalid(flag(node, "invalid"), cx);
                }
                if changed("disabled") {
                    c.set_disabled(disabled, cx);
                }
            }),
            Control::Time(e) => e.update(cx, |c, cx| {
                if (changed("value") || reset_clock) && v.get("value").is_some() {
                    c.set_value(time(&v["value"]), cx);
                }
                if reset_clock && v.get("value").is_none() {
                    let clock = c.clock();
                    c.set_value(
                        TimeOfDay {
                            hour: clock.hour_min,
                            minute: 0,
                            second: None,
                            meridiem: clock.meridiem.map(|_| 0),
                        },
                        cx,
                    );
                }
                if changed("seconds") || previous.is_empty() {
                    c.set_seconds(
                        v["seconds"]
                            .as_bool()
                            .unwrap_or(!v["value"]["second"].is_null()),
                        cx,
                    );
                }
                if changed("size") {
                    c.set_control_size(size(node), cx);
                }
                if changed("invalid") {
                    c.set_invalid(flag(node, "invalid"), cx);
                }
                if changed("disabled") {
                    c.set_disabled(disabled, cx);
                }
            }),
        }
        *entry.props.borrow_mut() = node.props.clone();
        match &entry.control {
            Control::Calendar(e) => e.clone().into_any_element(),
            Control::Date(e) => e.clone().into_any_element(),
            Control::Range(e) => e.clone().into_any_element(),
            Control::Time(e) => e.clone().into_any_element(),
        }
    }
    pub(super) fn invoke(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        _window: &mut Window,
        cx: &mut App,
    ) -> Result<Value> {
        static METHODS: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| {
            serde_json::from_str(include_str!("fixture/methods.json"))
                .expect("generated date methods")
        });
        let contract = METHODS
            .get(node.component.as_deref().unwrap_or_default())
            .and_then(|v| v.get(if query { "query" } else { "invoke" }))
            .and_then(|v| v.get(method))
            .ok_or_else(|| anyhow::anyhow!("unsupported date method"))?;
        super::validation::validate(args, &contract["args"])?;
        let result = self.dispatch(node, method, args, query, cx)?;
        super::validation::validate(&result, &contract["result"])?;
        Ok(result)
    }
    fn dispatch(
        &self,
        node: &Node,
        method: &str,
        args: &Value,
        query: bool,
        cx: &mut App,
    ) -> Result<Value> {
        let entry = self
            .entries
            .borrow()
            .get(&(node.instance, node.id.clone()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("native date target not mounted"))?;
        ensure!(
            query || (!flag(node, "disabled") && !entry.route.borrow().disabled),
            "disabled native date control refuses command"
        );
        let native_disabled = match &entry.control {
            Control::Calendar(e) => e.read(cx).is_disabled(),
            Control::Date(e) => e.read(cx).is_disabled(),
            Control::Range(e) => e.read(cx).is_disabled(),
            Control::Time(e) => e.read(cx).is_disabled(),
        };
        ensure!(
            query || !native_disabled,
            "disabled native entity refuses command"
        );
        if query && method == "is_disabled" {
            return Ok(json!(native_disabled));
        }
        if query {
            return Ok(match &entry.control {
                Control::Calendar(e) => {
                    let c = e.read(cx);
                    match method {
                        "selection" => json!(c.selection().iter().map(|d| d.0).collect::<Vec<_>>()),
                        "cursor" => json!(c.cursor().map(|d| d.0)),
                        "hovered_day" => json!(c.hovered_day().map(|d| d.0)),
                        "shown_month" => json!(c.shown_month().map(|m| m.0)),
                        "adapter" => {
                            let adapter = c.adapter();
                            json!({"today":adapter.today().map(|d|d.0),"weekdays":adapter.weekday_labels().iter().map(|s|s.as_ref()).collect::<Vec<_>>(),"clock":clock_json(adapter.clock())})
                        }
                        _ => bail!("unsupported Calendar query"),
                    }
                }
                Control::Date(e) => {
                    let c = e.read(cx);
                    match method {
                        "current" => json!(c.current().map(|d| d.0)),
                        "parsed_day" => json!(c.parsed_day(cx).map(|d| d.0)),
                        "message" => json!(c.message().map(|s| s.as_ref())),
                        "is_open" => json!(c.is_open()),
                        "shown_text" => json!(c.shown_text(cx).as_ref()),
                        "is_invalid" => json!(c.is_invalid()),
                        "calendar" => calendar_json(c.calendar().read(cx)),
                        "field" => {
                            let field = c.field().read(cx);
                            let range = field.selected_range();
                            json!({"value":field.value().as_ref(),"cursor":field.cursor_offset(),"selection":{"start":range.start,"end":range.end},"disabled":field.is_disabled()})
                        }
                        _ => bail!("unsupported DateInput query"),
                    }
                }
                Control::Range(e) => {
                    let c = e.read(cx);
                    match method {
                        "current_range" => range_json(c.current_range()),
                        "calendar" => calendar_json(c.calendar().read(cx)),
                        "state" => json!(c.state().name()),
                        "blocked" => match c.blocked() {
                            BlockedReport::NotApplicable => json!({"kind":"notApplicable"}),
                            BlockedReport::Unchecked => json!({"kind":"unchecked"}),
                            BlockedReport::Clear => json!({"kind":"clear"}),
                            BlockedReport::Blocked(days) => {
                                json!({"kind":"blocked","days":days.iter().map(|d|json!({"day":d.day.0,"reason":d.reason.as_ref()})).collect::<Vec<_>>()})
                            }
                        },
                        _ => bail!("unsupported RangePicker query"),
                    }
                }
                Control::Time(e) => {
                    let c = e.read(cx);
                    match method {
                        "current" => time_json(c.current()),
                        "active_segment" => json!(match c.active_segment() {
                            TimeSegment::Hour => "hour",
                            TimeSegment::Minute => "minute",
                            TimeSegment::Second => "second",
                            TimeSegment::Meridiem => "meridiem",
                        }),
                        "clock" => {
                            let c = c.clock();
                            json!({"hourMin":c.hour_min,"hourMax":c.hour_max,"minuteMax":c.minute_max,"secondMax":c.second_max,"meridiem":c.meridiem.map(|(a,b)|[a.to_string(),b.to_string()])})
                        }
                        _ => bail!("unsupported TimeInput query"),
                    }
                }
            });
        }
        if ["set_value", "set_hovered_day"].contains(&method)
            && !matches!(&entry.control, Control::Time(_))
        {
            entry
                .data
                .borrow()
                .known(day(&args[if method == "set_value" {
                    "value"
                } else {
                    "day"
                }]))?;
        }
        if method == "set_range"
            && let Some(r) = range(&args["range"])
        {
            entry.data.borrow().known(Some(r.start))?;
            entry.data.borrow().known(r.end)?;
        }
        match &entry.control {
            Control::Calendar(e) => e.update(cx, |c, cx| -> Result<()> {
                match method {
                    "set_selection" => {
                        let days = args["days"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(day)
                            .collect::<Vec<_>>();
                        for d in &days {
                            entry.data.borrow().known(Some(*d))?;
                        }
                        c.set_selection(days, cx);
                    }
                    "set_range" => c.set_range(range(&args["range"]), cx),
                    "set_hovered_day" => c.set_hovered_day(day(&args["day"]), cx),
                    "set_multi" => c.set_multi(args["multi"] == true, cx),
                    "reset_navigation" => c.reset_navigation(cx),
                    "set_overlay" => c.set_overlay(marks(&args["marks"]), cx),
                    "shift" => c.shift(args["delta"].as_i64().unwrap_or(0) as i32, cx),
                    "show_month" => {
                        let month = args["month"].as_i64().unwrap_or(0);
                        ensure!(
                            entry.data.borrow().months.iter().any(|m| m.month == month),
                            "unknown month token"
                        );
                        c.show_month(MonthKey(month), cx);
                    }
                    "set_disabled" => c.set_disabled(args["disabled"] == true, cx),
                    _ => bail!("unsupported Calendar command"),
                };
                Ok(())
            })?,
            Control::Date(e) => e.update(cx, |c, cx| -> Result<()> {
                match method {
                    "set_value" => c.set_value(day(&args["value"]), cx),
                    "set_invalid" => c.set_invalid(args["invalid"] == true, cx),
                    "set_required" => c.set_required(args["required"] == true, cx),
                    "set_control_size" => c.set_control_size(method_size(args), cx),
                    "set_disabled" => c.set_disabled(args["disabled"] == true, cx),
                    "open" => c.open(cx),
                    "close" => c.close(cx),
                    "toggle" => c.toggle(cx),
                    _ => bail!("unsupported DateInput command"),
                };
                Ok(())
            })?,
            Control::Range(e) => e.update(cx, |c, cx| -> Result<()> {
                match method {
                    "set_range" => c.set_range(range(&args["range"]), cx),
                    "set_overlay" => c.set_overlay(marks(&args["marks"]), cx),
                    "set_invalid" => c.set_invalid(args["invalid"] == true, cx),
                    "set_disabled" => c.set_disabled(args["disabled"] == true, cx),
                    _ => bail!("unsupported RangePicker command"),
                };
                Ok(())
            })?,
            Control::Time(e) => e.update(cx, |c, cx| -> Result<()> {
                match method {
                    "set_value" => {
                        let t = time(&args["value"]);
                        entry.data.borrow().time(t)?;
                        c.set_value(t, cx);
                    }
                    "set_seconds" => c.set_seconds(args["seconds"] == true, cx),
                    "set_control_size" => c.set_control_size(method_size(args), cx),
                    "set_invalid" => c.set_invalid(args["invalid"] == true, cx),
                    "set_disabled" => c.set_disabled(args["disabled"] == true, cx),
                    _ => bail!("unsupported TimeInput command"),
                };
                Ok(())
            })?,
        }
        if method == "set_disabled" {
            entry.route.borrow_mut().disabled = args["disabled"] == true;
        }
        Ok(Value::Null)
    }
}

fn method_size(args: &Value) -> ControlSize {
    match args["size"].as_str() {
        Some("xs") => ControlSize::Xs,
        Some("sm") => ControlSize::Sm,
        Some("lg") => ControlSize::Lg,
        _ => ControlSize::Md,
    }
}

fn clock_json(c: Clock) -> Value {
    json!({"hourMin":c.hour_min,"hourMax":c.hour_max,"minuteMax":c.minute_max,"secondMax":c.second_max,"meridiem":c.meridiem.map(|(a,b)|[a.to_string(),b.to_string()])})
}
fn calendar_json(c: &Calendar) -> Value {
    json!({"selection":c.selection().iter().map(|d|d.0).collect::<Vec<_>>(),"cursor":c.cursor().map(|d|d.0),"hoveredDay":c.hovered_day().map(|d|d.0),"shownMonth":c.shown_month().map(|m|m.0),"disabled":c.is_disabled()})
}

#[cfg(test)]
mod tests;
