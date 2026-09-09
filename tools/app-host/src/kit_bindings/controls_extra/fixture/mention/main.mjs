const last=gpui.state('Caller-owned candidates; no directory or service');
gpui.mount(()=>{
 const mention=gpui.kit.MentionInput('mention.input',{rows:3,suggestions:{state:'ready',value:[{id:'denied',label:'Alpha unavailable',refusal:'Fixture policy'},{id:'usable',label:'Alpha usable',description:'Caller fixture',replacement:'@alpha'}]}},{accepted:({id})=>last.set(`Accepted native candidate: ${id}`)});
 return gpui.column('mention.fixture',[
  gpui.text('mention.title','Native MentionInput · real TextArea and unavailable candidates'),
  gpui.text('mention.last',last.get()),
  gpui.button('mention.open','Open native mention candidates',async()=>{
    const area=await gpui.query(mention,'editor');
    await gpui.invoke(area,'set_value',{value:'@Al'});
    await gpui.invoke(area,'set_selected_range',{range:{start:3,end:3}});
    const focus=await gpui.query(area,'focus_handle');await gpui.invoke(focus,'focus');
    const text=await gpui.query(area,'value');last.set(`Verified mention child: ${text}`);
  }),
  gpui.kit.MentionInput('mention.disabled',{value:'Disabled caller fixture',disabled:true,rows:2}),
  mention,
 ]);
});
