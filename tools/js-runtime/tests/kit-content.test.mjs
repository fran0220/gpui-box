import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { familySchemas, familyMethods, validateDescriptor } from '../kit-content-schema.mjs';
import { validateValue } from '../kit-schema.mjs';

test('cross-field contracts reject ambiguous data and unowned resource locators', () => {
  const check = (component, props, slots = {}) => {
    validateValue(props, familySchemas[component].props);
    validateDescriptor({ component, props, slots });
  };
  assert.throws(() => check('CodeView', { text: 'a', lines: [{ number: 3, text: 'b' }] }), /choose text or lines/);
  assert.throws(() => check('CodeView', { lines: [{ number: 3, text: 'a' }, { number: 3, text: 'b' }] }), /duplicate line/);
  assert.throws(() => check('AgentDocument', { blocks: [{ id: 'code', kind: 'code' }] }), /needs its slot/);
  assert.throws(() => check('AgentDocument', { blocks: [{ id: 'text', kind: 'text' }] }), /text required/);
  check('AgentDocument', { blocks: [{ id: 'code', kind: 'code' }] }, { code: [] });
  assert.throws(() => check('ImageViewer', { minZoom: 3, maxZoom: 2 }), /zoom range/);
  check('Markdown', { source: '![approved](picture)', images: [{ src: 'picture', resource: { key: 'pixels' } }] });
  for (const resource of [{ key: '../pixels' }, { key: 'https://example.test/a' }, { key: 'pixels', path: '/tmp/a' }]) {
    assert.throws(() => check('Markdown', { source: '![x](x)', images: [{ src: 'x', resource }] }));
  }
});

test('content membership and every native fixture use the closed executable contract', () => {
  const catalog = JSON.parse(readFileSync(new URL('../../../docs/api-index.json', import.meta.url)));
  assert.deepEqual(Object.keys(familySchemas).sort(), catalog.components.filter(c => c.path.startsWith('gpui_kit::content::')).map(c => c.name).sort());
  const fixtures = JSON.parse(readFileSync(new URL('../../app-host/src/kit_bindings/content/fixture/nodes.json', import.meta.url)));
  for (const node of fixtures) validateValue(node.props, familySchemas[node.component].props);
});

test('reject executable and IO descriptors, invalid numbers, duplicate IDs and accessors', () => {
  for (const [component, schema] of Object.entries(familySchemas)) {
    const required = component === 'Markdown' ? { source: 'caller' } : {};
    for (const forbidden of ['path', 'process', 'transport', 'webview', 'loader']) assert.throws(() => validateValue({ ...required, [forbidden]: '/etc/passwd' }, schema.props), /unknown field/);
  }
  assert.throws(() => validateValue({ lines: [{ number: -1, text: 'bad' }] }, familySchemas.CodeView.props), /invalid number/);
  assert.throws(() => validateValue({ messages: [{ id: 'same', text: 'a' }, { id: 'same', text: 'b' }] }, familySchemas.MessageList.props), /duplicate identity/);
  assert.throws(() => validateValue({ get source() { assert.fail('getter executed'); } }, familySchemas.Markdown.props), /accessor/);
  assert.throws(() => validateValue({ source: 'x', streaming: 'yes' }, familySchemas.Markdown.props), /boolean/);
  assert.throws(() => validateValue({ block: 'answer', owner: 'spoof' }, familyMethods.AgentDocument.invoke.remeasure_block.args), /unknown field/);
  assert.throws(() => validateValue({ command: 'copy' }, familyMethods.CodeView.query.text.args), /unknown field/);
});

test('generated content/media declarations typecheck negative native IO and method cases', () => {
  const generator = fileURLToPath(new URL('../../app-host/src/kit_bindings/content/fixture/generate-types.mjs', import.meta.url));
  const generated = spawnSync(process.execPath, [generator, '--check'], { encoding: 'utf8' });
  assert.equal(generated.status, 0, generated.stdout + generated.stderr);
  const dir = mkdtempSync(join(tmpdir(), 'gpui-content-types-'));
  try {
    const content = fileURLToPath(new URL('../kit-content-sdk', import.meta.url));
    const media = fileURLToPath(new URL('../kit-media-sdk', import.meta.url));
    const path = join(dir, 'types.ts');
    writeFileSync(path, `import type {ContentFactories,ContentMethodContracts} from ${JSON.stringify(content)};
import type {MediaFactories} from ${JSON.stringify(media)};
declare const content:ContentFactories;
declare const media:MediaFactories;
content.Markdown('document',{source:'# Caller'});
content.AgentDocument('document',{blocks:[{id:'answer',kind:'markdown',text:'verified',revision:4}]});
media.AudioWaveform('wave',{peaks:[0.2,0.8],playhead:0.3});
const named:ContentMethodContracts['AgentDocument']['invoke']['remeasure_block']['args']={block:'answer'};
// @ts-expect-error native IO is not granted by string
media.AudioPlayer('audio',{source:'/etc/passwd'});
// @ts-expect-error arbitrary callbacks do not cross the descriptor boundary
content.Markdown('doc',{source:'text',highlight(){return []}});
// @ts-expect-error closed enum
content.CodeView('code',{lines:[{number:4,text:'code',mark:'good'}]});
// @ts-expect-error unknown method
type Bad=ContentMethodContracts['CodeView']['invoke']['copy'];
// @ts-expect-error precise query response
const bad:ContentMethodContracts['CodeView']['query']['text']['result']=4;
// @ts-expect-error media has no shell transport
media.VideoPlayer('video',{transport:'shell'});
`);
    const compiler = fileURLToPath(new URL('../../app-host/node_modules/typescript/bin/tsc', import.meta.url));
    const result = spawnSync(process.execPath, [compiler, '--strict', '--noEmit', path], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stdout + result.stderr);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
