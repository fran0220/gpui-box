const last = gpui.state('No child action requested');
gpui.mount(() => gpui.column('groups.fixture', [
  gpui.text('groups.title', 'Native typed ButtonGroup · fixture actions'),
  gpui.kit.ButtonGroup('groups.ready', { size: 'lg' }, {}, { buttons: [
    gpui.button('groups.legacy', 'Legacy child', () => last.set('Legacy child action retained')),
    gpui.kit.Button('groups.native', { label: 'Native child', checkedState: true }, { click: () => last.set('Native child action retained') }),
    gpui.kit.Button('groups.refused', { label: 'Refused child', disabled: true }),
  ] }),
  gpui.text('groups.last', last.get()),
  gpui.text('groups.disabled.label', 'Disabled parent refuses all child actions'),
  gpui.kit.ButtonGroup('groups.disabled', { disabled: true }, {}, { buttons: [
    gpui.kit.Button('groups.disabled.first', { label: 'First' }, { click: () => last.set('Forbidden first action') }),
    gpui.kit.Button('groups.disabled.second', { label: 'Second' }, { click: () => last.set('Forbidden second action') }),
  ] }),
]));
