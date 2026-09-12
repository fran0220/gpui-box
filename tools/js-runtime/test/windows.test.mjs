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
import { readWfp, assertWfpLoopbackBlock } from './windows-wfp.mjs';

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
  const child = spawn(config.execPath, [...config.execArgv, ...args], { stdio: config.stdio, detached: config.detached ?? false, ...extra });
  let stdout = '', stderr = '';
  child.stdout.on('data', chunk => stdout += chunk);
  child.stderr.on('data', chunk => stderr += chunk);
  const closed = once(child, 'close');
  t.after(async () => {
    if (child.exitCode === null && child.signalCode === null) child.kill();
    await closed;
    await config.cleanup?.();
  });
  return { child, closed, output: () => ({ stdout, stderr }) };
}

async function waitUntil(predicate, message, run) {
  for (let i = 0; i < 200; i++) {
    if (predicate()) return;
    if (run && (run.child.exitCode !== null || run.child.signalCode !== null)) {
      const [code, signal] = await run.closed;
      assert.fail(`${message}: child exited code=${code} signal=${signal}; ${JSON.stringify(run.output())}`);
    }
    await delay(25);
  }
  assert.fail(`${message}${run ? `: ${JSON.stringify(run.output())}` : ''}`);
}

function exists(pid) {
  try { process.kill(pid, 0); return true; }
  catch (error) { if (error.code === 'ESRCH') return false; throw error; }
}

function acl(path) {
  assert.ok(process.env.GPUI_WINDOWS_SANDBOX_PROBE, 'native ACL reader must be built');
  return execFileSync(process.env.GPUI_WINDOWS_SANDBOX_PROBE, ['--acl', path], { encoding: 'utf8', timeout: 10000 }).trim();
}

function profileExists(name) {
  return JSON.parse(execFileSync(process.env.GPUI_WINDOWS_SANDBOX_PROBE,
    ['--profile-exists', name], { encoding: 'utf8', timeout: 10000 }));
}

function option(config, flag) {
  const index = config.execArgv.indexOf(flag);
  assert.ok(index >= 0 && index + 1 < config.execArgv.length, `missing launcher option ${flag}`);
  return config.execArgv[index + 1];
}

test('Windows fixture reads launcher metadata by flag, independent of option order', () => {
  const config = { execArgv: ['--profile', 'gpui-js-example', '--instance', 'staging', '--'] };
  assert.equal(option(config, '--profile'), 'gpui-js-example');
  assert.equal(option(config, '--instance'), 'staging');
  assert.throws(() => option(config, '--missing'), /missing launcher option/);
});

test('Windows readiness observer reports an early launcher exit with stderr', async t => {
  const run = launch(t, { execPath: process.execPath, execArgv: ['-e', "process.stderr.write('launch failed');process.exit(125)"], stdio: ['pipe', 'pipe', 'pipe'] }, []);
  await assert.rejects(waitUntil(() => false, 'not ready', run), /child exited code=125.*launch failed/);
});

test('Windows factory refuses execution on another OS', { skip: windows }, async () => {
  await assert.rejects(windowsSandbox('.', '.'), /requires Windows/);
});

test('native strict handle policy readback and invalid-reference controls', nativeOptions, async t => {
  await fixture(t);
  const probe = { execPath: process.env.GPUI_WINDOWS_SANDBOX_PROBE, execArgv: [], stdio: ['pipe', 'pipe', 'pipe'] };
  const ordinary = launch(t, probe, ['--invalid-handle-control', '0']);
  assert.equal((await ordinary.closed)[0], 0, ordinary.output().stderr);
  const strict = launch(t, probe, ['--invalid-handle-control', '1']);
  assert.equal((await strict.closed)[0] >>> 0, 0xc0000008, strict.output().stderr);
  assert.match(strict.output().stderr, /strict handle policy readback=3/);
});

test('native handle snapshot checks low-rights high handles under strict invalid-handle policy', nativeOptions, async t => {
  const { root, secret } = await fixture(t);
  const other = join(root, 'not-the-sentinel');
  await writeFile(other, 'unrelated file');
  const identity = execFileSync(process.env.GPUI_WINDOWS_SANDBOX_PROBE, ['--file-id', secret], { encoding: 'utf8' }).trim();
  const probe = { execPath: process.env.GPUI_WINDOWS_SANDBOX_PROBE, execArgv: [], stdio: ['pipe', 'pipe', 'pipe'] };
  for (const rights of ['0', '1']) {
    for (const [file, inherit, failure] of [
      [other, '0', null],
      [secret, '0', /wcscmp\(identity, sentinel\) != 0 failed/],
      [other, '1', /!\(flags & HANDLE_FLAG_INHERIT\) failed/],
    ]) {
      const run = launch(t, probe, ['--handle-control', identity, file, inherit, rights]);
      assert.equal((await run.closed)[0], failure ? 125 : 0, run.output().stderr);
      assert.match(run.output().stderr, /strict handle policy readback=3/);
      const control = /control handle=(\d+)/.exec(run.output().stderr);
      assert.ok(control, run.output().stderr);
      assert.ok(Number(control[1]) >= 65536, 'control must exceed the former numeric scan range');
      const scanned = new RegExp(`disk handle=${control[1]} flags=(\\d+) access=([0-9a-f]+) id=([0-9a-f:]+)`).exec(run.output().stderr);
      assert.ok(scanned, 'the actual control handle must be inspected');
      assert.equal(Number(scanned[1]), Number(inherit));
      assert.equal(parseInt(scanned[2], 16), Number(rights), 'OS GrantedAccess must be zero or FILE_READ_DATA only, without FILE_READ_ATTRIBUTES');
      if (file === secret) assert.equal(scanned[3], identity);
      else assert.notEqual(scanned[3], identity);
      if (failure) assert.match(run.output().stderr, failure);
    }
  }
});

// Native TCP retransmission takes ~21s on this lane. Reserve a separate bounded
// 10s WFP read budget, plus fixture/ACL/positive-control work; no runtime deadline changes.
test('native AppContainer blocks host reads, writes, network, spawning and leaked handles', { ...nativeOptions, timeout: 45000 }, async t => {
  const { root, minimalRuntime, secret } = await fixture(t);
  assert.ok(process.env.GPUI_WINDOWS_SANDBOX_PROBE, 'build and set GPUI_WINDOWS_SANDBOX_PROBE');
  assert.ok(process.env.LOCALAPPDATA);
  const originals = [root, minimalRuntime, secret, process.env.GPUI_WINDOWS_SANDBOX_PROBE,
    process.env.LOCALAPPDATA, join(process.env.LOCALAPPDATA, 'Packages')];
  const originalAcls = originals.map(acl);
  let connections = 0;
  const server = createServer(socket => { connections++; socket.destroy(); });
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  t.after(() => new Promise(resolve => server.close(resolve)));
  const control = launch(t, { execPath: process.env.GPUI_WINDOWS_SANDBOX_PROBE, execArgv: [], stdio: ['pipe', 'pipe', 'pipe'] }, ['--connect-control', String(server.address().port)]);
  assert.equal((await control.closed)[0], 0, control.output().stderr);
  await waitUntil(() => connections === 1, 'native positive-control connection was not observed');
  const sentinel = await open(secret, 'r');
  t.after(() => sentinel.close());
  const identity = execFileSync(process.env.GPUI_WINDOWS_SANDBOX_PROBE, ['--file-id', secret], { encoding: 'utf8', timeout: 10000 }).trim();
  assert.match(identity, /^[0-9a-f]{8}:[0-9a-f]{8}:[0-9a-f]{8}$/);
  const probe = { execPath: process.env.GPUI_WINDOWS_SANDBOX_PROBE, execArgv: [], stdio: ['pipe', 'pipe', 'pipe'] };
  const clean = launch(t, probe, ['--leak-check', identity]);
  assert.equal((await clean.closed)[0], 0, clean.output().stderr);
  const leaked = launch(t, probe, ['--leak-check', identity], { stdio: [...probe.stdio, sentinel.fd] });
  assert.equal((await leaked.closed)[0], 125, 'positive control must detect a real inherited sentinel');
  assert.match(leaked.output().stderr, /wcscmp\(identity, sentinel\) != 0 failed/);
  const config = await windowsSandbox(root, minimalRuntime, { executable: process.env.GPUI_WINDOWS_SANDBOX_PROBE, node: false });
  const start = Date.now();
  const run = launch(t, config, [String(server.address().port), secret, identity], {
    stdio: [...config.stdio, sentinel.fd],
    env: { ...process.env, GPUI_TEST_SECRET: 'must-not-inherit' },
  });
  const [code] = await run.closed;
  const end = Date.now();
  assert.equal(connections, 1, 'sandbox reached the proven-live host listener');
  assert.deepEqual(originals.map(acl), originalAcls, 'source and host profile-parent ACLs must remain unchanged');
  assert.equal(code, 0, run.output().stderr);
  const { tcpError, tcpLocalPort, ...assertions } = JSON.parse(run.output().stdout);
  assert.deepEqual(assertions, {
    appcontainer: true, capabilities: 0, readonly: true, hostDenied: true,
    spawnDenied: true, udpDenied: true, handles: true,
    memory: 268435456, cpuSeconds: 30, cpuRate: 2500, activeProcesses: 1,
  });
  assert.ok(Number.isInteger(tcpLocalPort) && tcpLocalPort > 0 && tcpLocalPort <= 65535);
  assert.ok(tcpError === 10013 || tcpError === 10060, `unexpected TCP error ${tcpError}`);
  if (tcpError === 10060) {
    const metadata = /pid=(\d+) packageSID=(S-1-15-2-[\d-]+) tcp=/.exec(run.output().stderr);
    const appId = /Windows sandbox probe: appID=([^\r\n]+)/.exec(run.output().stderr);
    assert.ok(metadata && appId, run.output().stderr);
    const workerPid = Number(metadata[1]);
    const listenerAppId = execFileSync(probe.execPath, ['--app-id', process.execPath], { encoding: 'utf8', timeout: 10000 }).trim();
    const filterId = assertWfpLoopbackBlock(await readWfp(workerPid), {
      start, end, workerPid, sid: metadata[2], workerAppId: appId[1],
      listenerPid: process.pid, listenerAppId, sourcePort: tcpLocalPort, listenerPort: server.address().port,
    });
    t.diagnostic(`TCP timeout verified against AppContainer-isolation WFP filter ${filterId}, worker ${workerPid}, source port ${tcpLocalPort}`);
  }
  const after = launch(t, probe, ['--connect-control', String(server.address().port)]);
  assert.equal((await after.closed)[0], 0, after.output().stderr);
  await waitUntil(() => connections === 2, 'native listener must remain reachable after the denied attempt');
  assert.deepEqual(originals.map(acl), originalAcls, 'host ACLs must remain unchanged');
  assert.equal(await readFile(secret, 'utf8'), 'host-only-sentinel');
  await config.cleanup();
  await assert.rejects(access(option(config, '--instance')), { code: 'ENOENT' });
  assert.equal(profileExists(option(config, '--profile')), false);
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
  // Job time-limit termination returns an NTSTATUS, not its Win32 mapping.
  assert.equal(code >>> 0, 0xc0000044, `expected STATUS_QUOTA_EXCEEDED, got ${code}: ${run.output().stderr}`);
  const accounting = /quota exit user_100ns=(\d+)/.exec(run.output().stderr);
  assert.ok(accounting, run.output().stderr);
  assert.ok(BigInt(accounting[1]) >= 300000000n, 'worker must receive its full 30-second user CPU budget');
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
  await waitUntil(() => run.output().stdout.includes('"kind":"ready"'), 'worker did not become ready', run);
  const frames = () => {
    const output = run.output().stdout;
    return output.slice(0, output.lastIndexOf('\n') + 1).split('\n').filter(Boolean).map(line => JSON.parse(line));
  };
  assert.ok(frames().some(frame => frame.kind === 'render' && frame.generation === 7 && frame.tree.text === 'Windows ready'), JSON.stringify(run.output()));
  run.child.stdin.write(JSON.stringify({ kind: 'debug-evaluate', generation: 7, id: 1, expression: `process.getBuiltinModule('fs').readFileSync(${JSON.stringify(secret)}, 'utf8')` }) + '\n');
  await waitUntil(() => frames().some(frame => frame.kind === 'debug-response'), 'no inspector response', run);
  assert.ok(frames().find(frame => frame.kind === 'debug-response').value.exceptionDetails);
  run.child.stdin.write(JSON.stringify({ kind: 'dispose', generation: 7 }) + '\n');
  assert.equal((await run.closed)[0], 0, run.output().stderr);
});

test('killing the helper kills the worker, then deferred cleanup removes staging', nativeOptions, async t => {
  const { root, minimalRuntime } = await fixture(t);
  const config = await windowsSandbox(root, minimalRuntime);
  const run = launch(t, config, ['-e', 'console.log(process.pid); setInterval(() => {}, 1000)']);
  await waitUntil(() => run.output().stdout.includes('\n'), 'worker did not start', run);
  const pid = Number(run.output().stdout.trim());
  assert.ok(Number.isInteger(pid) && pid > 0, JSON.stringify(run.output()));
  assert.ok(exists(pid));
  assert.equal(profileExists(option(config, '--profile')), true);
  run.child.kill();
  await run.closed;
  await waitUntil(() => !exists(pid), 'worker survived helper death');
  await config.cleanup();
  await assert.rejects(access(option(config, '--instance')), { code: 'ENOENT' });
  assert.equal(profileExists(option(config, '--profile')), false);
});

test('host death terminates the helper and worker without their cooperation', nativeOptions, async t => {
  const { root, minimalRuntime, directory } = await fixture(t);
  const hostScript = join(directory, 'host.mjs');
  const sandboxModule = new URL('../windows-sandbox.mjs', import.meta.url).href;
  await writeFile(hostScript, `
    import assert from 'node:assert/strict';
    import { windowsSandbox } from ${JSON.stringify(sandboxModule)};
    import { spawn } from 'node:child_process';
    const option = ${option.toString()};
    const config = await windowsSandbox(${JSON.stringify(root)}, ${JSON.stringify(minimalRuntime)});
    assert.equal(config.detached, true, 'trusted watcher must survive the host libuv job');
    const helper = spawn(config.execPath, [...config.execArgv, '-e', 'console.log(process.pid); setInterval(() => {}, 1000)'], { stdio: config.stdio, detached: config.detached });
    console.log(JSON.stringify({ helper: helper.pid, instance: option(config, '--instance'), profile: option(config, '--profile') }));
    helper.stdout.pipe(process.stdout); helper.stderr.pipe(process.stderr);
    helper.on('error', error => { console.error(error.message); process.exit(125); });
    helper.on('close', (code, signal) => { console.error('helper exited', code, signal); process.exit(code || 125); });
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
    // Metadata is emitted before payload readiness, including failed launch.
    const metadata = output.includes('\n') ? JSON.parse(output.split('\n')[0]) : null;
    if (metadata) {
      instance = metadata.instance;
      execFileSync(process.env.GPUI_SANDBOX_LAUNCHER, ['--delete-profile', metadata.profile], { timeout: 15000 });
    }
    if (instance) await rm(instance, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
  });
  await waitUntil(() => output.split('\n').length >= 3, 'nested host did not start', {
    child: host, closed, output: () => ({ stdout: output, stderr: errors }),
  });
  const lines = output.trim().split('\n');
  assert.ok(lines.length >= 2, errors);
  const metadata = JSON.parse(lines[0]);
  instance = metadata.instance;
  const worker = Number(lines[1]);
  assert.ok(worker > 0 && exists(worker), errors);
  host.kill(); await closed;
  await waitUntil(() => !exists(worker) && !exists(metadata.helper), 'process survived host death');
  assert.equal(profileExists(metadata.profile), false);
});

test('failed image creation cleans its provisioned profile without executing a payload', nativeOptions, async t => {
  const { root, minimalRuntime } = await fixture(t);
  const config = await windowsSandbox(root, minimalRuntime);
  await writeFile(join(option(config, '--instance'), 'worker.exe'), 'not a PE executable');
  const run = launch(t, config, []);
  assert.equal((await run.closed)[0], 125);
  // Loader revisions report BAD_EXE_FORMAT or EXE_MACHINE_TYPE_MISMATCH.
  assert.match(run.output().stderr, /CreateProcessW failed \((193|216)\)/);
  assert.equal(run.output().stdout, '');
  assert.equal(profileExists(option(config, '--profile')), false);
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
  await symlink(minimalRuntime, join(option(config, '--instance'), 'package', 'escape'), 'junction');
  const run = launch(t, config, ['-e', 'console.log("UNSAFE-FALLBACK")']);
  assert.equal((await run.closed)[0], 125);
  assert.equal(run.output().stdout, '');
  assert.match(run.output().stderr, /FILE_ATTRIBUTE_REPARSE_POINT/);
  assert.equal(acl(minimalRuntime), originalAcl);
});
