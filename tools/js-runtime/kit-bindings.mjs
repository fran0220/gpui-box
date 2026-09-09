import { kitSchemas, validateKitProps, validateKitSlots, validateValue } from './kit-schema.mjs';

/** Registrar owns action lifetime, revision/generation, and disposal. */
export function createKitBindings(registerHandler) {
  if (typeof registerHandler !== 'function') throw new TypeError('Expected handler registrar');
  const api = {};
  for (const [component, schema] of Object.entries(kitSchemas)) {
    api[component] = (id, props = {}, handlers = {}, slots = {}) => {
      validateKitProps(component, id, props);
      validateKitSlots(component, props, slots);
      if (!handlers || Object.getPrototypeOf(handlers) !== Object.prototype) throw new TypeError('Expected event handlers');
      for (const key of Reflect.ownKeys(handlers)) {
        if (!Object.hasOwn(schema.events, key) || typeof Object.getOwnPropertyDescriptor(handlers, key)?.value !== 'function') throw new TypeError(`${component}: invalid event ${String(key)}`);
      }
      const events = {};
      // Do not retain callable actions for a refused control.
      if (!props.disabled) for (const [name, handler] of Object.entries(handlers)) {
        events[name] = registerHandler(id, name, (payload) => {
          validateValue(payload, schema.events[name], `${component}.${name}`);
          return handler(payload);
        });
        if (typeof events[name] !== 'string' || !events[name]) throw new TypeError('Registrar must return an action identity');
      }
      return { kind: 'kit', component, id, props: structuredClone(props), slots: structuredClone(slots), events };
    };
  }
  return Object.freeze(api);
}
