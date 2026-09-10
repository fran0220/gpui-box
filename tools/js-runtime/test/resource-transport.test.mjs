import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { once } from 'node:events';
import { spawn } from 'node:child_process';
import { Session } from '../session.mjs';
import { nativeBackend } from '../sandbox.mjs';
import { readFrames, encodeFrame } from '../wire.mjs';
import { validateResourceRegistration } from '../resource-schema.mjs';
import { validateManifest, bundleDirectory } from '../../plugin-platform/platform.mjs';

const registration = { key: 'pixels', mime: 'application/octet-stream', data: Buffer.alloc(24 * 1024, 7).toString('base64') };
const event = (session, name) => once(session, name, { signal: AbortSignal.timeout(7000) }).then(([value]) => value);
async function worker(t, requested = ['resources']) {
  const root = await mkdtemp(resolve(tmpdir(), 'gpui-resource-'));
  await writeFile(resolve(root, 'app.mjs'), `
    gpui.mount(() => gpui.text('root', 'mounted'));
    gpui.command('register', async () => {
      try { console.log(JSON.stringify(await gpui.resources.register(${JSON.stringify(registration)}))); }
      catch (error) { console.log(error.message); }
    });
  `);
  const session = new Session({ root, entry: 'app.mjs', sandbox: nativeBackend, requested });
  t.after(async () => { await session.stop(); await rm(root, { recursive: true, force: true }); });
  const ready = event(session, 'ready'); await session.start(); await ready;
  return session;
}

test('isolated mounted worker registers bytes above event cap only after explicit consent', async t => {
  const session = await worker(t);
  let approved = false;
  session.on('permission', ({ capability }) => {
    assert.equal(capability, 'resources'); approved = true; session.decide(capability, true);
  });
  session.on('register-resource', request => {
    assert.ok(approved);
    assert.deepEqual(validateResourceRegistration(request.registration), registration);
    assert.equal(request.generation, session.generation);
    assert.ok(request.deadline > Date.now() && request.deadline <= Date.now() + 3000);
    assert.equal(session.finishResource(request.id, request.generation + 1, { key: 'pixels' }), false);
    assert.equal(session.finishResource(request.id, request.generation, { key: 'pixels' }), true);
  });
  const result = event(session, 'log'); session.command('register');
  assert.equal((await result).message, `'{"key":"pixels"}'`);
});

test('undeclared, denied, and missing native host registrations fail explicitly', async t => {
  for (const mode of ['undeclared', 'denied', 'unavailable']) {
    const session = await worker(t, mode === 'undeclared' ? [] : ['resources']);
    session.on('permission', ({ capability }) => session.decide(capability, mode === 'unavailable'));
    const result = event(session, 'log'); session.command('register');
    assert.match((await result).message, mode === 'undeclared' ? /not declared/ : mode === 'denied' ? /Permission denied/ : /unavailable/);
  }
});

test('mismatched keys, timeout, and late replies cannot resolve registrations', async t => {
  const session = await worker(t);
  session.on('permission', ({ capability }) => session.decide(capability, true));
  let request;
  session.on('register-resource', value => { request = value; });
  let dispatched = event(session, 'register-resource');
  let result = event(session, 'log'); session.command('register'); await dispatched;
  session.finishResource(request.id, request.generation, { key: 'wrong' });
  assert.match((await result).message, /key mismatch/);
  assert.equal(session.finishResource(request.id, request.generation, { key: 'pixels' }), false);
  dispatched = event(session, 'register-resource');
  result = event(session, 'log'); session.command('register'); await dispatched;
  assert.match((await result).message, /timed out/);
  assert.equal(session.finishResource(request.id, request.generation, { key: 'pixels' }), false);
});

test('four native registrations are bounded and disposal cancels all', async t => {
  const session = await worker(t);
  session.decide('resources', true);
  session.on('register-resource', () => {});
  const pending = Array.from({ length: 4 }, () => assert.rejects(session.capability('resources', registration), /disposed/));
  await assert.rejects(session.capability('resources', registration), /limit/);
  assert.equal(session.resourceRequests.size, 4);
  await session.stop(); await Promise.all(pending);
  assert.equal(session.resourceRequests.size, 0);
  assert.equal(session.finishResource(1, session.generation, { key: 'pixels' }), false);
});

test('packaged activation disposal and revoke-between-reply-and-continuation retain no stale work', async t => {
  const session=await worker(t);
  session.decide('resources',true);
  session.on('register-resource', request=>{
    session.finishResource(request.id,request.generation,{key:request.registration.key});
    session.decide('resources',false);
    session.decide('resources',true);
  });
  assert.equal(await session.activatePackagedResources([registration]),false);
  assert.equal(session.registeredAssets.size,0,'resolved old epoch cannot mark new epoch ready');
  session.removeAllListeners('register-resource');
  session.on('register-resource',()=>{});
  const pending=session.activatePackagedResources();
  await Promise.resolve();await Promise.resolve();
  await session.stop();
  assert.equal(await pending,false);
  assert.equal(session.resourceRequests.size,0);
  assert.equal(session.operations.size,0);
  assert.equal(session.permissionWaiters.size,0);
  assert.equal(session.packagedResources.length,0);
  assert.equal(await session.activatePackagedResources([registration]),false);
});

test('asset declarations are closed, consent-required and copied by validated directory bundle', async t => {
  const manifest = { schema: 1, id: 'assets', version: '1.0.0', entry: 'app.mjs', permissions: ['resources'], dependencies: {}, contributes: { commands: [], panels: [], keymaps: [] }, assets: [{ key: 'bytes', path: 'asset.txt', mime: 'application/octet-stream' }] };
  assert.equal(validateManifest(manifest), manifest);
  assert.throws(() => validateManifest({ ...manifest, permissions: [] }), /resources permission/);
  assert.throws(() => validateManifest({ ...manifest, assets: [{ ...manifest.assets[0], extra: true }] }), /Invalid resource object/);
  const root = await mkdtemp(resolve(tmpdir(), 'gpui-assets-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  await writeFile(resolve(root, 'app.mjs'), '');
  await assert.rejects(bundleDirectory(root, manifest), { code: 'ENOENT' });
  await writeFile(resolve(root, 'asset.txt'), 'asset bytes');
  assert.deepEqual((await bundleDirectory(root, manifest)).files['asset.txt'], { base64: Buffer.from('asset bytes').toString('base64') });
});

test('supervisor publishes grant revision before routing native resource envelope and correlates response', async t => {
  const root = await mkdtemp(resolve(tmpdir(), 'gpui-resource-host-'));
  const manifest = { schema: 1, id: 'resources', version: '1.0.0', entry: 'app.mjs', permissions: ['resources'], dependencies: {}, contributes: { commands: [], panels: [], keymaps: [] } };
  await writeFile(resolve(root, 'app.json'), JSON.stringify(manifest));
  await writeFile(resolve(root, 'app.mjs'), `
    const status = gpui.state('waiting');
    gpui.mount(() => gpui.column('root', [gpui.text('status', status.get()), gpui.button('register', 'Register', async () => {
      try { status.set((await gpui.resources.register(${JSON.stringify(registration)})).key); }
      catch (error) { status.set(error.message); }
    })]));
  `);
  const child = spawn(process.execPath, [fileURLToPath(new URL('../../app-host/runner.mjs', import.meta.url)), root, '--data-dir', resolve(root, 'data')], { stdio: ['pipe', 'pipe', 'pipe'] });
  const frames = []; let diagnostic = '';
  child.stderr.on('data', bytes => { diagnostic += bytes; });
  readFrames(child.stdout, frame => frames.push(frame), error => { diagnostic += error.message; });
  const send = frame => child.stdin.write(encodeFrame(frame));
  t.after(async () => {
    if (child.exitCode === null) {
      const exited = once(child, 'exit'); send({ kind: 'close' });
      const timer = setTimeout(() => child.kill('SIGKILL'), 1500);
      await exited; clearTimeout(timer);
    }
    await rm(root, { recursive: true, force: true });
  });
  const wait = async predicate => {
    const deadline = Date.now() + 6000;
    while (Date.now() < deadline) {
      const value = frames.find(predicate); if (value) return value;
      await new Promise(resolve => setTimeout(resolve, 10));
    }
    assert.fail(`Host response timed out: ${diagnostic}`);
  };
  const nodes = tree => [tree, ...(tree?.children ?? []).flatMap(nodes)];
  const mounted = await wait(frame => frame.kind === 'render' && nodes(frame.tree).some(node => node.id.startsWith('app.register.')));
  const button = nodes(mounted.tree).find(node => node.id.startsWith('app.register.'));
  assert.equal(mounted.resources[button.instance], false);
  send({ kind: 'event', generation: 0, revision: mounted.revision, action: button.action });
  const prompt = await wait(frame => frame.kind === 'render' && nodes(frame.tree).some(node => node.id.endsWith('.resources.allow')));
  const allow = nodes(prompt.tree).find(node => node.id.endsWith('.resources.allow'));
  assert.equal(allow.disabled, false);
  assert.equal(frames.some(frame => frame.kind === 'register-resource'), false);
  send({ kind: 'event', generation: 0, revision: prompt.revision, action: allow.action });
  const request = await wait(frame => frame.kind === 'register-resource');
  assert.deepEqual(Object.keys(request).sort(), ['kind', 'id', 'instance', 'revision', 'deadline', 'registration'].sort());
  assert.deepEqual(validateResourceRegistration(request.registration), registration);
  assert.equal(request.instance, button.instance);
  const approved = frames.find(frame => frame.kind === 'render' && frame.revision === request.revision);
  assert.equal(approved.resources[request.instance], true);
  assert.ok(approved.revision > prompt.revision);
  assert.ok(frames.indexOf(approved) < frames.indexOf(request));
  send({ kind: 'resource-response', id: request.id, instance: request.instance, value: { key: registration.key } });
  await wait(frame => frame.kind === 'render' && nodes(frame.tree).some(node => node.id.startsWith('app.status.') && node.text === registration.key));
});
