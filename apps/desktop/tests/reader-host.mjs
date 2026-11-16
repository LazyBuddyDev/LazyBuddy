// Real packaged reader + isolated WebView. Uses CDP; never moves the physical mouse.
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {mkdir,mkdtemp,writeFile,readFile,cp,rm,realpath} from 'node:fs/promises';
import {createInterface} from 'node:readline';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import net from 'node:net';
import {chromium} from 'playwright-core';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../..');
const exe=path.resolve(process.argv[2]),pack=path.resolve(process.argv[3]),newsPack=path.resolve(process.argv[4]);
const cli=path.join(path.dirname(exe),'lazybuddy-cli.exe');
await mkdir(path.join(root,'tmp/profiles'),{recursive:true});
const profile=await mkdtemp(path.join(root,'tmp/profiles/reader-'));const data=path.join(profile,'data');
const evidence=path.join(root,'tmp/verification-reader');await mkdir(evidence,{recursive:true});
const env={...process.env,LAZYBUDDY_INSTANCE:`qa-reader-${process.pid}`,LAZYBUDDY_DATA_DIR:data,LAZYBUDDY_PROTOTYPE_PROBE:'1',WEBVIEW2_USER_DATA_FOLDER:path.join(profile,'webview')};
delete env.LAZYBUDDY_BROWSER_PACKAGES;delete env.LAZYBUDDY_BROWSER_FIXTURE_FILE;
const run=(...args)=>JSON.parse(execFileSync(cli,args,{env,encoding:'utf8',windowsHide:true}));
const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(fn,label='condition'){const end=Date.now()+30000;while(Date.now()<end){if(await fn())return;await wait(100);}throw Error(`Timed out: ${label}`);}
let browser,page,service,hostStarted=false,hostPid,pickerStarted=false;let imported;
async function seed(){
  await mkdir(path.join(data,'services/reader'),{recursive:true});
  const txt=path.join(profile,'小说.txt');
  await writeFile(txt,`第一章 起点\n${Array.from({length:220},(_,i)=>`第${i}段 春日的风吹过山谷，远方的灯火依次亮起。😀`).join('\n')}\n第二章 归途\n故事仍在继续。`);
  service=spawn(path.join(pack,'bin/reader-service.exe'),[],{env:{...env,LAZYBUDDY_DATA_DIR:path.join(data,'services/reader')},windowsHide:true,stdio:['pipe','pipe','pipe']});
  let next=0;const pending=new Map();const lines=createInterface({input:service.stdout});
  lines.on('line',line=>{const reply=JSON.parse(line);const done=pending.get(reply.id);pending.delete(reply.id);done(reply);});
  const call=async(method,args={})=>{const id=String(++next);const promise=new Promise(resolve=>pending.set(id,resolve));service.stdin.write(JSON.stringify({version:1,id,method,args})+'\n');const result=await promise;assert.equal(result.ok,true,JSON.stringify(result));return result.data;};
  const job=await call('library.import',{path:txt});let status;
  await until(async()=>{status=await call('import.status',{job:job.job});return status.status!=='pending';},'TXT import');assert.equal(status.status,'complete');imported=status.result.book;
  assert.equal((await call('library.import',{path:txt})).status,'pending');
  await call('lifecycle.suspend');
  service.stdin.end();await new Promise(resolve=>service.once('exit',resolve));service=undefined;
  // The reader must continue working after the source file disappears.
  await rm(txt);
  run('modules','install-local',newsPack);
  const sources=run('call','news',JSON.stringify({version:1,id:'sources',method:'sources',args:{}}));assert.equal(sources.ok,true);
  await mkdir(path.join(data,'services/news'),{recursive:true});
  await writeFile(path.join(data,'services/news/state.json'),JSON.stringify({preferences:{auto_refresh:false,refresh_minutes:5,followed:[],hidden:[],order:[],default_followed:false},cache:Object.fromEntries(sources.data.map(s=>[s.id,{id:s.id,ok:true,updated_ms:Date.now(),stories:[{title:'离线样例',url:'https://example.invalid/'}]}]))}));
  await writeFile(path.join(data,'web-preferences.json'),JSON.stringify({root:'reader',auto_update:false}));
}
const invoke=(name,args={})=>page.evaluate(([name,args])=>window.__TAURI_INTERNALS__.invoke(name,args),[name,args]);
try{
  await seed();
  const manifest=JSON.parse(await readFile(path.join(pack,'module.json'),'utf8'));
  const packages=path.join(profile,'packages');await cp(pack,path.join(packages,`reader/${manifest.version}`),{recursive:true});
  const socket=net.createServer();await new Promise(r=>socket.listen(0,'127.0.0.1',r));const port=socket.address().port;await new Promise(r=>socket.close(r));
  env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=`--remote-debugging-port=${port} --disable-background-timer-throttling`;
  execFileSync('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(root,'scripts/build-and-run.ps1'),'-LaunchOnly','-ModulePackages',packages],{env,encoding:'utf8',windowsHide:true});hostStarted=true;
  assert.ok(run('modules','list').some(m=>m.id==='reader'),'BAT must install a newly prepared feature');
  await until(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`)).ok;}catch{return false;}},'CDP');
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  await until(()=>{page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost'));return !!page;},'WebView');
  await page.waitForSelector('[data-events-ready=true]');await invoke('window_action',{action:'show'});
  await page.getByRole('button',{name:`继续阅读《${imported.title}》`}).click();await page.locator('[data-block="b20"]').waitFor();
  await page.locator('.reader-scroll').evaluate(e=>{e.scrollTop=1800;e.dispatchEvent(new Event('scroll'));});
  await wait(900);let saved=JSON.parse(await readFile(path.join(data,'services/reader/library.json'),'utf8')).entries[0].position;assert.ok(Number(saved.block_id.slice(1))>5);
  const generation=(await invoke('snapshot')).view.generation;
  await page.getByRole('button',{name:'设置',exact:true}).click();await page.getByRole('heading',{name:'阅读设置'}).waitFor();
  await invoke('resize',{width:880,height:1000});await wait(150);
  const readingTop=await page.getByRole('heading',{name:'阅读设置'}).evaluate(e=>e.getBoundingClientRect().top);
  await until(async()=>await page.getByLabel('行距',{exact:true}).inputValue()==='1.9','default line height selection');
  await page.getByLabel('行距',{exact:true}).selectOption('2.1');
  await until(async()=>await page.getByLabel('行距',{exact:true}).inputValue()==='2.1','saved line height selection');
  await page.screenshot({path:path.join(evidence,'settings-tall.png')});
  await page.getByRole('button',{name:'显示设置',exact:true}).click();
  const sharedTop=await page.getByRole('heading',{name:'显示设置'}).evaluate(e=>e.getBoundingClientRect().top);
  assert.ok(Math.abs(readingTop-sharedTop)<2,`settings tops differ: ${readingTop} vs ${sharedTop}`);
  await page.getByRole('button',{name:'阅读设置',exact:true}).click();
  await until(async()=>await page.getByLabel('行距',{exact:true}).inputValue()==='2.1','line height survives settings navigation');
  await invoke('resize',{width:780,height:540});
  await page.getByLabel('正文字号').selectOption('24');await wait(200);
  await page.getByRole('button',{name:'返回页面',exact:true}).click();await page.locator('.reader-scroll').waitFor();
  assert.ok(await page.locator('.reader-scroll').evaluate(e=>e.scrollTop>1000),'settings must restore the text anchor');
  await page.getByLabel('章内查找').fill('第150段');await page.getByRole('button',{name:'查找',exact:true}).click();await page.locator('[data-block="b150"] mark').waitFor();
  await page.getByRole('button',{name:'目录',exact:true}).click();await page.getByLabel('章节目录').selectOption('1');await page.getByRole('heading',{name:'第二章 归途'}).waitFor();
  await page.getByLabel('选择功能').selectOption('news');const quiet=(await invoke('snapshot')).reader_requests;await wait(1000);assert.equal((await invoke('snapshot')).reader_requests,quiet,'inactive reader dispatches no requests');
  await page.getByLabel('选择功能').selectOption('reader');await page.getByRole('heading',{name:'第二章 归途'}).waitFor();
  const stale=await invoke('reader_call',{method:'chapter.read',args:{book_id:imported.id,chapter_id:'c0'},generation}).then(()=>'',String);assert.match(stale,/inactive_reader_page/);
  await page.getByLabel('选择功能').evaluate(select=>{select.value='news';select.dispatchEvent(new Event('change',{bubbles:true}));select.value='reader';select.dispatchEvent(new Event('change',{bubbles:true}));});
  await wait(300);assert.equal((await invoke('snapshot')).view.root,'reader');await page.getByRole('heading',{name:'第二章 归途'}).waitFor();
  const current=(await invoke('snapshot')).view.generation;
  const unauthorized=await invoke('reader_call',{method:'library.import',args:{path:path.join(profile,'never-selected.txt')},generation:current}).then(()=>'',String);assert.match(unauthorized,/file_not_selected/);
  await invoke('window_action',{action:'hide'});await wait(400);const hidden=(await invoke('snapshot')).reader_requests;await wait(1000);assert.equal((await invoke('snapshot')).reader_requests,hidden);await invoke('window_action',{action:'show'});
  await page.getByRole('button',{name:'设置',exact:true}).click();await page.getByRole('button',{name:'显示设置',exact:true}).click();await page.getByRole('switch',{name:'深色模式'}).click();await page.getByLabel('背景透明度',{exact:true}).fill('.7');await page.getByLabel('背景透明度',{exact:true}).dispatchEvent('change');
  await page.getByRole('button',{name:'返回页面',exact:true}).click();await invoke('resize',{width:440,height:540});await wait(250);await page.screenshot({path:path.join(evidence,'dark-narrow.png')});
  assert.equal(await page.locator('.reader-page:visible').evaluate(e=>e.scrollWidth>e.clientWidth),false);
  await page.getByRole('button',{name:'设置',exact:true}).click();await page.getByRole('button',{name:'显示设置',exact:true}).click();await page.getByRole('switch',{name:'深色模式'}).click();await page.getByRole('button',{name:'返回页面',exact:true}).click();await invoke('resize',{width:780,height:540});await wait(250);await page.screenshot({path:path.join(evidence,'light.png')});
  await page.reload();await page.waitForSelector('[data-events-ready=true]');await page.getByRole('button',{name:`继续阅读《${imported.title}》`}).click();await page.getByRole('heading',{name:'第二章 归途'}).waitFor();
  await page.getByRole('button',{name:'书库',exact:true}).click();await page.getByRole('button',{name:`移除《${imported.title}》`}).click();await page.getByRole('button',{name:'确认移除',exact:true}).click();await until(async()=>await page.locator('.reader-book').count()===0,'remove');
  const nativeFile=path.join(profile,'原生导入.epub');
  execFileSync('python',['-c',`
import base64,sys,zipfile
with zipfile.ZipFile(sys.argv[1],'w',zipfile.ZIP_DEFLATED) as z:
 z.writestr('mimetype','application/epub+zip')
 z.writestr('META-INF/container.xml','<container><rootfile full-path="book.opf"/></container>')
 z.writestr('book.opf','<package xmlns:dc="http://purl.org/dc/elements/1.1/"><metadata><dc:title>原生导入</dc:title></metadata><manifest><item id="a" href="a.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="a"/></spine></package>')
 z.writestr('a.xhtml','<html><body><h1>第一章 新书</h1><p>通过原生文件选择器导入的<strong>正文</strong>。</p><img src="image.png" alt="本地插图"/></body></html>')
 z.writestr('image.png',base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII='))
`,nativeFile],{windowsHide:true,encoding:'utf8'});
  hostPid=run('host',JSON.stringify({version:1,command:'status',args:{}})).data.pid;
  const chooser=spawn('python',['-B',path.join(root,'apps/desktop/tests/native-reader-dialog.py'),String(hostPid),nativeFile],{windowsHide:true,stdio:['ignore','pipe','pipe']});
  const chooserFinished=new Promise((resolve,reject)=>{chooser.once('error',reject);chooser.once('exit',code=>code===0?resolve():reject(Error(`native chooser failed: ${code}`)));});
  pickerStarted=true;await page.getByRole('button',{name:'导入小说',exact:true}).click();await chooserFinished;
  await page.getByRole('button',{name:'继续阅读《原生导入》'}).waitFor();pickerStarted=false;await rm(nativeFile);await page.getByRole('button',{name:'继续阅读《原生导入》'}).click();await page.getByText('通过原生文件选择器导入的正文。',{exact:true}).waitFor();
  assert.equal(await page.locator('.reader-strong').textContent(),'正文');await page.waitForFunction(()=>document.querySelector('.reader-body img')?.naturalWidth===1);
  const indexPath=path.join(data,'services/reader/library.json');const local=JSON.parse(await readFile(indexPath,'utf8')).entries.find(e=>e.title==='原生导入');
  const position={book_id:local.id,content_version:local.content_version,chapter_id:'c0',block_id:'b1',offset:2};
  await invoke('reader_checkpoint',{position,generation:(await invoke('snapshot')).view.generation});
  const blocker=path.join(data,'services/reader/library.pending');await mkdir(blocker);
  const failedSave=await invoke('reader_call',{method:'progress.flush',args:{},generation:(await invoke('snapshot')).view.generation}).then(()=>'',String);assert.match(failedSave,/save_failed/);
  await invoke('window_action',{action:'quit'});await until(async()=>/未退出/.test(await page.locator('.notice').textContent()),'save failure blocks exit');
  await rm(blocker,{recursive:true});
  await invoke('reader_call',{method:'progress.flush',args:{},generation:(await invoke('snapshot')).view.generation});
  assert.deepEqual(JSON.parse(await readFile(indexPath,'utf8')).entries.find(e=>e.id===local.id).position,position);
  const report={passed:true,real_packaged_service:true,bat_installs_new_module:true,source_file_independent:true,scroll_checkpoint:true,font_change_restores_anchor:true,chapter_search:true,chapter_navigation:true,root_switch_silent:true,rapid_root_switch:true,tall_settings_top_aligned:true,line_height_selection_round_trip:true,stale_generation_rejected:true,unselected_file_rejected:true,hidden_silent:true,reload_restores_progress:true,remove:true,epub_rich_text_and_image:true,save_failure_blocks_exit_and_retries:true,visuals:['light.png','dark-narrow.png'],native_file_dialog_cli_verified:true};
  await writeFile(path.join(evidence,'report.json'),JSON.stringify(report,null,2));console.log(report);
}catch(error){
  if(page){console.error({notice:await page.locator('.notice').textContent().catch(()=>null)});await page.screenshot({path:path.join(evidence,'failure.png')}).catch(()=>{});}
  throw error;
}finally{
  service?.stdin.end();
  if(pickerStarted&&hostPid)execFileSync('python',['-B',path.join(root,'apps/desktop/tests/native-reader-dialog.py'),String(hostPid),'--cancel'],{windowsHide:true});
  if(hostStarted){
    hostPid??=run('host',JSON.stringify({version:1,command:'status',args:{}})).data.pid;
    if(page)await invoke('window_action',{action:'quit'}).catch(()=>{});else try{run('host',JSON.stringify({version:1,command:'app.quit',args:{}}));}catch{}
    await until(()=>{try{process.kill(hostPid,0);return false;}catch{return true;}},'normal host exit');
  }
  await browser?.close().catch(()=>{});
  const resolved=await realpath(profile);assert.ok(resolved.startsWith(root+path.sep+'tmp'+path.sep+'profiles'+path.sep));await rm(resolved,{recursive:true,force:true,maxRetries:8,retryDelay:250});
}
