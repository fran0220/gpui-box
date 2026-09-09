import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { validateValue, validateSlots, generateKitMethodTypes, schemaType, schemaDefinitions } from '../kit-schema.mjs';

const fixtures = JSON.parse(readFileSync(new URL('./schema-fixtures.json', import.meta.url), 'utf8'));

test('local references preserve recursive tags, nullable definitions and exact-one parity', () => {
  for (const { name, schema, cases } of fixtures.references) for (const [value, valid] of cases) {
    if (valid) assert.doesNotThrow(() => validateValue(value, schema), name);
    else assert.throws(() => validateValue(value, schema), TypeError, name);
  }
  for (const schema of fixtures.invalidReferences) {
    assert.throws(() => validateValue(null, schema), TypeError);
    assert.throws(() => schemaType(schema, 'InvalidDefinitions'), TypeError);
    assert.throws(() => schemaDefinitions(schema, 'InvalidDefinitions'), TypeError);
  }
});

test('data depth and aggregate union work enforce both sides of shared native boundaries', () => {
  let value = null;
  for (let depth = 0; depth < 32; depth++) value = { next: value };
  assert.doesNotThrow(() => validateValue(value, fixtures.depthSchema));
  assert.throws(() => validateValue({ next: value }, fixtures.depthSchema), /budget/);
  // Each element costs union + ref + boolean + failed null branch = 4,
  // plus one for the root array. Work must not reset at refs or branches.
  assert.doesNotThrow(() => validateValue(Array(24999).fill(true), fixtures.workSchema));
  assert.throws(() => validateValue(Array(25000).fill(true), fixtures.workSchema), /budget/);
  const cycle = {};
  cycle.next = cycle;
  assert.throws(() => validateValue(cycle, fixtures.depthSchema), /budget/);
});

test('recursive data and definition accessors are rejected without execution', () => {
  let reads = 0;
  const value = { get next() { reads++; return null; } };
  assert.throws(() => validateValue(value, fixtures.depthSchema), /accessor/);
  const schema = { $defs: { get Link() { reads++; return {}; } }, $ref: 'Link' };
  assert.throws(() => validateValue(null, schema), /accessor/);
  assert.throws(() => schemaDefinitions(schema, 'Definitions'), /accessor/);
  assert.equal(reads, 0);
});

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
    const generated = generateKitMethodTypes({ TextInput: { invoke: { probe: { args: fixtures.unions[0].schema, result: { enum: [null] } }, tree: { args: fixtures.references[0].schema, result: fixtures.depthSchema } }, query: {} } });
    writeFileSync(path, `import type { KitNode } from ${JSON.stringify(sdk)};\n${generated}
${schemaDefinitions(fixtures.references[0].schema, 'TreeDefinitions')}
type TreeProps = ${schemaType(fixtures.references[0].schema, 'TreeDefinitions')};
const tree: TreeProps = {value:{kind:'branch',children:[{kind:'leaf',text:'Alpha'}]}};
// @ts-expect-error recursive tag must match the nested branch
const invalidTree: TreeProps = {value:{kind:'branch',children:[{kind:'leaf',children:[]}]}};
type FamilyProps = ${schemaType(fixtures.unions[0].schema)};
const props: FamilyProps = {value:{kind:'counts',values:[-3,'unknown',7]}};
// @ts-expect-error family props preserve nested union branches
const invalidProps: FamilyProps = {value:{kind:'counts',values:[false]}};
declare const invoke: KitInvoke;
declare const target: KitNode<'TextInput'>;
const recursiveResult: Promise<{next: unknown} | null> = invoke(target, 'tree', tree);
// @ts-expect-error method args preserve local recursive definitions
invoke(target, 'tree', {value:{kind:'branch',children:[false]}});
invoke(target, 'probe', props);
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

test('catalog serialization and recursive declarations grow linearly without unfolding definitions', () => {
  const schema = fixtures.references[0].schema;
  const make = count => Object.fromEntries(Array.from({ length: count }, (_, i) => [`Tree${i}`, { props: schema, events: {} }]));
  const small = JSON.stringify(make(8));
  const large = JSON.stringify(make(16));
  assert.ok(large.length < small.length * 2.02);
  assert.ok(JSON.stringify(schema).length < 1024);
  const catalog = JSON.parse(large);
  assert.deepEqual(catalog.Tree0.props, schema);
  assert.doesNotThrow(() => validateValue(fixtures.references[0].cases[0][0], catalog.Tree0.props));
  const declarations = schemaDefinitions(schema, 'TreeDefinitions') + schemaType(schema, 'TreeDefinitions');
  assert.ok(declarations.length < 1024);
  assert.throws(() => schemaType(schema), /Named TypeScript definitions/);
});
