import test from 'node:test';
import assert from 'node:assert/strict';
import { WorkerPredicates, PREDICATE_LIMIT } from '../predicates.mjs';

function fixture(callback) {
  const context = { revision: 7, disposed: false };
  const registry = new WorkerPredicates(() => context);
  const reference = registry.register('rows', 'List', 'accepts', callback);
  context.tree = { kind: 'kit', id: 'rows', component: 'List', props: {}, predicates: { accepts: reference } };
  const request = (patch = {}) => ({ id: 1, revision: 7, target: { id: 'rows', component: 'List' }, name: 'accepts', reference, payload: { id: 'record-9' }, deadline: Date.now() + 1000, ...patch });
  return { context, registry, request, reference };
}

test('predicate references retain latest callback, exact target identity, and literal boolean results', async () => {
  const { registry, request, reference, context } = fixture(payload => payload.id === 'record-9');
  assert.equal(await registry.evaluate(request()), true);
  assert.equal(registry.register('rows', 'List', 'accepts', async () => false), reference);
  assert.equal(await registry.evaluate(request()), false);
  await assert.rejects(registry.evaluate(request({ target: { id: 'rows', component: 'Tabs' } })), /not mounted/);
  await assert.rejects(registry.evaluate(request({ reference: 'forged' })), /not mounted/);
  registry.register('rows', 'List', 'accepts', () => 'true');
  await assert.rejects(registry.evaluate(request()), /must return a boolean/);
  context.tree.props.disabled = true;
  await assert.rejects(registry.evaluate(request()), /disabled/);
});

test('revision invalidation rejects an in-flight predicate even if the same descriptor stays mounted', async () => {
  let finish;
  const { registry, request, context } = fixture(() => new Promise(resolve => { finish = resolve; }));
  const pending = registry.evaluate(request());
  await Promise.resolve();
  context.revision++;
  registry.cancel('Predicate cancelled by render');
  await assert.rejects(pending, /cancelled/);
  finish(true);
  assert.equal(registry.pending.size, 0);
  await assert.rejects(registry.evaluate(request()), /revision changed/);
});

test('predicate capacity, deadline, removal and disposal fail closed', async () => {
  let finish;
  const { registry, request, context } = fixture(() => new Promise(resolve => { finish = resolve; }));
  const pending = registry.evaluate(request());
  await Promise.resolve();
  context.tree.predicates = {};
  finish(true);
  await assert.rejects(pending, /not mounted/);
  context.tree.predicates = { accepts: request().reference };
  await assert.rejects(registry.evaluate(request({ deadline: Date.now() + 15 })), /deadline expired/);
  const calls = Array.from({ length: PREDICATE_LIMIT }, (_, id) => registry.evaluate(request({ id: id + 1 })));
  const rejected = calls.map(call => assert.rejects(call, /disposed/));
  await assert.rejects(registry.evaluate(request({ id: 99 })), /excessive/);
  await assert.rejects(registry.evaluate(request({ id: 1 })), /excessive/);
  context.disposed = true;
  registry.dispose();
  await Promise.all(rejected);
  assert.equal(registry.callbacks.size, 0);
  assert.equal(registry.pending.size, 0);
});
