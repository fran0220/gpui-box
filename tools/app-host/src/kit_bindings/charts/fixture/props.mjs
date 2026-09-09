export const series = [{ id: 'west', label: 'West', points: [
  { id: 'mon', x: 0.1, y: 0.2, label: 'Monday', value: '2.1' },
  { id: 'tue', x: 0.4, y: 0.85, label: 'Tuesday', value: '17.3', weight: 0.4 },
  { id: 'wed', x: 0.9, y: 0.45, label: 'Wednesday', value: '9.2' },
] }];
const chart = { label: 'Caller readings', state: { kind: 'ready', data: series } };
const axis = { xLabel: 'Day', yLabel: 'Value', xStart: 'Mon', xEnd: 'Wed', yStart: '0', yEnd: '20' };
export const sankey = { nodes: [
  { id: 'west', label: 'West', value: '13', bounds: { x: 0, y: 0.1, width: 0.08, height: 0.6 } },
  { id: 'east', label: 'East', value: '13', bounds: { x: 0.9, y: 0.2, width: 0.08, height: 0.6 } },
], links: [{ id: 'flow', source: 'west', target: 'east', label: 'Traffic', value: '13', start: { x: 0.08, y: 0.4 }, end: { x: 0.9, y: 0.5 }, startWidth: 0.3, endWidth: 0.3 }] };
export const props = {
  AreaChart: { ...chart, axes: axis, crosshair: true, current: { seriesId: 'west', pointId: 'tue' } },
  BarChart: { ...chart, axes: axis },
  CandlestickChart: { label: 'OHLC', state: { kind: 'ready', data: [{ id: 'day', x: 0.3, open: 0.2, high: 0.9, low: 0.1, close: 0.7, label: 'Tuesday', value: '7.1' }] }, bodyWidth: 0.1 },
  ChartLegend: { series },
  GaugeChart: chart,
  LineChart: { ...chart, axes: axis, crosshair: true, smooth: true },
  PieChart: { ...chart, donut: true },
  Plot: { label: 'Native paint', state: { kind: 'ready', data: [{ id: 'mark', label: 'A', value: '12', bounds: { x: 0.1, y: 0.4, width: 0.2, height: 0.5 } }] }, paint: [{ kind: 'rect', bounds: { x: 0.1, y: 0.4, width: 0.2, height: 0.5 }, tint: { h: 0.6, s: 0.7, l: 0.5, a: 1 } }, { kind: 'line', points: [{ x: 0.4, y: 0.8 }, { x: 0.8, y: 0.2 }], width: 2, tint: { h: 0.05, s: 0.7, l: 0.5, a: 1 } }] },
  RadarChart: chart,
  SankeyChart: { label: 'Traffic', state: { kind: 'ready', data: sankey } },
  ScatterChart: { ...chart, axes: axis, crosshair: true },
  Sparkline: { label: 'Trend', state: { kind: 'stale', reason: 'Refresh failed', data: { points: [{ x: 0, y: 0.2 }, { x: 0.4, y: 0.9 }, { x: 1, y: 0.5 }], current: '9.2', minimum: '2.1', maximum: '17.3' } } },
  StackedBarChart: { ...chart, axes: axis },
};
