import {test} from 'node:test';
import assert from 'node:assert/strict';
import {pageStart,progressLabel,highlights,readerError,type Entry} from '../src/reader.ts';
import {active,accepts,type View} from '../src/lifecycle.ts';
test('reading positions map to bounded pages and count chapters from zero',()=>{
  assert.equal(pageStart('b150'),96);assert.equal(pageStart('b192'),192);
  assert.equal(progressLabel({position:null} as Entry),'未开始');
  assert.equal(progressLabel({position:{chapter_id:'c1'},chapters:10} as Entry),'第 2 / 10 章');
});
test('search highlighting retains the original text and treats markup as text',()=>{
  const text='😀<script>你好你好';const result=highlights(text,'你好');
  assert.equal(result.map(p=>p.text).join(''),text);assert.equal(result.filter(p=>p.match).length,2);
  assert.deepEqual(highlights('abc',''),[{text:'abc',match:false}]);
  assert.match(readerError('encoding_required'),/编码/);
});
test('reader shares the host lifecycle and rejects A to B to A results',()=>{
  const view:View={root:'reader',section:'content',visible:true,generation:1};
  assert.equal(active(view,'reader'),true);assert.equal(accepts({...view,generation:3},view),false);
  assert.equal(active({...view,section:'settings'},'reader'),false);assert.equal(active({...view,visible:false},'reader'),false);
});
