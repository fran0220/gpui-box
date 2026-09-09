// Native references are issued by the host, never constructed by the SDK user.
// This worker-side identity check is not authority: native dispatch independently
// revalidates the issuing worker, current mount, parent and weak target.
import { validateInvocation } from './kit-schema.mjs';
import { validatePayload } from './wire.mjs';

export function validateNativeRef(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid native reference');
  const fields = Object.getOwnPropertyDescriptors(value);
  if (Reflect.ownKeys(fields).length !== 2 || !fields.$nativeRef || !fields.type ||
      !Object.hasOwn(fields.$nativeRef, 'value') || !Object.hasOwn(fields.type, 'value') ||
      typeof fields.$nativeRef.value !== 'string' || !/^native-[1-9][0-9]{0,19}$/.test(fields.$nativeRef.value) ||
      !['TextInput', 'FocusHandle'].includes(fields.type.value)) throw new Error('Invalid native reference fields');
  return value;
}

export function validateReferenceInvocation(reference, method, args, mode) {
  validateNativeRef(reference);
  validatePayload(args);
  if (!['invoke', 'query'].includes(mode)) throw new Error('Invalid native reference mode');
  if (method === '$release' && mode === 'invoke' && args && typeof args === 'object' && !Array.isArray(args) && !Object.keys(args).length) return { result: { enum: [null] } };
  if (reference.type === 'TextInput') return validateInvocation('TextInput', method, args, mode);
  if (!args || typeof args !== 'object' || Array.isArray(args) || Object.keys(args).length)
    throw new Error('Focus arguments must be an empty object');
  if (!(mode === 'invoke' ? method === 'focus' : ['is_focused', 'contains_focused', 'within_focused'].includes(method)))
    throw new Error('Unsupported focus operation or mode');
  return { result: mode === 'invoke' ? { enum: [null] } : { type: 'boolean' } };
}

export class NativeReferences {
  #issued = new Map();
  #closed = false;

  /** Adopt a validated native response, preserving stable reference identity.
   * Preflight the whole response before installing any capabilities.
   */
  adopt(value) {
    if (this.#closed) throw new Error('Native references revoked');
    validatePayload(value);
    const candidates = new Map();
    const visit = value => {
      if (!value || typeof value !== 'object') return;
      if (Object.hasOwn(value, '$nativeRef')) {
        validateNativeRef(value);
        const known = candidates.get(value.$nativeRef) ?? this.#issued.get(value.$nativeRef);
        if (known && known.type !== value.type) throw new Error('Native reference type changed');
        candidates.set(value.$nativeRef, value);
      } else for (const child of Object.values(value)) visit(child);
    };
    visit(value);
    const fresh = [...candidates.keys()].filter(id => !this.#issued.has(id));
    if (this.#issued.size + fresh.length > 128) throw new Error('Native reference quota exceeded');
    for (const id of fresh) this.#issued.set(id, Object.freeze({ ...candidates.get(id) }));
    const replace = value => {
      if (!value || typeof value !== 'object') return value;
      if (Object.hasOwn(value, '$nativeRef')) return this.#issued.get(value.$nativeRef);
      if (Array.isArray(value)) return value.map(replace);
      return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, replace(child)]));
    };
    return replace(value);
  }

  target(value, method, args, mode) {
    validateNativeRef(value);
    if (this.#closed || this.#issued.get(value.$nativeRef) !== value) throw new Error('Native reference was not issued to this worker');
    validateReferenceInvocation(value, method, args, mode);
    return value;
  }

  release(value) {
    validateNativeRef(value);
    if (this.#closed || this.#issued.get(value.$nativeRef) !== value) throw new Error('Native reference was not issued to this worker');
    this.#issued.delete(value.$nativeRef);
  }

  clear() { this.#closed = true; this.#issued.clear(); }
}
