<script lang="ts">
  import {onDestroy, tick, untrack} from 'svelte';
  import {invoke} from '@tauri-apps/api/core';
  import Icon from './Icon.svelte';import Cover from './Cover.svelte';
  import Switch from './Switch.svelte';
  import {formatTime} from './lifecycle';
  import {coverPixels,watchedLabel,filterFavorites,loginPending,adjacent,message,qualityLabels,favoriteValue,playbackDetails,playerPreferences,type CoverData,type Account,type Item,type Playback} from './hongguo';
  let {active,showing,accountOnly,notice,settingsChange}: {active:boolean;showing:boolean;accountOnly:boolean;notice:(text:string)=>void;settingsChange:(settings:boolean)=>void}=$props();
  function savedPlayer(){try{return playerPreferences(JSON.parse(localStorage.getItem('lazybuddy.hongguo.player')??'null'));}catch{return playerPreferences(null);}}
  const saved=savedPlayer();let defaultFavorites=$state(saved.favorites);
  let account=$state<Account>({state:'idle',name:''});let signedOut=false;
  let tab=$state<'search'|'favorites'>(saved.favorites?'favorites':'search');
  let mode=$state<'list'|'player'>('list');
  let favoritesView=$state(localStorage.getItem('lazybuddy.hongguo.favoritesView')==='grid'?'grid':'list');
  let searchView=$state(localStorage.getItem('lazybuddy.hongguo.searchView')==='grid'?'grid':'list');
  let listView=$derived(tab==='search'?searchView:favoritesView);
  function toggleView(){const value=listView==='list'?'grid':'list';if(tab==='search')searchView=value;else favoritesView=value;localStorage.setItem(`lazybuddy.hongguo.${tab==='search'?'searchView':'favoritesView'}`,value);}
  let favoritesQuery=$state('');let favoritesRevision=$state(0);
  let query=$state('');let searched=$state(false);let page=1;
  let items=$state<Item[]>([]);let favorites=$state<Item[]>([]);let hasMore=$state(false);let busy=$state(false);
  let visibleFavorites=$derived(filterFavorites(favorites,favoritesQuery));
  let visibleItems=$derived(tab==='search'?items:visibleFavorites);
  let selected=$state<Item>();let episode=$state(1);let episodes=$state<number[]>([]);let quality=$state('720p');let qualities=$state<string[]>([]);
  let source=$state<string>();let lease:number|undefined;let video=$state<HTMLVideoElement>(undefined!);
  let playing=$state(false);let position=$state(0);let duration=$state(0);let muted=$state(saved.mute);let volume=$state(saved.volume);let speed=$state(saved.speed);
  let muteChosen=$state(saved.muteChosen);
  let playerLoading=$state(false);let seeking=$state<number|null>(null);
  let playbackError=$state(false);
  let watched=0;let lastCheckpoint=0;let epoch=0;let playRequest=0;let live=$state(false);let timer:ReturnType<typeof setTimeout>|undefined;let frame:number|undefined;
  let cancelMetadata:(()=>void)|undefined;let history=$state<Record<string,{episode:number;time:number}>>({});
  let opening:Promise<Playback>|undefined;
  const coverCache=new Map<string,CoverData>();let coverEpoch=0;
  $effect(()=>{if(!(active&&live&&mode==='list'&&tab==='favorites'&&favoritesView==='grid'))coverEpoch++;});
  async function readCover(id:string){
    const cache=tab==='favorites';const cached=cache?coverCache.get(id):undefined;if(cached)return cached;
    const ticket=epoch;const generation=coverEpoch;
    const data=await raw<CoverData>('cover.read',{series:id});
    if(!coverPixels(data))throw new Error('invalid_cover');
    if(cache&&valid(ticket)&&generation===coverEpoch){coverCache.set(id,data);if(coverCache.size>64)coverCache.delete(coverCache.keys().next().value!);}
    return data;
  }
  let favoritesReady=false;let favoritesPending=false;
  let favoriteChange=$state<{id:string;value:boolean}|null>(null);
  function isFavorite(id:string|undefined){return favoriteValue(favorites,favoriteChange,id);}
  let requestedQuality=$state(saved.quality);
  $effect(()=>{localStorage.setItem('lazybuddy.hongguo.player',JSON.stringify({volume,mute:muted,muteChosen,speed,quality:requestedQuality,favorites:defaultFavorites}));});
  let direct=$state(localStorage.getItem('lazybuddy.hongguo.direct')!=='false');
  let frameTarget:number|null=null;
  let boostTimer:ReturnType<typeof setTimeout>|undefined;let boosted=false;
  function resetBoost(){clearTimeout(boostTimer);boostTimer=undefined;if(boosted&&video)video.playbackRate=speed;boosted=false;}
  function hold(event:PointerEvent){if(event.button!==2||!live||!playing)return;video.setPointerCapture(event.pointerId);const ticket=epoch;boostTimer=setTimeout(()=>{if(valid(ticket)&&!video.paused){boosted=true;video.playbackRate=2;}},400);}
  function release(event:PointerEvent){if(event.button!==2)return;const accelerated=boosted;resetBoost();if(!accelerated&&live&&playing)seekRelative(5);}
  const raw=<T,>(method:string,args:Record<string,unknown>={})=>invoke<T>('playback_call',{method,args,direct});
  async function changeConnection(value:boolean){
    signedOut=false;direct=value;localStorage.setItem('lazybuddy.hongguo.direct',String(value));
    notice(value?'红果直连已开启':'红果已恢复跟随系统代理');
    await suspend(false);await activate();
  }
  const valid=(ticket:number)=>live&&active&&ticket===epoch;
  function warn(error:unknown){const value=String(error);if(!/stale_|inactive_|页面已暂停/.test(value))notice(message(error));}
  function playerError(error:unknown,stage:string){notice(playbackDetails(error,{title:selected?.title??'当前短剧',episode,quality:requestedQuality,direct,stage}));}
  function stopFrames(){if(video&&frame!==undefined)video.cancelVideoFrameCallback(frame);frame=undefined;}
  function frames(){
    stopFrames();const ticket=epoch;
    const next=(_:number,metadata:VideoFrameCallbackMetadata)=>{
      frame=undefined;if(!valid(ticket)||!video)return;
      if(frameTarget!==null&&Math.abs(metadata.mediaTime-frameTarget)>.2){frame=video.requestVideoFrameCallback(next);return;}
      frameTarget=null;
      watched=metadata.mediaTime;
      if(seeking===null&&video.currentTime>=0)position=video.currentTime;
      if(playing&&performance.now()-lastCheckpoint>15000){lastCheckpoint=performance.now();void checkpoint(false);}
      if(!video.paused)frame=video.requestVideoFrameCallback(next);
    };
    frame=video.requestVideoFrameCallback(next);
  }
  async function checkpoint(final:boolean){
    if(lease===undefined||!duration||watched<=0)return;
    if(selected)history={...history,[selected.id]:{episode,time:watched}};
    const data=await raw<Playback>('playback.checkpoint',{session:lease,position:watched,duration,final});
    if(data.progress_error)notice('观看进度保存失败');
  }
  function detach(){seeking=null;resetBoost();cancelMetadata?.();stopFrames();frameTarget=null;playing=false;if(video){video.pause();video.removeAttribute('src');video.load();}source=undefined;}
  export async function suspend(retain=false){
    const wasLive=live;resetBoost();live=false;epoch++;playRequest++;clearTimeout(timer);timer=undefined;
    if(favoritesPending){favoritesReady=false;favoritesPending=false;favoriteChange=null;}
    if(video&&video.readyState>0)position=video.currentTime;
    if(video)video.pause();stopFrames();playing=false;
    if(wasLive&&source){try{await checkpoint(true);}catch(error){warn(error);}}
    detach();playerLoading=false;busy=false;seeking=null;
    if(!retain)lease=undefined;
    if(loginPending(account.state))account={state:'idle',name:''};
    else account={...account,qr_png:''};
  }
  async function attach(data:Playback,ticket:number){
    if(!valid(ticket)||!data.resource)return;
    lease=data.session;episodes=data.episodes??episodes;qualities=qualityLabels(data.available_qualities??[]);
    const element=video;
    const ready=new Promise<void>((resolve,reject)=>{
      const clear=()=>{element.removeEventListener('loadedmetadata',loaded);element.removeEventListener('error',failed);cancelMetadata=undefined;};
      const loaded=()=>{clear();resolve();};const failed=()=>{clear();reject(new Error('浏览器无法读取或解码该媒体资源'));};
      cancelMetadata=()=>{clear();reject(new Error('页面已暂停'));};
      element.addEventListener('loadedmetadata',loaded,{once:true});element.addEventListener('error',failed,{once:true});
    });
    source=data.resource.url;await tick();await ready;
    if(!valid(ticket))return;
    duration=element.duration;element.playbackRate=speed;position=Math.min(position,Math.max(0,duration-.05));frameTarget=position;frames();element.currentTime=position;
    const actual=qualities.find(value=>Number.parseInt(value)===Math.min(element.videoWidth,element.videoHeight));
    if(actual)quality=actual;
  }
  async function activate(){
    live=true;const ticket=++epoch;
    if(signedOut&&account.state==='idle'){notice('红果账号未登录');return;}
    busy=true;
    try{
      if(mode==='player'&&lease!==undefined&&!accountOnly){await attach(await raw<Playback>('playback.resume',{session:lease}),ticket);return;}
      lease=undefined;
      const data=await raw<Account>('account.restore');if(!valid(ticket))return;
      signedOut=data.state==='idle'&&!data.error;account=data;
      if(data.state!=='signed_in'){favorites=[];favoritesReady=false;coverCache.clear();coverEpoch++;}
      if(data.state==='signed_in')await signedIn(ticket);else if(data.error)notice(message(data.error));else notice('红果账号未登录');
    }catch(error){if(valid(ticket)){lease=undefined;if(mode==='player')playbackError=true;warn(error);}}finally{if(valid(ticket))busy=false;}
  }
  $effect(()=>{const enabled=active;const retain=showing&&!accountOnly;untrack(()=>{if(enabled){void activate();}else{void suspend(retain);}});});
  onDestroy(()=>{void suspend(false);});
  function acceptAccount(data:Account){
    account=data;
    if(data.favorites_error)notice(message(data.favorites_error));
    if(favoritesPending&&data.operation!=='pending'){
      if(!data.favorites_error&&data.favorites){favorites=data.favorites;favoritesReady=true;favoritesRevision++;}
      favoritesPending=false;
      favoriteChange=null;
    }
  }
  async function signedIn(ticket:number){
    notice(`已登录 ${account.name}`);
    if(accountOnly)return;
    const data=await raw<{history:typeof history}>('progress.read');if(!valid(ticket))return;
    history=data.history??{};
    if(!favoritesReady)await readFavorites();
  }
  function poll(ticket:number){
    clearTimeout(timer);
    timer=setTimeout(async()=>{
      if(!valid(ticket))return;
      try{
        const previous=account.state;const data=await raw<Account>('account.status');if(!valid(ticket))return;acceptAccount(data);
        if(previous!=='signed_in'&&data.state==='signed_in')await signedIn(ticket);
        if(loginPending(data.state)||data.operation==='pending')poll(ticket);
        else if(data.error)notice(message(data.error));
      }catch(error){if(valid(ticket))warn(error);}
    },1000);
  }
  async function login(){
    if(!active||!live||busy)return;signedOut=false;busy=true;const ticket=epoch;favorites=[];favoritesReady=false;favoritesPending=false;items=[];history={};coverCache.clear();coverEpoch++;
    try{const data=await raw<Account>('account.login');if(!valid(ticket))return;account=data;notice('请使用红果短剧 App 扫码；登录信息安全保存在本机');poll(ticket);}
    catch(error){if(valid(ticket))warn(error);}finally{if(valid(ticket))busy=false;}
  }
  async function cancelLogin(){
    clearTimeout(timer);const ticket=epoch;
    try{const data=await raw<Account>('account.cancel');if(valid(ticket))account=data;}catch(error){if(valid(ticket))warn(error);}
  }
  async function logout(){
    clearTimeout(timer);detach();lease=undefined;mode='list';favorites=[];favoritesReady=false;favoritesPending=false;items=[];history={};coverCache.clear();coverEpoch++;
    const ticket=epoch;
    try{const data=await raw<Account>('account.logout');if(valid(ticket)){signedOut=data.state==='idle'&&!data.error;account=data;notice('已退出账号并删除其保存凭据');}}catch(error){if(valid(ticket))warn(error);}
  }
  async function search(more=false){
    if(busy||!query.trim())return;busy=true;const ticket=epoch;const word=query.trim();const requested=more?page+1:1;
    try{const data=await raw<{items:Item[];has_more:boolean}>('search',{query:word,page:requested});if(!valid(ticket))return;
      items=more?[...items,...data.items]:data.items;page=requested;hasMore=data.has_more;searched=true;notice(`搜索完成：${word}`);
    }catch(error){if(valid(ticket))warn(error);}finally{if(valid(ticket))busy=false;}
  }
  async function readFavorites(){
    if(favoritesPending)return;favoritesPending=true;
    const ticket=epoch;
    try{const data=await raw<Account>('favorites.read');if(!valid(ticket))return;acceptAccount(data);if(data.operation==='pending')poll(ticket);}
    catch(error){if(valid(ticket)){favoritesPending=false;warn(error);}}
  }
  async function chooseTab(value:'search'|'favorites'){tab=value;if(accountOnly){settingsChange(false);return;}if(value==='favorites'&&account.state==='signed_in'&&!favoritesReady)await readFavorites();}
  async function favorite(item:Item){
    if(account.operation==='pending'||favoriteChange)return;const ticket=epoch;favoritesPending=true;
    const target=!isFavorite(item.id);favoriteChange={id:item.id,value:target};
    try{const data=await raw<Account>('favorites.set',{series:item.id,favorite:target});if(!valid(ticket))return;acceptAccount(data);if(data.operation==='pending')poll(ticket);}
    catch(error){if(valid(ticket)){favoritesPending=false;favoriteChange=null;warn(error);}}
  }
  async function open(item:Item){if(selected?.id!==item.id)episodes=[];selected=item;episode=history[item.id]?.episode??1;position=history[item.id]?.time??0;watched=position;duration=0;mode='player';await tick();await load(true);}
  async function load(autoplay:boolean){
    if(!selected||!live)return;
    const request=++playRequest;playerLoading=true;playbackError=false;quality=requestedQuality;const ticket=epoch;const current=()=>valid(ticket)&&request===playRequest;detach();lease=undefined;qualities=[];
    let stage='取源与媒体准备';
    try{
      const pending=raw<Playback>('playback.open',{series:selected.id,episode,quality:requestedQuality,origin:location.origin});opening=pending;
      let data=await pending;if(opening===pending)opening=undefined;if(!current())return;lease=data.session;
      const deadline=performance.now()+60000;
      while(data.loading&&current()&&performance.now()<deadline){await new Promise(resolve=>setTimeout(resolve,200));if(!current())return;data=await raw<Playback>('playback.status');}
      if(!current())return;
      if(data.error)throw new Error(data.error);
      if(!data.prepared){await raw('playback.close',{session:lease});lease=undefined;throw new Error('媒体资源准备超时');}
      stage='浏览器加载媒体';await attach(data,ticket);if(!current())return;
      if(autoplay){await video.play();if(!current())return;}
      notice(`${selected.title} 第${episode}集`);
    }catch(error){if(current()){playbackError=true;playerError(error,stage);}}finally{if(current())playerLoading=false;}
  }
  export async function toggle(){
    if(!live||mode!=='player'||playerLoading)return;
    if(playbackError||!source||video.error){await load(true);return;}
    if(video.paused){try{await video.play();}catch(error){warn(error);}}
    else{video.pause();void checkpoint(true).catch(warn);}
  }
  function setVolume(value:number){volume=Math.max(0,Math.min(1,Math.round(value*100)/100));}
  function volumeKey(event:KeyboardEvent){
    if(event.defaultPrevented||event.altKey||event.ctrlKey||event.metaKey||!live||mode!=='player'||event.target instanceof HTMLInputElement||event.target instanceof HTMLSelectElement||(event.target instanceof HTMLElement&&event.target.isContentEditable))return;
    if(event.code==='ArrowUp'||event.code==='ArrowDown'){event.preventDefault();setVolume(volume+(event.code==='ArrowUp'?.05:-.05));}
  }
  export function pause(){seeking=null;resetBoost();if(video)video.pause();stopFrames();playing=false;}
  function seekTo(target:number){
    position=target;frameTarget=target;seeking=null;frames();video.currentTime=target;
  }
  function commitSeek(){if(seeking!==null&&source)seekTo(seeking);}
  export function seekRelative(delta:number){if(video&&source)seekTo(Math.max(0,Math.min(duration,video.currentTime+delta)));}
  async function videoError(){
    if(!live||!source||!video?.error)return;
    playing=false;playbackError=true;stopFrames();
    const ticket=epoch;const request=playRequest;const code=video.error.code;
    if(lease!==undefined)await raw('playback.report_error',{session:lease,code}).catch(()=>{});
    try{const data=await raw<Playback>('playback.status');if(!valid(ticket)||request!==playRequest)return;
      playerError(data.error??(code===4?'browser_format_unsupported':code===3?'browser_decode_failed':'media_network_error'),'浏览器播放');
    }catch(error){if(valid(ticket))warn(error);}
  }
  async function leavePlayer(){playRequest++;pause();const pending=opening;await checkpoint(true).catch(warn);detach();const old=lease;lease=undefined;mode='list';
    try{const session=old??(pending? (await pending).session:undefined);if(session!==undefined)await raw('playback.close',{session});}catch(error){if(String(error)!=='media_not_loaded')warn(error);}}
  async function changeEpisode(value:number){if(value===episode)return;await checkpoint(true).catch(warn);episode=value;position=watched=0;await load(true);}
  async function changeQuality(value:string){requestedQuality=quality=value;position=video?.currentTime??position;await load(!source||!!video?.error||playing);}
  function ended(){if(!live||!active||playerLoading||!video.ended)return;playing=false;stopFrames();void checkpoint(true).catch(warn);const next=adjacent(episodes,episode,1);if(next!==undefined)void changeEpisode(next);}
</script>

{#snippet tabs()}<div class="tab-bar"><div class="tabs" aria-label="短剧页签"><button class:active={!accountOnly&&tab==='search'} aria-pressed={!accountOnly&&tab==='search'} title="搜索" onclick={()=>chooseTab('search')}>搜索</button><button class:active={!accountOnly&&tab==='favorites'} aria-pressed={!accountOnly&&tab==='favorites'} title="收藏" onclick={()=>chooseTab('favorites')}>收藏</button><button class:active={accountOnly} aria-pressed={accountOnly} title="短剧设置" onclick={()=>settingsChange(true)}>设置</button></div></div>{/snippet}

<svelte:window onkeydown={volumeKey} onblur={resetBoost}/>

{#if showing}
  {#if accountOnly}
    <section class="content-width feature-page feature-settings">{@render tabs()}<div class="settings-content"><h1>红果账号</h1><div class="setting-row"><strong>直连</strong><Switch label="直连" checked={direct} disabled={busy||loginPending(account.state)||account.operation==='pending'} onchange={changeConnection}/></div><div class="setting-row"><strong>默认显示收藏</strong><Switch label="默认显示收藏" checked={defaultFavorites} onchange={value=>defaultFavorites=value}/></div><div class="settings-actions">{#if account.state==='signed_in'}<p>{account.name}</p><button class="subtle-danger" title="退出登录" onclick={logout}>退出登录</button>{:else}<button title="登录" onclick={login} disabled={!active||!live||busy||loginPending(account.state)}>登录</button>{/if}</div></div></section>
  {:else if account.state!=='signed_in'}
    <section class="content-width feature-page feature-settings">{@render tabs()}<div class="settings-actions"><button onclick={login} disabled={!active||!live||busy||loginPending(account.state)}>登录</button></div></section>
  {:else if mode==='player'}
    <section class="player-page"><div class="video-stage" class:paused={!playing}>
      <video role="button" tabindex="0" onclick={toggle} onkeydown={event=>{if(event.code==='Enter'&&!event.repeat){event.preventDefault();void toggle();}}} onpointerdown={hold} onpointerup={release} onpointercancel={resetBoost} onlostpointercapture={resetBoost} oncontextmenu={event=>event.preventDefault()} bind:this={video} src={source} preload="metadata" crossorigin="anonymous" playsinline bind:muted bind:volume ontimeupdate={()=>{if(video.readyState>0&&seeking===null&&frameTarget===null)position=video.currentTime;}} onplay={()=>{if(!live){video.pause();return;}playing=true;frames();}} onpause={()=>{seeking=null;resetBoost();playing=false;stopFrames();}} onended={ended} onerror={videoError} aria-label="红果短剧视频"><track kind="captions"/></video>
      <div class="video-top"><button class="video-back" aria-label="返回列表" onclick={leavePlayer}><Icon name="arrow" size={17}/></button><span class="video-title">{selected?.title} 第{episode}集</span><button class="video-favorite" aria-label={isFavorite(selected?.id)?'取消收藏':'收藏'} onclick={()=>selected&&favorite(selected)} disabled={account.operation==='pending'}><Icon name="star" filled={isFavorite(selected?.id)}/></button></div>
      {#if !playing}<button class="center-play" aria-label={playerLoading?'加载中':playbackError?'重试播放':'播放视频'} disabled={playerLoading} onclick={toggle}><Icon name={playbackError?'refresh':'play'} size={28}/></button>{/if}
      <div class="video-bottom"><input class="seek" aria-label="播放进度" type="range" min="0" max={duration||1} step=".05" value={seeking??position} disabled={!source} oninput={event=>seeking=Number(event.currentTarget.value)} onchange={commitSeek} onpointercancel={()=>seeking=null} onblur={()=>seeking=null}/><div class="video-controls">
        <button aria-label="上一集" disabled={adjacent(episodes,episode,-1)===undefined} onclick={()=>{const value=adjacent(episodes,episode,-1);if(value!==undefined)void changeEpisode(value);}}><Icon name="previous"/></button>
        <button aria-label={playing?'暂停':'播放'} disabled={playerLoading} onclick={toggle}><Icon name={playing?'pause':'play'}/></button>
        <button aria-label="下一集" disabled={adjacent(episodes,episode,1)===undefined} onclick={()=>{const value=adjacent(episodes,episode,1);if(value!==undefined)void changeEpisode(value);}}><Icon name="next"/></button>
        <span class="time">{formatTime(seeking??position)} / {formatTime(duration)}</span><span class="control-spacer"></span>
        {#if episodes.length}<select aria-label="选集" value={episode} onchange={event=>changeEpisode(Number(event.currentTarget.value))}>{#each episodes as value}<option value={value}>第{value}集</option>{/each}</select>{/if}
        <select aria-label="清晰度" value={quality} onchange={event=>changeQuality(event.currentTarget.value)}>{#each qualities.length?qualities:['360p','480p','720p','1080p'] as value}<option value={value}>{value}</option>{/each}</select>
        <select aria-label="倍速" value={speed} onchange={event=>{speed=Number(event.currentTarget.value);video.playbackRate=speed;}}>{#each [.5,1,1.5,2] as value}<option value={value}>{value}×</option>{/each}</select>
        <div class="volume-control"><button aria-label={muted?'取消静音':'静音'} title="音量（↑/↓ 调节）" onclick={()=>{muteChosen=true;muted=!muted;}}><Icon name={muted?'mute':'volume'}/></button><div class="volume-popover"><output aria-hidden="true">{Math.round(volume*100)}%</output><input class="volume" aria-label="音量" aria-orientation="vertical" aria-valuetext={`${Math.round(volume*100)}%`} type="range" min="0" max="1" step=".05" value={volume} oninput={event=>setVolume(Number(event.currentTarget.value))}/></div></div>
      </div></div>
    </div></section>
  {:else}
    <section class="media-library feature-page">{@render tabs()}
      <form onsubmit={event=>{event.preventDefault();if(tab==='search')void search();else void readFavorites();}}><div class="media-search collection-toolbar" role="toolbar" aria-label={tab==='search'?'搜索工具栏':'收藏工具栏'}><label class="search-box"><Icon name="search" size={16}/>{#if tab==='search'}<input aria-label="搜索短剧" placeholder="搜索短剧" bind:value={query}/>{:else}<input aria-label="筛选收藏" placeholder="筛选收藏" bind:value={favoritesQuery}/>{/if}</label><button type="button" class="icon-button" aria-label={listView==='list'?'切换图标视图':'切换列表视图'} title={listView==='list'?'图标视图':'列表视图'} onclick={toggleView}><Icon name={listView==='list'?'grid':'list'} size={15}/></button><button type="submit" class="icon-button" aria-label={tab==='search'?'搜索':'刷新收藏'} title={tab==='search'?'搜索':'刷新收藏'} disabled={tab==='search'?busy||!query.trim():account.operation==='pending'}><Icon name={tab==='search'?'search':'refresh'} size={15}/></button></div></form>
      {#key tab==='favorites'?`favorites:${favoritesRevision}`:tab}
      {#if listView==='grid'}
      <div class="favorites-grid">{#each visibleItems as item (item.id)}<article class="favorite-tile"><button class="cover-open" aria-label={`播放${item.title}`} onclick={()=>open(item)}><Cover active={active&&live&&item.has_cover!==false} title={item.title} load={()=>readCover(item.id)}/><strong title={item.title}>{item.title}</strong><span class="item-description">{watchedLabel(history[item.id])}{item.episodes?` · 共 ${item.episodes} 集`:''}</span></button><button class="grid-favorite" aria-label={isFavorite(item.id)?`取消收藏${item.title}`:`收藏${item.title}`} disabled={account.operation==='pending'||!!favoriteChange} onclick={()=>favorite(item)}><Icon name="star" size={15} filled={isFavorite(item.id)}/></button></article>{/each}</div>
      {:else}
      {#each visibleItems as item (item.id)}<div class="media-result collection-row"><div><h2 class="item-title" title={item.title}>{item.title}</h2><span class="item-description">{watchedLabel(history[item.id])}{item.episodes?` · 共 ${item.episodes} 集`:''}</span></div><div class="row-actions"><button aria-label={isFavorite(item.id)?`取消收藏${item.title}`:`收藏${item.title}`} disabled={account.operation==='pending'||!!favoriteChange} onclick={()=>favorite(item)}><Icon name="star" size={15} filled={isFavorite(item.id)}/></button><button aria-label={`播放${item.title}`} onclick={()=>open(item)}><Icon name="play" size={15}/>播放</button></div></div>{/each}
      {/if}
      {/key}
      {#if tab==='search'&&hasMore}<button disabled={busy} onclick={()=>search(true)}>更多</button>{/if}
      {#if tab==='favorites'&&favorites.length===0&&account.operation!=='pending'}<div class="list-empty">暂无收藏</div>{:else if tab==='favorites'&&visibleFavorites.length===0}<div class="list-empty">没有匹配的收藏</div>{:else if tab==='search'&&searched&&items.length===0}<div class="list-empty">没有搜索结果</div>{/if}
    </section>
  {/if}
  {#if loginPending(account.state)}<div class="qr-backdrop"><div class="qr-dialog" role="dialog" aria-modal="true" aria-label="扫码登录" tabindex="-1"><h2>使用红果短剧 App 扫码</h2>{#if account.qr_png}<img src={`data:image/png;base64,${account.qr_png}`} alt="红果登录二维码"/>{/if}<button onclick={cancelLogin}>取消</button></div></div>{/if}
{/if}

<style>
  .qr-backdrop{position:fixed;inset:62px 0 30px;display:grid;place-items:center;background:#0005;z-index:10}.qr-dialog{display:flex;flex-direction:column;align-items:center;gap:12px;padding:20px;background:var(--surface);border:1px solid var(--line)}.qr-dialog h2{font-size:15px;margin:0}.qr-dialog img{width:220px;height:220px;object-fit:contain;background:white}.qr-dialog button{align-self:stretch}
  .video-controls select{width:auto;max-width:85px;min-width:44px;color:white;background:#222c;padding:3px;border:0;font-size:11px}.video-controls{flex-wrap:wrap}.video-title{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
</style>
