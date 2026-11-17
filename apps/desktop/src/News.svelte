<script lang="ts">
  import {sourceStyle} from './source-style';
  import {invoke} from '@tauri-apps/api/core';
  import {untrack} from 'svelte';
  import Icon from './Icon.svelte';import Switch from './Switch.svelte';
  import {ordered,batches,refreshSources,badgeLabel,sourceFailure,type SourceReply,type Source,type Story,type Preferences} from './news';
  let {active,settings,generation,notice,settingsChange,cardOpacity,setCardOpacity}:{active:boolean;settings:boolean;generation:number;notice:(value:string)=>void;settingsChange:(settings:boolean)=>void;cardOpacity:number;setCardOpacity:(value:number)=>void}=$props();
  let sources=$state<Source[]>([]);let cache=$state<Record<string,{stories:Story[];updated_ms:number}>>({});
  let preferences=$state<Preferences>({auto_refresh:true,refresh_minutes:5,followed:[],hidden:[],order:[],default_followed:false});
  let followed=$state(false);let ready=false;let refreshing=$state<string[]>([]);let allRefreshing=$state(false);let epoch=0;let pending=$state(0);let sourcePending=$state<string[]>([]);let saving=Promise.resolve();
  let direct=$state(localStorage.getItem('lazybuddy.news.direct')!=='false');
  const call=<T,>(method:string,args:Record<string,unknown>={})=>invoke<T>('news_call',{method,args,generation,direct});
  const valid=(ticket:number)=>active&&ticket===epoch;
  function warn(error:unknown){if(!/inactive_|stale_/.test(String(error)))notice(`新闻请求失败：${error}`);}
  async function refresh(force=true,sourceId?:string){
    if(!active||settings||(allRefreshing&&sourceId===undefined)||(sourceId!==undefined&&refreshing.includes(sourceId)))return;
    const ticket=epoch;
    const ids=refreshSources(sources,preferences,followed,cache,force,sourceId).filter(id=>!refreshing.includes(id));
    if(!ids.length)return;
    if(sourceId===undefined)allRefreshing=true;
    try{for(const batch of batches(ids)){
      const group=batch.filter(id=>!refreshing.includes(id));
      if(!group.length)continue;
      if(!valid(ticket))return;
      refreshing=[...refreshing,...group];
      try{
        const rows=await call<SourceReply[]>('fetch',{sources:group});
        if(!valid(ticket))return;
        for(const row of rows){if(row.ok)cache[row.id]=row;else notice(sourceFailure(row,sources));}
      }finally{if(valid(ticket))refreshing=refreshing.filter(id=>!group.includes(id));}
    }}catch(error){if(valid(ticket))warn(error);}finally{if(valid(ticket)&&sourceId===undefined)allRefreshing=false;}
  }
  async function activate(ticket:number){
    if(!ready){try{
      const catalog=await call<Source[]>('sources');if(!valid(ticket))return;
      const saved=await call<{preferences:Preferences;cache:typeof cache}>('state');if(!valid(ticket))return;
      sources=catalog;preferences=saved.preferences;cache=saved.cache;followed=preferences.default_followed;ready=true;
    }catch(error){if(valid(ticket))warn(error);return;}}
    if(!settings)await refresh(false);
  }
  async function configure(next:Preferences|((current:Preferences)=>Preferences),sourceId?:string){
    if(!active||(sourceId?sourcePending.includes(sourceId):pending>0))return;
    const ticket=epoch;pending++;
    if(sourceId)sourcePending=[...sourcePending,sourceId];
    const operation=saving.then(async()=>{
      if(!valid(ticket))return;
      try{const data=await call<{preferences:Preferences}>('configure',{preferences:typeof next==='function'?next(preferences):next});if(valid(ticket))preferences=data.preferences;}
      catch(error){if(valid(ticket))warn(error);}
    });
    saving=operation;
    try{await operation;}finally{pending--;if(sourceId)sourcePending=sourcePending.filter(id=>id!==sourceId);}
  }
  async function changeConnection(value:boolean){
    if(!active||pending)return;
    direct=value;localStorage.setItem('lazybuddy.news.direct',String(value));
    const ticket=++epoch;refreshing=[];allRefreshing=false;pending++;
    try{await call('state');if(valid(ticket))notice(value?'新闻直连已开启':'新闻已恢复跟随系统代理');}
    catch(error){if(valid(ticket))warn(error);}finally{pending--;}
  }
  function toggle(key:'followed'|'hidden'|'order',id:string){void configure(current=>{const values=current[key];return {...current,[key]:values.includes(id)?values.filter(value=>value!==id):[id,...values]};},id);}
  function scrollTitle(event:MouseEvent|FocusEvent){
    const button=event.currentTarget as HTMLButtonElement;
    const viewport=button.firstElementChild as HTMLElement;
    const text=viewport.firstElementChild as HTMLElement;
    const distance=Math.max(0,text.scrollWidth-viewport.clientWidth);
    button.style.setProperty('--title-shift',`-${distance}px`);
    button.style.setProperty('--title-duration',`${Math.max(2,distance/35)}s`);
    button.classList.toggle('title-overflow',distance>0);
  }
  $effect(()=>{const enabled=active;const gen=generation;const config=settings;const ticket=++epoch;refreshing=[];allRefreshing=false;
    if(enabled)untrack(()=>{void activate(ticket);});
    return()=>{epoch++;};
  });
  $effect(()=>{if(!active||settings||!preferences.auto_refresh)return;const timer=setInterval(()=>void refresh(false),preferences.refresh_minutes*60000);return()=>clearInterval(timer);});
</script>
{#snippet tabs()}<div class="tabs" aria-label="新闻页签"><button class:active={!settings&&!followed} onclick={()=>{followed=false;if(settings)settingsChange(false);else void refresh(false);}}>全部来源</button><button class:active={!settings&&followed} onclick={()=>{followed=true;if(settings)settingsChange(false);else void refresh(false);}}>已关注</button><button class:active={settings} title="新闻设置" onclick={()=>settingsChange(true)}>设置</button></div>{/snippet}
{#if settings}
<section class="content-width feature-settings">{@render tabs()}<div class="settings-content"><h1>新闻设置</h1>
  <div class="setting-row"><label for="card-opacity">卡片透明度</label><input id="card-opacity" aria-label="卡片透明度" type="range" min="0" max="1" step=".05" value={cardOpacity} onchange={event=>setCardOpacity(Number(event.currentTarget.value))}/></div>
  <div class="setting-row"><strong>直连</strong><Switch label="直连" checked={direct} disabled={!!pending} onchange={changeConnection}/></div>
  <div class="setting-row"><strong>自动刷新</strong><Switch label="自动刷新" checked={preferences.auto_refresh} onchange={value=>configure({...preferences,auto_refresh:value})}/></div>
  <div class="setting-row"><label for="interval">刷新间隔（分钟）</label><select id="interval" value={preferences.refresh_minutes} onchange={e=>configure({...preferences,refresh_minutes:Number(e.currentTarget.value)})}>{#each [1,3,5,10,15,30,60] as n}<option value={n}>{n}</option>{/each}</select></div>
  <div class="setting-row"><strong>默认只看关注</strong><Switch label="默认只看关注" checked={preferences.default_followed} onchange={value=>configure({...preferences,default_followed:value})}/></div>
  <h2 class="section-title">来源管理</h2>
  {#each sources as source}<div class="source-setting"><span>{source.name}</span><div class="row-actions"><button disabled={sourcePending.includes(source.id)} class:chosen={preferences.followed.includes(source.id)} aria-label={`${preferences.followed.includes(source.id)?'取消关注':'关注'}${source.name}`} title={preferences.followed.includes(source.id)?'取消关注':'关注'} aria-pressed={preferences.followed.includes(source.id)} onclick={()=>toggle('followed',source.id)}><Icon name="star" size={15} filled={preferences.followed.includes(source.id)}/></button><button disabled={sourcePending.includes(source.id)} class:chosen={preferences.hidden.includes(source.id)} onclick={()=>toggle('hidden',source.id)}>隐藏</button><button disabled={sourcePending.includes(source.id)} class:chosen={preferences.order.includes(source.id)} onclick={()=>toggle('order',source.id)}>置顶</button></div></div>{/each}
</div></section>
{:else}
<section class="content-width news-page" class:news-active={active}><div class="list-toolbar">{@render tabs()}<button class="refresh-button" disabled={allRefreshing} onclick={()=>refresh()}><Icon name="refresh" size={15}/>{allRefreshing?'获取中':'刷新'}</button></div>
<div class="source-grid" style:--news-card-opacity={cardOpacity}>{#each ordered(sources,preferences,followed) as source}<article class="source-card" style:--source-color={sourceStyle(source.id).color}><div class="source-heading"><img class="source-logo" src={sourceStyle(source.id).icon} alt="" draggable="false"/><h2 title={source.subtitle?`${source.name} · ${source.subtitle}`:source.name}>{source.name}{#if source.subtitle}<span class="source-subtitle">{' · '}{source.subtitle}</span>{/if}</h2><div class="source-actions"><button disabled={sourcePending.includes(source.id)} class:chosen={preferences.followed.includes(source.id)} title={preferences.followed.includes(source.id)?'取消关注':'关注'} aria-label={`${preferences.followed.includes(source.id)?'取消关注':'关注'}${source.name}`} aria-pressed={preferences.followed.includes(source.id)} onclick={()=>toggle('followed',source.id)}><Icon name="star" size={17} filled={preferences.followed.includes(source.id)}/></button><button disabled={refreshing.includes(source.id)} aria-label={`刷新${source.name}`} title="刷新" onclick={()=>refresh(true,source.id)}><Icon name="refresh" size={16}/></button></div></div><ol>{#each cache[source.id]?.stories??[] as item,index}<li><span class="rank" class:top-rank={index<3}>{index+1}</span><button class="story-title" title={item.title} onmouseenter={scrollTitle} onfocus={scrollTitle} onclick={()=>invoke('open_link',{url:item.url}).catch(warn)}><span class="story-text"><span class="story-text-content">{item.title}</span></span>{#if badgeLabel(item.badge)}<span class="story-badge" class:badge-new={item.badge==='New'}>{badgeLabel(item.badge)}</span>{/if}</button></li>{/each}</ol></article>{/each}</div></section>
{/if}
