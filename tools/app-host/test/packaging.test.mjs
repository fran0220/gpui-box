import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm, rename, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

test('directory packaging executes its launcher with relocated arguments and native sandbox helper', { skip: process.platform === 'win32' }, async t => {
  const dir = await mkdtemp(resolve(tmpdir(), 'gpui-package-'));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const cli = fileURLToPath(new URL('../cli.mjs', import.meta.url));
  const app = resolve(dir, 'app'), out = resolve(dir, 'package with spaces');
  execFileSync(process.execPath, [cli, 'init', app]);
  const host = resolve(dir, 'argument-recorder');
  const sandbox = resolve(dir, 'sandbox fixture');
  const debug = resolve(dir, 'debug helper fixture.exe');
  // This executable checks generated launcher relocation, not native rendering.
  await writeFile(host, '#!/bin/sh\nprintf "%s\\n" "$GPUI_SANDBOX_LAUNCHER" "$GPUI_DEBUG_PIPE_HELPER" "$@"\n', { mode: 0o755 });
  await writeFile(sandbox, '#!/bin/sh\nexit 0\n', { mode: 0o755 });
  await writeFile(debug, Buffer.from([0x4d, 0x5a, 0, 255])); // Packaging fixture, not an executable/security proof.
  execFileSync(process.execPath, [cli, 'build', app, out, '--host', host, '--sandbox-launcher', sandbox, '--debug-helper', debug]);
  const moved = resolve(dir, 'relocated package');
  await rename(out, moved);
  const lines = execFileSync(resolve(moved, 'run.sh'), ['--data-dir', resolve(dir, 'data with spaces')], { encoding: 'utf8' }).trim().split('\n');
  assert.deepEqual(lines, [resolve(moved, 'runtime/gpui-sandbox-launch'), resolve(moved, 'runtime/gpui-debug-pipe.exe'), resolve(moved, 'tools/app-host/runner.mjs'), resolve(moved, 'app'), '--data-dir', resolve(dir, 'data with spaces')]);
  await rename(moved, out);
  assert.deepEqual(await readFile(resolve(out, 'runtime/gpui-sandbox-launch')), await readFile(sandbox));
  assert.deepEqual(await readFile(resolve(out, 'runtime/gpui-debug-pipe.exe')), await readFile(debug));
  assert.equal(JSON.parse(await readFile(resolve(out, 'build-info.json'), 'utf8')).debugHelper, 'gpui-debug-pipe.exe');
  assert.equal(JSON.parse(await readFile(resolve(out, 'build-info.json'), 'utf8')).runtimeBundled, false);
  assert.throws(() => execFileSync(process.execPath, [cli, 'build', app, out, '--host', host], { stdio: 'pipe' }), /Command failed/);
});

test('debug helper packaging preserves bytes and refuses a missing helper before creating output', async t => {
  const dir = await mkdtemp(resolve(tmpdir(), 'gpui-debug-package-'));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const cli = fileURLToPath(new URL('../cli.mjs', import.meta.url));
  const app = resolve(dir, 'app'), out = resolve(dir, 'package'), helper = resolve(dir, 'helper.exe');
  execFileSync(process.execPath, [cli, 'init', app]);
  assert.throws(() => execFileSync(process.execPath, [cli, 'build', app, out, '--host', process.execPath, '--debug-helper', helper], { stdio: 'pipe' }));
  await assert.rejects(stat(out), { code: 'ENOENT' });
  await writeFile(helper, Buffer.from([77, 90, 0, 128, 255]));
  execFileSync(process.execPath, [cli, 'build', app, out, '--host', process.execPath, '--debug-helper', helper]);
  assert.deepEqual(await readFile(resolve(out, 'runtime/gpui-debug-pipe.exe')), await readFile(helper));
  assert.equal(JSON.parse(await readFile(resolve(out, 'build-info.json'), 'utf8')).debugHelper, 'gpui-debug-pipe.exe');
});
