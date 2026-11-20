// Headless playback controls with a local fixture; no account or upstream requests.
import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {chromium} from 'playwright-core';
import {readFile,mkdir,mkdtemp,rm} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
const profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url));
await mkdir(profiles,{recursive:true});const scratch=await mkdtemp(profiles+'video-click-');
process.env.TEMP=scratch;process.env.TMP=scratch;
const media=await readFile(new URL('../../../tmp/fixtures/media/phases.mp4',import.meta.url));
const server=await createServer({root,server:{host:'127.0.0.1',port:0},plugins:[{name:'local-video',configureServer(server){
  server.middlewares.use('/tests/click-fixture.mp4',(_req,res)=>{res.setHeader('Content-Type','video/mp4');res.end(media);});
}}]});
let browser;
try{
  await server.listen();browser=await chromium.launch({channel:'msedge',headless:true});
  const page=await browser.newPage({viewport:{width:550,height:600}});
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/tests/news-fixture.html`);
  await page.evaluate(async()=>{window.fixtureVideo=true;await window.showHongguo();});
  await page.getByRole('button',{name:'收藏',exact:true}).click();
  await page.getByRole('button',{name:'播放春日',exact:true}).click();
  const video=page.locator('video');
  await page.waitForFunction(()=>{const v=document.querySelector('video');return v&&!v.paused&&v.currentTime>0;});
  await video.click({position:{x:15,y:200}});
  await page.waitForFunction(()=>document.querySelector('video').paused);
  const opens=await page.evaluate(()=>window.hongguoCalls.filter(c=>c.method==='playback.open').length);
  await video.click({position:{x:15,y:200}});
  await page.waitForFunction(()=>!document.querySelector('video').paused);
  await video.click({button:'right',position:{x:15,y:200}});
  assert.equal(await video.evaluate(v=>v.paused),false,'right click must not pause');
  await page.getByRole('button',{name:'暂停',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('video').paused);
  await video.focus();await page.keyboard.press('Enter');
  await page.waitForFunction(()=>!document.querySelector('video').paused);
  assert.equal(await page.evaluate(()=>window.hongguoCalls.filter(c=>c.method==='playback.open').length),opens,'toggle must reuse the current source');
  await page.setViewportSize({width:400,height:600});
  await video.focus();await page.keyboard.press('ArrowDown');
  await page.waitForFunction(()=>Math.abs(document.querySelector('video').volume-.45)<.001);
  await page.keyboard.press('ArrowUp');
  await page.waitForFunction(()=>document.querySelector('video').volume===.5);
  const mute=page.getByRole('button',{name:'静音',exact:true}),slider=page.getByRole('slider',{name:'音量',exact:true});
  await mute.hover();await slider.waitFor({state:'visible'});
  await slider.focus();await page.keyboard.press('End');await page.keyboard.press('ArrowUp');assert.equal(await video.evaluate(v=>v.volume),1);
  const bounds=await slider.boundingBox(),button=await mute.boundingBox();
  assert.ok(bounds.y+bounds.height<button.y,'volume slider must appear above the mute button');
  assert.ok(bounds.x>=0&&bounds.x+bounds.width<=400,'volume slider must fit a narrow window');
  await slider.click({position:{x:10,y:bounds.height*.5}});
  await page.waitForFunction(()=>{const v=document.querySelector('video');return v.volume>.3&&v.volume<.7;});
  assert.equal(await video.evaluate(v=>v.paused),false,'volume adjustment must not pause');
  await mute.click();assert.equal(await video.evaluate(v=>v.muted),true);
  await video.focus();await page.keyboard.press('ArrowDown');
  assert.equal(await video.evaluate(v=>v.muted),true,'volume adjustment must preserve explicit mute');
  await slider.focus();await page.keyboard.press('Home');
  await page.waitForFunction(()=>document.querySelector('video').volume===0);
  await page.keyboard.press('ArrowDown');assert.equal(await video.evaluate(v=>v.volume),0);
  const stored=await page.evaluate(()=>JSON.parse(localStorage.getItem('lazybuddy.hongguo.player')));
  assert.equal(stored.volume,0);assert.equal(stored.mute,true);
  if(process.argv.includes('--capture-layout')){
    const captures=fileURLToPath(new URL('../../../tmp/verification-media-controls/',import.meta.url));await mkdir(captures,{recursive:true});
    for(const width of [780,400]){
      await page.setViewportSize({width,height:600});await page.evaluate(dark=>document.querySelector('.app-shell').classList.toggle('dark',dark),width===780);
      await page.getByRole('button',{name:'返回列表',exact:true}).focus();
      assert.equal(await page.locator('.video-controls').evaluate(el=>{const stage=el.closest('.video-stage').getBoundingClientRect();return [...el.children].some(child=>{const r=child.getBoundingClientRect();return r.left<stage.left||r.right>stage.right;});}),false);
      await page.screenshot({path:`${captures}/hongguo-${width}.png`});
    }
  }
  console.log('Video clicks and volume popup, slider, arrow keys, limits and mute persistence passed.');
}finally{
  await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});
}
