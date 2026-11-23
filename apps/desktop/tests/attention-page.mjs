// Headless host notifications: local fixtures only, no content requests or user accounts.
import assert from 'node:assert/strict';import {createServer} from 'vite';import {chromium} from 'playwright-core';
import {mkdir,mkdtemp,rm,writeFile} from 'node:fs/promises';import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));const profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url));
const evidence=fileURLToPath(new URL('../../../tmp/verification-attention/',import.meta.url));await mkdir(profiles,{recursive:true});await mkdir(evidence,{recursive:true});
const scratch=await mkdtemp(profiles+'attention-');process.env.TEMP=scratch;process.env.TMP=scratch;
const server=await createServer({root,cacheDir:scratch+'/vite',server:{host:'127.0.0.1',port:0}});let browser;
try{
 await server.listen();browser=await chromium.launch({channel:'msedge',headless:true});const page=await browser.newPage({viewport:{width:780,height:540}});
 await page.addInitScript(()=>{window.fixturePersist=true;window.fixtureModules=[{id:'bilibili',name:'B站',version:'0.1.2',installed:false,update:false},{id:'news',name:'新闻',version:'0.1.2',available_version:'0.1.3',installed:true,update:true}];});
 await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/tests/notice-fixture.html`);await page.waitForSelector('[data-events-ready=true]');
 const modules=page.getByRole('button',{name:'功能',exact:true}),settings=page.getByRole('button',{name:'设置',exact:true});
 const dot=target=>target.locator('.attention-unread');await dot(modules).waitFor();
 await modules.hover();await page.waitForFunction(()=>!!JSON.parse(localStorage.getItem('fixture.preferences')).attention_receipts?.['modules/new:bilibili']);assert.equal(await dot(modules).count(),0);
 await page.locator('.brand-logo').hover();await page.reload();await page.waitForSelector('[data-events-ready=true]');assert.equal(await dot(modules).count(),0,'hover acknowledgement survives reload');
 await modules.dispatchEvent('click');await page.getByRole('heading',{name:'可用功能'}).waitFor();
 await page.evaluate(()=>{window.fixtureModules[1].available_version='0.1.4';window.fixtureEvents['shell-catalog']({payload:true});});await dot(modules).waitFor();
 assert.equal(await page.locator('.module-row .attention-unread').count(),1,'only new update revision is unread');
 const revision=await page.locator('.module-row').nth(1).getByText('新闻',{exact:true});await revision.hover();assert.equal(await dot(modules).count(),0,'child hover also clears aggregate');
 await page.evaluate(()=>{window.fixtureUpdateReady=true;window.fixtureUpdateBuild='1.0.7:build-A';window.fixtureEvents['shell-update']({payload:true});});await dot(settings).waitFor();
 await settings.focus();await page.waitForFunction(()=>!document.querySelector('button[aria-label="设置"] .attention-unread'));assert.equal(await page.getByRole('button',{name:'重启更新'}).count(),1,'read acknowledgement does not apply update');
 await page.evaluate(()=>{window.fixtureUpdateBuild='1.0.7:build-B';window.fixtureEvents['shell-update']({payload:true});});await dot(settings).waitFor();
 await page.evaluate(()=>{window.fixtureModules[1].available_version='0.1.5';window.fixtureEvents['shell-catalog']({payload:true});});await dot(modules).waitFor();
 await page.screenshot({path:evidence+'light.png'});
 await page.locator('.brand-logo').hover();await page.setViewportSize({width:440,height:540});await page.evaluate(()=>document.querySelector('.app-shell').classList.add('dark'));assert.equal(await dot(modules).count(),1);assert.equal(await dot(settings).count(),1);assert.equal(await dot(modules).evaluate(e=>getComputedStyle(e).display),'block');assert.equal(await dot(settings).evaluate(e=>getComputedStyle(e).visibility),'visible');await page.screenshot({path:evidence+'dark-narrow.png'});
 await page.evaluate(()=>window.fixtureFailSettings=true);await modules.hover();await page.waitForFunction(()=>document.querySelector('.notice').textContent.includes('设置保存失败'));assert.equal(await dot(modules).count(),1,'failed persistence restores unread dot');
 await page.evaluate(()=>window.fixtureFailSettings=false);await page.locator('.brand-logo').hover();await modules.hover();await page.waitForFunction(()=>!document.querySelector('button[aria-label="功能"] .attention-unread'));
 assert.equal(await dot(settings).count(),1,'independent update source remains unread');
 await page.evaluate(()=>window.fixtureEvents['shell-visibility']({payload:false}));await page.waitForFunction(()=>document.querySelector('.app-shell').dataset.visible==='false');
 await settings.dispatchEvent('pointerenter',{pointerType:'mouse'});assert.equal(await dot(settings).count(),1,'hidden UI cannot acknowledge');
 assert.equal(await page.evaluate(()=>window.fixtureCalls.some(c=>c.command==='apply_update')),false);
 await writeFile(evidence+'report.json',JSON.stringify({passed:true,hover_persists:true,child_parent_aggregation:true,new_revision:true,same_version_build_identity:true,save_failure_rollback:true,hidden_no_ack:true,no_update_applied:true},null,2));console.log('Notification routing, hover/focus, persistence, new versions, failure rollback and hidden isolation passed.');
}finally{await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});}
