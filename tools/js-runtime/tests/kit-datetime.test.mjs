import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { familySchemas, familyMethods, validateFamilyProps } from '../kit-datetime-schema.mjs';
import { validateValue } from '../kit-schema.mjs';
const data = () => JSON.parse(readFileSync(new URL('../../app-host/src/kit_bindings/datetime/fixture/data.json', import.meta.url)));
const check = (component, props) => { validateValue(props, familySchemas[component].props); validateFamilyProps(component, props); };
test('native reference getters remain unsupported, snapshots have distinct data-query names', () => {
  for (const [component, method] of [['DateInput', 'field'], ['DateInput', 'calendar'], ['RangePicker', 'calendar'], ['Calendar', 'adapter']]) {
    assert.equal(Object.hasOwn(familyMethods[component].query, method), false);
    assert.equal(Object.hasOwn(familyMethods[component].query, `${method}_snapshot`), true);
  }
});
test('finite calendar tables support opaque months and exact caller spellings', () => {
  check('Calendar', { adapter: data(), month: 4, selected: [31, 11], multi: true });
  check('RangePicker', { adapter: data(), range: { start: 31, end: 11 } });
  check('TimeInput', { adapter: data(), value: { hour: 3, minute: 1, second: 2 } });
  for (const mutate of [d => d.today = 999, d => d.days[1].aliases = ['alpha'], d => d.months[0].weeks[0].pop(), d => d.months[0].weeks[0][0].adjacent = true, d => d.clock.hours.pop(), d => d.clock.hourMin = 5]) {
    const adapter = data(); mutate(adapter); assert.throws(() => check('Calendar', { adapter }));
  }
  assert.throws(() => check('TimeInput', { adapter: data(), value: { hour: 3, minute: 5 } }));
  assert.throws(() => check('DateInput', { adapter: data(), value: 1000 }));
});
test('native invocation grammar refuses loose argument bags and unbounded results', () => {
  validateValue({ range: { start: 31, end: 11 } }, familyMethods.RangePicker.invoke.set_range.args);
  for (const args of [{ value: '31' }, { value: 31, notify: true }, {}]) assert.throws(() => validateValue(args, familyMethods.DateInput.invoke.set_value.args));
  const blocked = familyMethods.RangePicker.query.blocked.result;
  validateValue({ kind: 'unchecked' }, blocked);
  validateValue({ kind: 'blocked', days: [{ day: 17, reason: 'maintenance' }] }, blocked);
  assert.throws(() => validateValue({ kind: 'unchecked', days: [] }, blocked));
  assert.throws(() => validateValue({ kind: 'blocked' }, blocked));
});
