// Offline navigation and preference checks; no accounts or upstream requests.
import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {chromium} from 'playwright-core';
import {mkdir,mkdtemp,rm} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
const profiles=fileURLToPath(new URL('../../../tmp/profiles/',import.meta.url));
const output=fileURLToPath(new URL('../../../tmp/verification-feature-settings/',import.meta.url));
await mkdir(profiles,{recursive:true});const scratch=await mkdtemp(profiles+'feature-settings-');
process.env.TEMP=scratch;process.env.TMP=scratch;
const server=await createServer({root,server:{host:'127.0.0.1',port:0}});let browser;
try{
  await server.listen();browser=await chromium.launch({channel:'msedge',headless:true});
  const page=await browser.newPage({viewport:{width:780,height:640}});
  const errors=[];page.on('pageerror',error=>errors.push(String(error)));
  await page.addInitScript(()=>{window.fixtureReader=true;window.fixturePersist=true;window.designSources=[{id:'fixture',name:'样例来源'}];});
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/tests/notice-fixture.html`);
  await page.waitForSelector('[data-events-ready=true]');
  const gear=page.locator('.toolbar').getByRole('button',{name:'设置',exact:true});
  const localSettings=()=>page.locator('main .tabs:visible').getByRole('button',{name:'设置',exact:true});
  const calls=()=>page.evaluate(()=>window.fixtureCalls.filter(c=>['news_call','reader_call','playback_call'].includes(c.command)).length);
  for(const [id,heading,tabs] of [['news','新闻设置',['全部来源','已关注','设置']],['hongguo','红果账号',['搜索','收藏','设置']],['reader','阅读设置',['书架','设置']]]){
    await page.getByLabel('选择功能').selectOption(id);
    await localSettings().click();await page.getByRole('heading',{name:heading,exact:true}).waitFor();
    assert.equal(await page.locator('main .tabs:visible button.active').textContent(),'设置');
    assert.deepEqual(await page.locator('main .tabs:visible button').allTextContents(),tabs);
    assert.equal(await gear.getAttribute('class').then(s=>s.includes('chosen')),false);
    if(id==='news'){
      await page.getByLabel('卡片透明度',{exact:true}).fill('.35');
      await page.getByLabel('卡片透明度',{exact:true}).dispatchEvent('change');
      await page.waitForFunction(()=>JSON.parse(localStorage.getItem('fixture.preferences')).card_opacity===.35);
      await page.getByRole('button',{name:'全部来源',exact:true}).click();
      const card=await page.locator('.source-card').evaluate(e=>getComputedStyle(e).backgroundColor);
      assert.match(card,/0\.35/);
      await localSettings().click();
    }
    await gear.click();await page.getByRole('heading',{name:'显示设置',exact:true}).waitFor();
    assert.deepEqual(await page.locator('.settings-nav button').allTextContents(),['显示设置','系统设置','关于我们','返回页面']);
    assert.equal(await page.locator('#card-opacity:visible').count(),0);
    const quiet=await calls();await page.waitForTimeout(250);assert.equal(await calls(),quiet,'global settings silence all features');
    // Global settings remain selected while changing roots; hidden/restored settings stay quiet.
    await page.getByLabel('选择功能').selectOption(id==='news'?'reader':'news');
    await page.getByRole('heading',{name:'显示设置',exact:true}).waitFor();
    const switched=await calls();
    await page.evaluate(()=>window.fixtureEvents['shell-visibility']({payload:false}));
    await page.waitForSelector('[data-visible=false]');
    await page.evaluate(()=>window.fixtureEvents['shell-visibility']({payload:true}));
    await page.waitForSelector('[data-visible=true]');
    assert.equal(await calls(),switched);
    await page.getByLabel('选择功能').selectOption(id);
    await page.getByRole('button',{name:'返回页面',exact:true}).click();
  }
  await page.getByLabel('选择功能').selectOption('news');await localSettings().click();
  assert.equal(await page.getByLabel('卡片透明度',{exact:true}).inputValue(),'0.35');
  await page.reload();await page.waitForSelector('[data-events-ready=true]');
  await page.getByLabel('选择功能').selectOption('news');await localSettings().click();
  assert.equal(await page.getByLabel('卡片透明度',{exact:true}).inputValue(),'0.35');
  await mkdir(output,{recursive:true});await page.screenshot({animations:'disabled',path:output+'news-settings.png'});
  await page.setViewportSize({width:440,height:600});
  await gear.click();await page.getByRole('switch',{name:'深色模式'}).click();
  await page.waitForSelector('.app-shell.dark');
  await page.getByRole('button',{name:'返回页面',exact:true}).click();
  await page.getByLabel('选择功能').selectOption('reader');await localSettings().click();
  await page.getByRole('heading',{name:'阅读设置',exact:true}).waitFor();
  assert.equal(await page.locator('main .tabs:visible button.active').textContent(),'设置');
  assert.equal(await page.locator('main').evaluate(e=>e.scrollWidth>e.clientWidth),false);
  await page.screenshot({animations:'disabled',path:output+'reader-settings-dark-narrow.png'});
  await page.addInitScript(()=>window.fixtureSignedOut=true);
  await page.reload();await page.waitForSelector('[data-events-ready=true]');
  await page.getByLabel('选择功能').selectOption('hongguo');
  for(const width of [780,440]){
    await page.setViewportSize({width,height:640});
    for(const tab of ['搜索','收藏']){
      await page.locator('main .tabs:visible').getByRole('button',{name:tab,exact:true}).click();
      const login=page.getByRole('button',{name:'登录',exact:true});await login.waitFor();
      const bounds=await login.evaluate(e=>({button:e.getBoundingClientRect().width,row:e.parentElement.getBoundingClientRect().width}));
      assert.ok(Math.abs(bounds.button-bounds.row)<1,'signed-out login fills its row');
      const top=await page.locator('main .tabs:visible').evaluate(e=>e.getBoundingClientRect().top);
      await localSettings().click();await page.getByRole('heading',{name:'红果账号',exact:true}).waitFor();
      const settingsTop=await page.locator('main .tabs:visible').evaluate(e=>e.getBoundingClientRect().top);
      assert.ok(Math.abs(top-settingsTop)<1,'signed-out tabs align with settings');
      const settingsWidth=await page.getByRole('button',{name:'登录',exact:true}).evaluate(e=>e.getBoundingClientRect().width);
      assert.ok(Math.abs(bounds.button-settingsWidth)<1,'signed-out login matches settings width');
    }
    await page.locator('main .tabs:visible').getByRole('button',{name:'搜索',exact:true}).click();
    await page.getByRole('button',{name:'登录',exact:true}).waitFor();
    await page.screenshot({animations:'disabled',path:output+`signed-out-${width}.png`});
  }
  await page.waitForFunction(()=>document.querySelector('.notice').textContent==='红果账号未登录');
  const accountCalls=()=>page.evaluate(()=>window.fixtureCalls.filter(c=>c.command==='playback_call'&&['account.restore','favorites.read','account.login'].includes(c.method)).map(c=>c.method));
  const beforeTabs=await accountCalls();
  await localSettings().click();await page.getByRole('heading',{name:'红果账号',exact:true}).waitFor();
  await page.locator('main .tabs:visible').getByRole('button',{name:'收藏',exact:true}).click();
  await page.waitForSelector('[data-section=content]');
  assert.equal(await page.getByRole('button',{name:'登录',exact:true}).isEnabled(),true);
  assert.equal(await page.locator('.notice').textContent(),'红果账号未登录');
  await page.locator('main .tabs:visible').getByRole('button',{name:'搜索',exact:true}).click();
  await page.locator('main .tabs:visible').getByRole('button',{name:'收藏',exact:true}).click();
  await page.waitForTimeout(100);
  assert.deepEqual(await accountCalls(),beforeTabs,'signed-out tabs do not restore accounts, fetch favorites or log in');
  await localSettings().click();await page.getByRole('heading',{name:'红果账号',exact:true}).waitFor();
  await page.getByRole('switch',{name:'直连',exact:true}).click();
  await page.waitForFunction(count=>window.fixtureCalls.filter(c=>c.command==='playback_call'&&c.method==='account.restore').length===count,
    beforeTabs.filter(method=>method==='account.restore').length+1);
  await page.waitForFunction(()=>!document.querySelector('[role=switch][aria-label=直连]').disabled);
  await page.getByRole('button',{name:'登录',exact:true}).click();
  assert.equal((await accountCalls()).filter(method=>method==='account.login').length,1,'explicit login still sends one request');
  assert.deepEqual(errors,[]);
  console.log('Passed: feature tabs, global-only gear, card opacity scope/persistence, root switches and hidden settings silence, dark/narrow layout.');
}finally{await browser?.close();await server.close();await rm(scratch,{recursive:true,force:true});}
