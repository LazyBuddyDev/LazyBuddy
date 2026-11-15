// Headless feature notice fixtures; no accounts or upstream requests.
import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {chromium} from 'playwright-core';
import {mkdir,mkdtemp,rm} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
const profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url));
await mkdir(profiles,{recursive:true});const scratch=await mkdtemp(profiles+'feature-notices-');
process.env.TEMP=scratch;process.env.TMP=scratch;
const server=await createServer({root,server:{host:'127.0.0.1',port:0}});let browser;
try{
  await server.listen();browser=await chromium.launch({channel:'msedge',headless:true});
  const page=await browser.newPage({viewport:{width:400,height:600}});
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/tests/notice-fixture.html`);
  await page.waitForSelector('[data-events-ready=true]');
  await page.getByLabel('选择功能').selectOption('news');
  await page.waitForSelector('.source-card');
  await page.getByLabel('选择功能').selectOption('hongguo');
  await page.waitForFunction(()=>document.querySelector('.notice').textContent.includes('已登录'));
  await page.getByLabel('选择功能').selectOption('news');
  await page.waitForSelector('.source-card');
  assert.doesNotMatch(await page.locator('.notice').textContent(),/已登录|用户样例/);
  await page.getByRole('button',{name:'刷新样例来源'}).click();
  await page.waitForFunction(()=>document.querySelector('.notice').textContent.includes('新闻请求失败'));
  await page.getByLabel('选择功能').selectOption('hongguo');
  await page.waitForFunction(()=>document.querySelector('.notice').textContent.includes('已登录'));
  assert.doesNotMatch(await page.locator('.notice').textContent(),/新闻请求失败/);
  console.log('Root switch clears the other feature notice and keeps current feature errors visible.');
}finally{await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});}
