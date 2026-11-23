<script lang="ts">
  import {tick} from 'svelte';import Hongguo from '../src/Hongguo.svelte';
  let active=$state(true),showing=$state(true),accountOnly=$state(false);let component:Hongguo;
  const delay=600;const enabled=!new URLSearchParams(location.search).has('baseline');
  let session=0,current=1,readyAt=0,next:number|undefined,warmAt=0;
  const fixture={calls:[] as any[],async visible(value:boolean){active=value;await tick();},async settings(value:boolean){active=false;await tick();accountOnly=value;active=true;},nextReady:()=>next===current+1&&performance.now()>=warmAt};
  (window as any).fixture=fixture;
  const snapshot=()=>({session,series:'1',episode:current,loading:performance.now()<readyAt,prepared:performance.now()>=readyAt,episodes:[1,2,3],available_qualities:[360],danmaku:true,resource:performance.now()>=readyAt?{url:`/tests/prefetch-fixture.mp4?episode=${current}`,mime:'video/mp4'}:null});
  (window as any).__TAURI_INTERNALS__={invoke:async(command:string,request:any)=>{
    const {method,args}=request;fixture.calls.push({method,args,at:performance.now()});
    if(method==='account.restore')return {state:'signed_in',name:'样例'};
    if(method==='progress.read')return {history:{}};
    if(method==='progress.sync')return {state:'signed_in',name:'样例',history:{},operation:null};
    if(method==='favorites.read')return {state:'signed_in',favorites:[{id:'1',title:'连播样例',episodes:3}],operation:null};
    if(method==='playback.open'){
      const reused=enabled&&next===args.episode;readyAt=reused?warmAt:performance.now()+delay;current=args.episode;session++;next=undefined;return snapshot();
    }
    if(method==='playback.prefetch'){if(enabled&&next===undefined){next=current+1;warmAt=performance.now()+delay;}return snapshot();}
    if(method==='playback.prefetch_cancel'||method==='playback.suspend'||method==='playback.close'){next=undefined;return snapshot();}
    if(method==='playback.status'||method==='playback.checkpoint'||method==='playback.resume')return snapshot();
    if(method==='danmaku.read'){
      const text=`第${current}集弹幕`;await new Promise(resolve=>setTimeout(resolve,150));
      return {items:[{text,offset_ms:0},{text:'稍后出现',offset_ms:500}]};
    }
    return {};
  }};
</script>
<div class="app-shell"><Hongguo bind:this={component} {active} {showing} {accountOnly} notice={()=>{}} settingsChange={fixture.settings}/></div>
