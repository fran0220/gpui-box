import { spawn } from 'node:child_process';
import { EventEmitter } from 'node:events';
import { fileURLToPath } from 'node:url';
import { dirname, resolve, sep, isAbsolute } from 'node:path';
import { realpath, readFile, mkdir, writeFile, rename, open, rm, readdir, stat } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import { validateTree } from './tree.mjs';
import { createSandbox, nativeBackend } from './sandbox.mjs';
import { readFrames, encodeFrame, MAX_MESSAGE, validatePayload } from './wire.mjs';

const runtimeRoot = dirname(fileURLToPath(import.meta.url));
let nextGeneration = 0;

export async function containedFile(root, relative) {
  if (typeof relative !== 'string' || !relative || isAbsolute(relative) || relative.includes('\\') || relative.split('/').some(p => !p || p === '.' || p === '..'))
    throw new Error('Invalid relative path');
  const base = await realpath(root);
  const target = await realpath(resolve(base, relative));
  if (!target.startsWith(base + sep)) throw new Error('Path escapes capability root');
  return target;
}

/** One app/plugin process. Untrusted execution requires the explicit Linux OS backend. */
export class Session extends EventEmitter {
  constructor({ root, entry, trusted = false, sandbox, sandboxLauncher, debug = false, requested = [], grants = [], storageRoot, origins = [], executables = {}, timeoutMs = 2000 }) {
    super();
    if (!trusted && !['linux', 'macos', 'windows'].includes(sandbox)) throw new Error('Untrusted execution unavailable: no OS sandbox backend configured');
    this.options = { root, entry, sandbox, sandboxLauncher, debug, requested, storageRoot, origins, executables, timeoutMs };
    this.grants = new Set(grants.filter(p => requested.includes(p)));
    this.generation = ++nextGeneration;
    this.revision = 0;
    this.closed = false;
    this.aborts = new Set();
    this.inflight = new Set();
    this.permissionWaiters = new Map();
    this.storageWrites = Promise.resolve();
    this.debugRequests = new Map();
    this.debugSequence = 0;
    this.operations = new Set();
  }
  start() {
    if (this.starting || this.child || this.closed) return Promise.reject(new Error('Session cannot be started twice'));
    this.starting = this.startProcess();
    return this.starting;
  }
  async startProcess() {
    const entry = await containedFile(this.options.root, this.options.entry);
    const root = await realpath(this.options.root);
    const isolation = this.options.sandbox ? await createSandbox(this.options.sandbox, root, runtimeRoot, { launcher: this.options.sandboxLauncher }) : {
      execPath: process.execPath,
      execArgv: ['--permission', `--allow-fs-read=${root}`, `--allow-fs-read=${runtimeRoot}`, '--max-old-space-size=64', '--disable-proto=throw'],
      stdio: ['pipe', 'pipe', 'pipe'],
    };
    if (this.closed) {
      await isolation.afterSpawn?.(); await isolation.cleanup?.();
      throw new Error('Session disposed during launch');
    }
    const isolated = this.options.sandbox === 'linux';
    this.child = spawn(isolation.execPath, [...isolation.execArgv, ...(this.options.debug ? ['--allow-inspector'] : []),
      isolated ? '/runtime/worker.mjs' : resolve(runtimeRoot, 'worker.mjs'),
      isolated ? `/app/${this.options.entry}` : entry, String(this.generation), this.options.debug ? 'debug' : 'run'], {
      cwd: root, env: { PATH: process.env.PATH ?? '', NODE_NO_WARNINGS: '1' }, stdio: isolation.stdio,
    });
    const child = this.child;
    let lastHeartbeat = Date.now();
    let outputBytes = 0;
    let windowStart = Date.now();
    const meter = bytes => {
      if (Date.now() - windowStart > 1000) { outputBytes = 0; windowStart = Date.now(); }
      outputBytes += bytes;
      if (outputBytes > 1024 * 1024) { this.fail('Worker output rate limit exceeded'); return false; }
      return true;
    };
    child.stderr.on('data', chunk => { if (meter(chunk.length)) this.emit('log', { level: 'error', message: chunk.toString().slice(0, 16384) }); });
    child.stdin.on('error', error => { if (!this.closed) this.fail(error.message); });
    child.on('error', error => this.fail(error.message));
    this.exitPromise = new Promise(resolve => child.on('close', async (code, signal) => {
      clearInterval(this.watchdog); this.cancelRequests();
      const expected = this.closed; this.closed = true;
      try { await isolation.cleanup?.(); }
      catch (error) { this.emit('log', { level: 'error', message: `OS cleanup failed: ${error.message}` }); }
      this.emit('exit', { code, signal, expected }); resolve();
    }));
    readFrames(child.stdout, message => {
      if (message?.generation !== this.generation) return;
      if (this.closed && !['log', 'disposed'].includes(message.kind)) return;
      try {
        const bytes = Buffer.byteLength(JSON.stringify(message));
        if (bytes > MAX_MESSAGE) throw new Error('Worker message exceeds limit');
        if (!meter(bytes)) return;
        if (message.kind === 'heartbeat') { lastHeartbeat = Date.now(); return; }
        if (message.kind === 'debug-response') {
          this.debugRequests.get(message.id)?.(message);
          this.debugRequests.delete(message.id);
          return;
        }
        if (message.kind === 'render') {
          validateTree(message.tree);
          if (!Number.isSafeInteger(message.revision) || message.revision <= this.revision) throw new Error('Invalid revision');
          this.revision = message.revision;
          this.tree = message.tree;
        }
        if (message.kind === 'request') { void this.handleRequest(message); return; }
        if (!['render', 'ready', 'log', 'error', 'disposed'].includes(message.kind)) throw new Error('Unknown worker message');
        this.emit(message.kind, message);
      } catch (error) { this.fail(error.message); }
    }, error => this.fail(error.message));
    this.watchdog = setInterval(() => {
      if (Date.now() - lastHeartbeat > this.options.timeoutMs) this.fail('Worker heartbeat deadline exceeded');
    }, 100);
    // Attach all handlers before yielding: a missing backend can fail immediately.
    await isolation.afterSpawn?.();
    return this;
  }
  send(message) {
    if (!this.closed && this.child?.stdin.writable) {
      if (this.child.stdin.writableLength > MAX_MESSAGE) { this.fail('Worker input queue exceeds limit'); return; }
      this.child.stdin.write(encodeFrame({ ...message, generation: this.generation }));
    }
  }
  event(action, revision = this.revision, generation = this.generation, payload = null) {
    if (generation !== this.generation || revision !== this.revision || this.closed) return false;
    validatePayload(payload);
    this.send({ kind: 'event', action, revision, payload });
    return true;
  }
  command(command) { this.send({ kind: 'command', command }); }
  debugEvaluate(expression) {
    if (!this.options.debug || this.closed) return Promise.reject(new Error('Debug evaluation unavailable'));
    if (typeof expression !== 'string' || expression.length > 16384 || this.debugRequests.size >= 4) return Promise.reject(new Error('Debug request exceeds limit'));
    return new Promise((resolve, reject) => {
      const id = ++this.debugSequence;
      const timer = setTimeout(() => { this.debugRequests.delete(id); reject(new Error('Debug request timed out')); }, 3000);
      this.debugRequests.set(id, message => { clearTimeout(timer); message.error ? reject(new Error(message.error)) : resolve(message.value); });
      this.send({ kind: 'debug-evaluate', id, expression });
    });
  }
  decide(capability, allow) {
    if (!this.options.requested.includes(capability)) return;
    if (allow) this.grants.add(capability); else this.grants.delete(capability);
    this.permissionWaiters.get(capability)?.(allow);
    this.permissionWaiters.delete(capability);
  }
  async permitted(capability) {
    if (!this.options.requested.includes(capability)) throw new Error(`Permission denied: ${capability} was not declared`);
    if (this.grants.has(capability)) return;
    // One prompt per capability; concurrent requests are denied rather than hidden behind it.
    if (this.permissionWaiters.has(capability)) throw new Error(`Permission pending: ${capability}`);
    const allow = await new Promise(resolve => {
      const timer = setTimeout(() => { this.permissionWaiters.delete(capability); resolve(false); }, 30000);
      this.permissionWaiters.set(capability, value => { clearTimeout(timer); resolve(value); });
      this.emit('permission', { capability });
    });
    if (!allow || this.closed) throw new Error(`Permission denied: ${capability}`);
  }
  async handleRequest(message) {
    const { id, capability, args } = message;
    if (!Number.isSafeInteger(id) || this.inflight.has(id) || this.inflight.size >= 64) { this.fail('Invalid or excessive host requests'); return; }
    this.inflight.add(id);
    try {
      await this.permitted(capability);
      if (this.closed) throw new Error('Session disposed');
      const value = await this.capability(capability, args);
      this.send({ kind: 'response', id, value });
    } catch (error) { this.send({ kind: 'response', id, error: error.message }); }
    finally { this.inflight.delete(id); }
  }
  capability(capability, args) {
    const operation = this.performCapability(capability, args);
    this.operations.add(operation);
    operation.finally(() => this.operations.delete(operation)).catch(() => {});
    return operation;
  }
  async performCapability(capability, args) {
    if (capability === 'fs.read') {
      const path = await containedFile(this.options.root, args.path);
      const handle = await open(path, 'r');
      try {
        if (!(await handle.stat()).isFile()) throw new Error('Only regular files can be read');
        const buffer = Buffer.alloc(MAX_MESSAGE + 1);
        const { bytesRead } = await handle.read(buffer, 0, buffer.length, 0);
        if (bytesRead > MAX_MESSAGE) throw new Error('File exceeds read limit');
        return buffer.subarray(0, bytesRead).toString('utf8');
      } finally { await handle.close(); }
    }
    if (capability === 'storage') {
      if (!this.options.storageRoot) throw new Error('Storage unavailable');
      if (typeof args.key !== 'string' || !/^[a-zA-Z0-9_-]{1,80}$/.test(args.key)) throw new Error('Invalid storage key');
      await mkdir(this.options.storageRoot, { recursive: true, mode: 0o700 });
      const path = resolve(this.options.storageRoot, `${args.key}.json`);
      if (args.op === 'get') {
        try { return JSON.parse(await readFile(path, 'utf8')); } catch (error) { if (error.code === 'ENOENT') return null; throw error; }
      }
      if (args.op !== 'set') throw new Error('Unsupported storage operation');
      const data = JSON.stringify(args.value);
      if (typeof data !== 'string' || Buffer.byteLength(data) > 65536) throw new Error('Storage value exceeds limit');
      const write = this.storageWrites.then(async () => {
        if (this.closed) throw new Error('Session disposed');
        let total = Buffer.byteLength(data), count = 1;
        for (const name of await readdir(this.options.storageRoot)) {
          const existing = resolve(this.options.storageRoot, name);
          if (existing === path) continue;
          total += (await stat(existing)).size; count++;
        }
        if (total > 1024 * 1024 || count > 128) throw new Error('Storage quota exceeded');
        const temporary = `${path}.${randomUUID()}.tmp`;
        try {
          await writeFile(temporary, data, { mode: 0o600, flag: 'wx' });
          await rename(temporary, path);
        } finally { await rm(temporary, { force: true }); }
        return null;
      });
      this.storageWrites = write.catch(() => {});
      return write;
    }
    if (capability === 'network') {
      const url = new URL(args.url);
      if (url.protocol !== 'https:' || url.username || url.password || !this.options.origins.includes(url.origin)) throw new Error('Network origin unavailable or denied');
      const controller = new AbortController();
      this.aborts.add(controller);
      const timer = setTimeout(() => controller.abort(), 5000);
      try {
        const response = await fetch(url, { signal: controller.signal, redirect: 'error' });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const chunks = []; let size = 0;
        for await (const chunk of response.body) { size += chunk.length; if (size > MAX_MESSAGE) throw new Error('Response exceeds limit'); chunks.push(chunk); }
        return Buffer.concat(chunks).toString('utf8');
      } finally { clearTimeout(timer); controller.abort(); this.aborts.delete(controller); }
    }
    if (capability === 'process') {
      const executable = Object.hasOwn(this.options.executables, args.command) && this.options.executables[args.command];
      if (typeof executable !== 'string' || !isAbsolute(executable) || !Array.isArray(args.args) || args.args.length > 32 || args.args.some(a => typeof a !== 'string' || a.length > 4096)) throw new Error('Process command unavailable or denied');
      const controller = new AbortController();
      this.aborts.add(controller);
      let isolation, afterSpawn;
      try {
        // Granted binaries do not escape the worker boundary or leave descendants.
        // Unsupported OS backends refuse rather than run with host privileges.
        isolation = await createSandbox(nativeBackend, await realpath(this.options.root), runtimeRoot, { executable, node: false, launcher: this.options.sandboxLauncher });
        controller.signal.throwIfAborted();
        return await new Promise((resolve, reject) => {
          const child = spawn(isolation.execPath, [...isolation.execArgv, ...args.args], { env: {}, stdio: isolation.stdio });
          const chunks = []; let bytes = 0, failure;
          const abort = () => { failure = new Error('Process aborted'); child.kill('SIGKILL'); };
          controller.signal.addEventListener('abort', abort, { once: true });
          const timer = setTimeout(abort, 5000);
          child.stdin.end();
          for (const stream of [child.stdout, child.stderr]) stream.on('data', chunk => {
            bytes += chunk.length;
            if (bytes > MAX_MESSAGE) { failure = new Error('Process output exceeds limit'); child.kill('SIGKILL'); }
            else if (stream === child.stdout) chunks.push(chunk);
          });
          child.on('error', error => { failure = error; });
          child.on('close', code => {
            clearTimeout(timer); controller.signal.removeEventListener('abort', abort);
            if (failure || code !== 0) reject(failure ?? new Error(`Process exited ${code}`));
            else resolve(Buffer.concat(chunks).toString('utf8'));
          });
          afterSpawn = isolation.afterSpawn?.().catch(error => { failure = error; child.kill('SIGKILL'); });
        });
      } finally { await (afterSpawn ?? isolation?.afterSpawn?.()); await isolation?.cleanup?.(); this.aborts.delete(controller); }
    }
    throw new Error(`Unsupported capability: ${capability}`);
  }
  cancelRequests() {
    for (const finish of this.debugRequests.values()) finish({ error: 'Session disposed' });
    this.debugRequests.clear();
    for (const abort of this.aborts) abort.abort();
    this.aborts.clear();
    for (const resolve of this.permissionWaiters.values()) resolve(false);
    this.permissionWaiters.clear();
  }
  fail(message) {
    if (this.closed) return;
    this.emit('fault', { message });
    this.closed = true;
    clearInterval(this.watchdog);
    this.cancelRequests();
    this.child?.kill('SIGKILL');
  }
  async stop() {
    this.cancelRequests();
    if (!this.child?.pid || this.child.exitCode !== null || this.child.signalCode !== null) {
      this.closed = true; await this.starting?.catch(() => {}); await this.exitPromise;
      await Promise.allSettled([...this.operations]); return;
    }
    this.send({ kind: 'dispose' });
    this.closed = true;
    clearInterval(this.watchdog);
    this.cancelRequests();
    const timer = setTimeout(() => this.child.kill('SIGKILL'), 250);
    try { await this.exitPromise; } finally { clearTimeout(timer); }
    await Promise.allSettled([...this.operations]);
  }
}
