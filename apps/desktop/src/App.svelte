<script lang="ts">
  import {onMount,tick} from 'svelte';import {invoke} from '@tauri-apps/api/core';import {listen} from '@tauri-apps/api/event';
  import ShortcutButton from './ShortcutButton.svelte';import Icon from './Icon.svelte';import Switch from './Switch.svelte';import Hongguo from './Hongguo.svelte';import News from './News.svelte';
  import {toggleSection,moduleTarget,type Root,type Section,type View} from './lifecycle';import logoUrl from '../../../assets/lazybuddy.svg';
  type Module={id:string;name:string;version:string;installed:boolean;update:boolean};
  type Preferences={dark:boolean;topmost:boolean;root:string|null;auto_update:boolean;background_opacity:number;card_opacity:number;shortcut:{modifiers:number;key:number};[key:string]:unknown};
  let root=$state<Root>('news');let section=$state<Section>('content');let category=$state('page');let visible=$state(false);let docked=$state(false);
  let view=$state<View>({root:'news',section:'content',visible:false,generation:0});let eventsReady=$state(false);let ready=$state(false);
  let preferences=$state<Preferences>();let dark=$derived(preferences?.dark??false);let pinned=$state(false);let startup=$state(false);let isolated=$state(false);
  let modules=$state<Module[]>([]);let roots=$state<{id:Root;name:string}[]>([]);let managerBusy=$state(false);let catalogReady=$state(false);
  let servicePlayback=$state(false);let realHongguo=$state<Hongguo>();let serviceActive=$state(false);
  let notice=$state('正在启动');let updateReady=$state(false);let page:HTMLElement;let sequence=0;let lastAccountPage=false;
  let displayedCategory='page';
  const scrollPositions=new Map<string,number>();const scrollKey=()=>`${root}:${section}:${section==='settings'?displayedCategory:''}`;
  const call=<T,>(name:string,args:Record<string,unknown>={})=>invoke<T>(name,args);
  let saving:Promise<unknown>=Promise.resolve();
  function save(patch:Partial<Preferences>){const operation=saving.then(async()=>{if(!preferences)return false;try{const result=await call<{preferences:Preferences}>('settings',{value:{...preferences,...patch}});preferences=result.preferences;return true;}catch(error){notice=`设置保存失败：${error}`;return false;}});saving=operation;return operation;}
  function pageNotice(owner:Root,text:string){if(root===owner&&(section==='content'||section==='settings'&&category==='page'))notice=text;}
  async function change(nextRoot=root,nextSection=section,nextVisible=visible){
    const accountPage=nextSection==='settings'&&category==='page';
    if(view.generation>0&&nextRoot===root&&nextSection===section&&nextVisible===visible&&accountPage===lastAccountPage&&category===displayedCategory)return;
    if(section==='settings')void call('capture_shortcut',{enabled:false}).catch(error=>notice=String(error));
    const ticket=++sequence;serviceActive=false;await realHongguo?.suspend(nextRoot==='hongguo'&&nextSection==='content');if(ticket!==sequence)return;
    if(page&&visible&&!docked)scrollPositions.set(scrollKey(),page.scrollTop);
    if(root!==nextRoot)notice='就绪';
    root=nextRoot;section=nextSection;visible=nextVisible;lastAccountPage=accountPage;displayedCategory=category;
    try{const lease=await call<View>('transition',{root,section,visible,accountPage});if(ticket!==sequence)return;view=lease;
      serviceActive=root==='hongguo'&&visible&&(section==='content'||accountPage&&section==='settings');
      await tick();if(ticket===sequence&&page)page.scrollTop=scrollPositions.get(scrollKey())??0;
      if(preferences?.root!==root)void save({root});
      if(section==='modules'&&visible&&!catalogReady)void moduleAction('catalog');
    }catch(error){if(ticket===sequence)notice=String(error);}
  }
  async function moduleAction(action:string,id?:string){
    if(managerBusy)return;managerBusy=true;if(action!=='list')notice=action==='catalog'?'正在读取功能目录':action==='uninstall'?'正在卸载':'正在下载安装';
    try{const data=await call<{entries:Module[];installed:{id:Root;name:string}[];catalog_ready:boolean}>('module_call',{action,id});modules=data.entries;roots=data.installed;catalogReady=data.catalog_ready;
      servicePlayback=(await call<{media_route:string}>('snapshot')).media_route==='browser_service';
      if(ready&&!roots.some(m=>m.id===root)&&roots.length)await change(roots[0].id,section);
      if(action!=='list')notice='功能列表已更新';
    }catch(error){notice=`功能操作失败：${error}`;}finally{managerBusy=false;}
  }
  async function toggleModules(){await change(root,moduleTarget(section));if(section==='modules'){await moduleAction('list');if(!catalogReady)void moduleAction('catalog');}}
  async function action(action:string){if(action!=='show')await realHongguo?.suspend(action!=='quit');try{await call('window_action',{action});}catch(error){notice=String(error);}}
  async function togglePin(){try{pinned=await call<boolean>('toggle_pin');void save({topmost:pinned});}catch(error){notice=String(error);}}
  async function checkUpdate(){notice='正在检查应用更新';try{updateReady=await call<boolean>('check_update');notice=updateReady?'应用更新已就绪':'当前已是最新构建';}catch(error){notice=`更新检查失败：${error}`;}}
  function mediaKey(event:KeyboardEvent){if(event.target instanceof HTMLInputElement||event.target instanceof HTMLSelectElement||event.target instanceof HTMLButtonElement||!serviceActive||section!=='content')return;
    if(event.code==='Space'){event.preventDefault();void realHongguo?.toggle();}else if(['ArrowRight','ArrowLeft'].includes(event.code)){event.preventDefault();realHongguo?.seekRelative(event.code==='ArrowRight'?5:-5);}}
  function shortcutLabel(){const s=preferences?.shortcut;if(!s)return '';return `${s.modifiers&2?'Ctrl+':''}${s.modifiers&1?'Alt+':''}${s.modifiers&4?'Shift+':''}${s.key>=112?`F${s.key-111}`:String.fromCharCode(s.key)}`;}
  onMount(()=>{const subscriptions:(()=>void)[]=[];let disposed=false;
    async function subscribe<T>(event:string,handler:(value:T)=>void){const off=await listen<T>(event,e=>handler(e.payload));if(disposed)off();else subscriptions.push(off);}
    void(async()=>{try{
      await Promise.all([subscribe<boolean>('shell-visibility',value=>void change(root,section,value)),subscribe<boolean>('shell-docked',value=>{docked=value;}),subscribe<string>('shell-error',value=>notice=value),subscribe<boolean>('shell-update',value=>{updateReady=value;if(value)notice='应用更新已就绪';})]);
      const saved=await call<{preferences:Preferences;startup:boolean;isolated:boolean}>('settings');preferences=saved.preferences;startup=saved.startup;isolated=saved.isolated;pinned=preferences.topmost;
      await moduleAction('list');root=roots.find(m=>m.id===preferences?.root)?.id??roots[0]?.id??'news';const status=await call<{view:View}>('snapshot');visible=status.view.visible;
      ready=true;eventsReady=true;await change(root,roots.length?'content':'modules',visible);notice=roots.length?'就绪':'从功能页安装需要的功能';
    }catch(error){notice=`启动失败：${error}`;}})();
    const visibility=()=>{if(document.hidden)realHongguo?.pause();};document.addEventListener('visibilitychange',visibility);
    return()=>{disposed=true;subscriptions.forEach(off=>off());document.removeEventListener('visibilitychange',visibility);};
  });
</script>
<svelte:window onkeydown={mediaKey}/>
{#snippet settingsNavigation()}<aside class="settings-nav" aria-label="设置分类">{#if roots.some(m=>m.id===root)}<button class:chosen={category==='page'} onclick={()=>{category='page';void change();}}>{root==='news'?'新闻设置':'红果账号'}</button>{/if}{#each [['appearance','显示设置'],['system','系统设置'],['about','关于我们']] as [id,label]}<button class:chosen={category===id} onclick={()=>{category=id;void change();}}>{label}</button>{/each}<button class="back-button" onclick={()=>change(root,'content')}><Icon name="arrow" size={16}/>返回页面</button></aside>{/snippet}
<div class:dark class:docked class="app-shell" style:--background-opacity={preferences?.background_opacity??.94} style:--card-opacity={preferences?.card_opacity??.94} data-testid="shell" data-root={root} data-section={section} data-visible={visible} data-docked={docked} data-events-ready={eventsReady}>
{#if docked}<button class="dock-button" aria-label="展开 LazyBuddy" onclick={()=>action('expand')}><img src={logoUrl} alt=""/></button>{/if}
<header class="toolbar" data-tauri-drag-region><div class="identity" data-tauri-drag-region><img class="brand-logo" src={logoUrl} alt="" draggable="false" data-tauri-drag-region/><strong data-tauri-drag-region>LazyBuddy</strong><span class="divider" data-tauri-drag-region></span>
<label class="root-selector"><select aria-label="选择功能" value={root} disabled={!roots.length} onchange={e=>change(e.currentTarget.value as Root)}>{#each roots as module}<option value={module.id}>{module.name}</option>{/each}</select><Icon name="chevron" size={14}/></label></div><div class="toolbar-spacer" data-tauri-drag-region></div>
<nav aria-label="全局操作" data-tauri-drag-region><button class="icon-button" class:chosen={pinned} aria-label="置顶" title="置顶" aria-pressed={pinned} onclick={togglePin}><Icon name="pin" size={21}/></button><button class="icon-button" class:chosen={section==='modules'} aria-label="功能" title="功能" onclick={toggleModules}><Icon name="box" size={21}/></button><button class="icon-button" class:chosen={section==='settings'} aria-label="设置" title="设置" onclick={()=>{if(!roots.length)category='appearance';void change(root,toggleSection(section,'settings'));}}><Icon name="settings" size={21}/></button><span class="divider" data-tauri-drag-region></span><button class="icon-button" aria-label="最小化" title="最小化" onclick={()=>action('minimize')}><Icon name="minus" size={20}/></button><button class="icon-button close-button" aria-label="关闭到托盘" title="关闭到托盘" onclick={()=>action('close')}><Icon name="close"/></button></nav></header>
<main class="page" bind:this={page}>
{#if ready}
{#if servicePlayback}<Hongguo bind:this={realHongguo} active={serviceActive&&!docked} showing={root==='hongguo'&&(section==='content'||section==='settings'&&category==='page')} accountOnly={section==='settings'} notice={text=>pageNotice('hongguo',text)} navigation={settingsNavigation}/>{/if}
{#if roots.some(m=>m.id==='news')}<div hidden={root!=='news'||!(section==='content'||section==='settings'&&category==='page')}><News active={root==='news'&&visible&&!docked&&(section==='content'||section==='settings'&&category==='page')} settings={section==='settings'} generation={view.generation} notice={text=>pageNotice('news',text)} navigation={settingsNavigation}/></div>{/if}
{#if section==='modules'}<section class="content-width module-page"><div class="module-heading"><h1>可用功能</h1><button onclick={toggleModules}><Icon name="arrow" size={16}/>返回</button></div><div class="module-list">{#each modules as module}<div class="module-row"><div><strong>{module.name}</strong><span class="version">{module.version}</span></div><div class="row-actions">{#if module.installed}<button disabled={managerBusy||!module.update} onclick={()=>moduleAction('install',module.id)}>更新</button><button class="subtle-danger" disabled={managerBusy} onclick={()=>moduleAction('uninstall',module.id)}>卸载</button>{:else}<button disabled={managerBusy||!catalogReady} onclick={()=>moduleAction('install',module.id)}>安装</button>{/if}</div></div>{/each}</div></section>
{:else if section==='settings'&&category!=='page'}<section class="settings-layout content-width">{@render settingsNavigation()}<div class="settings-content"><h1>{category==='appearance'?'显示设置':category==='system'?'系统设置':'关于我们'}</h1>
{#if category==='appearance'}<div class="setting-row"><strong>深色模式</strong><Switch label="深色模式" checked={dark} onchange={value=>save({dark:value})}/></div><div class="setting-row"><label for="background-opacity">背景透明度</label><input id="background-opacity" aria-label="背景透明度" type="range" min="0" max="1" step=".05" value={preferences?.background_opacity??.94} onchange={event=>save({background_opacity:Number(event.currentTarget.value)})}/></div><div class="setting-row"><label for="card-opacity">卡片透明度</label><input id="card-opacity" aria-label="卡片透明度" type="range" min="0" max="1" step=".05" value={preferences?.card_opacity??.94} onchange={event=>save({card_opacity:Number(event.currentTarget.value)})}/></div>
{:else if category==='system'}<div class="setting-row"><strong>开机启动</strong><Switch label="开机启动" checked={startup} disabled={isolated} onchange={value=>call('set_startup',{enabled:value}).then(()=>startup=value).catch(error=>notice=String(error))}/></div><div class="setting-row"><span>显示 / 隐藏快捷键</span><ShortcutButton active={visible&&!docked} label={shortcutLabel()} save={value=>save({shortcut:value})} notice={text=>notice=text}/></div><div class="setting-row"><strong>应用自动更新</strong><Switch label="应用自动更新" checked={preferences?.auto_update??true} onchange={value=>save({auto_update:value})}/></div><div class="setting-row"><button onclick={checkUpdate}>检查更新</button><button onclick={()=>call('open_logs').catch(error=>notice=String(error))}>打开日志</button></div>
{:else}<h2>LazyBuddy</h2><p>2.0.0</p><p>Copyright © 2026 Leager · MIT</p>{/if}</div></section>
{:else if section==='content'&&!roots.length}<div class="list-empty"><button onclick={toggleModules}>安装功能</button></div>
{:else if section==='content'&&root==='hongguo'&&!servicePlayback}<div class="list-empty">请在功能页更新红果短剧模块</div>{/if}
{/if}
</main>
<footer class="info-bar" aria-live="polite"><span class="status-dot"></span><details class="runtime-notice"><summary class="notice" title="展开运行信息">{notice.split('\n')[0]}</summary><pre>{notice}</pre></details>{#if updateReady}<button onclick={()=>call('apply_update').catch(error=>notice=String(error))}>重启更新</button>{/if}<button class="log-button" onclick={()=>call('open_logs').catch(error=>notice=String(error))}>日志</button></footer>
</div>
