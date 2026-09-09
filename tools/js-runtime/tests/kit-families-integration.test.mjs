import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { Session } from '../session.mjs';
import { createKitBindings } from '../kit-bindings.mjs';
import { kitSchemas, kitMethods, validateInvocation, validateValue, generateKitMethodTypes } from '../kit-schema.mjs';
import { bindings } from '../bindings.mjs';

const families = ['display', 'charts', 'agent', 'game-effects', 'canvas', 'overlay', 'content', 'media', 'data', 'structured'];

test('central contracts preserve every frozen family member, method and pending boundary', async () => {
  const names = new Set();
  for (const family of families) {
    const source = await import(`../kit-${family}-schema.mjs`);
    for (const [name, schema] of Object.entries(source.familySchemas)) {
      assert.equal(names.has(name), false, name);
      names.add(name);
      assert.deepEqual(kitSchemas[name], schema, name);
      assert.deepEqual(kitMethods[name], source.familyMethods[name], name);
      assert.equal(bindings[name].nativeIntegration, 'pending-central-hooks');
    }
  }
  assert.equal(names.size, 126);
  assert.equal(Object.keys(kitSchemas).length, 177);
  assert.equal(bindings.CinematicEffect.status, 'fallback-only');
  assert.equal(bindings.Drawer.referenceIntegration, 'pending-native-registry');
  assert.ok(bindings.AgentRoster.props.includes('agents'));
  assert.ok(bindings.AgentRoster.props.includes('run'));
  assert.equal(bindings.TextInput.nativeMethodSources.focus_handle, 'gpui::window::Focusable::focus_handle');
  const sdk = readFileSync(new URL('../kit-sdk.d.ts', import.meta.url), 'utf8');
  assert.equal(sdk.slice(sdk.indexOf('// Generated from kitMethods')), generateKitMethodTypes());
});

test('real central factories accept all display/chart/agent/game/content/media fixtures', async () => {
  const kit = createKitBindings((id, event) => `${id}.${event}`);
  const fixtures = [];
  for (const family of ['display', 'charts']) {
    const { props } = await import(`../../app-host/src/kit_bindings/${family}/fixture/props.mjs`);
    fixtures.push(...Object.entries(props).map(([component, props]) => ({ component, props })));
  }
  for (const family of ['agent', 'game_effects']) {
    const { cases } = await import(`../../app-host/src/kit_bindings/${family}/fixture/cases.mjs`);
    fixtures.push(...cases);
  }
  for (const family of ['content', 'media']) fixtures.push(...JSON.parse(readFileSync(new URL(`../../app-host/src/kit_bindings/${family}/fixture/nodes.json`, import.meta.url), 'utf8')));
  for (const fixture of fixtures) {
    const handlers = Object.fromEntries(Object.keys(fixture.events ?? {}).map(event => [event, () => {}]));
    const node = kit[fixture.component](fixture.id ?? fixture.component, fixture.props, handlers, fixture.slots ?? {});
    assert.deepEqual(node.props, fixture.props, fixture.component);
    assert.notEqual(node.props, fixture.props);
    assert.throws(() => kit[fixture.component]('invalid', { ...fixture.props, unknownHostOption: true }), undefined, fixture.component);
  }
  assert.equal(new Set(fixtures.map(f => f.component)).size, 96);
});

test('central factories route relational, resource, recursive and typed-slot checks before events', () => {
  const kit = createKitBindings(() => assert.fail('invalid props registered an event'));
  const invalid = [
    ['Rating', { maximum: 3, value: 4 }],
    ['LineChart', { label: 'Trend', state: { kind: 'ready' } }],
    ['AgentAvatar', {}],
    ['AbilityBar', { abilities: [{ id: 'west', label: 'West', state: { kind: 'ready' }, charges: { current: 4, maximum: 3 } }] }],
    ['NodeGraph', { zoom_range: { min: 3, max: 2 } }],
    ['Menu', { items: [{ kind: 'command', id: 'same', label: 'One' }, { kind: 'command', id: 'same', label: 'Two' }] }],
    ['CodeView', { text: 'one', lines: [{ number: 7, text: 'two' }] }],
    ['VideoPlayer', { posterResource: { key: '../secret' } }],
    ['Tree', { nodes: [{ id: 'same', label: 'Parent', children: [{ id: 'same', label: 'Child' }] }] }],
    ['JsonView', { value: { kind: 'number', text: '01' } }],
  ];
  for (const [name, props] of invalid) assert.throws(() => kit[name]('invalid', props), undefined, name);
  assert.throws(() => kit.AgentDocument('doc', { blocks: [{ id: 'chart', kind: 'chart' }] }), /typed block needs its slot/);
  const chart = kit.LineChart('plot', { label: 'Trend', state: { kind: 'empty' } });
  assert.equal(kit.AgentDocument('doc', { blocks: [{ id: 'chart', kind: 'chart' }] }, {}, { chart: [chart] }).slots.chart[0].component, 'LineChart');
  assert.equal(kit.JsonView('json', { value: { kind: 'array', items: [{ kind: 'number', text: '-1.25e2' }] } }).props.value.items[0].text, '-1.25e2');
  assert.equal(kit.VideoPlayer('video', { posterResource: { key: 'poster' } }).props.posterResource.key, 'poster');
});

test('merged typed events preserve caller state and validate before calling handlers', () => {
  const actions = new Map();
  const kit = createKitBindings((id, event, handler) => { const key = `${id}.${event}`; actions.set(key, handler); return key; });
  const cases = [
    ['Rating', { value: 2 }, 'change', 4, 'four'],
    ['ChartLegend', { series: [] }, 'toggle', { id: 'west', hidden: true }, { id: 'west', hidden: 'yes' }],
    ['FeedbackRating', {}, 'vote', 'down', 'unknown'],
    ['NodeGraph', {}, 'node_click', 'west', 4],
    ['Tree', { nodes: [{ id: 'west', label: 'West' }] }, 'toggle', { id: 'west', expanded: true }, { id: 'west' }],
  ];
  for (const [name, props, event, valid, invalid] of cases) {
    const received = [];
    const node = kit[name](name, props, { [event]: value => received.push(value) });
    actions.get(node.events[event])(valid);
    assert.throws(() => actions.get(node.events[event])(invalid));
    assert.deepEqual(received, [valid]);
    assert.deepEqual(node.props, props);
    assert.deepEqual(kit[name](`${name}.disabled`, { ...props, disabled: true }, { [event]: () => assert.fail('disabled') }).events, {});
  }
});

test('central methods keep mode, argument and result validation including reference markers', () => {
  const form = validateInvocation('SchemaForm', 'set_files', { path: 'uploads', files: ['west', 'east'] }, 'invoke');
  validateValue(true, form.result);
  assert.throws(() => validateValue(null, form.result));
  assert.throws(() => validateInvocation('SchemaForm', 'set_files', { path: 'uploads', files: [7] }, 'invoke'));
  assert.throws(() => validateInvocation('SchemaForm', 'set_files', { path: 'uploads', files: [] }, 'query'));
  validateInvocation('ContextMenu', 'open_at', { position: { x: 37, y: 91 } }, 'invoke');
  assert.throws(() => validateInvocation('ContextMenu', 'open_at', { x: 37, y: 91 }, 'invoke'));
  for (const name of ['TextInput', 'Drawer']) {
    const method = validateInvocation(name, 'focus_handle', {}, 'query');
    validateValue({ $nativeRef: 'issued-focus', type: 'FocusHandle' }, method.result);
    assert.throws(() => validateValue({ $nativeRef: 'issued-focus', type: 'TextInput' }, method.result));
    assert.throws(() => validateValue({ $nativeRef: 'issued-focus', type: 'FocusHandle', owner: 'forged' }, method.result));
  }
});

test('merged SDK strict positive and negative contracts cover all five groups and branded refs', () => {
  const dir = mkdtempSync(join(tmpdir(), 'gpui-families-types-'));
  try {
    const sdk = fileURLToPath(new URL('../kit-sdk', import.meta.url));
    const refs = fileURLToPath(new URL('../reference-sdk', import.meta.url));
    const path = join(dir, 'contract.ts');
    writeFileSync(path, `import type { KitAPI, KitInvoke, KitQuery } from ${JSON.stringify(sdk)};
import type { NativeRef } from ${JSON.stringify(refs)};
declare const kit: KitAPI; declare const invoke: KitInvoke; declare const query: KitQuery;
kit.Rating('rating', {value:2}, {change(v) {const value: number|null=v;}});
kit.LineChart('chart', {label:'Trend',state:{kind:'empty'}});
kit.FeedbackRating('feedback', {}, {vote(v) {const vote: 'up'|'down'=v;}});
kit.AbilityBar('abilities', {abilities:[]});
kit.NodeGraph('graph', {zoom_range:{min:0.2,max:3}});
const focus: Promise<NativeRef<'FocusHandle'>> = query(kit.Drawer('drawer'), 'focus_handle');
const inputFocus: Promise<NativeRef<'FocusHandle'>> = query(kit.TextInput('input'), 'focus_handle');
kit.Markdown('markdown', {source:'Caller document'});
kit.VideoPlayer('video', {posterResource:{key:'poster'}});
kit.Tree('tree', {nodes:[{id:'west',label:'West'}]});
const changed: Promise<boolean> = invoke(kit.SchemaForm('form', {fields:[]}), 'set_files', {path:'uploads',files:['west']});
// @ts-expect-error caller data required
kit.AgentAvatar('agent', {});
// @ts-expect-error chart data state requires payload
kit.LineChart('chart', {label:'Trend',state:{kind:'ready'}});
// @ts-expect-error wrong event payload type
kit.FeedbackRating('feedback', {}, {vote(v: number) {}});
// @ts-expect-error no raw resource URL authority
kit.VideoPlayer('video', {posterResource:{url:'https://example.invalid'}});
// @ts-expect-error component cannot widen to another family's method
query(kit.NodeGraph('graph'), 'focus_handle');
// @ts-expect-error data result is boolean, not null
const wrong: Promise<null> = invoke(kit.SchemaForm('form', {fields:[]}), 'set_files', {path:'uploads',files:[]});
// @ts-expect-error native references are not structural caller objects
const forged: NativeRef<'FocusHandle'> = {$nativeRef:'forged',type:'FocusHandle'};
// @ts-expect-error Tree live predicate fifth argument remains deferred
kit.Tree('tree', {nodes:[]}, {}, {}, {canDrop: () => true});
`);
    const compiler = fileURLToPath(new URL('../../app-host/node_modules/typescript/bin/tsc', import.meta.url));
    const result = spawnSync(process.execPath, [compiler, '--strict', '--noEmit', '--target', 'ES2022', '--moduleResolution', 'node', path], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stdout + result.stderr);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('actual isolated worker routes new family methods through the host transport, preserving refusals and stale revision guards', async t => {
  const root = mkdtempSync(join(tmpdir(), 'gpui-family-session-'));
  writeFileSync(join(root, 'main.mjs'), `
const form = gpui.kit.SchemaForm('form', {fields:[]});
const result = gpui.state('ready');
gpui.mount(() => gpui.column('root', [form, gpui.text('result', result.get())]));
gpui.command('set', async () => {try {result.set(JSON.stringify(await gpui.invoke(form, 'set_files', {path:'uploads', files:['west','east']})));} catch(e) {result.set(e.message);}});
gpui.command('wrong', async () => {try {await gpui.query({id:'form',component:'JsonView'}, 'disclosed_paths');} catch(e) {result.set(e.message);}});
gpui.command('replace', () => result.set('replacement'));
`);
  const session = new Session({ root, entry: 'main.mjs', sandbox: process.platform === 'linux' ? 'linux' : undefined, trusted: process.platform !== 'linux' });
  session.on('error', () => {});
  t.after(async () => { await session.stop(); rmSync(root, { recursive: true, force: true }); });
  const ready = once(session, 'ready'); await session.start(); await ready;
  const rendered = async pattern => {
    while (!pattern.test(session.tree.children[1].text)) await once(session, 'render', { signal: AbortSignal.timeout(4000) });
  };
  const request = async () => {
    const next = once(session, 'invoke'); session.command('set');
    const [call] = await next;
    assert.deepEqual(call.target, { id: 'form', component: 'SchemaForm' });
    assert.equal(call.mode, 'invoke');
    assert.equal(call.method, 'set_files');
    assert.deepEqual(call.args, { path: 'uploads', files: ['west', 'east'] });
    return call;
  };
  // This is a transport stand-in, not evidence of Rust family registration.
  const good = await request();
  assert.equal(session.finishNative(good.id, good.revision, true), true);
  await rendered(/^true$/);
  const bad = await request();
  session.finishNative(bad.id, bad.revision, null, 'Native family refused unknown field path');
  await rendered(/^Native family refused unknown field path$/);
  session.command('wrong');
  await rendered(/not mounted with that component identity/);
  const old = await request();
  session.command('replace');
  await rendered(/cancelled by render revision change/);
  assert.equal(session.finishNative(old.id, old.revision, false), false);
  assert.equal(session.nativeRequests.size, 0);
});
