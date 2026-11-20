<script lang="ts">
  import {tick} from 'svelte';import Bilibili from '../src/Bilibili.svelte';
  let active=$state(true),showing=$state(true),settings=$state(false),generation=$state(1),notice=$state('');let component:Bilibili;
  const fixture={confirmLogin:false,failMethod:'',async accountState(state:string){account={...account,state};await fixture.refresh();},calls:[] as any[],delay:0,next:0,jobs:new Map<number,{method:string;args:any}>(),async visibility(value:boolean){await component.suspend();active=value;generation++;await tick();},async section(value:boolean){await component.suspend(true);active=false;await tick();settings=value;generation++;active=true;},async refresh(){active=false;await tick();generation++;active=true;}};
  let account={state:new URLSearchParams(location.search).has('signedOut')?'idle':'signed_in',name:'样例用户',qr_image:'',attempt:0};
  (window as any).fixture=fixture;
  if(!(window as any).bilibiliInvoke){
  (window as any).__TAURI_INTERNALS__={invoke:async(command:string,request:any)=>{
    fixture.calls.push({command,...request});if(command==='bilibili_suspend')return {};
    const {method,args}=request;
    if(method==='task.status'){
      const job=fixture.jobs.get(args.job)!;if(fixture.failMethod===job.method){account={...account,state:'expired'};return {job:args.job,status:'failed',error:'account_expired'};}await new Promise(r=>setTimeout(r,fixture.delay));let result:any;
      if(job.method==='account.restore')result=account;
      else if(job.method==='account.login')result={state:'pending',name:'',qr_image:'data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg"/%3E',attempt:args.job};
      else if(job.method==='account.poll'){if(fixture.confirmLogin){account={state:'signed_in',name:'新账号',qr_image:'',attempt:0};result=account;}else result={state:'pending',name:'',qr_image:'',attempt:job.args.attempt};}
      else if(job.method==='playback.open')result={session:args.job,resource:{url:'http://127.0.0.1:1/fixture',mime:'video/mp4'},title:'样例',bvid:job.args.bvid,cid:job.args.cid??'1',duration:200,entries:{complete:true,items:[{bvid:'BV1oJ41137oE',cid:'1',title:'第一P'},{bvid:'BV1oJ41137oE',cid:'2',title:'第二P'}]}};
      else result={items:[{id:'BV1oJ41137oE',title:`${job.method} ${job.args.query||job.args.fresh||job.args.offset||'1'}`,author:'UP',duration:'3:20'}],has_more:job.method!=='recommend',offset:'2'};
      return {job:args.job,status:'complete',result};
    }
    if(method==='account.get')return account;
    if(method==='account.logout'){account={...account,state:'idle'};return account;}
    if(['account.restore','account.login','account.poll','search','recommend','following','playback.open'].includes(method)){const job=++fixture.next;fixture.jobs.set(job,{method,args});return {job,status:'pending'};}
    return {};
  }};
  }else{(window as any).__TAURI_INTERNALS__={invoke:(window as any).bilibiliInvoke};}
  if(!(window as any).bilibiliInvoke){
  // This fixture checks UI dispatch/lifecycle only, never claims decoded media.
  const get=Element.prototype.getAttribute,remove=Element.prototype.removeAttribute;
  HTMLMediaElement.prototype.getAttribute=function(name){return name==='src'?this.dataset.fixtureSrc??null:get.call(this,name);};
  HTMLMediaElement.prototype.removeAttribute=function(name){if(name==='src')delete this.dataset.fixtureSrc;else remove.call(this,name);};
  Object.defineProperty(HTMLMediaElement.prototype,'src',{get(){return this.dataset.fixtureSrc??'';},set(value){this.dataset.fixtureSrc=value;}});
  Object.defineProperties(HTMLMediaElement.prototype,{readyState:{get:()=>1},duration:{get:()=>200}});
  HTMLMediaElement.prototype.load=function(){if(this.getAttribute('src'))queueMicrotask(()=>this.dispatchEvent(new Event('loadedmetadata')));};
  HTMLMediaElement.prototype.play=async function(){Object.defineProperty(this,'paused',{value:false,configurable:true});this.dispatchEvent(new Event('play'));};
  HTMLMediaElement.prototype.pause=function(){Object.defineProperty(this,'paused',{value:true,configurable:true});this.dispatchEvent(new Event('pause'));};
  }
</script>
<div class="app-shell" data-ready="true"><main class="page"><Bilibili bind:this={component} {active} {showing} {settings} {generation} notice={v=>notice=v} settingsChange={fixture.section}/></main><output>{notice}</output></div>

