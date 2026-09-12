import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

export async function readWfp(pid) {
  const directory = fileURLToPath(new URL('../../../target/runtime-native/wfp/', import.meta.url));
  await mkdir(directory, { recursive: true });
  const { stdout } = await promisify(execFile)('powershell.exe', [
    '-NoProfile', '-NonInteractive', '-File', fileURLToPath(new URL('./windows-wfp.ps1', import.meta.url)),
    '-EventsPath', join(directory, `events-${pid}.xml`),
    '-FiltersPath', join(directory, `filters-${pid}.xml`),
  ], { encoding: 'utf8', timeout: 10000, maxBuffer: 16 * 1024 * 1024 });
  return JSON.parse(stdout);
}

// A timeout is not a denial. Require the same worker's outbound attempt and
// its reversed inbound drop, resolving the causal filter ID from the OS dump.
// Public netevents omit PIDs on some Windows versions; app IDs, package SID,
// the explicitly bound source port and the attempt's time interval still bind
// both events to this probe. When ETW PIDs are available, check them too.
export function assertWfpLoopbackBlock({ events, filters }, attempt) {
  const path = hex => Buffer.from(hex, 'hex').toString('utf16le').replace(/\0+$/, '').toLowerCase();
  const sameAttempt = event => Number(event.protocol) === 6 &&
    event.localAddress === '127.0.0.1' && event.remoteAddress === '127.0.0.1' &&
    Date.parse(event.time) >= attempt.start && Date.parse(event.time) <= attempt.end;
  const outbound = events.filter(event => sameAttempt(event) && event.allow &&
    Number(event.localPort) === attempt.sourcePort && Number(event.remotePort) === attempt.listenerPort &&
    event.sid === attempt.sid && path(event.appId) === attempt.workerAppId.toLowerCase() &&
    (!event.pid || Number(event.pid) === attempt.workerPid));
  assert.ok(outbound.length, 'no same-attempt WFP outbound event for worker SID, image and TCP tuple');
  const drop = events.find(event => sameAttempt(event) && event.drop &&
    Number(event.localPort) === attempt.listenerPort && Number(event.remotePort) === attempt.sourcePort &&
    path(event.appId) === attempt.listenerAppId.toLowerCase() &&
    (!event.pid || Number(event.pid) === attempt.listenerPid) &&
    event.direction === 'MS_FWP_DIRECTION_IN' && event.loopback === 'true' &&
    outbound.some(allow => Date.parse(event.time) >= Date.parse(allow.time)) &&
    filters.some(rule => rule.id === event.filterId &&
      rule.layer === 'FWPM_LAYER_ALE_AUTH_RECV_ACCEPT_V4' &&
      rule.sublayer === 'FWPM_SUBLAYER_MPSSVC_APP_ISOLATION' && rule.action === 'FWP_ACTION_BLOCK'));
  assert.ok(drop, 'no correlated inbound AppContainer-isolation WFP blocking filter');
  return drop.filterId;
}
