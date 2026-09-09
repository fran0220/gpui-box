import { realpath, access } from 'node:fs/promises';
import { constants } from 'node:fs';

/** Seatbelt is the filesystem/network/process boundary. The mandatory native
 * launcher enforces CPU/fd/file limits and a physical-footprint kill budget.
 * No unsupported lane falls back to unconfined execution.
 */
export async function macosSandbox(root, runtimeRoot, { executable = process.execPath, node = true, launcher = process.env.GPUI_SANDBOX_LAUNCHER } = {}) {
  if (process.platform !== 'darwin') throw new Error('macOS OS backend requires a native macOS lane');
  if (!launcher) throw new Error('macOS resource launcher unavailable; build native/macos-launch.c and set GPUI_SANDBOX_LAUNCHER');
  launcher = await realpath(launcher);
  await access(launcher, constants.X_OK); await access('/usr/bin/sandbox-exec', constants.X_OK);
  const binary = await realpath(executable);
  root = await realpath(root); runtimeRoot = await realpath(runtimeRoot);
  const literal = value => {
    if (/[\x00-\x1f\x7f]/.test(value)) throw new Error('Unsupported sandbox path characters');
    return JSON.stringify(value);
  };
  const profile = `(version 1)
    (deny default)
    (allow process-exec (literal ${literal(binary)}))
    (allow signal (target self))
    (allow sysctl-read)
    (allow file-read-metadata)
    ; dyld/libignition opens the root directory during cache discovery. This
    ; literal permits that directory only, NOT reads of its descendants.
    (allow file-read-data (literal "/"))
    (allow file-read* (subpath ${literal(root)}) (subpath ${literal(runtimeRoot)})
      (literal ${literal(binary)}) (subpath "/System/Library") (subpath "/usr/lib")
      (subpath "/System/Cryptexes/OS/System/Library/dyld")
      (subpath "/System/Volumes/Preboot/Cryptexes/OS/System/Library/dyld")
      (subpath "/private/var/db/dyld") (subpath "/usr/share/zoneinfo")
      (literal "/dev/null") (literal "/dev/random") (literal "/dev/urandom"))
    (allow file-map-executable (literal ${literal(binary)})
      (subpath "/System/Library") (subpath "/usr/lib")
      (subpath "/System/Cryptexes/OS/System/Library/dyld")
      (subpath "/System/Volumes/Preboot/Cryptexes/OS/System/Library/dyld")
      (subpath "/private/var/db/dyld"))
    (allow file-write-data (literal "/dev/null"))
    (allow mach-lookup (global-name "com.apple.system.logger"))`;
  return { execPath: launcher, execArgv: [profile, binary, ...(node ? ['--max-old-space-size=64', '--permission', `--allow-fs-read=${root}`, `--allow-fs-read=${runtimeRoot}`, '--disable-proto=throw'] : [])], stdio: ['pipe', 'pipe', 'pipe'] };
}
