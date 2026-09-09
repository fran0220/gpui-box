// Trusted package loader only; never imported by the untrusted worker SDK.
// Caller supplies a host-approved IMMUTABLE package root. This module creates a
// byte snapshot, not a path capability. Native registration still needs the
// separate resources grant and an active owner. Never mount a mutable adversary
// tree as an approved package: portable Node lacks directory-relative openat.
import { constants } from 'node:fs';
import { lstat, open, realpath } from 'node:fs/promises';
import { resolve, sep } from 'node:path';
import { RESOURCE_LIMITS, validateAssetDeclarations, validateResourceRegistration } from './resource-schema.mjs';

const activeRoots = new Set();
const sameFile = (a, b) => a.dev === b.dev && a.ino === b.ino && a.size === b.size && a.mtimeNs === b.mtimeNs && a.ctimeNs === b.ctimeNs;

/** Validate/copy bytes serially. Concurrent attempts for the same root fail
 * retryably rather than building an unbounded queue of package reads.
 * Returns immutable validated Registration[]; never paths, URLs or handles.
 */
export async function loadPackagedResources(root, assets) {
  // Validate every declaration before any IO (including later invalid entries).
  const declarations = validateAssetDeclarations(assets).map(asset => ({ ...asset }));
  if (typeof root !== 'string' || !root) throw new Error('Invalid package root');
  const base = await realpath(root);
  if (activeRoots.has(base) || activeRoots.size >= 4) throw new Error('Resource load concurrency limit; retry');
  activeRoots.add(base);
  try {
    const rootInfo = await lstat(root);
    if (rootInfo.isSymbolicLink() || !rootInfo.isDirectory()) throw new Error('Package root must be a real directory');
    const files = [];
    let total = 0;
    // Preflight the complete manifest and quotas before reading any file bytes.
    for (const asset of declarations) {
      let candidate = base;
      const parts = asset.path.split('/');
      for (let i = 0; i < parts.length; i++) {
        candidate = resolve(candidate, parts[i]);
        const info = await lstat(candidate);
        if (info.isSymbolicLink() || (i < parts.length - 1 && !info.isDirectory())) throw new Error('Package symlinks/non-directories refused');
      }
      const canonical = await realpath(candidate);
      if (!canonical.startsWith(base + sep)) throw new Error('Asset escapes package');
      const info = await lstat(canonical, { bigint: true });
      if (!info.isFile() || info.nlink !== 1n || info.size < 1n || info.size > BigInt(RESOURCE_LIMITS.bytes)) throw new Error('Invalid asset file/size');
      total += Number(info.size);
      if (total > RESOURCE_LIMITS.ownerBytes) throw new Error('Package resource byte quota exceeded');
      files.push({ asset, canonical, info });
    }
    const registrations = [];
    for (const { asset, canonical, info } of files) {
      const file = await open(canonical, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0) | (constants.O_NONBLOCK ?? 0));
      try {
        const opened = await file.stat({ bigint: true });
        if (!opened.isFile() || !sameFile(info, opened)) throw new Error('Package changed before resource read');
        // Linux additionally verifies the actual opened inode's pathname before
        // reading bytes. Other OS lanes rely on the immutable-package contract.
        if (process.platform === 'linux') {
          const actual = await realpath(`/proc/self/fd/${file.fd}`);
          if (actual !== canonical || !actual.startsWith(base + sep)) throw new Error('Opened asset escapes package');
        }
        const bytes = Buffer.alloc(Number(info.size) + 1);
        let length = 0;
        while (length < bytes.length) {
          const { bytesRead } = await file.read(bytes, length, bytes.length - length, length);
          if (!bytesRead) break;
          length += bytesRead;
        }
        if (length !== Number(info.size) || !sameFile(info, await file.stat({ bigint: true })) || !sameFile(info, await lstat(canonical, { bigint: true }))) throw new Error('Package changed during resource read');
        registrations.push(Object.freeze(validateResourceRegistration({ key: asset.key, mime: asset.mime, data: bytes.subarray(0, length).toString('base64') })));
      } finally {
        await file.close();
      }
    }
    return Object.freeze(registrations);
  } finally {
    activeRoots.delete(base);
  }
}
