import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises';
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
  // This executable checks generated launcher relocation, not native rendering.
  await writeFile(host, '#!/bin/sh\nprintf "%s\\n" "$GPUI_SANDBOX_LAUNCHER" "$@"\n', { mode: 0o755 });
  execFileSync(process.execPath, [cli, 'build', app, out, '--host', host, '--sandbox-launcher', '/bin/true']);
  const lines = execFileSync(resolve(out, 'run.sh'), ['--data-dir', resolve(dir, 'data with spaces')], { encoding: 'utf8' }).trim().split('\n');
  assert.deepEqual(lines, [resolve(out, 'runtime/gpui-sandbox-launch'), resolve(out, 'tools/app-host/runner.mjs'), resolve(out, 'app'), '--data-dir', resolve(dir, 'data with spaces')]);
  assert.deepEqual(await readFile(resolve(out, 'runtime/gpui-sandbox-launch')), await readFile('/bin/true'));
  assert.equal(JSON.parse(await readFile(resolve(out, 'build-info.json'), 'utf8')).runtimeBundled, false);
  assert.throws(() => execFileSync(process.execPath, [cli, 'build', app, out, '--host', host], { stdio: 'pipe' }), /Command failed/);
});
