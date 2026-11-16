// Render only the shared dock styles; no real host, accounts or desktop input.
import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {chromium} from 'playwright-core';
import {mkdir,mkdtemp,rm} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
const profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url));
const evidence=fileURLToPath(new URL('../../../tmp/verification-dock-edge/',import.meta.url));
await mkdir(profiles,{recursive:true});await mkdir(evidence,{recursive:true});
const scratch=await mkdtemp(profiles+'dock-edge-');process.env.TEMP=scratch;process.env.TMP=scratch;
const server=await createServer({root,cacheDir:scratch+'/vite',server:{host:'127.0.0.1',port:0}});let browser;
try{
 await server.listen();browser=await chromium.launch({channel:'msedge',headless:true});
 for(const scale of [1,1.25,1.5,2])for(const dark of [false,true]){
   const context=await browser.newContext({viewport:{width:40,height:40},deviceScaleFactor:scale});const page=await context.newPage();
   await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/tests/dock-fixture.html`);
   await page.waitForFunction(()=>document.querySelector('img')?.naturalWidth>0);
   await page.waitForFunction(()=>getComputedStyle(document.querySelector('.dock-button')).borderRadius==='50%');
   await page.locator('.app-shell').evaluate((el,value)=>el.classList.toggle('dark',value),dark);
   const button=page.getByRole('button',{name:'展开 LazyBuddy'});
   const bounds=await button.boundingBox();assert.deepEqual(bounds,{x:1,y:1,width:38,height:38});
   let border;
   for(const state of ['idle','hover','focus']){
     if(state==='hover')await button.hover();
     if(state==='focus'){await button.focus();assert.equal(await button.evaluate(el=>el.matches(':focus-visible')),true);}
     await page.evaluate(()=>new Promise(resolve=>setTimeout(resolve,150)));
     const style=await button.evaluate(el=>({border:getComputedStyle(el).borderTopColor,width:getComputedStyle(el).borderTopWidth}));
     assert.equal(style.width,'1px');border??=style.border;assert.equal(style.border,border,'hover/focus preserve the neutral contour');
     assert.deepEqual(await button.boundingBox(),bounds,'focus never changes size');
     const png=await page.screenshot({omitBackground:true,path:evidence+`${dark?'dark':'light'}-${scale}-${state}.png`});
     const pixels=await page.evaluate(async data=>{
       const img=new Image();img.src='data:image/png;base64,'+data;await img.decode();
       const canvas=document.createElement('canvas');canvas.width=img.width;canvas.height=img.height;const ctx=canvas.getContext('2d');ctx.drawImage(img,0,0);
       const {data:p}=ctx.getImageData(0,0,canvas.width,canvas.height);let outer=0,partial=0;
       for(let y=0;y<canvas.height;y++)for(let x=0;x<canvas.width;x++){
         const alpha=p[(y*canvas.width+x)*4+3];if(x===0||y===0||x===canvas.width-1||y===canvas.height-1)outer+=alpha;
         if(alpha>0&&alpha<255&&Math.hypot(x+.5-canvas.width/2,y+.5-canvas.height/2)>canvas.width*.44)partial++;
       }
       return {outer,partial};
     },png.toString('base64'));
     assert.equal(pixels.outer,0,'visible pixels stay away from the native hard clip');
     assert.ok(pixels.partial>0,'circle retains an antialiased alpha edge');
   }
   await page.setViewportSize({width:780,height:540});
   assert.deepEqual(await button.boundingBox(),bounds,'native resize before the dock event cannot inflate the icon');
   await page.setViewportSize({width:40,height:40});
   assert.deepEqual(await button.boundingBox(),bounds,'collapse restores the same contour');
   await context.close();
 }
 console.log('PASS: neutral stable edges, transparent inset and antialiasing at 100/125/150/200%, light/dark, idle/hover/focus.');
}finally{await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});}
