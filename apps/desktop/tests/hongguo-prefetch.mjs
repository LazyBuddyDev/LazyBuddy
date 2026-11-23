// Local real-media EOS with bounded, simulated service preparation latency. No real account or network.
import assert from 'node:assert/strict';import {createServer} from 'vite';import {chromium} from 'playwright-core';import {readFile,mkdir,mkdtemp,rm,writeFile} from 'node:fs/promises';import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url)),profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url)),output=fileURLToPath(new URL('../../../tmp/verification-hongguo-prefetch/',import.meta.url));await mkdir(profiles,{recursive:true});await mkdir(output,{recursive:true});const scratch=await mkdtemp(profiles+'hongguo-prefetch-');process.env.TEMP=scratch;process.env.TMP=scratch;
const media=await readFile(new URL('../../../tmp/fixtures/media/phases.mp4',import.meta.url));
const server=await createServer({root,server:{host:'127.0.0.1',port:0},plugins:[{name:'prefetch-media',configureServer(server){
 server.middlewares.use('/tests/prefetch-fixture.mp4',(req,res)=>{
  res.setHeader('Content-Type','video/mp4');res.setHeader('Accept-Ranges','bytes');
  const range=/^bytes=(\d+)-(\d*)$/.exec(req.headers.range??'');
  if(!range){res.setHeader('Content-Length',media.length);res.end(media);return;}
  const start=Number(range[1]),end=Math.min(media.length-1,range[2]?Number(range[2]):media.length-1);
  if(start>end){res.writeHead(416,{'Content-Range':`bytes */${media.length}`});res.end();return;}
  res.writeHead(206,{'Content-Range':`bytes ${start}-${end}/${media.length}`,'Content-Length':end-start+1});
  res.end(media.subarray(start,end+1));
 });
}}]});let browser;const results=[];
try{
 await server.listen();browser=await chromium.launch({channel:'msedge',headless:true});
 for(const baseline of [true,false]){
  const page=await browser.newPage();const errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/tests/hongguo-prefetch-fixture.html${baseline?'?baseline':''}`);
  await page.getByRole('button',{name:'收藏',exact:true}).click();await page.getByRole('button',{name:'播放连播样例',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('video')?.currentTime>.05);
  await page.waitForFunction(()=>fixture.calls.some(c=>c.method==='playback.prefetch'));
  assert.equal(await page.evaluate(()=>fixture.calls.filter(c=>c.method==='playback.open').length),1,'prefetch does not switch episodes');
  if(!baseline)await page.waitForFunction(()=>fixture.nextReady());
  await page.evaluate(()=>{
   const v=document.querySelector('video');window.eosAt=0;window.nextFrameAt=0;v.addEventListener('ended',()=>window.eosAt=performance.now(),{once:true});
   const frame=()=>v.requestVideoFrameCallback(()=>{if(v.getAttribute('src')?.includes('episode=2'))window.nextFrameAt=performance.now();else frame();});frame();v.currentTime=v.duration-.15;
  });
  await page.waitForFunction(()=>window.nextFrameAt>window.eosAt&&window.eosAt>0);
  const gap=await page.evaluate(()=>nextFrameAt-eosAt);results.push({baseline,simulated_prepare_ms:600,eos_to_next_frame_ms:Math.round(gap)});
  assert.equal(await page.getByLabel('选集',{exact:true}).inputValue(),'2');
  assert.equal(await page.evaluate(()=>fixture.calls.filter(c=>c.method==='playback.checkpoint'&&c.args.final).length),1,'EOS submits the final checkpoint once');
  await page.waitForFunction(()=>fixture.calls.filter(c=>c.method==='playback.prefetch').length===2);
  await page.evaluate(()=>fixture.visible(false));const before=await page.evaluate(()=>fixture.calls.length);await page.waitForTimeout(800);assert.equal(await page.evaluate(()=>fixture.calls.length),before,'hidden page dispatches no work');
  await page.evaluate(()=>fixture.visible(true));await page.waitForTimeout(150);assert.equal(await page.locator('video').evaluate(v=>v.paused),true);
  const opens=await page.evaluate(()=>fixture.calls.filter(c=>c.method==='playback.open').length);assert.equal(opens,2,'resume does not open or autoplay another episode');assert.deepEqual(errors,[]);await page.close();
 }
 assert.ok(results[1].eos_to_next_frame_ms<results[0].eos_to_next_frame_ms/2,'prepared handoff removes the simulated preparation wait');
 const page=await browser.newPage();const errors=[];page.on('pageerror',e=>errors.push(String(e)));
 await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/tests/hongguo-prefetch-fixture.html`);
 await page.getByRole('button',{name:'收藏',exact:true}).click();
 await page.getByRole('button',{name:'播放连播样例',exact:true}).click();
 const toggle=page.getByRole('button',{name:'弹幕',exact:true});
 const comments=page.locator('.danmaku-overlay');
 await page.waitForFunction(()=>document.querySelector('video')?.currentTime>.05);
 assert.equal(await toggle.getAttribute('aria-pressed'),'false');
 assert.equal(await page.evaluate(()=>fixture.calls.some(c=>c.method.startsWith('danmaku.'))),false);
 await toggle.click();
 const first=comments.getByText('第1集弹幕',{exact:true});await first.waitFor();
 const initial=await first.getAttribute('style');
 await page.waitForFunction(style=>document.querySelector('.danmaku-overlay span')?.getAttribute('style')!==style,initial);
 await toggle.click();await comments.waitFor({state:'detached'});
 assert.equal(await page.locator('video').evaluate(v=>v.paused),false);
 await toggle.click();await first.waitFor();
 await page.getByRole('button',{name:'暂停',exact:true}).click();
 await page.getByRole('button',{name:'播放',exact:true}).waitFor();
 const seek=page.getByRole('slider',{name:'播放进度',exact:true});
 async function seekTo(seconds){
  await seek.evaluate((el,value)=>{el.value=String(value);el.dispatchEvent(new Event('input',{bubbles:true}));},seconds);
  await seek.evaluate(el=>el.dispatchEvent(new Event('change',{bubbles:true})));
  await page.waitForFunction(value=>Math.abs(document.querySelector('video').currentTime-value)<.06,seconds,{timeout:3000});
 }
 await seekTo(.75);
 await comments.getByText('稍后出现',{exact:true}).waitFor();
 await seekTo(.1);
 await comments.getByText('稍后出现',{exact:true}).waitFor({state:'detached'});
 await page.getByLabel('选集',{exact:true}).selectOption('2');
 await first.waitFor({state:'detached'});await comments.getByText('第2集弹幕',{exact:true}).waitFor();
 await page.evaluate(()=>fixture.visible(false));await comments.waitFor({state:'detached'});
 const reads=await page.evaluate(()=>fixture.calls.filter(c=>c.method==='danmaku.read').length);
 await page.waitForTimeout(250);
 assert.equal(await page.evaluate(()=>fixture.calls.filter(c=>c.method==='danmaku.read').length),reads);
 assert.deepEqual(errors,[]);await page.close();
 console.log('Passed: danmaku off by default, scrolls with real media, hides without pausing, follows seeks and episode changes, and stays silent when hidden.');
 await writeFile(output+'timings.json',JSON.stringify({scope:'local real-media fixture, simulated 600ms preparation; not upstream performance',results},null,2));console.log(JSON.stringify(results));
}finally{await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});}
