const last=gpui.state('Caller-owned option and tag fixtures');
const trimmed=gpui.state(false);
const options=[{id:'alpha',label:'Alpha',description:'First fixture',group:'Letters'},{id:'omega',label:'Omega',disabled:true,group:'Letters'},{id:'beta',label:'Beta',description:'Second fixture',group:'Letters'}];
gpui.mount(()=>{
  const combo=gpui.kit.Combobox('selection.combo',{options,selected:'alpha',allowCustom:true,...(trimmed.get()?{}:{placeholder:'Temporary fixture hint'})},{selected:id=>last.set(`Native selected: ${id}`),custom:text=>last.set(`Native custom: ${text}`)});
  const multi=gpui.kit.MultiSelect('selection.multi',{options,selected:['alpha','beta'],clearable:true},{removed:id=>last.set(`Native remove intent: ${id}`)});
  const tag=gpui.kit.TagInput('selection.tags',{tags:['alpha','omega','beta'],...(trimmed.get()?{}:{max:3,collapseAt:1,placeholder:'Temporary tag hint'})},{added:text=>last.set(`Native tag intent: ${text}`),refused:reason=>last.set(`Native refusal: ${reason}`)});
  return gpui.column('selection.fixture',[
    gpui.text('selection.title','Native selection controls · retained drafts and caller-owned intents'),
    gpui.text('selection.last',last.get()),
    gpui.button('selection.references','Verify actual retained editors',async()=>{
      const refs=await Promise.all([gpui.query(combo,'query_input'),gpui.query(multi,'query_input'),gpui.query(tag,'field')]);
      for(const ref of refs)await gpui.invoke(ref,'set_value',{value:'Fixture draft'});
      trimmed.set(true);
      const values=await Promise.all(refs.map(ref=>gpui.query(ref,'value')));
      last.set(`Retained native drafts: ${values.join(' / ')}`);
    }),
    combo,multi,tag,
    gpui.kit.Combobox('selection.disabled',{options,selected:'beta',disabled:true}),
    gpui.kit.MultiSelect('selection.invalid',{options,selected:['omega'],invalid:true,disabled:true}),
    gpui.kit.TagInput('selection.disabled.tags',{tags:['Refused fixture'],disabled:true,invalid:true}),
  ]);
});
