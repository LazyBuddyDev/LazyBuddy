// Real hidden host + private fixture API. No upstream request, account or user installation.
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {mkdir,mkdtemp,writeFile,readFile,copyFile,readdir,realpath,rm} from 'node:fs/promises';
import path from 'node:path';import net from 'node:net';import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright-core';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../..');
const exe=path.resolve(process.argv[2]);const cli=path.resolve(process.argv[3]);
const base=path.join(root,'tmp/profiles');await mkdir(base,{recursive:true});
const profile=await mkdtemp(path.join(base,'host-logs-'));const data=path.join(profile,'data');
const fixture=path.join(profile,'fixture');await mkdir(path.join(fixture,'bin'),{recursive:true});
await copyFile(path.join(root,'tmp/verification-host-logs/service.exe'),path.join(fixture,'bin/service.exe'));
await writeFile(path.join(fixture,'module.json'),JSON.stringify({schema:1,id:'hongguo',name:'QA',version:'0.2.11',service_api:1,target:'windows-x86_64',entrypoint:'bin/service.exe',kind:'feature',page:{template:'media_library',search_method:'search',favorites_method:'favorites.read'},dependencies:[]}));
const env={...process.env,LAZYBUDDY_INSTANCE:`qa-log-${process.pid}`,LAZYBUDDY_DATA_DIR:data,LAZYBUDDY_PROTOTYPE_PROBE:'1',WEBVIEW2_USER_DATA_FOLDER:path.join(profile,'webview')};
delete env.LAZYBUDDY_BROWSER_PACKAGES;delete env.LAZYBUDDY_BROWSER_FIXTURE_FILE;
const run=(...args)=>JSON.parse(execFileSync(cli,args,{env,encoding:'utf8',windowsHide:true}));
run('modules','install-local',fixture);
const host=command=>run('host',JSON.stringify({version:1,command,args:{}}));
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(check){const end=Date.now()+30000;while(Date.now()<end){if(await check())return;await delay(100);}throw Error('host log check timed out');}
let child,browser;
async function start(){
 const socket=net.createServer();await new Promise(resolve=>socket.listen(0,'127.0.0.1',resolve));const port=socket.address().port;await new Promise(resolve=>socket.close(resolve));
 env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=`--remote-debugging-port=${port}`;
 child=spawn(exe,['--background'],{env,windowsHide:true,stdio:'ignore'});
 await until(async()=>{try{browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);return true;}catch{return false;}});
 let page;await until(()=>{page=browser.contexts().flatMap(context=>context.pages()).find(page=>page.url().includes('tauri.localhost'));return !!page;});
 await page.waitForSelector('[data-events-ready=true]');return page;
}
async function quit(){
 assert.equal(host('app.quit').ok,true);
 await until(()=>child.exitCode!==null);
 await browser.close().catch(()=>{});browser=undefined;child=undefined;
}
try{
 const page=await start();
 const invoke=(command,args={})=>page.evaluate(([command,args])=>window.__TAURI_INTERNALS__.invoke(command,args),[command,args]);
 await invoke('transition',{root:'hongguo',section:'content',visible:true,accountPage:false});
 const failure=await invoke('playback_call',{method:'playback.status',args:{},direct:true}).then(()=>'',String);
 assert.match(failure,/fixture_failure/);
 const log=path.join(data,'logs/LB.log');let text=await readFile(log,'utf8');
 assert.match(text,/playback.rs:\d+/);assert.match(text,/接口：playback.status/);
 assert.match(text,/app_input.py:37（resolve）/);assert.match(text,/HTTP状态：200/);assert.ok(!text.includes('SECRET'));
 assert.equal((await readdir(path.join(data,'logs'))).filter(name=>name.endsWith('.log')).length,1);
 await quit();text=await readFile(log,'utf8');const stamp=text.match(/closed_at=(\d{12})/)[1];
 await start();
 const backup=(await readdir(path.join(data,'logs'))).find(name=>name.startsWith(`LB_backup_${stamp}`));assert.ok(backup);
 assert.equal(await readFile(path.join(data,'logs',backup),'utf8'),text);
 assert.ok(!(await readFile(log,'utf8')).includes('fixture_failure'));await quit();
 const output=path.join(root,'tmp/verification-host-logs');await mkdir(output,{recursive:true});
 await writeFile(path.join(output,'sample-LB.log'),text);
 await writeFile(path.join(output,'result.json'),JSON.stringify({passed:true,api_error_logged_by_host:true,service_source_position:true,plain_text:true,secret_filtered:true,archive_close_timestamp:stamp,old_log_preserved:true,real_account_used:false},null,2));
 console.log('host LB.log API/error/source/close-time archive checks passed');
}finally{
 if(child&&child.exitCode===null)await quit();else await browser?.close().catch(()=>{});
 const resolved=await realpath(profile);assert.ok(resolved.startsWith(await realpath(base))&&path.basename(resolved).startsWith('host-logs-'));
 await rm(resolved,{recursive:true,force:true,maxRetries:15,retryDelay:300});
}
