import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, open, readFile, rm, symlink, writeFile, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync, spawn } from 'node:child_process';
import { once } from 'node:events';
import { createServer } from 'node:net';
import { setTimeout as delay } from 'node:timers/promises';
import { windowsSandbox } from '../windows-sandbox.mjs';

const windows = process.platform === 'win32';
const runtime = fileURLToPath(new URL('..', import.meta.url));
const nativeOptions = { skip: !windows, timeout: 30000 };

async function fixture(t) {
  assert.equal(process.version, 'v26.5.1', 'native lane must use pinned Node');
  assert.ok(process.env.GPUI_SANDBOX_LAUNCHER, 'build and set GPUI_SANDBOX_LAUNCHER');
  const directory = await mkdtemp(join(tmpdir(), 'gpui-windows-test-'));
  t.after(() => rm(directory, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 }));
  const root = join(directory, 'package with spaces 中文');
  const minimalRuntime = join(directory, 'runtime');
  await mkdir(root); await mkdir(minimalRuntime);
  const secret = join(directory, 'host-secret');
  await writeFile(secret, 'host-only-sentinel');
  assert.equal(await readFile(secret, 'utf8'), 'host-only-sentinel');
  return { directory, root, minimalRuntime, secret };
}

function launch(t, config, args, extra = {}) {
  const child = spawn(config.execPath, [...config.execArgv, ...args], { stdio: config.stdio, ...extra });
  let stdout = '', stderr = '';
  child.stdout.on('data', chunk => stdout += chunk);
  child.stderr.on('data', chunk => stderr += chunk);
  const closed = once(child, 'close');
  t.after(async () => {
    if (child.exitCode === null && child.signalCode === null) child.kill();
    await closed;
    await config.cleanup();
  });
  return { child, closed, output: () => ({ stdout, stderr }) };
}

async function waitUntil(predicate, message) {
  for (let i = 0; i < 200; i++) {
    if (predicate()) return;
    await delay(25);
  }
  assert.fail(message);
}

function exists(pid) {
  try { process.kill(pid, 0); return true; }
  catch (error) { if (error.code === 'ESRCH') return false; throw error; }
}

function acl(path) {
  return execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
    `(Get-Acl -LiteralPath '${path.replaceAll("'", "''")}').Sddl`], { encoding: 'utf8' }).trim();
}

test('Windows factory refuses execution on another OS', { skip: windows }, async () => {
  await assert.rejects(windowsSandbox('.', '.'), /requires Windows/);
});

test('native AppContainer blocks host reads, writes, network, spawning and leaked handles', nativeOptions, async t => {
  const { root, minimalRuntime, secret } = await fixture(t);
  assert.ok(process.env.GPUI_WINDOWS_SANDBOX_PROBE, 'build and set GPUI_WINDOWS_SANDBOX_PROBE');
  const originals = [root, minimalRuntime, secret, process.env.GPUI_WINDOWS_SANDBOX_PROBE];
  const originalAcls = originals.map(acl);
  const server = createServer(socket => { socket.destroy(); assert.fail('sandbox reached host listener'); });
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  t.after(() => new Promise(resolve => server.close(resolve)));
  const sentinel = await open(secret, 'r');
  t.after(() => sentinel.close());
  const config = await windowsSandbox(root, minimalRuntime, { executable: process.env.GPUI_WINDOWS_SANDBOX_PROBE, node: false });
  const run = launch(t, config, [String(server.address().port), secret], {
    stdio: [...config.stdio, sentinel.fd],
    env: { ...process.env, GPUI_TEST_SECRET: 'must-not-inherit' },
  });
  const [code] = await run.closed;
  assert.equal(code, 0, run.output().stderr);
  assert.deepEqual(JSON.parse(run.output().stdout), {
    appcontainer: true, capabilities: 0, readonly: true, hostDenied: true,
    spawnDenied: true, networkDenied: true, handles: true,
    memory: 268435456, cpuSeconds: 30, cpuRate: 2500, activeProcesses: 1,
  });
  assert.deepEqual(originals.map(acl), originalAcls, 'host ACLs must remain unchanged');
  assert.equal(await readFile(secret, 'utf8'), 'host-only-sentinel');
  await config.cleanup();
  await assert.rejects(access(config.execArgv[1]), { code: 'ENOENT' });
});

test('native committed allocation is refused before 256 MiB', nativeOptions, async t => {
  const { root, minimalRuntime, secret } = await fixture(t);
  assert.ok(process.env.GPUI_WINDOWS_SANDBOX_PROBE);
  const config = await windowsSandbox(root, minimalRuntime, { executable: process.env.GPUI_WINDOWS_SANDBOX_PROBE, node: false });
  const run = launch(t, config, ['memory', secret]);
  assert.equal((await run.closed)[0], 0, run.output().stderr);
  const { allocations } = JSON.parse(run.output().stdout);
  assert.ok(allocations > 0 && allocations < 32, run.output().stdout);
});

test('native infinite loop is terminated by the 30-second user CPU budget', { ...nativeOptions, timeout: 180000 }, async t => {
  const { root, minimalRuntime, secret } = await fixture(t);
  assert.ok(process.env.GPUI_WINDOWS_SANDBOX_PROBE);
  const config = await windowsSandbox(root, minimalRuntime, { executable: process.env.GPUI_WINDOWS_SANDBOX_PROBE, node: false });
  const run = launch(t, config, ['spin', secret]);
  const [code] = await run.closed;
  assert.match(run.output().stdout, /spinning/);
  assert.equal(code >>> 0, 0x718, `expected ERROR_NOT_ENOUGH_QUOTA, got ${code}: ${run.output().stderr}`);
});

test('Node runs with exact argument quoting, mapped read paths, clean environment and read-only package', nativeOptions, async t => {
  const { root, minimalRuntime, secret } = await fixture(t);
  const script = join(root, 'entry.mjs');
  await writeFile(script, `
    import fs from 'node:fs';
    let denied = false;
    try { fs.writeFileSync(new URL('./forbidden', import.meta.url), 'no'); } catch { denied = true; }
    console.log(JSON.stringify({ args: process.argv.slice(2), content: fs.readFileSync(new URL('./data', import.meta.url), 'utf8'), denied, secret: process.env.GPUI_TEST_SECRET ?? null }));
  `);
  await writeFile(join(root, 'data'), 'readable copy');
  const config = await windowsSandbox(root, minimalRuntime);
  const values = ['space value', 'embedded"quote', 'trailing\\', '', secret];
  const run = launch(t, config, [script, ...values], { env: { ...process.env, GPUI_TEST_SECRET: 'host-value' } });
  assert.equal((await run.closed)[0], 0, run.output().stderr);
  assert.deepEqual(JSON.parse(run.output().stdout), { args: values, content: 'readable copy', denied: true, secret: null });
});

test('real runtime worker renders TypeScript and the inspector remains contained', nativeOptions, async t => {
  const { root, secret } = await fixture(t);
  const entry = join(root, 'app.mts');
  await writeFile(entry, "const label: string = 'Windows ready'; gpui.mount(() => gpui.text('result', label));");
  const config = await windowsSandbox(root, runtime);
  const run = launch(t, config, ['--allow-inspector', join(runtime, 'worker.mjs'), entry, '7', 'debug']);
  await waitUntil(() => run.output().stdout.includes('"kind":"ready"') || run.child.exitCode !== null,
    `worker did not become ready: ${JSON.stringify(run.output())}`);
  const frames = () => {
    const output = run.output().stdout;
    return output.slice(0, output.lastIndexOf('\n') + 1).split('\n').filter(Boolean).map(line => JSON.parse(line));
  };
  assert.ok(frames().some(frame => frame.kind === 'render' && frame.generation === 7 && frame.tree.text === 'Windows ready'), JSON.stringify(run.output()));
  run.child.stdin.write(JSON.stringify({ kind: 'debug-evaluate', generation: 7, id: 1, expression: `process.getBuiltinModule('fs').readFileSync(${JSON.stringify(secret)}, 'utf8')` }) + '\n');
  await waitUntil(() => frames().some(frame => frame.kind === 'debug-response'), 'no inspector response');
  assert.ok(frames().find(frame => frame.kind === 'debug-response').value.exceptionDetails);
  run.child.stdin.write(JSON.stringify({ kind: 'dispose', generation: 7 }) + '\n');
  assert.equal((await run.closed)[0], 0, run.output().stderr);
});

test('killing the helper kills the worker, then deferred cleanup removes staging', nativeOptions, async t => {
  const { root, minimalRuntime } = await fixture(t);
  const config = await windowsSandbox(root, minimalRuntime);
  const run = launch(t, config, ['-e', 'console.log(process.pid); setInterval(() => {}, 1000)']);
  await waitUntil(() => run.output().stdout.includes('\n') || run.child.exitCode !== null, 'worker did not start');
  const pid = Number(run.output().stdout.trim());
  assert.ok(Number.isInteger(pid) && pid > 0, JSON.stringify(run.output()));
  assert.ok(exists(pid));
  run.child.kill();
  await run.closed;
  await waitUntil(() => !exists(pid), 'worker survived helper death');
  await config.cleanup();
  await assert.rejects(access(config.execArgv[1]), { code: 'ENOENT' });
});

test('host death terminates the helper and worker without their cooperation', nativeOptions, async t => {
  const { root, minimalRuntime, directory } = await fixture(t);
  const hostScript = join(directory, 'host.mjs');
  const sandboxModule = new URL('../windows-sandbox.mjs', import.meta.url).href;
  await writeFile(hostScript, `
    import { windowsSandbox } from ${JSON.stringify(sandboxModule)};
    import { spawn } from 'node:child_process';
    const config = await windowsSandbox(${JSON.stringify(root)}, ${JSON.stringify(minimalRuntime)});
    const helper = spawn(config.execPath, [...config.execArgv, '-e', 'console.log(process.pid); setInterval(() => {}, 1000)'], { stdio: config.stdio });
    console.log(JSON.stringify({ helper: helper.pid, instance: config.execArgv[1] }));
    helper.stdout.pipe(process.stdout); helper.stderr.pipe(process.stderr);
    setInterval(() => {}, 1000);
  `);
  const host = spawn(process.execPath, [hostScript], { stdio: ['pipe', 'pipe', 'pipe'] });
  const closed = once(host, 'close');
  let output = '', errors = '', instance;
  host.stdout.on('data', data => output += data);
  host.stderr.on('data', data => errors += data);
  t.after(async () => {
    if (host.exitCode === null && host.signalCode === null) host.kill();
    await closed;
    if (instance) await rm(instance, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
  });
  await waitUntil(() => output.split('\n').length >= 3 || host.exitCode !== null, 'nested host did not start');
  const lines = output.trim().split('\n');
  assert.ok(lines.length >= 2, errors);
  const metadata = JSON.parse(lines[0]);
  instance = metadata.instance;
  const worker = Number(lines[1]);
  assert.ok(worker > 0 && exists(worker), errors);
  host.kill(); await closed;
  await waitUntil(() => !exists(worker) && !exists(metadata.helper), 'process survived host death');
});

test('junction packages and missing launcher fail closed', nativeOptions, async t => {
  const { root, minimalRuntime, directory } = await fixture(t);
  await symlink(minimalRuntime, join(root, 'escape'), 'junction');
  await assert.rejects(windowsSandbox(root, minimalRuntime), /non-regular entry/);
  await assert.rejects(windowsSandbox(root, minimalRuntime, { launcher: join(directory, 'absent.exe') }), { code: 'ENOENT' });
});

test('native helper rejects a reparse point introduced after staging without changing its target ACL', nativeOptions, async t => {
  const { root, minimalRuntime } = await fixture(t);
  const config = await windowsSandbox(root, minimalRuntime);
  const originalAcl = acl(minimalRuntime);
  await symlink(minimalRuntime, join(config.execArgv[1], 'package', 'escape'), 'junction');
  const run = launch(t, config, ['-e', 'console.log("UNSAFE-FALLBACK")']);
  assert.equal((await run.closed)[0], 125);
  assert.equal(run.output().stdout, '');
  assert.match(run.output().stderr, /FILE_ATTRIBUTE_REPARSE_POINT/);
  assert.equal(acl(minimalRuntime), originalAcl);
});
