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
  let {active,title,load}:{active:boolean;title:string;load:()=>Promise<CoverData>}=$props();
  let container:HTMLDivElement;let canvas:HTMLCanvasElement;let loaded=$state(false);let generation=0;
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
        const pixels=coverPixels(data);if(!pixels)return;
        canvas.width=data.width;canvas.height=data.height;
        canvas.getContext('2d')?.putImageData(new ImageData(pixels,data.width,data.height),0,0);loaded=true;
        }catch{}
      });
    });
    observer.observe(container);
    return()=>{generation++;observer.disconnect();};
  }
  $effect(()=>{if(active)return untrack(observe);});
</script>
<div class="cover-image" bind:this={container}><canvas bind:this={canvas} hidden={!loaded} aria-label={`${title}封面`}></canvas>{#if !loaded}<span>暂无封面</span>{/if}</div>
