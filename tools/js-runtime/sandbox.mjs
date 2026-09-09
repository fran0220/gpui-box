import { linuxSandbox } from './linux-sandbox.mjs';
import { macosSandbox } from './macos-sandbox.mjs';

export const nativeBackend = { linux: 'linux', darwin: 'macos', win32: 'windows' }[process.platform];
export async function createSandbox(backend, root, runtimeRoot, options = {}) {
  if (!backend || backend !== nativeBackend) throw new Error(`OS sandbox unavailable: ${backend ?? 'no backend'} on ${process.platform}`);
  if (backend === 'linux') return linuxSandbox(root, runtimeRoot, options);
  if (backend === 'macos') return macosSandbox(root, runtimeRoot, options);
  const { windowsSandbox } = await import('./windows-sandbox.mjs');
  return windowsSandbox(root, runtimeRoot, options);
}
