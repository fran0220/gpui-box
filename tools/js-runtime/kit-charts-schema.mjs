const str = { type: 'string', max: 16384 };
const id = { type: 'string', min: 1, max: 256 };
const bool = { type: 'boolean' };
const unit = { type: 'number', min: 0, max: 1 };
const num = { type: 'number', min: -1e9, max: 1e9 };
const choice = (...values) => ({ enum: values });
const object = (fields, required = []) => ({ type: 'object', fields, required });
const array = items => ({ type: 'array', items, max: 1024 });
const tint = object({ h: unit, s: unit, l: unit, a: unit }, ['h', 's', 'l', 'a']);
const xy = object({ x: unit, y: unit }, ['x', 'y']);
const box = object({ x: unit, y: unit, width: unit, height: unit }, ['x', 'y', 'width', 'height']);
const point = object({ id, x: unit, y: unit, label: str, value: str, weight: unit }, ['id', 'x', 'y', 'label', 'value']);
const series = array(object({ id, label: str, points: array(point), tint }, ['id', 'label', 'points']));
const state = data => object({ kind: choice('loading', 'empty', 'unavailable', 'error', 'ready', 'stale'), data, reason: str }, ['kind']);
const axes = object({ xLabel: str, yLabel: str, xStart: str, xEnd: str, yStart: str, yEnd: str });
const selection = object({ seriesId: id, pointId: id }, ['seriesId', 'pointId']);
const candle = object({ id, x: unit, open: unit, high: unit, low: unit, close: unit, label: str, value: str }, ['id', 'x', 'open', 'high', 'low', 'close', 'label', 'value']);
const mark = object({ id, label: str, value: str, bounds: box }, ['id', 'label', 'value', 'bounds']);
const sankey = object({
  nodes: array(object({ ...mark.fields, tint }, mark.required)),
  links: array(object({ id, source: id, target: id, label: str, value: str, start: xy, end: xy, startWidth: unit, endWidth: unit, tint }, ['id', 'source', 'target', 'label', 'value', 'start', 'end', 'startWidth', 'endWidth'])),
}, ['nodes', 'links']);
const reading = object({ points: array(xy), current: str, minimum: str, maximum: str }, ['points', 'current', 'minimum', 'maximum']);
const paint = array(object({ kind: choice('rect', 'line', 'polygon'), tint, bounds: box, points: array(xy), width: { type: 'number', min: 0.1, max: 100 } }, ['kind', 'tint']));
const chart = (extra = {}, events = {}) => ({ props: object({ label: str, state: state(series), ...extra }, ['label', 'state']), events, slots: ['empty', 'failed', 'loading'] });
const interactive = { axes, crosshair: bool, current: selection, disabled: bool };
export const familySchemas = Object.freeze({
  AreaChart: chart({ ...interactive, polyline: bool }, { current: selection }),
  BarChart: chart({ axes }),
  CandlestickChart: { props: object({ label: str, state: state(array(candle)), bodyWidth: unit, risingTint: tint, fallingTint: tint, current: id, disabled: bool }, ['label', 'state']), events: { current: id } },
  ChartLegend: { props: object({ series, hidden: array(id), disabled: bool }, ['series']), events: { toggle: object({ id, hidden: bool }, ['id', 'hidden']) } },
  GaugeChart: chart(),
  LineChart: chart({ ...interactive, area: bool, smooth: bool }, { current: selection }),
  PieChart: chart({ donut: bool }),
  Plot: { props: object({ label: str, state: state(array(mark)), current: id, disabled: bool, paint }, ['label', 'state']), events: { current: id } },
  RadarChart: chart(),
  SankeyChart: { props: object({ label: str, state: state(sankey), current: id, disabled: bool }, ['label', 'state']), events: { current: id } },
  ScatterChart: chart(interactive, { current: selection }),
  Sparkline: { props: object({ label: str, state: state(reading), tint, stale: bool, embedded: bool }, ['label', 'state']), events: {}, slots: ['empty', 'failed', 'loading'] },
  StackedBarChart: chart({ axes }),
});
const method = (fields, result) => ({ args: object(fields, Object.keys(fields)), result });
export const familyMethods = Object.freeze({
  Sparkline: { invoke: {}, query: { published_points: method({}, { type: 'number', min: 0, max: 1024, integer: true }) } },
  SankeyChart: { invoke: {}, query: { layout: method({ data: sankey, weights: array({ ...num, min: 0 }), nodeWidth: { ...unit, min: Number.MIN_VALUE, max: 0.999999 }, gap: { ...unit, max: 0.999999 }, alignment: choice('left', 'right', 'justify') }, object({ scale: { type: 'number', min: 0, max: Number.MAX_VALUE }, nodes: array(object({ id, bounds: box }, ['id', 'bounds'])), links: array(object({ id, start: xy, end: xy, startWidth: unit, endWidth: unit }, ['id', 'start', 'end', 'startWidth', 'endWidth'])) }, ['scale', 'nodes', 'links'])) } },
});

/** Run after generic closed-schema validation; state payloads are conditional. */
export function validateFamilyProps(component, props) {
  if (!Object.hasOwn(familySchemas, component)) return;
  if (component === 'Plot') for (const op of props.paint ?? []) {
    if (op.kind === 'rect') {
      if (!op.bounds || op.points !== undefined || op.width !== undefined) throw new TypeError('rectangle paint payload mismatch');
      validateBounds(op.bounds);
    } else {
      if (op.bounds !== undefined || !op.points || op.points.length < (op.kind === 'polygon' ? 3 : 2) || (op.width !== undefined) !== (op.kind === 'line')) throw new TypeError('path paint payload mismatch');
    }
  }
  const state = props.state;
  if (!state) return;
  const needsData = ['ready', 'stale'].includes(state.kind);
  const needsReason = ['unavailable', 'error', 'stale'].includes(state.kind);
  if (Object.hasOwn(state, 'data') !== needsData || Object.hasOwn(state, 'reason') !== needsReason) throw new TypeError('chart state payload does not match kind');
  if (!needsData) return;
  if (component === 'CandlestickChart') {
    for (const c of state.data) if (c.low > Math.min(c.open, c.close) || c.high < Math.max(c.open, c.close)) throw new TypeError('invalid OHLC range');
  }
  if (component === 'Plot') for (const m of state.data) validateBounds(m.bounds);
  if (component === 'SankeyChart') {
    const ids = new Set(state.data.nodes.map(n => n.id));
    for (const n of state.data.nodes) validateBounds(n.bounds);
    for (const l of state.data.links) if (!ids.has(l.source) || !ids.has(l.target)) throw new TypeError('unknown Sankey endpoint');
  }
}
function validateBounds(b) {
  if (b.width <= 0 || b.height <= 0 || b.x + b.width > 1 || b.y + b.height > 1) throw new TypeError('invalid normalized bounds');
}
