<script lang="ts">
  let {active,label,inline=false,id}:{active:boolean;label:string;inline?:boolean;id?:string}=$props();
</script>
<span class="attention-dot" class:attention-inline={inline} class:attention-unread={active} role="img" aria-label={active?label:undefined} aria-hidden={!active}></span>

{#if id}<span {id} hidden>{label}</span>{/if}
