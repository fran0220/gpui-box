const count = gpui.state(0);
const increment = () => count.set(value => value + 1);
gpui.command('increment', increment);
gpui.onDispose(() => console.info('Plugin disposed'));
gpui.mount(() => gpui.column('counter-panel', [
  gpui.text('count', `Plugin count: ${count.get()}`),
  gpui.button('increment', 'Increment in plugin', increment),
]));
