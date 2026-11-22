// Offline regression for paginated reader collections and lifecycle isolation.
import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {chromium} from 'playwright-core';
import {mkdir,mkdtemp,rm} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';

const root=fileURLToPath(new URL('../',import.meta.url));
const profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url));
await mkdir(profiles,{recursive:true});
const scratch=await mkdtemp(profiles+'reader-library-');
process.env.TEMP=scratch;process.env.TMP=scratch;
const server=await createServer({root,server:{host:'127.0.0.1',port:0}});
let browser;
try{
  await server.listen();
  browser=await chromium.launch({channel:'msedge',headless:true});
  const url=`http://127.0.0.1:${server.httpServer.address().port}/tests/notice-fixture.html`;
  for(const legacy of [false,true]){
    const page=await browser.newPage();
    const errors=[];page.on('pageerror',error=>errors.push(String(error)));
    await page.addInitScript(legacy=>{
      window.fixtureReader=true;window.fixtureLegacyPaging=legacy;window.fixturePersist=true;
      window.fixtureReadingPreferences={history_limit:100};
      localStorage.setItem('fixture.preferences',JSON.stringify({root:'reader'}));
      window.fixtureReaderEntries=Array.from({length:125},(_,i)=>({
        id:`book-${i}`,title:i===124?'远端匹配书':`藏书 ${i+1}`,author:'',
        source:{kind:'local',id:String(i),format:'txt',encoding:'utf-8'},content_version:'1',
        chapters:1,position:null,revision:0,updated_ms:125-i,visited_ms:i<81?81-i:0,
      }));
    },legacy);
    await page.goto(url);
    const nav=page.getByRole('navigation',{name:'书架分页'});
    const recent=page.getByRole('navigation',{name:'浏览记录分页'});
    const tabs=page.getByLabel('小说页签');
    const rootPicker=page.getByLabel('选择功能');
    const requests=()=>page.evaluate(()=>window.fixtureCalls
      .filter(({command,method})=>command==='reader_call'&&['library.list','history.list'].includes(method))
      .map(({method,args})=>({method,...args})));
    const shelfFilter=page.getByRole('textbox',{name:'筛选书名',exact:true});
    await nav.getByText('1–40 / 125 本').waitFor();
    assert.deepEqual(await requests(),[{method:'library.list',start:0,limit:40,...(legacy?{}:{query:''})}]);
    assert.equal(await page.locator('.reader-book').count(),40);
    await nav.getByRole('button',{name:'下一页'}).click();
    await nav.getByText('41–80 / 125 本').waitFor();
    await tabs.getByRole('button',{name:'浏览记录'}).click();
    await recent.getByText('1–40 / 81 本').waitFor();
    assert.deepEqual((await requests()).map(r=>r.method),['library.list','library.list','history.list']);
    await tabs.getByRole('button',{name:'书架',exact:true}).click();
    await nav.getByText('41–80 / 125 本').waitFor();
    await tabs.getByRole('button',{name:'在线搜书'}).click();
    await page.getByLabel('在线搜索书名或作者').waitFor();
    assert.equal((await requests()).length,3);
    await tabs.getByRole('button',{name:'书架',exact:true}).click();
    await nav.getByText('41–80 / 125 本').waitFor();
    await rootPicker.selectOption('news');
    await rootPicker.selectOption('reader');
    await nav.getByText('41–80 / 125 本').waitFor();
    assert.equal((await requests()).length,3);
    await shelfFilter.fill('远端匹配');
    await page.getByRole('button',{name:'继续阅读《远端匹配书》'}).waitFor();
    assert.equal(await page.locator('.reader-book').count(),1);
    if(legacy){
      assert.deepEqual((await requests()).slice(3).map(r=>[r.start,r.limit]),[[0,100],[100,100]]);
    }else{
      assert.deepEqual((await requests()).slice(3),[{method:'library.list',start:0,limit:40,query:'远端匹配'}]);
      await page.evaluate(()=>{window.fixtureHoldList=true;});
      await shelfFilter.fill('藏书 1');
      await page.waitForFunction(()=>window.fixtureListPending?.length===1);
      await rootPicker.selectOption('news');
      await page.locator('main').getByRole('button',{name:'全部来源',exact:true}).waitFor();
      await page.evaluate(()=>{window.fixtureHoldList=false;window.fixtureListPending.shift()();});
      await rootPicker.selectOption('reader');
      await page.getByRole('button',{name:'继续阅读《藏书 1》',exact:true}).waitFor();
      assert.equal(await page.getByRole('button',{name:'继续阅读《远端匹配书》'}).count(),0);
    }
    assert.deepEqual(errors,[]);
    await page.close();
  }
  console.log('Passed: bounded shelf and history paging, cached return, whole-list filtering, legacy compatibility and delayed-result isolation.');
}finally{
  await browser?.close();
  await server.close();
  await rm(scratch,{recursive:true,force:true});
}
