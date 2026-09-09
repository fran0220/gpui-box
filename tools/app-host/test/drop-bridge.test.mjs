import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {once} from 'node:events';
import {Session} from '../../js-runtime/session.mjs';
import {DropBridge} from '../drop-bridge.mjs';

async function fixture(t, callback = 'async () => true', component = 'List') {
  const root=await mkdtemp(join(tmpdir(),'gpui-drop-bridge-'));
  await writeFile(join(root,'main.mjs'), `gpui.mount(()=>gpui.kit.${component}('rows',{${component==='Tree'?'nodes':component==='Tabs'?'tabs':'rows'}:[{id:'a',label:'A'},{id:'b',label:'B'}],reorderable:true},{},{},{accepts:${callback}}));`);
  const session=new Session({root,entry:'main.mjs',trusted:true});
  session.on('error',()=>{});
  t.after(async()=>{await session.stop();await rm(root,{recursive:true,force:true,maxRetries:3});});
  const first=once(session,'render',{signal:AbortSignal.timeout(5000)});
  await session.start();await first;
  const context={revision:19,sessions:[session],nativeTargets:new Map([[`${session.generation}:rows`,[{id:'app.rows.g9.m3',component}] ]])};
  const outputs=[];const bridge=new DropBridge(()=>context,packet=>outputs.push(packet));
  t.after(()=>bridge.close());
  const request=(patch={})=>({kind:'drop-request',id:73,instance:session.generation,revision:19,target:'app.rows.g9.m3',component,reference:session.tree.predicates.accepts,
    payload:{id:'b',source:'app.rows.g9.m3',label:'B',kind:'row',anchor:'a',position:'before',velocity:{x:-17,y:31},...(component==='Tree'?{icon:null}:{})},deadline:Date.now()+2000,...patch});
  return {session,context,outputs,bridge,request};
}

test('actual worker boolean/Promise decisions use native identity, Tree payload and exact correlation', async t=>{
  for (const [component,callback,accepted] of [['List','() => true',true],['Tabs','async () => false',false],['Tree','async intent => intent.velocity.x === -17 && intent.velocity.y === 31',true],['List','async () => "true"',false],['List','() => {throw new Error("refused")}',false]]) {
    const {bridge,outputs,request}=await fixture(t,callback,component);const packet=request();
    await bridge.request(packet);
    assert.deepEqual(outputs,[{kind:'drop-response',id:73,instance:packet.instance,revision:19,accepted}]);
    assert.equal(bridge.pending.size,0);
  }
});

test('stale revision, generation, removed/ambiguous mount, cancel and dispose abort real worker promises',async t=>{
  for (const change of ['revision','generation','removed','ambiguous','cancel','dispose']) {
    const f=await fixture(t,'() => new Promise(resolve => setTimeout(()=>resolve(true),180))');
    const packet=f.request();const pending=f.bridge.request(packet);
    assert.equal(f.session.predicateRequests.size,1);
    if(change==='revision')f.context.revision++;
    if(change==='generation')f.context.sessions=[];
    if(change==='removed')f.context.nativeTargets.clear();
    if(change==='ambiguous')f.context.nativeTargets.get(`${f.session.generation}:rows`).push({id:'second',component:'List'});
    if(change==='cancel')f.bridge.cancel({instance:packet.instance,id:73,revision:19});
    else if(change==='dispose')f.bridge.close();
    else f.bridge.reconcile();
    await pending;
    await new Promise(resolve=>setTimeout(resolve,220));
    assert.deepEqual(f.outputs,[],change);
    assert.equal(f.bridge.pending.size,0,change);
    assert.equal(f.session.predicateRequests.size,0,change);
    if(change==='dispose') {
      await f.bridge.request(f.request({id:74}));
      assert.deepEqual(f.outputs,[]);
      assert.equal(f.session.predicateRequests.size,0);
    }
  }
});

test('closed payload, wrong reference, expired deadline and real timeout refuse; duplicates never re-evaluate',async t=>{
  const f=await fixture(t,'() => new Promise(resolve => setTimeout(()=>resolve(true),180))');
  for(const patch of [{reference:'forged'},{payload:{...f.request().payload,extra:1}},{revision:18},{deadline:Date.now()-1}]) {
    await f.bridge.request(f.request(patch));assert.equal(f.outputs.at(-1).accepted,false);
  }
  f.outputs.length=0;
  const packet=f.request({deadline:Date.now()+40});
  const pending=f.bridge.request(packet);
  await f.bridge.request(packet);
  assert.equal(f.session.predicateRequests.size,1);
  await pending;
  assert.equal(f.outputs.length,1);assert.equal(f.outputs[0].accepted,false);
  assert.equal(f.session.predicateRequests.size,0);
});
