// Explicit synthetic fixture: only a real native reorder callback changes order.
import {component, mode} from './case.mjs';
const ids = gpui.state(['alpha', 'beta', 'gamma']);
const phase = gpui.state('ready');
const present = gpui.state(true);
const result = gpui.state('commits:0');
let commits = 0;
const pause = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
async function accepts(intent) {
  if (intent.id !== 'gamma' || intent.anchor !== 'alpha' || intent.position !== 'before') throw new Error('Wrong native intent');
  if (mode === 'stale') phase.set('revision changed');
  if (mode === 'removed') present.set(false);
  await pause(mode === 'timeout' ? 3500 : 150);
  if (mode === 'invalid') return 'true';
  return mode !== 'refuse';
}
function moved(intent) {
  const next = ids.get().filter(id => id !== intent.id);
  next.splice(next.indexOf(intent.anchor) + (intent.position === 'after' ? 1 : 0), 0, intent.id);
  ids.set(next);
  result.set(`commits:${++commits}`);
}
gpui.mount(() => gpui.column('deferred-root', [
  gpui.text('fixture-title', `DEFERRED · ${component} · ${mode}`),
  gpui.text('fixture-phase', phase.get()),
  gpui.text('fixture-result', result.get()),
  gpui.text('fixture-order', ids.get().join(',')),
  ...(present.get() ? [gpui.kit[component]('surface', {
    reorderable:true,
    [component === 'List' ? 'rows' : component === 'Tabs' ? 'tabs' : 'nodes']:ids.get().map(id => ({id,label:id.toUpperCase()})),
    ...(component === 'Tabs' ? {} : {visibleRows:3}),
  }, {[component === 'Tree' ? 'move' : 'reorder']:moved}, {}, {accepts})] : []),
]));
