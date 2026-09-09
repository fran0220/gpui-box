// Explicit fixture data. The host never supplies product records here.
const position = gpui.state({ x: 80, y: 50 });
const selected = gpui.state('source');
const last = gpui.state('ready');
gpui.mount(() => gpui.column('canvas.fixture', [
  gpui.kit.NodeGraph('graph', {
    viewport: { offset: { x: 17, y: 31 }, zoom: 0.75 },
    nodes: [
      { id: 'source', ...position.get(), props: { title: 'Source', selected: selected.get() === 'source', width: 160, ports: [{ id: 'out', label: 'Output', direction: 'output' }] } },
      { id: 'sink', x: 450, y: 90, props: { title: 'Sink', selected: selected.get() === 'sink', width: 160, ports: [{ id: 'in', label: 'Input', direction: 'input' }] } },
    ],
    can_connect: [{ from: { node: 'source', port: 'out' }, to: { node: 'sink', port: 'in' } }],
  }, { event(event) {
    if (event.type === 'node_moved' && event.id === 'source') position.set(event.position);
    if (event.type === 'selection_changed') selected.set(event.ids[0] ?? '');
    last.set(JSON.stringify(event));
  } }, { 'source:content': [gpui.text('source.description', 'Caller-owned graph')] }),
  gpui.text('canvas.event', last.get()),
]));
