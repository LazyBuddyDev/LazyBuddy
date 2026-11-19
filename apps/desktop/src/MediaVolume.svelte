<script lang="ts">
  import Icon from './Icon.svelte';
  let {volume,muted,onmute,onvolume}:{volume:number;muted:boolean;onmute:()=>void;onvolume:(value:number)=>void}=$props();
</script>
<div class="volume-control"><button aria-label={muted?'取消静音':'静音'} title="音量（↑/↓ 调节）" onclick={onmute}><Icon name={muted?'mute':'volume'}/></button><div class="volume-popover"><output aria-hidden="true">{Math.round(volume*100)}%</output><input class="volume" aria-label="音量" aria-orientation="vertical" aria-valuetext={`${Math.round(volume*100)}%`} type="range" min="0" max="1" step=".05" value={volume} oninput={event=>onvolume(Number(event.currentTarget.value))}/></div></div>
