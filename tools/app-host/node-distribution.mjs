// Optional distributable Node runtime. Hashes are from the official 26.5.1
// SHASUMS256.txt over HTTPS; they pin bytes, not a verified release signature.
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, rm, cp, lstat, chmod } from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Transform } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

const hashes = {
  'linux-x64': '2b07f09c218d473a26442bff5a90151f53f7b7c0a23bad244eda2c26303a2ba7',
  'darwin-arm64': 'f4387df0b46556516d19abf2f2d6806481ac8368aa7f9d96bafed422a56a1d01',
  'darwin-x64': '077d5c936868dab19d21f77f1e71ce13697e80b3e86a399dcab238902a2ebf93',
  'win32-x64': 'c432c996b95cbf7568f13a0fbb37526de84a27e3a5c520c3be15f05a9a168212',
};
export async function bundleNode(destination) {
  const lane = `${process.platform}-${process.arch}`;
  if (!hashes[lane]) throw new Error(`Bundled Node unavailable for ${lane}`);
  const windows = process.platform === 'win32';
  const stem = `node-v26.5.1-${windows ? 'win' : process.platform}-${process.arch}`;
  const url = `https://nodejs.org/dist/v26.5.1/${stem}.${windows ? 'zip' : 'tar.gz'}`;
  const temporary = await mkdtemp(resolve(tmpdir(), 'gpui-node-'));
  try {
    const archive = resolve(temporary, 'distribution');
    const response = await fetch(url, { redirect: 'error', signal: AbortSignal.timeout(120000) });
    if (!response.ok) throw new Error(`Node download failed: HTTP ${response.status}`);
    const hash = createHash('sha256'); let bytes = 0;
    await pipeline(response.body, new Transform({ transform(chunk, _encoding, callback) {
      bytes += chunk.length;
      if (bytes > 128 * 1024 * 1024) return callback(new Error('Node archive exceeds download limit'));
      hash.update(chunk); callback(null, chunk);
    } }), createWriteStream(archive, { flags: 'wx', mode: 0o600 }));
    if (hash.digest('hex') !== hashes[lane]) throw new Error('Node distribution checksum mismatch');
    const binary = windows ? 'node.exe' : 'bin/node';
    // Only two pinned members are extracted. No npm, scripts, or install hooks.
    await promisify(execFile)('tar', ['-xf', archive, '-C', temporary, `${stem}/${binary}`, `${stem}/LICENSE`], { timeout: 30000 });
    for (const name of [binary, 'LICENSE']) {
      const source = resolve(temporary, stem, name);
      if (!(await lstat(source)).isFile()) throw new Error('Node distribution member is not a regular file');
      const target = resolve(destination, name);
      await mkdir(resolve(target, '..'), { recursive: true });
      await cp(source, target, { errorOnExist: true, force: false });
    }
    if (!windows) await chmod(resolve(destination, binary), 0o755);
    return { version: '26.5.1', url, sha256: hashes[lane], license: 'runtime/node/LICENSE', signatureVerified: false };
  } finally { await rm(temporary, { recursive: true, force: true }); }
}
