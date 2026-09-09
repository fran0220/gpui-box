// Developer-only local socket. Untrusted workers cannot reach this host path or
// create sockets; OS file mode, not a JS whitelist, protects the debug endpoint.
import { createServer, createConnection } from 'node:net';
import { chmod, mkdir, lstat } from 'node:fs/promises';
import { resolve } from 'node:path';
import { readFrames, encodeFrame } from '../js-runtime/wire.mjs';

export async function startDebug(data, evaluate) {
  if (process.platform === 'win32') throw new Error('Debug socket unavailable: Windows ACL backend not implemented');
  const directory = resolve(data, 'debug');
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const info = await lstat(directory);
  if (!info.isDirectory() || info.uid !== process.getuid() || (info.mode & 0o077)) throw new Error('Debug directory must be private and owned by the current user');
  const clients = new Set();
  const server = createServer(socket => {
    clients.add(socket); socket.on('close', () => clients.delete(socket));
    socket.on('error', () => socket.destroy()); socket.setTimeout(5000, () => socket.destroy());
    let used = false;
    readFrames(socket, request => {
      if (used) { socket.destroy(); return; } used = true;
      Promise.resolve().then(() => evaluate(request.expression)).then(
        value => socket.end(encodeFrame({ value })),
        error => socket.end(encodeFrame({ error: error.message })),
      ).catch(() => socket.destroy());
    }, () => socket.destroy());
  });
  server.maxConnections = 4;
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(resolvePath(data), resolve); });
  try { await chmod(resolvePath(data), 0o600); }
  catch (error) { server.close(); throw error; }
  return { close() { for (const socket of clients) socket.destroy(); return new Promise(resolve => server.close(resolve)); } };
}
const resolvePath = data => resolve(data, 'debug', 'debug.sock');

export function evaluateDebug(data, expression) {
  return new Promise((resolve, reject) => {
    const socket = createConnection(resolvePath(data));
    socket.on('error', reject); socket.setTimeout(5000, () => { socket.destroy(); reject(new Error('Debug socket timed out')); });
    socket.on('connect', () => socket.write(encodeFrame({ expression })));
    readFrames(socket, message => { socket.destroy(); message.error ? reject(new Error(message.error)) : resolve(message.value); }, error => { socket.destroy(); reject(error); });
  });
}
