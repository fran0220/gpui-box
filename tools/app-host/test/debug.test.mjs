import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, stat, mkdir, chmod, symlink } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { createConnection } from 'node:net';
import { execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { startDebug, evaluateDebug, debugEndpoint, debugPipeHelper } from '../debug.mjs';
import { MAX_MESSAGE } from '../../js-runtime/wire.mjs';

async function directory(t) {
  const data = await mkdtemp(resolve(tmpdir(), 'gpui-debug-'));
  t.after(() => rm(data, { recursive: true, force: true }));
  return data;
}
async function fixture(t, evaluate = expression => `evaluated:${expression}`) {
  const data = await directory(t);
  const server = await startDebug(data, evaluate);
  t.after(() => server.close());
  return { data, server };
}
async function raw(data, frame) {
  const socket = createConnection(debugEndpoint(data));
  socket.on('error', () => {});
  const closed = new Promise(resolve => socket.once('close', resolve));
  await once(socket, 'connect');
  socket.resume();
  socket.write(frame);
  await closed;
}

test('debug OS security is owner-only, permits the owner and removes the endpoint on close', { timeout: 15000 }, async t => {
  const { data, server } = await fixture(t);
  assert.equal(server.endpoint, debugEndpoint(data));
  if (process.platform === 'win32') {
    assert.equal(server.security.transport, 'windows-named-pipe');
    assert.equal(server.security.protected, true);
    const owner = execFileSync('whoami.exe', ['/user', '/fo', 'csv', '/nh'], { encoding: 'utf8' }).match(/S-1-[\d-]+/)[0];
    assert.equal(server.security.owner, owner);
    assert.deepEqual(server.security.allowedSids, [owner]);
    assert.match(server.security.sddl, /D:P/);
    assert.equal((server.security.sddl.match(/\(A;/g) || []).length, 1);
    const denied = JSON.parse(execFileSync(debugPipeHelper(), ['probe-denied', server.endpoint], { encoding: 'utf8', timeout: 5000 }));
    assert.deepEqual(denied, { restrictedTokenDenied: true, win32Error: 5 });
    const anonymous = JSON.parse(execFileSync(debugPipeHelper(), ['probe-anonymous', server.endpoint], { encoding: 'utf8', timeout: 5000 }));
    assert.deepEqual(anonymous, { anonymousClientRejected: true });
  } else {
    assert.equal(server.security.transport, 'posix-socket');
    const socket = await stat(server.endpoint), folder = await stat(resolve(data, 'debug'));
    assert.equal(socket.mode & 0o777, 0o600);
    assert.equal(folder.mode & 0o777, 0o700);
    assert.equal(socket.uid, process.getuid());
    assert.equal(folder.uid, process.getuid());
  }
  assert.equal(await evaluateDebug(data, 'π + 7'), 'evaluated:π + 7');
  assert.equal(await evaluateDebug(data, 'second'), 'evaluated:second');
  await server.close();
  await assert.rejects(evaluateDebug(data, 'after-close'));
  const restarted = await startDebug(data, () => 41);
  try { assert.equal(await evaluateDebug(data, 'restart'), 41); } finally { await restarted.close(); }
});

test('duplicate endpoint fails startup without disturbing the live server', { timeout: 15000 }, async t => {
  const { data } = await fixture(t);
  await assert.rejects(startDebug(data, () => 'wrong-server'));
  assert.equal(await evaluateDebug(data, 'original'), 'evaluated:original');
});

test('debug validates expressions, bounds frames and propagates evaluator errors', { timeout: 15000 }, async t => {
  let calls = 0;
  const { data } = await fixture(t, () => { calls++; throw new Error('evaluation refused'); });
  await assert.rejects(evaluateDebug(data, null), /must be a string/);
  await assert.rejects(evaluateDebug(data, 'x'.repeat(MAX_MESSAGE)), /exceeds limit/);
  await raw(data, 'x'.repeat(MAX_MESSAGE + 1));
  await raw(data, '{"expression":\n');
  assert.equal(calls, 0);
  await assert.rejects(evaluateDebug(data, 'valid'), /evaluation refused/);
  assert.equal(calls, 1);
});

test('debug frame limit accepts the boundary and rejects one byte more', { timeout: 15000 }, async t => {
  const { data } = await fixture(t, expression => Buffer.byteLength(expression));
  const overhead = Buffer.byteLength('{"expression":""}\n');
  const expression = 'a'.repeat(MAX_MESSAGE - overhead);
  assert.equal(await evaluateDebug(data, expression), MAX_MESSAGE - overhead);
  await assert.rejects(evaluateDebug(data, expression + 'b'), /exceeds limit/);
});

test('evaluation deadline aborts the evaluator and does not poison the next request', { timeout: 15000 }, async t => {
  let signal;
  const { data } = await fixture(t, (expression, options) => {
    if (expression === 'stall') { signal = options.signal; return new Promise(() => {}); }
    return 23;
  });
  await assert.rejects(evaluateDebug(data, 'stall'), /cancelled or timed out/);
  assert.equal(signal.aborted, true);
  assert.equal(await evaluateDebug(data, 'recovered'), 23);
});

test('caller cancellation and closing active evaluation are bounded', { timeout: 15000 }, async t => {
  let entered;
  const called = new Promise(resolve => { entered = resolve; });
  let evaluationSignal;
  const { data, server } = await fixture(t, (_, { signal }) => { evaluationSignal = signal; entered(); return new Promise(() => {}); });
  const cancel = new AbortController();
  const request = assert.rejects(evaluateDebug(data, 'pending', { signal: cancel.signal }), /cancelled/);
  await called;
  cancel.abort();
  await request;
  await server.close();
  assert.equal(evaluationSignal.aborted, true);
});

test('shutdown closes a connected client that never sends a frame', { timeout: 15000 }, async t => {
  const { data, server } = await fixture(t);
  const socket = createConnection(debugEndpoint(data));
  socket.on('error', () => {});
  const closed = new Promise(resolve => socket.once('close', resolve));
  await once(socket, 'connect');
  socket.resume();
  await server.close();
  await closed;
});

test('unsafe transport configuration is refused rather than silently weakened', { timeout: 15000 }, async t => {
  const data = await directory(t);
  if (process.platform === 'win32') {
    const saved = process.env.GPUI_DEBUG_PIPE_HELPER;
    process.env.GPUI_DEBUG_PIPE_HELPER = resolve(data, 'missing-helper.exe');
    try { await assert.rejects(startDebug(data, () => 1), /Windows debug helper failed/); }
    finally { if (saved === undefined) delete process.env.GPUI_DEBUG_PIPE_HELPER; else process.env.GPUI_DEBUG_PIPE_HELPER = saved; }
  } else {
    await mkdir(resolve(data, 'debug'));
    await chmod(resolve(data, 'debug'), 0o755);
    await assert.rejects(startDebug(data, () => 1), /must be private/);
    await rm(resolve(data, 'debug'), { recursive: true });
    const target = await directory(t);
    await symlink(target, resolve(data, 'debug'));
    await assert.rejects(startDebug(data, () => 1), /must be private/);
  }
});
