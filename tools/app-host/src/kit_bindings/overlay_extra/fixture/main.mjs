// Explicit fixture data. Slot factories stay in the host, not in this worker.
const title = gpui.state('First title');
const result = gpui.state('ready');
let drawer;
gpui.mount(() => gpui.column('overlay.fixture', [
  drawer = gpui.kit.Drawer('drawer', { title: title.get(), size: 273 }, {
    open() { result.set('opened'); }, close() { result.set('closed'); },
  }, { content: [gpui.text('drawer.body', title.get())] }),
  gpui.text('overlay.result', result.get()),
]));
gpui.command('rename', () => title.set('Updated while open'));
gpui.command('query', async () => {
  try { result.set(String(await gpui.query(drawer, 'is_open', {}))); }
  catch (error) { result.set(error.message); }
});
gpui.command('open', async () => {
  try { await gpui.invoke(drawer, 'open', {}); }
  catch (error) { result.set(error.message); }
});
