// Synthetic values, never product-backed data or host file acquisition.
const {kit}=gpui;
gpui.mount(()=>gpui.column('structured-fixture',[
  gpui.text('structured-title','STRUCTURED · identified duplicates and native form'),
  kit.JsonView('document',{value:{kind:'identifiedObject',members:[{id:'left',key:'same',value:{kind:'number',text:'1.10'}},{id:'right',key:'same',value:{kind:'redacted',text:'withheld shape'}},{id:'nested',key:'details',value:{kind:'object',members:[{key:'enabled',value:{kind:'boolean',boolean:false}},{key:'empty',value:{kind:'array',items:[]}}]}}]},expanded:['~2nested'],selected:'~2left',visibleRows:5,size:'sm'}),
  kit.JsonView('ambiguous',{value:{kind:'object',members:[{key:'same',value:{kind:'number',text:'1.10'}},{key:'same',value:{kind:'redacted',text:'shape'}}]}}),
  kit.SchemaForm('form',{fields:[{name:'title',kind:'text',label:'Caller title',placeholder:'Enter fixture title',required:true},{name:'enabled',kind:'boolean',label:'Enabled'},{name:'files',kind:'files',label:'Files (host policy not installed)'}]}),
]));
