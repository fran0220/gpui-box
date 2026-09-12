import test from 'node:test';
import assert from 'node:assert/strict';
import fixture from './windows-wfp-fixture.json' with { type: 'json' };
import udpFixture from './windows-wfp-udp-fixture.json' with { type: 'json' };
import { assertWfpLoopbackBlock, assertWfpExternalUdpBlock } from './windows-wfp.mjs';

// Normalized by windows-wfp.ps1 from runtime-native-windows/wfp/wfpdiag.xml:
// https://github.com/fran0220/gpui-box/actions/runs/34680808729/artifacts/10294033468
// Expectations below come from that job's independent probe/launcher log.
const attempt = {
  start: Date.parse('2026-09-12T07:29:50Z'), end: Date.parse('2026-09-12T07:30:13Z'),
  workerPid: 3440, listenerPid: 7472, sourcePort: 52646, listenerPort: 52644,
  sid: 'S-1-15-2-767269591-1719422916-2757674391-3374017764-3735602449-313950896-3109700752',
  workerAppId: String.raw`\Device\HarddiskVolume4\Users\runneradmin\AppData\Local\Temp\gpui-js-jqzfoz\worker.exe`,
  listenerAppId: String.raw`\Device\HarddiskVolume4\hostedtoolcache\windows\node\26.5.1\x64\node.exe`,
};

test('WFP accepts actual correlated AppContainer loopback drop, not the receiving null package SID', () => {
  assert.equal(assertWfpLoopbackBlock(fixture, attempt), '71179');
  const publicEvents = structuredClone(fixture);
  for (const event of publicEvents.events) delete event.pid;
  assert.equal(assertWfpLoopbackBlock(publicEvents, attempt), '71179');
});

test('WFP rejects stale, unrelated, missing and non-isolation evidence for a TCP timeout', () => {
  const cases = [
    ['missing outbound', data => data.events.shift()],
    ['missing drop', data => data.events.pop()],
    ['missing filter', data => data.filters.pop()],
    ['wrong worker SID', data => data.events[0].sid = 'S-1-0-0'],
    ['wrong worker PID', data => data.events[0].pid = '7472'],
    ['wrong listener PID', data => data.events[1].pid = '3440'],
    ['wrong worker image', data => data.events[0].appId = data.events[1].appId],
    ['wrong listener image', data => data.events[1].appId = data.events[0].appId],
    ['wrong source port', data => data.events[0].localPort = '52647'],
    ['wrong destination port', data => data.events[1].localPort = '52647'],
    ['wrong reverse tuple', data => data.events[1].remotePort = '52644'],
    ['wrong address', data => data.events[1].remoteAddress = '127.0.0.2'],
    ['wrong protocol', data => data.events[1].protocol = '17'],
    ['stale outbound', data => data.events[0].time = '2026-09-12T07:29:49.999Z'],
    ['drop after attempt', data => data.events[1].time = '2026-09-12T07:30:13.001Z'],
    ['drop before allow', data => data.events[1].time = '2026-09-12T07:29:50.999Z'],
    ['malformed time', data => data.events[1].time = 'invalid'],
    ['outbound is not allowed', data => data.events[0].allow = false],
    ['inbound is not dropped', data => data.events[1].drop = false],
    ['wrong direction', data => data.events[1].direction = 'MS_FWP_DIRECTION_OUT'],
    ['not loopback', data => data.events[1].loopback = 'false'],
    ['different causal filter', data => data.events[1].filterId = '72898'],
    ['unrelated firewall block', data => data.filters[0].sublayer = 'FWPM_SUBLAYER_MPSSVC_WF'],
    ['non-blocking isolation rule', data => data.filters[0].action = 'FWP_ACTION_PERMIT'],
    ['wrong filter layer', data => data.filters[0].layer = 'FWPM_LAYER_ALE_AUTH_CONNECT_V4'],
  ];
  for (const [name, change] of cases) {
    const data = structuredClone(fixture);
    change(data);
    assert.throws(() => assertWfpLoopbackBlock(data, attempt), assert.AssertionError, name);
  }
  assert.equal(assertWfpLoopbackBlock(fixture, {
    ...attempt, start: Date.parse(fixture.events[0].time), end: Date.parse(fixture.events[1].time),
  }), '71179', 'both time boundaries are inclusive');
});

test('WFP proves actual external UDP blocking, not socket allocation or an arbitrary drop', () => {
  // Existing PS normalizer, actual raw-probe UDP events and filter from:
  // https://github.com/fran0220/gpui-box/actions/runs/34683324192/artifacts/10294437934
  const udpAttempt = {
    start: Date.parse('2026-09-12T08:29:38.049Z'), end: Date.parse('2026-09-12T08:29:38.049Z'),
    workerPid: 5028, sourcePort: 51164, sourceAddresses: ['10.1.0.129'],
    sid: 'S-1-15-2-1264071765-3781351782-1002261419-3028620457-728995327-1533812909-848223275',
    workerAppId: String.raw`\Device\HarddiskVolume4\Users\runneradmin\AppData\Local\Temp\gpui-js-qbqseu\worker.exe`,
  };
  assert.equal(assertWfpExternalUdpBlock(udpFixture, udpAttempt), '72861', 'inclusive time boundaries');
  const publicEvents = structuredClone(udpFixture);
  for (const event of publicEvents.events) delete event.pid;
  assert.equal(assertWfpExternalUdpBlock(publicEvents, udpAttempt), '72861');
  const cases = [
    ['allocation alone is not a drop', data => data.events.pop()],
    ['missing filter', data => data.filters.pop()],
    ['outbound allowed', data => data.events[1].drop = false],
    ['wrong protocol', data => data.events[1].protocol = '6'],
    ['non-host source address', data => data.events[1].localAddress = '10.1.0.130'],
    ['wrong source port', data => data.events[1].localPort = '51165'],
    ['different destination', data => data.events[1].remoteAddress = '192.0.2.2'],
    ['different destination port', data => data.events[1].remotePort = '10'],
    ['stale attempt', data => data.events[1].time = '2026-09-12T08:29:38.048Z'],
    ['after attempt', data => data.events[1].time = '2026-09-12T08:29:38.050Z'],
    ['invalid time', data => data.events[1].time = 'invalid'],
    ['wrong SID', data => data.events[1].sid = 'S-1-0-0'],
    ['wrong image', data => data.events[1].appId = fixture.events[0].appId],
    ['wrong PID', data => data.events[1].pid = '5029'],
    ['inbound drop', data => data.events[1].direction = 'MS_FWP_DIRECTION_IN'],
    ['loopback drop', data => data.events[1].loopback = 'true'],
    ['unresolved filter', data => data.events[1].filterId = '72846'],
    ['wrong layer', data => data.filters[0].layer = 'FWPM_LAYER_ALE_RESOURCE_ASSIGNMENT_V4'],
    ['ordinary firewall', data => data.filters[0].sublayer = 'FWPM_SUBLAYER_MPSSVC_WF'],
    ['non-blocking filter', data => data.filters[0].action = 'FWP_ACTION_PERMIT'],
  ];
  for (const [name, change] of cases) {
    const data = structuredClone(udpFixture);
    change(data);
    assert.throws(() => assertWfpExternalUdpBlock(data, udpAttempt), assert.AssertionError, name);
  }
});
