<script module lang="ts">
  const queue:Array<()=>Promise<void>>=[];let running=false;
  async function enqueue(work:()=>Promise<void>){
    queue.push(work);if(running)return;running=true;
    try{while(queue.length)await queue.shift()!();}finally{running=false;}
  }
</script>
<script lang="ts">
  import {untrack} from 'svelte';
  import {coverPixels,type CoverData} from './hongguo';
  let {active,title,load,cached}:{active:boolean;title:string;load:()=>Promise<CoverData>;cached?:CoverData}=$props();
  let container:HTMLDivElement;let canvas:HTMLCanvasElement;let loaded=$state(false);let failed=$state(false);let generation=0;
  function paint(data:CoverData){
    const pixels=coverPixels(data);if(!pixels)return false;
    canvas.width=data.width;canvas.height=data.height;
    canvas.getContext('2d')?.putImageData(new ImageData(pixels,data.width,data.height),0,0);loaded=true;return true;
  }
  function observe(){
    const ticket=++generation;
    if(loaded)return;
    const observer=new IntersectionObserver(entries=>{
      if(!entries.some(entry=>entry.isIntersecting)||ticket!==generation)return;
      observer.disconnect();
      void enqueue(async()=>{
        if(ticket!==generation)return;
        try{const data=await load();
        if(ticket!==generation)return;
        failed=!paint(data);
        }catch{if(ticket===generation)failed=true;}
      });
    });
    observer.observe(container);
    return()=>{generation++;observer.disconnect();};
  }
  $effect(()=>{if(active)return untrack(()=>{if(cached&&paint(cached))return;return observe();});});
</script>
<div class="cover-image" bind:this={container}><canvas bind:this={canvas} hidden={!loaded} aria-label={`${title}封面`}></canvas>{#if !loaded}<span>{failed||!active?'暂无封面':'封面加载中…'}</span>{/if}</div>
