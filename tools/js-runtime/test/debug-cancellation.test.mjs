import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {once} from 'node:events';
import {Session} from '../session.mjs';
import {nativeBackend} from '../sandbox.mjs';

async function worker(t) {
  const root=await mkdtemp(join(tmpdir(),'gpui-evaluation-'));
  await writeFile(join(root,'main.mjs'),"gpui.mount(()=>gpui.text('root','real isolated evaluator')); ");
  const session=new Session({root,entry:'main.mjs',sandbox:nativeBackend,debug:true});
  session.on('error',()=>{});
  t.after(async()=>{await session.stop();await rm(root,{recursive:true,force:true,maxRetries:3});});
  const ready=once(session,'ready',{signal:AbortSignal.timeout(7000)});
  await session.start();await ready;
  return session;
}

test('pre-aborted evaluation does not start or retire an otherwise healthy worker',async t=>{
  const session=await worker(t);
  await assert.rejects(session.debugEvaluate('throw new Error("must not run")',{signal:AbortSignal.abort()}),/cancelled/);
  assert.equal(session.closed,false);
  assert.equal(session.debugRequests.size,0);
  assert.equal((await session.debugEvaluate('17 + 31')).result.value,48);
  assert.equal(session.debugRequests.size,0);
});

test('cancel and timeout reap actual evaluator-created timers and outstanding inspector promises', {timeout:15000},async t=>{
  for(const mode of ['cancel','timeout']) {
    const session=await worker(t), abort=new AbortController();
    let ticks=0;
    session.on('log',message=>{if(message.message.includes('owned-evaluation-tick'))ticks++;});
    const evaluation=session.debugEvaluate("new Promise(() => { setInterval(() => console.log('owned-evaluation-tick'), 20); })",{signal:abort.signal});
    const rejected=assert.rejects(evaluation,mode==='cancel'?/cancelled/:/timed out/);
    const deadline=Date.now()+2000;
    while(!ticks&&Date.now()<deadline)await new Promise(resolve=>setTimeout(resolve,10));
    assert.ok(ticks>0,'real evaluated interval started before cancellation');
    if(mode==='cancel')abort.abort();
    await rejected;
    assert.equal(session.closed,true);
    assert.equal(session.childClosed,true,'rejection follows actual child-close/reap proof');
    assert.equal(session.debugRequests.size,0);
    assert.equal(session.operations.size,0);
    assert.equal(session.permissionWaiters.size,0);
    const stopped=ticks;
    await new Promise(resolve=>setTimeout(resolve,100));
    assert.equal(ticks,stopped,'no evaluation timer survives cancellation');
    await assert.rejects(session.debugEvaluate('1'),/unavailable/);
  }
});
