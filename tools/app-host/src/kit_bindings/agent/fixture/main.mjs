import { cases } from './cases.mjs';
const log = gpui.state('No action requested');
const detail = gpui.state(false);
gpui.mount(() => gpui.column('fixture', [
  gpui.text('title', 'Agent adapters · explicit non-product fixtures'),
  gpui.button('page', 'Switch request / evidence fixtures', () => detail.set(!detail.get())),
  ...cases.filter(item => (detail.get() ? ['tool', 'context', 'offerings', 'dialogue'] : ['approval', 'clarification']).includes(item.id)).map(item => gpui.kit[item.component](item.id, item.props, Object.fromEntries(Object.keys(item.events ?? {}).map(event => [event, value => log.set(`${event}: ${JSON.stringify(value)}`)])))),
  gpui.text('event-log', log.get()),
]));
