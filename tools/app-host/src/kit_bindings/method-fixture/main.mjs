// Explicit fixture. These values are not product-backed records.
const input = gpui.kit.TextInput('method-input', { placeholder: 'Native retained value' });
const select = gpui.kit.Select('method-select', {
  options: [{ id: 'alpha', label: 'Alpha' }, { id: 'beta', label: 'Beta' }],
});
const search = gpui.kit.SearchInput('method-search', { placeholder: 'Family native retained value' });
const aspect = gpui.kit.AspectRatio('method-aspect', { ratio: 17.5 }, {}, {
  content: [gpui.text('method-aspect-label', 'Native 17.5:1 aspect frame')],
});
const result = gpui.state('Not invoked');
gpui.mount(() => gpui.column('methods', [
  gpui.text('methods-title', 'Typed native method fixture'),
  input,
  select,
  search,
  aspect,
  gpui.button('methods-run', 'Set native values and query them', async () => {
    await gpui.invoke(input, 'set_text_quietly', { value: 'native É🙂' });
    await gpui.invoke(select, 'set_selected', { id: 'beta' });
    await gpui.invoke(search, 'set_value', { value: 'family 東京' });
    const text = await gpui.query(input, 'value');
    const option = await gpui.query(select, 'selected_option');
    const query = await gpui.query(search, 'value');
    const ratio = await gpui.query(aspect, 'ratio');
    result.set(`Read native: ${text} · ${option.id} · ${query} · ${ratio}`);
  }),
  gpui.text('methods-result', result.get()),
]));
