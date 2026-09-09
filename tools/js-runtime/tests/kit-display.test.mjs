import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { familySchemas, familyMethods, validateFamilyProps } from '../kit-display-schema.mjs';
import { validateValue } from '../kit-schema.mjs';
import { props } from '../../app-host/src/kit_bindings/display/fixture/props.mjs';
import { createKitBindings } from '../kit-bindings.mjs';
const validate = (component, value) => { validateValue(value, familySchemas[component].props); validateFamilyProps(component, value); };
test('real factories clone caller data and register only enabled typed actions', () => {
  const actions = new Map();
  const kit = createKitBindings((id, event, handler) => { const key = `${id}:${event}`; actions.set(key, handler); return key; });
  for (const [component, value] of Object.entries(props)) {
    const output = kit[component]('subject', value);
    assert.equal(output.component, component);
    assert.deepEqual(output.props, value);
    assert.notEqual(output.props, value);
  }
  let rating;
  kit.Rating('quality', { maximum: 5 }, { change: value => { rating = value; } });
  actions.get('quality:change')(2.5);
  assert.equal(rating, 2.5);
  assert.throws(() => actions.get('quality:change')('2.5'));
  kit.Rating('disabled', { disabled: true }, { change() { throw new Error('disabled'); } });
  assert.equal(actions.has('disabled:change'), false);
});
test('every display component has a native fixture and closed schema; generated data is current', () => {
  assert.deepEqual(Object.keys(props).sort(), Object.keys(familySchemas).sort());
  const index = JSON.parse(readFileSync(new URL('../../../docs/api-index.json', import.meta.url)));
  const expected = index.components.filter(c => c.source.startsWith('crates/gpui-kit/src/display/') && !c.name.endsWith('Chart') && !['Divider', 'Plot', 'Sparkline', 'ChartLegend'].includes(c.name)).map(c => c.name).sort();
  assert.deepEqual(Object.keys(familySchemas).sort(), expected);
  for (const [component, value] of Object.entries(props)) {
    assert.doesNotThrow(() => validate(component, value), component);
    assert.throws(() => validate(component, { ...value, accidental: true }), component);
  }
  for (const [name, value] of [['schemas', familySchemas], ['methods', familyMethods]]) assert.deepEqual(JSON.parse(readFileSync(new URL(`../../app-host/src/kit_bindings/display/${name}.json`, import.meta.url))), value);
});
test('data states cannot counterfeit ready values or silently become empty', () => {
  for (const [component, value] of [
    ['FailurePanel', { result: { ok: false } }],
    ['AnimatedNumber', { value: 1, spec: { durationMs: 12 } }],
    ['Heatmap', { label: 'Heat', state: { kind: 'error' } }],
    ['Timeline', { entries: [{ id: 'shared', description: 'A' }], groups: [{ id: 'today', label: 'Today', entries: [{ id: 'shared', description: 'B' }] }] }],
    ['MetricCard', { label: 'Revenue', state: { kind: 'stale', reason: 'refused' } }],
    ['MetricCard', { label: 'Revenue', state: { kind: 'empty', data: { value: '12' } } }],
    ['PerformanceHud', { state: { kind: 'ready' } }],
    ['AttachmentTile', { title: 'A', state: { kind: 'paused' } }],
    ['DescriptionList', { items: [{ id: 'secret', term: 'Secret', value: { kind: 'unknown', text: 'not unknown' } }] }],
    ['Skeleton', { shapes: [{ kind: 'circle', width: 0.2 }] }],
    ['Rating', { maximum: 3, value: 3.5 }],
    ['ProgressBar', { fraction: 0.4, count: { done: 7, total: 9 } }],
    ['TraceView', { label: 'Trace', spans: [{ id: 'backward', label: 'B', start: 0.8, end: 0.2 }] }],
    ['Heatmap', { label: 'Heat', cells: [{ id: 'missing', row: 'r', column: 'c' }] }],
  ]) assert.throws(() => validate(component, value), component);
});
test('nested identity, resource and glyph escape hatches are refused', () => {
  for (const value of [{ key: '../avatar' }, { key: 'https://host/avatar' }, { key: 'safe', path: '/etc/passwd' }, '/etc/passwd']) assert.throws(() => validate('Avatar', { name: 'Ada', image: value }));
  assert.doesNotThrow(() => validate('Avatar', { name: 'Ada', image: { key: 'approved-image' } }));
  assert.throws(() => validate('Badge', { label: 'Glyph', icon: { key: 'check', weight: 'bold' } }));
  assert.throws(() => validate('AvatarGroup', { members: [{ id: 'same', name: 'First' }, { id: 'same', name: 'Second' }] }));
  assert.throws(() => validate('Tag', { label: 'Color', color: { kind: 'semantic', name: 'invented' } }));
});
test('all declared query contracts have bounded named arguments and no command placeholders', () => {
  assert.deepEqual(Object.keys(familyMethods), ['HighlightedText', 'Icon']);
  for (const methods of Object.values(familyMethods)) {
    assert.deepEqual(methods.invoke, {});
    for (const method of Object.values(methods.query)) assert.throws(() => validateValue({ accidental: true }, method.args));
  }
  assert.throws(() => validateValue({ direction: 'sideways' }, familyMethods.Icon.query.flips_in.args));
});
