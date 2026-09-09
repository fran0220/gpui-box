/// <reference path="./gpui.d.ts" />
import { greeting } from './message.mts';

const count = gpui.state<number>(7);
const status = gpui.state<string>('Ready · click to change native state');
gpui.onDispose(() => console.info('Counter app disposed'));
gpui.mount(() => gpui.column('counter', [
  gpui.text('greeting', greeting),
  gpui.text('value', `Count: ${count.get()}`),
  gpui.row('actions', [
    gpui.button('increment', 'Add three', async () => { await Promise.resolve(); count.set(n => n + 3); }),
    gpui.button('save', 'Save count', async () => {
      try { await gpui.storage.set('count', count.get()); status.set('Saved by host'); }
      catch (error) { status.set(`Refused: ${(error as Error).message}`); }
    }),
    gpui.button('disabled', 'Unavailable action', () => count.set(999), true),
  ]),
  gpui.text('status', status.get()),
]));
