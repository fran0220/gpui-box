const last=gpui.state('Caller-owned rich document; no HTML or remote provider');
const document={blocks:[
 {id:'intro',text:'Rich native fixture · AλZ',styles:[{range:{start:22,end:24},style:{italic:true}}]},
 {id:'item',text:'Caller-owned list item',paragraph:{alignment:'start',list:{kind:'ordered',depth:0}}},
]};
gpui.mount(()=>{
 const editor=gpui.kit.RichTextEditor('rich.editor',{name:'Rich fixture',document,rows:5,maxRows:10},{intentRefused:event=>last.set(event.reason)});
 return gpui.column('rich.fixture',[
  gpui.text('rich.title','Native RichTextEditor · document styles, session and toolbar'),
  gpui.text('rich.last',last.get()),
  gpui.button('rich.format','Format native rich document',async()=>{
   const session=await gpui.query(editor,'session');
   await gpui.invoke(editor,'apply_intent',{intent:{kind:'select',selection:{anchor:{block:'intro',offset:0},head:{block:'intro',offset:4}}}});
   await gpui.invoke(editor,'apply_intent',{intent:{kind:'toggleFormat',format:'bold'}});
   const doc=await gpui.query(session,'document');
   if(doc.blocks[0].styles[0].style.bold!==true)throw new Error('Native bold style missing');
   const focus=await gpui.query(editor,'focus_handle');await gpui.invoke(focus,'focus');
   last.set(`Verified rich style: bold · ${doc.blocks[0].text}`);
  }),
  editor,
  gpui.kit.RichTextEditor('rich.readonly',{document:{blocks:[{id:'readonly',text:'Read-only rich fixture',styles:[{range:{start:0,end:9},style:{bold:true}}]}]},readOnly:true,toolbar:false,rows:2}),
 ]);
});
