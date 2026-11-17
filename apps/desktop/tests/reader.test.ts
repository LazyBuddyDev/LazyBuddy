import {test} from 'node:test';
import assert from 'node:assert/strict';
import {visibleAnchor,hasChapterTitle,pageStart,viewportStep,pageOffset,progressLabel,markedRuns,matchOffsets,wrapMatch,readerError,type Entry,type Block} from '../src/reader.ts';
import {active,accepts,type View} from '../src/lifecycle.ts';
test('reading positions map to bounded pages and count chapters from zero',()=>{
  assert.equal(pageStart('b150'),96);assert.equal(pageStart('b192'),192);
  assert.equal(progressLabel({source:{kind:'local'},position:null} as Entry),'未开始');
  assert.equal(progressLabel({source:{kind:'local'},position:{chapter_id:'c1'},chapters:10} as Entry),'第 2 / 10 章');
  assert.equal(progressLabel({source:{kind:'fanqie'},position:null,chapters:4423} as Entry),'共 4423 章');
  assert.match(readerError('source_body_unavailable'),/正文暂不可用/);
  assert.match(readerError('source_detail_required'),/重新载入目录/);
  assert.match(readerError('source_helper_missing'),/正文组件缺失/);
  assert.match(readerError('credential_read_failed'),/原有凭据已保留/);
  assert.equal(progressLabel({source:{kind:'fanqie'},position:{chapter_id:'123'},chapters:10} as Entry),'继续阅读 · 共 10 章');
});
test('search highlighting retains the original text and treats markup as text',()=>{
  const text='😀<script>你好你好';const result=markedRuns({text,runs:[]} as unknown as Block,'你好');
  assert.equal(result.map(p=>p.text).join(''),text);assert.equal(result.filter(p=>p.match).length,2);
  assert.equal(markedRuns({text:'abc',runs:[]} as unknown as Block,'')[0].match,false);
  assert.match(readerError('encoding_required'),/编码/);
});
test('reader shares the host lifecycle and rejects A to B to A results',()=>{
  const view:View={root:'reader',section:'content',visible:true,generation:1};
  assert.equal(active(view,'reader'),true);assert.equal(accepts({...view,generation:3},view),false);
  assert.equal(active({...view,section:'settings'},'reader'),false);assert.equal(active({...view,visible:false},'reader'),false);
});
test('online search failures stay readable and never look like empty results',()=>{
  assert.match(readerError('source_verification_required'),/验证/);
  assert.match(readerError('source_empty_response'),/空响应/);
  assert.match(readerError('source_access_denied'),/拒绝访问/);
  assert.match(readerError('source_text_obfuscated'),/无法直接显示/);
  assert.match(readerError('search_busy'),/收尾/);
  assert.match(readerError('incomplete_source_directory'),/目录不完整/);
  assert.match(readerError('invalid_source_book'),/书籍标识/);
});

test('current match spans styled runs and repeated matches wrap both ways',()=>{
  const block={text:'😀你好你好',runs:[{text:'😀你',strong:true,emphasis:false},{text:'好你好',strong:false,emphasis:true}]} as Block;
  assert.deepEqual(matchOffsets(block.text,'你好'),[2,4]);
  const parts=markedRuns(block,'你好',2);
  assert.equal(parts.filter(p=>p.current).map(p=>p.text).join(''),'你好');
  assert.equal(parts.filter(p=>p.match&&!p.current).map(p=>p.text).join(''),'你好');
  assert.equal(parts.map(p=>p.text).join(''),block.text);
  assert.equal(wrapMatch(-1,4),3);assert.equal(wrapMatch(4,4),0);assert.equal(wrapMatch(0,0),-1);
});

test('viewport page steps stay bounded and keep a line of context',()=>{
  assert.equal(viewportStep(500,38),456);assert.equal(viewportStep(20,38),38);
  assert.equal(pageOffset(0,400,1200,-1),0);assert.equal(pageOffset(800,400,950,1),950);
  assert.equal(pageOffset(400,400,1200,-1),0);assert.equal(pageOffset(0,400,1200,1),400);
});

test('chapter headings reuse source titles without changing body anchors',()=>{
 const blocks=(...texts:string[])=>texts.map((text,i)=>({id:`b${i}`,kind:'heading',text,runs:[],resource:null} as Block));
 assert.equal(hasChapterTitle(blocks('第2章','我们在看着你','“是……鬼吗？”'),'第2章 我们在看着你'),true);
 assert.equal(hasChapterTitle(blocks('第2章 我们在看着你'),'第2章 我们在看着你'),true);
 assert.equal(hasChapterTitle(blocks('第2章','\u200b','\u200b我们在看着你'),'第2章 我们在看着你'),true);
 assert.equal(hasChapterTitle(blocks('第一段正文'),'第2章 我们在看着你'),false);
 assert.equal(hasChapterTitle(blocks('第2章','不同标题'),'第2章 我们在看着你'),false);
 assert.equal(hasChapterTitle([{...blocks('第2章 我们在看着你')[0],id:'b96'}],'第2章 我们在看着你'),false);
 assert.equal(hasChapterTitle([],''),false);
});

test('horizontal anchors ignore a previous paragraph box without visible text',()=>{
 const saved=Object.getOwnPropertyDescriptor(globalThis,'document');
 const visible={top:20,bottom:40,left:10,right:90},previous={...visible,left:-90,right:-10};
 const paragraph={dataset:{block:'b4',kind:'paragraph'},textContent:'previous page',getClientRects:()=>[visible],textRects:[previous]};
 const image={dataset:{block:'b5',kind:'image'},textContent:'',getClientRects:()=>[visible]};
 Object.defineProperty(globalThis,'document',{configurable:true,value:{createRange:()=>{let selected:any;return {selectNodeContents:(e:any)=>selected=e,getClientRects:()=>selected.textRects};}}});
 try {assert.deepEqual(visibleAnchor({dataset:{pageMode:'horizontal'},getBoundingClientRect:()=>({top:0,bottom:100,left:0,right:100}),querySelectorAll:()=>[paragraph,image]} as unknown as HTMLElement),{block_id:'b5',offset:0});}
 finally {if(saved)Object.defineProperty(globalThis,'document',saved);else delete (globalThis as any).document;}
});
