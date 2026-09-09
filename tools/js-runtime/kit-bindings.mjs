import { kitSchemas, validateKitProps, validateKitSlots, validateValue } from './kit-schema.mjs';
import { validatePayload } from './wire.mjs';

const bindings = Object.freeze({ Checkbox: ['checked', 'change', 'boolean'], Switch: ['on', 'change', 'boolean'], Slider: ['value', 'change', 'number'], SegmentedControl: ['selected', 'select', 'string'], TextInput: ['text', 'change', 'string'], Select: ['selected', 'change', 'selection'] });

function validateHandlers(component, handlers, kind = 'events') {
  if (!handlers || Object.getPrototypeOf(handlers) !== Object.prototype) throw new TypeError('Expected event handlers');
  for (const key of Reflect.ownKeys(handlers)) {
    if (!Object.hasOwn(kitSchemas[component][kind] ?? {}, key) || typeof Object.getOwnPropertyDescriptor(handlers, key)?.value !== 'function') throw new TypeError(`${component}: invalid ${kind} ${String(key)}`);
  }
}

function readBinding(state) {
  if (!state || typeof state.get !== 'function' || typeof state.set !== 'function') throw new TypeError('Expected caller-owned state with get/set');
  return state.get();
}

function sameJSON(left, right) {
  if (left === right) return true;
  if (!left || !right || typeof left !== 'object' || typeof right !== 'object' || Array.isArray(left) !== Array.isArray(right)) return false;
  const keys = Object.keys(left);
  return keys.length === Object.keys(right).length && keys.every(key => Object.hasOwn(right, key) && sameJSON(left[key], right[key]));
}

/** Registrar owns action lifetime, revision/generation, and disposal. */
export function createKitBindings(registerHandler, registerPredicate) {
  if (typeof registerHandler !== 'function') throw new TypeError('Expected handler registrar');
  const api = {};
  for (const [component, schema] of Object.entries(kitSchemas)) {
    api[component] = (id, props = {}, handlers = {}, slots = {}, predicates = {}) => {
      validateKitProps(component, id, props);
      validateKitSlots(component, props, slots);
      validateHandlers(component, handlers);
      validateHandlers(component, predicates, 'predicates');
      if (Object.keys(predicates).length && typeof registerPredicate !== 'function') throw new TypeError('Predicate registrar unavailable');
      const events = {};
      // Do not retain callable actions for a refused control.
      if (!props.disabled) for (const [name, handler] of Object.entries(handlers)) {
        events[name] = registerHandler(id, name, (payload) => {
          validateValue(payload, schema.events[name], `${component}.${name}`);
          return handler(payload);
        });
        if (typeof events[name] !== 'string' || !events[name]) throw new TypeError('Registrar must return an action identity');
      }
      const refs = {};
      if (!props.disabled) for (const [name, callback] of Object.entries(predicates)) {
        refs[name] = registerPredicate(id, component, name, payload => {
          validateValue(payload, schema.predicates[name], `${component}.${name}`);
          const decision = callback(payload);
          const checked = value => {
            if (typeof value !== 'boolean') throw new TypeError('Predicate must return a boolean');
            return value;
          };
          if (decision instanceof Promise) return decision.then(checked);
          return checked(decision);
        });
        if (typeof refs[name] !== 'string' || !refs[name] || refs[name].length > 256) throw new TypeError('Predicate registrar must return an opaque identity');
      }
      return { kind: 'kit', component, id, props: structuredClone(props), slots: structuredClone(slots), events, ...(Object.keys(refs).length ? { predicates: refs } : {}) };
    };
  }
  // Adapted JS semantics: no Binding/Signal object crosses the native wire.
  api.bind = (component, id, state, props = {}, handlers = {}, slots = {}) => {
    if (!Object.hasOwn(bindings, component)) throw new TypeError('Unsupported bound component');
    validateKitProps(component, id, props);
    validateHandlers(component, handlers);
    const [property, event, type] = bindings[component];
    const value = readBinding(state);
    if (type === 'selection' ? value !== null && typeof value !== 'string' : typeof value !== type) throw new TypeError('Incorrect binding value type');
    return api[component](id, { ...props, [property]: value }, { ...handlers, [event]: value => state.set(value) }, slots);
  };
  api.bind_value = (component, id, state, value, props = {}, handlers = {}) => {
    if (component !== 'Radio') throw new TypeError('bind_value requires Radio');
    validateKitProps(component, id, props);
    validateHandlers(component, handlers);
    const current = validatePayload(structuredClone(readBinding(state)));
    const choice = validatePayload(structuredClone(value));
    return api.Radio(id, { ...props, selected: sameJSON(current, choice) }, { ...handlers, select: () => state.set(structuredClone(choice)) });
  };
  return Object.freeze(api);
}
