import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { familySchemas, familyMethods, validateFamilyProps } from '../kit-controls_extra-schema.mjs';
import { validateValue } from '../kit-schema.mjs';

test('color payloads are complete, finite normalized HSLA without executable fields', () => {
  const color = { h: 0.125, s: 0.75, l: 0.25, a: 0.5 };
  const schema = familySchemas.ColorPicker.props;
  validateValue({ value: color, presets: [color], alpha: true }, schema);
  for (const value of [{ ...color, h: -0.01 }, { ...color, a: 1.01 }, { ...color, s: NaN }, { ...color, l: Infinity }, { h: 0, s: 0, l: 0 }, { ...color, path: '/tmp/icon' }]) {
    assert.throws(() => validateValue({ value }, schema));
  }
  assert.throws(() => validateValue({ value: color, bind: { signal: 1 } }, schema));
});

test('filter identities and all method argument/result contracts fail closed', () => {
  const condition = { id: 'owner', field: 'Owner', operator: 'is', value: 'Alice' };
  validateValue({ conditions: [condition], countState: 'unavailable', countReason: 'Permission denied' }, familySchemas.FilterBar.props);
  assert.throws(() => validateValue({ conditions: [condition, condition] }, familySchemas.FilterBar.props));
  assert.throws(() => validateValue({ countState: 'failed' }, familySchemas.FilterBar.props));
  for (const contract of Object.values(familyMethods.FormField.query)) {
    validateValue({}, contract.args);
    assert.throws(() => validateValue({ value: 1 }, contract.args));
    validateValue(false, contract.result);
    assert.throws(() => validateValue(null, contract.result));
  }
});

test('button sources, glyph-only controls, and refused states are explicit', () => {
  for (const [component, props] of [
    ['Button', { color: {} }], ['Button', { color: { semantic: 'info', palette: 'blue' } }],
    ['Button', { iconOnly: true }], ['ToggleGroup', { items: [{ id: 'a', label: 'A', iconOnly: true }] }],
    ['FormField', { label: 'Field', validation: 'invalid' }],
    ['FilterBar', { countState: 'known' }], ['FilterBar', { countState: 'unavailable' }],
  ]) {
    validateValue(props, familySchemas[component].props);
    assert.throws(() => validateFamilyProps(component, props));
  }
  for (const icon of [{ key: 'unknown' }, { key: 'plus-circle', path: '/tmp/icon' }, { key: 'plus-circle', weight: null }]) {
    assert.throws(() => validateValue({ icon, accessibleName: 'Add' }, familySchemas.IconButton.props));
  }
});

test('family factory options, event payloads, and every search method typecheck exactly', () => {
  const dir = mkdtempSync(join(tmpdir(), 'controls-extra-types-'));
  try {
    const sdk = fileURLToPath(new URL('../kit-controls_extra-sdk', import.meta.url));
    const path = join(dir, 'contract.ts');
    writeFileSync(path, `import type { ControlsExtraFactories, ControlsExtraMethodContracts } from ${JSON.stringify(sdk)};
declare const kit: ControlsExtraFactories;
kit.Button('button', {variant:'white', color:{custom:{h:0.1,s:0.7,l:0.2,a:0.5}}});
kit.IconButton('icon', {icon:{key:'plus-circle',weight:'fill'},accessibleName:'Add'});
kit.Toggle('toggle', {pressed:true}, {press(value) { const checked: boolean = value; }});
kit.ToggleGroup('group', {items:[{id:'beta',label:'Beta'}],pressed:['beta']}, {change(value) { const ids: string[] = value.pressed; const changed: string = value.changed; }});
kit.ColorPicker('color', {value:{h:0.1,s:0.7,l:0.2,a:0.5}}, {change(value) { const opacity: number = value.a; }});
kit.ColorSwatch('swatch', {color:{h:0,s:1,l:0.5,a:1}});
kit.FormField('field', {label:'Name',validation:'validating'}, {}, {content:[]});
kit.FilterBar('filter', {countState:'unavailable',countReason:'Refused'}, {remove(id) { const key: string = id; }});
kit.SearchInput('search', {}, {change(value) { const query: string = value; }});
kit.SettingsRow('setting', {label:'Retention',managed:'Policy'}, {}, {control:[]});
kit.TransferList('transfer', {source:[{id:'alpha',label:'Alpha',disabled:true}],targetSelected:['zeta']}, {toggleSource(id) { const key: string = id; }});
type TransferMethods = ControlsExtraMethodContracts['TransferList']['invoke'];
const transferValues: { [K in keyof TransferMethods]: TransferMethods[K]['args'] } = {
set_query:{query:'Alpha'},set_items:{source:[],target:[]},set_selection:{source:['alpha'],target:['zeta']},set_labels:{source:'Available',target:'Assigned'},set_control_size:{size:'sm'},set_disabled:{disabled:true}
};
// @ts-expect-error transfer selections use identities, not positions
kit.TransferList('bad', {sourceSelected:[0]});
// @ts-expect-error only the named control slot is accepted
kit.SettingsRow('bad', {label:'Setting'}, {}, {content:[]});
type Methods = ControlsExtraMethodContracts['SearchInput']['invoke'];
const values: { [K in keyof Methods]: Methods[K]['args'] } = {
set_value:{value:'next'},set_name:{name:'Name'},set_placeholder:{placeholder:'Query'},set_disabled:{disabled:true},set_presentation:{name:null,placeholder:null,size:'lg'}
};
const queried: ControlsExtraMethodContracts['SearchInput']['query']['value']['result'] = 'text';
// @ts-expect-error color source is exclusive
kit.Button('bad', {color:{palette:'blue',semantic:'info'}});
// @ts-expect-error custom icon path is not a builtin descriptor
kit.IconButton('bad', {icon:{key:'plus-circle',path:'/tmp/icon'},accessibleName:'Bad'});
// @ts-expect-error required constructor color cannot be omitted
kit.ColorPicker('missing', {});
// @ts-expect-error toggle reports boolean
kit.Toggle('bad', {}, {press(value:string) {}});
// @ts-expect-error count is not text
kit.FilterBar('bad', {count:'42'});
// @ts-expect-error named method argument, not a positional value
const bad: Methods['set_value']['args'] = 'text';
// @ts-expect-error no serialized Signal handle
kit.SearchInput('bad', {bind:{signal:1}});
`);
    const compiler = fileURLToPath(new URL('../../app-host/node_modules/typescript/bin/tsc', import.meta.url));
    const result = spawnSync(process.execPath, [compiler, '--strict', '--noEmit', path], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stdout + result.stderr);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('transfer methods reject positional IDs, duplicate records and missing pane arguments', () => {
  const schema = familySchemas.TransferList.props;
  assert.throws(() => validateValue({ sourceSelected: [0] }, schema));
  assert.throws(() => validateValue({ source: [{id:'a',label:'A'},{id:'a',label:'Different'}] }, schema));
  for (const [method, args] of [
    ['set_query',{query:4}], ['set_items',{source:[]}], ['set_selection',{source:[1],target:[]}],
    ['set_labels',{source:'A',target:'B',extra:true}], ['set_control_size',{size:'huge'}], ['set_disabled',{disabled:null}],
  ]) assert.throws(() => validateValue(args, familyMethods.TransferList.invoke[method].args));
});
