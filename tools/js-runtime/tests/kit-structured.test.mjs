import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {familySchemas,familyMethods,validateFamilyProps} from '../kit-structured-schema.mjs';
import {validateValue,schemaDefinitions,schemaType} from '../kit-schema.mjs';
import {createKitBindings} from '../kit-bindings.mjs';
const validate=(component,props)=>{validateValue(props,familySchemas[component].props);validateFamilyProps(component,props);};
test('recursive documents stay linear and enforce the shared data depth budget',()=>{
  let value={kind:'null'};
  for(let i=0;i<12;i++)value={kind:'array',items:[value]};
  validate('JsonView',{value});
  for(let i=0;i<5;i++)value={kind:'array',items:[value]};
  assert.throws(()=>validate('JsonView',{value}),/budget exceeded/i);
  let field={name:'leaf',kind:'text'};
  for(let i=0;i<10;i++)field={name:`level${i}`,kind:'object',fields:[field]};
  validate('SchemaForm',{fields:[field]});
  assert.ok(JSON.stringify(familySchemas).length<10000);
  for(const [name,contract] of Object.entries(familySchemas)){
    const definitions=schemaDefinitions(contract.props,`${name}Defs`);
    const root=schemaType(contract.props,`${name}Defs`);
    assert.ok(definitions.length+root.length<10000);
    assert.match(root,new RegExp(`${name}Defs\\[`));
  }
});
test('structured tagged values preserve lexical numbers and caller member identity',()=>{
  const props={value:{kind:'identifiedObject',members:[{id:'left',key:'same',value:{kind:'number',text:'1.10'}},{id:'right',key:'same',value:{kind:'redacted',text:'withheld'}}]}};
  const kit=createKitBindings(()=> 'action');const node=kit.JsonView('document',props);
  assert.deepEqual(node.props,props);
  props.value.members[0].value.text='2.00';assert.equal(node.props.value.members[0].value.text,'1.10');
  for(const bad of [{kind:'number',text:'01'},{kind:'null',text:'hidden'},{kind:'boolean'},{kind:'array',items:[{kind:'string',text:'s',callback:()=>{}}]}])assert.throws(()=>validate('JsonView',{value:bad}));
  assert.throws(()=>validate('JsonView',{value:{kind:'identifiedObject',members:[{id:'same',key:'a',value:{kind:'null'}},{id:'same',key:'b',value:{kind:'null'}}]}}));
  validate('JsonView',{value:{kind:'number',text:'-12.50e+9999'}});
});
test('schema fields and every declared method use closed bounded named arguments',()=>{
  validate('SchemaForm',{fields:[{name:'items',kind:'list',item:{name:'entry',kind:'object',fields:[{name:'title',kind:'text'}]}}]});
  for(const fields of [[{name:'x',kind:'list'}],[{name:'x',kind:'boolean',secret:true}],[{name:'a/b',kind:'text'}],[{name:'x',kind:'text'},{name:'x',kind:'number'}],[{name:'n',kind:'number',min:4,max:-3}]])assert.throws(()=>validate('SchemaForm',{fields}));
  const args={set_files:{path:'files',files:[]},add_list_item:{path:'items'},remove_list_item:{path:'items',index:0},move_list_item:{path:'items',from:1,to:0},set_field_validation:{path:'x',validation:{state:'invalid',reason:'host'}},clear_field_validation:{path:'x'},set_field_visibility:{path:'x',visibility:'hiddenInclude'},set_validation:{validation:{state:'validating'}},clear_validation:{},set_error:{path:'x',message:'host'},clear_host_errors:{},validate:{},set_disabled:{disabled:true},field_validation:{path:'x'},field_visibility:{path:'x'},validation:{},values:{},submission_values:{},unrenderable:{},has_unrenderable_required:{},disclosed_paths:{}};
  let methods=0;
  for(const modes of Object.values(familyMethods))for(const mode of Object.values(modes))for(const [name,schema] of Object.entries(mode)){
    methods++;validateValue(args[name],schema.args);assert.throws(()=>validateValue({...args[name],foreign:true},schema.args));
  }
  assert.equal(methods,21);
  assert.throws(()=>validateValue({path:'items',from:-1,to:0},familyMethods.SchemaForm.invoke.move_list_item.args));
  assert.throws(()=>validateValue({validation:{state:'invalid'}},familyMethods.SchemaForm.invoke.set_validation.args));
});
test('structured TypeScript rejects impossible variants and wrong method args',()=>{
  const tsc=fileURLToPath(new URL('../../app-host/node_modules/typescript/bin/tsc',import.meta.url));
  const fixture=fileURLToPath(new URL('../../app-host/src/kit_bindings/structured/fixture/types.mts',import.meta.url));
  const r=spawnSync(process.execPath,[tsc,'--strict','--noEmit','--module','nodenext','--target','ES2022',fixture],{encoding:'utf8'});assert.equal(r.status,0,r.stdout+r.stderr);
});
