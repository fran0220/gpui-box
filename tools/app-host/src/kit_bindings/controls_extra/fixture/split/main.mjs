const last = gpui.state('No split action requested');
const items = [
  {kind:'command',id:'export',label:'Export fixture',shortcut:'ctrl-e'},
  {kind:'check',id:'pin',label:'Pin fixture',checked:true},
  {kind:'command',id:'refused',label:'Unavailable action',disabled:true},
  {kind:'submenu',id:'more',label:'More actions',items:[{kind:'command',id:'archive',label:'Archive fixture',destructive:true}]},
];
gpui.mount(() => gpui.column('split.fixture', [
  gpui.text('split.title','Native SplitButton · default and alternative fixture actions'),
  gpui.text('split.last',last.get()),
  gpui.kit.SplitButton('split.ready',{label:'Save fixture',variant:'primary',items}, {
    click:() => last.set('Default action retained'),
    open:() => last.set('Native alternatives opened'),
    invoked:id => last.set(`Alternative action: ${id}`),
  }),
  gpui.text('split.default.denied','Default refused; alternatives remain available'),
  gpui.kit.SplitButton('split.alternatives',{label:'Save unavailable',defaultDisabled:true,items}, {invoked:id => last.set(`Allowed alternative: ${id}`)}),
  gpui.text('split.whole.denied','Whole control disabled'),
  gpui.kit.SplitButton('split.disabled',{label:'Disabled save',disabled:true,items}),
]));
