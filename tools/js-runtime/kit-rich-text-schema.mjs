// RichTextDocument data, not HTML, a plain-text fallback, or an entity marker.
const string={type:'string',max:16384};
const id={...string,min:1,max:256};
const integer={type:'number',integer:true,min:0,max:1000000};
const bool={type:'boolean'};
const choice=(...values)=>({enum:values});
const object=(fields,required=Object.keys(fields))=>({type:'object',fields,required});
const array=items=>({type:'array',items,max:1024});
const method=(fields,result)=>({args:object(fields),result});
const range=object({start:integer,end:integer});
const position=object({block:id,offset:integer});
const selection=object({anchor:position,head:position});
const listKind={...choice('ordered','unordered'),nullable:true};
const align=choice('start','center','end');
const format=choice('bold','italic','underline','strike','code');
const inlineFields={bold:bool,italic:bool,underline:bool,strike:bool,code:bool,link:{...string,nullable:true}};
const inline=object(inlineFields,[]);
const paragraphFields={alignment:align,list:{...object({kind:choice('ordered','unordered'),depth:{...integer,max:255}}),nullable:true}};
const paragraph=object(paragraphFields,[]);
const block=object({id,text:string,styles:array(object({range,style:inline})),paragraph},['id','text']);
export const richDocumentSchema=object({blocks:{...array(block),min:1}});
const documentResult=object({blocks:{...array(object({id,text:string,styles:array(object({range,style:object(inlineFields)})),paragraph:object(paragraphFields)})),min:1}});
const input=choice('typing','deleting','paste','cut');
export const richIntentSchema={oneOf:[
 object({kind:choice('select'),selection}),object({kind:choice('replace'),text:string,input}),
 object({kind:choice('replaceMultiline'),text:string,newBlocks:array(id),input}),
 object({kind:choice('hardBreak'),newBlock:id}),object({kind:choice('softBreak','backspaceAtStart','endComposition','undo','redo')}),
 object({kind:choice('toggleFormat'),format}),object({kind:choice('setLink'),destination:{...string,nullable:true}}),
 object({kind:choice('setAlignment'),alignment:align}),object({kind:choice('setList'),list:listKind}),
 object({kind:choice('changeListDepth'),delta:{type:'number',integer:true,min:-128,max:127}}),
 object({kind:choice('compose'),text:string,selection:{...range,nullable:true}}),
]};
const result=object({documentChanged:bool,selectionChanged:bool,pendingStyleChanged:bool});
export const richSessionMethods=Object.freeze({invoke:{},query:{
 document:method({},documentResult),selection:method({},selection),pending_style:method({},object(inlineFields)),
 marked_range:method({},{...object({start:position,end:position}),nullable:true}),can_undo:method({},bool),can_redo:method({},bool),
}});
const rows={...integer,min:1,max:1000};
export const richTextSchema=Object.freeze({props:object({document:richDocumentSchema,name:string,placeholder:string,frame:choice('own','host'),toolbar:bool,rows,maxRows:rows,disabled:bool,readOnly:bool,required:bool,invalid:bool},['document']),events:{intentApplied:object({intent:richIntentSchema,result}),intentRefused:object({intent:richIntentSchema,reason:string}),linkRequested:selection,focus:choice(null),blur:choice(null)}});
export const richTextMethods=Object.freeze({invoke:{
 set_diagnostics:method({diagnostics:array(object({range:object({start:position,end:position}),severity:choice('info','warning','error')}))},choice(null)),
 apply_intent:method({intent:richIntentSchema},choice(null)),replace_document:method({document:richDocumentSchema,selection},choice(null)),forbid_history:method({},choice(null)),
 set_name:method({name:string},choice(null)),set_placeholder:method({placeholder:string},choice(null)),set_frame:method({frame:choice('own','host')},choice(null)),set_toolbar:method({visible:bool},choice(null)),set_rows:method({rows},choice(null)),set_max_rows:method({max_rows:{...rows,nullable:true}},choice(null)),set_disabled:method({disabled:bool},choice(null)),set_read_only:method({read_only:bool},choice(null)),set_required:method({required:bool},choice(null)),set_invalid:method({invalid:bool},choice(null)),
},query:{...richSessionMethods.query,is_disabled:method({},bool),is_read_only:method({},bool),session:method({},object({$nativeRef:id,type:choice('RichTextEditSession')})),focus_handle:method({},object({$nativeRef:id,type:choice('FocusHandle')}))}});
