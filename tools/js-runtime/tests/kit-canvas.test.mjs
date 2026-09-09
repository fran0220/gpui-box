import test from 'node:test';
import assert from 'node:assert/strict';
import { once } from 'node:events';
import { fileURLToPath } from 'node:url';
import { Session } from '../session.mjs';
import { validateValue, validateSlots } from '../kit-schema.mjs';
import { familySchemas, familyMethods, validateCanvasProps, canvasEventSchema } from '../kit-canvas-schema.mjs';

const graph={nodes:[{id:'a',x:-70,y:23,props:{title:'Source',ports:[{id:'out',label:'Output',direction:'output'}]}},{id:'b',x:420,y:83,props:{title:'Sink',ports:[{id:'in',label:'Input',direction:'input'}]}}],edges:[{id:'wire',from:'a',to:'b',ports:{from:'out',to:'in'},lane:-7}],can_connect:[{from:{node:'a',port:'out'},to:{node:'b',port:'in'}}]};
test('actual isolated worker owns graph movement state and rejects stale event routes',async t=>{
  const session=new Session({root:fileURLToPath(new URL('../../app-host/src/kit_bindings/canvas/fixture/',import.meta.url)),entry:'main.mjs',sandbox:process.platform==='linux'?'linux':undefined,trusted:process.platform!=='linux'});
  session.on('error',()=>{});t.after(()=>session.stop());
  const ready=once(session,'ready');await session.start();await ready;
  const graph=session.tree.children[0], revision=session.revision;
  const event={type:'node_moved',id:'source',position:{x:140,y:26}};
  const rendered=once(session,'render');session.event(graph.events.event,revision,session.generation,event);await rendered;
  while(session.tree.children[1].text!==JSON.stringify(event))await once(session,'render',{signal:AbortSignal.timeout(4000)});
  assert.equal(session.tree.children[0].props.nodes[0].x,140);
  assert.equal(session.tree.children[0].props.nodes[0].y,26);
  assert.equal(session.tree.children[1].text,JSON.stringify(event));
  assert.equal(session.event(graph.events.event,revision,session.generation,{...event,position:{x:999,y:999}}),false);
});
function validate(props){validateValue(props,familySchemas.NodeGraph.props);validateCanvasProps('NodeGraph',props);}
test('closed canvas grammar preserves caller topology and asymmetric coordinates',()=>{
  validate(graph);
  assert.deepEqual(Object.keys(familySchemas).sort(),['CanvasToolbar','GraphNode','Minimap','NodeGraph','NodeGroup'].sort());
  assert.deepEqual(familyMethods,{});
  for(const patch of [{zoom_range:{min:2,max:1}},{can_connect:[{from:{node:'a',port:'missing'},to:{node:'b',port:'in'}}]},{edges:[{id:'wire',from:'a',to:'absent'}]},{state:{kind:'refused'}},{state:{kind:'failed',reason:'offline',value:3}},{viewport:{offset:{x:0,y:0},zoom:Infinity}},{arbitraryFile:'/etc/passwd'}]) assert.throws(()=>validate({...graph,...patch}));
});
test('placed-node slots only accept currently declared identities and suffixes',()=>{
  validateSlots(familySchemas.NodeGraph,graph,{'a:content':[],'b:thumbnail':[],failed:[]});
  for(const name of ['a','c:content','a:footer','a:content:other'])assert.throws(()=>validateSlots(familySchemas.NodeGraph,graph,{[name]:[]}));
});
test('all graph event variants are discriminated and bounded',()=>{
  const events=[{type:'viewport_changed',offset:{x:-17,y:31},zoom:0.75},{type:'selection_changed',ids:['b','a']},{type:'node_moved',id:'a',position:{x:140,y:26}},{type:'node_resized',id:'a',size:{width:173,height:89}},{type:'node_deleted',id:'a'},{type:'surface_pressed',position:{x:1,y:2},button:'right',click_count:2},{type:'connection_requested',...graph.can_connect[0]},{type:'connection_dropped',from:graph.can_connect[0].from,at:{x:12,y:-7}},{type:'disconnect_requested',id:'wire'}];
  for(const event of events){validateValue(event,canvasEventSchema);assert.throws(()=>validateValue({...event,unexpected:true},canvasEventSchema));assert.throws(()=>validateValue({...event,type:'invalid'},canvasEventSchema));}
});
