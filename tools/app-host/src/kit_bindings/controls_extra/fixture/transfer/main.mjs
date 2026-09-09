// Fixture-owned sets: move requests deliberately do not apply themselves.
const last = gpui.state('No move requested');
const source = [{ id: 'alpha', label: 'Alpha' }, { id: 'beta', label: 'Beta' }, { id: 'locked', label: 'Locked item', disabled: true }];
const target = [{ id: 'zeta', label: 'Zeta' }];
gpui.mount(() => gpui.column('transfer.fixture', [
  gpui.text('transfer.title', 'Native transfer controls · fixture data'),
  gpui.kit.TransferList('transfer.enabled', { source, target, sourceSelected: ['beta'], targetSelected: ['zeta'], sourceLabel: 'Available', targetLabel: 'Assigned' }, {
    toggleSource: id => last.set(`Source selection requested: ${id}`),
    toggleTarget: id => last.set(`Target selection requested: ${id}`),
    moveToTarget: () => last.set('Move right refused by fixture host; sets unchanged'),
    moveToSource: () => last.set('Move left refused by fixture host; sets unchanged'),
  }),
  gpui.text('transfer.last', last.get()),
  gpui.kit.TransferList('transfer.disabled', { source, target, sourceSelected: ['alpha'], targetSelected: [], disabled: true, sourceLabel: 'Disabled source', targetLabel: 'Disabled target' }),
]));
