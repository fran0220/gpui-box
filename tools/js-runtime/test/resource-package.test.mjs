import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, symlink, link, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { loadPackagedResources } from '../resource-package.mjs';
import { RESOURCE_LIMITS, rgbaResource, validateAssetDeclarations } from '../resource-schema.mjs';

async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), 'gpui-resources-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(join(root, 'assets'));
  return root;
}
const asset = (key = 'data', path = 'assets/data.bin') => ({ key, path, mime: 'application/octet-stream' });

test('manifest is closed and wholly rejected before filesystem access', async () => {
  for (const path of ['../escape', '/etc/passwd', 'https://example.com/x', 'a\\b', 'a//b', 'a/./b', 'a/../b', 'a\0b', 'C:/x']) assert.throws(() => validateAssetDeclarations([asset('x', path)]));
  for (const declarations of [[asset(), asset()], [{ ...asset(), url: 'file:///x' }], [{ ...asset(), mime: 'image/png' }], Array.from({ length: 33 }, (_, i) => asset(`x${i}`))]) {
    await assert.rejects(loadPackagedResources('/does-not-exist', declarations), error => !error.code);
  }
});

test('approved immutable package bytes are snapshots, not resource authority', async t => {
  const root = await fixture(t);
  const rgba = rgbaResource('pixels', 2, 1, Uint8Array.from([255, 0, 0, 255, 0, 0, 255, 255]));
  await writeFile(join(root, 'assets/image.rgba'), Buffer.from(rgba.data, 'base64'));
  await writeFile(join(root, 'assets/data.bin'), Buffer.from([7, 9, 11]));
  const loaded = await loadPackagedResources(root, [asset(), { key: 'pixels', path: 'assets/image.rgba', mime: rgba.mime }]);
  assert.deepEqual(loaded, [{ key: 'data', mime: 'application/octet-stream', data: 'BwkL' }, rgba]);
  assert.ok(Object.isFrozen(loaded) && loaded.every(Object.isFrozen));
  await writeFile(join(root, 'assets/data.bin'), 'changed');
  assert.equal(loaded[0].data, 'BwkL');
  assert.deepEqual(Object.keys(loaded[0]), ['key', 'mime', 'data']);
});

test('escaping/interior symlinks and hardlinks are refused before reading targets', async t => {
  const root = await fixture(t);
  await writeFile(join(root, 'assets/data.bin'), 'known');
  await symlink('/a-target-that-must-never-be-read', join(root, 'assets/escape'));
  await symlink('data.bin', join(root, 'assets/inside'));
  await symlink('/etc', join(root, 'escape-directory'));
  for (const path of ['assets/escape', 'assets/inside', 'escape-directory/passwd']) await assert.rejects(loadPackagedResources(root, [asset('x', path)]), /symlinks/);
  await link(join(root, 'assets/data.bin'), join(root, 'assets/hardlink'));
  await assert.rejects(loadPackagedResources(root, [asset('x', 'assets/hardlink')]), /file\/size/);
});

test('file, aggregate bytes and concurrency limits are retryable', async t => {
  const root = await fixture(t);
  await writeFile(join(root, 'assets/data.bin'), Buffer.alloc(RESOURCE_LIMITS.bytes + 1));
  await assert.rejects(loadPackagedResources(root, [asset()]), /file\/size/);
  await writeFile(join(root, 'assets/data.bin'), Buffer.alloc(RESOURCE_LIMITS.bytes));
  await assert.rejects(loadPackagedResources(root, Array.from({ length: 11 }, (_, i) => asset(`x${i}`))), /quota/);
  const attempts = await Promise.allSettled(Array.from({ length: 8 }, () => loadPackagedResources(root, [asset()])));
  assert.ok(attempts.some(attempt => attempt.status === 'rejected' && /concurrency/.test(attempt.reason.message)));
  assert.equal((await loadPackagedResources(root, [asset()])).length, 1);
});
