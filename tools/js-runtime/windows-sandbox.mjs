import { cp, lstat, mkdtemp, realpath, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

// Copy only ordinary files/directories. In particular, never grant an
// AppContainer access through a package-provided junction or symlink.
async function copyTree(source, destination) {
  await cp(source, destination, {
    recursive: true,
    force: false,
    errorOnExist: true,
    filter: async (entry) => {
      const stat = await lstat(entry);
      if (!stat.isDirectory() && !stat.isFile()) {
        throw new Error(`Windows sandbox rejects non-regular entry: ${entry}`);
      }
      return true;
    },
  });
}

export async function windowsSandbox(root, runtimeRoot, {
  executable = process.execPath,
  node = true,
  launcher = process.env.GPUI_SANDBOX_LAUNCHER,
} = {}) {
  if (process.platform !== 'win32') throw new Error('Windows sandbox requires Windows');
  if (!launcher) throw new Error('GPUI_SANDBOX_LAUNCHER must name the Windows sandbox helper');
  const [sourceRoot, sourceRuntime, sourceExecutable, helper] = await Promise.all(
    [root, runtimeRoot, executable, launcher].map((entry) => realpath(entry)),
  );
  if (!(await lstat(sourceRoot)).isDirectory() || !(await lstat(sourceRuntime)).isDirectory()) {
    throw new Error('Windows sandbox roots must be directories');
  }
  const instance = await mkdtemp(path.join(tmpdir(), 'gpui-js-'));
  const cleanup = () => rm(instance, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
  try {
    await copyTree(sourceRoot, path.join(instance, 'package'));
    await copyTree(sourceRuntime, path.join(instance, 'runtime'));
    await copyTree(sourceExecutable, path.join(instance, 'worker.exe'));
    // The helper accepts only this fixed layout, checks for reparse points,
    // and replaces the copies' ACLs before creating the untrusted process.
    return {
      execPath: helper,
      execArgv: [
        '--instance', instance,
        '--root', sourceRoot,
        '--runtime', sourceRuntime,
        '--parent', String(process.pid),
        '--',
        ...(node ? [
          '--disable-wasm-trap-handler', '--max-old-space-size=64', '--permission',
          `--allow-fs-read=${sourceRoot}`, `--allow-fs-read=${sourceRuntime}`,
          '--disable-proto=throw',
        ] : []),
      ],
      stdio: ['pipe', 'pipe', 'pipe'],
      cleanup,
    };
  } catch (error) {
    await cleanup();
    throw error;
  }
}
