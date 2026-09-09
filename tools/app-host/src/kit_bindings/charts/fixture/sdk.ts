import type { FamilyFactories, FamilyMethodContracts } from '../../../../../js-runtime/kit-charts-sdk.js';
declare const charts: FamilyFactories;
charts.LineChart('line', { label: 'Revenue', state: { kind: 'ready', data: [{ id: 'west', label: 'West', points: [{ id: 'tue', x: 0.2, y: 0.8, label: 'Tue', value: '$17.3' }] }] } }, { current(value) { value.seriesId.toUpperCase(); } });
charts.Plot('plot', { label: 'P', state: { kind: 'empty' }, paint: [{ kind: 'rect', bounds: { x: 0.1, y: 0.2, width: 0.3, height: 0.4 }, tint: { h: 0, s: 0, l: 0, a: 1 } }] });
// @ts-expect-error ready requires data
charts.LineChart('bad', { label: 'L', state: { kind: 'ready' } });
// @ts-expect-error custom paint is data, not transported functions
charts.Plot('bad', { label: 'P', state: { kind: 'empty' }, paint: () => {} });
// @ts-expect-error series current selection is not an index
charts.LineChart('bad', { label: 'L', state: { kind: 'empty' }, current: 2 });
const args: FamilyMethodContracts['SankeyChart']['query']['layout']['args'] = { data: { nodes: [], links: [] }, weights: [], nodeWidth: 0.08, gap: 0.1, alignment: 'justify' };
void args;
// @ts-expect-error query arguments are named
const positional: FamilyMethodContracts['SankeyChart']['query']['layout']['args'] = [[], 0.08, 0.1];
void positional;
