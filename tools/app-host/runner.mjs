import { readFile, mkdir } from 'node:fs/promises';
import { watch } from 'node:fs';
import { resolve } from 'node:path';
import { homedir } from 'node:os';
import { createHash } from 'node:crypto';
import { Session } from '../js-runtime/session.mjs';
import { readFrames, encodeFrame, MAX_MESSAGE, validatePayload } from '../js-runtime/wire.mjs';
import { PluginPlatform, validateManifest } from '../plugin-platform/platform.mjs';
import { startDebug } from './debug.mjs';
import { nativeBackend } from '../js-runtime/sandbox.mjs';

const args = process.argv.slice(2);
const root = resolve(args[0]);
const flag = name => args.includes(name);
const value = name => { const i = args.indexOf(name); return i < 0 ? undefined : args[i + 1]; };
if (value('--sandbox-launcher')) process.env.GPUI_SANDBOX_LAUNCHER = resolve(value('--sandbox-launcher'));
const data = value('--data-dir') ?? resolve(homedir(), '.local/share/gpui-box/apps', createHash('sha256').update(root).digest('hex').slice(0, 16));
const sandbox = flag('--trust-local') ? undefined : nativeBackend;
const platform = new PluginPlatform(resolve(data, 'plugins'));
let app, appFrame, status = 'Loading app…', error = '', revision = 0, callbacks = new Map();
let plugins = [], closing = false, busy = false, pendingReload = false;
const panels = new Map(), permissions = new Map();
let nativeTargets = new Map();
const mounts = new Map();
let liveMounts = new Set(), mountSequence = 0;
let watcher, debounce, debugServer, lastView;
const output = message => {
  if (process.stdout.writableLength > MAX_MESSAGE) { void shutdown(); return; }
  try { process.stdout.write(encodeFrame(message)); }
  catch {
    callbacks.clear();
    process.stdout.write(encodeFrame({ kind: 'render', generation: 0, revision,
      tree: { kind: 'text', id: 'host.limit', text: 'Unavailable: combined app/plugin view exceeds the host frame budget. Disable large plugins and restart.' } }));
  }
};
const text = (id, text) => ({ kind: 'text', id, text });
const column = (id, children) => ({ kind: 'column', id, children });
const button = (id, text, callback, disabled = false) => {
  if (!disabled) callbacks.set(id, callback);
  return { kind: 'button', id, text, disabled, ...(!disabled ? { action: id } : {}) };
};
function copyTree(node, owner, session, frame) {
  const mountKey = `${owner}:${session.generation}:${node.id}`;
  const type = `${node.kind}:${node.component ?? ''}`;
  let mount = mounts.get(mountKey);
  if (!mount || mount.type !== type) {
    mount = { type, serial: ++mountSequence };
    mounts.set(mountKey, mount);
  }
  liveMounts.add(mountKey);
  // Kit keyed caches use native semantic identity, not EffectOwner. Distinguish
  // generations and remounts even during the framework's cache grace period.
  const id = `${owner}.${node.id}.g${session.generation}.m${mount.serial}`;
  const copied = { ...node, id, instance: session.generation };
  if (node.action && !node.disabled) {
    copied.action = id;
    callbacks.set(id, payload => session.event(node.action, frame.revision, frame.generation, payload));
  }
  if (node.children) copied.children = node.children.map(child => copyTree(child, owner, session, frame));
  if (node.kind === 'kit') {
    const key = `${session.generation}:${node.id}`;
    const targets = nativeTargets.get(key) ?? [];
    targets.push({ id, component: node.component }); nativeTargets.set(key, targets);
    copied.events = {};
    if (!node.props.disabled) for (const [name, action] of Object.entries(node.events)) {
      const routed = `${id}:event:${name}`;
      copied.events[name] = routed;
      callbacks.set(routed, payload => session.event(action, frame.revision, frame.generation, payload));
    }
    copied.slots = Object.fromEntries(Object.entries(node.slots).map(([name, children]) => [name, children.map(child => copyTree(child, owner, session, frame))]));
  }
  return copied;
}
function find(tree, id) {
  if (tree?.id === id) return tree;
  for (const child of [...(tree?.children ?? []), ...Object.values(tree?.slots ?? {}).flat()]) { const result = find(child, id); if (result) return result; }
}
function render() {
  if (closing) return;
  callbacks = new Map();
  nativeTargets = new Map();
  liveMounts = new Set();
  const children = [text('host.title', 'GPUI Box · Native JavaScript app'), text('host.status', status)];
  if (error) children.push(text('host.error', `Error (last verified view retained): ${error.slice(0, 2000)}`));
  children.push(button('host.reload', 'Reload app', () => reload()));
  if (appFrame && app) children.push(copyTree(appFrame.tree, 'app', app, appFrame));
  for (const [key, permission] of permissions) {
    const supported = ['storage', 'fs.read', 'clipboard.read', 'clipboard.write', 'resources'].includes(permission.capability);
    children.push(column(`permission.${key}`, [
      text(`permission.${key}.title`, `${permission.owner} requests ${permission.capability}${supported ? ' for this session' : ' — unavailable in this host'}`),
      button(`permission.${key}.allow`, 'Allow for this session', () => decide(key, true), !supported),
      button(`permission.${key}.deny`, 'Deny', () => decide(key, false)),
    ]));
  }
  children.push(text('host.plugins', 'Installed plugins · OS boundary required; permissions are separate'));
  if (!plugins.length) children.push(text('host.plugins.empty', 'No plugins installed. Use gpui-app plugin-install.'));
  for (const plugin of plugins) {
    const active = platform.active.get(plugin.id);
    const base = `plugin.${plugin.id}`;
    const controls = [text(`${base}.name`, `${plugin.id}@${plugin.version ?? '?'}${active ? ' · enabled' : ' · disabled'}`)];
    if (plugin.error) controls.push(text(`${base}.error`, plugin.error));
    else {
      for (const version of plugin.versions) controls.push(button(`${base}.enable.${version}`, `Enable ${version} (${nativeBackend ?? 'unavailable'} OS backend)`, async () => {
        await platform.enable(plugin.id, { version, sandbox: nativeBackend });
      }));
      controls.push(button(`${base}.disable`, 'Disable', () => platform.disable(plugin.id), !active));
      controls.push(button(`${base}.rollback`, 'Rollback to previous version', () => platform.rollback(plugin.id, { sandbox: nativeBackend })));
      if (active) {
        for (const command of active.manifest.contributes.commands) controls.push(button(`${base}.command.${command.id}`, command.title, () => platform.command(plugin.id, command.id)));
        const frame = panels.get(plugin.id);
        for (const panel of active.manifest.contributes.panels) {
          controls.push(text(`${base}.panel.${panel.id}.title`, panel.title));
          const node = find(frame?.tree, panel.id);
          if (node) controls.push(copyTree(node, `${base}.panel.${panel.id}.view`, active.session, frame));
          else controls.push(text(`${base}.panel.${panel.id}.missing`, 'Unavailable: plugin has not mounted this panel'));
        }
      }
    }
    children.push(column(base, controls));
  }
  const clipboard = Object.fromEntries([app, ...[...platform.active.values()].map(active => active.session)]
    .filter(session => session && !session.closed)
    .map(session => [session.generation, { read: session.grants.has('clipboard.read'), write: session.grants.has('clipboard.write') }]));
  const resources = Object.fromEntries([app, ...[...platform.active.values()].map(active => active.session)]
    .filter(session => session && !session.closed)
    .map(session => [session.generation, session.grants.has('resources')]));
  const tree = column('host.root', children);
  for (const key of mounts.keys()) if (!liveMounts.has(key)) mounts.delete(key);
  const view = JSON.stringify({ tree, clipboard, resources });
  // Discovery and consent completion can converge on the same view. Refresh the
  // callback routes, but do not invalidate a visible native frame without change.
  if (view === lastView) return;
  lastView = view;
  output({ kind: 'render', generation: 0, revision: ++revision, tree, clipboard, resources });
}
function prompt(owner, session, capability) {
  const key = `${session.generation}.${capability}`;
  permissions.set(key, { owner, session, capability }); render();
}
function decide(key, allow) {
  const permission = permissions.get(key); permissions.delete(key);
  permission?.session.decide(permission.capability, allow); render();
}
function clearPermissions(session) {
  for (const [key, permission] of permissions) if (permission.session === session) permissions.delete(key);
}
function registerResource(session, request) {
  const active = session === app || [...platform.active.values()].some(plugin => plugin.session === session);
  const mounted = [...liveMounts].some(key => key.split(':')[1] === String(session.generation));
  if (!active || !mounted || session.closed || !session.grants.has('resources')) {
    session.finishResource(request.id, request.generation, null, 'Native resource registration requires an active mounted permitted generation');
    return;
  }
  // Commit changed grants to the root frame before dispatching native work.
  render();
  output({ kind: 'register-resource', id: request.id, instance: session.generation, revision,
    deadline: request.deadline, registration: request.registration });
}
function invokeNative(session, request) {
  const active = session === app || [...platform.active.values()].some(plugin => plugin.session === session);
  if (active && !session.closed && Object.hasOwn(request.target, '$nativeRef')) {
    output({ kind: 'invoke', id: request.id, instance: session.generation, revision,
      workerRevision: request.revision, target: '', component: '', reference: request.target,
      mode: request.mode, method: request.method, args: request.args, deadline: request.deadline });
    return;
  }
  const targets = nativeTargets.get(`${session.generation}:${request.target.id}`) ?? [];
  if (!active || targets.length !== 1 || targets[0].component !== request.target.component) {
    session.finishNative(request.id, request.revision, null, 'Native target is inactive, invisible, or ambiguous');
    return;
  }
  output({ kind: 'invoke', id: request.id, instance: session.generation, revision,
    workerRevision: request.revision, target: targets[0].id, component: request.target.component,
    mode: request.mode, method: request.method, args: request.args, deadline: request.deadline });
}
function diagnose(message) { error = message; process.stderr.write(message.slice(0, 16384) + '\n'); render(); }
async function refreshPlugins() { plugins = await platform.discover(); render(); }
platform.on('changed', () => {
  for (const [key, permission] of permissions) if (permission.session.closed) permissions.delete(key);
  for (const id of panels.keys()) if (!platform.active.has(id)) panels.delete(id);
  void refreshPlugins().catch(e => diagnose(e.message));
});
platform.on('diagnostic', message => diagnose(`${message.id}: ${message.message}`));
platform.on('invoke', ({ plugin, request }) => {
  const session = platform.active.get(plugin)?.session;
  if (session?.generation === request.generation) invokeNative(session, request);
});
platform.on('register-resource', ({ plugin, request }) => {
  const session = platform.active.get(plugin)?.session;
  if (session?.generation === request.generation) registerResource(session, request);
});
platform.on('render', frame => { panels.set(frame.id, frame); render(); });
platform.on('permission', ({ id, capability, generation }) => {
  const session = platform.active.get(id)?.session;
  if (session?.generation === generation) prompt(id, session, capability);
});

async function reload() {
  if (busy) { pendingReload = true; return; }
  busy = true;
  let candidate;
  try {
    const manifest = validateManifest(JSON.parse(await readFile(resolve(root, 'app.json'), 'utf8')));
    candidate = new Session({ root, entry: manifest.entry, trusted: flag('--trust-local'), sandbox, debug: flag('--debug'),
      requested: manifest.permissions, storageRoot: resolve(data, 'storage') });
    let frame;
    candidate.on('render', next => { frame = next; if (app === candidate) { appFrame = next; render(); } });
    candidate.on('error', message => diagnose(message.message));
    candidate.on('fault', message => diagnose(message.message));
    candidate.on('log', message => process.stderr.write(`[app ${candidate.generation}] ${message.message}\n`));
    candidate.on('invoke', request => invokeNative(candidate, request));
    candidate.on('register-resource', request => registerResource(candidate, request));
    candidate.on('permission', ({ capability }) => prompt(manifest.id, candidate, capability));
    candidate.on('exit', ({ expected }) => {
      clearPermissions(candidate);
      if (app === candidate && !expected) { status = 'App stopped · last verified view retained'; render(); }
    });
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('App activation deadline exceeded')), 35000);
      const done = callback => value => { clearTimeout(timer); callback(value); };
      candidate.once('ready', done(resolve));
      candidate.once('exit', done(() => reject(new Error('App failed to start'))));
      candidate.once('fault', done(message => reject(new Error(message.message))));
      candidate.start().catch(done(reject));
    });
    if (!frame) throw new Error('App must mount a view during activation');
    const old = app; app = candidate; appFrame = frame;
    status = `${manifest.id}@${manifest.version} · ${sandbox ? `${sandbox} OS boundary${sandbox === 'linux' ? '' : ' (native validation pending)'}` : 'trusted local process'} · generation ${app.generation}`;
    error = ''; render();
    if (old) { clearPermissions(old); await old.stop(); }
  } catch (failure) { diagnose(failure.message); clearPermissions(candidate); await candidate?.stop(); }
  finally { busy = false; if (pendingReload && !closing) { pendingReload = false; void reload(); } }
}
async function shutdown() {
  if (closing) return;
  closing = true; clearTimeout(debounce); watcher?.close();
  await app?.stop(); await platform.close(); await debugServer?.close(); process.exit(0);
}
readFrames(process.stdin, message => {
  if (message.kind === 'event' && message.revision === revision && message.generation === 0) {
    const callback = callbacks.get(message.action);
    Promise.resolve().then(() => callback?.(validatePayload(message.payload ?? null))).catch(e => diagnose(e.message));
  } else if (message.kind === 'key') platform.key(message.key);
  else if (message.kind === 'resource-response') {
    const session = [app, ...[...platform.active.values()].map(active => active.session)]
      .find(session => session && !session.closed && session.generation === message.instance);
    session?.finishResource(message.id, message.instance, message.value, message.error);
  }
  else if (message.kind === 'native-response') {
    const session = [app, ...[...platform.active.values()].map(active => active.session)]
      .find(session => session && !session.closed && session.generation === message.instance);
    session?.finishNative(message.id, message.workerRevision, message.value, message.error);
  }
  else if (message.kind === 'native-permission' && ['clipboard.read', 'clipboard.write'].includes(message.capability)) {
    const session = [app, ...[...platform.active.values()].map(active => active.session)]
      .find(session => session && !session.closed && session.generation === message.instance);
    if (session && !session.permissionWaiters.has(message.capability)) void session.permitted(message.capability).catch(error => diagnose(error.message));
  }
  else if (message.kind === 'close') void shutdown();
}, error => { diagnose(error.message); void shutdown(); });
process.stdin.on('end', () => void shutdown());
process.on('SIGTERM', () => void shutdown()); process.on('SIGINT', () => void shutdown());
process.stdout.on('error', () => void shutdown());
await mkdir(data, { recursive: true, mode: 0o700 });
if (flag('--debug')) debugServer = await startDebug(data, expression => {
  if (!app) throw new Error('No active app generation');
  return app.debugEvaluate(expression);
});
await refreshPlugins();
void reload();
if (flag('--dev')) watcher = watch(root, { recursive: true }, (_event, filename) => {
  if (filename?.split('/').some(part => part.startsWith('.') || part === 'node_modules')) return;
  clearTimeout(debounce); debounce = setTimeout(() => void reload(), 150);
});
