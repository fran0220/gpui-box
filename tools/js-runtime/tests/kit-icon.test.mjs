import { test } from 'node:test';
import assert from 'node:assert/strict';
import { builtinIconKeys } from '../kit-icon-catalog.mjs';
import { iconSchema } from '../kit-icon-schema.mjs';
import { validateValue } from '../kit-schema.mjs';

test('every built-in key supports the two weights and default', () => {
  assert.equal(new Set(builtinIconKeys).size, builtinIconKeys.length);
  assert.ok(builtinIconKeys.length > 50);
  for (const key of builtinIconKeys) {
    for (const descriptor of [{ key }, { key, weight: 'regular' }, { key, weight: 'fill' }]) {
      assert.doesNotThrow(() => validateValue(descriptor, iconSchema));
    }
  }
});

test('rejects noncanonical keys and every non-data escape hatch', () => {
  for (const descriptor of [
    {}, { key: 'ArrowLeft' }, { key: 'arrow-left.svg' }, { key: '../arrow-left' },
    { key: 'https://example.com/a.svg' }, { key: 'arrow-left', weight: 'bold' },
    { key: 'arrow-left', weight: null }, { key: 'arrow-left', path: 'a.svg' },
    { key: 'arrow-left', url: 'https://example.com/a.svg' },
    { get key() { throw Error('must not execute'); } },
    { key: 'arrow-left', [Symbol('hidden')]: true },
  ]) assert.throws(() => validateValue(descriptor, iconSchema), TypeError);
});
