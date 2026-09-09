const last=gpui.state('Caller-owned branch fixtures; no data service');
gpui.mount(()=>{
  const options=[{id:'ready',label:'Ready fixture',children:{state:'ready',value:[{id:'leaf',label:'Selectable leaf'},{id:'disabled-leaf',label:'Disabled leaf',disabled:true}]}},{id:'loading',label:'Loading fixture',children:{state:'loading'}},{id:'idle',label:'Idle fixture',children:{state:'idle'}},{id:'empty',label:'Empty fixture',children:{state:'empty'}},{id:'error',label:'Failed fixture',children:{state:'error',reason:'Fixture fetch failed'}},{id:'refused',label:'Unavailable fixture',children:{state:'unavailable',reason:'Host refused fixture'}}];
  const cascade=gpui.kit.Cascader('cascade.fixture',{name:'Fixture branches',options},{selected:id=>last.set(`Native selection intent: ${id}`),expanded:id=>last.set(`Native expanded: ${id}`),retry:id=>last.set(`Retry intent: ${id}`)});
  return gpui.column('cascade.scene',[
    gpui.text('cascade.title','Native Cascader · branch state and retained path'),gpui.text('cascade.last',last.get()),
    gpui.button('cascade.open','Open native branch menu',async()=>{await gpui.invoke(cascade,'open',{});last.set(`Native menu open: ${await gpui.query(cascade,'is_open')}`);}),
    cascade,
    gpui.kit.Cascader('cascade.disabled',{name:'Disabled fixture',options,selected:'leaf',disabled:true}),
  ]);
});
