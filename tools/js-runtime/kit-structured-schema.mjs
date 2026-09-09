const s={type:'string',max:16384};
const id={type:'string',min:1,max:256};
const b={type:'boolean'};
const n={type:'number',min:-1e9,max:1e9};
const int={type:'number',integer:true,min:0,max:1000000};
const e=(...values)=>({enum:values});
const a=items=>({type:'array',items,max:1024});
const o=(fields,required=[])=>({type:'object',fields,required});
const nullable=s=>({...s,nullable:true});
const method=(fields,result=e(null))=>({args:o(fields,Object.keys(fields)),result});
const validation={oneOf:[o({state:e('pending','validating','valid'),reason:e(null)},['state']),o({state:e('invalid'),reason:s},['state','reason'])]};
const visibility=e('visible','hiddenInclude','hiddenOmit');
function value() {
  const branch=(kind,fields={})=>o({kind:e(kind),...fields},['kind',...Object.keys(fields)]);
  const variants=[branch('null'),branch('boolean',{boolean:b}),branch('number',{text:s}),branch('string',{text:s}),branch('redacted',{text:s})];
  {
    const child={$ref:'JsonValue'};
    variants.push(branch('array',{items:a(child)}),branch('object',{members:a(o({key:s,value:child},['key','value']))}),branch('identifiedObject',{members:a(o({id,key:s,value:child},['id','key','value']))}));
  }
  return {oneOf:variants};
}
function field() {
  const branch=(k,fields={},required=[])=>o({name:id,kind:e(k),label:s,description:s,required:b,...fields},['name','kind',...required]);
  const bounds={min:n,max:n,step:{...n,min:0.000001}}, choices=a(o({id,label:s,description:s},['id','label']));
  const variants=[branch('text',{placeholder:s,secret:b}),branch('number',bounds),branch('integer',bounds),branch('boolean'),branch('enum',{choices},['choices']),branch('openEnum',{choices},['choices']),branch('textList',{maxItems:int}),branch('date'),branch('time'),branch('dateRange'),branch('files',{maxItems:int}),branch('unrenderable',{reason:s},['reason'])];
  {const child={$ref:'SchemaField'};variants.push(branch('object',{fields:a(child)},['fields']),branch('list',{item:child,maxItems:int},['item']));}
  return {oneOf:variants};
}
const fieldValue={oneOf:[
  o({kind:e('text','choice'),text:s},['kind','text']),
  o({kind:e('number','itemCount','day'),number:n},['kind','number']),
  o({kind:e('boolean'),boolean:b},['kind','boolean']),
  o({kind:e('list','files'),items:a(s)},['kind','items']),
  o({kind:e('time'),hour:{...int,max:23},minute:{...int,max:59},second:nullable({...int,max:59})},['kind','hour','minute','second']),
  o({kind:e('range'),start:n,end:nullable(n)},['kind','start','end']),
  o({kind:e('absent','unrenderable')},['kind']),
]};
export const familySchemas=Object.freeze({
  JsonView:{props:{$defs:{JsonValue:value()},...o({value:{$ref:'JsonValue'},rootLabel:s,expanded:a(s),selected:s,visibleRows:{...int,min:1},rowHeight:{...n,min:0.01},disabled:b,size:e('xs','sm','md','lg')},['value'])},events:{toggle:o({path:s,expanded:b},['path','expanded']),select:s}},
  SchemaForm:{props:{$defs:{SchemaField:field()},...o({fields:a({$ref:'SchemaField'}),disabled:b},['fields'])},events:{change:s,submit:e(null),filesRequested:o({path:s,label:s,max:nullable(int)},['path','label','max'])}},
});
export const familyMethods=Object.freeze({
  JsonView:{invoke:{},query:{disclosed_paths:method({},a(s))}},
  SchemaForm:{invoke:{
    set_files:method({path:id,files:a(s)},b),add_list_item:method({path:id},b),remove_list_item:method({path:id,index:int},b),move_list_item:method({path:id,from:int,to:int},b),
    set_field_validation:method({path:id,validation},b),clear_field_validation:method({path:id},b),set_field_visibility:method({path:id,visibility},b),
    set_validation:method({validation}),clear_validation:method({}),set_error:method({path:id,message:s}),clear_host_errors:method({}),validate:method({},b),set_disabled:method({disabled:b}),
  },query:{
    field_validation:method({path:id},nullable(validation)),field_visibility:method({path:id},nullable(visibility)),validation:method({},nullable(validation)),
    values:method({},a(o({path:s,value:fieldValue},['path','value']))),submission_values:method({},a(o({path:s,value:fieldValue},['path','value']))),
    unrenderable:method({},a(o({path:s,label:s,required:b,reason:s},['path','label','required','reason']))),has_unrenderable_required:method({},b),
  }},
});

// Additional semantic checks run only after the closed grammar, before conversion.
export function validateFamilyProps(component,props) {
  function json(v) {
    if(v.kind==='number' && !/^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?$/.test(v.text)) throw new TypeError('Invalid JSON number spelling');
    for(const child of v.items??[]) json(child);
    for(const member of v.members??[]) json(member.value);
  }
  function fields(values) {
    if(new Set(values.map(v=>v.name)).size!==values.length) throw new TypeError('Duplicate schema field');
    for(const v of values) {
      if(/[.\[\]/]/.test(v.name)) throw new TypeError('Schema name contains path syntax');
      if(v.min!==undefined&&v.max!==undefined&&v.min>v.max) throw new TypeError('Reversed number bounds');
      if(v.fields) fields(v.fields);
      if(v.item) fields([v.item]);
    }
  }
  if(component==='JsonView')json(props.value);
  if(component==='SchemaForm')fields(props.fields);
}
