import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { once } from 'node:events';
import { fileURLToPath } from 'node:url';
import { Session } from '../session.mjs';
import { validateValue, validateSlots } from '../kit-schema.mjs';
import { familySchemas, familyMethods, familyReferenceMethods, validateOverlayProps, generateOverlayTypes } from '../kit-overlay-schema.mjs';

test('actual worker revisions revoke pending overlay requests and update slot descriptors',async t=>{
  const session=new Session({root:fileURLToPath(new URL('../../app-host/src/kit_bindings/overlay_extra/fixture/',import.meta.url)),entry:'main.mjs',sandbox:process.platform==='linux'?'linux':undefined,trusted:process.platform!=='linux'});
  session.on('error',()=>{});t.after(()=>session.stop());
  const ready=once(session,'ready');await session.start();await ready;
  session.on('invoke',()=>{});
  const pending=once(session,'invoke');session.command('query');const [request]=await pending;
  assert.deepEqual(request.target,{id:'drawer',component:'Drawer'});
  assert.equal(request.method,'is_open');assert.equal(request.mode,'query');assert.deepEqual(request.args,{});
  const rendered=once(session,'render');session.command('rename');await rendered;
  assert.equal(session.tree.children[0].props.title,'Updated while open');
  assert.equal(session.tree.children[0].slots.content[0].text,'Updated while open');
  assert.equal(session.finishNative(request.id,request.revision,true),false);
  await session.stop();assert.equal(session.nativeRequests.size,0);
});

test('native fixtures execute every declared data method with closed bounded results',()=>{
  const fixtures=JSON.parse(readFileSync(new URL('../../app-host/src/kit_bindings/overlay_extra/fixture/methods.json',import.meta.url)));
  const exercised=new Set();
  for(const {component,props,steps} of fixtures){
    validateValue(props,familySchemas[component].props);
    for(const [name,args,result] of steps){
      const mode=name in familyMethods[component].query?'query':'invoke';
      const contract=familyMethods[component][mode][name];
      validateValue(args,contract.args);validateValue(result,contract.result);
      assert.throws(()=>validateValue({...args,undeclared:1},contract.args));
      exercised.add(`${component}.${mode}.${name}`);
    }
  }
  const declared=Object.entries(familyMethods).flatMap(([component,modes])=>Object.entries(modes).flatMap(([mode,methods])=>Object.keys(methods).filter(name=>!Object.hasOwn(familyReferenceMethods[component]?.[mode]??{},name)).map(name=>`${component}.${mode}.${name}`)));
  assert.deepEqual([...exercised].sort(),declared.sort());
  assert.equal(readFileSync(new URL('../kit-overlay-sdk.d.ts',import.meta.url),'utf8'),generateOverlayTypes());
});

test('registry-backed methods have closed kind-specific markers, never pointers or serialized entities',()=>{
  const focus={$nativeRef:'issued-by-host',type:'FocusHandle'};
  assert.deepEqual(Object.keys(familyReferenceMethods).sort(),['Drawer','HoverCard','Menu','ContextMenu','CommandPalette','NotificationCenter'].sort());
  for(const methods of Object.values(familyReferenceMethods)){
    const schema=methods.query.focus_handle;
    validateValue({},schema.args);validateValue(focus,schema.result);
    for(const invalid of [42,{...focus,ptr:42},{...focus,type:'TextInput'},{$nativeRef:'',type:'FocusHandle'}])assert.throws(()=>validateValue(invalid,schema.result));
  }
  validateValue({stops:[focus]},familyMethods.Drawer.invoke.set_focus_stops.args);
  validateValue({focus_stops:[focus]},familySchemas.Drawer.props);
  const input={$nativeRef:'issued-child',type:'TextInput'};
  validateValue(input,familyMethods.CommandPalette.query.query_input.result);
  assert.throws(()=>validateValue(input,familyMethods.Drawer.query.focus_handle.result));
});

test('overlay grammar is closed and submenu identity is recursive',()=>{
  const props={trigger:'Actions',items:[{kind:'submenu',id:'more',label:'More',items:[{kind:'check',id:'pin',label:'Pin',checked:true}]}]};
  validateValue(props,familySchemas.Menu.props);validateOverlayProps('Menu',props);
  assert.throws(()=>validateValue({...props,trigger_icon:{key:'arrow-left',weight:'bold'}},familySchemas.Menu.props));
  assert.throws(()=>validateOverlayProps('Menu',{items:[...props.items,{kind:'command',id:'pin',label:'Duplicate'}]}));
  assert.throws(()=>validateValue({items:[{kind:'separator',id:'divider',label:'Invalid'}]},familySchemas.Menu.props));
  assert.throws(()=>validateValue({title:'Drawer',focus_handle:42},familySchemas.Drawer.props));
  validateSlots(familySchemas.Drawer,{}, {content:[],footer:[]});
  assert.throws(()=>validateSlots(familySchemas.Drawer,{}, {unknown:[]}));
});
test('native command arguments use exact snake_case named arguments',()=>{
  validateValue({position:{x:37,y:91}},familyMethods.ContextMenu.invoke.open_at.args);
  assert.throws(()=>validateValue({x:37,y:91},familyMethods.ContextMenu.invoke.open_at.args));
  assert.throws(()=>validateValue({title:'changed',extra:true},familyMethods.Drawer.invoke.set_title.args));
  assert.throws(()=>validateValue({toast:{id:'notice',message:'Hello',handler:()=>{}}},familyMethods.ToastLayer.invoke.push.args));
  validateValue({toast:{id:'notice',message:'Hello',timeout:123,action:'Retry'}},familyMethods.ToastLayer.invoke.push.args);
  for(const methods of Object.values(familyMethods))for(const method of Object.values(methods.query))assert.throws(()=>validateValue({arbitrary:true},method.args));
});
