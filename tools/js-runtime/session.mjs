import { spawn } from 'node:child_process';
import { EventEmitter } from 'node:events';
import { fileURLToPath } from 'node:url';
import { dirname, resolve, sep, isAbsolute } from 'node:path';
import { realpath, readFile, mkdir, writeFile, rename, open, rm, readdir, stat } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import { validateTree } from './tree.mjs';
import { createSandbox, nativeBackend } from './sandbox.mjs';
import { readFrames, encodeFrame, MAX_MESSAGE, validatePayload } from './wire.mjs';
import { invocationTarget } from './invocation.mjs';
import { predicateTarget, validatePredicatePayload, PREDICATE_LIMIT, PREDICATE_TIMEOUT } from './predicates.mjs';
import { validateResourceRegistration, validateResourceRef } from './resource-schema.mjs';

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
    this.nativeRequests = new Map();
    this.predicateRequests = new Map();
    this.predicateSequence = 0;
    this.resourceRequests = new Map();
    this.resourceSequence = 0;
    this.resourceInflight = new Set();
    this.packagedResources = [];
    this.registeredAssets = new Set();
    this.assetStatus = '';
    this.assetEpoch = 0;
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
      try { await isolation.afterSpawn?.(); }
      finally { await isolation.cleanup?.(); }
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
    // Only close proves that the owned child and its stdio have been reaped.
    // Cleanup stays attached even if a caller's shutdown deadline expires.
    this.closePromise = new Promise(resolve => child.once('close', (code, signal) => {
      this.childClosed = true;
      resolve({ code, signal });
    }));
    this.exitPromise = this.closePromise.then(async ({ code, signal }) => {
      clearInterval(this.watchdog);
      const expected = this.closed; this.closed = true;
      const errors = [];
      try { this.cancelRequests(); } catch (error) { errors.push(error); }
      try { await isolation.cleanup?.(); }
      catch (error) { errors.push(error); }
      try { this.emit('exit', { code, signal, expected }); } catch (error) { errors.push(error); }
      if (errors.length) throw new AggregateError(errors, 'Worker exit cleanup failed');
    });
    // Natural exits may precede stop(); retain the rejection for that caller.
    this.exitPromise.catch(() => {});
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
          this.cancelNative('Native request cancelled by render revision change');
          this.cancelPredicates('Predicate cancelled by render revision change');
          this.revision = message.revision;
          this.tree = message.tree;
        }
        if (message.kind === 'invoke') { this.handleNative(message); return; }
        if (message.kind === 'predicate-response') { this.finishPredicate(message); return; }
        if (message.kind === 'request') { void this.handleRequest(message); return; }
        if (!['render', 'ready', 'log', 'error', 'disposed'].includes(message.kind)) throw new Error('Unknown worker message');
        this.emit(message.kind, message);
      } catch (error) { this.fail(error.message); }
    }, error => this.fail(error.message));
    this.watchdog = setInterval(() => {
      if (Date.now() - lastHeartbeat > this.options.timeoutMs) this.fail('Worker heartbeat deadline exceeded');
    }, 100);
    // Attach all handlers before yielding: a missing backend can fail immediately.
    try { await isolation.afterSpawn?.(); }
    catch (error) { this.fail(error.message); throw error; }
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
  // The app/plugin owner calls this only AFTER committing the mounted session.
  // Inputs are validated package byte snapshots, never guest-supplied paths.
  activatePackagedResources(registrations) {
    if (this.assetActivation) return this.assetActivation;
    if (this.closed) return Promise.resolve(false);
    if (registrations) this.packagedResources = registrations.map(value => Object.freeze({ ...validateResourceRegistration(value) }));
    if (!this.packagedResources.length) return Promise.resolve(true);
    const epoch = this.assetEpoch;
    const operation = Promise.resolve().then(async () => {
      try {
        if (this.closed) throw new Error('Asset activation cancelled');
        this.assetStatus = 'Awaiting permission'; this.emit('assets');
        await this.permitted('resources');
        this.assetStatus = 'Registering'; this.emit('assets');
        for (const registration of this.packagedResources) {
          if (this.closed || epoch !== this.assetEpoch || !this.grants.has('resources')) throw new Error('Asset activation cancelled');
          if (this.registeredAssets.has(registration.key)) continue;
          await this.registerResource(registration);
          if (this.closed || epoch !== this.assetEpoch || !this.grants.has('resources')) throw new Error('Asset activation cancelled');
          this.registeredAssets.add(registration.key);
        }
        this.assetStatus = 'Ready';
        return true;
      } catch (error) {
        this.assetStatus = `Unavailable: ${String(error.message).slice(0, 1024)}`;
        return false;
      } finally { if (!this.closed) this.emit('assets'); }
    });
    this.assetActivation = operation;
    this.operations.add(operation);
    operation.finally(() => { this.operations.delete(operation); if (this.assetActivation === operation) this.assetActivation = undefined; }).catch(() => {});
    return operation;
  }
  registerResource(registration) {
    validateResourceRegistration(registration);
    if (this.closed || !this.tree || !this.grants.has('resources')) throw new Error('Resource registration requires a mounted, permitted session');
    if (this.resourceRequests.size >= 4) throw new Error('Resource request limit exceeded');
    if (!this.listenerCount('register-resource')) throw new Error('Native resource registration unavailable in this host');
    const id = ++this.resourceSequence;
    const deadline = Date.now() + 3000;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => this.finishResource(id, this.generation, null, 'Resource request timed out'), 3000);
      this.resourceRequests.set(id, { key: registration.key, deadline, timer, resolve, reject });
      try { this.emit('register-resource', { id, generation: this.generation, registration, deadline }); }
      catch (error) { this.finishResource(id, this.generation, null, error.message); }
    });
  }
  finishResource(id, generation, value, error) {
    const pending = this.resourceRequests.get(id);
    if (!pending || generation !== this.generation) return false;
    this.resourceRequests.delete(id); clearTimeout(pending.timer);
    try {
      if (error) throw new Error(String(error).slice(0, 2048));
      if (Date.now() >= pending.deadline) throw new Error('Resource request timed out');
      if (this.closed || !this.grants.has('resources')) throw new Error('Resource request cancelled');
      validateResourceRef(value);
      if (value.key !== pending.key) throw new Error('Resource response key mismatch');
      pending.resolve(value);
    } catch (failure) { pending.reject(failure); }
    return true;
  }
  handleNative(message) {
    const { id, revision, target, method, args, mode } = message;
    const reject = error => this.send({ kind: 'native-response', id, revision, error: error.message.slice(0, 2048) });
    try {
      if (!Number.isSafeInteger(id) || id <= 0 || this.nativeRequests.has(id) || this.nativeRequests.size >= 32) throw new Error('Invalid or excessive native requests');
      if (this.closed || revision !== this.revision) throw new Error('Stale native request');
      invocationTarget(this.tree, target, method, args, mode);
      if (!this.listenerCount('invoke')) throw new Error('Native invocation unavailable in this host');
      const timer = setTimeout(() => this.finishNative(id, revision, null, 'Native request timed out'), 3000);
      this.nativeRequests.set(id, { revision, timer });
      this.emit('invoke', { id, generation: this.generation, revision, target: Object.hasOwn(target, '$nativeRef') ? target : { id: target.id, component: target.component }, method, args, mode, deadline: Date.now() + 3000 });
    } catch (error) { reject(error); }
  }
  finishNative(id, revision, value, error) {
    const pending = this.nativeRequests.get(id);
    if (!pending || pending.revision !== revision) return false;
    this.nativeRequests.delete(id); clearTimeout(pending.timer);
    if (revision !== this.revision) error = 'Stale native response';
    try { if (!error) validatePayload(value); } catch (failure) { error = failure.message; }
    this.send({ kind: 'native-response', id, revision, ...(error ? { error: String(error).slice(0, 2048) } : { value }) });
    return true;
  }
  cancelNative(error) {
    for (const [id, pending] of this.nativeRequests) this.finishNative(id, pending.revision, null, error);
  }
  evaluatePredicate({ target, name, reference, payload, deadline }, signal) {
    if (this.closed || signal?.aborted) return Promise.reject(new Error('Predicate cancelled'));
    const now = Date.now();
    if (!Number.isSafeInteger(deadline) || deadline <= now || deadline > now + PREDICATE_TIMEOUT || this.predicateRequests.size >= PREDICATE_LIMIT)
      return Promise.reject(new Error('Predicate deadline or capacity exceeded'));
    try {
      predicateTarget(this.tree, target, name, reference);
      validatePredicatePayload(target.component, payload);
    } catch (error) { return Promise.reject(error); }
    const id = ++this.predicateSequence, revision = this.revision;
    return new Promise((resolve, reject) => {
      const finish = (message) => {
        if (!this.predicateRequests.delete(id)) return;
        clearTimeout(timer);
        signal?.removeEventListener('abort', cancel);
        try {
          if (message.error) throw new Error(String(message.error).slice(0, 2048));
          if (this.closed || this.revision !== revision || message.revision !== revision) throw new Error('Predicate cancelled: stale response');
          predicateTarget(this.tree, target, name, reference);
          if (typeof message.value !== 'boolean') throw new Error('Predicate must return a boolean');
          resolve(message.value);
        } catch (error) {
          this.send({ kind: 'predicate-cancel', id });
          reject(error);
        }
      };
      const cancel = () => finish({ error: 'Predicate cancelled by native host' });
      const timer = setTimeout(() => finish({ error: 'Predicate deadline expired' }), deadline - now);
      this.predicateRequests.set(id, finish);
      signal?.addEventListener('abort', cancel, { once: true });
      this.send({ kind: 'predicate-request', id, revision, target, name, reference, payload, deadline });
    });
  }
  finishPredicate(message) { this.predicateRequests.get(message.id)?.(message); }
  cancelPredicates(reason) {
    for (const finish of this.predicateRequests.values()) finish({ error: reason });
  }
  debugEvaluate(expression, { signal } = {}) {
    if (!this.options.debug || this.closed) return Promise.reject(new Error('Debug evaluation unavailable'));
    if (signal?.aborted) return Promise.reject(new Error('Debug evaluation cancelled'));
    if (typeof expression !== 'string' || expression.length > 16384 || this.debugRequests.size >= 4) return Promise.reject(new Error('Debug request exceeds limit'));
    return new Promise((resolve, reject) => {
      const id = ++this.debugSequence;
      const cleanup = () => { clearTimeout(timer); signal?.removeEventListener('abort', abort); };
      const cancel = reason => {
        if (!this.debugRequests.delete(id)) return;
        cleanup();
        // Inspector cannot undo arbitrary evaluated Promise/timer ownership.
        // Retire the isolated process, and reject only after reaping/cleanup.
        this.fail(`${reason}; reload the app to start a new generation`);
        this.stop().then(() => reject(new Error(reason)), reject);
      };
      const abort = () => cancel('Debug evaluation cancelled');
      const timer = setTimeout(() => cancel('Debug request timed out'), 3000);
      this.debugRequests.set(id, message => {
        if (!this.debugRequests.delete(id)) return;
        cleanup();
        message.error ? reject(new Error(message.error)) : resolve(message.value);
      });
      signal?.addEventListener('abort', abort, { once: true });
      this.send({ kind: 'debug-evaluate', id, expression });
    });
  }
  decide(capability, allow) {
    if (!this.options.requested.includes(capability)) return;
    if (allow) this.grants.add(capability); else this.grants.delete(capability);
    if (capability === 'resources' && !allow) {
      this.assetEpoch++;
      this.registeredAssets.clear();
      for (const id of this.resourceRequests.keys()) this.finishResource(id, this.generation, null, 'Resource permission revoked');
      if (this.packagedResources.length) { this.assetStatus = 'Unavailable: Resource permission denied'; this.emit('assets'); }
    }
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
      if (capability === 'resources') {
        validateResourceRegistration(args);
        if (this.resourceInflight.size >= 4) throw new Error('Resource request limit exceeded');
        this.resourceInflight.add(id);
      }
      await this.permitted(capability);
      if (this.closed) throw new Error('Session disposed');
      const value = await this.capability(capability, args);
      this.send({ kind: 'response', id, value });
    } catch (error) { this.send({ kind: 'response', id, error: error.message }); }
    finally { this.inflight.delete(id); this.resourceInflight.delete(id); }
  }
  capability(capability, args) {
    const operation = this.performCapability(capability, args);
    this.operations.add(operation);
    operation.finally(() => this.operations.delete(operation)).catch(() => {});
    return operation;
  }
  async performCapability(capability, args) {
    if (capability === 'resources') return this.registerResource(args);
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
    this.assetEpoch++;
    this.packagedResources = [];
    this.registeredAssets.clear();
    for (const id of this.resourceRequests.keys()) this.finishResource(id, this.generation, null, 'Session disposed');
    this.cancelNative('Session disposed');
    this.cancelPredicates('Predicate cancelled: session disposed');
    for (const finish of this.debugRequests.values()) finish({ error: 'Session disposed' });
    this.debugRequests.clear();
    for (const abort of this.aborts) abort.abort();
    this.aborts.clear();
    for (const resolve of this.permissionWaiters.values()) resolve(false);
    this.permissionWaiters.clear();
  }
  fail(message) {
    if (this.closed) return;
    this.closed = true;
    clearInterval(this.watchdog);
    try { this.child?.kill('SIGKILL'); }
    catch (error) { (this.lifecycleErrors ??= []).push(error); }
    try { this.emit('fault', { message }); }
    catch (error) { (this.lifecycleErrors ??= []).push(error); }
    // Event callbacks cannot await shutdown. The same promise remains available
    // to the owner, including any reap/cleanup failure.
    this.stop().catch(() => {});
  }
  stop() {
    if (this.stopPromise) return this.stopPromise;
    // One overall budget includes launch, reaping, native staging cleanup
    // (which can take two seconds on Windows), and outstanding operations.
    const deadline = Date.now() + 5000;
    const bounded = async (promise, stage) => {
      let timer;
      try {
        return await Promise.race([promise, new Promise((_, reject) => {
          timer = setTimeout(() => reject(new Error(`Session shutdown deadline exceeded: ${stage}`)), Math.max(0, deadline - Date.now()));
        })]);
      } finally { clearTimeout(timer); }
    };
    // Install the shared promise before synchronous cancellation/listeners can
    // reenter, while still closing admission before stop() returns.
    const { promise, resolve, reject } = Promise.withResolvers();
    this.stopPromise = promise;
    const shutdown = async () => {
      const errors = this.lifecycleErrors ??= [];
      try { this.send({ kind: 'dispose' }); } catch (error) { errors.push(error); }
      this.closed = true;
      clearInterval(this.watchdog);
      try { this.cancelRequests(); } catch (error) { errors.push(error); }
      const timer = setTimeout(() => {
        try { if (!this.childClosed) this.child?.kill('SIGKILL'); } catch (error) { errors.push(error); }
      }, 250);
      try {
        try { await bounded(this.starting, 'launch'); } catch (error) { errors.push(error); }
        if (this.child) {
          // exitCode/signalCode and kill() success are not close/reap proof.
          if (!this.closePromise) errors.push(new Error('Worker close/reap proof unavailable'));
          else {
            try { await bounded(this.closePromise, 'worker close/reap not confirmed'); }
            catch (error) { errors.push(error); }
          }
        }
        try { await bounded(this.exitPromise, 'post-close cleanup'); } catch (error) { errors.push(error); }
        try { await bounded(Promise.allSettled([...this.operations]), 'pending operations'); }
        catch (error) { errors.push(error); }
      } finally { clearTimeout(timer); }
      if (errors.length) throw new AggregateError(errors, 'Session shutdown failed: ' + errors.map(error => error.message).join('; '));
    };
    shutdown().then(resolve, reject);
    return this.stopPromise;
  }
}
