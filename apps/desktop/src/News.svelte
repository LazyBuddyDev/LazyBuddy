<script lang="ts">
  import {invoke} from '@tauri-apps/api/core';
  import {untrack,type Snippet} from 'svelte';
  import Icon from './Icon.svelte';import Switch from './Switch.svelte';
  import {ordered,batches,type Source,type Story,type Preferences} from './news';
  let {active,settings,generation,notice,navigation}:{active:boolean;settings:boolean;generation:number;notice:(value:string)=>void;navigation:Snippet}=$props();
  let sources=$state<Source[]>([]);let cache=$state<Record<string,{stories:Story[];updated_ms:number}>>({});
  let preferences=$state<Preferences>({auto_refresh:true,refresh_minutes:5,followed:[],hidden:[],order:[],default_followed:false});
  let followed=$state(false);let ready=false;let loading=$state(false);let epoch=0;let pending=$state(0);
  const call=<T,>(method:string,args:Record<string,unknown>={})=>invoke<T>('news_call',{method,args,generation});
  const valid=(ticket:number)=>active&&ticket===epoch;
  function warn(error:unknown){if(!/inactive_|stale_/.test(String(error)))notice(`新闻请求失败：${error}`);}
  async function refresh(force=true){
    if(!active||settings||loading)return;const ticket=epoch;loading=true;
    const ids=ordered(sources,preferences,followed).filter(s=>force||!cache[s.id]||Date.now()-cache[s.id].updated_ms>=preferences.refresh_minutes*60000).map(s=>s.id);
    try{for(const group of batches(ids)){
      if(!valid(ticket))return;
      const rows=await call<{id:string;ok:boolean;stories:Story[];updated_ms:number}[]>('fetch',{sources:group});
      if(!valid(ticket))return;
      for(const row of rows){if(row.ok)cache[row.id]=row;else notice(`来源 ${row.id} 暂时无法获取，保留已有内容`);}
    }}catch(error){if(valid(ticket))warn(error);}finally{if(valid(ticket))loading=false;}
  }
  async function activate(ticket:number){
    if(!ready){try{
      const catalog=await call<Source[]>('sources');if(!valid(ticket))return;
      const saved=await call<{preferences:Preferences;cache:typeof cache}>('state');if(!valid(ticket))return;
      sources=catalog;preferences=saved.preferences;cache=saved.cache;followed=preferences.default_followed;ready=true;
    }catch(error){if(valid(ticket))warn(error);return;}}
    if(!settings)await refresh(false);
  }
  async function configure(next:Preferences){
    if(!active||!settings||pending)return;const ticket=epoch;pending++;
    try{const data=await call<{preferences:Preferences}>('configure',{preferences:next});if(valid(ticket))preferences=data.preferences;}
    catch(error){if(valid(ticket))warn(error);}finally{pending--;}
  }
  function toggle(key:'followed'|'hidden'|'order',id:string){const values=preferences[key];void configure({...preferences,[key]:values.includes(id)?values.filter(value=>value!==id):[id,...values]});}
  $effect(()=>{const enabled=active;const gen=generation;const config=settings;const ticket=++epoch;loading=false;
    if(enabled)untrack(()=>{void activate(ticket);});
    return()=>{epoch++;};
  });
  $effect(()=>{if(!active||settings||!preferences.auto_refresh)return;const timer=setInterval(()=>void refresh(false),preferences.refresh_minutes*60000);return()=>clearInterval(timer);});
</script>
{#if settings}
<section class="settings-layout content-width">{@render navigation()}<div class="settings-content"><h1>新闻设置</h1>
  <div class="setting-row"><strong>自动刷新</strong><Switch label="自动刷新" checked={preferences.auto_refresh} onchange={value=>configure({...preferences,auto_refresh:value})}/></div>
  <div class="setting-row"><label for="interval">刷新间隔（分钟）</label><select id="interval" value={preferences.refresh_minutes} onchange={e=>configure({...preferences,refresh_minutes:Number(e.currentTarget.value)})}>{#each [1,3,5,10,15,30,60] as n}<option value={n}>{n}</option>{/each}</select></div>
  <div class="setting-row"><strong>默认只看关注</strong><Switch label="默认只看关注" checked={preferences.default_followed} onchange={value=>configure({...preferences,default_followed:value})}/></div>
  <h2 class="section-title">来源管理</h2>
  {#each sources as source}<div class="source-setting"><span>{source.name}</span><div class="row-actions"><button disabled={!!pending} class:chosen={preferences.followed.includes(source.id)} onclick={()=>toggle('followed',source.id)}>关注</button><button disabled={!!pending} class:chosen={preferences.hidden.includes(source.id)} onclick={()=>toggle('hidden',source.id)}>隐藏</button><button disabled={!!pending} class:chosen={preferences.order.includes(source.id)} onclick={()=>toggle('order',source.id)}>置顶</button></div></div>{/each}
</div></section>
{:else}
<section class="content-width news-page"><div class="list-toolbar"><div class="tabs"><button class:active={!followed} onclick={()=>{followed=false;void refresh(false);}}>全部来源</button><button class:active={followed} onclick={()=>{followed=true;void refresh(false);}}>已关注</button></div><button class="refresh-button" disabled={loading} onclick={()=>refresh()}><Icon name="refresh" size={15}/>{loading?'获取中':'刷新'}</button></div>
<div class="source-grid">{#each ordered(sources,preferences,followed) as source}<article class="source-card"><div class="source-heading"><h2>{source.name}</h2></div><ol>{#each cache[source.id]?.stories??[] as item,index}<li><span class="rank" class:top-rank={index<3}>{index+1}</span><button title={item.title} onclick={()=>invoke('open_link',{url:item.url}).catch(warn)}>{item.title}</button></li>{/each}</ol></article>{/each}</div></section>
{/if}
