import test from 'node:test';
import assert from 'node:assert/strict';
import { dragItemSchema } from '../kit-drag-schema.mjs';
import { validateValue } from '../kit-schema.mjs';

test('bare native drag data rejects target geometry and external file authority', () => {
  const item = { id: 'row.beta', source: 'source.panel', label: 'Beta', kind: 'custom-row', icon: null };
  validateValue(item, dragItemSchema);
  validateValue({ ...item, icon: { key: 'check', weight: 'fill' } }, dragItemSchema);
  for (const extra of [{ anchor: 'target' }, { velocity: { x: 2, y: -3 } }, { path: '/host/file' }, { $resource: 'fake' }]) {
    assert.throws(() => validateValue({ ...item, ...extra }, dragItemSchema));
  }
  assert.throws(() => validateValue({ ...item, icon: { key: '/host/icon' } }, dragItemSchema));
  const { icon, ...missing } = item;
  assert.throws(() => validateValue(missing, dragItemSchema));
});
