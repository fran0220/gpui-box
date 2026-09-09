import { iconSchema } from './kit-icon-schema.mjs';
const string = { type: 'string', max: 16384 };
const id = { type: 'string', min: 1, max: 256 };
const boolean = { type: 'boolean' };
const integer = { type: 'number', min: 0, max: 1024, integer: true };
const choice = (...values) => ({ enum: values });
const object = (fields, required = []) => ({ type: 'object', fields, required });
const array = items => ({ type: 'array', items, max: 1024 });
const common = { disabled: boolean, size: choice('xs', 'sm', 'md', 'lg') };
const item = { id, label: string };
const intent = { oneOf: [object({ kind: choice('step'), id }, ['kind', 'id']), ...['back', 'next', 'finish'].map(kind => object({ kind: choice(kind) }, ['kind']))] };
const carouselEvent = { oneOf: [object({ kind: choice('selected'), id }, ['kind', 'id']), ...['previous', 'next'].map(kind => object({ kind: choice(kind) }, ['kind']))] };
export const familySchemas = Object.freeze({
  AnchorList: { props: object({ ...common, anchors: array(object({ ...item, disabled: boolean }, ['id', 'label'])), active: id, overflow: boolean, overflowAfter: integer }), events: { navigate: id } },
  Breadcrumb: { props: object({ crumbs: array(object(item, ['id', 'label'])), maxVisible: integer }), events: { select: id, reveal: array(id) } },
  Collapsible: { props: object({ ...common, title: string, description: string, open: boolean }, ['title']), events: { toggle: boolean }, slots: ['body'] },
  Carousel: { props: object({ size: common.size, items: array(object(item, ['id', 'label'])), active: id, phase: choice('idle', 'queued', 'blocked', 'loading', 'refreshing', 'ready', 'empty', 'unavailable', 'error', 'cancelled'), reason: string, stale: boolean, looped: boolean }), events: { event: carouselEvent }, slotIds: 'items' },
  NavStack: { props: object({ entries: array(object({ id }, ['id'])), cursor: integer, label: string }, ['entries', 'cursor', 'label']), events: {}, slotIds: 'entries' },
  Sidebar: { props: object({ ...common, sections: array(object({ id, title: string }, ['id'])), items: array(object({ ...item, section: id, within: id, disabled: boolean, badge: string, icon: iconSchema }, ['id', 'label', 'section'])), active: id, collapsed: boolean }), events: { select: id }, slots: ['header', 'footer'] },
  UndoHistory: { props: object({ disabled: boolean, label: string, entries: array(object({ ...item, description: string, time: string, source: string, unavailable: string }, ['id', 'label'])), current: id }, ['label']), events: { jump: id } },
  Wizard: { props: object({ ...common, steps: array(object({ id, title: string, description: string, status: choice('upcoming', 'current', 'complete', 'blocked', 'failed'), reason: string, reachable: boolean }, ['id', 'title'])), layout: choice('horizontal', 'vertical'), backTo: id, finish: boolean, canAdvance: boolean, backLabel: string, nextLabel: string, finishLabel: string }), events: { navigate: intent }, slots: ['body'] },
});
export const familyMethods = Object.freeze({});

export function validateFamilyProps(component, props) {
  if (component === 'NavStack') {
    if (!props.entries.length || props.cursor >= props.entries.length || new Set(props.entries.map(v => v.id)).size !== props.entries.length) throw new TypeError('invalid navigation history');
  }
  if (component === 'Sidebar') {
    const rows = new Map((props.items ?? []).map(v => [v.id, v]));
    const sections = new Set((props.sections ?? []).map(v => v.id));
    for (const row of rows.values()) {
      if (!sections.has(row.section)) throw new TypeError('unknown sidebar section');
      const seen = new Set();
      for (let parent = row.within; parent !== undefined; parent = rows.get(parent).within) {
        if (seen.has(parent) || !rows.has(parent) || rows.get(parent).section !== row.section) throw new TypeError('invalid sidebar parent');
        seen.add(parent);
      }
    }
  }
}
