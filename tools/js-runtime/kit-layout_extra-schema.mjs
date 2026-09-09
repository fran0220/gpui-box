import { iconSchema } from './kit-icon-schema.mjs';
const string = { type: 'string', max: 16384 };
const id = { type: 'string', min: 1, max: 256 };
const boolean = { type: 'boolean' };
const number = { type: 'number', min: 0, max: 1e6 };
const positive = { ...number, min: 0.0001 };
const integer = { ...number, integer: true };
const ratio = { type: 'number', min: 0, max: 1 };
const choice = (...values) => ({ enum: values });
const object = (fields, required = []) => ({ type: 'object', fields, required });
const array = (items, max = 1024) => ({ type: 'array', items, max });
const space = choice('xxs', 'xs', 'sm', 'md', 'lg', 'xl', 'xxl');
const breakpoint = choice('small', 'medium', 'large', 'extraLarge');
const region = choice('left', 'centre', 'right', 'bottom');
const count = { ...integer, min: 1, max: 65535 };
const record = { id, parent: id, ratio, minWidth: number, minHeight: number, rail: number, collapsed: boolean };
const dockRecord = { oneOf: [
  object({ ...record, kind: choice('stack'), panels: array(id), active: id }, ['id', 'kind']),
  object({ ...record, kind: choice('horizontal', 'vertical') }, ['id', 'kind']),
] };
const bounds = object({ x: ratio, y: ratio, width: ratio, height: ratio }, ['x', 'y', 'width', 'height']);
const panel = { id, title: string, badge: string, unavailable: string, icon: iconSchema };
const edges = { top: boolean, bottom: boolean, left: boolean, right: boolean, band: number };
const variant = (kind, fields) => object({ kind: choice(kind), ...fields }, ['kind', ...Object.keys(fields)]);
const splitEvent = { oneOf: [variant('ratio', { split: id, ratio }), variant('collapsed', { split: id, side: choice('start', 'end'), pane: id })] };
const dockTreeEvent = { oneOf: [variant('floatingRaised', { stack: id }), variant('floatingCancelled', { stack: id }), variant('floatingChanged', { stack: id, bounds, finished: boolean }), variant('panelSelected', { stack: id, panel: id }), variant('panelMoved', { panel: id, toStack: id, before: { ...id, nullable: true } }), variant('panelSplit', { panel: id, targetStack: id, placement: choice('left', 'right', 'top', 'bottom') }), variant('splitResized', { split: id, ratio }), variant('stackCollapsed', { stack: id, collapsed: boolean })] };
const dockEvent = { oneOf: [variant('panelSelected', { region, panel: id }), variant('panelMoved', { panel: id, toRegion: region, before: { ...id, nullable: true } }), variant('regionCollapsed', { region, collapsed: boolean }), variant('regionResized', { region, ratio })] };
export const familySchemas = Object.freeze({
  AspectRatio: { props: object({ ratio: positive, fit: choice('width', 'height') }, ['ratio']), events: {}, slots: ['content'] },
  Container: { props: object({ width: choice('full', 'readable', 'dialog', 'custom'), customWidth: positive, padding: space }), events: {}, slots: ['content'] },
  Grid: { props: object({ columns: count, columnsAt: array(object({ breakpoint, columns: count }, ['breakpoint', 'columns']), 4), gap: space, items: array(object({ id, span: count, spanAt: array(object({ breakpoint, span: count }, ['breakpoint', 'span']), 4) }, ['id'])) }), events: {}, slotIds: 'items' },
  Responsive: { props: object({ threshold: number, fill: boolean }, ['threshold']), events: {}, slots: ['unmeasured', 'narrow', 'wide'] },
  ScrollFade: { props: object({ ...edges, fitHeight: boolean }), events: {}, slots: ['content'] },
  ScrollEdgeEffect: { props: object({ ...edges, blur: number, kind: choice('soft', 'hard') }), events: {}, slots: ['content'] },
  SplitTree: { props: object({ disabled: boolean, records: array(object({ ...record, kind: choice('pane', 'horizontal', 'vertical') }, ['id', 'kind'])) }, ['records']), events: { change: splitEvent }, slotIds: 'records' },
  DockTree: { props: object({ disabled: boolean, records: array(dockRecord), floating: array(object({ stack: dockRecord, bounds }, ['stack', 'bounds'])), panels: array(object(panel, ['id', 'title'])) }, ['records']), events: { event: dockTreeEvent }, slotIds: 'panels' },
  Dock: { props: object({ disabled: boolean, panels: array(object({ ...panel, region }, ['id', 'title', 'region'])), regions: array(object({ id: region, active: id, collapsed: boolean, share: ratio, minSize: number }, ['id']), 4) }), events: { event: dockEvent }, slotIds: 'panels' },
  Toolbar: { props: object({ size: choice('xs', 'sm', 'md', 'lg'), label: string, groups: array(object({ id, spacer: boolean }, ['id'])), items: array(object({ id, label: string, group: id, disabled: boolean, shortcut: string, icon: iconSchema }, ['id', 'label', 'group'])), overflow: boolean, overflowAfter: integer }), events: { overflowSelect: id }, slotIds: 'items' },
  DesktopTitlebar: { props: object({ title: string, subtitle: string, buttons: object({ left: array(choice('minimize', 'maximize', 'close'), 3), right: array(choice('minimize', 'maximize', 'close'), 3) }), controls: object({ fullscreen: boolean, maximize: boolean, minimize: boolean, windowMenu: boolean }, ['fullscreen', 'maximize', 'minimize', 'windowMenu']) }, ['title']), events: { event: choice('minimize', 'toggleMaximize', 'close') }, slots: ['left', 'right'] },
  StatusBar: { props: object({ label: string, items: array(object({ id, label: string, group: choice('start', 'centre', 'end'), kind: choice('text', 'state', 'progress', 'action', 'element'), tone: choice('neutral', 'accent', 'success', 'warning', 'danger', 'info'), disabled: boolean, stale: boolean, stateName: string, fraction: ratio, count: object({ done: integer, total: integer }, ['done', 'total']), icon: iconSchema }, ['id', 'label'])) }), events: { click: id }, slotIds: 'items' },
});
export const familyMethods = Object.freeze({
  AspectRatio: { invoke: {}, query: { ratio: { args: object({}), result: positive } } },
  Toolbar: { invoke: {}, query: { item_count: { args: object({}), result: { ...integer, max: 1024 } } } },
});

export function validateFamilyProps(component, props) {
  if (component === 'Container' && props.width === 'custom' && props.customWidth === undefined) throw new TypeError('custom width required');
  if (component === 'Toolbar') {
    const groups = new Set((props.groups ?? []).map(v => v.id));
    if ((props.items ?? []).some(v => !groups.has(v.group))) throw new TypeError('unknown toolbar group');
  }
  if (component === 'DesktopTitlebar') {
    const buttons = [...(props.buttons?.left ?? []), ...(props.buttons?.right ?? [])];
    if (new Set(buttons).size !== buttons.length) throw new TypeError('duplicate window button');
  }
  if (!['DockTree', 'SplitTree'].includes(component)) return;
  const records = props.records;
  const byId = new Map(records.map(v => [v.id, v]));
  const roots = records.filter(v => v.parent === undefined);
  if (roots.length !== 1 || byId.size !== records.length) throw new TypeError('invalid topology root or identity');
  const visited = new Set();
  const panels = new Set();
  const inspectStack = v => {
    for (const panel of v.panels ?? []) {
      if (panels.has(panel)) throw new TypeError('duplicate panel');
      panels.add(panel);
    }
    if (v.active !== undefined && !(v.panels ?? []).includes(v.active)) throw new TypeError('missing active panel');
  };
  const visit = v => {
    if (visited.has(v.id)) throw new TypeError('topology cycle');
    visited.add(v.id);
    const children = records.filter(child => child.parent === v.id);
    const leaf = v.kind === 'stack' || v.kind === 'pane';
    if (children.length !== (leaf ? 0 : 2)) throw new TypeError('invalid topology child count');
    if (component === 'DockTree') {
      if (!leaf && ((v.panels ?? []).length || v.active !== undefined)) throw new TypeError('split carries panels');
      inspectStack(v);
    }
    for (const child of children) visit(child);
  };
  visit(roots[0]);
  if (visited.size !== records.length) throw new TypeError('unreachable topology');
  for (const tile of props.floating ?? []) {
    const { x, y, width, height } = tile.bounds;
    if (tile.stack.kind !== 'stack' || tile.stack.parent !== undefined || visited.has(tile.stack.id) || width <= 0 || height <= 0 || x + width > 1 || y + height > 1) throw new TypeError('invalid floating stack');
    visited.add(tile.stack.id);
    inspectStack(tile.stack);
  }
}
