import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, readFile, writeFile, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { readFrames, encodeFrame } from '../../js-runtime/wire.mjs';

const here = fileURLToPath(new URL('..', import.meta.url));
function find(node, id) {
  if (node?.id === id) return node;
  for (const child of node?.children ?? []) { const match = find(child, id); if (match) return match; }
}
async function host(t, dev = false, debug = false) {
  const dir = await mkdtemp(resolve(tmpdir(), 'gpui-host-'));
  const app = resolve(dir, 'app'), data = resolve(dir, 'data');
  execFileSync(process.execPath, [resolve(here, 'cli.mjs'), 'init', app]);
  const bundle = resolve(dir, 'plugin.json');
  execFileSync(process.execPath, [resolve(here, 'cli.mjs'), 'bundle', resolve(here, 'example-plugin'), bundle]);
  execFileSync(process.execPath, [resolve(here, 'cli.mjs'), 'plugin-install', bundle, resolve(data, 'plugins')]);
  const child = spawn(process.execPath, [resolve(here, 'runner.mjs'), app, '--data-dir', data, ...(dev ? ['--dev'] : []), ...(debug ? ['--debug'] : [])], { stdio: ['pipe', 'pipe', 'pipe'] });
  let latest, stderr = '';
  const waiters = new Set();
  readFrames(child.stdout, frame => { latest = frame; for (const check of waiters) check(); }, error => assert.fail(error.message));
  child.stderr.on('data', data => stderr += data);
  const wait = predicate => new Promise((resolve, reject) => {
    const timer = setTimeout(() => { waiters.delete(check); reject(new Error(`Host wait timed out (${predicate}): ${stderr}\n${JSON.stringify(latest)}`)); }, 6000);
    function check() { if (predicate(latest?.tree)) { clearTimeout(timer); waiters.delete(check); resolve(latest); } }
    waiters.add(check); check();
  });
  const send = message => child.stdin.write(encodeFrame(message));
  const click = id => {
    assert.ok(find(latest.tree, id), `missing ${id}`);
    send({ kind: 'event', generation: 0, revision: latest.revision, action: id });
  };
  t.after(async () => {
    if (child.exitCode === null) {
      const exited = once(child, 'exit'); send({ kind: 'close' });
      const kill = setTimeout(() => child.kill('SIGKILL'), 1500);
      await exited; clearTimeout(kill);
    }
    await rm(dir, { recursive: true, force: true });
  });
  await wait(tree => find(tree, 'app.value')?.text === 'Count: 7');
  return { app, data, child, wait, send, click, get frame() { return latest; } };
}

test('end-to-end TS template, state, permission refusal, isolated panel and keymap, cleanup', async t => {
  const h = await host(t);
  execFileSync(process.execPath, [resolve(here, 'node_modules/typescript/bin/tsc'), '--project', resolve(h.app, 'tsconfig.json'), '--noEmit']);
  h.click('app.increment');
  await h.wait(tree => find(tree, 'app.value')?.text === 'Count: 10');
  assert.equal(find(h.frame.tree, 'app.disabled').disabled, true);
  h.click('app.save');
  await h.wait(tree => tree.children.some(n => n.id.startsWith('permission.')));
  const permission = h.frame.tree.children.find(n => n.id.startsWith('permission.'));
  h.click(`${permission.id}.deny`);
  await h.wait(tree => find(tree, 'app.status')?.text.startsWith('Refused: Permission denied'));
  h.click('plugin.sample-panel.enable.1.0.0');
  const countId = 'plugin.sample-panel.panel.counter-panel.view.count';
  await h.wait(tree => find(tree, countId)?.text === 'Plugin count: 0');
  h.send({ kind: 'key', key: 'ctrl-j' });
  await h.wait(tree => find(tree, countId)?.text === 'Plugin count: 1');
  h.click('plugin.sample-panel.disable');
  await h.wait(tree => !find(tree, countId));
  assert.equal(find(h.frame.tree, 'app.value').text, 'Count: 10');
});

test('dev watches imported TS module; bad reload retains verified view and recovery creates new generation', async t => {
  const h = await host(t, true);
  await writeFile(resolve(h.app, 'message.mts'), "export const greeting: string = 'Changed imported module';");
  await h.wait(tree => find(tree, 'app.greeting')?.text === 'Changed imported module');
  const original = await readFile(resolve(h.app, 'main.mts'), 'utf8');
  await writeFile(resolve(h.app, 'main.mts'), "throw new Error('reload rejected');");
  await h.wait(tree => find(tree, 'host.error')?.text.includes('App failed to start'));
  assert.equal(find(h.frame.tree, 'app.greeting').text, 'Changed imported module');
  await writeFile(resolve(h.app, 'main.mts'), original.replace('state<number>(7)', 'state<number>(19)'));
  await h.wait(tree => find(tree, 'app.value')?.text === 'Count: 19');
  assert.equal(find(h.frame.tree, 'host.error'), undefined);
});

test('developer CLI evaluates inside app through a private socket and shutdown removes the endpoint', async t => {
  const h = await host(t, false, true);
  const result = JSON.parse(execFileSync(process.execPath, [resolve(here, 'cli.mjs'), 'debug', h.data, '7 + 3'], { encoding: 'utf8' }));
  assert.equal(result.result.value, 10);
  assert.equal((await stat(resolve(h.data, 'debug/debug.sock'))).mode & 0o777, 0o600);
  assert.equal((await stat(resolve(h.data, 'debug'))).mode & 0o777, 0o700);
  const exit = once(h.child, 'exit'); h.send({ kind: 'close' }); await exit;
  await assert.rejects(stat(resolve(h.data, 'debug/debug.sock')), { code: 'ENOENT' });
});

test('supervisor namespaces Kit events and preserves typed payloads, rejecting stale native frames', async t => {
  const h = await host(t, true);
  await writeFile(resolve(h.app, 'main.mts'), `const checked=gpui.state(false); gpui.mount(()=>gpui.kit.Checkbox('check',{checked:checked.get(),label:String(checked.get())},{change:value=>checked.set(value)}));`);
  await h.wait(tree => find(tree, 'app.check')?.props.label === 'false');
  const initial = h.frame;
  h.send({ kind: 'event', generation: 0, revision: initial.revision, action: find(initial.tree, 'app.check').events.change, payload: true });
  await h.wait(tree => find(tree, 'app.check')?.props.label === 'true');
  h.send({ kind: 'event', generation: 0, revision: initial.revision, action: find(initial.tree, 'app.check').events.change, payload: false });
  await new Promise(resolve => setTimeout(resolve, 100));
  assert.equal(find(h.frame.tree, 'app.check').props.label, 'true');
  assert.ok(find(h.frame.tree, 'app.check').instance > 0);
});

test('native clipboard consent is declared, separate for read/write, and revoked on reload', async t => {
  const h = await host(t, true);
  const initial = find(h.frame.tree, 'app.value').instance;
  const manifest = JSON.parse(await readFile(resolve(h.app, 'app.json'), 'utf8'));
  manifest.permissions.push('clipboard.read', 'clipboard.write');
  await writeFile(resolve(h.app, 'app.json'), JSON.stringify(manifest));
  await h.wait(tree => find(tree, 'app.value')?.instance !== initial);
  const instance = find(h.frame.tree, 'app.value').instance;
  h.send({ kind: 'native-permission', instance, capability: 'clipboard.write' });
  const key = `permission.${instance}.clipboard.write`;
  await h.wait(tree => find(tree, `${key}.allow`));
  assert.deepEqual(h.frame.clipboard[instance], { read: false, write: false });
  h.click(`${key}.allow`);
  await h.wait(() => h.frame.clipboard[instance]?.write);
  assert.deepEqual(h.frame.clipboard[instance], { read: false, write: true });
  h.click('host.reload');
  await h.wait(tree => find(tree, 'app.value')?.instance !== instance);
  assert.equal(h.frame.clipboard[instance], undefined);
  h.send({ kind: 'native-permission', instance, capability: 'clipboard.read' });
  await new Promise(resolve => setTimeout(resolve, 100));
  assert.equal(find(h.frame.tree, `permission.${instance}.clipboard.read.allow`), undefined);
});

test('unchanged native views keep their revision while event routes track worker rerenders', async t => {
  const h = await host(t, true);
  await writeFile(resolve(h.app, 'main.mts'), `const count=gpui.state(0), show=gpui.state(false); gpui.mount(()=>gpui.column('root',[gpui.button('hidden','Change hidden state',()=>count.set(count.get()+1)),gpui.button('reveal','Reveal',()=>show.set(true)),gpui.text('result',show.get()?String(count.get()):'Hidden')]));`);
  await h.wait(tree => find(tree, 'app.result')?.text === 'Hidden');
  const revision = h.frame.revision;
  h.click('app.hidden');
  await new Promise(resolve => setTimeout(resolve, 100));
  assert.equal(h.frame.revision, revision);
  h.click('app.hidden');
  await new Promise(resolve => setTimeout(resolve, 100));
  h.click('app.reveal');
  await h.wait(tree => find(tree, 'app.result')?.text === '2');
});
