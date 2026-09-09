// Explicit fixture. Bindings are caller-owned JS state, not native pointers.
const enabled = gpui.state(false);
const selected = gpui.state({ id: 'alpha' });
gpui.mount(() => gpui.column('binding-fixture', [
  gpui.text('binding-title', 'Adapted state binding fixture'),
  gpui.kit.bind('Checkbox', 'bound-check', enabled, { label: 'Enable linked native controls' }),
  gpui.kit.bind('Switch', 'bound-switch', enabled, { label: 'Same caller state' }),
  gpui.kit.bind_value('Radio', 'bound-alpha', selected, { id: 'alpha' }, { label: 'Alpha object value' }),
  gpui.kit.bind_value('Radio', 'bound-beta', selected, { id: 'beta' }, { label: 'Beta object value' }),
  gpui.text('bound-result', `Bound: ${enabled.get()} · ${selected.get().id}`),
]));
