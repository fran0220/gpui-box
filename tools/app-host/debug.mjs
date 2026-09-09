// Developer-only transport: private filesystem socket on POSIX, protected
// current-user ACL + authenticated local named pipe on Windows.
import { createServer, createConnection } from 'node:net';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmod, mkdir, lstat } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { readFrames, encodeFrame } from '../js-runtime/wire.mjs';

const REQUEST_MS = 3500;
export const debugPipeHelper = () => process.env.GPUI_DEBUG_PIPE_HELPER || fileURLToPath(new URL('../../target/app-host-windows/gpui-debug-pipe.exe', import.meta.url));
export const debugEndpoint = data => process.platform === 'win32'
  ? `\\\\.\\pipe\\gpui-box-debug-${createHash('sha256').update(resolve(data).toLowerCase()).digest('hex')}`
  : resolve(data, 'debug', 'debug.sock');

async function evaluateRequest(request, evaluate, signal) {
  if (!request || typeof request.expression !== 'string') throw new Error('Debug expression must be a string');
  const deadline = AbortSignal.any([signal, AbortSignal.timeout(REQUEST_MS)]);
  deadline.throwIfAborted();
  let aborted;
  try {
    return await Promise.race([
      Promise.resolve().then(() => evaluate(request.expression, { signal: deadline })),
      new Promise((_, reject) => { aborted = () => reject(new Error('Debug evaluation cancelled or timed out')); deadline.addEventListener('abort', aborted, { once: true }); }),
    ]);
  } finally { deadline.removeEventListener('abort', aborted); }
}

export async function startDebug(data, evaluate) {
  if (process.platform === 'win32') return startWindowsDebug(data, evaluate);
  const directory = resolve(data, 'debug');
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const info = await lstat(directory);
  if (!info.isDirectory() || info.uid !== process.getuid() || (info.mode & 0o077)) throw new Error('Debug directory must be private and owned by the current user');
  const clients = new Set();
  const shutdown = new AbortController();
  const server = createServer(socket => {
    clients.add(socket); socket.on('close', () => clients.delete(socket));
    socket.on('error', () => socket.destroy()); socket.setTimeout(5000, () => socket.destroy());
    const cancel = new AbortController();
    socket.on('close', () => cancel.abort());
    let used = false;
    readFrames(socket, request => {
      if (used) { socket.destroy(); return; } used = true;
      evaluateRequest(request, evaluate, AbortSignal.any([cancel.signal, shutdown.signal])).then(
        value => socket.end(encodeFrame({ value })),
        error => socket.end(encodeFrame({ error: error.message })),
      ).catch(() => socket.destroy());
    }, () => socket.destroy());
  });
  server.maxConnections = 4;
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(debugEndpoint(data), resolve); });
  try { await chmod(debugEndpoint(data), 0o600); }
  catch (error) { server.close(); throw error; }
  let closing;
  return { endpoint: debugEndpoint(data), security: { transport: 'posix-socket' }, close() {
    return closing ||= new Promise((resolve, reject) => {
      shutdown.abort();
      for (const socket of clients) socket.destroy();
      server.close(error => error ? reject(error) : resolve());
    });
  } };
}

async function startWindowsDebug(data, evaluate) {
  const child = spawn(debugPipeHelper(), ['serve', debugEndpoint(data)], { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
  const cancel = new AbortController();
  let stderr = '', stopping = false, closing, ready = false, busy = false, failure;
  let resolveReady, rejectReady;
  const started = new Promise((resolve, reject) => { resolveReady = resolve; rejectReady = reject; });
  const fail = error => {
    if (failure) return;
    failure = error; rejectReady(error); cancel.abort(); child.stdin.destroy(); child.kill();
  };
  child.stderr.on('data', chunk => { stderr = (stderr + chunk.toString()).slice(-8192); });
  child.on('error', error => fail(new Error(`Windows debug helper failed: ${error.message}. Build tools/app-host/native/build-windows-debug.ps1`)));
  child.stdin.on('error', fail);
  const closed = new Promise((resolve, reject) => child.once('close', (code, signal) => {
    cancel.abort();
    if (stopping && code === 0 && !failure) resolve();
    else {
      const error = failure || new Error(`Windows debug helper exited (${code ?? signal}): ${stderr.trim()}`);
      rejectReady(error); reject(error);
    }
  }));
  closed.catch(() => {}); // Observable via closed/close, without an unhandled rejection.
  readFrames(child.stdout, message => {
    if (!ready) {
      if (message?.ready !== true || message.security?.transport !== 'windows-named-pipe' || message.security.protected !== true ||
          !/^S-1-/.test(message.security.owner) || message.security.allowedSids?.length !== 1 || message.security.allowedSids[0] !== message.security.owner) {
        fail(new Error('Windows debug helper did not attest a protected owner-only ACL')); return;
      }
      ready = true; resolveReady(message.security); return;
    }
    if (stopping) return;
    if (busy) { fail(new Error('Unexpected concurrent debug request')); return; }
    busy = true;
    evaluateRequest(message, evaluate, cancel.signal).then(
      value => encodeFrame({ value }),
      error => encodeFrame({ error: error.message }),
    ).then(frame => { busy = false; if (!stopping && !cancel.signal.aborted) child.stdin.write(frame); }, fail);
  }, fail);
  const startupTimeout = setTimeout(() => fail(new Error('Windows debug helper startup timed out')), 15000);
  let security;
  try { security = await started; }
  catch (error) { fail(error); await closed.catch(() => {}); throw error; }
  finally { clearTimeout(startupTimeout); }
  return { endpoint: debugEndpoint(data), security, closed, close() {
    return closing ||= (async () => {
      stopping = true; cancel.abort(); child.stdin.end();
      const timer = setTimeout(() => child.kill(), 5000);
      try { await closed; } finally { clearTimeout(timer); }
    })();
  } };
}

export function evaluateDebug(data, expression, { signal } = {}) {
  return new Promise((resolve, reject) => {
    let frame;
    try { signal?.throwIfAborted(); frame = encodeFrame({ expression }); } catch (error) { reject(error); return; }
    const socket = createConnection(debugEndpoint(data));
    const fail = error => { socket.destroy(); reject(error); };
    const abort = () => fail(new Error('Debug request cancelled'));
    signal?.addEventListener('abort', abort, { once: true });
    const timer = setTimeout(() => fail(new Error('Debug socket timed out')), 5000);
    socket.on('error', fail);
    socket.on('close', () => { clearTimeout(timer); signal?.removeEventListener('abort', abort); reject(new Error('Debug socket closed without a response')); });
    socket.on('connect', () => socket.write(frame));
    readFrames(socket, message => {
      socket.destroy();
      if (!message || typeof message !== 'object') { reject(new Error('Invalid debug response')); return; }
      message.error ? reject(new Error(message.error)) : resolve(message.value);
    }, fail);
  });
}
