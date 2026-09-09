import type { KitAPI, KitNode, KitInvoke, KitQuery } from './kit-sdk.js';
import type { ResourceAPI } from './resource-sdk.js';
import type { NativeRef, NativeInvoke, NativeQuery } from './reference-sdk.js';
export type { NativeRef } from './reference-sdk.js';
export type JSONValue = null | boolean | number | string | JSONValue[] | { [key: string]: JSONValue };
export type Node = KitNode | { kind: 'column' | 'row' | 'text' | 'button'; id: string; text?: string; children?: Node[]; disabled?: boolean; action?: string };
export interface State<T> { get(): T; set(value: T | ((previous: T) => T)): void }
export interface GPUI {
  readonly resources: Readonly<ResourceAPI>;
  readonly kit: Readonly<KitAPI>;
  readonly invoke: KitInvoke & NativeInvoke;
  readonly query: KitQuery & NativeQuery;
  releaseReference(reference: NativeRef): Promise<void>;
  mount(view: () => Node): void;
  state<T>(initial: T): State<T>;
  column(id: string, children: Node[]): Node;
  row(id: string, children: Node[]): Node;
  text(id: string, text: unknown): Node;
  button(id: string, text: string, onClick: () => void | Promise<void>, disabled?: boolean): Node;
  command(id: string, handler: () => void | Promise<void>): () => void;
  onDispose(cleanup: () => void | Promise<void>): void;
  fs: { readText(path: string): Promise<string> };
  storage: { get(key: string): Promise<unknown>; set(key: string, value: unknown): Promise<null> };
  network: { get(url: string): Promise<string> };
  process: { run(command: string, args?: string[]): Promise<string> };
}
declare global { const gpui: GPUI; }
