import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { familySchemas, validateFamilyProps } from '../kit-navigation_extra-schema.mjs';
import { validateValue } from '../kit-schema.mjs';
import { artifacts, families } from '../../app-host/src/kit_bindings/navigation_extra/fixture/generate.mjs';

test('explicit adapters cover the owned families except documented unbound components', async () => {
  const index = JSON.parse(await readFile(new URL('../../../docs/api-index.json', import.meta.url)));
  const coverage = JSON.parse(await readFile(new URL('../binding-coverage.json', import.meta.url)));
  // These mobile Rust components have no JS adapters; do not imply them from a family.
  const unbound = ['BottomNavigation', 'AppBar', 'PageLayout'];
  for (const name of unbound) {
    assert.equal(coverage.components.find(v => v.name === name).binding.status, 'unbound');
  }
  const excluded = new Set(['Pagination', 'Tabs', 'Accordion', 'ScrollArea', 'SplitPane', ...unbound]);
  for (const [family, source] of [['navigation_extra', 'navigation'], ['layout_extra', 'layout'], ['datetime', 'datetime']]) {
    const names = index.components.filter(v => v.source.startsWith(`crates/gpui-kit/src/${source}/`) && !excluded.has(v.name)).map(v => v.name).sort();
    assert.deepEqual(Object.keys(families[family].familySchemas).sort(), names);
  }
  await artifacts();
});
test('history and sidebar topology reject invalid state without inventing data', () => {
  const check = (component, props) => { validateValue(props, familySchemas[component].props); validateFamilyProps(component, props); };
  check('NavStack', { entries: [{ id: 'root' }, { id: 'a' }, { id: 'b' }], cursor: 1, label: 'Visits' });
  for (const props of [{ entries: [], cursor: 0, label: 'Visits' }, { entries: [{ id: 'a' }], cursor: 1, label: 'Visits' }, { entries: [{ id: 'a' }, { id: 'a' }], cursor: 0, label: 'Visits' }]) assert.throws(() => check('NavStack', props));
  const sections = [{ id: 'one' }, { id: 'two' }];
  check('Sidebar', { sections, items: [{ id: 'parent', label: 'P', section: 'one' }, { id: 'child', label: 'C', within: 'parent', section: 'one' }] });
  for (const within of ['missing', 'child', 'other']) assert.throws(() => check('Sidebar', { sections, items: [{ id: 'child', label: 'C', section: 'one', within }, { id: 'other', label: 'O', section: 'two' }] }));
  assert.throws(() => check('Sidebar', { items: [{ id: 'a', label: 'A', section: 'one', image: '/etc/passwd' }] }));
});
test('navigation event variants are exact, not kind plus optional fields', () => {
  const schema = familySchemas.Wizard.events.navigate;
  validateValue({ kind: 'step', id: 'review' }, schema);
  validateValue({ kind: 'finish' }, schema);
  for (const value of [{ kind: 'step' }, { kind: 'finish', id: 'unexpected' }, { kind: 'next', path: '/tmp/x' }]) assert.throws(() => validateValue(value, schema));
});
test('all family SDKs compile exact constructor, event and method types', () => {
  const result = spawnSync(process.execPath, [fileURLToPath(new URL('../../app-host/node_modules/typescript/bin/tsc', import.meta.url)), '--noEmit', '--strict', '--skipLibCheck', '--module', 'NodeNext', '--moduleResolution', 'NodeNext', '--target', 'ES2022', fileURLToPath(new URL('../../app-host/src/kit_bindings/navigation_extra/fixture/types.mts', import.meta.url))], { encoding: 'utf8' });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stdout + result.stderr);
});
