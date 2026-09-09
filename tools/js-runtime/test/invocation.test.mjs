import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { once } from 'node:events';
import { Session } from '../session.mjs';

const source = `const input=gpui.kit.TextInput('input',{text:'é🙂abc'}); const result=gpui.state('ready');
gpui.mount(()=>gpui.column('root',[input,gpui.text('result',result.get())]));
gpui.command('query',async()=>{try {result.set(await gpui.query(input,'value',{}));}catch(e){result.set(e.message);}});
gpui.command('wrong',async()=>{try {await gpui.query({id:'input',component:'Select'},'value',{});}catch(e){result.set(e.message);}});
gpui.command('render',()=>result.set('replacement'));
`;
async function session(t) {
  const root = await mkdtemp(resolve(tmpdir(), 'gpui-invoke-'));
  await writeFile(resolve(root, 'main.mjs'), source);
  const s = new Session({ root, entry: 'main.mjs', sandbox: process.platform === 'linux' ? 'linux' : undefined, trusted: process.platform !== 'linux' });
  s.on('error', () => {});
  t.after(async () => { await s.stop(); await rm(root, { recursive: true, force: true }); });
  const ready = once(s, 'ready'); await s.start(); await ready;
  return s;
}
function rendered(s, expected) {
  return new Promise((resolve, reject) => {
    const matches = () => s.tree.children[1].text === expected;
    if (matches()) return resolve();
    const timer = setTimeout(() => { s.off('render', check); reject(new Error(`Expected ${expected}`)); }, 4000);
    function check() { if (matches()) { clearTimeout(timer); s.off('render', check); resolve(); } }
    s.on('render', check);
  });
}

test('actual worker correlates native queries with mounted typed identity and bounded results', async t => {
  const s = await session(t);
  let invoked = 0;
  s.on('invoke', request => {
    invoked++;
    assert.deepEqual(request.target, { id: 'input', component: 'TextInput' });
    assert.equal(request.mode, 'query');
    assert.equal(request.method, 'value');
    assert.equal(s.finishNative(request.id, request.revision, 'é🙂', undefined), true);
  });
  s.command('query'); await rendered(s, 'é🙂');
  s.command('wrong'); await rendered(s, 'Native target is not mounted with that component identity');
  assert.equal(invoked, 1);
  assert.equal(s.nativeRequests.size, 0);
});

test('rerender cancels pending native query; late reply cannot mutate replacement, disposal clears requests', async t => {
  const s = await session(t);
  s.on('invoke', () => {});
  const request = once(s, 'invoke'); s.command('query');
  const [old] = await request;
  s.command('render');
  await rendered(s, 'Native request cancelled by render revision change');
  assert.equal(s.finishNative(old.id, old.revision, 'late'), false);
  s.send({ kind: 'native-response', id: old.id, revision: old.revision, value: 'forged late' });
  await new Promise(resolve => setTimeout(resolve, 50));
  assert.equal(s.tree.children[1].text, 'Native request cancelled by render revision change');
  const pending = once(s, 'invoke'); s.command('query'); await pending;
  await s.stop();
  assert.equal(s.nativeRequests.size, 0);
});

test('missing native host is an explicit refusal rather than a fabricated query result', async t => {
  const s = await session(t);
  s.command('query');
  await rendered(s, 'Native invocation unavailable in this host');
});
