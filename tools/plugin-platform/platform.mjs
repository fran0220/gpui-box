import { EventEmitter } from 'node:events';
import { createHash, randomUUID } from 'node:crypto';
import { mkdir, readFile, writeFile, rename, rm, readdir, lstat } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { Session } from '../js-runtime/session.mjs';
import { nativeBackend } from '../js-runtime/sandbox.mjs';

const idPattern = /^[a-z][a-z0-9-]{0,63}$/;
const versionPattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const capabilities = ['fs.read', 'storage', 'network', 'process', 'clipboard.read', 'clipboard.write'];
function object(value, keys) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).some(k => !keys.includes(k))) throw new Error('Invalid manifest fields');
}
export function safePath(path) {
  if (typeof path !== 'string' || path.length > 240 || !/^[a-zA-Z0-9_./-]+$/.test(path) || path.split('/').some(p => !p || p === '.' || p === '..' || p.startsWith('.')))
    throw new Error('Invalid bundle path');
  return path;
}
export function validateManifest(manifest) {
  object(manifest, ['schema', 'id', 'version', 'entry', 'permissions', 'dependencies', 'contributes']);
  if (manifest.schema !== 1 || !idPattern.test(manifest.id ?? '') || !versionPattern.test(manifest.version ?? '')) throw new Error('Invalid manifest identity/version');
  safePath(manifest.entry);
  if (!/\.(mjs|js|ts|mts)$/.test(manifest.entry)) throw new Error('Unsupported entry extension');
  if (!Array.isArray(manifest.permissions) || manifest.permissions.some(p => !capabilities.includes(p)) || new Set(manifest.permissions).size !== manifest.permissions.length) throw new Error('Invalid permissions');
  object(manifest.dependencies, Object.keys(manifest.dependencies ?? {}));
  if (Object.entries(manifest.dependencies).some(([id, version]) => !idPattern.test(id) || id === manifest.id || !versionPattern.test(version))) throw new Error('Dependencies require distinct ids and exact versions');
  object(manifest.contributes, ['commands', 'keymaps', 'panels']);
  const seen = new Set();
  for (const kind of ['commands', 'panels']) {
    if (!Array.isArray(manifest.contributes[kind])) throw new Error(`Missing ${kind}`);
    for (const item of manifest.contributes[kind]) {
      object(item, ['id', 'title']);
      if (!idPattern.test(item.id ?? '') || typeof item.title !== 'string' || !item.title.length || item.title.length > 120 || seen.has(item.id)) throw new Error('Invalid or duplicate contribution');
      seen.add(item.id);
    }
  }
  if (!Array.isArray(manifest.contributes.keymaps)) throw new Error('Missing keymaps');
  const keys = new Set();
  for (const item of manifest.contributes.keymaps) {
    object(item, ['key', 'command']);
    if (typeof item.key !== 'string' || !/^(ctrl|alt|cmd)-[a-z0-9]$/.test(item.key) || keys.has(item.key) || !manifest.contributes.commands.some(c => c.id === item.command)) throw new Error('Invalid keymap');
    keys.add(item.key);
  }
  return manifest;
}
export function validateBundle(bundle) {
  object(bundle, ['manifest', 'files', 'sha256']);
  validateManifest(bundle.manifest);
  object(bundle.files, Object.keys(bundle.files ?? {}));
  if (Object.keys(bundle.files).length > 256 || !Object.hasOwn(bundle.files, bundle.manifest.entry)) throw new Error('Missing entry or too many files');
  for (const [path, content] of Object.entries(bundle.files)) {
    safePath(path);
    if (typeof content !== 'string') throw new Error('Only UTF-8 text files are supported');
  }
  const data = JSON.stringify({ manifest: bundle.manifest, files: bundle.files });
  if (Buffer.byteLength(data) > 4 * 1024 * 1024) throw new Error('Bundle exceeds 4 MiB limit');
  const hash = createHash('sha256').update(data).digest('hex');
  if (bundle.sha256 !== hash) throw new Error('Bundle integrity mismatch');
  return bundle;
}
export async function bundleDirectory(root, manifest) {
  validateManifest(manifest);
  const files = {};
  async function visit(path = '') {
    for (const name of (await readdir(resolve(root, path))).sort()) {
      if (name.startsWith('.') || name === 'node_modules') continue;
      const relative = safePath(path ? `${path}/${name}` : name);
      const stat = await lstat(resolve(root, relative));
      if (stat.isSymbolicLink()) throw new Error('Symlinks are not allowed in bundles');
      if (stat.isDirectory()) await visit(relative);
      else if (stat.isFile()) files[relative] = await readFile(resolve(root, relative), 'utf8');
      else throw new Error('Only regular files are allowed in bundles');
    }
  }
  await visit();
  const data = JSON.stringify({ manifest, files });
  return validateBundle({ manifest, files, sha256: createHash('sha256').update(data).digest('hex') });
}

/** Offline, single-writer plugin store. Installing bytes never executes them. */
export class PluginPlatform extends EventEmitter {
  constructor(root) { super(); this.root = resolve(root); this.active = new Map(); this.mutations = Promise.resolve(); }
  mutate(operation) {
    const result = this.mutations.then(async () => {
      await mkdir(this.root, { recursive: true, mode: 0o700 });
      const lock = resolve(this.root, '.mutation-lock');
      try { await mkdir(lock); } catch (error) {
        if (error.code === 'EEXIST') throw new Error('Plugin store is busy; concurrent writers are refused');
        throw error;
      }
      try { return await operation(); } finally { await rm(lock, { recursive: true }); }
    });
    this.mutations = result.catch(() => {});
    return result;
  }
  async registry() {
    try { return JSON.parse(await readFile(resolve(this.root, 'registry.json'), 'utf8')); }
    catch (error) { if (error.code === 'ENOENT') return {}; throw error; }
  }
  async save(registry) {
    await mkdir(this.root, { recursive: true, mode: 0o700 });
    const path = resolve(this.root, `registry-${randomUUID()}.tmp`);
    await writeFile(path, JSON.stringify(registry, null, 2), { flag: 'wx', mode: 0o600 });
    await rename(path, resolve(this.root, 'registry.json'));
  }
  async install(bundle) { validateBundle(bundle); return this.mutate(() => this.installVersion(bundle)); }
  async installVersion(bundle) {
    validateBundle(bundle);
    const { id, version } = bundle.manifest;
    const registry = await this.registry();
    const destination = resolve(this.root, 'packages', id, version);
    await mkdir(dirname(destination), { recursive: true, mode: 0o700 });
    const staging = `${destination}-${randomUUID()}.stage`;
    try {
      await mkdir(staging, { mode: 0o700 });
      for (const [path, content] of Object.entries(bundle.files)) {
        const target = resolve(staging, safePath(path));
        await mkdir(dirname(target), { recursive: true });
        await writeFile(target, content, { flag: 'wx', mode: 0o600 });
      }
      await writeFile(resolve(staging, '.receipt.json'), JSON.stringify(bundle));
      await rename(staging, destination); // Existing versions are immutable; never overwrite.
      if (!Object.hasOwn(registry, id)) registry[id] = { current: version, previous: null, versions: [] };
      if (!registry[id].versions.includes(version)) registry[id].versions.push(version);
      await this.save(registry);
    } finally { await rm(staging, { recursive: true, force: true }); }
    return { id, version };
  }
  async receipt(id, version) {
    if (!idPattern.test(id) || !versionPattern.test(version)) throw new Error('Invalid plugin identity');
    const bundle = validateBundle(JSON.parse(await readFile(resolve(this.root, 'packages', id, version, '.receipt.json'), 'utf8')));
    if (bundle.manifest.id !== id || bundle.manifest.version !== version) throw new Error('Receipt identity mismatch');
    // Detect accidental edits before trusting installed code. This is integrity, not authenticity.
    for (const [path, content] of Object.entries(bundle.files)) {
      if (await readFile(resolve(this.root, 'packages', id, version, path), 'utf8') !== content) throw new Error('Installed content changed');
    }
    return bundle;
  }
  async discover() {
    const result = [];
    for (const [id, record] of Object.entries(await this.registry())) {
      try { result.push({ ...(await this.receipt(id, record.current)).manifest, enabled: this.active.has(id), versions: record.versions }); }
      catch (error) { result.push({ id, error: error.message, enabled: false }); }
    }
    return result;
  }
  enable(id, options) { return this.mutate(() => this.activate(id, options)); }
  async activate(id, { version, trusted = false, sandbox } = {}) {
    if (!trusted && (!sandbox || sandbox !== nativeBackend)) throw new Error('Untrusted execution unavailable: explicit code trust or a native OS sandbox required');
    const registry = await this.registry();
    version ??= registry[id]?.current;
    const { manifest } = await this.receipt(id, version);
    const previousRegistry = structuredClone(registry);
    if (!this.active.has(id) && this.active.size >= 8) throw new Error('At most 8 plugins may be active');
    for (const [dependency, expected] of Object.entries(manifest.dependencies)) {
      if (this.active.get(dependency)?.manifest.version !== expected) throw new Error(`Enable dependency ${dependency}@${expected} first`);
    }
    for (const [otherId, active] of this.active) {
      if (otherId === id) continue;
      if (Object.hasOwn(active.manifest.dependencies, id) && active.manifest.dependencies[id] !== version) throw new Error(`Upgrade would break dependent ${otherId}`);
      if (active.manifest.contributes.keymaps.some(a => manifest.contributes.keymaps.some(b => a.key === b.key))) throw new Error('Keymap conflict');
    }
    const session = new Session({ root: resolve(this.root, 'packages', id, version), entry: manifest.entry, trusted, sandbox,
      requested: manifest.permissions, storageRoot: resolve(this.root, 'storage', id) });
    session.on('error', message => this.emit('diagnostic', { id, ...message }));
    session.on('log', message => this.emit('diagnostic', { id, ...message }));
    session.on('fault', message => this.emit('diagnostic', { id, ...message }));
    let activated = false;
    session.on('render', message => { if (activated) this.emit('render', { id, ...message }); });
    session.on('permission', message => {
      if (activated) this.emit('permission', { id, generation: session.generation, ...message });
      else session.decide(message.capability, false); // Activation cannot silently acquire capabilities.
    });
    session.on('exit', () => {
      if (this.active.get(id)?.session !== session) return;
      this.active.delete(id);
      this.emit('changed');
      // Dependents lose their prerequisite, so remove them as well.
      for (const [dependent, active] of this.active) if (Object.hasOwn(active.manifest.dependencies, id)) void this.disable(dependent);
    });
    try {
      await new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('Plugin activation timed out')), 3000);
        const finish = callback => value => { clearTimeout(timer); callback(value); };
        session.once('ready', finish(resolve));
        session.once('fault', finish(message => reject(new Error(message.message))));
        session.once('exit', finish(() => reject(new Error('Plugin crashed during activation'))));
        session.start().catch(finish(reject));
      });
      const old = this.active.get(id);
      if (session.closed) throw new Error('Plugin exited before activation commit');
      if (registry[id].current !== version) { registry[id].previous = registry[id].current; registry[id].current = version; }
      await this.save(registry);
      if (session.closed) {
        await this.save(previousRegistry);
        throw new Error('Plugin exited during activation commit');
      }
      this.active.set(id, { manifest, session });
      activated = true;
      if (session.tree) this.emit('render', { id, kind: 'render', generation: session.generation, revision: session.revision, tree: session.tree });
      if (old) await old.session.stop();
      this.emit('changed');
      return session;
    } catch (error) { await session.stop(); throw error; }
  }
  disable(id) { return this.mutate(() => this.deactivate(id)); }
  async deactivate(id) {
    for (const [dependent, active] of [...this.active]) if (Object.hasOwn(active.manifest.dependencies, id)) await this.deactivate(dependent);
    const active = this.active.get(id);
    this.active.delete(id);
    if (active) await active.session.stop();
    this.emit('changed');
  }
  rollback(id, options) {
    return this.mutate(async () => {
      const registry = await this.registry();
      if (!registry[id]?.previous) throw new Error('No previous version');
      return this.activate(id, { ...options, version: registry[id].previous });
    });
  }
  command(id, command) {
    const active = this.active.get(id);
    if (!active?.manifest.contributes.commands.some(c => c.id === command)) return false;
    active.session.command(command); return true;
  }
  key(key) {
    for (const [id, { manifest }] of this.active) {
      const binding = manifest.contributes.keymaps.find(k => k.key === key);
      if (binding) return this.command(id, binding.command);
    }
    return false;
  }
  async close() { for (const id of [...this.active.keys()]) await this.disable(id); }
}
