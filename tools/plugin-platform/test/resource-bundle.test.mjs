import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, writeFile, rm, symlink, link } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { bundleDirectory, bundleFileBytes, validateBundle, PluginPlatform } from '../platform.mjs';
import { RESOURCE_LIMITS, rgbaResource } from '../../js-runtime/resource-schema.mjs';
import { nativeBackend } from '../../js-runtime/sandbox.mjs';

const asset = (key = 'opaque', path = 'assets/data.bin', mime = 'application/octet-stream') => ({ key, path, mime });
const manifest = (assets = []) => ({ schema: 1, id: 'binary-test', version: '1.0.0', entry: 'main.mjs', permissions: ['resources'], dependencies: {}, contributes: { commands: [], panels: [], keymaps: [] }, assets });
const binary = bytes => ({ base64: Buffer.from(bytes).toString('base64') });
function bundle(assets = [asset()], files = { 'assets/data.bin': binary([0, 255, 128, 1]) }) {
  const data = { manifest: manifest(assets), files: { 'main.mjs': '// héllo 世界\n', ...files } };
  return { ...data, sha256: createHash('sha256').update(JSON.stringify(data)).digest('hex') };
}
async function directory(t) {
  const root = await mkdtemp(resolve(tmpdir(), 'resource-bundle-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  return root;
}

test('installed package activation asks after commit, retries only missing assets and revokes on replacement', async t => {
  const root=await mkdtemp(resolve(tmpdir(),'resource-activation-')), p=new PluginPlatform(root);
  t.after(async()=>{await p.close();await rm(root,{recursive:true,force:true});});
  const data={manifest:manifest([asset('first'),asset('second')]),files:{'main.mjs':"gpui.mount(()=>gpui.text('root','Mounted package'));",'assets/data.bin':binary([0,255,17,129])}};
  await p.install({...data,sha256:createHash('sha256').update(JSON.stringify(data)).digest('hex')});
  let allow=false, failSecond=true;
  const requests=[];
  p.on('permission',({id,capability,generation})=>{
    const session=p.active.get(id)?.session;
    assert.equal(session?.generation,generation,'consent only after committed ownership');
    session.decide(capability,allow);
  });
  p.on('register-resource',({plugin,request})=>{
    const session=p.active.get(plugin).session;
    assert.equal(session.grants.has('resources'),true);
    requests.push(request.registration);
    session.finishResource(request.id,request.generation,{key:request.registration.key},failSecond&&request.registration.key==='second'?'Native host refused second':undefined);
  });
  const first=await p.enable('binary-test',{sandbox:nativeBackend});
  await first.assetActivation;
  assert.match(first.assetStatus,/^Unavailable:/);
  assert.deepEqual(requests,[],'denial performs no native registration');
  allow=true;
  assert.equal(await first.activatePackagedResources(),false);
  assert.deepEqual(requests.map(r=>r.key),['first','second']);
  assert.equal(first.registeredAssets.has('first'),true,'partial failure keeps verified first asset');
  failSecond=false;
  assert.equal(await first.activatePackagedResources(),true);
  assert.deepEqual(requests.map(r=>r.key),['first','second','second']);
  assert.equal(requests[0].data,Buffer.from([0,255,17,129]).toString('base64'));
  first.decide('resources',false);
  assert.equal(first.registeredAssets.size,0);
  assert.equal(await first.activatePackagedResources(),true);
  assert.deepEqual(requests.slice(-2).map(r=>r.key),['first','second']);
  const replacement=await p.enable('binary-test',{sandbox:nativeBackend});
  assert.equal(first.closed,true);
  assert.equal(first.packagedResources.length,0);
  assert.equal(first.registeredAssets.size,0);
  await replacement.assetActivation;
  assert.equal(replacement.assetStatus,'Ready');
  assert.notEqual(replacement.generation,first.generation);
  await p.disable('binary-test');
  assert.equal(replacement.operations.size,0);
  assert.equal(replacement.resourceRequests.size,0);
  assert.equal(replacement.packagedResources.length,0);
});

test('asymmetric RGBA and opaque bytes survive directory, JSON digest, install and receipt', async t => {
  const root = await directory(t), store = await directory(t);
  const rgba = rgbaResource('pixels', 2, 1, Uint8Array.from([255, 0, 19, 128, 2, 231, 17, 255]));
  const assets = [asset(), asset('pixels', 'assets/image.rgba', rgba.mime)];
  const expected = { 'assets/data.bin': Buffer.from([0, 255, 254, 128, 1, 13, 10]), 'assets/image.rgba': Buffer.from(rgba.data, 'base64') };
  await mkdir(resolve(root, 'assets'));
  await writeFile(resolve(root, 'main.mjs'), '// héllo 世界\n');
  for (const [path, bytes] of Object.entries(expected)) await writeFile(resolve(root, path), bytes);
  const packed = JSON.parse(JSON.stringify(await bundleDirectory(root, manifest(assets))));
  assert.equal(validateBundle(packed), packed);
  const p = new PluginPlatform(store);
  await p.install(packed);
  assert.deepEqual(await p.receipt('binary-test', '1.0.0'), packed);
  for (const [path, bytes] of Object.entries(expected)) {
    assert.deepEqual(bundleFileBytes(packed.files[path]), bytes);
    assert.deepEqual(await readFile(resolve(store, 'packages/binary-test/1.0.0', path)), bytes);
  }
  const altered = structuredClone(packed);
  altered.files['assets/data.bin'] = binary([0, 254, 254, 128, 1, 13, 10]);
  assert.throws(() => validateBundle(altered), /integrity/);
  // 0xff -> 0xfe would be invisible to a lossy UTF-8 receipt comparison.
  await writeFile(resolve(store, 'packages/binary-test/1.0.0/assets/data.bin'), bundleFileBytes(altered.files['assets/data.bin']));
  await assert.rejects(p.receipt('binary-test', '1.0.0'), /content changed/);
});

test('closed canonical binary encoding, declaration, source and MIME validation', () => {
  for (const content of [{}, { base64: '/w==', extra: true }, { base64: '/x==' }, { base64: '/w' }, { base64: '/w==\n' }, { base64: '' }, { base64: 42 }, [], null]) {
    assert.throws(() => validateBundle(bundle([asset()], { 'assets/data.bin': content })));
    assert.throws(() => bundleFileBytes(content));
  }
  assert.throws(() => validateBundle(bundle([], { 'extra.bin': binary([255]) })), /declared asset/);
  assert.throws(() => validateBundle(bundle([], { 'main.mjs': binary([255]) })), /source entry/);
  assert.throws(() => validateBundle(bundle([asset('source', 'main.mjs')], { 'main.mjs': binary([255]) })), /source entry/);
  assert.throws(() => validateBundle(bundle([asset()], {})), /Missing package asset/);
  assert.throws(() => validateBundle(bundle([asset('pixels', 'assets/data.bin', 'image/x.gpui-rgba8')])), /RGBA/);
  assert.throws(() => validateBundle(bundle([asset('opaque', 'assets/data.bin', 'image/png')])), /MIME/);
});

test('per-file and per-owner quotas apply to binary and legacy string assets', () => {
  for (const content of [binary(Buffer.alloc(RESOURCE_LIMITS.bytes + 1)), 'x'.repeat(RESOURCE_LIMITS.bytes + 1)]) {
    assert.throws(() => validateBundle(bundle([asset()], { 'assets/data.bin': content })), /resource/);
  }
  assert.doesNotThrow(() => validateBundle(bundle([asset()], { 'assets/data.bin': binary(Buffer.alloc(RESOURCE_LIMITS.bytes)) })));
  for (const content of [binary(Buffer.alloc(RESOURCE_LIMITS.bytes)), 'x'.repeat(RESOURCE_LIMITS.bytes)]) {
    const assets = Array.from({ length: 11 }, (_, i) => asset(`key${i}`));
    assert.throws(() => validateBundle(bundle(assets, { 'assets/data.bin': content })), /quota/);
  }
  assert.throws(() => validateBundle(bundle(Array.from({ length: 33 }, (_, i) => asset(`key${i}`)))), /asset count/);
});

test('directory snapshot rejects missing, oversized and aggregate assets plus symlinks/hardlinks', async t => {
  const root = await directory(t);
  await mkdir(resolve(root, 'assets'));
  await writeFile(resolve(root, 'main.mjs'), '// source');
  await assert.rejects(bundleDirectory(root, manifest([asset()])));
  await writeFile(resolve(root, 'assets/data.bin'), Buffer.alloc(RESOURCE_LIMITS.bytes + 1));
  await assert.rejects(bundleDirectory(root, manifest([asset()])), /file\/size/);
  await writeFile(resolve(root, 'assets/data.bin'), Buffer.alloc(RESOURCE_LIMITS.bytes));
  await assert.rejects(bundleDirectory(root, manifest(Array.from({ length: 11 }, (_, i) => asset(`key${i}`)))), /quota/);
  await rm(resolve(root, 'assets/data.bin'));
  await symlink(resolve(root, 'main.mjs'), resolve(root, 'assets/data.bin'));
  await assert.rejects(bundleDirectory(root, manifest([asset()])), /symlinks/);
  await assert.rejects(bundleDirectory(root, manifest()), /Symlinks/);
  await rm(resolve(root, 'assets/data.bin'));
  await link(resolve(root, 'main.mjs'), resolve(root, 'assets/data.bin'));
  await assert.rejects(bundleDirectory(root, manifest([asset()])), /file\/size/);
  await assert.rejects(bundleDirectory(root, manifest()), /Hardlinks/);
});

test('existing UTF-8 files and string assets retain their digest and installed bytes', async t => {
  const packed = bundle([asset()], { 'assets/data.bin': 'héllo\u0000世界' });
  assert.equal(bundleFileBytes(packed.files['assets/data.bin']), 'héllo\u0000世界');
  const p = new PluginPlatform(await directory(t));
  await p.install(packed);
  assert.deepEqual(await p.receipt('binary-test', '1.0.0'), packed);
  const legacy = bundle([], {});
  delete legacy.manifest.assets;
  legacy.sha256 = createHash('sha256').update(JSON.stringify({ manifest: legacy.manifest, files: legacy.files })).digest('hex');
  assert.equal(validateBundle(legacy), legacy);
});
