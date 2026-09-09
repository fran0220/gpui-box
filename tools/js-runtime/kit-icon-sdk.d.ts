import type { BuiltinIconKey } from './kit-icon-catalog.js';
export type { BuiltinIconKey } from './kit-icon-catalog.js';
export interface BuiltinIconDescriptor {
  key: BuiltinIconKey;
  /** Defaults to regular. */
  weight?: 'regular' | 'fill';
}
