import { test } from 'node:test';
import assert from 'node:assert/strict';
import { familySchemas, validateFamilyProps } from '../kit-layout_extra-schema.mjs';
import { validateValue } from '../kit-schema.mjs';
const check = (component, props) => { validateValue(props, familySchemas[component].props); validateFamilyProps(component, props); };
test('dock topology preserves caller order, active panel and floating geometry', () => {
  const props = { records: [{ id: 'root', kind: 'horizontal', ratio: 0.27 }, { id: 'left', parent: 'root', kind: 'stack', panels: ['a'] }, { id: 'right', parent: 'root', kind: 'stack', panels: ['b', 'c'], active: 'c' }], floating: [{ stack: { id: 'float', kind: 'stack', panels: ['d'] }, bounds: { x: 0.13, y: 0.29, width: 0.4, height: 0.6 } }] };
  check('DockTree', props);
  for (const mutate of [p => p.records[2].active = 'missing', p => p.records[1].parent = 'right', p => p.floating[0].stack.panels = ['c'], p => p.floating[0].bounds.height = 0.8, p => p.floating[0].stack.id = 'right']) {
    const invalid = structuredClone(props); mutate(invalid); assert.throws(() => check('DockTree', invalid));
  }
});
test('floating events distinguish cancellation from completion and live movement', () => {
  const schema = familySchemas.DockTree.events.event;
  for (const finished of [false, true]) validateValue({ kind: 'floatingChanged', stack: 'float', bounds: { x: .1, y: .2, width: .3, height: .4 }, finished }, schema);
  validateValue({ kind: 'floatingCancelled', stack: 'float' }, schema);
  assert.throws(() => validateValue({ kind: 'floatingChanged', stack: 'float' }, schema));
  assert.throws(() => validateValue({ kind: 'floatingCancelled', stack: 'float', finished: true }, schema));
});
test('layout schema rejects unknown effects, dimensions, paths and group identities', () => {
  for (const [component, props] of [['AspectRatio', { ratio: 0 }], ['ScrollEdgeEffect', { kind: 'custom' }], ['Toolbar', { groups: [], items: [{ id: 'a', group: 'missing', label: 'A' }] }], ['Container', { width: 'custom' }], ['DesktopTitlebar', { title: 'Test', buttons: { left: ['close'], right: ['close'] } }]]) assert.throws(() => check(component, props));
});
