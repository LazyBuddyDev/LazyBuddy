<script lang="ts">
  import {invoke} from '@tauri-apps/api/core';
  import {onDestroy,tick,untrack} from 'svelte';
  import Icon from './Icon.svelte';import MediaControls from './MediaControls.svelte';
  let {active,showing,settings,generation,notice,settingsChange}:{active:boolean;showing:boolean;settings:boolean;generation:number;notice:(text:string)=>void;settingsChange:(value:boolean)=>void}=$props();
  type Account={state:string;name:string;qr_image:string;attempt:number};type Item={id:string;title:string;author:string;duration:string};type Episode={bvid:string;cid:string;title:string};type Media={session:number;resource:{url:string;mime:string};title:string;bvid:string;cid:string;duration:number;entries:{items:Episode[];complete:boolean}};type Listing={items:Item[];page:number;has_more:boolean;offset:string;loaded:boolean};
  const empty=():Listing=>({items:[],page:1,has_more:false,offset:'',loaded:false});let tab=$state<'home'|'search'|'following'>('home');let lists=$state({home:empty(),search:empty(),following:empty()});let fresh=0;let episodes=$state<Episode[]>([]);let current=$state<Episode>();
  let account=$state<Account>({state:'idle',name:'',qr_image:'',attempt:0});let query=$state('');let items=$derived(lists[tab].items);let more=$derived(lists[tab].has_more);let term='';let searched=$derived(lists[tab].loaded);let busy=$state(false);let failure=$state('');let selected=$state<Item>();let resource=$state<Media>();let mode=$state<'list'|'player'>('list');
  let listElement=$state<HTMLElement>();const scrolls=new Map<string,number>();const scrollPage=()=>listElement?.closest('.page');function rememberList(){if(live&&mode==='list'&&!settings)scrolls.set(tab,scrollPage()?.scrollTop??0);}async function restoreList(ticket=epoch){await tick();if(valid(ticket)&&mode==='list'&&!settings){const page=scrollPage();if(page)page.scrollTop=scrolls.get(tab)??0;}}
  let video=$state<HTMLVideoElement>();let playing=$state(false);let position=$state(0);let duration=$state(0);let seeking=$state<number|null>(null);let loading=$state(false);let epoch=0;let live=false;let initialized=false;let poll:ReturnType<typeof setTimeout>|undefined;
  let volume=$state(.5),muted=$state(false),speed=$state(1);let muteChosen=false;try{const p=JSON.parse(localStorage.getItem('lazybuddy.bilibili.player')??'{}');if(Number.isFinite(p.volume))volume=Math.max(0,Math.min(1,p.volume));if([.5,.75,1,1.25,1.5,2].includes(p.speed))speed=p.speed;muteChosen=p.muteChosen===true;muted=muteChosen&&p.mute===true;}catch{}
  $effect(()=>{localStorage.setItem('lazybuddy.bilibili.player',JSON.stringify({volume,speed,mute:muted,muteChosen}));});
  const delays=new Map<ReturnType<typeof setTimeout>,()=>void>();let cancelMetadata:(()=>void)|undefined;let boost:ReturnType<typeof setTimeout>|undefined;let boosted=false;
  const valid=(ticket:number)=>active&&live&&epoch===ticket;
  const raw=<T,>(method:string,args:Record<string,unknown>={},g=generation)=>invoke<T>('bilibili_call',{method,args,generation:g});
  function message(error:unknown){const code=String(error);const messages:Record<string,string>={account_required:'此操作需要登录B站账号',account_expired:'登录已过期，请重新扫码',source_access_denied:'来源拒绝访问，本次请求已停止',source_cdn_rejected:'当前来源地址尚未支持，本次请求已停止',source_verification_required:'B站要求完成官方验证，本次请求已停止',source_rate_limited:'请求过于频繁，请稍后手动重试',source_timeout:'来源响应超时',source_network_failed:'无法连接来源',source_tls_failed:'来源安全连接失败，请检查证书或代理',source_proxy_failed:'代理连接失败，请检查系统代理',source_dns_failed:'来源域名解析失败',source_connect_failed:'无法建立来源连接',source_http_failed:'来源返回异常响应，请稍后手动重试',source_incomplete_media:'来源只提供部分内容，本次请求已停止',unsupported_media_format:'当前视频格式尚未验证',credential_save_failed:'账号安全保存失败，尚未确认登录',credential_read_failed:'账号读取失败，原有凭据已保留',qr_expired:'二维码已过期，请重新点击登录',request_busy:'上一项请求仍在收尾，请稍后操作',bilibili_disconnected:'B站服务已断开，请重新进入页面后重试',video_decode_failed:'视频读取或解码失败，点击重试播放',invalid_source_response:'来源响应格式异常'};return messages[code]??(code.startsWith('invalid_search_')?'搜索数据格式异常':'操作未完成，请稍后手动重试');}
  function warn(error:unknown){if(String(error)==='bilibili_disconnected')initialized=false;if(['account_required','account_expired'].includes(String(error))){account={...account,state:'expired'};lists={home:empty(),search:empty(),following:empty()};detach();resource=undefined;mode='list';}failure=(mode==='player'&&selected?`${selected.title}${current?' · '+current.title:''}：`:'')+message(error);notice(failure);}
  function sleep(ms:number){return new Promise<void>(resolve=>{const t=setTimeout(()=>{delays.delete(t);resolve();},ms);delays.set(t,resolve);});}
  async function work<T>(method:string,args:Record<string,unknown>,ticket:number):Promise<T>{const g=generation;const ack=await raw<{job:number}>(method,args,g);if(!valid(ticket))throw 'cancelled';if(method==='account.login')account={...account,attempt:ack.job};while(valid(ticket)){const r=await raw<{status:string;error?:string;result:T}>('task.status',{job:ack.job},g);if(!valid(ticket))throw 'cancelled';if(r.status==='complete')return r.result;if(r.status!=='pending')throw r.error??'cancelled';await sleep(100);}throw 'cancelled';}
  function qrModal(node:HTMLDialogElement){node.showModal();return{destroy(){node.close();}};}
  function stopBoost(){clearTimeout(boost);boost=undefined;if(boosted&&video)video.playbackRate=speed;boosted=false;}
  function detach(){seeking=null;stopBoost();cancelMetadata?.();cancelMetadata=undefined;if(video){if(video.readyState>0&&Number.isFinite(video.currentTime))position=video.currentTime;video.pause();video.removeAttribute('src');video.load();}playing=false;}
  export function suspend(release=false){rememberList();live=false;epoch++;clearTimeout(poll);poll=undefined;for(const[t,r]of delays){clearTimeout(t);r();}delays.clear();detach();busy=false;loading=false;account={...account,qr_image:'',attempt:0,state:account.state==='signed_in'?'signed_in':'idle'};if(release)resource=undefined;}
  async function suspendTab(){const g=generation;suspend();try{await invoke('bilibili_suspend',{generation:g,release:false});}catch{}}
  async function activate(){
    live=true;const ticket=++epoch;void restoreList(ticket);busy=true;
    try{
      const result=initialized?await raw<Account>('account.get'):await work<Account>('account.restore',{},ticket);
      if(valid(ticket)){account=result;initialized=true;if(account.state!=='signed_in'){lists={home:empty(),search:empty(),following:empty()};resource=undefined;mode='list';}}
    }catch(e){if(valid(ticket)){initialized=true;warn(e);}}
    finally{if(valid(ticket)){busy=false;if(account.state==='signed_in'&&!settings&&!lists[tab].loaded&&tab!=='search')void loadList();}}
  }
  $effect(()=>{const enabled=active;untrack(()=>{if(enabled)void activate();else void suspend();});});onDestroy(()=>{void suspend();});
  async function loadList(next=false){
    if(!live||busy||settings||tab==='search'&&!query.trim())return;
    if(account.state!=='signed_in'){notice('登录后可使用推荐、搜索和动态');return;}
    const ticket=epoch,targetTab=tab;busy=true;failure='';
    try{
      const prior=lists[targetTab],target=next?prior.page+1:1,q=next?term:query.trim();
      const method=targetTab==='home'?'recommend':targetTab==='following'?'following':'search';
      const args=targetTab==='home'?{fresh:++fresh}:targetTab==='following'?{offset:next?prior.offset:''}:{query:q,page:target};
      const result=await work<Partial<Listing>&{items:Item[];has_more:boolean}>(method,args,ticket);
      if(!valid(ticket))return;
      lists[targetTab]={items:result.items,page:target,has_more:result.has_more,offset:result.offset??'',loaded:true};
      if(targetTab==='search')term=q;
    }catch(e){if(valid(ticket))warn(e);}finally{if(valid(ticket))busy=false;}
  }
  async function chooseTab(next:typeof tab){
    if(!settings&&next===tab)return;
    if(settings){tab=next;settingsChange(false);return;}
    await suspendTab();if(!active)return;live=true;epoch++;tab=next;failure='';void restoreList();
    if(!lists[next].loaded&&next!=='search')void loadList();
  }
  async function login(){if(!live||busy)return;const ticket=epoch;busy=true;failure='';account={state:'preparing',name:'',qr_image:'',attempt:0};try{account=await work<Account>('account.login',{},ticket);if(valid(ticket))schedulePoll(ticket);}catch(e){if(valid(ticket)){account={...account,state:'idle',qr_image:''};warn(e);}}finally{if(valid(ticket))busy=false;}}
  function schedulePoll(ticket:number){if(!valid(ticket)||!['pending','scanned'].includes(account.state))return;poll=setTimeout(async()=>{poll=undefined;if(!valid(ticket))return;try{account=await work<Account>('account.poll',{attempt:account.attempt},ticket);if(valid(ticket)){if(account.state==='signed_in'){notice('B站登录已确认并安全保存');if(!settings&&tab!=='search'&&!lists[tab].loaded)void loadList();}else schedulePoll(ticket);}}catch(e){if(valid(ticket)){account={...account,state:'idle',qr_image:''};warn(e);}}},2000);}
  async function cancelLogin(){await suspendTab();if(active)await activate();}
  async function logout(){if(!live||busy)return;const ticket=++epoch;busy=true;try{const result=await raw<Account>('account.logout');if(!valid(ticket))return;account=result;lists={home:empty(),search:empty(),following:empty()};episodes=[];current=undefined;resource=undefined;selected=undefined;mode='list';notice('已退出B站账号');}catch(e){if(valid(ticket))warn(e);}finally{if(valid(ticket))busy=false;}}
  async function attachAndPlay(ticket:number){await tick();if(!valid(ticket)||!video||!resource)return;const element=video;const src=resource.resource.url;
    if(!element.getAttribute('src')){const ready=new Promise<void>((resolve,reject)=>{const clean=()=>{element.removeEventListener('loadedmetadata',loaded);element.removeEventListener('error',failed);cancelMetadata=undefined;};const loaded=()=>{clean();resolve();};const failed=()=>{clean();reject('video_decode_failed');};cancelMetadata=()=>{clean();reject('cancelled');};element.addEventListener('loadedmetadata',loaded,{once:true});element.addEventListener('error',failed,{once:true});});element.src=src;element.preload='metadata';element.load();await ready;if(!valid(ticket))return;duration=element.duration;if(position>0)element.currentTime=Math.min(position,Math.max(0,element.duration-.1));}
    element.playbackRate=speed;element.preservesPitch=true;await element.play();if(!valid(ticket))element.pause();
  }
  async function open(item:Item,episode?:Episode,retry=false){
    if(!live||busy||account.state!=='signed_in')return;rememberList();const ticket=++epoch;detach();if(!retry){position=0;duration=0;}
    selected=item;current=episode;mode='player';busy=loading=true;failure='';resource=undefined;
    try{
      const result=await work<Media>('playback.open',{bvid:episode?.bvid??item.id,...(episode?{cid:episode.cid}:{}),origin:location.origin},ticket);
      if(!valid(ticket))return;
      resource=result;duration=result.duration;episodes=result.entries.items;current=episodes.find(e=>e.bvid===result.bvid&&e.cid===result.cid)??{bvid:result.bvid,cid:result.cid,title:result.title};
      if(!result.entries.complete)notice('合集目录不完整，仅播放当前视频');
      await attachAndPlay(ticket);
    }catch(e){if(valid(ticket))warn(e);}finally{if(valid(ticket))busy=loading=false;}
  }
  function commitSeek(){if(seeking!==null&&video&&resource&&video.readyState>0){video.currentTime=Math.max(0,Math.min(duration,seeking));position=video.currentTime;}seeking=null;}
  function episodeIndex(){return episodes.findIndex(e=>e.bvid===current?.bvid&&e.cid===current?.cid);}
  function changeEpisode(index:number){const e=episodes[index];if(e&&selected&&!loading)void open(selected,e);}
  export async function toggle(){if(!live||!video||loading)return;if(!resource){if(selected)await open(selected,current,true);return;}if(!video.paused){video.pause();playing=false;position=video.currentTime;detach();const ticket=epoch;await raw('playback.suspend',{session:resource.session}).catch(e=>{if(valid(ticket))warn(e);});return;}const ticket=epoch;loading=true;try{await raw('playback.resume',{session:resource.session});if(valid(ticket))await attachAndPlay(ticket);}catch(e){if(valid(ticket))warn(e);}finally{if(valid(ticket))loading=false;}}
  async function back(){const ticket=++epoch;detach();const old=resource;resource=undefined;mode='list';busy=loading=false;void restoreList(ticket);try{if(old)await raw('playback.close',{session:old.session});else await invoke('bilibili_suspend',{generation});}catch(e){if(valid(ticket))warn(e);}}
  function setVolume(value:number){volume=Math.max(0,Math.min(1,value));}
  function key(event:KeyboardEvent){if(!live||settings||mode!=='player'||event.target instanceof HTMLInputElement||event.target instanceof HTMLTextAreaElement||event.target instanceof HTMLSelectElement||event.target instanceof HTMLButtonElement||event.target instanceof HTMLElement&&event.target.isContentEditable)return;if(event.code==='Space'||event.code==='Enter'){event.preventDefault();void toggle();}else if((event.code==='ArrowLeft'||event.code==='ArrowRight')&&video&&video.readyState>0&&Number.isFinite(video.duration)){event.preventDefault();video.currentTime=Math.max(0,Math.min(video.duration,video.currentTime+(event.code==='ArrowRight'?5:-5)));}else if(event.code==='ArrowUp'||event.code==='ArrowDown'){event.preventDefault();setVolume(volume+(event.code==='ArrowUp'?.05:-.05));}}
  function hold(event:PointerEvent){if(event.button!==2||!playing||!video)return;video.setPointerCapture(event.pointerId);const ticket=epoch;boost=setTimeout(()=>{if(valid(ticket)&&video&&!video.paused){video.playbackRate=2;boosted=true;}},400);}
</script>
<svelte:window onkeydown={key} onblur={stopBoost}/>
{#snippet tabs()}<div class="tab-bar"><div class="tabs" aria-label="B站页签">{#each [['home','推荐'],['search','搜索'],['following','动态']] as [id,label]}<button class:active={!settings&&tab===id} aria-pressed={!settings&&tab===id} title={label} onclick={()=>chooseTab(id as typeof tab)}>{label}</button>{/each}<button class:active={settings} aria-pressed={settings} title="B站设置" onclick={()=>settingsChange(true)}>设置</button></div></div>{/snippet}
{#if showing}
 {#if settings}<section class="content-width feature-page feature-settings">{@render tabs()}<div class="settings-content"><h1>B站账号</h1><div class="settings-actions">{#if account.state==='signed_in'}<p>{account.name}</p><button class="subtle-danger" title="退出登录" disabled={busy} onclick={logout}>退出登录</button>{:else}<button title="登录" disabled={!active||busy} onclick={login}>登录</button>{/if}</div></div></section>
 {:else if account.state!=='signed_in'}<section class="content-width feature-page feature-settings">{@render tabs()}<div class="settings-actions"><button title="登录B站后使用推荐、搜索和动态" aria-label="登录" disabled={!active||busy||['preparing','pending','scanned'].includes(account.state)} onclick={login}>登录</button></div></section>
 {:else if mode==='player'}<section class="player-page"><div class="video-stage" class:paused={!playing}>
  <video bind:this={video} class="focus-inset" role="button" aria-label="视频画面，点击播放或暂停" tabindex="0" crossorigin="anonymous" preload="none" playsinline bind:volume bind:muted onclick={()=>toggle()} onpointerdown={hold} onpointerup={stopBoost} onpointercancel={stopBoost} onlostpointercapture={stopBoost} oncontextmenu={e=>e.preventDefault()} onplay={()=>{if(!live){video?.pause();return;}playing=true;}} onpause={()=>{seeking=null;playing=false;stopBoost();}} onended={()=>{playing=false;stopBoost();}} ontimeupdate={()=>{if(live&&video&&video.readyState>0&&seeking===null)position=video.currentTime;}} onerror={()=>{if(live&&video?.getAttribute('src')){playing=false;stopBoost();warn('video_decode_failed');}}}></video>
  <MediaControls title={selected?.title??''} {playing} {loading} failed={!!failure} {position} {duration} bind:seeking seekable={!!resource&&!loading} previousDisabled={episodeIndex()<=0} nextDisabled={episodeIndex()<0||episodeIndex()>=episodes.length-1} onback={back} ontoggle={()=>failure&&selected?void open(selected,current,true):void toggle()} onprevious={()=>changeEpisode(episodeIndex()-1)} onnext={()=>changeEpisode(episodeIndex()+1)} onseek={commitSeek} {volume} {muted} onmute={()=>{muteChosen=true;muted=!muted;}} onvolume={setVolume}>
    {#snippet menus()}
      {#if episodes.length}<select aria-label="选集" title="选集" value={episodeIndex()} disabled={loading} onchange={e=>changeEpisode(Number(e.currentTarget.value))}>{#each episodes as e,index}<option value={index}>{index+1}. {e.title}</option>{/each}</select>{/if}
      <select aria-label="播放倍速" title="播放倍速" value={speed} onchange={e=>{speed=Number(e.currentTarget.value);if(video)video.playbackRate=speed;}}>{#each [.5,.75,1,1.25,1.5,2]as value}<option value={value}>{value}×</option>{/each}</select>
    {/snippet}
  </MediaControls>
 </div></section>
 {:else}<section bind:this={listElement} class="content-width feature-page">{@render tabs()}{#if tab==='search'}<form class="collection-toolbar" onsubmit={e=>{e.preventDefault();void loadList();}}><label class="search-box"><Icon name="search" size={16}/><input aria-label="搜索B站视频" placeholder="搜索视频" maxlength="100" bind:value={query} disabled={busy}/></label><button title="搜索视频" disabled={busy||!query.trim()}>{busy?'搜索中…':'搜索'}</button></form>{:else}<div class="collection-toolbar"><button title={tab==='home'?'换一批推荐':'刷新动态'} onclick={()=>loadList()} disabled={busy}>{busy?'加载中…':tab==='home'?'换一批':'刷新'}</button></div>{/if}<div class="media-list" aria-busy={busy}>{#each items as item(item.id)}<div class="media-result collection-row"><div><h2 class="item-title" title={item.title}>{item.title}</h2><span class="item-description">{item.author} · {item.duration}</span></div><button title={`播放${item.title}`} onclick={()=>open(item)} disabled={busy}><Icon name="play" size={15}/>播放</button></div>{/each}{#if searched&&!items.length}<div class="list-empty">暂无视频</div>{/if}{#if more}<button title="下一页" onclick={()=>loadList(true)} disabled={busy}>下一页</button>{/if}</div></section>{/if}
 {#if failure}<div class="feature-error" role="alert">{failure}</div>{/if}
 {#if ['preparing','pending','scanned'].includes(account.state)}<dialog class="dialog-surface qr-dialog" aria-label="扫码登录B站" use:qrModal oncancel={e=>{e.preventDefault();void cancelLogin();}}><h2>使用哔哩哔哩 App 扫码</h2>{#if account.qr_image}<img src={account.qr_image} alt="B站登录二维码"/>{/if}<button title="取消扫码登录" onclick={cancelLogin}>取消</button></dialog>{/if}
{/if}
<style>.qr-dialog{align-items:center}.qr-dialog img{width:220px;height:220px;object-fit:contain;background:white}.qr-dialog button{align-self:stretch}</style>
