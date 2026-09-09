const last=gpui.state('Caller-owned fixture text; no language provider');
gpui.mount(()=>{
  const editor=gpui.kit.Editor('editors.editor',{label:'Native code fixture',value:'first line\nsecond line\nlast line',rows:4,languageServices:true});
  return gpui.column('editors.fixture',[
    gpui.text('editors.title','Native TextArea / Editor · retained documents and real child refs'),
    gpui.text('editors.last',last.get()),
    gpui.button('editors.references','Edit through native child',async()=>{
      const area=await gpui.query(editor,'text_area');
      await gpui.invoke(area,'set_value',{value:'Native λ document'});
      const focus=await gpui.query(area,'focus_handle');
      await gpui.invoke(focus,'focus');
      const snapshot=await gpui.query(editor,'snapshot');
      const child=await gpui.query(area,'snapshot');
      if(snapshot.text!==child.text||snapshot.revision!==child.revision)throw new Error('Native document mismatch');
      last.set(`Verified native document: ${snapshot.text}`);
    }),
    editor,
    gpui.kit.TextArea('editors.soft',{value:'Caller fixture · soft-wrapped native text area',rows:2,wrap:'soft'}),
    gpui.kit.TextArea('editors.readonly',{value:'Read-only fixture remains selectable',readOnly:true,rows:2,wrap:'none'}),
    gpui.kit.TextArea('editors.disabled',{value:'Disabled fixture',disabled:true,rows:2}),
  ]);
});
