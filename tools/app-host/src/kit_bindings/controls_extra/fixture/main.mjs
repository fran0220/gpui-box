// Explicit fixture data, not a product-backed settings page.
const { kit } = gpui;
const pressed = gpui.state(['beta']);
const query = gpui.state('Native search');
gpui.mount(() => gpui.column('extra.fixture', [
  gpui.text('extra.title', 'Native controls · fixture data'),
  gpui.row('extra.body', [gpui.column('extra.controls', [
  gpui.row('extra.buttons', [
    kit.Button('extra.button', { label: 'Native Kit Button', variant: 'light', color: { semantic: 'info' } }),
    kit.IconButton('extra.icon', { icon: { key: 'plus-circle', weight: 'fill' }, accessibleName: 'Add fixture' }),
    kit.Button('extra.loading', { label: 'Waiting for host', loading: true }),
    kit.Toggle('extra.toggle', { label: 'Pressed toggle', pressed: true }),
  ]),
  kit.ToggleGroup('extra.group', { items: [{ id: 'alpha', label: 'Alpha' }, { id: 'beta', label: 'Beta' }, { id: 'gamma', label: 'Unavailable', disabled: true }], pressed: pressed.get() }, { change: value => pressed.set(value.pressed) }),
  kit.FormField('extra.field', { label: 'Search fixture', control: 'extra.search', validation: 'invalid', reason: 'Host refused the requested change', description: 'The current value remains visible.' }, {}, {
    content: [kit.SearchInput('extra.search', { name: 'Search fixture', value: query.get() }, { change: value => query.set(value) })],
  }),
  kit.FilterBar('extra.filters', { conditions: [{ id: 'status', field: 'Status', operator: 'is', value: 'Ready' }], countState: 'unavailable', countReason: 'Count permission refused', addLabel: 'Add condition', clearLabel: 'Clear conditions' }),
  kit.SettingsRow('extra.managed', { label: 'Retention', value: '90 days', managed: 'Workspace policy', description: 'Fixture policy refusal', labelWidth: 120 }),
  ]), gpui.column('extra.colors', [
  kit.ColorSwatch('extra.swatch', { color: { h: 0.125, s: 0.75, l: 0.25, a: 0.5 }, selected: true }),
  kit.ColorPicker('extra.picker', { value: { h: 0.625, s: 0.75, l: 0.5, a: 0.75 }, alpha: true, presets: [{ h: 0.125, s: 0.75, l: 0.25, a: 1 }], recent: [{ h: 0.875, s: 0.5, l: 0.75, a: 0.5 }] }),
  ])]),
]));
