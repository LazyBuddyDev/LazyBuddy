import {test} from 'node:test';import assert from 'node:assert/strict';
import {AttentionCenter,acknowledgeAttention,unreadAttention,moduleAttention,updateAttention} from '../src/attention.ts';
test('registered sources route to parent and child; acknowledging a snapshot cannot clear future versions',()=>{
  const center=new AttentionCenter();const source=center.register('modules');
  source.publish(moduleAttention([{id:'bilibili',version:'0.1.2',installed:false,update:false}]));
  const old=unreadAttention(center.items(),{},'modules');assert.equal(old.length,1);
  const seen=acknowledgeAttention({},old);assert.equal(unreadAttention(center.items(),seen,'module:bilibili').length,0);
  source.publish(moduleAttention([{id:'bilibili',version:'0.1.2',available_version:'0.1.3',installed:true,update:true}]));
  const first=unreadAttention(center.items(),seen,'modules');assert.equal(first[0].revision,'0.1.3');
  source.publish(moduleAttention([{id:'bilibili',version:'0.1.2',available_version:'0.1.4',installed:true,update:true}]));
  assert.equal(unreadAttention(center.items(),acknowledgeAttention(seen,first),'modules').length,1);
});
test('providers unregister independently and obsolete publishers cannot write through a replacement',()=>{
  let rendered=0;const center=new AttentionCenter(items=>rendered=items.length);const old=center.register('plugin');const other=center.register('host');
  other.publish(updateAttention('1.0.7:build-A'));old.dispose();const replacement=center.register('plugin');
  old.publish([{id:'late',revision:'1',targets:['modules']}]);old.dispose();assert.equal(center.items().length,1);
  replacement.publish([{id:'ready',revision:'1',targets:['modules']}]);assert.equal(center.items().length,2);
  assert.equal(rendered,2);
  assert.throws(()=>center.register('plugin'));other.dispose();assert.equal(center.items().length,1);assert.equal(rendered,1);
});
test('same-version application rebuilds re-notify, installed modules and unprepared downloads do not',()=>{
  assert.deepEqual(updateAttention(null),[]);assert.deepEqual(moduleAttention([{id:'news',version:'1.0.0',installed:true,update:false}]),[]);
  const center=new AttentionCenter();const source=center.register('host');source.publish(updateAttention('1.0.7:A'));
  const seen=acknowledgeAttention({},center.items());source.publish(updateAttention('1.0.7:B'));
  assert.equal(unreadAttention(center.items(),seen,'settings').length,1);
  assert.equal(unreadAttention(center.items(),JSON.parse(JSON.stringify(acknowledgeAttention(seen,center.items()))),'app.update').length,0);
});
