import { cases } from './cases.mjs';
const log = gpui.state('No game action requested');
const detail = gpui.state(false);
gpui.mount(() => gpui.column('fixture', [
  gpui.text('title', 'Game/effects adapters · explicit non-product fixtures'),
  gpui.button('page', 'Switch game / effect fixtures', () => detail.set(!detail.get())),
  ...cases.filter(item => (detail.get() ? ['reward', 'cinematic', 'micro'] : ['abilities', 'objectives', 'party']).includes(item.id)).map(item => {
    const native = gpui.kit[item.component](item.id, item.props, Object.fromEntries(Object.keys(item.events ?? {}).map(event => [event, value => log.set(`${event}: ${JSON.stringify(value)}`)])));
    return item.id === 'cinematic' ? gpui.kit.ScrollArea('cinematic-frame', { height: 100, width: 400 }, {}, { content: [native] }) : native;
  }),
  gpui.text('event-log', log.get()),
]));
