import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { PassThrough } from 'node:stream';
import { linuxSandbox } from '../linux-sandbox.mjs';
import { Session } from '../session.mjs';
import { readFrames, MAX_MESSAGE } from '../wire.mjs';

const runtime = fileURLToPath(new URL('..', import.meta.url));
const linux = process.platform === 'linux' && process.arch === 'x64';

test('Linux OS denies raw native filesystem writes, host reads, socket and fork syscalls; hard resource limits apply', { skip: !linux }, async t => {
  const root = await mkdtemp(resolve(tmpdir(), 'gpui-os-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  execFileSync('cc', [fileURLToPath(new URL('./os-probe.c', import.meta.url)), '-pthread', '-o', resolve(root, 'probe')]);
  const config = await linuxSandbox(root, runtime);
  const nodeAt = config.execArgv.indexOf('/bin/node', config.execArgv.indexOf('--seccomp'));
  const child = spawn(config.execPath, [...config.execArgv.slice(0, nodeAt), '/app/probe'], { stdio: config.stdio, env: {} });
  let stdout = '', stderr = '';
  child.stdout.on('data', data => stdout += data); child.stderr.on('data', data => stderr += data);
  const exit = once(child, 'exit'); await config.afterSpawn();
  assert.equal((await exit)[0], 0, stderr);
  t.diagnostic(`Native OS probe: ${stdout.trim()}`);
  assert.deepEqual(JSON.parse(stdout), { readonly: 1, hidden: 1, network: 1, processes: 1, clone: 1, clone3: 1, thread_bypass: 1, threads: 1, allocation: 1, memory: 2147483648, cpu: 30, files: 64 });
});

test('untrusted TS executes through actual Linux isolation and still uses broker storage after consent', { skip: !linux }, async t => {
  const root = await mkdtemp(resolve(tmpdir(), 'gpui-isolated-'));
  await writeFile(resolve(root, 'app.mts'), `
    const value = gpui.state<string>('before'); gpui.mount(() => gpui.text('result', value.get()));
    await gpui.storage.set('answer', 42); value.set(String(await gpui.storage.get('answer')));
  `);
  const session = new Session({ root, entry: 'app.mts', sandbox: 'linux', requested: ['storage'], storageRoot: resolve(root, 'data') });
  t.after(async () => { await session.stop(); await rm(root, { recursive: true, force: true }); });
  const diagnostics = [];
  session.on('error', error => diagnostics.push(error.message));
  session.on('log', error => diagnostics.push(error.message));
  session.on('fault', error => diagnostics.push(error.message));
  session.on('permission', ({ capability }) => session.decide(capability, true));
  const ready = once(session, 'ready', { signal: AbortSignal.timeout(5000) });
  await session.start();
  try { await ready; } catch (error) { assert.fail(`${error.message}: ${diagnostics.join('\n')}`); }
  assert.equal(session.tree.text, '42');
});

test('protocol caps bytes before parsing, preserves split UTF-8 and rejects truncated frames', () => {
  const stream = new PassThrough(); const frames = []; const errors = [];
  readFrames(stream, x => frames.push(x), x => errors.push(x.message));
  const bytes = Buffer.from('{"text":"你好"}\n');
  stream.write(bytes.subarray(0, 11)); stream.write(bytes.subarray(11));
  assert.deepEqual(frames, [{ text: '你好' }]);
  for (let i = 0; i < 5; i++) stream.write(Buffer.alloc(MAX_MESSAGE / 4, 97));
  assert.deepEqual(errors, ['Protocol frame exceeds limit']);
});

test('failed OS sandbox launch refuses execution instead of falling back to a host process', { skip: !linux }, async t => {
  const root = await mkdtemp(resolve(tmpdir(), 'gpui-os-failure-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const config = await linuxSandbox(root, resolve(root, 'missing-runtime'));
  const child = spawn(config.execPath, [...config.execArgv, '-e', 'console.log("UNSAFE-FALLBACK")'], { stdio: config.stdio, env: {} });
  let stdout = '', stderr = '';
  child.stdout.on('data', data => stdout += data); child.stderr.on('data', data => stderr += data);
  const exit = once(child, 'exit'); await config.afterSpawn();
  assert.notEqual((await exit)[0], 0);
  assert.equal(stdout, '');
  assert.match(stderr, /missing-runtime|No such file/);
  t.diagnostic(`OS launch refusal: ${stderr.trim()}`);
});

test('opt-in inspector evaluates inside isolated worker and remains denied without opt-in', { skip: !linux }, async t => {
  const root = await mkdtemp(resolve(tmpdir(), 'gpui-debug-'));
  await writeFile(resolve(root, 'app.mjs'), "gpui.mount(() => gpui.text('value', 'ready'));");
  const session = new Session({ root, entry: 'app.mjs', sandbox: 'linux', debug: true });
  const normal = new Session({ root, entry: 'app.mjs', sandbox: 'linux' });
  session.on('error', () => {});
  t.after(async () => { await session.stop(); await normal.stop(); await rm(root, { recursive: true, force: true }); });
  const ready = once(session, 'ready', { signal: AbortSignal.timeout(5000) });
  await session.start(); await ready;
  assert.equal((await session.debugEvaluate('7 + 3')).result.value, 10);
  const read = await session.debugEvaluate("process.getBuiltinModule('fs').readFileSync('/etc/passwd','utf8')");
  assert.ok(read.exceptionDetails, 'debug evaluation must not gain host filesystem access');
  await assert.rejects(normal.debugEvaluate('7 + 3'), /unavailable/);
  await session.stop();
  await assert.rejects(session.debugEvaluate('1'), /unavailable/);
});
