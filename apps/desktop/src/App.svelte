<script lang="ts">
  import {onMount,tick} from 'svelte';import {invoke,Channel} from '@tauri-apps/api/core';import {listen} from '@tauri-apps/api/event';
  import {getCurrentWindow} from '@tauri-apps/api/window';
  import ShortcutButton from './ShortcutButton.svelte';import Icon from './Icon.svelte';import Switch from './Switch.svelte';import Hongguo from './Hongguo.svelte';import News from './News.svelte';
  import {moduleTarget,type Root,type Section,type View} from './lifecycle';import logoUrl from '../../../assets/lazybuddy.svg';
  import Reader from './Reader.svelte';import Bilibili from './Bilibili.svelte';import {focusNavigation} from './focus-navigation';
  import AttentionDot from './AttentionDot.svelte';
  import {AttentionCenter,attentionTarget,unreadAttention,acknowledgeAttention,moduleAttention,updateAttention,type AttentionItem,type AttentionReceipts} from './attention';
  type Module={id:string;name:string;compatible?:boolean;version:string;available_version?:string;installed:boolean;update:boolean};
  type ModuleProgress={stage:string;downloaded:number;total:number};
  type Preferences={dark:boolean;topmost:boolean;root:string|null;auto_update:boolean;background_opacity:number;card_opacity:number;shortcut:{modifiers:number;key:number};attention_receipts?:AttentionReceipts;[key:string]:unknown};
  let root=$state<Root>('news');let section=$state<Section>('content');let category=$state('appearance');let globalCategory='appearance';let visible=$state(false);let docked=$state(false);
  let view=$state<View>({root:'news',section:'content',visible:false,generation:0});let eventsReady=$state(false);let ready=$state(false);
  let preferences=$state<Preferences>();let dark=$derived(preferences?.dark??false);let pinned=$state(false);let startup=$state(false);let isolated=$state(false);
  let modules=$state<Module[]>([]);let roots=$state<{id:Root;name:string}[]>([]);let managerBusy=$state(false);let catalogReady=$state(false);
  let installing=$state<string>();let moduleProgress=$state<ModuleProgress>();
  let servicePlayback=$state(false);let realHongguo=$state<Hongguo>();let serviceActive=$state(false);
  let reader=$state<Reader>();let bilibili=$state<Bilibili>();
  let changing=$state(false);let pinSaving=false;let preferencesReadOnly=$state(false);
  let notice=$state('正在启动');let updateReady=$state(false);let appUpdates=$state(false);let page:HTMLElement;let sequence=0;let lastAccountPage=false;
  let displayedCategory='appearance';
  let attentionItems=$state<AttentionItem[]>([]);
  const attention=new AttentionCenter(items=>attentionItems=items);const moduleSource=attention.register('modules');const updateSource=attention.register('host.update');
  let receipts=$state.raw<AttentionReceipts>({});let acknowledgement=0;let catalogPending=false;let updateSequence=0;
  const unread=(target:string)=>unreadAttention(attentionItems,receipts,target);
  const attentionOptions=(target:string)=>({ack:()=>void acknowledge(target),enabled:ready&&visible&&!docked});
  async function acknowledge(target:string){
    const items=unread(target);if(!items.length)return;
    const sequence=++acknowledgement;receipts=acknowledgeAttention(receipts,items);
    if(!await save({attention_receipts:receipts})&&sequence===acknowledgement)receipts=preferences?.attention_receipts??{};
  }
  function publishModules(){moduleSource.publish(moduleAttention(modules));}
  async function syncUpdate(){
    if(!appUpdates)return;
    const sequence=++updateSequence;
    try{const status=await call<{ready:boolean;build:string|null}>('update_status');if(sequence!==updateSequence)return;updateReady=status.ready;updateSource.publish(updateAttention(status.ready?status.build:null));}catch(error){notice=`读取更新状态失败：${error}`;}
  }
  function catalogChanged(){catalogPending=true;if(!managerBusy){catalogPending=false;void moduleAction('list');}}

  let dockPress:{id:number;x:number;y:number}|undefined;let dockDragged=false;
  function pressDock(event:PointerEvent){if(event.button!==0||!event.isPrimary)return;dockDragged=false;dockPress={id:event.pointerId,x:event.clientX,y:event.clientY};(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);}
  function dragDock(event:PointerEvent){
    if(!dockPress||event.pointerId!==dockPress.id||Math.hypot(event.clientX-dockPress.x,event.clientY-dockPress.y)<4)return;
    dockPress=undefined;dockDragged=true;(event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
    void getCurrentWindow().startDragging().catch(error=>notice=`拖动失败：${error}`);
  }
  function releaseDock(){dockPress=undefined;}
  function clickDock(event:MouseEvent){if(event.detail===0||!dockDragged)void action('expand');}
  async function renderDock(value:boolean){
    const began=performance.now();docked=value;await tick();
    const size=(element:Element|null)=>{const r=element?.getBoundingClientRect();return [Math.round(r?.width??0),Math.round(r?.height??0)];};
    void call('dock_rendered',{report:{docked:value,visible:!document.hidden,viewport:[innerWidth,innerHeight],button:size(document.querySelector('.dock-button')),icon:size(document.querySelector('.dock-button img')),delay_ms:Math.min(60000,Math.round(performance.now()-began))}}).catch(()=>{});
  }
  const scrollPositions=new Map<string,number>();const scrollKey=()=>`${root}:${section}:${section==='settings'?displayedCategory:''}`;
  const call=<T,>(name:string,args:Record<string,unknown>={})=>invoke<T>(name,args);
  let saving:Promise<unknown>=Promise.resolve();
  function save(patch:Partial<Preferences>){const operation=saving.then(async()=>{if(!preferences)return false;try{const result=await call<{preferences:Preferences}>('settings',{value:{...preferences,...patch}});preferences=result.preferences;return true;}catch(error){notice=`设置保存失败：${error}`;return false;}});saving=operation;return operation;}
  function pageNotice(owner:Root,text:string){if(root===owner&&(section==='content'||section==='settings'&&category==='page'))notice=text;}
  async function pauseCurrent(nextRoot:Root,nextSection:Section,capture:boolean){
    const content=section==='content'||section==='settings'&&displayedCategory==='page';
    if(!content)return;
    if(root==='reader')await reader?.suspend(capture);
    if(root==='bilibili')bilibili?.suspend(nextRoot!=='bilibili'||nextSection!=='content');
    if(root==='hongguo')await realHongguo?.suspend(nextRoot==='hongguo'&&nextSection==='content');
  }
  async function change(nextRoot=root,nextSection=section,nextVisible=visible){
    const accountPage=nextSection==='settings'&&category==='page';
    if(!changing&&view.generation>0&&nextRoot===root&&nextSection===section&&nextVisible===visible&&accountPage===lastAccountPage&&category===displayedCategory)return;
    if(section==='settings')void call('capture_shortcut',{enabled:false}).catch(error=>notice=String(error));
    const ticket=++sequence;changing=true;serviceActive=false;
    try{await pauseCurrent(nextRoot,nextSection,nextVisible);}catch(error){if(ticket===sequence){notice=`阅读进度保存失败：${error}`;changing=false;}return;}
    if(ticket!==sequence)return;
    if(page&&visible&&!docked)scrollPositions.set(scrollKey(),page.scrollTop);
    const sameFeature=root===nextRoot&&(section==='content'||section==='settings'&&displayedCategory==='page')&&(nextSection==='content'||accountPage);
    if(!sameFeature&&(root!==nextRoot||section!==nextSection||displayedCategory!==category))notice='就绪';
    root=nextRoot;section=nextSection;visible=nextVisible;lastAccountPage=accountPage;displayedCategory=category;
    try{const lease=await call<View>('transition',{root,section,visible,accountPage});if(ticket!==sequence)return;view=lease;
      serviceActive=root==='hongguo'&&visible&&(section==='content'||accountPage&&section==='settings');
      await tick();if(ticket===sequence){if(page)page.scrollTop=scrollPositions.get(scrollKey())??0;reader?.resume();}
      if(preferences?.root!==root)void save({root});
      if(section==='modules'&&visible&&!catalogReady)void moduleAction('catalog');
    }catch(error){if(ticket===sequence)notice=String(error);}finally{if(ticket===sequence)changing=false;}
  }
  async function moduleAction(action:string,id?:string){
    if(managerBusy)return;managerBusy=true;if(action!=='list')notice=action==='catalog'?'正在读取功能目录':action==='uninstall'?'正在卸载':'正在下载安装';
    let finished=false;const onProgress=action==='install'?new Channel<ModuleProgress>():undefined;
    if(onProgress){installing=id;moduleProgress=undefined;onProgress.onmessage=value=>{if(!finished)moduleProgress=value;};}
    try{const data=await call<{entries:Module[];installed:{id:Root;name:string}[];catalog_ready:boolean}>('module_call',{action,id,onProgress});modules=data.entries;roots=data.installed;catalogReady=data.catalog_ready;publishModules();
      servicePlayback=(await call<{media_route:string}>('snapshot')).media_route==='browser_service';
      if(ready&&!roots.some(m=>m.id===root)&&roots.length)await change(roots[0].id,section);
      if(action!=='list')notice='功能列表已更新';
    }catch(error){notice=`功能操作失败：${error}`;}finally{finished=true;installing=undefined;moduleProgress=undefined;managerBusy=false;if(catalogPending)catalogChanged();}
  }
  function featureSettings(settings:boolean){category='page';return change(root,settings?'settings':'content');}
  function globalSettings(){if(section==='settings'&&category!=='page')return change(root,'content');category=globalCategory;return change(root,'settings');}
  async function toggleModules(){await change(root,moduleTarget(section));if(section==='modules'){await moduleAction('list');if(!catalogReady)void moduleAction('catalog');}}
  async function action(action:string){try{if(action!=='show')await pauseCurrent(root,action==='quit'?'modules':section,true);await call('window_action',{action});}catch(error){notice=String(error);}}
  async function togglePin(){if(pinSaving)return;pinSaving=true;try{
    const value=await call<boolean>('toggle_pin');pinned=value;
    if(!await save({topmost:value}))pinned=await call<boolean>('toggle_pin');
  }catch(error){notice=String(error);}finally{pinSaving=false;}}
  async function checkUpdate(){notice='正在检查应用更新';try{updateReady=await call<boolean>('check_update');notice=updateReady?'应用更新已就绪':'当前已是最新构建';await syncUpdate();}catch(error){notice=`更新检查失败：${error}`;}}
  function mediaKey(event:KeyboardEvent){if(event.target instanceof HTMLInputElement||event.target instanceof HTMLSelectElement||event.target instanceof HTMLButtonElement||!serviceActive||section!=='content')return;
    if(event.code==='Space'){event.preventDefault();void realHongguo?.toggle();}else if(['ArrowRight','ArrowLeft'].includes(event.code)){event.preventDefault();realHongguo?.seekRelative(event.code==='ArrowRight'?5:-5);}}
  function shortcutLabel(){const s=preferences?.shortcut;if(!s)return '';return `${s.modifiers&2?'Ctrl+':''}${s.modifiers&1?'Alt+':''}${s.modifiers&4?'Shift+':''}${s.key>=112?`F${s.key-111}`:String.fromCharCode(s.key)}`;}
  onMount(()=>{const subscriptions:(()=>void)[]=[];let disposed=false;
    async function subscribe<T>(event:string,handler:(value:T)=>void){const off=await listen<T>(event,e=>handler(e.payload));if(disposed)off();else subscriptions.push(off);}
    void(async()=>{try{
      await Promise.all([subscribe<boolean>('shell-catalog',()=>catalogChanged()),subscribe<boolean>('shell-visibility',value=>void change(root,section,value)),subscribe<boolean>('shell-docked',value=>void renderDock(value)),subscribe<string>('shell-error',value=>notice=value),subscribe<boolean>('shell-update',value=>{if(appUpdates){updateReady=value;void syncUpdate();if(value)notice='应用更新已就绪';}})]);
      const saved=await call<{preferences:Preferences;startup:boolean;isolated:boolean;app_updates:boolean;preferences_read_only?:boolean}>('settings');preferences=saved.preferences;receipts=preferences.attention_receipts??{};startup=saved.startup;isolated=saved.isolated;appUpdates=saved.app_updates;preferencesReadOnly=saved.preferences_read_only===true;pinned=preferences.topmost;
      await moduleAction('list');root=roots.find(m=>m.id===preferences?.root)?.id??roots[0]?.id??'news';const status=await call<{view:View}>('snapshot');visible=status.view.visible;
      ready=true;eventsReady=true;await change(root,roots.length?'content':'modules',visible);notice=roots.length?'就绪':'从功能页安装需要的功能';void syncUpdate();
    }catch(error){notice=`启动失败：${error}`;}})();
    const visibility=()=>{if(document.hidden)realHongguo?.pause();};document.addEventListener('visibilitychange',visibility);
    return()=>{disposed=true;subscriptions.forEach(off=>off());document.removeEventListener('visibilitychange',visibility);};
  });
</script>
<svelte:window onkeydown={mediaKey}/>
{#snippet settingsNavigation()}<aside class="settings-nav" aria-label="设置分类">{#each [['appearance','显示设置'],['system','系统设置'],['about','关于我们']] as [id,label]}<button class:chosen={category===id} aria-label={label} use:attentionTarget={attentionOptions(`settings.${id}`)} onclick={()=>{globalCategory=category=id;void change();}}>{label}<AttentionDot inline active={unread(`settings.${id}`).length>0} label="有可用更新，悬停清除提醒"/></button>{/each}<button class="back-button" onclick={()=>change(root,'content')}><Icon name="arrow" size={16}/>返回页面</button></aside>{/snippet}
<div class:dark class:docked class="app-shell" use:focusNavigation style:--background-opacity={preferences?.background_opacity??.94} data-testid="shell" data-root={root} data-section={section} data-visible={visible} data-docked={docked} data-events-ready={eventsReady}>
{#if docked}<button class="dock-button focus-inset" aria-label="展开 LazyBuddy" title="点击展开，拖动调整停靠位置" onpointerdown={pressDock} onpointermove={dragDock} onpointerup={releaseDock} onpointercancel={releaseDock} onlostpointercapture={releaseDock} onclick={clickDock}><img src={logoUrl} alt="" draggable="false"/></button>{/if}
<header class="toolbar" data-tauri-drag-region><div class="identity" data-tauri-drag-region><img class="brand-logo" src={logoUrl} alt="" draggable="false" data-tauri-drag-region/><strong data-tauri-drag-region>LazyBuddy</strong><span class="divider" data-tauri-drag-region></span>
<label class="root-selector"><select aria-label="选择功能" value={root} disabled={!roots.length} onchange={e=>change(e.currentTarget.value as Root)}>{#each roots as module}<option value={module.id}>{module.name}</option>{/each}</select><Icon name="chevron" size={14}/></label></div><div class="toolbar-spacer" data-tauri-drag-region></div>
<nav aria-label="全局操作" data-tauri-drag-region><button class="icon-button" class:chosen={pinned} aria-label="置顶" title="置顶" aria-pressed={pinned} onclick={togglePin}><Icon name="pin" size={21}/></button><button class="icon-button attention-anchor" use:attentionTarget={attentionOptions('modules')} class:chosen={section==='modules'} aria-label="功能" aria-describedby={unread('modules').length?'attention-modules':undefined} title="功能" onclick={toggleModules}><Icon name="box" size={21}/><AttentionDot id="attention-modules" active={unread('modules').length>0} label="有新功能或模块更新，悬停清除提醒"/></button><button class="icon-button attention-anchor" use:attentionTarget={attentionOptions('settings')} class:chosen={section==='settings'&&category!=='page'} aria-label="设置" aria-describedby={unread('settings').length?'attention-settings':undefined} title="全局设置" onclick={globalSettings}><Icon name="settings" size={21}/><AttentionDot id="attention-settings" active={unread('settings').length>0} label="应用更新已就绪，悬停清除提醒"/></button><span class="divider" data-tauri-drag-region></span><button class="icon-button close-button" aria-label="关闭到托盘" title="关闭到托盘" onclick={()=>action('close')}><Icon name="close"/></button></nav></header>
<main class="page" bind:this={page}>
{#if ready}
{#if roots.some(m=>m.id==='bilibili')}<Bilibili bind:this={bilibili} active={!changing&&root==='bilibili'&&visible&&!docked&&(section==='content'||section==='settings'&&category==='page')} showing={root==='bilibili'&&(section==='content'||section==='settings'&&category==='page')} settings={section==='settings'} generation={view.generation} notice={text=>pageNotice('bilibili',text)} settingsChange={featureSettings}/>{/if}
{#if roots.some(m=>m.id==='reader')}<Reader bind:this={reader} active={!changing&&root==='reader'&&visible&&!docked&&(section==='content'||section==='settings'&&category==='page')} showing={root==='reader'&&(section==='content'||section==='settings'&&category==='page')} settings={section==='settings'} generation={view.generation} notice={text=>pageNotice('reader',text)} settingsChange={featureSettings}/>{/if}
{#if servicePlayback}<Hongguo bind:this={realHongguo} active={serviceActive&&!docked} showing={root==='hongguo'&&(section==='content'||section==='settings'&&category==='page')} accountOnly={section==='settings'} generation={view.generation} notice={text=>pageNotice('hongguo',text)} settingsChange={featureSettings}/>{/if}
{#if roots.some(m=>m.id==='news')}<div hidden={root!=='news'||!(section==='content'||section==='settings'&&category==='page')}><News active={!changing&&root==='news'&&visible&&!docked&&(section==='content'||section==='settings'&&category==='page')} settings={section==='settings'} generation={view.generation} notice={text=>pageNotice('news',text)} settingsChange={featureSettings} cardOpacity={preferences?.card_opacity??.94} setCardOpacity={value=>save({card_opacity:value})}/></div>{/if}
{#if section==='modules'}<section class="content-width module-page"><div class="module-heading"><h1>可用功能</h1><button onclick={toggleModules}><Icon name="arrow" size={16}/>返回</button></div><div class="module-list">{#each modules as module}<div class="module-row" use:attentionTarget={attentionOptions(`module:${module.id}`)}><div class="module-name"><strong>{module.name}</strong><span class="version">{module.version}{module.compatible===false?' · 当前宿主无法打开':''}</span><AttentionDot inline active={unread(`module:${module.id}`).length>0} label="有新功能或模块更新，悬停清除提醒"/></div><div class="module-progress-slot">{#if installing===module.id}{@const downloading=moduleProgress?.stage==='download'&&moduleProgress.total>0}{@const percent=downloading?Math.floor(moduleProgress!.downloaded/moduleProgress!.total*100):undefined}{@const label=downloading?`正在下载 ${percent}%`:moduleProgress?.stage==='verify'?'正在校验并解包':moduleProgress?.stage==='install'?'正在安装':'正在准备'}<div class="module-progress" title={`${module.name}：${label}`}>{#if downloading}<progress max="100" value={percent} aria-label={`${module.name}安装进度`} aria-valuetext={label}></progress>{:else}<progress aria-label={`${module.name}安装进度`} aria-valuetext={label}></progress>{/if}<span>{label}</span></div>{/if}</div><div class="row-actions">{#if module.installed}<button disabled={managerBusy||!module.update} onclick={()=>moduleAction('install',module.id)}>更新</button><button class="subtle-danger" disabled={managerBusy} onclick={()=>moduleAction('uninstall',module.id)}>卸载</button>{:else}<button disabled={managerBusy||!catalogReady} onclick={()=>moduleAction('install',module.id)}>安装</button>{/if}</div></div>{/each}</div></section>
{:else if section==='settings'&&category!=='page'}<section class="settings-layout content-width">{@render settingsNavigation()}<div class="settings-content"><h1>{category==='appearance'?'显示设置':category==='system'?'系统设置':'关于我们'}</h1>
{#if category==='appearance'}<div class="setting-row"><strong>深色模式</strong><Switch label="深色模式" checked={dark} onchange={value=>save({dark:value})}/></div><div class="setting-row"><label for="background-opacity">背景透明度</label><input id="background-opacity" aria-label="背景透明度" type="range" min="0" max="1" step=".05" value={preferences?.background_opacity??.94} onchange={event=>save({background_opacity:Number(event.currentTarget.value)})}/></div>
{:else if category==='system'}<div class="setting-row"><strong>开机启动</strong><Switch label="开机启动" checked={startup} disabled={isolated} onchange={value=>call('set_startup',{enabled:value}).then(()=>startup=value).catch(error=>notice=String(error))}/></div><div class="setting-row"><span>显示 / 隐藏快捷键</span><ShortcutButton active={visible&&!docked} label={shortcutLabel()} save={value=>save({shortcut:value})} notice={text=>notice=text}/></div>{#if appUpdates}<div class="setting-row"><strong>应用自动更新</strong><Switch label="应用自动更新" checked={preferences?.auto_update??true} onchange={value=>save({auto_update:value})}/></div>{/if}<div class="setting-row">{#if appUpdates}<button onclick={checkUpdate}>检查更新</button>{/if}<button onclick={()=>call('open_logs').catch(error=>notice=String(error))}>打开日志</button></div>
{:else}<h2>LazyBuddy</h2><p>1.0.3</p><p>Copyright © 2026 Leager · MIT</p>{/if}</div></section>
{:else if section==='content'&&!roots.length}<div class="list-empty"><button onclick={toggleModules}>安装功能</button></div>
{:else if section==='content'&&root==='hongguo'&&!servicePlayback}<div class="list-empty">请在功能页更新红果短剧模块</div>{/if}
{/if}
</main>
<footer class="info-bar" aria-live="polite"><span class="status-dot"></span><details class="runtime-notice"><summary class="notice" title="展开运行信息">{preferencesReadOnly?'设置文件保留，当前为只读模式':notice.split('\n')[0]}</summary><pre>{preferencesReadOnly?'设置文件无法读取，使用临时默认值。修复文件并重启后可保存。\n':''}{notice}</pre></details>{#if appUpdates&&updateReady}<button class="attention-anchor" aria-label="重启更新" use:attentionTarget={attentionOptions('app.update')} onclick={()=>call('apply_update').catch(error=>notice=String(error))}>重启更新<AttentionDot active={unread('app.update').length>0} label="应用更新已就绪，悬停清除提醒"/></button>{/if}<button class="log-button" onclick={()=>call('open_logs').catch(error=>notice=String(error))}>日志</button></footer>
</div>
