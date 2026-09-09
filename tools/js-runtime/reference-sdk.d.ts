import type { KitMethodContracts } from './kit-sdk.js';

declare const issuedNativeReference: unique symbol;
/** Issued by native getters. Identity is scoped to a worker generation and the
 * original mount's lifetime; it cannot be constructed from an id or snapshot.
 */
export interface NativeRef<T extends keyof NativeReferenceContracts = keyof NativeReferenceContracts> {
  readonly $nativeRef: string;
  readonly type: T;
  readonly [issuedNativeReference]: T;
}
export interface NativeReferenceContracts {
  TextArea: KitMethodContracts['TextArea'];
  TextInput: KitMethodContracts['TextInput'];
  Menu: KitMethodContracts['Menu'];
  SearchField: KitMethodContracts['SearchField'];
  FocusHandle: {
    invoke: { focus: { args: Record<string, never>; result: null } };
    query: {
      is_focused: { args: Record<string, never>; result: boolean };
      contains_focused: { args: Record<string, never>; result: boolean };
      within_focused: { args: Record<string, never>; result: boolean };
    };
  };
}
type Args<S> = S extends { args: infer A } ? {} extends A ? [args?: A] : [args: A] : never;
type Result<S> = S extends { result: infer R } ? R extends { $nativeRef: string; type: infer T extends keyof NativeReferenceContracts } ? NativeRef<T> : R : never;
export type NativeInvoke = <T extends keyof NativeReferenceContracts, M extends keyof NativeReferenceContracts[NoInfer<T>]['invoke']>(target: NativeRef<T>, method: M, ...args: Args<NativeReferenceContracts[T]['invoke'][M]>) => Promise<Result<NativeReferenceContracts[T]['invoke'][M]>>;
export type NativeQuery = <T extends keyof NativeReferenceContracts, M extends keyof NativeReferenceContracts[NoInfer<T>]['query']>(target: NativeRef<T>, method: M, ...args: Args<NativeReferenceContracts[T]['query'][M]>) => Promise<Result<NativeReferenceContracts[T]['query'][M]>>;
