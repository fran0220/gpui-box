import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { validateValue, validateSlots, generateKitMethodTypes } from '../kit-schema.mjs';

const fixtures = JSON.parse(readFileSync(new URL('./schema-fixtures.json', import.meta.url), 'utf8'));

test('oneOf matches exactly one branch recursively using the native parity cases', () => {
  for (const { name, schema, cases } of fixtures.unions) for (const [value, valid] of cases) {
    if (valid) assert.doesNotThrow(() => validateValue(value, schema), name);
    else assert.throws(() => validateValue(value, schema), TypeError, name);
  }
  assert.throws(() => validateValue(null, { oneOf: [] }), /oneOf/);
  assert.throws(() => validateValue(null, { oneOf: {} }), /oneOf/);
});

test('overlapping union attempts never evaluate accessors or accept malformed data', () => {
  let reads = 0;
  const value = { kind: 'text', get text() { reads++; return 'Executed'; } };
  assert.throws(() => validateValue({ value }, fixtures.unions[0].schema), /one matching/);
  assert.equal(reads, 0);
  const values = [];
  Object.defineProperty(values, '0', { enumerable: true, get() { reads++; return 1; } });
  assert.throws(() => validateValue({ value: { kind: 'counts', values } }, fixtures.unions[0].schema));
  assert.equal(reads, 0);
});

test('suffix-qualified dynamic slots match native parity cases and reject accessors', () => {
  for (const { schema, props, cases } of fixtures.slots) for (const [name, valid] of cases) {
    if (valid) assert.doesNotThrow(() => validateSlots(schema, props, { [name]: [] }));
    else assert.throws(() => validateSlots(schema, props, { [name]: [] }), /slots/);
  }
  let reads = 0;
  assert.throws(() => validateSlots(fixtures.slots[0].schema, fixtures.slots[0].props, { get 'alpha:content'() { reads++; return []; } }), /slots/);
  assert.equal(reads, 0);
});

test('generated method types compile nested discriminated unions and reject wrong branch fields', () => {
  const dir = mkdtempSync(join(tmpdir(), 'gpui-union-types-'));
  try {
    const path = join(dir, 'contract.ts');
    const sdk = fileURLToPath(new URL('../kit-sdk', import.meta.url));
    const generated = generateKitMethodTypes({ TextInput: { invoke: { probe: { args: fixtures.unions[0].schema, result: { enum: [null] } } }, query: {} } });
    writeFileSync(path, `import type { KitNode } from ${JSON.stringify(sdk)};\n${generated}
declare const invoke: KitInvoke;
declare const target: KitNode<'TextInput'>;
const answer: Promise<null> = invoke(target, 'probe', {value:{kind:'counts',values:[-3,'unknown',7]}});
invoke(target, 'probe', {value:{kind:'text',text:'Ready'}});
// @ts-expect-error wrong branch field
invoke(target, 'probe', {value:{kind:'counts',text:'Wrong'}});
// @ts-expect-error malformed nested union value
invoke(target, 'probe', {value:{kind:'counts',values:[false]}});
// @ts-expect-error required argument omitted
invoke(target, 'probe');
`);
    const compiler = fileURLToPath(new URL('../../app-host/node_modules/typescript/bin/tsc', import.meta.url));
    const result = spawnSync(process.execPath, [compiler, '--strict', '--noEmit', '--skipLibCheck', path], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stdout + result.stderr);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
