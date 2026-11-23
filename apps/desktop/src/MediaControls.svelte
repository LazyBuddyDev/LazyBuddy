<script lang="ts">
  import type {Snippet} from 'svelte';
  import Icon from './Icon.svelte';
  import MediaVolume from './MediaVolume.svelte';
  import {formatTime} from './lifecycle';
  let {title,playing,loading,failed,position,duration,seeking=$bindable(null),seekable,previousDisabled,nextDisabled,onback,ontoggle,onprevious,onnext,onseek,volume,muted,onvolume,onmute,menus,headerActions,footerActions}: {
    title:string;playing:boolean;loading:boolean;failed:boolean;position:number;duration:number;seeking?:number|null;seekable:boolean;
    previousDisabled:boolean;nextDisabled:boolean;onback:()=>void;ontoggle:()=>void;onprevious:()=>void;onnext:()=>void;onseek:()=>void;
    volume:number;muted:boolean;onvolume:(value:number)=>void;onmute:()=>void;menus?:Snippet;headerActions?:Snippet;footerActions?:Snippet;
  }=$props();
</script>
<div class="video-top">
  <button class="video-back" aria-label="返回列表" title="返回列表" onclick={onback}><Icon name="arrow" size={17}/></button>
  <span class="video-title" title={title}>{title}</span>
  {#if headerActions}{@render headerActions()}{/if}
</div>
{#if !playing||failed}<button class="center-play" aria-label={loading?'加载中':failed?'重试播放':'播放视频'} title={loading?'加载中':failed?'重试播放':'播放视频'} disabled={loading} onclick={ontoggle}><Icon name={failed?'refresh':'play'} size={28}/></button>{/if}
<div class="video-bottom">
  <input class="seek" aria-label="播放进度" title="播放进度" type="range" min="0" max={duration||1} step=".05" value={seeking??position} disabled={!seekable} oninput={event=>seeking=Number(event.currentTarget.value)} onchange={onseek} onpointercancel={()=>seeking=null} onblur={()=>seeking=null}/>
  <div class="video-controls">
    <button aria-label="上一集" title="上一集" disabled={previousDisabled||loading} onclick={onprevious}><Icon name="previous"/></button>
    <button aria-label={playing?'暂停':'播放'} title={failed?'重试播放':playing?'暂停':'播放'} disabled={loading} onclick={ontoggle}><Icon name={failed?'refresh':playing?'pause':'play'}/></button>
    <button aria-label="下一集" title="下一集" disabled={nextDisabled||loading} onclick={onnext}><Icon name="next"/></button>
    <span class="time">{formatTime(seeking??position)} / {formatTime(duration)}</span><span class="control-spacer"></span>
    {#if menus}{@render menus()}{/if}
    <MediaVolume {volume} {muted} {onmute} {onvolume}/>
  </div>
  {#if footerActions}<div class="video-controls-footer">{@render footerActions()}</div>{/if}
</div>
<style>
  .video-controls-footer{margin-top:4px}
</style>
