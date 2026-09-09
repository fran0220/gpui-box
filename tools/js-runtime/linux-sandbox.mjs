import { open, mkdtemp, writeFile, rm, realpath } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

/** Linux x86-64 only. Deny process creation (threads may share this process),
 * network sockets, namespace changes and privileged kernel APIs at syscall level.
 * This filter is inherited across exec; Node's permission model is not the boundary.
 */
export function seccompFilter() {
  const instructions = [];
  const stmt = (code, k, jt = 0, jf = 0) => instructions.push([code, jt, jf, k]);
  const deny = 0x00050001; // SECCOMP_RET_ERRNO | EPERM
  stmt(0x20, 4); // seccomp_data.arch
  stmt(0x15, 0xc000003e, 1); // AUDIT_ARCH_X86_64
  stmt(0x06, 0x80000000); // kill process on a different syscall ABI
  stmt(0x20, 0); // seccomp_data.nr
  stmt(0x45, 0x40000000, 0, 1); // reject x32 ABI
  stmt(0x06, deny);
  // clone3 returns ENOSYS so libc falls back to clone for native threads.
  stmt(0x15, 435, 0, 1); stmt(0x06, 0x00050026);
  for (const syscall of [57, 58, 41, 42, 53, 101, 155, 165, 166, 169, 175, 176, 246, 248, 249, 250, 272, 298, 304, 308, 313, 321, 323, 425, 426, 427, 29, 64, 68]) {
    stmt(0x15, syscall, 0, 1); stmt(0x06, deny);
  }
  stmt(0x15, 56, 0, 4); // clone
  stmt(0x20, 16); // clone flags, argument 0
  stmt(0x45, 0x00010000, 1, 0); // CLONE_THREAD: require same process/address space
  stmt(0x06, deny);
  stmt(0x06, 0x7fff0000);
  stmt(0x06, 0x7fff0000); // SECCOMP_RET_ALLOW
  const buffer = Buffer.alloc(instructions.length * 8);
  instructions.forEach(([code, jt, jf, k], i) => {
    buffer.writeUInt16LE(code, i * 8); buffer[i * 8 + 2] = jt; buffer[i * 8 + 3] = jf;
    buffer.writeUInt32LE(k, i * 8 + 4);
  });
  return buffer;
}

export async function linuxSandbox(root, runtimeRoot, { executable = process.execPath, node = true } = {}) {
  if (process.platform !== 'linux' || process.arch !== 'x64') throw new Error('OS sandbox unavailable: supported lane is Linux x86-64 with bubblewrap and user namespaces');
  const temporary = await mkdtemp(resolve(tmpdir(), 'gpui-seccomp-'));
  let handle;
  try {
    const path = resolve(temporary, 'filter.bpf');
    await writeFile(path, seccompFilter(), { mode: 0o600 });
    handle = await open(path, 'r');
    const binary = await realpath(executable);
    return {
      execPath: '/usr/bin/prlimit',
      execArgv: [
        '--as=2147483648', '--cpu=30', '--nofile=64', '--fsize=1048576', '--core=0', '--',
        '/usr/bin/bwrap', '--unshare-all', '--unshare-user', '--die-with-parent', '--new-session', '--cap-drop', 'ALL',
        '--clearenv',
        '--setenv', 'NODE_NO_WARNINGS', '1',
        '--ro-bind', binary, '/bin/node', '--ro-bind', '/lib', '/lib', '--ro-bind', '/lib64', '/lib64',
        '--ro-bind', '/usr/bin/prlimit', '/bin/prlimit',
        '--ro-bind', root, '/app', '--ro-bind', runtimeRoot, '/runtime',
        '--proc', '/proc', '--remount-ro', '/proc', '--ro-bind', '/dev/null', '/dev/null',
        '--ro-bind', '/dev/urandom', '/dev/urandom', '--size', '16777216', '--tmpfs', '/tmp',
        '--chdir', '/app', '--remount-ro', '/', '--seccomp', '3',
        '/bin/prlimit', '--nproc=64', '--',
        '/bin/node', ...(node ? ['--disable-wasm-trap-handler', '--max-old-space-size=64', '--permission', '--allow-fs-read=/app', '--allow-fs-read=/runtime', '--disable-proto=throw'] : []),
      ],
      stdio: ['pipe', 'pipe', 'pipe', handle.fd],
      async afterSpawn() { await handle.close(); await rm(temporary, { recursive: true, force: true }); },
    };
  } catch (error) {
    await handle?.close(); await rm(temporary, { recursive: true, force: true }); throw error;
  }
}
