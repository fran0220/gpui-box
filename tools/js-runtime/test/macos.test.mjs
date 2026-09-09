import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { macosSandbox } from '../macos-sandbox.mjs';
import { Session } from '../session.mjs';

const native = process.platform === 'darwin';
const runtime = fileURLToPath(new URL('..', import.meta.url));
async function fixture(t) {
  const root = await mkdtemp(resolve(tmpdir(), 'gpui-macos-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const launcher = resolve(root, 'launcher'), probe = resolve(root, 'probe');
  execFileSync('cc', ['-Wall', '-Wextra', '-Werror', '-O2', resolve(runtime, 'native/macos-launch.c'), '-o', launcher]);
  execFileSync('cc', ['-Wall', '-Wextra', '-Werror', '-pthread', resolve(runtime, 'test/macos-probe.c'), '-o', probe]);
  return { root, launcher, probe };
}
function run(config, root, args = []) {
  const child = spawn(config.execPath, [...config.execArgv, ...args], { cwd: root, env: {}, stdio: config.stdio });
  let stdout = '', stderr = '';
  child.stdout.on('data', chunk => stdout += chunk); child.stderr.on('data', chunk => stderr += chunk);
  const done = once(child, 'close').then(([code, signal]) => ({ code, signal, stdout, stderr }));
  return { child, done };
}
test('macOS Seatbelt denies native filesystem/network/fork/spawn, permits threads and applies kernel limits', { skip: !native }, async t => {
  const f = await fixture(t);
  const config = await macosSandbox(f.root, runtime, { executable: f.probe, node: false, launcher: f.launcher });
  const result = await run(config, f.root).done;
  assert.equal(result.code, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), { writes: 1, reads: 1, network: 1, fork: 1, spawn: 1, threads: 1, cpu: 30, files: 64, bytes: 1048576 });
  t.diagnostic(`macOS native probe: ${result.stdout.trim()}`);
});
test('macOS footprint budget kills oversized native allocation; malformed policy never executes payload', { skip: !native }, async t => {
  const f = await fixture(t);
  const config = await macosSandbox(f.root, runtime, { executable: f.probe, node: false, launcher: f.launcher });
  assert.equal((await run(config, f.root, ['--memory']).done).code, 137);
  config.execArgv[0] = '(invalid sandbox profile';
  const failure = await run(config, f.root).done;
  assert.notEqual(failure.code, 0); assert.equal(failure.stdout, '');
});
test('macOS killing the outer launcher kills and reaps its worker through the independent watcher', { skip: !native }, async t => {
  const f = await fixture(t);
  const config = await macosSandbox(f.root, runtime, { executable: f.probe, node: false, launcher: f.launcher });
  const { child, done } = run(config, f.root, ['--wait']);
  const [data] = await once(child.stdout, 'data', { signal: AbortSignal.timeout(5000) });
  const pid = Number(data.toString().trim()); assert.ok(pid > 0);
  child.kill('SIGKILL'); await done;
  for (let i = 0; i < 100; i++) {
    try { process.kill(pid, 0); } catch (error) { assert.equal(error.code, 'ESRCH'); return; }
    await new Promise(resolve => setTimeout(resolve, 25));
  }
  assert.fail('worker survived launcher death');
});
test('macOS isolated TypeScript mounts a real view and disposes', { skip: !native }, async t => {
  const f = await fixture(t);
  await writeFile(resolve(f.root, 'app.mts'), "const value:number=17;gpui.mount(()=>gpui.text('value',value));");
  const session = new Session({ root: f.root, entry: 'app.mts', sandbox: 'macos', sandboxLauncher: f.launcher });
  t.after(() => session.stop()); session.on('error', error => t.diagnostic(error.message));
  const ready = once(session, 'ready', { signal: AbortSignal.timeout(5000) });
  await session.start(); await ready; assert.equal(session.tree.text, '17');
  await session.stop(); assert.equal(session.closed, true);
});
