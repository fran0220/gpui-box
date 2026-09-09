// Explicit fixture data: no host or product-backed values.
const { kit } = gpui;
const tab = gpui.state('overview');
const page = gpui.state(3);
const expanded = gpui.state(['details']);
const ratio = gpui.state(0.3);
gpui.mount(() => gpui.column('fixture', [
  gpui.text('title', 'Kit navigation and layout · fixture data'),
  kit.Tabs('tabs', { tabs: [{ id: 'overview', label: 'Overview' }, { id: 'activity', label: 'Activity', badge: '4' }], selected: tab.get() }, { select: id => tab.set(id) }),
  kit.Accordion('accordion', { sections: [{ id: 'details', title: 'Native child slot' }], expanded: expanded.get() }, { toggle: value => expanded.set(value.expanded ? [value.id] : []) }, {
    details: [gpui.text('details.text', 'This text is mounted in the section body.')],
  }),
  kit.Divider('divider', { label: 'Asymmetric native split' }),
  kit.SplitPane('split', { ratio: ratio.get(), minStart: 80, minEnd: 80 }, { resize: value => ratio.set(value) }, {
    start: [gpui.text('start.text', 'Start pane · 30%')],
    end: [kit.ScrollArea('scroll', { height: 120, label: 'Scrollable fixture content' }, {}, { content: [
      gpui.text('end.text', 'End pane · native scroll area'),
      ...Array.from({ length: 8 }, (_, index) => gpui.text(`item.${index}`, `Fixture line ${index + 1}`)),
    ] })],
  }),
  kit.Pagination('pages', { page: page.get(), totalPages: 8 }, { select: value => page.set(value) }),
  gpui.text('state', `Tab: ${tab.get()} · page: ${page.get()}`),
]));
