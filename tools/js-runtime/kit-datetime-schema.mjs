const string = { type: 'string', max: 16384 };
const boolean = { type: 'boolean' };
const token = { type: 'number', integer: true, min: -9007199254740991, max: 9007199254740991 };
const nullableDay = { ...token, nullable: true };
const part = { type: 'number', integer: true, min: 0, max: 99 };
const choice = (...values) => ({ enum: values });
const object = (fields, required = []) => ({ type: 'object', fields, required });
const array = (items, max = 4096) => ({ type: 'array', items, max });
const size = choice('xs', 'sm', 'md', 'lg');
const time = object({ hour: part, minute: part, second: { ...part, nullable: true }, meridiem: choice(0, 1, null) }, ['hour', 'minute']);
const range = { ...object({ start: token, end: nullableDay }, ['start']), nullable: true };
const marks = array(object({ day: token, label: string, tone: choice('neutral', 'accent', 'success', 'warning', 'danger', 'info') }, ['day', 'label']));
const clockFields = { hourMin: part, hourMax: part, minuteMax: part, secondMax: part, meridiem: { ...array(string, 2), nullable: true } };
export const adapterSchema = object({
  today: nullableDay,
  weekdays: array(string, 16),
  months: array(object({ month: token, label: string, weeks: array(array(object({ day: nullableDay, adjacent: boolean }), 16), 16) }, ['month', 'label', 'weeks']), 120),
  days: array(object({ day: token, month: token, label: string, formatted: string, aliases: array(string, 32), blocked: { ...string, nullable: true } }, ['day', 'month', 'label', 'formatted'])),
  parseError: string, unknownDay: string, completeRange: boolean,
  clock: object({ ...clockFields, hours: array(string, 100), minutes: array(string, 100), seconds: array(string, 100), separator: string, meridiemSeparator: string }, ['hourMin', 'hourMax', 'minuteMax', 'secondMax', 'hours', 'minutes', 'seconds', 'separator', 'meridiemSeparator']),
}, ['weekdays', 'months', 'days', 'parseError', 'unknownDay', 'clock']);
export const familySchemas = Object.freeze({
  Calendar: { props: object({ adapter: adapterSchema, selected: array(token), multi: boolean, month: token, range, disabled: boolean, marks }, ['adapter']), events: { pick: token, monthShown: token, hover: nullableDay }, slots: ['empty'] },
  DateInput: { props: object({ adapter: adapterSchema, value: nullableDay, required: boolean, invalid: boolean, disabled: boolean, size }, ['adapter']), events: { change: token, unparsable: object({ text: string, message: string }, ['text', 'message']), open: choice(null), close: choice(null), submit: choice(null) } },
  RangePicker: { props: object({ adapter: adapterSchema, range, disabled: boolean, invalid: boolean, marks }, ['adapter']), events: { startPick: token, endPick: token } },
  TimeInput: { props: object({ adapter: adapterSchema, value: time, seconds: boolean, disabled: boolean, invalid: boolean, size }, ['adapter']), events: { change: time } },
});
const command = fields => ({ args: object(fields, Object.keys(fields)), result: choice(null) });
const query = result => ({ args: object({}), result });
const disabled = { set_disabled: command({ disabled: boolean }) };
const invalid = { set_invalid: command({ invalid: boolean }) };
const density = { set_control_size: command({ size }) };
const overlay = { set_overlay: command({ marks }) };
const disabledQuery = { is_disabled: query(boolean) };
const calendarSnapshot = object({ selection: array(token), cursor: nullableDay, hoveredDay: nullableDay, shownMonth: nullableDay, disabled: boolean }, ['selection', 'cursor', 'hoveredDay', 'shownMonth', 'disabled']);
const fieldOffset = { type: 'number', min: 0, max: 16384, integer: true };
const fieldSnapshot = object({ value: string, cursor: fieldOffset, selection: object({ start: fieldOffset, end: fieldOffset }, ['start', 'end']), disabled: boolean }, ['value', 'cursor', 'selection', 'disabled']);
const adapterSnapshot = object({ today: nullableDay, weekdays: array(string, 16), clock: object(clockFields, Object.keys(clockFields)) }, ['today', 'weekdays', 'clock']);
const blockedReport = { oneOf: [...['notApplicable', 'unchecked', 'clear'].map(kind => object({ kind: choice(kind) }, ['kind'])), object({ kind: choice('blocked'), days: array(object({ day: token, reason: string }, ['day', 'reason'])) }, ['kind', 'days'])] };
export const familyMethods = Object.freeze({
  Calendar: { invoke: { ...disabled, ...overlay, reset_navigation: command({}), set_selection: command({ days: array(token) }), set_multi: command({ multi: boolean }), set_range: command({ range }), set_hovered_day: command({ day: nullableDay }), shift: command({ delta: { type: 'number', min: -120, max: 120, integer: true } }), show_month: command({ month: token }) }, query: { ...disabledQuery, adapter_snapshot: query(adapterSnapshot), selection: query(array(token)), cursor: query(nullableDay), hovered_day: query(nullableDay), shown_month: query(nullableDay) } },
  DateInput: { invoke: { ...disabled, ...invalid, ...density, set_value: command({ value: nullableDay }), set_required: command({ required: boolean }), open: command({}), close: command({}), toggle: command({}) }, query: { ...disabledQuery, field_snapshot: query(fieldSnapshot), calendar_snapshot: query(calendarSnapshot), current: query(nullableDay), parsed_day: query(nullableDay), message: query({ ...string, nullable: true }), is_open: query(boolean), shown_text: query(string), is_invalid: query(boolean) } },
  RangePicker: { invoke: { ...disabled, ...invalid, ...overlay, set_range: command({ range }) }, query: { ...disabledQuery, calendar_snapshot: query(calendarSnapshot), current_range: query(range), state: query(choice('unset', 'incomplete', 'complete', 'end before start')), blocked: query(blockedReport) } },
  TimeInput: { invoke: { ...disabled, ...invalid, ...density, set_value: command({ value: time }), set_seconds: command({ seconds: boolean }) }, query: { ...disabledQuery, current: query(time), active_segment: query(choice('hour', 'minute', 'second', 'meridiem')), clock: query(object(clockFields, Object.keys(clockFields))) } },
});

export function validateFamilyProps(component, props) {
  const d = props.adapter;
  const days = new Map(d.days.map(v => [v.day, v]));
  const months = new Set(d.months.map(v => v.month));
  if (!d.weekdays.length || days.size !== d.days.length || months.size !== d.months.length) throw new TypeError('invalid calendar identity or weekdays');
  const aliases = new Map();
  for (let i = 0; i < d.days.length; i++) {
    const day = d.days[i];
    if (!months.has(day.month) || (i && d.days[i - 1].day >= day.day)) throw new TypeError('invalid day ordering or month');
    for (const alias of [day.formatted, ...(day.aliases ?? [])]) {
      if (aliases.has(alias) && aliases.get(alias) !== day.day) throw new TypeError('ambiguous parse alias');
      aliases.set(alias, day.day);
    }
  }
  for (const month of d.months) {
    const seen = new Set();
    for (const week of month.weeks) {
      if (week.length !== d.weekdays.length) throw new TypeError('ragged month grid');
      for (const cell of week) if (cell.day != null) {
        if (!days.has(cell.day) || seen.has(cell.day) || Boolean(cell.adjacent) !== (days.get(cell.day).month !== month.month)) throw new TypeError('invalid month day');
        seen.add(cell.day);
      }
    }
  }
  const known = value => { if (value != null && !days.has(value)) throw new TypeError('unknown day'); };
  known(d.today);
  for (const day of props.selected ?? []) known(day);
  if (component === 'DateInput') known(props.value);
  if (props.range != null) { known(props.range.start); known(props.range.end); }
  if (props.month !== undefined && !months.has(props.month)) throw new TypeError('unknown month');
  const c = d.clock;
  if (c.hourMin > c.hourMax || c.hours.length !== c.hourMax + 1 || c.minutes.length !== c.minuteMax + 1 || c.seconds.length !== c.secondMax + 1 || (c.meridiem != null && c.meridiem.length !== 2)) throw new TypeError('invalid clock table');
  if (component === 'TimeInput' && props.value) {
    const t = props.value;
    if (t.hour < c.hourMin || t.hour > c.hourMax || t.minute > c.minuteMax || (t.second != null && t.second > c.secondMax) || (c.meridiem != null ? ![0, 1].includes(t.meridiem) : t.meridiem != null)) throw new TypeError('time outside caller clock');
  }
}
