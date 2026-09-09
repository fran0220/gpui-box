import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createKitBindings } from '../kit-bindings.mjs';
import { kitSchemas, kitMethods, generateKitMethodTypes, validateInvocation, validateKitProps, validateKitDescriptor } from '../kit-schema.mjs';

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

test('merged family factories validate required data, relational rules and typed intents', () => {
  const actions = new Map();
  const kit = createKitBindings((id, event, handler) => { const action = `${id}.${event}`; actions.set(action, handler); return action; });
  const adapter = JSON.parse(readFileSync(new URL('../../app-host/src/kit_bindings/datetime/fixture/data.json', import.meta.url), 'utf8'));
  let intent;
  const wizard = kit.Wizard('flow', { steps: [{ id: 'review', title: 'Review' }] }, { navigate(value) { intent = value; } });
  actions.get(wizard.events.navigate)({ kind: 'step', id: 'review' });
  assert.deepEqual(intent, { kind: 'step', id: 'review' });
  assert.throws(() => actions.get(wizard.events.navigate)({ kind: 'finish', id: 'review' }), /exactly one/);
  const date = kit.DateInput('date', { adapter, value: 31 }, { change(value) { intent = value; } });
  actions.get(date.events.change)(11);
  assert.equal(intent, 11);
  assert.equal(date.props.value, 31);
  assert.throws(() => kit.DateInput('date', {}), /required/);
  assert.throws(() => kit.DateInput('date', { adapter, value: 999 }), /unknown day/);
  assert.throws(() => kit.FormField('field', { label: 'Name', validation: 'invalid' }), /requires reason/);
  assert.throws(() => kit.Container('box', { width: 'custom' }), /custom width required/);
  assert.throws(() => kit.NavStack('history', { entries: [{ id: 'one' }], cursor: 1, label: 'History' }), /invalid navigation history/);
  assert.equal(kit.AspectRatio('ratio', { ratio: 1.75 }, {}, { content: [kit.SearchInput('search')] }).slots.content[0].component, 'SearchInput');
  assert.throws(() => validateInvocation('DateInput', 'calendar', {}, 'query'), /Unsupported/);
  assert.throws(() => validateInvocation('Calendar', 'adapter', {}, 'query'), /Unsupported/);
  assert.equal(validateInvocation('DateInput', 'calendar_snapshot', {}, 'query'), kitMethods.DateInput.query.calendar_snapshot);
  assert.equal(Object.keys(kitSchemas).length, 177);
  assert.equal(Object.keys(kit).length, 177);
});

test('disabled controls register no callable handlers', () => {
  const kit = createKitBindings(() => assert.fail('disabled registrar called'));
  assert.deepEqual(kit.Switch('permission', { disabled: true }, { change() {} }).events, {});
});

test('state binding seeds controlled native props and registers only the replacement callback', () => {
  for (const [component, property, event, initial, next] of [
    ['Checkbox', 'checked', 'change', false, true], ['Switch', 'on', 'change', true, false],
    ['Slider', 'value', 'change', 0.23, 0.47], ['SegmentedControl', 'selected', 'select', 'alpha', 'beta'],
    ['TextInput', 'text', 'change', 'old', 'new'], ['Select', 'selected', 'change', null, 'beta'],
  ]) {
    const actions = new Map();
    const kit = createKitBindings((id, name, handler) => { const key = `${id}:${name}`; assert.equal(actions.has(key), false); actions.set(key, handler); return key; });
    let current = initial;
    const state = { get: () => current, set: value => { current = value; } };
    const node = kit.bind(component, 'bound', state, {}, { [event]() { assert.fail('replaced handler called'); } });
    assert.equal(node.props[property], initial);
    validateKitDescriptor(JSON.parse(JSON.stringify(node)));
    assert.equal(actions.size, 1);
    actions.get(node.events[event])(next);
    assert.equal(current, next);
    assert.equal(node.props[property], initial, 'events do not mutate the previous frame');
    actions.clear();
    assert.equal(kit.bind(component, 'bound', state).props[property], next);
    actions.clear();
    assert.deepEqual(kit.bind(component, 'bound', state, { disabled: true }).events, {});
    assert.equal(actions.size, 0);
  }
});

test('radio binding compares JSON values structurally and owns a copy of its choice', () => {
  let current = { id: 'alpha', details: [1, 2] };
  const state = { get: () => current, set: value => { current = value; } };
  const actions = new Map();
  const kit = createKitBindings((id, name, handler) => { actions.set(id, handler); return `${id}:${name}`; });
  assert.equal(kit.bind_value('Radio', 'same', state, { details: [1, 2], id: 'alpha' }).props.selected, true);
  const choice = { id: 'beta', details: [2, 1] };
  assert.equal(kit.bind_value('Radio', 'other', state, choice).props.selected, false);
  choice.id = 'mutated';
  actions.get('other')(null);
  assert.deepEqual(current, { id: 'beta', details: [2, 1] });
  assert.throws(() => kit.bind_value('Radio', 'function', state, { callback() {} }));
  assert.throws(() => kit.bind('Checkbox', 'wrong', { get: () => 'yes', set() {} }), /value type/);
  assert.throws(() => kit.bind('Dialog', 'wrong', state), /Unsupported bound/);
});

test('predicate registrars receive typed data callbacks and descriptors retain only opaque refs', async () => {
  const callbacks = new Map();
  const kit = createKitBindings(() => 'event', (id, component, name, callback) => {
    const ref = `${component}:${id}:${name}`; callbacks.set(ref, callback); return ref;
  });
  const payload = { id: 'beta', source: 'list', label: 'Beta', kind: 'row', anchor: 'alpha', position: 'before', velocity: { x: -12, y: 7 } };
  const descriptor = kit.List('list', { reorderable: true }, {}, {}, { accepts: async intent => intent.id === 'beta' });
  validateKitDescriptor(JSON.parse(JSON.stringify(descriptor)));
  assert.deepEqual(descriptor.predicates, { accepts: 'List:list:accepts' });
  assert.equal(await callbacks.get(descriptor.predicates.accepts)(payload), true);
  assert.throws(() => callbacks.get(descriptor.predicates.accepts)({ ...payload, owner: 'secret-token' }), /unknown field/);
  const invalid = kit.Tabs('tabs', {}, {}, {}, { accepts: () => 'yes' });
  assert.throws(() => callbacks.get(invalid.predicates.accepts)(payload), /boolean/);
  const invalidAsync = kit.Tabs('async-tabs', {}, {}, {}, { accepts: async () => 1 });
  await assert.rejects(callbacks.get(invalidAsync.predicates.accepts)(payload), /boolean/);
  const count = callbacks.size;
  assert.equal(kit.List('disabled', { disabled: true }, {}, {}, { accepts: () => true }).predicates, undefined);
  assert.equal(callbacks.size, count);
  assert.throws(() => validateKitDescriptor({ ...descriptor, predicates: { execute: 'ref' } }), /unknown field/);
  assert.throws(() => validateKitDescriptor({ ...descriptor, predicates: { accepts: 123 } }), /string/);
  assert.throws(() => validateKitDescriptor({ ...descriptor, predicates: null }), /object/);
  assert.throws(() => validateKitDescriptor({ ...descriptor, props: { disabled: true } }), /Disabled/);
});

test('unsupported or accessor predicates fail before either registrar runs', () => {
  let registrations = 0;
  const kit = createKitBindings(() => { registrations++; return 'event'; }, () => { registrations++; return 'predicate'; });
  assert.throws(() => kit.List('list', {}, { select() {} }, {}, { get accepts() { assert.fail('accessor executed'); } }), /invalid predicates/);
  assert.throws(() => kit.Checkbox('check', {}, { change() {} }, {}, { accepts() { return true; } }), /invalid predicates/);
  assert.equal(registrations, 0);
  assert.throws(() => createKitBindings(() => 'event').List('list', {}, {}, {}, { accepts() { return true; } }), /unavailable/);
});

test('specific schemas reject unknown options, invalid ranges, duplicate business identities', () => {
  const kit = createKitBindings(() => 'action');
  assert.throws(() => kit.Radio('r', { on: true }), /unknown field/);
  assert.throws(() => kit.Checkbox('', {}), /string length/);
  assert.throws(() => kit.Checkbox('bad-unicode', { label: '\ud800' }), /Unicode/);
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
  const methods = JSON.parse(readFileSync(new URL('../../app-host/src/kit_bindings/methods.json', import.meta.url), 'utf8'));
  assert.deepEqual(methods, kitMethods);
  const sdk = readFileSync(new URL('../kit-sdk.d.ts', import.meta.url), 'utf8');
  assert.equal(sdk.slice(sdk.indexOf('// Generated from kitMethods')), generateKitMethodTypes());
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

test('lazy List slots require row identities and acyclic semantic parents', () => {
  const kit = createKitBindings((id, event) => `${id}.${event}`);
  const rows = [{ id: 'parent', label: 'Parent' }, { id: 'child', label: 'Child', within: 'parent' }];
  assert.equal(validateKitDescriptor(kit.List('list', { rows }, {}, { child: [kit.Checkbox('nested')] })).component, 'List');
  assert.throws(() => kit.List('list', { rows }, {}, { missing: [] }), /unknown field/);
  assert.throws(() => kit.List('list', { rows: [{ id: 'self', label: 'Self', within: 'self' }] }), /invalid row parent/);
  assert.throws(() => kit.List('list', { rows: [{ id: 'a', label: 'A', within: 'b' }, { id: 'b', label: 'B', within: 'a' }] }), /invalid row parent/);
});

test('native method validation is mode-specific and rejects unknown or executable arguments', () => {
  assert.equal(validateInvocation('TextInput', 'set_max_length', { max_length: null }, 'invoke').result.enum[0], null);
  assert.throws(() => validateInvocation('TextInput', 'value', {}, 'invoke'), /Unsupported/);
  assert.throws(() => validateInvocation('Select', 'set_value', { value: 'x' }, 'invoke'), /Unsupported/);
  assert.throws(() => validateInvocation('TextInput', 'set_value', { value: 'x', source: '/etc/passwd' }, 'invoke'), /unknown field/);
  assert.throws(() => validateInvocation('TextInput', 'set_value', { get value() { assert.fail('getter executed'); } }, 'invoke'), /accessor/);
});

test('SDK typechecks component options and typed callbacks, rejecting unknown members', () => {
  const dir = mkdtempSync(join(tmpdir(), 'gpui-kit-types-'));
  try {
    const sdk = fileURLToPath(new URL('../kit-sdk', import.meta.url));
    const path = join(dir, 'contract.ts');
    writeFileSync(path, `import type { KitAPI, KitInvoke, KitQuery } from ${JSON.stringify(sdk)};
declare const kit: KitAPI;
declare const invoke: KitInvoke;
declare const query: KitQuery;
const input = kit.TextInput('typed-input');
const inputKind: 'TextInput' = input.component;
const response: Promise<string> = query(input, 'value');
const changed: Promise<null> = invoke(input, 'set_value', {value:'next'});
const selection: Promise<string|null> = query(kit.Select('typed-select'), 'selected_id');
const opened: Promise<null> = invoke(kit.Dialog('modal'), 'open');
const isOpen: Promise<boolean> = query(kit.Popover('tip'), 'is_open');
const searchValue: Promise<string> = query(kit.SearchInput('search'), 'value');
const ratio: Promise<number> = query(kit.AspectRatio('ratio', {ratio:1.75}), 'ratio');
kit.Wizard('wizard', {}, {navigate(intent) { if (intent.kind === 'step') { const id: string = intent.id; } }});
kit.TransferList('transfer', {}, {toggleSource(id) { const selected: string = id; }});
// @ts-expect-error required caller adapter cannot be omitted
kit.DateInput('date', {});
// @ts-expect-error native refs are not data snapshot aliases
query({id:'date', component:'DateInput'}, 'calendar');
// @ts-expect-error wrong family cannot widen native method target
query(kit.SearchInput('search'), 'ratio');
declare const boolState: { get(): boolean; set(value: boolean): void };
declare const textState: { get(): string; set(value: string): void };
const boundKind: 'Checkbox' = kit.bind('Checkbox', 'bound', boolState, {label:'Bound'}).component;
kit.bind_value('Radio', 'choice', textState, 'beta', {label:'Beta'});
// @ts-expect-error component fixes the binding value type
kit.bind('Checkbox', 'bad-state', textState);
// @ts-expect-error arbitrary native components do not acquire fake bind support
kit.bind('Dialog', 'bad-component', boolState);
// @ts-expect-error radio choice must match caller state value type
kit.bind_value('Radio', 'bad-choice', textState, 123);
// @ts-expect-error wrong component's method cannot widen target inference
query(input, 'selected_id');
// @ts-expect-error query result is not arbitrary
const wrongResult: Promise<number> = query(input, 'value');
// @ts-expect-error a required typed argument cannot be omitted
invoke(input, 'set_value');
// @ts-expect-error unknown argument is not silently ignored
invoke(input, 'set_value', {value:'next',path:'/etc/passwd'});
kit.Checkbox('check', {checked:null}, {change(value) { const checked: boolean = value; }});
kit.Slider('range', {min:-10,max:20,value:-3,high:17}, {rangeChange(value) { const high: number = value.high; }});
kit.Select('select', {selected:null}, {change(value) { const selected: string|null = value; }});
kit.Select('grouped', {options:[{id:'a',label:'A',description:'Detail',group:'Group'}]});
invoke(kit.Select('grouped'), 'set_options', {options:[{id:'b',label:'B',description:'Other',group:'Other group'}]});
kit.SplitPane('panes', {ratio:0.3}, {collapse(side) { const value: 'start'|'end' = side; }}, {start:[kit.Radio('nested')]});
kit.TextInput('secure', {}, {clipboardDenied(reason) { const refusal: 'missingOwner'|'denied' = reason; }});
kit.List('list', {rows:[{id:'a',label:'A'}]}, {select(id) { const selected:string=id; }}, {a:[kit.Radio('row')]});
kit.List('predicate-list', {reorderable:true}, {}, {}, {accepts(intent) { const velocity:number=intent.velocity.x; return intent.position === 'before'; }});
// @ts-expect-error predicate result must be literal boolean or Promise<boolean>
kit.Tabs('predicate-tabs', {}, {}, {}, {accepts() { return 'yes'; }});
// @ts-expect-error ScrollArea has only content slot
kit.ScrollArea('scroll', {}, {}, {start:[]});
// @ts-expect-error radio does not have the switch option
kit.Radio('radio', {on:true});
// @ts-expect-error unsupported catalog names are not callable
kit.NumberInput('number');
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
