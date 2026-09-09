import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { familySchemas, familyMethods } from '../kit-media-schema.mjs';
import { validateValue } from '../kit-schema.mjs';

test('media membership exactly matches the native catalog and fixtures', () => {
  const catalog=JSON.parse(readFileSync(new URL('../../../docs/api-index.json',import.meta.url)));
  assert.deepEqual(Object.keys(familySchemas).sort(),catalog.components.filter(c=>c.path.startsWith('gpui_kit::media::')).map(c=>c.name).sort());
  const fixtures=JSON.parse(readFileSync(new URL('../../app-host/src/kit_bindings/media/fixture/nodes.json',import.meta.url)));
  for(const node of fixtures) validateValue(node.props,familySchemas[node.component].props);
  assert.deepEqual(familyMethods,{});
});

test('sample boundaries and absent resource authority are enforced before native rendering',()=>{
  const wave=familySchemas.AudioWaveform.props;
  validateValue({peaks:[0,0.15,1],playhead:0.3},wave);
  for(const peak of [-0.01,1.01,NaN,Infinity])assert.throws(()=>validateValue({peaks:[peak]},wave),/invalid number/);
  for(const [name,schema]of Object.entries(familySchemas)){
    for(const key of ['source','url','path','transport','decoder','frame'])assert.throws(()=>validateValue({[key]:'untrusted'},schema.props),/unknown field/);
    assert.equal(schema.events.played,undefined,`${name} must not invent playback`);
  }
  assert.throws(()=>validateValue({document:'x'.repeat(16385)},familySchemas.ModelViewer.props),/string length/);
});
