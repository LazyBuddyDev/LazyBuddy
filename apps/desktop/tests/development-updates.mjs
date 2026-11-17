// Update capability presentation only; no native host, downloads or shared preferences.
import assert from 'node:assert/strict';import {createServer} from 'vite';import {chromium} from 'playwright-core';
import {mkdir,mkdtemp,rm} from 'node:fs/promises';import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));const profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url));
await mkdir(profiles,{recursive:true});const scratch=await mkdtemp(profiles+'development-updates-');process.env.TEMP=scratch;process.env.TMP=scratch;
const server=await createServer({root,cacheDir:scratch+'/vite',server:{host:'127.0.0.1',port:0}});let browser;
try{
 await server.listen();browser=await chromium.launch({channel:'msedge',headless:true});
 for(const enabled of [false,true]){
   const page=await browser.newPage();await page.addInitScript(value=>window.fixtureAppUpdates=value,enabled);
   await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/tests/notice-fixture.html`);await page.waitForSelector('[data-events-ready=true]');
   await page.locator('.toolbar').getByRole('button',{name:'设置',exact:true}).click();await page.getByRole('button',{name:'系统设置',exact:true}).click();
   assert.equal(await page.getByRole('switch',{name:'应用自动更新'}).count(),enabled?1:0);
   assert.equal(await page.getByRole('button',{name:'检查更新',exact:true}).count(),enabled?1:0);
   await page.evaluate(()=>window.fixtureEvents['shell-update']({payload:true}));
   assert.equal(await page.getByRole('button',{name:'重启更新',exact:true}).count(),enabled?1:0);
   const settings=await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('settings',{}));assert.equal(settings.preferences.auto_update,false,'presentation never rewrites the preference');
   assert.equal(await page.getByRole('button',{name:'打开日志',exact:true}).count(),1);await page.close();
 }
 console.log('PASS: development hides update controls and ignores prepared notices; release retains controls; preferences and logs remain intact.');
}finally{await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});}
