// Installed external news API, native docking and live uninstall; no upstream requests.
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {mkdir,writeFile,realpath,rm} from 'node:fs/promises';
import path from 'node:path';import {fileURLToPath} from 'node:url';import net from 'node:net';import {chromium} from 'playwright-core';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../..');
const exe=path.resolve(process.argv[2]);assert.ok(process.env.LAZYBUDDY_TEST_NEWS_PACKAGE,'explicit external news package required');
const profile=path.join(root,'target/qa-web-host');const data=path.join(profile,'data');await mkdir(path.join(data,'services/news'),{recursive:true});
const env={...process.env,LAZYBUDDY_INSTANCE:`qa-host-${process.pid}`,LAZYBUDDY_DATA_DIR:data,LAZYBUDDY_PROTOTYPE_PROBE:'1',WEBVIEW2_USER_DATA_FOLDER:path.join(profile,'webview')};
delete env.LAZYBUDDY_BROWSER_PACKAGES;delete env.LAZYBUDDY_BROWSER_FIXTURE_FILE;
const cli=path.join(path.dirname(exe),'lazybuddy-cli.exe');
const run=(...args)=>JSON.parse(execFileSync(cli,args,{env,encoding:'utf8',windowsHide:true}));
run('modules','install-local',path.resolve(process.env.LAZYBUDDY_TEST_NEWS_PACKAGE));
const catalog=run('call','news',JSON.stringify({version:1,id:'sources',method:'sources',args:{}}));assert.equal(catalog.ok,true);
const cache=Object.fromEntries(catalog.data.map(s=>[s.id,{id:s.id,ok:true,updated_ms:Date.now(),stories:Array.from({length:10},(_,i)=>({title:`离线验收新闻 ${i+1}`,url:'https://example.invalid/'}))}]));
await writeFile(path.join(data,'services/news/state.json'),JSON.stringify({preferences:{auto_refresh:false,refresh_minutes:5,followed:[],hidden:[],order:[],default_followed:false},cache}));
const socket=net.createServer();await new Promise(r=>socket.listen(0,'127.0.0.1',r));const port=socket.address().port;await new Promise(r=>socket.close(r));
env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=`--remote-debugging-port=${port}`;
const child=spawn(exe,[],{env,windowsHide:true,stdio:'ignore'});let page,browser;const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function until(fn){const end=Date.now()+25000;while(Date.now()<end){if(child.exitCode!==null)throw Error('owned Web host exited');if(await fn())return;await delay(80);}throw Error('host condition timeout');}
const invoke=(name,args={})=>page.evaluate(([name,args])=>window.__TAURI_INTERNALS__.invoke(name,args),[name,args]);
const native=action=>JSON.parse(execFileSync('python',['-B',path.join(root,'apps/desktop/tests/native-dock.py'),'--pid',String(child.pid),'--action',action],{encoding:'utf8',windowsHide:true}));
try{
 await until(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`)).ok;}catch{return false;}});
 browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);await until(()=>{page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost'));return !!page;});
 await page.waitForSelector('[data-events-ready=true]');await invoke('window_action',{action:'show'});
 await until(async()=>await page.locator('.source-card li').count()===catalog.data.length*10);
 const requests=(await invoke('snapshot')).feed_requests;assert.equal(requests,2,'catalog/state only, cached news must not fetch');
 const stale=(await invoke('snapshot')).view.generation;
 await page.getByRole('button',{name:'设置',exact:true}).click();await page.getByRole('heading',{name:'新闻设置',exact:true}).waitFor();
 const late=await invoke('news_call',{method:'fetch',args:{sources:[catalog.data[0].id]},generation:stale}).then(()=>'',String);assert.match(late,/inactive_news_page/);
 await page.getByRole('button',{name:'显示设置',exact:true}).click();await page.getByRole('switch',{name:'深色模式'}).click();await until(async()=>await page.locator('.app-shell.dark').count()===1);
 await page.getByLabel('背景透明度',{exact:true}).fill('.5');await page.getByLabel('背景透明度',{exact:true}).dispatchEvent('change');
 await until(async()=>Number((await invoke('settings')).preferences.background_opacity)===.5);
 await page.reload();await page.waitForSelector('[data-events-ready=true]');assert.equal(await page.locator('.app-shell.dark').count(),1);
 assert.equal(Number((await invoke('settings')).preferences.background_opacity),.5);
 const left=native('drag-left');await page.getByRole('button',{name:'展开 LazyBuddy'}).waitFor();
 assert.deepEqual(left.after.slice(2),[Math.round(40*left.dpi/96),Math.round(40*left.dpi/96)]);assert.equal(left.region.corner,false);
 const hidden=(await invoke('snapshot')).feed_requests;await delay(400);assert.equal((await invoke('snapshot')).feed_requests,hidden);
 await page.getByRole('button',{name:'展开 LazyBuddy'}).click();await page.waitForFunction(()=>innerWidth>400);
 native('drag-right');await page.getByRole('button',{name:'展开 LazyBuddy'}).waitFor();await page.getByRole('button',{name:'展开 LazyBuddy'}).click();await page.waitForFunction(()=>innerWidth>400);
 await page.evaluate(()=>{const original=window.__TAURI_INTERNALS__.invoke;window.__TAURI_INTERNALS__.invoke=(name,args,...rest)=>name==='module_call'&&args.action==='catalog'?Promise.reject('offline catalog fixture'):original(name,args,...rest);});
 await page.getByRole('button',{name:'功能',exact:true}).click();await page.getByRole('heading',{name:'可用功能'}).waitFor();
 await page.getByRole('button',{name:'卸载',exact:true}).click();await until(async()=>await page.getByRole('button',{name:'卸载',exact:true}).count()===0);
 assert.equal(run('modules','list').length,0);assert.equal(await page.getByLabel('选择功能').isDisabled(),true);
 assert.equal(run('host',JSON.stringify({version:1,command:'status',args:{}})).data.backend,'web');
 await mkdir(path.join(root,'target/verification-web-host'),{recursive:true});
 await page.screenshot({path:path.join(root,'target/verification-web-host/modules.png')});
 const report={passed:true,installed_news_api:true,cached_news_no_network:true,news_rows:catalog.data.length*10,stale_fetch_rejected:true,theme_survives_reload:true,left_right_docking:true,circular_hit_region:true,hidden_requests_unchanged:true,live_uninstall:true,cli_web_status:true,real_account_used:false};
 await writeFile(path.join(root,'target/verification-web-host/host.json'),JSON.stringify(report,null,2));console.log(report);
}catch(error){if(page)console.error({notice:await page.locator('.notice').textContent(),state:await invoke('snapshot')});throw error;}
finally{
 if(page)await invoke('window_action',{action:'quit'}).catch(()=>{});await browser?.close().catch(()=>{});
 if(child.exitCode===null){await Promise.race([new Promise(r=>child.once('exit',r)),delay(15000)]);if(child.exitCode===null){child.kill();throw Error('host normal exit timed out');}}
 const resolved=await realpath(profile).catch(()=>null);if(resolved){assert.ok(resolved.startsWith(root+path.sep));await rm(resolved,{recursive:true,force:true,maxRetries:6,retryDelay:300});}
}
