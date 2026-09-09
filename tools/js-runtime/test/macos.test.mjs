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
  const outside = await mkdtemp(resolve(tmpdir(), 'gpui-macos-host-'));
  t.after(() => rm(outside, { recursive: true, force: true }));
  const hostFile = resolve(outside, 'host-only.txt');
  await writeFile(hostFile, 'host fixture must remain unreadable');
  return { root, launcher, probe, hostFile };
}
function run(t, config, root, args = []) {
  const child = spawn(config.execPath, [...config.execArgv, ...args], { cwd: root, env: {}, stdio: config.stdio });
  let stdout = '', stderr = '';
  child.stdout.on('data', chunk => stdout += chunk); child.stderr.on('data', chunk => stderr += chunk);
  const deadline = setTimeout(() => child.kill('SIGTERM'), 10000);
  const done = once(child, 'close').then(([code, signal]) => ({ code, signal, stdout, stderr }))
    .finally(() => clearTimeout(deadline));
  t.after(async () => { if (child.exitCode === null && child.signalCode === null) child.kill('SIGTERM'); await done; });
  async function firstLine() {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(new Error(`startup timed out: ${stderr}`)), 5000);
    try {
      return await Promise.race([
        (async () => {
          while (!stdout.includes('\n')) await once(child.stdout, 'data', { signal: controller.signal });
          return stdout.slice(0, stdout.indexOf('\n'));
        })(),
        done.then(result => { throw new Error(`exited before readiness: ${JSON.stringify(result)}`); }),
      ]);
    } finally { clearTimeout(timer); controller.abort(); }
  }
  return { child, done, firstLine };
}
test('native startup observer reports early exits instead of waiting for readiness timeout', async t => {
  const config = { execPath: process.execPath, execArgv: ['-e', "process.stderr.write('early failure');process.exit(23)"], stdio: ['pipe', 'pipe', 'pipe'] };
  const launched = run(t, config, runtime);
  await assert.rejects(launched.firstLine(), /exited before readiness:.*"code":23.*early failure/);
});
test('native startup observer assembles a fragmented readiness line', async t => {
  const config = { execPath: process.execPath, execArgv: ['-e', "process.stdout.write('rea');setTimeout(()=>process.stdout.write('dy\\n'),20)"], stdio: ['pipe', 'pipe', 'pipe'] };
  const launched = run(t, config, runtime);
  assert.equal(await launched.firstLine(), 'ready');
  assert.equal((await launched.done).code, 0);
});
test('macOS minimal native payload reaches main through the real Seatbelt launcher', { skip: !native, timeout: 15000 }, async t => {
  const f = await fixture(t);
  assert.equal(execFileSync(f.probe, ['--startup'], { encoding: 'utf8' }).trim(), 'ready');
  const config = await macosSandbox(f.root, runtime, { executable: f.probe, node: false, launcher: f.launcher });
  const result = await run(t, config, f.root, ['--startup']).done;
  assert.equal(result.code, 0, JSON.stringify(result));
  assert.equal(result.stdout.trim(), 'ready');
  assert.match(result.stderr, /macos-probe: entered main/);
});
test('macOS Seatbelt denies native filesystem/network/fork/spawn, permits threads and applies kernel limits', { skip: !native }, async t => {
  const f = await fixture(t);
  const config = await macosSandbox(f.root, runtime, { executable: f.probe, node: false, launcher: f.launcher });
  const result = await run(t, config, f.root, [f.hostFile]).done;
  assert.equal(result.code, 0, JSON.stringify(result));
  assert.deepEqual(JSON.parse(result.stdout), { writes: 1, reads: 1, hostReads: 1, inherited: 0, network: 1, fork: 1, spawn: 1, threads: 1, cpu: 30, files: 64, bytes: 1048576 });
  t.diagnostic(`macOS native probe: ${result.stdout.trim()}`);
});
test('macOS footprint budget kills oversized native allocation; malformed policy never executes payload', { skip: !native }, async t => {
  const f = await fixture(t);
  const config = await macosSandbox(f.root, runtime, { executable: f.probe, node: false, launcher: f.launcher });
  const oversized = await run(t, config, f.root, ['--memory']).done;
  assert.equal(oversized.code, 137, JSON.stringify(oversized));
  assert.match(oversized.stderr, /reason=footprint-limit/, JSON.stringify(oversized));
  config.execArgv[0] = '(invalid sandbox profile';
  const failure = await run(t, config, f.root).done;
  assert.notEqual(failure.code, 0); assert.equal(failure.stdout, '');
  assert.doesNotMatch(failure.stderr, /macos-probe: entered main/);
});
test('macOS killing the outer launcher kills and reaps its worker through the independent watcher', { skip: !native }, async t => {
  const f = await fixture(t);
  const config = await macosSandbox(f.root, runtime, { executable: f.probe, node: false, launcher: f.launcher });
  const { child, done, firstLine } = run(t, config, f.root, ['--wait']);
  const pid = Number(await firstLine()); assert.ok(pid > 0);
  child.kill('SIGKILL'); await done;
  for (let i = 0; i < 100; i++) {
    try { process.kill(pid, 0); } catch (error) { assert.equal(error.code, 'ESRCH'); return; }
    await new Promise(resolve => setTimeout(resolve, 25));
  }
  assert.fail('worker survived launcher death');
});
test('macOS isolated TypeScript mounts a real view and disposes', { skip: !native }, async t => {
  const f = await fixture(t);
  // Isolate Node/libuv startup from Session's readiness/event handling.
  const config = await macosSandbox(f.root, runtime, { launcher: f.launcher });
  const startup = await run(t, config, f.root, ['-e', "process.stdout.write('node-ready\\n')"]).done;
  assert.equal(startup.code, 0, JSON.stringify(startup));
  assert.equal(startup.stdout.trim(), 'node-ready');
  await writeFile(resolve(f.root, 'app.mts'), "const value:number=17;gpui.mount(()=>gpui.text('value',value));");
  const session = new Session({ root: f.root, entry: 'app.mts', sandbox: 'macos', sandboxLauncher: f.launcher });
  t.after(() => session.stop()); session.on('error', error => t.diagnostic(error.message));
  let logs = '';
  session.on('log', entry => { logs = (logs + entry.message).slice(-16384); });
  const controller = new AbortController();
  const signal = AbortSignal.any([controller.signal, AbortSignal.timeout(5000)]);
  const readiness = Promise.race([
    once(session, 'ready', { signal }),
    once(session, 'exit', { signal }).then(([result]) => {
      throw new Error(`Session exited before ready: ${JSON.stringify(result)}; ${logs}`);
    }),
  ]);
  try { await Promise.all([session.start(), readiness]); }
  finally { controller.abort(); }
  assert.equal(session.tree.text, '17');
  await session.stop(); assert.equal(session.closed, true);
});
