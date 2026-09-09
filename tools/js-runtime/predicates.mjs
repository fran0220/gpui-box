import { validatePayload } from './wire.mjs';
import { kitSchemas, validateValue } from './kit-schema.mjs';

export const PREDICATE_TIMEOUT = 3000;
export const PREDICATE_LIMIT = 32;

export function predicateTarget(tree, target, name, reference) {
  function find(node) {
    if (node?.id === target?.id) return node;
    for (const child of [...(node?.children ?? []), ...Object.values(node?.slots ?? {}).flat()]) {
      const found = find(child); if (found) return found;
    }
  }
  const node = find(tree);
  if (!node || node.kind !== 'kit' || node.component !== target?.component ||
      !['List', 'Tabs', 'Tree'].includes(node.component) || name !== 'accepts' ||
      typeof reference !== 'string' || node.predicates?.[name] !== reference)
    throw new Error('Predicate target or reference is not mounted');
  if (node.props.disabled) throw new Error('Predicate target is disabled');
  return node;
}

export function validatePredicatePayload(component, payload) {
  validatePayload(payload);
  validateValue(payload, kitSchemas[component].predicates.accepts, 'DropIntent');
}

/** Callbacks never leave this worker. References are identities, not authority. */
export class WorkerPredicates {
  constructor(context) {
    this.context = context;
    this.callbacks = new Map();
    this.pending = new Map();
    this.sequence = 0;
  }
  register(id, component, name, callback) {
    if (!/^[\w.-]{1,120}$/.test(id) || !['List', 'Tabs', 'Tree'].includes(component) ||
        name !== 'accepts' || typeof callback !== 'function') throw new Error('Invalid predicate registration');
    const key = `${component}:${id}:${name}`;
    const previous = this.callbacks.get(key);
    if (!previous && this.callbacks.size >= 1000) throw new Error('Predicate registry limit exceeded');
    const reference = previous?.reference ?? `predicate-${++this.sequence}`;
    this.callbacks.set(key, { reference, callback });
    return reference;
  }
  cancel(reason, id) {
    if (id !== undefined) {
      this.pending.get(id)?.reject(new Error(reason));
      this.pending.delete(id);
      return;
    }
    for (const call of this.pending.values()) call.reject(new Error(reason));
    this.pending.clear();
  }
  dispose() {
    this.cancel('Predicate cancelled: worker disposed');
    this.callbacks.clear();
  }
  async evaluate(request) {
    const { id, revision, target, name, reference, payload, deadline } = request;
    if (!Number.isSafeInteger(id) || id < 1 || this.pending.has(id) || this.pending.size >= PREDICATE_LIMIT)
      throw new Error('Invalid or excessive predicate requests');
    const now = Date.now();
    if (!Number.isSafeInteger(deadline) || deadline <= now || deadline > now + PREDICATE_TIMEOUT)
      throw new Error('Predicate deadline expired or invalid');
    const check = () => {
      const context = this.context();
      if (context.disposed || context.revision !== revision) throw new Error('Predicate cancelled: revision changed or worker disposed');
      predicateTarget(context.tree, target, name, reference);
    };
    check();
    validatePredicatePayload(target.component, payload);
    const registered = this.callbacks.get(`${target.component}:${target.id}:${name}`);
    if (!registered || registered.reference !== reference) throw new Error('Predicate callback unavailable');
    let timer, call;
    const cancelled = new Promise((_, reject) => {
      call = { reject };
      this.pending.set(id, call);
      timer = setTimeout(() => reject(new Error('Predicate deadline expired')), deadline - now);
    });
    try {
      const value = await Promise.race([Promise.resolve().then(() => {
        if (this.pending.get(id) !== call) throw new Error('Predicate cancelled');
        check();
        return registered.callback(payload);
      }), cancelled]);
      if (this.pending.get(id) !== call) throw new Error('Predicate cancelled');
      check();
      if (Date.now() >= deadline) throw new Error('Predicate deadline expired');
      if (typeof value !== 'boolean') throw new Error('Predicate must return a boolean');
      return value;
    } finally {
      clearTimeout(timer);
      if (this.pending.get(id) === call) this.pending.delete(id);
    }
  }
}
