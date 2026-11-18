import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {chromium} from 'playwright-core';
import {mkdir,mkdtemp,rm} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
const profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url));
await mkdir(profiles,{recursive:true});const scratch=await mkdtemp(profiles+'reader-sources-');
process.env.TEMP=scratch;process.env.TMP=scratch;
const server=await createServer({root,server:{host:'127.0.0.1',port:0}});let browser;
try{
  await server.listen();browser=await chromium.launch({channel:'msedge',headless:true});
  const page=await browser.newPage({viewport:{width:780,height:640}}),errors=[];
  page.on('pageerror',e=>errors.push(String(e)));
  await page.addInitScript(()=>{window.fixtureReaderSources=['fanqie','qimao','shuqi'];window.fixtureLegacySourceInfo=!!localStorage.getItem('fixture.oldReader');});
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/tests/notice-fixture.html?reader-online&reader-body`);
  await page.waitForSelector('[data-events-ready=true]');
  await page.getByRole('button',{name:'在线搜书',exact:true}).click();
  const source=page.getByRole('combobox',{name:'搜索源'});
  assert.deepEqual(await source.locator('option').allTextContents(),['番茄小说（App）','七猫小说（App）']);
  for(const kind of ['fanqie','qimao']){
    await source.selectOption(kind);
    assert.equal(await page.locator('[aria-label=搜索结果分页]').count(),0,'source changes clear the previous cursor');
    await page.getByRole('textbox',{name:'在线搜索书名或作者'}).fill('同名书');
    await page.getByRole('button',{name:'搜索',exact:true}).click();
    await page.getByTitle('查看《同名书 · 1》详情与目录',{exact:true}).click();
    await page.getByRole('button',{name:'加入书架',exact:true}).click();
    await page.getByRole('button',{name:'移出书架',exact:true}).waitFor();
    await page.getByTitle('阅读此书',{exact:true}).click();
    await page.getByRole('region',{name:'小说正文'}).waitFor();
    await page.getByRole('button',{name:'在线搜书',exact:true}).click();
    await page.getByRole('button',{name:'返回搜索结果',exact:true}).click();
    await source.waitFor();
  }
  const entries=await page.evaluate(()=>window.fixtureReaderEntries);
  assert.equal(entries.length,2);assert.equal(new Set(entries.map(e=>e.id)).size,2);
  assert.deepEqual(entries.map(e=>e.source.kind),['fanqie','qimao']);
  const calls=await page.evaluate(()=>window.fixtureCalls.filter(c=>c.command==='reader_call'));
  assert.deepEqual(calls.filter(c=>c.method==='online.detail').map(c=>c.args.source),['fanqie','qimao']);
  assert.equal(calls.filter(c=>c.method==='online.chapter').length,2);
  assert.deepEqual(errors,[]);
  // Old installed modules advertise only the original source.
  await page.evaluate(()=>localStorage.setItem('fixture.oldReader','1'));
  await page.reload();await page.getByRole('button',{name:'在线搜书',exact:true}).click();
  assert.deepEqual(await source.locator('option').allTextContents(),['番茄小说（App）']);
  console.log('Passed: source choices, cursor reset, source-aware details/shelves, actual body dispatch and App channel gating and rejection of legacy web-only source choices.');
}finally{await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});}
