// Headless install/update progress; no network or real module changes.
import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {chromium} from 'playwright-core';
import {mkdir,mkdtemp,rm} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
const profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url));
const evidence=fileURLToPath(new URL('../../../tmp/verification-module-progress/',import.meta.url));
await mkdir(profiles,{recursive:true});await mkdir(evidence,{recursive:true});
const scratch=await mkdtemp(profiles+'module-progress-');process.env.TEMP=scratch;process.env.TMP=scratch;
const server=await createServer({root,server:{host:'127.0.0.1',port:0}});let browser;
try{
 await server.listen();browser=await chromium.launch({channel:'msedge',headless:true});
 const page=await browser.newPage({viewport:{width:620,height:600}});
 await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/tests/module-progress-fixture.html`);
 await page.waitForSelector('[data-events-ready=true]');
 await page.getByRole('button',{name:'功能',exact:true}).click();
 const rows=page.locator('.module-row');
 await rows.nth(0).getByRole('button',{name:'安装',exact:true}).click();
 const bar=rows.nth(0).getByRole('progressbar');await bar.waitFor();
 assert.equal(await bar.getAttribute('value'),null);
 assert.equal(await rows.nth(1).getByRole('progressbar').count(),0);
 await page.evaluate(()=>window.installation.progress({stage:'download',downloaded:50,total:200}));
 await page.waitForFunction(()=>document.querySelector('progress')?.value===25);
 assert.equal(await bar.getAttribute('aria-valuetext'),'正在下载 25%');
 for(const [width,dark] of [[620,false],[360,true]]){
   await page.setViewportSize({width,height:600});
   await page.locator('.app-shell').evaluate((el,dark)=>el.classList.toggle('dark',dark),dark);
   await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
   const [name,progress,actions]=await Promise.all(['.module-name','.module-progress-slot','.row-actions'].map(s=>rows.nth(0).locator(s).boundingBox()));
   assert.ok(name.x+name.width<=progress.x&&progress.x+progress.width<=actions.x,'bar stays between name and actions');
   assert.ok(actions.x+actions.width<=width,'actions fit the window');
   await page.screenshot({path:evidence+`progress-${width}.png`});
 }
 await page.evaluate(()=>window.installation.progress({stage:'verify',downloaded:200,total:200}));
 await page.waitForFunction(()=>document.querySelector('progress')?.getAttribute('value')===null);
 assert.equal(await bar.getAttribute('aria-valuetext'),'正在校验并解包');
 await page.evaluate(()=>window.installation.progress({stage:'install',downloaded:200,total:200}));
 await page.waitForFunction(()=>document.querySelector('progress')?.getAttribute('aria-valuetext')==='正在安装');
 await page.evaluate(()=>{window.oldInstallation=window.installation;window.installation.finish(false);});
 await bar.waitFor({state:'detached'});
 await rows.nth(1).getByRole('button',{name:'更新',exact:true}).click();
 await rows.nth(1).getByRole('progressbar').waitFor();
 await page.evaluate(()=>window.oldInstallation.progress({stage:'download',downloaded:99,total:100}));
 assert.equal(await rows.nth(1).getByRole('progressbar').getAttribute('value'),null,'late result cannot affect a new operation');
 await page.evaluate(()=>window.installation.finish(true));
 await rows.nth(1).getByRole('progressbar').waitFor({state:'detached'});
 assert.match(await page.locator('.notice').textContent(),/样例安装失败/);
 assert.equal(await rows.nth(1).getByRole('button',{name:'更新',exact:true}).isEnabled(),true);
 console.log('PASS: install/update progress, phases, per-row isolation, stale messages, failure cleanup, light/dark narrow layouts.');
}finally{await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});}
