import test from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter, once } from 'node:events';
import childProcess from 'node:child_process';
import { syncBuiltinESMExports } from 'node:module';
import { PassThrough } from 'node:stream';
import { fileURLToPath } from 'node:url';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Session } from '../session.mjs';
import { encodeFrame, readFrames } from '../wire.mjs';

const deferred = () => Promise.withResolvers();
const empty = () => new Session({ root: '.', entry: 'app.mts', trusted: true });

async function worker(t, source) {
  const root = await mkdtemp(resolve(tmpdir(), 'gpui-lifecycle-'));
  await writeFile(resolve(root, 'app.mts'), source);
  const session = new Session({ root, entry: 'app.mts', trusted: true });
  t.after(async () => {
    // Tests may intentionally reject stop, but must still prove actual close
    // before removing a Windows worker's cwd.
    await session.stop().catch(() => {});
    await session.closePromise;
    await rm(root, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
  });
  const ready = once(session, 'ready', { signal: AbortSignal.timeout(5000) });
  await session.start(); await ready;
  return session;
}

test('trusted worker gracefully disposes once and concurrent stop callers share close proof', async t => {
  const session = await worker(t, "gpui.onDispose(() => console.log('disposed-cleanly')); gpui.mount(() => gpui.text('root', 'ready'));");
  let disposed = 0, closed = false;
  session.on('log', ({ message }) => { if (message.includes('disposed-cleanly')) disposed++; });
  session.child.once('close', () => { closed = true; });
  const first = session.stop();
  assert.equal(session.closed, true);
  assert.equal(session.event('late-action'), false);
  assert.equal(session.stop(), first);
  await first;
  assert.equal(session.stop(), first);
  assert.equal(disposed, 1);
  assert.equal(closed, true);
  assert.equal(session.child.exitCode, 0);
});

test('trusted worker stuck in disposal is forcibly killed and actually closed', async t => {
  const session = await worker(t, "gpui.onDispose(() => { while (true) {} }); gpui.mount(() => gpui.text('root', 'ready'));");
  let closed = false;
  session.child.once('close', () => { closed = true; });
  const started = Date.now();
  await session.stop();
  assert.equal(closed, true);
  assert.ok(Date.now() - started < 5000);
  assert.ok(session.child.signalCode !== null || session.child.exitCode !== 0);
});

test('throwing exit listener rejects shutdown without preventing reaping', async t => {
  const session = await worker(t, "gpui.mount(() => gpui.text('root', 'ready'));");
  session.on('exit', () => { throw new Error('exit listener failed'); });
  await assert.rejects(session.stop(), error => {
    assert.match(error.errors[0].errors[0].message, /exit listener failed/);
    return true;
  });
  await session.closePromise;
});

test('throwing fault listener cannot prevent kill or shared shutdown', async t => {
  const session = await worker(t, "gpui.mount(() => gpui.text('root', 'ready'));");
  session.on('fault', () => { throw new Error('fault listener failed'); });
  assert.doesNotThrow(() => session.fail('controlled failure'));
  const stopping = session.stop();
  await assert.rejects(stopping, /fault listener failed/);
  assert.equal(stopping, session.stop());
  await session.closePromise;
});

test('post-close cleanup rejection is surfaced, not converted to success', async () => {
  const session = empty();
  session.exitPromise = Promise.reject(new Error('staging removal failed'));
  await assert.rejects(session.stop(), /staging removal failed/);
});

test('reentrant cancellation sees the same stop promise; launch failure is retained', async () => {
  const session = empty();
  session.starting = Promise.reject(new Error('launch setup failed'));
  let reentrant;
  session.cancelRequests = () => { reentrant = session.stop(); };
  const stopping = session.stop();
  assert.equal(reentrant, stopping);
  await assert.rejects(stopping, /launch setup failed/);
});

test('shutdown deadlines bound launch, missing close, cleanup and operations', { concurrency: true }, async t => {
  await Promise.all(['launch', 'close', 'cleanup', 'operations'].map(stage => t.test(stage, async () => {
    const session = empty();
    const pending = deferred();
    let kills = 0, cleaned = false;
    if (stage === 'launch') session.starting = pending.promise;
    if (stage === 'close') {
      // Even an exit code and a successful kill are not proof of close.
      session.child = { exitCode: 0, signalCode: null, kill(signal) { assert.equal(signal, 'SIGKILL'); kills++; return true; } };
      session.closePromise = pending.promise;
      session.exitPromise = pending.promise.then(() => { cleaned = true; });
    }
    if (stage === 'cleanup') session.exitPromise = pending.promise.then(() => { cleaned = true; });
    if (stage === 'operations') session.operations.add(pending.promise);
    const started = Date.now();
    const stopping = session.stop();
    assert.equal(stopping, session.stop());
    await assert.rejects(stopping, /shutdown deadline exceeded/);
    assert.ok(Date.now() - started < 7000, `${stage} must not wait forever`);
    if (stage === 'close') {
      assert.equal(kills, 1);
      assert.equal(cleaned, false, 'cleanup cannot run before actual close');
    }
    pending.resolve();
    await session.exitPromise;
    if (stage === 'close' || stage === 'cleanup') assert.equal(cleaned, true, 'late cleanup remains attached');
    await assert.rejects(session.stop(), /shutdown deadline exceeded/, 'late completion does not rewrite the failed stop result');
  })));
});

test('two-second native cleanup fits the shutdown budget', async () => {
  const session = empty();
  session.exitPromise = new Promise(resolve => setTimeout(resolve, 2000));
  await session.stop();
});

// Exercise the real Session framing/watchdog with a controlled launcher clock,
// rather than sleeping or relying on a particular CI machine's startup speed.
async function launcher(t) {
  t.mock.timers.enable({ apis: ['Date', 'setInterval', 'setTimeout'], now: Date.now() });
  const child = new EventEmitter();
  child.stdin = new PassThrough(); child.stdout = new PassThrough(); child.stderr = new PassThrough();
  let kills = 0;
  child.kill = signal => { kills++; queueMicrotask(() => child.emit('close', null, signal)); return true; };
  readFrames(child.stdin, message => { if (message.kind === 'dispose') queueMicrotask(() => child.emit('close', 0, null)); });
  const spawn = t.mock.method(childProcess, 'spawn', () => child);
  syncBuiltinESMExports();
  t.after(() => { spawn.mock.restore(); syncBuiltinESMExports(); });
  const session = new Session({ root: fileURLToPath(new URL('.', import.meta.url)), entry: 'lifecycle.test.mjs', trusted: true });
  const faults = [];
  session.on('fault', fault => faults.push(fault.message));
  await session.start();
  t.after(() => session.stop());
  return { session, faults, get kills() { return kills; }, send(message) { child.stdout.write(encodeFrame({ generation: session.generation, ...message })); } };
}

test('native startup can exceed heartbeat budget; first heartbeat switches to the strict running budget', async t => {
  const run = await launcher(t);
  t.mock.timers.tick(2500);
  assert.equal(run.session.closed, false, 'launcher startup is not a stalled worker');
  run.send({ kind: 'heartbeat' });
  run.send({ kind: 'ready' });
  t.mock.timers.tick(1900);
  assert.equal(run.session.closed, false);
  run.send({ kind: 'heartbeat' });
  t.mock.timers.tick(1900);
  assert.equal(run.session.closed, false, 'a continuing heartbeat renews liveness');
  t.mock.timers.tick(200);
  assert.deepEqual(run.faults, ['Worker heartbeat deadline exceeded']);
  await run.session.stop();
  assert.equal(run.kills, 1);
  assert.equal(run.session.childClosed, true);
});

test('silent startup is bounded and other generations or logs cannot extend its deadline', async t => {
  const run = await launcher(t);
  t.mock.timers.tick(29900);
  assert.equal(run.session.closed, false);
  run.send({ kind: 'heartbeat', generation: run.session.generation + 1 });
  run.send({ kind: 'log', level: 'info', message: 'still launching' });
  t.mock.timers.tick(100);
  assert.deepEqual(run.faults, ['Worker startup deadline exceeded (no heartbeat)']);
  await run.session.stop();
  assert.equal(run.kills, 1);
  assert.equal(run.session.childClosed, true);
});

test('first heartbeat arms liveness even if guest activation never becomes ready', async t => {
  const run = await launcher(t);
  run.send({ kind: 'heartbeat' });
  t.mock.timers.tick(2100);
  assert.deepEqual(run.faults, ['Worker heartbeat deadline exceeded']);
  await run.session.stop();
  assert.equal(run.session.childClosed, true);
});
