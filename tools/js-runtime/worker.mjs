import { pathToFileURL } from 'node:url';
import { inspect } from 'node:util';
import { validateTree } from './tree.mjs';
import { readFrames, encodeFrame, validatePayload } from './wire.mjs';
import { createKitBindings } from './kit-bindings.mjs';
import { invocationTarget } from './invocation.mjs';
import inspector from 'node:inspector';

const generation = Number(process.argv[3]);
const send = (message) => process.stdout.write(encodeFrame({ ...message, generation }));
let revision = 0;
let mounted;
let disposed = false;
let sequence = 0;
const handlers = new Map();
const commands = new Map();
const pending = new Map();
const nativeCalls = new Map();
let tree;
const cleanups = [];
let debuggerSession;
const report = (error) => send({ kind: 'error', message: String(error?.stack ?? error).slice(0, 16384) });
for (const level of ['log', 'info', 'warn', 'error', 'debug']) {
  console[level] = (...args) => send({ kind: 'log', level, message: args.map(x => inspect(x, { depth: 3 })).join(' ').slice(0, 16384) });
}
function render() {
  if (disposed || !mounted) return;
  handlers.clear();
  const next = mounted();
  validateTree(next);
  tree = next;
  for (const call of nativeCalls.values()) call.reject(new Error('Native request cancelled by render revision change'));
  nativeCalls.clear();
  send({ kind: 'render', revision: ++revision, tree });
}
async function invoke(target, method, args = {}, mode = 'invoke') {
  if (disposed) throw new Error('Session disposed');
  invocationTarget(tree, target, method, args, mode);
  if (nativeCalls.size >= 32) throw new Error('Native request limit exceeded');
  return new Promise((resolve, reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => { nativeCalls.delete(id); reject(new Error('Native request timed out')); }, 3000);
    nativeCalls.set(id, { revision, resolve(value) { clearTimeout(timer); resolve(value); }, reject(error) { clearTimeout(timer); reject(error); } });
    send({ kind: 'invoke', id, revision, target: { id: target.id, component: target.component }, method, args, mode });
  });
}
function request(capability, args) {
  if (disposed) return Promise.reject(new Error('Session disposed'));
  if (pending.size >= 64) return Promise.reject(new Error('Too many outstanding host requests'));
  return new Promise((resolve, reject) => {
    const id = ++sequence;
    pending.set(id, { resolve, reject });
    send({ kind: 'request', id, capability, args });
  });
}
globalThis.gpui = Object.freeze({
  invoke: (target, method, args) => invoke(target, method, args),
  query: (target, method, args) => invoke(target, method, args, 'query'),
  kit: createKitBindings((id, event, handler) => {
    const action = `${id}:${event}`;
    if (handlers.has(action)) throw new Error('Duplicate Kit event identity');
    handlers.set(action, handler);
    return action;
  }),
  mount(view) { mounted = view; render(); },
  state(initial) {
    let value = initial;
    return { get: () => value, set(next) { if (disposed) return; value = typeof next === 'function' ? next(value) : next; render(); } };
  },
  column: (id, children) => ({ kind: 'column', id, children }),
  row: (id, children) => ({ kind: 'row', id, children }),
  text: (id, text) => ({ kind: 'text', id, text: String(text) }),
  button(id, text, onClick, disabled = false) {
    if (!disabled) handlers.set(id, onClick);
    return { kind: 'button', id, text, disabled, ...(!disabled ? { action: id } : {}) };
  },
  command(id, handler) {
    if (commands.has(id)) throw new Error(`Duplicate command: ${id}`);
    commands.set(id, handler);
    return () => commands.delete(id);
  },
  onDispose(callback) { cleanups.push(callback); },
  fs: Object.freeze({ readText: path => request('fs.read', { path }) }),
  storage: Object.freeze({ get: key => request('storage', { op: 'get', key }), set: (key, value) => request('storage', { op: 'set', key, value }) }),
  network: Object.freeze({ get: url => request('network', { url }) }),
  process: Object.freeze({ run: (command, args = []) => request('process', { command, args }) }),
});
readFrames(process.stdin, async message => {
  if (message.generation !== generation || disposed) return;
  try {
    if (message.kind === 'event' && message.revision === revision) {
      await handlers.get(message.action)?.(validatePayload(message.payload ?? null));
    } else if (message.kind === 'command') {
      await commands.get(message.command)?.();
    } else if (message.kind === 'response') {
      const call = pending.get(message.id);
      pending.delete(message.id);
      if (message.error) call?.reject(new Error(message.error)); else call?.resolve(message.value);
    } else if (message.kind === 'native-response') {
      const call = nativeCalls.get(message.id);
      if (!call) return;
      nativeCalls.delete(message.id);
      if (message.revision !== call.revision || revision !== call.revision) call.reject(new Error('Stale native response'));
      else if (message.error) call.reject(new Error(message.error));
      else call.resolve(validatePayload(message.value));
    } else if (message.kind === 'debug-evaluate') {
      if (process.argv[4] !== 'debug') throw new Error('Debugger not enabled');
      if (typeof message.expression !== 'string' || message.expression.length > 16384) throw new Error('Debug request exceeds limit');
      if (!debuggerSession) { debuggerSession = new inspector.Session(); debuggerSession.connect(); }
      debuggerSession.post('Runtime.evaluate', { expression: message.expression, returnByValue: true, awaitPromise: true, timeout: 1000 }, (error, value) => {
        try { send({ kind: 'debug-response', id: message.id, ...(error ? { error: error.message } : { value }) }); }
        catch (error) { send({ kind: 'debug-response', id: message.id, error: error.message }); }
      });
    } else if (message.kind === 'dispose') {
      disposed = true;
      clearInterval(heartbeat);
      for (const call of pending.values()) call.reject(new Error('Session disposed'));
      pending.clear();
      for (const call of nativeCalls.values()) call.reject(new Error('Session disposed'));
      nativeCalls.clear();
      debuggerSession?.disconnect();
      for (const cleanup of cleanups.reverse()) { try { await cleanup(); } catch (error) { report(error); } }
      send({ kind: 'disposed' });
      process.exit(0);
    }
  } catch (error) { report(error); }
}, error => { report(error); process.exit(1); });
process.on('uncaughtException', error => { report(error); process.exit(1); });
process.on('unhandledRejection', error => { report(error); process.exit(1); });
process.stdin.on('end', () => process.exit(0));
const heartbeat = setInterval(() => send({ kind: 'heartbeat' }), 100);
try {
  await import(pathToFileURL(process.argv[2]).href);
  send({ kind: 'ready' });
} catch (error) { report(error); process.exit(1); }
