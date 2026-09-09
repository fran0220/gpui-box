#!/usr/bin/env node
import { readFile, writeFile, mkdir, cp, chmod, stat, readdir } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import { PluginPlatform, bundleDirectory, bundleFileBytes } from '../plugin-platform/platform.mjs';
import { bundleNode } from './node-distribution.mjs';
import { debugPipeHelper, evaluateDebug } from './debug.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '../..');
const [command, ...args] = process.argv.slice(2);
const option = key => { const i = args.indexOf(key); return i < 0 ? undefined : args[i + 1]; };
const manifest = root => readFile(resolve(root, 'app.json'), 'utf8').then(JSON.parse);
const run = (program, parameters) => new Promise((resolve, reject) => {
  const child = spawn(program, parameters, { stdio: 'inherit' });
  child.on('error', reject); child.on('exit', (code, signal) => code === 0 ? resolve() : reject(new Error(`${program} exited ${code ?? signal}`)));
});
try {
  const [major, minor, patch] = process.versions.node.split('.').map(Number);
  if (major < 26 || (major === 26 && (minor < 5 || (minor === 5 && patch < 1)))) throw new Error('Node >=26.5.1 required; 26.5.1 is the verified runtime');
  if (command === 'init') {
    const destination = resolve(args[0] ?? 'my-gpui-app');
    await mkdir(destination); // Never merge a template into existing work.
    for (const name of await readdir(resolve(here, 'template'))) {
      await cp(resolve(here, 'template', name), resolve(destination, name), { recursive: true, force: false, errorOnExist: true });
    }
    // The template entry re-exports the SDK so its global is declared only once.
    await writeFile(resolve(destination, 'gpui.d.ts'), "export * from './sdk.js';\n");
    // Keep all sibling declaration imports usable outside this checkout.
    for (const name of await readdir(resolve(here, '../js-runtime'))) {
      if (name.endsWith('.d.ts')) {
        await cp(resolve(here, '../js-runtime', name), resolve(destination, name));
      }
    }
    console.log(`Created ${destination}. Run: gpui-app dev ${destination}`);
  } else if (command === 'dev' || command === 'run') {
    const binary = option('--host') ?? process.env.GPUI_APP_HOST ?? resolve(repo, 'target/debug/gpui-box-app-host');
    await stat(binary).catch(() => { throw new Error('Build the native host first: cargo build -p gpui-box-app-host (or pass --host)'); });
    await run(binary, [resolve(here, 'runner.mjs'), resolve(args[0] ?? '.'), ...(command === 'dev' ? ['--dev'] : []), ...args.slice(1)]);
  } else if (command === 'debug') {
    console.log(JSON.stringify(await evaluateDebug(resolve(args[0]), args[1]), null, 2));
  } else if (command === 'bundle') {
    const root = resolve(args[0]);
    const bundle = await bundleDirectory(root, await manifest(root));
    await writeFile(resolve(args[1]), JSON.stringify(bundle), { flag: 'wx' });
    console.log(`Bundle ${bundle.manifest.id}@${bundle.manifest.version}: ${bundle.sha256}`);
  } else if (command === 'plugin-install') {
    const path = resolve(args[0]);
    if ((await stat(path)).size > 5 * 1024 * 1024) throw new Error('Bundle file exceeds limit');
    const result = await new PluginPlatform(resolve(args[1])).install(JSON.parse(await readFile(path, 'utf8')));
    console.log(`Installed ${result.id}@${result.version}; execution remains disabled until enabled in host`);
  } else if (command === 'plugin-list') {
    console.log(JSON.stringify(await new PluginPlatform(resolve(args[0])).discover(), null, 2));
  } else if (command === 'build') {
    const root = resolve(args[0]), destination = resolve(args[1]);
    const bundle = await bundleDirectory(root, await manifest(root));
    const host = option('--host') ?? resolve(repo, 'target/release/gpui-box-app-host');
    await stat(host);
    const debugHelper = option('--debug-helper') ?? (process.platform === 'win32' ? debugPipeHelper() : null);
    if (debugHelper && !(await stat(debugHelper)).isFile()) throw new Error('Debug helper must be a regular executable file');
    await mkdir(destination); // Packaging is local and refuses an existing output directory.
    await mkdir(resolve(destination, 'app'));
    for (const [path, content] of Object.entries(bundle.files)) {
      await mkdir(dirname(resolve(destination, 'app', path)), { recursive: true });
      await writeFile(resolve(destination, 'app', path), bundleFileBytes(content));
    }
    await writeFile(resolve(destination, 'app/.gpui-bundle.json'), JSON.stringify(bundle), { flag: 'wx', mode: 0o600 });
    await mkdir(resolve(destination, 'tools/app-host'), { recursive: true });
    await cp(host, resolve(destination, process.platform === 'win32' ? 'gpui-box-app-host.exe' : 'gpui-box-app-host'));
    for (const dir of ['js-runtime', 'plugin-platform']) await cp(resolve(here, '..', dir), resolve(destination, 'tools', dir), { recursive: true });
    await cp(resolve(here, 'runner.mjs'), resolve(destination, 'tools/app-host/runner.mjs'));
    await cp(resolve(here, 'drop-bridge.mjs'), resolve(destination, 'tools/app-host/drop-bridge.mjs'));
    await cp(resolve(here, 'debug.mjs'), resolve(destination, 'tools/app-host/debug.mjs'));
    const bundled = args.includes('--bundle-node') ? await bundleNode(resolve(destination, 'runtime/node')) : null;
    if (debugHelper) {
      await mkdir(resolve(destination, 'runtime'), { recursive: true });
      await cp(debugHelper, resolve(destination, 'runtime/gpui-debug-pipe.exe'));
    }
    const sandboxLauncher = option('--sandbox-launcher');
    const launcherName = `gpui-sandbox-launch${process.platform === 'win32' ? '.exe' : ''}`;
    if (sandboxLauncher) {
      if (!(await stat(sandboxLauncher)).isFile()) throw new Error('Sandbox launcher must be a regular executable file');
      await mkdir(resolve(destination, 'runtime'), { recursive: true });
      await cp(sandboxLauncher, resolve(destination, 'runtime', launcherName));
      await chmod(resolve(destination, 'runtime', launcherName), 0o755);
    }
    const launch = '#!/bin/sh\nset -eu\nHERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)\n' +
      (bundled ? 'export GPUI_NODE="$HERE/runtime/node/bin/node"\n' : '') +
      (debugHelper ? 'export GPUI_DEBUG_PIPE_HELPER="$HERE/runtime/gpui-debug-pipe.exe"\n' : '') +
      (sandboxLauncher ? `export GPUI_SANDBOX_LAUNCHER="$HERE/runtime/${launcherName}"\n` : '') +
      'exec "$HERE/gpui-box-app-host" "$HERE/tools/app-host/runner.mjs" "$HERE/app" "$@"\n';
    await writeFile(resolve(destination, 'run.sh'), launch); await chmod(resolve(destination, 'run.sh'), 0o755);
    await writeFile(resolve(destination, 'run.cmd'), '@echo off\r\n' + (bundled ? 'set "GPUI_NODE=%~dp0runtime\\node\\node.exe"\r\n' : '') + (debugHelper ? 'set "GPUI_DEBUG_PIPE_HELPER=%~dp0runtime\\gpui-debug-pipe.exe"\r\n' : '') + (sandboxLauncher ? `set "GPUI_SANDBOX_LAUNCHER=%~dp0runtime\\${launcherName}"\r\n` : '') + '"%~dp0gpui-box-app-host.exe" "%~dp0tools\\app-host\\runner.mjs" "%~dp0app" %*\r\n');
    await writeFile(resolve(destination, 'build-info.json'), JSON.stringify({ schema: 1, app: bundle.manifest, digest: bundle.sha256, platform: process.platform, arch: process.arch, node: bundled?.version ?? process.versions.node, runtimeBundled: Boolean(bundled), runtime: bundled, sandboxLauncher: sandboxLauncher ? launcherName : null, debugHelper: debugHelper ? 'gpui-debug-pipe.exe' : null }, null, 2));
    console.log(`Packaged ${destination}. ${bundled ? 'Pinned Node26.5.1 and its LICENSE included.' : 'External Node >=26.5.1 required.'} Linux bubblewrap/prlimit remain required; this is not a signed installer.`);
  } else {
    console.log('gpui-app init DIR | dev APP [--debug] [--trust-local] [--data-dir DIR] [--host BIN] | run APP | debug DATA EXPRESSION | bundle APP FILE | plugin-install FILE STORE | plugin-list STORE | build APP OUT [--host BIN] [--bundle-node] [--sandbox-launcher BIN] [--debug-helper BIN]');
    if (command && command !== '--help') process.exitCode = 1;
  }
} catch (error) { console.error(error.message); process.exitCode = 1; }
