// Focus appearance and input behavior on real components, with offline services.
import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {chromium} from 'playwright-core';
import {mkdir,mkdtemp,rm,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
const output=fileURLToPath(new URL('../../../tmp/verification-focus-navigation/',import.meta.url));
const profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url));
await mkdir(profiles,{recursive:true});await mkdir(output,{recursive:true});
const scratch=await mkdtemp(profiles+'focus-navigation-');process.env.TEMP=scratch;process.env.TMP=scratch;
const server=await createServer({root,server:{host:'127.0.0.1',port:0,hmr:false}});let browser;const checked=[];
try{
  await server.listen();browser=await chromium.launch({channel:'msedge',headless:true});
  const page=await browser.newPage({viewport:{width:780,height:640},reducedMotion:'reduce'});
  const errors=[];page.on('pageerror',e=>errors.push(String(e)));
  await page.addInitScript(()=>{
    window.fixtureReader=true;window.designSources=[{id:'fixture',name:'样例来源'}];
    window.fixtureNewsTitle='用于检查公共焦点与长标题滚动状态的样例文本。'.repeat(5);
    window.fixtureItems=[{id:'sample',title:'焦点验证短剧',has_cover:false,episodes:1}];
    window.fixtureReaderEntries=[{id:'book',title:'焦点验证书籍',author:'样例',chapters:20,source:{kind:'local',id:'sample',format:'txt',encoding:'utf-8'},content_version:'1',position:null,revision:0,updated_ms:1}];
  });
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/tests/notice-fixture.html`);
  await page.waitForSelector('[data-events-ready=true]');
  const gear=page.locator('.toolbar').getByRole('button',{name:'设置',exact:true});
  const styles=locator=>locator.evaluate(e=>({outline:getComputedStyle(e).outlineStyle,width:getComputedStyle(e).outlineWidth,offset:getComputedStyle(e).outlineOffset}));
  async function mouse(locator,name,key='ArrowRight'){
    if(await locator.evaluate(e=>e.tagName==='VIDEO'))await locator.click({position:{x:10,y:100}});else await locator.click();if(await locator.evaluate(e=>e.tagName==='SELECT'))await page.keyboard.press('Escape');
    await page.keyboard.press(key);
    assert.equal((await styles(locator)).outline,'none',`${name}: pointer operation has no extra outline`);
    assert.equal(await page.locator('.app-shell').getAttribute('data-keyboard-navigation'),'false');
    checked.push(name);
  }
  async function keyboard(locator,name,inset=false){
    await locator.focus();await page.keyboard.press('Tab');await locator.focus();
    assert.equal((await styles(locator)).width,'2px',`${name}: Tab has a visible focus cue`);
    assert.equal((await styles(locator)).offset,inset?'-2px':'3px');
    await page.keyboard.press('Shift+Tab');await locator.focus();
    assert.equal((await styles(locator)).width,'2px',`${name}: Shift+Tab retains focus navigation mode`);
  }
  for(const dark of [false,true]){
    await gear.click();await page.getByRole('button',{name:'显示设置',exact:true}).click();
    const theme=page.getByRole('switch',{name:'深色模式',exact:true});
    if((await theme.getAttribute('aria-checked')==='true')!==dark)await theme.click();
    await page.getByRole('button',{name:'返回页面',exact:true}).click();
    const pin=page.getByRole('button',{name:'置顶',exact:true});await mouse(pin,`pin-${dark}`);await keyboard(pin,'pin');
    const rootSelect=page.getByLabel('选择功能');await mouse(rootSelect,`root-select-${dark}`);await keyboard(rootSelect,'root select');
    await rootSelect.selectOption('news');
    const followed=page.getByRole('button',{name:'已关注',exact:true});await mouse(followed,`tab-${dark}`);await keyboard(followed,'tab');assert.ok(await followed.evaluate(e=>e.classList.contains('active')));
    await page.getByRole('button',{name:'全部来源',exact:true}).click();
    const story=page.locator('.story-title').first();await mouse(story,`story-${dark}`);await page.locator('.toolbar').hover();
    assert.equal(await story.locator('.story-text-content').evaluate(e=>getComputedStyle(e).animationName),'none','mouse-focused titles do not keep a keyboard marquee');
    await keyboard(story,'story');
    const summary=page.locator('.runtime-notice summary');await mouse(summary,`summary-${dark}`);await keyboard(summary,'summary');if(await page.locator('.runtime-notice').getAttribute('open')!==null)await summary.click();
    await page.locator('[aria-label="新闻页签"]').getByRole('button',{name:'设置',exact:true}).click();
    const toggle=page.getByRole('switch',{name:'自动刷新',exact:true});await mouse(toggle,`switch-${dark}`);await keyboard(toggle,'switch');
    const interval=page.getByLabel('刷新间隔（分钟）');await mouse(interval,`setting-select-${dark}`);await keyboard(interval,'setting select');
    const range=page.getByLabel('卡片透明度',{exact:true});await range.click();const value=Number(await range.inputValue());await page.keyboard.press('ArrowRight');assert.ok(Number(await range.inputValue())>value);assert.equal((await styles(range)).outline,'none');checked.push(`range-${dark}`);await keyboard(range,'range');
    await page.screenshot({animations:'disabled',path:output+`settings-${dark?'dark':'light'}.png`});
    await gear.click();await page.getByRole('button',{name:'系统设置',exact:true}).click();
    const shortcut=page.getByRole('button',{name:'显示隐藏快捷键',exact:true});await mouse(shortcut,`shortcut-${dark}`);assert.match(await shortcut.textContent(),/请按快捷键/);
    await page.keyboard.press('Control+Alt+K');await page.waitForFunction(()=>!document.querySelector('[aria-label="显示隐藏快捷键"]').textContent.includes('请按快捷键'));
    await keyboard(shortcut,'shortcut');await page.getByRole('button',{name:'返回页面',exact:true}).click();
    await rootSelect.selectOption('reader');await page.getByRole('button',{name:'书架',exact:true}).click();
    const search=page.getByRole('textbox',{name:'筛选书名',exact:true});await search.click();await search.fill('样例');await page.keyboard.press('ArrowLeft');assert.equal(await search.evaluate(e=>e.selectionStart),1);assert.equal((await styles(search)).outline,'none');
    assert.notEqual(await search.locator('..').evaluate(e=>getComputedStyle(e).borderColor),'rgba(0, 0, 0, 0)');checked.push(`text-input-${dark}`);
    await search.fill('');await page.getByRole('button',{name:'继续阅读《焦点验证书籍》',exact:true}).click();
    const reader=page.getByRole('region',{name:'小说正文'});await reader.click({position:{x:5,y:5}});await page.keyboard.press('ArrowRight');await page.waitForFunction(()=>document.querySelector('.reader-scroll').getAttribute('aria-busy')==='false');
    assert.equal((await styles(reader)).outline,'none');checked.push(`reader-${dark}`);await keyboard(reader,'reader',true);
    await page.screenshot({animations:'disabled',path:output+`reader-${dark?'dark':'light'}.png`});
    await page.getByRole('button',{name:'书库',exact:true}).click();
  }
  await page.getByLabel('选择功能').selectOption('hongguo');
  await page.getByRole('textbox',{name:'搜索短剧'}).fill('样例');await page.getByRole('toolbar',{name:'搜索工具栏'}).getByRole('button',{name:'搜索',exact:true}).click();
  await page.getByRole('button',{name:'播放焦点验证短剧',exact:true}).click();
  const video=page.getByRole('button',{name:'红果短剧视频'});await mouse(video,'video');await keyboard(video,'video',true);
  await page.getByRole('button',{name:'返回列表',exact:true}).click();
  await page.evaluate(()=>window.fixtureEvents['shell-docked']({payload:true}));
  const dock=page.getByRole('button',{name:'展开 LazyBuddy',exact:true});await mouse(dock,'dock');await keyboard(dock,'dock',true);
  assert.deepEqual(errors,[]);await writeFile(output+'report.json',JSON.stringify({passed:true,checked,keyboard_cues:true,editing_and_shortcut_capture:true,media_scope:'offline component; no decoding or account requests'},null,2));
  console.log(`Shared focus navigation passed ${checked.length} pointer/keyboard cases, including editing, shortcut capture, media and dock.`);
}finally{await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});}
