import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createKitBindings } from '../kit-bindings.mjs';
import { kitSchemas, validateKitProps, validateKitDescriptor } from '../kit-schema.mjs';

test('caller state is copied, typed events report intent without mutating descriptor', () => {
  const actions = new Map();
  const kit = createKitBindings((id, event, handler) => { const action = `${id}.${event}`; actions.set(action, handler); return action; });
  let next;
  const props = { checked: false, label: 'Allow' };
  const node = kit.Checkbox('permission.allow', props, { change: value => { next = value; } });
  props.checked = true;
  assert.equal(node.props.checked, false);
  actions.get(node.events.change)(true);
  assert.equal(next, true);
  assert.equal(node.props.checked, false);
  assert.throws(() => actions.get(node.events.change)('yes'), /expected boolean/);
});

test('disabled controls register no callable handlers', () => {
  const kit = createKitBindings(() => assert.fail('disabled registrar called'));
  assert.deepEqual(kit.Switch('permission', { disabled: true }, { change() {} }).events, {});
});

test('specific schemas reject unknown options, invalid ranges, duplicate business identities', () => {
  const kit = createKitBindings(() => 'action');
  assert.throws(() => kit.Radio('r', { on: true }), /unknown field/);
  assert.throws(() => kit.Checkbox('', {}), /string length/);
  assert.throws(() => kit.Slider('s', { min: -10, max: 20, value: -3, high: -4 }), /invalid range/);
  assert.throws(() => kit.Slider('s', { step: 0 }), /invalid number/);
  assert.throws(() => kit.Slider('s', { value: NaN }), /invalid number/);
  assert.throws(() => kit.Select('s', { options: [{ id: 'a', label: 'A' }, { id: 'a', label: 'B' }] }), /duplicate identity/);
  assert.equal(kit.Slider('s', { min: -10, max: 20, value: -3, high: 17 }).props.high, 17);
  assert.throws(() => validateKitProps('NotBound', 'x', {}), /Unsupported/);
  assert.equal(kit.NotBound, undefined);
});

test('reject getters and unknown events before registering any action', () => {
  const kit = createKitBindings(() => assert.fail('registrar called'));
  assert.throws(() => kit.TextInput('input', { get text() { assert.fail('getter evaluated'); } }), /accessor/);
  assert.throws(() => kit.Checkbox('c', {}, { change() {}, click() {} }), /invalid event/);
});

test('native embedded contracts match executable JS schemas', () => {
  const native = JSON.parse(readFileSync(new URL('../../app-host/src/kit_bindings/schemas.json', import.meta.url), 'utf8'));
  assert.deepEqual(native, kitSchemas);
});

test('wire descriptors reject unbound slots and events', () => {
  const kit = createKitBindings((id, event) => `${id}.${event}`);
  const node = kit.Select('s', { selected: null }, { change() {} });
  assert.equal(validateKitDescriptor(node), node);
  assert.throws(() => validateKitDescriptor({ ...node, slots: { content: [] } }), /unknown field/);
  assert.throws(() => validateKitDescriptor({ ...node, events: { launch: 'action' } }), /unknown field/);
  assert.throws(() => validateKitDescriptor({ ...node, props: { disabled: true } }), /Disabled/);
});

test('named slots are copied and accordion slot names follow section identity', () => {
  const kit = createKitBindings((id, event) => `${id}.${event}`);
  const child = kit.Checkbox('nested', { checked: false });
  const node = kit.Accordion('accordion', { sections: [{ id: 'billing', title: 'Billing' }] }, {}, { billing: [child] });
  child.props.checked = true;
  assert.equal(node.slots.billing[0].props.checked, false);
  assert.equal(validateKitDescriptor(node), node);
  assert.throws(() => kit.Accordion('a', { sections: [{ id: 'billing', title: 'Billing' }] }, {}, { profile: [] }), /unknown field/);
  assert.throws(() => kit.ScrollArea('s', {}, {}, { start: [] }), /unknown field/);
  assert.throws(() => kit.SplitPane('s', { ratio: 1.01 }), /invalid number/);
  assert.throws(() => kit.Pagination('p', { page: 0 }), /invalid number/);
});

test('SDK typechecks component options and typed callbacks, rejecting unknown members', () => {
  const dir = mkdtempSync(join(tmpdir(), 'gpui-kit-types-'));
  try {
    const sdk = fileURLToPath(new URL('../kit-sdk', import.meta.url));
    const path = join(dir, 'contract.ts');
    writeFileSync(path, `import type { KitAPI } from ${JSON.stringify(sdk)};
declare const kit: KitAPI;
kit.Checkbox('check', {checked:null}, {change(value) { const checked: boolean = value; }});
kit.Slider('range', {min:-10,max:20,value:-3,high:17}, {rangeChange(value) { const high: number = value.high; }});
kit.Select('select', {selected:null}, {change(value) { const selected: string|null = value; }});
kit.SplitPane('panes', {ratio:0.3}, {collapse(side) { const value: 'start'|'end' = side; }}, {start:[kit.Radio('nested')]});
// @ts-expect-error ScrollArea has only content slot
kit.ScrollArea('scroll', {}, {}, {start:[]});
// @ts-expect-error radio does not have the switch option
kit.Radio('radio', {on:true});
// @ts-expect-error unsupported catalog names are not callable
kit.NodeGraph('graph');
// @ts-expect-error numeric text is not accepted
kit.TextInput('input', {text:4});
// @ts-expect-error change is boolean rather than arbitrary event data
kit.Checkbox('bad', {}, {change(value: string) {}});
`);
    const compiler = fileURLToPath(new URL('../../app-host/node_modules/typescript/bin/tsc', import.meta.url));
    const result = spawnSync(process.execPath, [compiler, '--strict', '--noEmit', '--skipLibCheck', path], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stdout + result.stderr);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
