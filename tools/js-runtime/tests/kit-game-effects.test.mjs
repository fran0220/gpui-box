import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { familySchemas, validateProps } from '../kit-game-effects-schema.mjs';
import { familySchemas as agentSchemas, validateProps as validateAgent } from '../kit-agent-schema.mjs';
import { validateValue } from '../kit-schema.mjs';

test('every family fixture satisfies the shared closed grammar and native fixture parity', async () => {
  for (const [family, schemas, validate] of [['agent', agentSchemas, validateAgent], ['game_effects', familySchemas, validateProps]]) {
    const { cases } = await import(`../../app-host/src/kit_bindings/${family}/fixture/cases.mjs`);
    assert.deepEqual(cases.map(item => item.component).sort(), Object.keys(schemas).sort());
    for (const item of cases) {
      validateValue(item.props, schemas[item.component].props);
      validate(item.component, item.props);
      assert.throws(() => validateValue({ ...item.props, serviceExecute: true }, schemas[item.component].props));
    }
    const expected = JSON.stringify(cases.map(item => ({ kind: 'kit', ...item })), null, 2) + '\n';
    const file = new URL(`../../app-host/src/kit_bindings/${family}/fixture/cases.json`, import.meta.url);
    if (process.env.KIT_WRITE_TYPES === '1') writeFileSync(file, expected);
    assert.equal(readFileSync(file, 'utf8'), expected);
  }
});

test('ability charge relation accepts equality and rejects overflow and zero maximum', () => {
  const check = (current, maximum) => {
    const props = { abilities: [{ id: 'west', label: 'West', state: { kind: 'ready' }, charges: { current, maximum } }] };
    validateValue(props, familySchemas.AbilityBar.props);
    validateProps('AbilityBar', props);
  };
  check(3, 3);
  check(2, 7);
  assert.throws(() => check(4, 3));
  assert.throws(() => check(0, 0));
});

test('game data never accepts native handles, path images or fabricated cooldown fields', () => {
  const check = props => validateValue(props, familySchemas.AbilityBar.props);
  assert.throws(() => check({ abilities: [{ id: 'x', label: 'X', state: { kind: 'ready', remaining: '2s' } }] }));
  assert.throws(() => check({ abilities: [{ id: 'x', label: 'X', state: { kind: 'ready' }, icon: { key: 'missing-native-icon' } }] }));
  assert.throws(() => validateProps('PartyRoster', { members: [{ image: { key: '/tmp/secret' } }] }));
  assert.throws(() => validateValue({ clip: {}, plan: {} }, familySchemas.CinematicEffect.props));
});
