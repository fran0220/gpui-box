import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, readdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import { PluginPlatform, validateBundle, validateManifest, safePath } from '../platform.mjs';

const manifest = (id = 'example', version = '1.0.0') => ({ schema: 1, id, version, entry: 'main.mjs', permissions: [], dependencies: {}, contributes: { commands: [{ id: 'add', title: 'Add' }], keymaps: [{ key: 'ctrl-j', command: 'add' }], panels: [{ id: 'panel', title: 'Example panel' }] } });
const source = `const n = gpui.state(5); gpui.command('add', () => n.set(x => x + 2)); gpui.mount(() => gpui.text('panel', n.get()));`;
function bundle(m = manifest(), code = source) {
  const data = { manifest: m, files: { 'main.mjs': code } };
  return { ...data, sha256: createHash('sha256').update(JSON.stringify(data)).digest('hex') };
}
async function platform(t) {
  const root = await mkdtemp(resolve(tmpdir(), 'gpui-plugins-'));
  const p = new PluginPlatform(root);
  t.after(async () => { await p.close(); await rm(root, { recursive: true, force: true }); });
  return p;
}

test('install is inert, trust explicit, commands/keymaps route, disable removes process/contributions', async t => {
  const p = await platform(t);
  await p.install(bundle());
  assert.equal(p.active.size, 0);
  assert.equal((await p.discover())[0].enabled, false);
  await assert.rejects(p.enable('example'), /explicit code trust/);
  const session = await p.enable('example', { trusted: true });
  assert.equal(session.tree.text, '5');
  const next = once(session, 'render'); assert.equal(p.key('ctrl-j'), true);
  assert.equal((await next)[0].tree.text, '7');
  assert.equal(p.command('example', 'undeclared'), false);
  await p.disable('example');
  assert.equal(p.active.size, 0);
  assert.equal(p.key('ctrl-j'), false);
  assert.notEqual(session.child.exitCode, null);
});

test('failed upgrade preserves live previous version; successful upgrade and explicit rollback', async t => {
  const p = await platform(t);
  await p.install(bundle());
  const old = await p.enable('example', { trusted: true });
  await p.install(bundle(manifest('example', '2.0.0'), 'throw new Error("upgrade exploded");'));
  await assert.rejects(p.enable('example', { version: '2.0.0', trusted: true }), /crashed/);
  assert.equal(p.active.get('example').session, old);
  assert.equal(old.closed, false);
  assert.equal((await p.registry()).example.current, '1.0.0');
  await p.install(bundle(manifest('example', '3.0.0'), source.replace('state(5)', 'state(17)')));
  const upgraded = await p.enable('example', { version: '3.0.0', trusted: true });
  assert.equal(upgraded.tree.text, '17');
  assert.equal(old.closed, true);
  const reverted = await p.rollback('example', { trusted: true });
  assert.equal(reverted.tree.text, '5');
  assert.equal(upgraded.closed, true);
  assert.equal((await p.registry()).example.current, '1.0.0');
});

test('exact dependency versions, keymap conflicts, and dependent teardown', async t => {
  const p = await platform(t);
  const base = manifest('base'); base.contributes.keymaps = [];
  const dependent = manifest('dependent'); dependent.dependencies = { base: '1.0.0' };
  await p.install(bundle(base)); await p.install(bundle(dependent));
  await assert.rejects(p.enable('dependent', { trusted: true }), /Enable dependency/);
  await p.enable('base', { trusted: true });
  const child = await p.enable('dependent', { trusted: true });
  await p.install(bundle(manifest('conflict')));
  await assert.rejects(p.enable('conflict', { trusted: true }), /Keymap conflict/);
  await p.disable('base');
  assert.equal(p.active.size, 0); assert.equal(child.closed, true);
});

test('plugin crash removes only its own contribution; peer survives', async t => {
  const p = await platform(t);
  const bad = manifest('broken'); bad.contributes.keymaps = [];
  await p.install(bundle(bad, `${source}; gpui.command('crash', () => process.exit(4));`));
  await p.install(bundle());
  const broken = await p.enable('broken', { trusted: true });
  const healthy = await p.enable('example', { trusted: true });
  const exit = once(broken, 'exit'); broken.command('crash'); await exit;
  assert.equal(p.active.has('broken'), false);
  assert.equal(p.active.get('example').session, healthy);
  assert.equal(healthy.closed, false);
});

test('malicious manifest and bundle paths are rejected before any install write', async t => {
  const p = await platform(t);
  for (const path of ['../escape', '/absolute', 'C:\\escape', 'a/../../escape', 'a//b', '.receipt.json', 'a/./b']) assert.throws(() => safePath(path), /Invalid bundle path/);
  for (const patch of [{ id: '../escape' }, { version: '1.0.0/../../escape' }, { permissions: ['everything'] }, { extra: true }, { dependencies: { other: '^1.0.0' } }]) {
    assert.throws(() => validateManifest({ ...manifest(), ...patch }));
  }
  const bad = bundle(); bad.files['../escape'] = 'bad';
  await assert.rejects(p.install(bad), /Invalid bundle path/);
  assert.deepEqual(await readdir(p.root), []);
  const corrupted = bundle(); corrupted.files['main.mjs'] += 'tampered';
  assert.throws(() => validateBundle(corrupted), /integrity/);
  await p.install(bundle(manifest('constructor')));
  assert.equal((await p.discover())[0].id, 'constructor');
});

test('immutable versions cannot overwrite known-good code', async t => {
  const p = await platform(t);
  await p.install(bundle());
  await assert.rejects(p.install(bundle(manifest(), 'throw new Error("replacement")')));
  assert.equal((await p.receipt('example', '1.0.0')).files['main.mjs'], source);
});

test('candidate crash during registry commit restores selection and retains previous process', async t => {
  const p = await platform(t);
  await p.install(bundle());
  const old = await p.enable('example', { trusted: true });
  await p.install(bundle(manifest('example', '2.0.0'), `${source}; setTimeout(() => process.exit(7), 30);`));
  const save = p.save.bind(p);
  p.save = async registry => { await new Promise(r => setTimeout(r, 120)); return save(registry); };
  await assert.rejects(p.enable('example', { version: '2.0.0', trusted: true }), /activation commit/);
  assert.equal((await p.registry()).example.current, '1.0.0');
  assert.equal(p.active.get('example').session, old);
  assert.equal(old.closed, false);
});

test('concurrent installs on one store serialize instead of dropping registry records', async t => {
  const p = await platform(t);
  await Promise.all([p.install(bundle(manifest('first'))), p.install(bundle(manifest('second')))]);
  assert.deepEqual((await p.discover()).map(x => x.id).sort(), ['first', 'second']);
  assert.equal((await readdir(p.root)).includes('.mutation-lock'), false);
});
