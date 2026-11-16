// Compare the real feature components using local fixtures; no upstream data.
import assert from 'node:assert/strict';import {createServer} from 'vite';import {chromium} from 'playwright-core';
import {mkdir,mkdtemp,rm,writeFile} from 'node:fs/promises';import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
const profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url));
const evidence=fileURLToPath(new URL('../../../tmp/verification-tabs/',import.meta.url));
await mkdir(profiles,{recursive:true});await mkdir(evidence,{recursive:true});const scratch=await mkdtemp(profiles+'tab-sizes-');process.env.TEMP=scratch;process.env.TMP=scratch;
const server=await createServer({root,cacheDir:scratch+'/vite',server:{host:'127.0.0.1',port:0}});let browser;const measurements=[];
try{
 await server.listen();browser=await chromium.launch({channel:'msedge',headless:true});const page=await browser.newPage({viewport:{width:780,height:600},deviceScaleFactor:1.5});
 const url=`http://127.0.0.1:${server.httpServer.address().port}/tests/news-fixture.html`;
 async function measure(){return page.locator('.tabs button').evaluateAll(buttons=>buttons.map(b=>{const s=getComputedStyle(b),r=b.getBoundingClientRect();return {width:r.width,height:r.height,font:s.font,padding:s.padding,gap:getComputedStyle(b.parentElement).gap,background:s.backgroundColor,color:s.color};}));}
 for(const width of [440,600,780,1180])for(const dark of [false,true]){
   await page.setViewportSize({width,height:600});await page.goto(url);await page.waitForSelector('.source-card');
   await page.locator('.app-shell').evaluate((el,dark)=>el.classList.toggle('dark',dark),dark);
   const news=[];for(const index of [0,1]){await page.locator('.tabs button').nth(index).click();await page.waitForTimeout(150);news.push(await measure());}
   if(width===440||width===780)await page.locator('.list-toolbar').screenshot({path:evidence+`news-${width}-${dark?'dark':'light'}.png`});
   await page.evaluate(()=>window.showHongguo());await page.waitForSelector('.media-library');
   const media=[];for(const index of [0,1]){await page.locator('.tabs button').nth(index).click();await page.waitForTimeout(150);media.push(await measure());}
   if(width===440||width===780)await page.locator('.list-toolbar').screenshot({path:evidence+`media-${width}-${dark?'dark':'light'}.png`});
   for(const row of [...news,...media])for(const style of row){assert.equal(style.width,66);assert.equal(style.height,29);assert.equal(style.padding,'5px 10px');assert.equal(style.gap,'5px');}
   assert.deepEqual(news,media,'both features share geometry, typography and selected/unselected styles');
   measurements.push({width,dark,tabWidth:66,tabHeight:29,statesCompared:4});
 }
 await writeFile(evidence+'measurements.json',JSON.stringify(measurements,null,2));console.log('PASS: both real feature components use 66×29 tabs with matching fonts, padding, spacing and selection styles at four widths in light/dark.');
}finally{await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});}
