import { iconSchema } from './kit-icon-schema.mjs';

const string = { type: 'string', max: 16384 };
const id = { type: 'string', min: 1, max: 256 };
const boolean = { type: 'boolean' };
const number = { type: 'number', min: -1e9, max: 1e9 };
const positive = { ...number, min: 0.001 };
const natural = { ...number, min: 0, integer: true };
const unit = { ...number, min: 0, max: 1 };
const choice = (...values) => ({ enum: values });
const object = (fields, required = []) => ({ type: 'object', fields, required });
const array = items => ({ type: 'array', items, max: 1024 });
const point = object({ x: number, y: number }, ['x', 'y']);
const rect = object({ x: number, y: number, width: positive, height: positive }, ['x', 'y', 'width', 'height']);
const endpoint = object({ node: id, port: id }, ['node', 'port']);
export const canvasColorSchema = { oneOf: [
  object({ palette: id }, ['palette']),
  object({ semantic: choice('accent', 'accent_strong', 'success', 'warning', 'danger', 'info') }, ['semantic']),
  object({ hsla: object({ h: unit, s: unit, l: unit, a: unit }, ['h', 's', 'l', 'a']) }, ['hsla']),
] };
const glass = choice('liquid', 'frosted', 'clear', 'lens');
const toolbar = object({ zoom: string, disabled: boolean, snap: boolean, actions: array(choice('fit', 'snap', 'arrange')), glass });
const graphNode = object({
  title: string, selected: boolean, disabled: boolean, icon: iconSchema, color: canvasColorSchema,
  action: string, kind: string, note: string, note_lines: natural, status: string, width: positive,
  thumbnail_ratio: positive, progress: { ...unit, nullable: true }, active_glass: glass,
  state: choice('pending', 'idle', 'queued', 'starting', 'running', 'waiting', 'blocked', 'succeeded', 'partial', 'failed', 'refused', 'cancelling', 'cancelled', 'timed_out', 'unavailable'),
  diff: object({ added: natural, removed: natural }, ['added', 'removed']),
  metrics: array(object({ label: string, value: string, labelled: boolean }, ['label', 'value'])),
  ports: array(object({ id, label: string, direction: choice('input', 'output'), side: choice('top', 'right', 'bottom', 'left'), typed: object({ id, color: canvasColorSchema, glyph: iconSchema }, ['id', 'color']) }, ['id', 'label', 'direction'])),
});
const event = (type, fields) => object({ type: choice(type), ...fields }, ['type', ...Object.keys(fields)]);
export const canvasEventSchema = { oneOf: [
  event('viewport_changed', { offset: point, zoom: positive }),
  event('selection_changed', { ids: array(id) }),
  event('node_moved', { id, position: point }),
  event('node_resized', { id, size: object({ width: positive, height: positive }, ['width', 'height']) }),
  event('node_deleted', { id }),
  event('surface_pressed', { position: point, button: choice('left', 'right', 'middle', 'back', 'forward'), click_count: natural }),
  event('connection_requested', { from: endpoint, to: endpoint }),
  event('connection_dropped', { from: endpoint, at: point }),
  event('disconnect_requested', { id }),
] };
export const familySchemas = Object.freeze({
  CanvasToolbar: { props: toolbar, events: { action: choice('fit', 'snap', 'arrange') } },
  GraphNode: { props: graphNode, events: { click: choice(null) }, slots: ['content', 'thumbnail'] },
  NodeGroup: { props: object({ label: string, selected: boolean }), events: {}, slots: ['content'] },
  Minimap: { props: object({ disabled: boolean, marks: array(object({ id, ...rect.fields, color: canvasColorSchema }, ['id', ...rect.required])), view: rect }), events: { pan: point } },
  NodeGraph: { props: object({
    disabled: boolean, nodes: array(object({ id, props: graphNode, x: number, y: number, height: positive }, ['id', 'props', 'x', 'y'])),
    edges: array(object({ id, from: id, to: id, ports: object({ from: id, to: id }, ['from', 'to']), label: string, active: boolean, selected: boolean, state: choice('idle', 'active', 'succeeded', 'failed'), marker: choice('none', 'dot', 'arrow'), lane: { ...natural, min: -32768, max: 32767 }, feedback: boolean, color: canvasColorSchema }, ['id', 'from', 'to'])),
    bands: array(object({ id, label: string, ...rect.fields, selected: boolean, color: canvasColorSchema }, ['id', 'label', ...rect.required])),
    empty: object({ title: string, detail: string, icon: iconSchema, kind: choice('empty','unstarted','queued','blocked','cancelled','unavailable','failed','unauthorized') }, ['title']),
    state: { oneOf: [object({ kind: choice('ready', 'loading') }, ['kind']), object({ kind: choice('failed', 'refused'), reason: string }, ['kind', 'reason'])] },
    grid: boolean, axes: boolean, ground_light: boolean, minimap: boolean,
    viewport: object({ offset: point, zoom: positive }, ['offset', 'zoom']),
    offset: point, zoom: positive,
    zoom_range: object({ min: positive, max: positive }, ['min', 'max']),
    interaction: choice('inspect', 'arrange', 'edit'), routing: choice('lanes', 'curves'),
    toolbar, fit: { ...natural, nullable: true }, fit_clearance: object({ top: { ...number, min: 0 }, right: { ...number, min: 0 }, bottom: { ...number, min: 0 }, left: { ...number, min: 0 } }, ['top', 'right', 'bottom', 'left']),
    can_connect: array(object({ from: endpoint, to: endpoint }, ['from', 'to'])),
  }), events: { event: canvasEventSchema, toolbar_action: choice('fit', 'snap', 'arrange'), node_click: id }, slots: ['empty', 'empty_action', 'failed', 'loading'], slotIds: 'nodes', slotSuffixes: ['content', 'thumbnail'] },
});
// Canvas builders expose no imperative native methods. Viewport and topology
// changes are proposals, not hidden adapter-owned graph mutations.
export const familyMethods = Object.freeze({});

export function validateCanvasProps(component, props) {
  if (component !== 'NodeGraph') return;
  if (props.zoom_range && props.zoom_range.min > props.zoom_range.max) throw new TypeError('NodeGraph: invalid zoom_range');
  const nodes = new Map((props.nodes ?? []).map(node => [node.id, node]));
  const port = (nodeId, portId) => nodes.get(nodeId)?.props.ports?.find(port => port.id === portId);
  const identities = [...(props.nodes ?? []).map(node => node.id), ...(props.edges ?? []).map(edge => edge.id), ...(props.bands ?? []).map(band => band.id)];
  if (new Set(identities).size !== identities.length) throw new TypeError('NodeGraph: duplicate graph identity');
  for (const edge of props.edges ?? []) {
    if (!nodes.has(edge.from) || !nodes.has(edge.to)) throw new TypeError('NodeGraph: unknown endpoint node');
    if (edge.ports && (!port(edge.from, edge.ports.from) || !port(edge.to, edge.ports.to))) throw new TypeError('NodeGraph: unknown endpoint port');
  }
  for (const pair of props.can_connect ?? []) {
    if (port(pair.from.node, pair.from.port)?.direction !== 'output' || port(pair.to.node, pair.to.port)?.direction !== 'input') throw new TypeError('NodeGraph: can_connect requires output-to-input endpoints');
  }
}
