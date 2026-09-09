const last=gpui.state('Caller-owned upload states; no transfer or file read');
gpui.mount(()=>{
 const rows=[
  {id:'queued',name:'Queued fixture',state:{state:'queued'}},
  {id:'running',name:'Unknown-length fixture',state:{state:'uploading',fraction:null}},
  {id:'done',name:'Complete fixture',state:{state:'done'}},
  {id:'failed',name:'Failed fixture',state:{state:'failed',reason:'Fixture failure'}},
  {id:'cancelled',name:'Cancelled fixture',state:{state:'cancelled'}},
  {id:'refused',name:'Refused fixture',state:{state:'refused',reason:'Fixture policy'}},
 ];
 const list=gpui.kit.UploadList('uploads.list',{uploads:rows},{retry:id=>last.set(`Retry intent: ${id}`),cancel:id=>last.set(`Cancel intent: ${id}`),remove:id=>last.set(`Remove intent: ${id}`)},{dropzone:[gpui.kit.Dropzone('uploads.zone',{label:'Internal rows only',hint:'External files are not authorized',accepts:['row']},{filesRefused:r=>last.set(r.reason),drop:item=>last.set(`Native drop: ${item.id}`)})]});
 return gpui.column('uploads.fixture',[
  gpui.text('uploads.title','Native UploadList / typed Dropzone · six caller-owned states'),
  gpui.text('uploads.last',last.get()),
  gpui.button('uploads.overall','Query native overall progress',async()=>{const value=await gpui.query(list,'overall');last.set(`Verified native progress: ${value.state}`);}),
  list,
  gpui.kit.Dropzone('uploads.refusal',{label:'Refusal fixture',state:'refusing',refusal:'External reads unavailable'}),
 ]);
});
