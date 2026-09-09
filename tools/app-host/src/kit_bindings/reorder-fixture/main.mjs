// Explicit fixture: native controls emit intent, caller keeps the original order.
const rows = [{ id: 'alpha', label: 'Alpha' }, { id: 'beta', label: 'Beta' }, { id: 'gamma', label: 'Gamma' }];
const last = gpui.state('No drop');
const report = intent => last.set(`${intent.source}: ${intent.id} ${intent.position} ${intent.anchor}`);
gpui.mount(() => gpui.column('reorder-fixture', [
  gpui.text('reorder-title', 'Native reorder intent fixture'),
  gpui.kit.Tabs('reorder-tabs', { tabs: rows, selected: 'alpha', reorderable: true }, { reorder: report }),
  gpui.kit.List('reorder-list', { rows, selected: 'alpha', reorderable: true, visibleRows: 3, rowHeight: 40 }, { reorder: report }),
  gpui.text('reorder-result', last.get()),
  gpui.text('reorder-policy', 'Caller order stays Alpha, Beta, Gamma. No async predicate is installed.'),
]));
