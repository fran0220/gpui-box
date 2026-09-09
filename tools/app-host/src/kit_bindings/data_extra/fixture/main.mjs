// Explicit synthetic fixture. All data and resulting edits stay in this caller.
const {kit}=gpui;
const selected=gpui.state(['row-2']);
const editing=gpui.state({row:'row-2',column:'name',value:'Fixture 2'});
const rows=Array.from({length:173},(_,i)=>({id:`row-${i}`,label:`Fixture ${i}`,cells:[{id:'name',text:`Fixture ${i}`},{id:'quantity',text:`${i*7+3}`},{id:'status',text:i%2?'Pending':'Ready'}]}));
gpui.mount(()=>gpui.column('data-fixture',[
  gpui.text('data-title','DATA · synthetic caller-owned fixtures'),
  kit.DataGrid('grid',{rows,columns:[{id:'name',header:'Name',fixed:250,pinned:true,editable:true},{id:'quantity',header:'Quantity',fixed:150,align:'end',sortable:true},{id:'status',header:'Status',fixed:200}],selected:selected.get(),selectionMode:'multiple',editing:editing.get(),visibleRows:4,lines:'rows'}, {select:v=>{if(v.kind==='replace')selected.set([v.id]);},edit:()=>editing.set(null)}),
  kit.BulkBar('bulk',{count:selected.get().length,total:173,noun:'fixture rows'},{dismiss:()=>selected.set([])}),
  kit.TreeGrid('hierarchy',{rows:[{id:'root',label:'Workspace',level:1,hasChildren:true,expanded:true,cells:[{id:'name',text:'Workspace'},{id:'count',text:'3'}]},{id:'child',label:'Nested record',level:2,parent:'root',cells:[{id:'name',text:'Nested record'},{id:'count',text:'17'}]}],columns:[{id:'name',header:'Hierarchy',fixed:350,pinned:true},{id:'count',header:'Count',fixed:180}],visibleRows:2,selected:'child'}),
  kit.DiagnosticsList('diagnostics',{state:'ready',diagnostics:[{id:'warning',severity:'warning',location:'Fixture document · 8:3',message:'Caller-owned warning; no file is opened.'}],visibleRows:2}),
  kit.Tree('tree',{nodes:[{id:'folder',label:'Fixture branch',children:[{id:'leaf',label:'Child row'}]},{id:'refused',label:'Unavailable branch',branch:'unavailable',reason:'Fixture host refused'}],expanded:['folder','refused'],visibleRows:4}),
]));
