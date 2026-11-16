// Real App pointer handling in a headless browser; no native cursor or accounts.
import assert from 'node:assert/strict';
import {createServer} from 'vite';import {chromium} from 'playwright-core';
import {mkdir,mkdtemp,rm} from 'node:fs/promises';import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
const profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url));
await mkdir(profiles,{recursive:true});const scratch=await mkdtemp(profiles+'dock-drag-');process.env.TEMP=scratch;process.env.TMP=scratch;
const server=await createServer({root,cacheDir:scratch+'/vite',server:{host:'127.0.0.1',port:0}});let browser;
try{
 await server.listen();browser=await chromium.launch({channel:'msedge',headless:true});const page=await browser.newPage({viewport:{width:40,height:40}});
 await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/tests/dock-drag-fixture.html`);await page.waitForSelector('[data-events-ready=true]');
 const button=page.getByRole('button',{name:'展开 LazyBuddy'});
 async function dock(){await page.evaluate(()=>{window.fixtureCalls=[];window.fixtureDock(true);});await button.waitFor();}
 const calls=()=>page.evaluate(()=>window.fixtureCalls);
 await dock();await button.click();assert.deepEqual(await calls(),['expand']);
 await dock();await page.mouse.move(20,20);await page.mouse.down();await page.mouse.move(22,20);
 assert.deepEqual(await calls(),[],'small movement remains a click');
 await page.mouse.move(27,20);await page.mouse.move(32,20);await page.mouse.up();
 assert.deepEqual(await calls(),['drag'],'drag starts once and never expands on mouse release');assert.equal(await button.count(),1);
 await button.press('Enter');assert.deepEqual(await calls(),['drag','expand'],'keyboard expansion works immediately after dragging');
 await dock();await page.mouse.move(20,20);await page.mouse.down();
 await button.dispatchEvent('pointercancel',{pointerId:1,isPrimary:true});await page.mouse.move(30,20);
 assert.deepEqual(await calls(),[],'cancellation disarms dragging');await page.mouse.up();
 await dock();await page.mouse.move(20,20);await page.mouse.down({button:'right'});await page.mouse.move(30,20);await page.mouse.up({button:'right'});
 assert.deepEqual(await calls(),[],'right button does not drag or expand');
 await dock();await page.evaluate(()=>window.fixtureDragError=true);await page.mouse.move(20,20);await page.mouse.down();await page.mouse.move(28,20);await page.mouse.up();
 assert.deepEqual(await calls(),['drag']);assert.match(await page.locator('.notice').textContent(),/拖动失败/);
 await page.evaluate(()=>window.fixtureDragError=false);await button.click();assert.deepEqual(await calls(),['drag','expand'],'a new click works after drag failure');
 console.log('PASS: click/drag threshold, one drag dispatch, drag-click suppression, keyboard, cancellation, right button and failure recovery.');
}finally{await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});}
