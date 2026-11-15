import test from 'node:test';import assert from 'node:assert/strict';
import {ordered,batches,refreshSources,badgeLabel,sourceFailure,type Preferences} from '../src/news.ts';
test('news queue never submits more than four sources per batch',()=>{const groups=batches(Array.from({length:11},(_,i)=>i));assert.deepEqual(groups.map(g=>g.length),[4,4,3]);assert.equal(new Set(groups.flat()).size,11);});
test('source preferences hide, filter and pin without mutating catalog',()=>{const sources=[{id:'a',name:'A'},{id:'b',name:'B'},{id:'c',name:'C'}];const prefs:Preferences={auto_refresh:true,refresh_minutes:5,followed:['b','c'],hidden:['c'],order:['b'],default_followed:false};assert.deepEqual(ordered(sources,prefs,false).map(s=>s.id),['b','a']);assert.deepEqual(ordered(sources,prefs,true).map(s=>s.id),['b']);assert.equal(sources.length,3);});

test('single source refresh forces only that visible source; automatic refresh respects freshness',()=>{
  const sources=[{id:'a',name:'A'},{id:'b',name:'B'},{id:'c',name:'C'}];
  const prefs:Preferences={auto_refresh:true,refresh_minutes:5,followed:['a'],hidden:['c'],order:[],default_followed:false};
  const cache={a:{updated_ms:600000},b:{updated_ms:0}};
  assert.deepEqual(refreshSources(sources,prefs,false,cache,true,'a',600001),['a']);
  assert.deepEqual(refreshSources(sources,prefs,false,cache,false,undefined,600001),['b']);
  assert.deepEqual(refreshSources(sources,prefs,false,cache,true,'c',600001),[]);
  assert.deepEqual(refreshSources(sources,prefs,true,cache,true,'b',600001),[]);
});

test('upstream badges have compact labels; missing and unknown badges leave no placeholder',()=>{
  assert.equal(badgeLabel('Hot'),'热');assert.equal(badgeLabel('New'),'新');assert.equal(badgeLabel('Explosive'),'爆');
  for(const value of [undefined,null,'','Unknown'])assert.equal(badgeLabel(value),undefined);
});

test('source failure shows a readable name and diagnostic without discarding cached stories',()=>{
 const sources=[{id:'x',name:'小红书热榜'}];
 const row={id:'x',ok:false,stories:[],updated_ms:0,error:{code:'source_failed',detail:'stage=request kind=timeout phase=Global'}};
 assert.match(sourceFailure(row,sources),/^小红书热榜：请求超时，保留已有内容/);
 assert.match(sourceFailure(row,sources),/stage=request/);
 assert.match(sourceFailure({...row,error:{code:'source_failed',detail:'stage=request kind=http_status status=403'}},sources),/上游拒绝访问/);
 assert.equal(sourceFailure({...row,error:undefined},sources),'小红书热榜：暂时无法获取，保留已有内容');
});
