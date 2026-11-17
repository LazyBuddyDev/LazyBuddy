<script lang="ts">
  import {invoke} from '@tauri-apps/api/core';
  import {tick,untrack,onDestroy} from 'svelte';
  import Icon from './Icon.svelte';
  import {pageSize,pageStart,progressLabel,readerError,matchOffsets,wrapMatch,markedRuns,visibleAnchor,restoreAnchor,viewportStep,pageOffset,type Position,type Entry,type Book,type Block,type ReadingPreferences} from './reader';
  let {active,showing,settings,generation,notice,settingsChange}:{active:boolean;showing:boolean;settings:boolean;generation:number;notice:(text:string)=>void;settingsChange:(settings:boolean)=>void}=$props();
  let entries=$state<Entry[]>([]),book=$state<Book>(),blocks=$state<Block[]>([]);
  let preferences=$state<ReadingPreferences>({font_size:20,line_height:1.9,width:680,page_mode:'horizontal'});
  let chapter=$state(0),start=$state(0),total=$state(0),filter=$state(''),query=$state(''),encoding=$state('auto');
  let loading=$state(false),importing=$state(false),saving=$state(false),directory=$state(false),removing=$state<string>();
  let failure=$state(''),images=$state<Record<string,string>>({});
  let scroll=$state<HTMLElement>();let epoch=0;let initialized=false;let needsList=true;let lastBook=$state<string|null>(null);
  let currentPosition:Position|undefined;let restoring=false;let timer:ReturnType<typeof setTimeout>|undefined;
  let stagedPosition:Position|undefined;let checkpointWrites:Promise<unknown>=Promise.resolve();
  let suspended=true;let turning=$state(false),extending=$state(false),offset=$state(0),extent=$state(0);
  let finishTurn:(()=>void)|undefined;let swapping=false;
  const horizontal=$derived(preferences.page_mode==='horizontal');
  function needsMore(){return !!scroll&&start+blocks.length<total&&extent-offset<(horizontal?scroll.getBoundingClientRect().width:scroll.clientHeight)*2;}
  function metrics(){if(scroll){offset=horizontal?scroll.scrollLeft:scroll.scrollTop;extent=horizontal?scroll.scrollWidth-scroll.getBoundingClientRect().width:scroll.scrollHeight-scroll.clientHeight;}}
  let searchOpen=$state(false),searchQuery=$state(''),matchCurrent=$state(-1),matchTotal=$state(0);
  let searchBlocks:{id:string;index:number;count:number}[]=[];let searchRun=0;
  let selectedMatch=$state<{id:string;offset:number}>();
  function clearSearch(){searchRun++;searchQuery='';searchBlocks=[];matchCurrent=-1;matchTotal=0;selectedMatch=undefined;}
  async function toggleDirectory(){if(loading||turning||extending)return;await checkpoint();directory=!directory;await tick();if(scroll&&currentPosition)restoreAnchor(scroll,currentPosition);}

  let importRun=0;let waitTimer:ReturnType<typeof setTimeout>|undefined;let wake:(()=>void)|undefined;
  let observer:IntersectionObserver|undefined;let resourceQueue:string[]=[];let readingImages=false;
  const filtered=$derived(entries.filter(e=>e.title.toLocaleLowerCase().includes(filter.toLocaleLowerCase())));
  const lease=()=>({epoch,generation,cancelled:false});
  const valid=(ticket:ReturnType<typeof lease>)=>!ticket.cancelled&&active&&ticket.epoch===epoch&&ticket.generation===generation;
  async function call<T>(method:string,args:Record<string,unknown>={},ticket=lease()):Promise<T>{
    if(!valid(ticket))throw Error('inactive_reader_page');
    const result=await invoke<T>('reader_call',{method,args,generation:ticket.generation});
    if(!valid(ticket))throw Error('stale_reader_reply');return result;
  }
  function report(error:unknown,ticket=lease()){if(valid(ticket)){failure=readerError(error);notice(failure);}}
  async function list(ticket=lease()){
    const all:Entry[]=[];let offset=0;
    while(valid(ticket)){
      const reply=await call<{entries:Entry[];total:number;last_book:string|null}>('library.list',{start:offset,limit:100},ticket);
      all.push(...reply.entries);offset+=reply.entries.length;lastBook=reply.last_book;
      if(offset>=reply.total)break;if(!reply.entries.length)throw Error('invalid_reader_reply');
    }
    if(valid(ticket)){entries=all.sort((a,b)=>b.updated_ms-a.updated_ms);needsList=false;}
  }
  async function activate(){
    suspended=false;
    const ticket=lease();failure='';
    try{
      preferences=await call<ReadingPreferences>('preferences.get',{},ticket);
      if(!settings){if(needsList||!initialized)await list(ticket);if(book){await tick();if(currentPosition)if(scroll)restoreAnchor(scroll,currentPosition);metrics();if(needsMore())await extend(1,ticket);watchImages();}}
      initialized=true;
    }catch(error){report(error,ticket);}
  }
  function stopLocal(){
    suspended=true;finishTurn?.();if(scroll)scroll.scrollTo({left:scroll.scrollLeft,top:scroll.scrollTop,behavior:'instant'});turning=false;extending=false;restoring=false;swapping=false;
    epoch++;searchRun++;importRun++;clearTimeout(timer);timer=undefined;clearTimeout(waitTimer);wake?.();wake=undefined;
    observer?.disconnect();observer=undefined;resourceQueue=[];images={};readingImages=false;importing=false;loading=false;saving=false;
  }
  export async function suspend(capture=true){
    if(capture&&active&&!settings)await checkpoint();
    stopLocal();needsList=true;
    await invoke('reader_suspend');
  }
  export function resume(){if(active&&suspended)void activate();}
  $effect(()=>{const enabled=active;const gen=generation;const prefs=settings;untrack(()=>{stopLocal();if(enabled)void activate();});});
  onDestroy(()=>stopLocal());
  async function checkpoint(){
    if(!active||settings||!book||!scroll||restoring||swapping||!blocks.length)return;
    const anchor=visibleAnchor(scroll);if(!anchor)return;
    const position:Position={book_id:book.id,content_version:book.content_version,chapter_id:book.chapters[chapter].id,...anchor};
    currentPosition=position;
    if(JSON.stringify(position)===JSON.stringify(stagedPosition)){await checkpointWrites;return;}
    stagedPosition=position;const ownerGeneration=generation;
    const entry=entries.find(e=>e.id===position.book_id);if(entry)entry.position=position;
    const write=checkpointWrites.catch(()=>{}).then(()=>invoke('reader_checkpoint',{position,generation:ownerGeneration}));checkpointWrites=write;
    try{await write;}catch(error){if(stagedPosition===position)stagedPosition=undefined;throw error;}
  }
  function onScroll(){
    if(!active||settings||suspended||restoring||turning)return;metrics();
    if(!loading&&!extending&&needsMore())void extend(1).catch(error=>report(error));
    const ticket=lease();void checkpoint().catch(error=>report(error,ticket));
    clearTimeout(timer);timer=setTimeout(()=>{timer=undefined;void call('progress.flush',{},ticket).catch(error=>report(error,ticket));},600);
  }
  async function open(entry:Entry){
    if(loading||!active)return;const ticket=lease();loading=true;failure='';
    try{
      await checkpoint();await call('progress.flush',{},ticket);
      const result=await call<{book:Book;position:Position|null}>('book.open',{book_id:entry.id},ticket);
      book=result.book;currentPosition=result.position??undefined;query='';clearSearch();
      chapter=Math.max(0,book.chapters.findIndex(c=>c.id===result.position?.chapter_id));
      await load(chapter,result.position?pageStart(result.position.block_id):0,result.position??undefined,ticket);
      if(book.warnings.length)notice(book.warnings.join('；'));
    }catch(error){report(error,ticket);}finally{if(valid(ticket))loading=false;}
  }
  async function load(nextChapter:number,nextStart=0,position?:Position,ticket=lease()){
    if(!book)return;
    observer?.disconnect();resourceQueue=[];images={};
    const result=await call<{blocks:Block[];start:number;total:number}>('chapter.read',{book_id:book.id,chapter_id:book.chapters[nextChapter].id,start:nextStart},ticket);
    restoring=true;blocks=result.blocks;chapter=nextChapter;start=result.start;total=result.total;
    await tick();if(!valid(ticket))return;
    if(!scroll){restoring=false;return;}scroll.scrollTop=0;scroll.scrollLeft=0;if(position)restoreAnchor(scroll,position);restoring=false;
    metrics();if(needsMore())await extend(1,ticket);if(!valid(ticket))return;await checkpoint();watchImages();
  }
  async function move(nextChapter:number,nextStart=0,ticket=lease()){
    if(!book||loading||nextChapter<0||nextChapter>=book.chapters.length)return;
    if(nextChapter!==chapter)clearSearch();
    loading=true;failure='';
    try{await checkpoint();await call('progress.flush',{},ticket);await load(nextChapter,nextStart,undefined,ticket);}catch(error){report(error,ticket);}finally{if(valid(ticket))loading=false;}
  }
  async function back(){
    if(loading||turning||extending)return;
    const ticket=lease();try{await checkpoint();await call('progress.flush',{},ticket);book=undefined;blocks=[];observer?.disconnect();images={};needsList=true;await list(ticket);}catch(error){report(error,ticket);}
  }
  async function extend(direction:number,ticket=lease()){
    if(!book||!scroll||extending)return;
    const next=direction>0?start+blocks.length:Math.max(0,start-pageSize);
    if(direction>0?next>=total:start===0)return;
    extending=true;
    try{
      const result=await call<{blocks:Block[];start:number}>('chapter.read',{book_id:book.id,chapter_id:book.chapters[chapter].id,start:next},ticket);
      if(!result.blocks.length)throw Error('invalid_reader_reply');
      const anchor=visibleAnchor(scroll);restoring=true;
      let combined=direction>0?[...blocks,...result.blocks]:[...result.blocks,...blocks];
      if(direction<0)start=result.start;
      if(combined.length>pageSize*3){
        if(direction>0){start+=pageSize;combined=combined.slice(pageSize);}else combined=combined.slice(0,pageSize*3);
      }
      blocks=combined;resourceQueue=[];
      const resources=new Set(blocks.map(b=>b.resource));images=Object.fromEntries(Object.entries(images).filter(([id])=>resources.has(id)));
      await tick();if(!valid(ticket))return;
      if(anchor)restoreAnchor(scroll,anchor);metrics();watchImages();
    }finally{if(valid(ticket)){restoring=false;extending=false;}}
  }
  async function exchangePage(direction:number,ticket:ReturnType<typeof lease>,change:()=>Promise<boolean|void>){
    if(!scroll)return false;
    const element=scroll,frame=element.parentElement!;
    const old={chapter,start,total,blocks,images,currentPosition,x:element.scrollLeft,y:element.scrollTop};
    const ghost=element.cloneNode(true) as HTMLElement;
    ghost.classList.remove('reader-scroll');ghost.setAttribute('aria-hidden','true');ghost.inert=true;
    ghost.querySelectorAll('[data-block],[data-resource]').forEach(node=>{node.removeAttribute('data-block');node.removeAttribute('data-resource');});
    Object.assign(ghost.style,{position:'absolute',inset:'0',width:'100%',height:'100%',overflow:'hidden',pointerEvents:'none',containerType:'size',scrollbarGutter:getComputedStyle(element).scrollbarGutter,background:getComputedStyle(element).background});
    ghost.dataset.readerSnapshot='true';frame.append(ghost);ghost.scrollLeft=old.x;ghost.scrollTop=old.y;
    let done=false,cancelled=false;const animations:Animation[]=[];swapping=true;element.style.opacity='0';
    const finish=(rollback:boolean)=>{
      if(done)return;done=true;cancelled=rollback;animations.forEach(animation=>animation.cancel());ghost.remove();element.style.opacity='';
      if(finishTurn===cancel)finishTurn=undefined;
      if(rollback){ticket.cancelled=true;turning=false;loading=false;extending=false;restoring=false;resourceQueue=[];observer?.disconnect();chapter=old.chapter;start=old.start;total=old.total;blocks=old.blocks;images=old.images;currentPosition=old.currentPosition;
        const restored=lease();void tick().then(()=>{if(valid(restored)&&scroll===element&&!turning&&!loading){if(old.currentPosition)restoreAnchor(element,old.currentPosition);else{element.scrollLeft=old.x;element.scrollTop=old.y;}metrics();watchImages();}});
      }
      swapping=false;
    };
    const cancel=()=>finish(true);finishTurn=cancel;
    try{
      const changed=await change();if(changed===false||cancelled||!valid(ticket))return false;
      await tick();if(cancelled||!valid(ticket))return false;
      element.style.opacity='';
      if(preferences.page_mode!=='scroll'&&!matchMedia('(prefers-reduced-motion: reduce)').matches){
        const axis=horizontal?'X':'Y',distance=horizontal?element.getBoundingClientRect().width:element.clientHeight;
        const timing={duration:240,easing:'cubic-bezier(.2,.8,.2,1)',fill:'both' as FillMode};
        animations.push(ghost.animate([{transform:`translate${axis}(0)`},{transform:`translate${axis}(${-direction*distance}px)`}],timing));
        animations.push(element.animate([{transform:`translate${axis}(${direction*distance}px)`},{transform:`translate${axis}(0)`}],timing));
        await Promise.allSettled(animations.map(animation=>animation.finished));
      }
      if(cancelled||!valid(ticket))return false;finish(false);return true;
    }finally{if(!done)finish(true);}
  }
  async function turn(direction:number){
    if(!active||settings||suspended||!book||!scroll||loading||turning||extending)return;
    const ticket=lease();turning=true;
    try{
      await checkpoint();if(!valid(ticket)||!scroll)return;
      let step=horizontal?scroll.getBoundingClientRect().width:viewportStep(scroll.clientHeight,preferences.font_size*preferences.line_height);
      metrics();
      if(direction>0&&extent-offset<step*2&&start+blocks.length<total||direction<0&&offset<step&&start>0)await extend(direction,ticket);
      if(!valid(ticket))return;metrics();step=horizontal?scroll.getBoundingClientRect().width:viewportStep(scroll.clientHeight,preferences.font_size*preferences.line_height);
      if(direction>0&&offset>=extent-1||direction<0&&offset<=1){
        const next=chapter+direction;if(next<0||next>=book.chapters.length)return;
        const blockStart=direction>0?0:Math.floor((book.chapters[next].blocks-1)/pageSize)*pageSize;
        const changed=await exchangePage(direction,ticket,async()=>{
          await move(next,blockStart,ticket);if(!valid(ticket)||chapter!==next||!scroll)return false;
          if(direction<0){metrics();scroll.scrollTo({left:horizontal?extent:0,top:horizontal?0:extent,behavior:'instant'});}
        });if(!changed)return;
      }else{
        restoring=true;const target=pageOffset(offset,step,extent,direction);
        const changed=await exchangePage(direction,ticket,async()=>{scroll!.scrollTo({left:horizontal?target:0,top:horizontal?0:target,behavior:'instant'});});
        if(!changed)return;restoring=false;
      }
      metrics();await checkpoint();if(needsMore())await extend(1,ticket);
    }catch(error){report(error,ticket);}finally{if(valid(ticket)){turning=false;restoring=false;}}
  }
  function readingKey(event:KeyboardEvent){
    if(!active||settings||!book||event.defaultPrevented)return;
    if(event.ctrlKey||event.altKey||event.metaKey||event.shiftKey)return;
    if(event.target instanceof Element){
      if(event.target.closest('input,textarea,select,[contenteditable]:not([contenteditable="false"]),[role="textbox"]'))return;
      if(event.target.closest('button')&&!event.target.closest('.reader-navigation'))return;
    }
    if(event.key==='ArrowLeft'||event.key==='ArrowRight'){event.preventDefault();void turn(event.key==='ArrowRight'?1:-1);}
  }
  function wheel(event:WheelEvent){
    if(preferences.page_mode==='scroll'||event.ctrlKey||!active||settings||suspended)return;
    event.preventDefault();if(Math.abs(event.deltaY||event.deltaX)>8)void turn((event.deltaY||event.deltaX)>0?1:-1);
  }
  function wheelInput(element:HTMLElement){element.addEventListener('wheel',wheel,{passive:false});return{destroy:()=>element.removeEventListener('wheel',wheel)};}
  $effect(()=>{
    const element=scroll;if(!element||!active||settings)return;
    let size=element.getBoundingClientRect();
    const resize=new ResizeObserver(()=>{
      const next=element.getBoundingClientRect();if(next.width===size.width&&next.height===size.height)return;size=next;
      if(!active||suspended)return;
      if(turning&&finishTurn){finishTurn();return;}
      if(restoring)return;restoring=true;if(currentPosition)restoreAnchor(element,currentPosition);metrics();restoring=false;
    });resize.observe(element);return()=>resize.disconnect();
  });
  function watchImages(){
    observer?.disconnect();if(!active||settings||!scroll)return;
    const ticket=lease();
    observer=new IntersectionObserver(rows=>{
      if(!valid(ticket))return;
      for(const row of rows)if(row.isIntersecting){const id=(row.target as HTMLElement).dataset.resource;if(id&&!images[id]&&!resourceQueue.includes(id))resourceQueue.push(id);observer?.unobserve(row.target);}
      void drainImages(ticket);
    },{root:scroll});
    scroll.querySelectorAll('[data-resource]').forEach(element=>observer!.observe(element));
  }
  async function drainImages(ticket:ReturnType<typeof lease>){
    if(readingImages)return;readingImages=true;const owner=book?.id;
    try{while(resourceQueue.length&&valid(ticket)&&owner===book?.id){const id=resourceQueue.shift()!;try{const result=await call<{url:string}>('resource.read',{book_id:owner,resource_id:id},ticket);if(owner===book?.id&&blocks.some(b=>b.resource===id))images[id]=result.url;}catch(error){report(error,ticket);}}}finally{if(valid(ticket))readingImages=false;}
  }
  async function find(direction=0){
    if(!book||!query.trim()||loading||turning||extending)return;
    const ticket=lease();const run=++searchRun;loading=true;failure='';
    const needle=query.trim();
    try{
      if(searchQuery!==needle){
        const found:{id:string;index:number;count:number}[]=[];let count=0;
        for(let offset=0;offset<book.chapters[chapter].blocks;offset+=pageSize){
          const page=await call<{blocks:Block[]}>('chapter.read',{book_id:book.id,chapter_id:book.chapters[chapter].id,start:offset},ticket);
          if(run!==searchRun)return;
          page.blocks.forEach((block,index)=>{if(block.kind==='image')return;const hits=matchOffsets(block.text,needle).length;if(hits){found.push({id:block.id,index:offset+index,count:hits});count+=hits;}});
        }
        searchBlocks=found;matchTotal=count;matchCurrent=-1;searchQuery=needle;
      }
      if(!matchTotal){notice('本章未找到匹配内容');return;}
      const next=wrapMatch(matchCurrent<0?(direction<0?-1:0):matchCurrent+direction,matchTotal);
      let ordinal=next;const block=searchBlocks.find(item=>{if(ordinal<item.count)return true;ordinal-=item.count;return false;})!;
      const page=await call<{blocks:Block[]}>('chapter.read',{book_id:book.id,chapter_id:book.chapters[chapter].id,start:pageStart(block.id)},ticket);
      if(run!==searchRun)return;
      const text=page.blocks.find(b=>b.id===block.id)!.text;
      const offset=matchOffsets(text,needle)[ordinal];
      matchCurrent=next;selectedMatch={id:block.id,offset};
      const target:Position={book_id:book.id,content_version:book.content_version,chapter_id:book.chapters[chapter].id,block_id:block.id,offset:Array.from(text.slice(0,offset)).length};
      await load(chapter,pageStart(block.id),target,ticket);
    }catch(error){report(error,ticket);}finally{if(valid(ticket))loading=false;}
  }
  async function setPreference<K extends keyof ReadingPreferences>(key:K,value:ReadingPreferences[K]){
    if(saving||!active)return;const ticket=lease();saving=true;
    try{preferences=await call<ReadingPreferences>('preferences.set',{...preferences,[key]:value},ticket);}catch(error){report(error,ticket);}finally{if(valid(ticket))saving=false;}
  }
  const delay=()=>new Promise<void>(resolve=>{wake=resolve;waitTimer=setTimeout(()=>{wake=undefined;resolve();},150);});
  async function importBooks(){
    if(importing)return;const ticket=lease();const run=++importRun;importing=true;failure='';
    const results:string[]=[];let imported=0,duplicates=0,failed=0;
    try{
      const paths=await invoke<string[]>('reader_pick',{generation:ticket.generation});
      if(!valid(ticket)||run!==importRun)return;
      for(const path of paths){
        if(!valid(ticket)||run!==importRun)break;
        const name=path.split(/[\\/]/).pop()??'文件';
        notice(`正在导入：${name}`);
        const job=await call<{job:number}>('library.import',{path,encoding},ticket);
        let status:{status:string;error?:string;result?:{duplicate:boolean;warnings?:string[]}};
        do{await delay();if(!valid(ticket)||run!==importRun)return;status=await call('import.status',{job:job.job},ticket);}while(status.status==='pending');
        if(status.status==='complete'){if(status.result?.duplicate)duplicates++;else imported++;}else failed++;
        const message=status.status==='complete'?(status.result?.duplicate?'已在书库':'已导入'):(status.error?readerError(status.error):'已取消');
        results.push(`${name}：${message}`);
        needsList=true;await list(ticket);
      }
      if(results.length)notice([`导入完成：成功 ${imported} 本，已存在 ${duplicates} 本，失败 ${failed} 本`,...results].join('\n'));
    }catch(error){if(valid(ticket)){failure=readerError(error);notice([`导入未完成：${failure}`,...results].join('\n'));}}finally{if(valid(ticket)&&run===importRun)importing=false;}
  }
  async function remove(entry:Entry){
    const ticket=lease();try{await call('library.remove',{book_id:entry.id},ticket);removing=undefined;await list(ticket);}catch(error){report(error,ticket);if(String(error)==='removal_cleanup_pending'){removing=undefined;await list(ticket);}}
  }
</script>
<svelte:window onkeydown={readingKey}/>

<section hidden={!showing} class="reader-page feature-page" class:reader-content={!settings} data-testid="reader" data-active={active}>
<div class="tab-bar"><div class="tabs" aria-label="小说页签"><button class:active={!settings} aria-pressed={!settings} title="书架" onclick={()=>settingsChange(false)}>书架</button><button class:active={settings} aria-pressed={settings} title="阅读设置" onclick={()=>settingsChange(true)}>设置</button></div></div>
{#if settings}
  <section><div class="settings-content"><h1>阅读设置</h1>
    <div class="setting-row"><label for="reader-page-mode">翻页模式</label><select id="reader-page-mode" disabled={saving} value={preferences.page_mode} onchange={e=>setPreference('page_mode',e.currentTarget.value as ReadingPreferences['page_mode'])}><option value="horizontal">左右翻页</option><option value="vertical">上下翻页</option><option value="scroll">连续滚动</option></select></div>
    <div class="setting-row"><label for="reader-font">正文字号</label><select id="reader-font" disabled={saving} value={preferences.font_size} onchange={e=>setPreference('font_size',Number(e.currentTarget.value))}>{#each [14,16,18,20,22,24,28,32,36] as size}<option value={size}>{size}</option>{/each}</select></div>
    <div class="setting-row"><label for="reader-height">行距</label><select id="reader-height" disabled={saving} value={Math.round(preferences.line_height*10)/10} onchange={e=>setPreference('line_height',Number(e.currentTarget.value))}>{#each [1.3,1.5,1.7,1.9,2.1,2.3,2.5] as height}<option value={height}>{height}</option>{/each}</select></div>
    <div class="setting-row"><label for="reader-width">正文宽度</label><select id="reader-width" disabled={saving} value={preferences.width} onchange={e=>setPreference('width',Number(e.currentTarget.value))}>{#each [320,440,560,680,800,1000] as width}<option value={width}>{width}</option>{/each}</select></div>
  </div></section>
{:else if book}
  <div class="collection-toolbar"><button title="返回书库" onclick={back} aria-disabled={loading||turning||extending}><Icon name="arrow" size={16}/>书库</button><strong class="reader-title" title={book.title}>{book.title}</strong><button title="展开或收起查找栏" aria-label="章内查找" aria-expanded={searchOpen} aria-disabled={loading||turning||extending} onclick={()=>{if(!loading&&!turning&&!extending)searchOpen=!searchOpen;}}><Icon name="search" size={16}/></button><button title="展开或收起章节目录" aria-expanded={directory} aria-disabled={loading||turning||extending} onclick={toggleDirectory}>目录</button></div>
  {#if searchOpen}<form class="collection-toolbar" onsubmit={e=>{e.preventDefault();void find(1);}}><label class="search-box"><Icon name="search" size={16}/><input aria-label="查找本章内容" placeholder="查找本章内容" maxlength="200" bind:value={query} oninput={clearSearch}/></label><output class="reader-match-count" aria-label="查找匹配位置" aria-live="polite">{matchCurrent+1}/{matchTotal}</output><button type="button" title="查找上一处" disabled={!query.trim()} aria-disabled={loading||turning||extending||!query.trim()} onclick={()=>find(-1)}>上一处</button><button title="查找下一处" disabled={!query.trim()} aria-disabled={loading||turning||extending||!query.trim()}>下一处</button></form>{/if}
  <div class="reader-spread">
  {#if directory}<nav class="reader-directory" aria-label="章节目录">{#each book.chapters as item,index}<button class:chosen={chapter===index} aria-current={chapter===index?'page':undefined} title={item.title} aria-disabled={loading||turning||extending} onclick={()=>{if(!turning&&!extending)void move(index);}}>{item.title}</button>{/each}</nav>{/if}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (The scroll region needs focus for keyboard reading.) -->
  <div class="reader-viewport"><div class="reader-scroll focus-inset" bind:this={scroll} data-page-mode={preferences.page_mode} class:horizontal onscroll={onScroll} use:wheelInput role="region" aria-label="小说正文" aria-busy={loading||turning||extending} tabindex="0" style:--reader-font={`${preferences.font_size}px`} style:--reader-height={preferences.line_height} style:--reader-width={`${preferences.width}px`}>
    <article class="reader-body" class:horizontal><h2>{book.chapters[chapter]?.title}</h2>
    {#each blocks as block (block.id)}
      {#if block.kind==='image'}<figure data-block={block.id} data-kind="image" data-resource={block.resource??undefined}>{#if block.resource&&images[block.resource]}<img src={images[block.resource]} alt={block.text} onload={()=>{if(currentPosition&&!restoring&&!turning&&!swapping&&scroll)restoreAnchor(scroll,currentPosition);}}/>{:else}<figcaption>{block.text||'插图'}</figcaption>{/if}</figure>
      {:else}<div class:reader-heading={block.kind==='heading'} class:reader-pre={block.kind==='pre'} class="reader-paragraph" data-block={block.id} data-kind={block.kind}>
        {#each markedRuns(block,searchOpen?searchQuery:'',selectedMatch?.id===block.id?selectedMatch.offset:-1) as part}<span class:reader-strong={part.strong} class:reader-emphasis={part.emphasis}>{#if part.match}<mark class:current-match={part.current} aria-current={part.current?'true':undefined}>{part.text}</mark>{:else}{part.text}{/if}</span>{/each}
      </div>{/if}
    {/each}

    </article>
  </div></div>
  </div>
  <div class="reader-navigation row-actions"><button title="上一页（←）" disabled={chapter===0&&start===0&&offset<=1} aria-disabled={loading||turning||extending||chapter===0&&start===0&&offset<=1} onclick={()=>turn(-1)}>上一页</button><span>{chapter+1} / {book.chapters.length} 章</span><button title="下一页（→）" disabled={chapter===book.chapters.length-1&&start+blocks.length>=total&&offset>=extent-1} aria-disabled={loading||turning||extending||chapter===book.chapters.length-1&&start+blocks.length>=total&&offset>=extent-1} onclick={()=>turn(1)}>下一页</button></div>
{:else}
  <div class="collection-toolbar"><label class="search-box"><Icon name="search" size={16}/><input aria-label="筛选书名" placeholder="筛选书名" bind:value={filter}/></label><button title="导入TXT或EPUB文件" onclick={importBooks} disabled={importing}>导入小说</button></div>
  <label class="collection-options">TXT编码<select aria-label="TXT编码" bind:value={encoding} disabled={importing}>{#each [['auto','自动识别'],['utf-8','UTF-8'],['gb18030','GB18030 / GBK'],['big5','Big5'],['utf-16le','UTF-16 LE'],['utf-16be','UTF-16 BE']] as [value,label]}<option {value}>{label}</option>{/each}</select></label>
  {#if !entries.length}<div class="list-empty">{failure?'书库暂未载入':'导入TXT或EPUB，开始阅读'}{#if failure}<button title="重新读取书库" onclick={()=>{needsList=true;void activate();}}>重试</button>{/if}</div>
  {:else}<div class="reader-library">{#each filtered as entry (entry.id)}<div class="reader-book collection-row"><button class="reader-book-open" aria-label={`继续阅读《${entry.title}》`} title={`继续阅读《${entry.title}》`} disabled={loading} onclick={()=>open(entry)}><strong class="item-title">{entry.title}</strong><span class="item-description">{entry.author?`${entry.author} · `:''}{entry.source.format.toUpperCase()} · {progressLabel(entry)}{lastBook===entry.id?' · 最近阅读':''}</span></button>
    {#if removing===entry.id}<div class="reader-remove row-actions"><span>移除书库副本和阅读记录？原文件保留。</span><button title="确认移除书籍" onclick={()=>remove(entry)}>确认移除</button><button title="取消移除书籍" onclick={()=>removing=undefined}>取消</button></div>{:else}<div class="row-actions"><button aria-label={`移除《${entry.title}》`} title={`移除《${entry.title}》`} onclick={()=>removing=entry.id}>移除</button></div>{/if}</div>{/each}{#if !filtered.length}<div class="list-empty">没有匹配的书籍</div>{/if}</div>{/if}
{/if}
</section>

<style>
  .reader-content{display:flex;flex-direction:column;min-height:0;height:100%}.reader-page[hidden]{display:none}.reader-title{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.reader-library{overflow:auto;scrollbar-gutter:stable;min-height:0}.reader-book-open{flex:1;min-width:0;display:flex;flex-direction:column;align-items:flex-start;text-align:left;border:0;padding:0;min-height:0}.reader-remove{flex-wrap:wrap;justify-content:flex-end;font-size:11px}.reader-remove span{width:100%;text-align:right}.reader-scroll{min-width:0;overflow:auto;min-height:0;flex:1;scrollbar-gutter:stable;background:var(--surface);overflow-anchor:none}.reader-body{--reader-padding:24px;max-width:var(--reader-width);margin:auto;padding:20px 24px 28px;font-size:var(--reader-font);line-height:var(--reader-height);overflow-wrap:anywhere}.reader-body h2{font-size:1.15em;margin-bottom:1.5em}.reader-paragraph{margin:0 0 1em;white-space:pre-wrap}.reader-heading{font-weight:600;font-size:1.1em}.reader-pre{font-family:monospace}.reader-strong{font-weight:700}.reader-emphasis{font-style:italic}figure{margin:1em 0;min-height:80px}figure img{display:block;max-width:100%;height:auto;margin:auto}figcaption{color:var(--muted);font-size:13px}.reader-navigation{display:flex;align-items:center;justify-content:space-between;font-size:12px;flex-shrink:0;margin-top:12px}@media(max-width:520px){.reader-body{--reader-padding:16px;padding:16px}.reader-remove{max-width:160px}.reader-navigation{font-size:11px}}
  .reader-viewport{position:relative;display:flex;flex:1;min-width:0;min-height:0;overflow:hidden}
  .reader-spread{display:flex;flex:1;min-height:0;gap:12px}.reader-directory{position:relative;flex:0 0 clamp(110px,28%,210px);overflow-y:auto;scrollbar-gutter:stable;min-height:0}.reader-directory button{display:block;width:100%;text-align:left;border:0;padding:9px 10px;overflow-wrap:anywhere}.reader-directory .chosen{background:var(--selected);font-weight:600;box-shadow:inset 3px 0 var(--accent)}.reader-match-count{flex-shrink:0;min-width:3ch;text-align:center;font-variant-numeric:tabular-nums}.reader-body mark{background:#f8e599;color:#302400}.reader-body mark.current-match{background:#c84913;color:#fff;outline:2px solid #702400;outline-offset:1px}

  .reader-scroll.horizontal{overflow:hidden;scrollbar-gutter:auto}
  .reader-body.horizontal{width:100%;max-width:none;height:100%;margin:0;padding:16px 0;column-count:1;column-gap:0;column-fill:auto}
  .reader-body.horizontal > *{margin-left:max(var(--reader-padding),calc((100% - var(--reader-width))/2 + var(--reader-padding)));margin-right:max(var(--reader-padding),calc((100% - var(--reader-width))/2 + var(--reader-padding)))}
  .reader-body.horizontal figure{break-inside:avoid;min-height:0;margin-top:0;margin-bottom:1em}
  .reader-body.horizontal img{max-height:calc(100cqh - 80px);object-fit:contain}
  .reader-scroll{container-type:size}
</style>
