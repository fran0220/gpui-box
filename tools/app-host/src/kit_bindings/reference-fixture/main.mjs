// Explicit native-reference review fixture, not product-backed data.
const input = gpui.kit.TextInput('reference-input', { text: 'Native É🙂' });
const status = gpui.state('Reference not requested');
gpui.mount(() => gpui.column('references', [
  input,
  gpui.button('reference-run', 'Focus issued handle, query, then release', async () => {
    try {
      const focus = await gpui.query(input, 'focus_handle');
      const again = await gpui.query(input, 'focus_handle');
      if (focus !== again) throw new Error('Native identity did not remain stable');
      await gpui.invoke(focus, 'focus');
      const focused = await gpui.query(focus, 'is_focused');
      await gpui.releaseReference(focus);
      let released = false;
      try { await gpui.query(focus, 'is_focused'); } catch { released = true; }
      if (!focused || !released) throw new Error('Focus/release invariant failed');
      status.set('Native focus: true · released: true');
    } catch (error) { status.set(`Refused: ${error.message}`); }
  }),
  gpui.text('reference-status', status.get()),
]));
