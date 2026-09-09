import type { KitNode, SlotNode } from './kit-sdk.js';
import type { BuiltinIconDescriptor } from './kit-icon-sdk.js';
export type CanvasColor = { palette: string } | { semantic: 'accent' | 'accent_strong' | 'success' | 'warning' | 'danger' | 'info' } | { hsla: { h: number; s: number; l: number; a: number } };
export type CanvasGlass = 'liquid' | 'frosted' | 'clear' | 'lens';
export interface CanvasPoint { x: number; y: number }
export interface CanvasRect extends CanvasPoint { width: number; height: number }
export interface GraphEndpoint { node: string; port: string }
export interface CanvasToolbarProps { zoom?: string; disabled?: boolean; snap?: boolean; actions?: ('fit' | 'snap' | 'arrange')[]; glass?: CanvasGlass }
export interface GraphNodeProps {
  title?: string; selected?: boolean; disabled?: boolean; icon?: BuiltinIconDescriptor; color?: CanvasColor;
  action?: string; kind?: string; note?: string; note_lines?: number; status?: string; width?: number;
  thumbnail_ratio?: number; progress?: number | null; active_glass?: CanvasGlass;
  state?: 'pending' | 'idle' | 'queued' | 'starting' | 'running' | 'waiting' | 'blocked' | 'succeeded' | 'partial' | 'failed' | 'refused' | 'cancelling' | 'cancelled' | 'timed_out' | 'unavailable';
  diff?: { added: number; removed: number };
  metrics?: { label: string; value: string; labelled?: boolean }[];
  ports?: { id: string; label: string; direction: 'input' | 'output'; side?: 'top' | 'right' | 'bottom' | 'left'; typed?: { id: string; color: CanvasColor; glyph?: BuiltinIconDescriptor } }[];
}
export type NodeGraphEvent =
  | { type: 'viewport_changed'; offset: CanvasPoint; zoom: number }
  | { type: 'selection_changed'; ids: string[] }
  | { type: 'node_moved'; id: string; position: CanvasPoint }
  | { type: 'node_resized'; id: string; size: { width: number; height: number } }
  | { type: 'node_deleted' | 'disconnect_requested'; id: string }
  | { type: 'surface_pressed'; position: CanvasPoint; button: 'left' | 'right' | 'middle' | 'back' | 'forward'; click_count: number }
  | { type: 'connection_requested'; from: GraphEndpoint; to: GraphEndpoint }
  | { type: 'connection_dropped'; from: GraphEndpoint; at: CanvasPoint };
export interface NodeGraphProps {
  disabled?: boolean;
  nodes?: { id: string; props: GraphNodeProps; x: number; y: number; height?: number }[];
  edges?: { id: string; from: string; to: string; ports?: { from: string; to: string }; label?: string; active?: boolean; selected?: boolean; state?: 'idle' | 'active' | 'succeeded' | 'failed'; marker?: 'none' | 'dot' | 'arrow'; lane?: number; feedback?: boolean; color?: CanvasColor }[];
  bands?: (CanvasRect & { id: string; label: string; selected?: boolean; color?: CanvasColor })[];
  empty?: { title: string; detail?: string; icon?: BuiltinIconDescriptor; kind?: 'empty' | 'unstarted' | 'queued' | 'blocked' | 'cancelled' | 'unavailable' | 'failed' | 'unauthorized' };
  state?: { kind: 'ready' | 'loading' } | { kind: 'refused' | 'failed'; reason: string };
  grid?: boolean; axes?: boolean; ground_light?: boolean; minimap?: boolean;
  viewport?: { offset: CanvasPoint; zoom: number }; zoom_range?: { min: number; max: number };
  offset?: CanvasPoint; zoom?: number;
  interaction?: 'inspect' | 'arrange' | 'edit'; routing?: 'lanes' | 'curves'; toolbar?: CanvasToolbarProps;
  fit?: number | null; fit_clearance?: { top: number; right: number; bottom: number; left: number };
  /** Precomputed output-to-input pairs for native validity previews. Proposals still reach the caller, which owns acceptance/refusal. */
  can_connect?: { from: GraphEndpoint; to: GraphEndpoint }[];
}
export interface CanvasFactories {
  CanvasToolbar(id: string, props?: CanvasToolbarProps, events?: { action?(value: 'fit' | 'snap' | 'arrange'): void }): KitNode;
  GraphNode(id: string, props?: GraphNodeProps, events?: { click?(): void }, slots?: { content?: SlotNode[]; thumbnail?: SlotNode[] }): KitNode;
  NodeGroup(id: string, props?: { label?: string; selected?: boolean }, events?: Record<string, never>, slots?: { content?: SlotNode[] }): KitNode;
  Minimap(id: string, props?: { disabled?: boolean; marks?: (CanvasRect & { id: string; color?: CanvasColor })[]; view?: CanvasRect }, events?: { pan?(point: CanvasPoint): void }): KitNode;
  NodeGraph(id: string, props?: NodeGraphProps, events?: { event?(value: NodeGraphEvent): void; toolbar_action?(value: 'fit' | 'snap' | 'arrange'): void; node_click?(id: string): void }, slots?: { empty?: SlotNode[]; empty_action?: SlotNode[]; failed?: SlotNode[]; loading?: SlotNode[] } & Partial<Record<`${string}:content` | `${string}:thumbnail`, SlotNode[]>>): KitNode;
}
