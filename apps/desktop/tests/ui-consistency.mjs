// Shared layout checks against the real components with offline data.
import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {chromium} from 'playwright-core';
import {mkdir,mkdtemp,rm,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
const profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url));
const capture=process.argv.includes('--capture-only');
const output=fileURLToPath(new URL(`../../../tmp/verification-ui-consistency/${capture?'before':'after'}/`,import.meta.url));
await mkdir(profiles,{recursive:true});await mkdir(output,{recursive:true});
const scratch=await mkdtemp(profiles+'ui-consistency-');process.env.TEMP=scratch;process.env.TMP=scratch;
const server=await createServer({root,server:{host:'127.0.0.1',port:0}});let browser;const measurements=[];
try{
  await server.listen();browser=await chromium.launch({channel:'msedge',headless:true});
  const page=await browser.newPage();const errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await page.addInitScript(()=>{
    window.fixtureReader=true;window.designSources=[{id:'fixture',name:'样例来源'}];
    window.fixtureItems=[{id:'sample',title:'样例剧集：长名称在窄窗口中的布局检查',episodes:12,has_cover:false}];
    window.fixtureReaderEntries=[{id:'book',title:'样例小说：长书名在窄窗口中的布局检查',author:'样例作者',chapters:20,source:{kind:'local',id:'sample',format:'txt',encoding:'utf-8'},content_version:'1',position:null,revision:0,updated_ms:1}];
  });
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/tests/notice-fixture.html`);
  await page.waitForSelector('[data-events-ready=true]');
  for(const width of [780,440])for(const dark of [false,true]){
    await page.setViewportSize({width,height:640});
    await page.locator('.toolbar').getByRole('button',{name:'设置',exact:true}).click();
    await page.getByRole('button',{name:'显示设置',exact:true}).click();
    const toggle=page.getByRole('switch',{name:'深色模式',exact:true});
    if((await toggle.getAttribute('aria-checked')==='true')!==dark)await toggle.click();
    await page.getByRole('button',{name:'返回页面',exact:true}).click();
    for(const id of ['news','hongguo','reader'])for(const settings of [false,true]){
      await page.getByLabel('选择功能').selectOption(id);
      if(settings)await page.locator('main .tabs:visible').getByRole('button',{name:'设置',exact:true}).click();
      else await page.locator('main .tabs:visible button').first().click();
      await page.waitForSelector(`[data-section=${settings?'settings':'content'}]`);
      if(id==='reader'&&!settings)await page.getByRole('button',{name:'继续阅读《样例小说：长书名在窄窗口中的布局检查》'}).waitFor();
      if(id==='hongguo'&&!settings){await page.getByRole('textbox',{name:'搜索短剧'}).fill('样例');await page.getByRole('toolbar',{name:'搜索工具栏'}).getByRole('button',{name:'搜索',exact:true}).click();await page.getByRole('heading',{name:'样例剧集：长名称在窄窗口中的布局检查'}).waitFor();}
      const name=`${width}-${dark?'dark':'light'}-${id}-${settings?'settings':'content'}`;
      const metrics=await page.locator('main').evaluate(main=>{
        const visible=e=>e.getClientRects().length>0;
        const read=e=>{const s=getComputedStyle(e),r=e.getBoundingClientRect();return {text:e.textContent?.trim(),height:r.height,font:s.fontSize,display:s.display,padding:s.padding,margin:s.margin,border:s.borderColor};};
        return {overflow:main.scrollWidth>main.clientWidth,collections:[...main.querySelectorAll('.collection-row')].filter(visible).map(read),rows:[...main.querySelectorAll('.setting-row')].filter(visible).map(read),labels:[...main.querySelectorAll('.setting-row > label,.setting-row > strong')].filter(visible).map(read),selects:[...main.querySelectorAll('.setting-row select')].filter(visible).map(read),toolbars:[...main.querySelectorAll('.collection-toolbar button')].filter(visible).map(read),actions:[...main.querySelectorAll('.row-actions button,.reader-book > button[title^="移除"]')].filter(visible).map(read)};
      });measurements.push({name,...metrics});
      if(!capture){assert.equal(metrics.overflow,false,`${name}: no horizontal overflow`);for(const row of metrics.collections)assert.equal(row.display,'flex',`${name}: common collection layout`);for(const row of metrics.rows)assert.equal(row.height,56,`${name}: common setting row height`);for(const control of metrics.toolbars)assert.equal(control.height,33,`${name}: common toolbar height`);for(const label of metrics.labels)assert.equal(label.font,'12px',`${name}: common label size`);for(const select of metrics.selects)assert.equal(select.height,30,`${name}: common dropdown height`);for(const action of metrics.actions)assert.equal(action.height,29,`${name}: common row actions`);}
      if(!capture){assert.equal(await page.locator('main .tabs:visible button[aria-pressed=true]').count(),1);for(const button of await page.locator('main .tabs:visible button').all())assert.ok(await button.getAttribute('title'));}
      await page.screenshot({animations:'disabled',path:output+name+'.png'});
    }
  }
  await page.locator('main .tabs:visible button').first().click();
  await page.getByRole('button',{name:'继续阅读《样例小说：长书名在窄窗口中的布局检查》'}).click();
  await page.getByRole('heading',{name:'第1章 布局检查',exact:true}).waitFor();
  await page.getByRole('button',{name:'章内查找',exact:true}).click();
  await page.getByRole('button',{name:'目录',exact:true}).click();
  assert.equal(await page.locator('main').evaluate(e=>e.scrollWidth>e.clientWidth),false,'reader tools and directory fit the narrow window');
  if(!capture)for(const h of await page.locator('.collection-toolbar:visible button').evaluateAll(nodes=>nodes.map(e=>e.getBoundingClientRect().height)))assert.equal(h,33);
  await page.screenshot({animations:'disabled',path:output+'reader-reading.png'});
  assert.deepEqual(errors,[]);await writeFile(output+'measurements.json',JSON.stringify(measurements,null,2));
  console.log(`${capture?'Captured':'Verified'} 24 feature/settings screens in light/dark at 780/440px.`);
}finally{await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});}
