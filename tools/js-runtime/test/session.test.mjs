import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm, symlink, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { once } from 'node:events';
import { Session, containedFile } from '../session.mjs';
import { validateTree } from '../tree.mjs';

async function fixture(t, source, options = {}) {
  const root = await mkdtemp(resolve(tmpdir(), 'gpui-runtime-'));
  await writeFile(resolve(root, 'app.mts'), source);
  const session = new Session({ root, entry: 'app.mts', trusted: true, ...options });
  const errors = [];
  session.on('error', error => errors.push(error.message));
  const sessions = new Set([session]);
  t.after(async () => {
    // One hook owns the shared cwd: Node skips subsequent after hooks if an
    // earlier hook throws. Windows cannot remove a running child's cwd.
    const stopped = await Promise.allSettled([...sessions].map(session => session.stop()));
    const failures = stopped.filter(result => result.status === 'rejected').map(result => result.reason);
    try { await rm(root, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 }); }
    catch (error) { failures.push(error); }
    if (failures.length) throw new AggregateError(failures, 'Fixture cleanup failed');
    for (const session of sessions) {
      if (session.child?.pid) assert.ok(session.child.exitCode !== null || session.child.signalCode !== null, 'owned worker must be reaped');
    }
  });
  return { session, root, errors, sessions };
}
const event = (emitter, name) => once(emitter, name, { signal: AbortSignal.timeout(5000) }).then(([value]) => value);

test('real TS modules, async events, state, disabled actions and stale revisions', async t => {
  const { session, root } = await fixture(t, `
    import { initial } from './value.mts';
    const count = gpui.state<number>(initial);
    gpui.mount(() => gpui.column('root', [gpui.text('count', count.get()),
      gpui.button('increment', 'Increment', async () => { await Promise.resolve(); count.set(n => n + 3); }),
      gpui.button('disabled', 'Disabled', () => count.set(999), true)]));
  `);
  await writeFile(resolve(root, 'value.mts'), 'export const initial: number = 7;');
  const first = event(session, 'render'); await session.start();
  assert.equal((await first).tree.children[0].text, '7');
  const next = event(session, 'render'); session.event('increment');
  assert.equal((await next).tree.children[0].text, '10');
  assert.equal(session.event('increment', 1), false);
  session.event('disabled');
  await new Promise(r => setTimeout(r, 100));
  assert.equal(session.tree.children[0].text, '10');
  assert.equal(session.tree.children[2].action, undefined);
});

test('reload is a new generation; old async callbacks cannot mutate replacement; cleanup runs', async t => {
  const { session, root, sessions } = await fixture(t, `
    const count = gpui.state(1);
    gpui.onDispose(() => console.log('cleanup-finished'));
    gpui.mount(() => gpui.button('later', String(count.get()), async () => {
      await new Promise(r => setTimeout(r, 500)); count.set(99);
    }));
  `);
  const ready = event(session, 'ready'); await session.start(); await ready;
  session.event('later');
  const cleanup = event(session, 'log'); await session.stop();
  assert.match((await cleanup).message, /cleanup-finished/);
  assert.notEqual(session.child.exitCode, null);
  const replacement = new Session({ root, entry: 'app.mts', trusted: true });
  sessions.add(replacement);
  replacement.on('error', () => {});
  const frame = event(replacement, 'render'); await replacement.start(); await frame;
  assert.equal(replacement.event('later', session.revision, session.generation), false);
  await new Promise(r => setTimeout(r, 600));
  assert.equal(replacement.tree.text, '1');
});

test('undeclared permission and user refusal are rejected, not empty data', async t => {
  const { session } = await fixture(t, `
    const result = gpui.state('waiting');
    gpui.mount(() => gpui.text('result', result.get()));
    try { await gpui.fs.readText('app.mts'); } catch(e) { result.set(e.message); }
    try { await gpui.storage.get('item'); } catch(e) { result.set(result.get() + ';' + e.message); }
  `, { requested: ['storage'] });
  session.on('permission', ({ capability }) => session.decide(capability, false));
  const ready = event(session, 'ready'); await session.start(); await ready;
  assert.match(session.tree.text, /fs.read was not declared/);
  assert.match(session.tree.text, /Permission denied: storage/);
});

test('host-mediated storage and rooted reads work; traversal and symlinks fail', async t => {
  const { session, root } = await fixture(t, '');
  session.options.storageRoot = resolve(root, 'storage');
  await writeFile(resolve(root, 'sample.txt'), 'asymmetric content 23');
  assert.equal(await session.capability('fs.read', { path: 'sample.txt' }), 'asymmetric content 23');
  for (const path of ['../outside', '/etc/passwd', 'nested/../../outside', 'C:\\escape']) {
    await assert.rejects(containedFile(root, path), /Invalid relative path/);
  }
  const outside = await mkdtemp(resolve(tmpdir(), 'gpui-outside-'));
  try {
    await writeFile(resolve(outside, 'target.txt'), 'outside capability root');
    await symlink(outside, resolve(root, 'escape'), process.platform === 'win32' ? 'junction' : 'dir');
    await assert.rejects(containedFile(root, 'escape/target.txt'), /escapes/);
  } finally { await rm(outside, { recursive: true, force: true }); }
  await session.capability('storage', { op: 'set', key: 'counter', value: { value: 31 } });
  assert.deepEqual(await session.capability('storage', { op: 'get', key: 'counter' }), { value: 31 });
  await assert.rejects(session.capability('storage', { op: 'set', key: '../escape', value: 0 }), /Invalid storage key/);
  assert.deepEqual(JSON.parse(await readFile(resolve(root, 'storage/counter.json'))), { value: 31 });
});

test('infinite loop is killed and reaped while an independent worker stays usable', async t => {
  const bad = await fixture(t, 'while (true) {}', { timeoutMs: 300 });
  const good = await fixture(t, "gpui.mount(() => gpui.text('healthy', 'still alive'));");
  const exited = event(bad.session, 'exit');
  const fault = event(bad.session, 'fault');
  const frame = event(good.session, 'render');
  await Promise.all([bad.session.start(), good.session.start()]);
  assert.match((await fault).message, /heartbeat/);
  assert.equal((await exited).signal, 'SIGKILL');
  assert.equal((await frame).tree.text, 'still alive');
  assert.equal(good.session.closed, false);
});

test('unsupported policies and untrusted execution fail closed; bounded process broker executes', async t => {
  assert.throws(() => new Session({ root: '.', entry: 'app.mjs' }), /Untrusted execution unavailable/);
  const { session } = await fixture(t, '');
  await assert.rejects(session.capability('network', { url: 'https://example.com' }), /denied/);
  await assert.rejects(session.capability('process', { command: 'shell', args: [] }), /denied/);
  session.options.executables.echo = process.execPath;
  assert.equal(await session.capability('process', { command: 'echo', args: ['-e', 'process.stdout.write(process.argv[1])', 'hello; not a shell'] }), 'hello; not a shell');
});

test('tree validation rejects duplicate identities, unsupported components, and depth', () => {
  assert.throws(() => validateTree({ kind: 'webview', id: 'view' }), /Unsupported/);
  assert.throws(() => validateTree({ kind: 'row', id: 'root', children: [{ kind: 'text', id: 'root' }] }), /unique/);
  let tree = { kind: 'text', id: 'leaf' };
  for (let i = 0; i < 34; i++) tree = { kind: 'column', id: `level${i}`, children: [tree] };
  assert.throws(() => validateTree(tree), /limit/);
});

test('storage quota serializes racing writes and bounds UTF-8 bytes', async t => {
  const { session, root } = await fixture(t, '');
  session.options.storageRoot = resolve(root, 'storage');
  await assert.rejects(session.capability('storage', { op: 'set', key: 'wide', value: '界'.repeat(22000) }), /value exceeds/);
  const results = await Promise.allSettled(Array.from({ length: 18 }, (_, i) =>
    session.capability('storage', { op: 'set', key: `key${i}`, value: 'a'.repeat(65534) })));
  assert.equal(results.filter(x => x.status === 'fulfilled').length, 16);
  assert.equal(results.filter(x => x.status === 'rejected' && /quota/.test(x.reason.message)).length, 2);
  await session.capability('storage', { op: 'set', key: 'key0', value: 'smaller' });
  assert.equal(await session.capability('storage', { op: 'get', key: 'key0' }), 'smaller');
});

test('broker denies spoofed origins and commands; disposal cancels pending consent and child operation', async t => {
  const { session } = await fixture(t, '', { requested: ['storage'], origins: ['https://allowed.example'], executables: { sleep: process.execPath, relative: 'echo' } });
  for (const url of ['https://allowed.example.evil', 'http://allowed.example', 'https://allowed.example:444', 'https://user@allowed.example'])
    await assert.rejects(session.capability('network', { url }), /denied/);
  for (const command of ['constructor', '__proto__', 'relative', '/bin/sleep'])
    await assert.rejects(session.capability('process', { command, args: [] }), /denied/);
  const pending = assert.rejects(session.permitted('storage'), /Permission denied/);
  const sleeping = assert.rejects(session.capability('process', { command: 'sleep', args: ['-e', 'setTimeout(() => {}, 5000)'] }), /abort/i);
  const start = Date.now();
  await session.stop(); await pending; await sleeping;
  assert.ok(Date.now() - start < 2000, 'cancel should not wait for process timeout');
  assert.equal(session.permissionWaiters.size, 0);
  assert.equal(session.aborts.size, 0);
});

test('Kit event payloads reach the current callback and reject wrong types and old generations', async t => {
  const { session, errors } = await fixture(t, `
    const checked = gpui.state(false);
    gpui.mount(() => gpui.column('root', [gpui.text('value', checked.get()),
      gpui.kit.Checkbox('check', { checked: checked.get() }, { change: value => checked.set(value) })]));
  `);
  const ready = event(session, 'ready'); await session.start(); await ready;
  const action = session.tree.children[1].events.change;
  const next = event(session, 'render'); session.event(action, session.revision, session.generation, true);
  assert.equal((await next).tree.children[0].text, 'true');
  assert.equal(session.event(action, session.revision, session.generation - 1, false), false);
  const refused = event(session, 'error');
  session.event(action, session.revision, session.generation, 'false'); await refused;
  assert.match(errors.at(-1), /expected boolean/);
  assert.equal(session.tree.children[0].text, 'true');
  assert.throws(() => session.event(action, session.revision, session.generation, '界'.repeat(6000)), /payload limit/);
});
