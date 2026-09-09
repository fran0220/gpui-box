import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {familySchemas,validateFamilyProps} from '../kit-data-schema.mjs';
import {validateValue} from '../kit-schema.mjs';
import {createKitBindings} from '../kit-bindings.mjs';

const validate=(component,props)=>{validateValue(props,familySchemas[component].props);validateFamilyProps(component,props);};
test('data constructors create real registered descriptors and typed intents',()=>{
  const actions=new Map();const kit=createKitBindings((id,event,handler)=>{actions.set(event,handler);return `${id}.${event}`;});
  let selection;
  const grid=kit.DataGrid('grid',{rows:[{id:'z',cells:[{id:'c',text:'3.1'}]}],columns:[{id:'c',header:'Count'}],selectionMode:'multiple'},{select:v=>selection=v});
  actions.get('select')({kind:'range',anchor:'z',to:'a'});
  assert.deepEqual(selection,{kind:'range',anchor:'z',to:'a'});
  assert.equal(grid.props.rows[0].cells[0].text,'3.1');
  assert.throws(()=>actions.get('select')({kind:'range',anchor:'z',to:'a',secret:3}));
  for(const [component,props] of Object.entries({BulkBar:{count:2},Flow:{rows:[]},Table:{rows:[],columns:[]},TreeGrid:{rows:[],columns:[]},Tree:{nodes:[]},ImageList:{items:[]},Masonry:{items:[]},KanbanBoard:{columns:[],cards:[]},DiagnosticsList:{state:'empty',diagnostics:[]}})){
    assert.equal(kit[component]('fixture',props).component,component);
  }
});
test('closed data rejects accessors, sparse data, foreign columns and ambiguous identity',()=>{
  const grid={rows:[{id:'one',cells:[{id:'a',text:'A'}]}],columns:[{id:'a',header:'A'}]};
  validate('DataGrid',grid);
  for(const bad of [{...grid,rows:[grid.rows[0],grid.rows[0]]},{...grid,rows:[{id:'one',cells:[{id:'b',text:'B'}]}]},{...grid,expanded:[{id:'other',index:0}]},{...grid,columns:[{id:'a',header:'A',fixed:30,flex:2}]},{...grid,rows:new Array(1)}])assert.throws(()=>validate('DataGrid',bad));
  assert.throws(()=>validate('Tree',{nodes:[{id:'same',label:'A',children:[{id:'same',label:'B'}]}]}));
  assert.throws(()=>validate('TreeGrid',{rows:[{id:'child',level:2,parent:'absent',cells:[]}],columns:[]}));
  assert.throws(()=>validate('Flow',{get rows(){assert.fail('getter evaluated');}}));
  assert.throws(()=>validate('Masonry',{items:[{id:'x',height:Infinity}]}));
  assert.throws(()=>validate('Tree',{nodes:[],accepts:()=>true}));
});
test('data TypeScript rejects wrong props and event payloads',()=>{
  const tsc=fileURLToPath(new URL('../../app-host/node_modules/typescript/bin/tsc',import.meta.url));
  const fixture=fileURLToPath(new URL('../../app-host/src/kit_bindings/data_extra/fixture/types.mts',import.meta.url));
  const r=spawnSync(process.execPath,[tsc,'--strict','--noEmit','--module','nodenext','--target','ES2022',fixture],{encoding:'utf8'});
  assert.equal(r.status,0,r.stdout+r.stderr);
});
