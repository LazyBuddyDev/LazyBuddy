// Local real-media EOS with bounded, simulated service preparation latency. No real account or network.
import assert from 'node:assert/strict';import {createServer} from 'vite';import {chromium} from 'playwright-core';import {readFile,mkdir,mkdtemp,rm,writeFile} from 'node:fs/promises';import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url)),profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url)),output=fileURLToPath(new URL('../../../tmp/verification-hongguo-prefetch/',import.meta.url));await mkdir(profiles,{recursive:true});await mkdir(output,{recursive:true});const scratch=await mkdtemp(profiles+'hongguo-prefetch-');process.env.TEMP=scratch;process.env.TMP=scratch;
const media=await readFile(new URL('../../../tmp/fixtures/media/phases.mp4',import.meta.url));
const server=await createServer({root,server:{host:'127.0.0.1',port:0},plugins:[{name:'prefetch-media',configureServer(server){server.middlewares.use('/tests/prefetch-fixture.mp4',(_req,res)=>{res.setHeader('Content-Type','video/mp4');res.end(media);});}}]});let browser;const results=[];
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
 await writeFile(output+'timings.json',JSON.stringify({scope:'local real-media fixture, simulated 600ms preparation; not upstream performance',results},null,2));console.log(JSON.stringify(results));
}finally{await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});}
