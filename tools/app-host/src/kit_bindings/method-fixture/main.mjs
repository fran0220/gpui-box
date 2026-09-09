// Explicit fixture. These values are not product-backed records.
const input = gpui.kit.TextInput('method-input', { placeholder: 'Native retained value' });
const select = gpui.kit.Select('method-select', {
  options: [{ id: 'alpha', label: 'Alpha' }, { id: 'beta', label: 'Beta' }],
});
const result = gpui.state('Not invoked');
gpui.mount(() => gpui.column('methods', [
  gpui.text('methods-title', 'Typed native method fixture'),
  input,
  select,
  gpui.button('methods-run', 'Set native values and query them', async () => {
    await gpui.invoke(input, 'set_text_quietly', { value: 'native É🙂' });
    await gpui.invoke(select, 'set_selected', { id: 'beta' });
    const text = await gpui.query(input, 'value');
    const option = await gpui.query(select, 'selected_option');
    result.set(`Read native: ${text} · ${option.id}`);
  }),
  gpui.text('methods-result', result.get()),
]));
