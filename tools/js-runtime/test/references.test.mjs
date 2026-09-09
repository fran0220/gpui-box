import test from 'node:test';
import assert from 'node:assert/strict';
import { NativeReferences, validateNativeRef, validateReferenceInvocation } from '../references.mjs';
import { kitMethods, validateValue } from '../kit-schema.mjs';

const ref = (id, type = 'FocusHandle') => ({ $nativeRef: `native-${id}`, type });
test('references require issued identity, retain same-worker identity, and release explicitly', () => {
  const registry = new NativeReferences(), peer = new NativeReferences();
  const reference = registry.adopt({ child: ref(3) }).child;
  assert.equal(registry.adopt(ref(3)), reference);
  assert.equal(registry.target(reference, 'focus', {}, 'invoke'), reference);
  assert.throws(() => registry.target({ ...reference }, 'focus', {}, 'invoke'), /not issued/);
  assert.throws(() => peer.target(reference, 'focus', {}, 'invoke'), /not issued/);
  registry.release(reference);
  assert.throws(() => registry.target(reference, 'focus', {}, 'invoke'), /not issued/);
  const replacement = registry.adopt(ref(3));
  assert.notEqual(replacement, reference);
  registry.clear();
  assert.throws(() => registry.target(replacement, 'focus', {}, 'invoke'), /not issued/);
  assert.throws(() => registry.adopt(ref(4)), /revoked/);
});

test('reference kind fixes method schemas and command/query modes', () => {
  validateReferenceInvocation(ref(1), 'contains_focused', {}, 'query');
  validateReferenceInvocation(ref(2, 'TextInput'), 'set_text_quietly', { value: 'É🙂' }, 'invoke');
  for (const [method, args, mode] of [['focus', {}, 'query'], ['is_focused', {}, 'invoke'], ['focus', { extra: 1 }, 'invoke'], ['focus', {}, 'unknown']])
    assert.throws(() => validateReferenceInvocation(ref(1), method, args, mode));
  assert.throws(() => validateReferenceInvocation(ref(2, 'TextInput'), 'set_text_quietly', { value: 3 }, 'invoke'));
});

test('Menu references reuse family contracts, recursive identity checks and fixed methods', () => {
  const registry = new NativeReferences();
  const menu = registry.adopt(ref(5, 'Menu'));
  const args = { items: [{ kind: 'submenu', id: 'more', label: 'More', items: [{ kind: 'command', id: 'pin', label: 'Pin' }] }] };
  assert.equal(validateReferenceInvocation(menu, 'set_items', args, 'invoke'), kitMethods.Menu.invoke.set_items);
  assert.equal(registry.target(menu, 'open_submenu', { id: 'more' }, 'invoke'), menu);
  assert.throws(() => registry.target(menu, 'set_value', { value: 'wrong kind' }, 'invoke'));
  assert.throws(() => registry.target(menu, 'open', {}, 'query'));
  assert.throws(() => registry.target(menu, 'set_items', { items: [{ ...args.items[0], id: 'pin' }] }, 'invoke'), /duplicate identity/);
  assert.throws(() => registry.target(menu, 'set_items', { items: [{ ...args.items[0], extra: true }] }, 'invoke'));
  const result = validateReferenceInvocation(menu, 'open_submenu', { id: 'missing' }, 'invoke').result;
  validateValue(false, result);
  validateValue(true, result);
  assert.throws(() => validateValue('true', result));
  const focusSchema = validateReferenceInvocation(menu, 'focus_handle', {}, 'query').result;
  validateValue(ref(6), focusSchema);
  assert.throws(() => validateValue(ref(6, 'Menu'), focusSchema));
  registry.release(menu);
  assert.throws(() => registry.target(menu, 'is_open', {}, 'query'), /not issued/);
});

test('closed markers reject unknown kinds and accessors without evaluating callbacks', () => {
  for (const value of [null, ref(0), ref(1, 'Window'), { ...ref(1), extra: true }, { $nativeRef: 'native-01', type: 'FocusHandle' }])
    assert.throws(() => validateNativeRef(value));
  assert.throws(() => validateNativeRef({ get $nativeRef() { assert.fail('must not invoke accessor'); }, type: 'FocusHandle' }), /Invalid native reference fields/);
});

test('failed response preflight cannot consume capacity or partly install references', () => {
  const registry = new NativeReferences();
  assert.throws(() => registry.adopt([ref(1), ref(2, 'Window')]));
  const values = registry.adopt(Array.from({ length: 128 }, (_, i) => ref(i + 10)));
  assert.equal(values.length, 128);
  assert.throws(() => registry.adopt(ref(999)), /quota/);
  assert.throws(() => registry.adopt(ref(10, 'TextInput')), /type changed/);
  registry.release(values[7]);
  assert.equal(registry.target(registry.adopt(ref(999)), 'is_focused', {}, 'query').$nativeRef, 'native-999');
});
