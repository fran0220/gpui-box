import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { familySchemas, familyMethods, validateFamilyProps } from '../kit-charts-schema.mjs';
import { validateValue } from '../kit-schema.mjs';
import { props } from '../../app-host/src/kit_bindings/charts/fixture/props.mjs';
import { createKitBindings } from '../kit-bindings.mjs';
import { validateTree } from '../tree.mjs';
const validate = (component, value) => { validateValue(value, familySchemas[component].props); validateFamilyProps(component, value); };
test('actual chart factories retain caller series and typed business identity events', () => {
  const actions = new Map();
  const kit = createKitBindings((id, event, handler) => { actions.set(`${id}:${event}`, handler); return `${id}:${event}`; });
  for (const [component, value] of Object.entries(props)) {
    const output = kit[component]('chart', value);
    assert.deepEqual(output.props, value);
    assert.notEqual(output.props, value);
  }
  let selected;
  kit.AreaChart('area', props.AreaChart, { current: value => { selected = value; } });
  actions.get('area:current')({ seriesId: 'west', pointId: 'tue' });
  assert.deepEqual(selected, { seriesId: 'west', pointId: 'tue' });
  assert.throws(() => actions.get('area:current')({ seriesId: 'west', index: 1 }));
  kit.AreaChart('disabled', { ...props.AreaChart, disabled: true }, { current() {} });
  assert.equal(actions.has('disabled:current'), false);
});
test('every chart is accounted for by a supported adapter or explicit unbound contract', () => {
  assert.deepEqual(Object.keys(props).sort(), Object.keys(familySchemas).sort());
  assert.deepEqual(JSON.parse(readFileSync(new URL('../../app-host/src/kit_bindings/charts/fixture/props.json', import.meta.url))), props);
  const index = JSON.parse(readFileSync(new URL('../../../docs/api-index.json', import.meta.url)));
  const expected = index.components.filter(c => c.source.startsWith('crates/gpui-kit/src/display/') && (c.name.endsWith('Chart') || ['Plot', 'Sparkline', 'ChartLegend'].includes(c.name))).map(c => c.name).sort();
  const coverage = JSON.parse(readFileSync(new URL('../binding-coverage.json', import.meta.url)));
  // Raw scales/series and specialized layouts do not have wire adapters yet.
  // Keep this list explicit: a new catalog component must fail this partition.
  const unbound = ['CartesianChart', 'SpecializedChart'];
  assert.deepEqual([...Object.keys(familySchemas), ...unbound].sort(), expected);
  const kit = createKitBindings(() => assert.fail('unsupported component registered an action'));
  for (const component of unbound) {
    assert.deepEqual(coverage.components.filter(c => c.name === component).map(c => c.binding), [
      { status: 'unbound', reason: 'Native adapter and behavioral tests not implemented' },
    ], component);
    assert.equal(Object.hasOwn(kit, component), false, component);
    const node = { kind: 'kit', component, id: 'unsupported', props: {}, slots: {}, events: {} };
    for (const tree of [node, { kind: 'column', id: 'root', children: [node] }]) {
      assert.throws(() => validateTree(tree), { name: 'TypeError', message: `Unsupported Kit component: ${component}` });
    }
  }
  for (const [component, value] of Object.entries(props)) {
    assert.equal(coverage.components.find(c => c.name === component)?.binding.nativeIntegration, 'registered', component);
    assert.doesNotThrow(() => validate(component, value), component);
    assert.throws(() => validate(component, { ...value, plotUrl: 'https://invalid' }));
  }
  for (const [name, value] of [['schemas', familySchemas], ['methods', familyMethods]]) assert.deepEqual(JSON.parse(readFileSync(new URL(`../../app-host/src/kit_bindings/charts/${name}.json`, import.meta.url))), value);
});
test('OHLC, mark geometry, series ids and state distinctions reject asymmetric invalid input', () => {
  const candle = structuredClone(props.CandlestickChart); candle.state.data[0].high = 0.4;
  assert.throws(() => validate('CandlestickChart', candle));
  const plot = structuredClone(props.Plot); plot.state.data[0].bounds.width = 0.95;
  assert.throws(() => validate('Plot', plot));
  const line = structuredClone(props.LineChart); line.state.data[0].points.push({ ...line.state.data[0].points[0], value: 'conflict' });
  assert.throws(() => validate('LineChart', line));
  for (const state of [{ kind: 'ready' }, { kind: 'stale', data: [] }, { kind: 'empty', reason: 'actually failed' }, { kind: 'unavailable' }]) assert.throws(() => validate('LineChart', { label: 'L', state }));
  const sankey = structuredClone(props.SankeyChart); sankey.state.data.links[0].target = 'unknown';
  assert.throws(() => validate('SankeyChart', sankey));
});
test('custom painting is closed normalized native geometry, not a JS callback or source URL', () => {
  for (const paint of [() => {}, [{ kind: 'line', points: [{ x: 0.2, y: 0.7 }], width: 2, tint: { h: 0, s: 0, l: 0, a: 1 } }], [{ kind: 'rect', bounds: { x: 0.8, y: 0, width: 0.4, height: 0.2 }, tint: { h: 0, s: 0, l: 0, a: 1 } }]]) assert.throws(() => validate('Plot', { ...props.Plot, paint }));
});
test('layout query accepts caller data and rejects positional arguments and nonfinite weights', () => {
  const schema = familyMethods.SankeyChart.query.layout.args;
  validateValue({ data: props.SankeyChart.state.data, weights: [13], nodeWidth: 0.08, gap: 0.1, alignment: 'justify' }, schema);
  assert.throws(() => validateValue([props.SankeyChart.state.data, [13]], schema));
  assert.throws(() => validateValue({ data: props.SankeyChart.state.data, weights: [Infinity], nodeWidth: 0.08, gap: 0.1, alignment: 'left' }, schema));
  assert.deepEqual(familyMethods.SankeyChart.invoke, {});
});
