<script lang="ts">
  let {checked,label,thumbText,disabled=false,onchange}:{checked:boolean;label:string;thumbText?:string;disabled?:boolean;onchange:(value:boolean)=>void}=$props();
</script>
<button type="button" class="toggle-switch" role="switch" aria-label={label} title={label} aria-checked={checked} {disabled} onclick={()=>onchange(!checked)} class:checked><span aria-hidden="true">{thumbText}</span></button>
<style>
  .toggle-switch{width:36px;min-width:36px;height:20px;min-height:20px;padding:2px;border:1px solid var(--line);border-radius:12px;background:var(--muted);display:flex;align-items:center;justify-content:flex-start;flex-shrink:0}
  button span{display:grid;place-items:center;width:14px;height:14px;border-radius:50%;background:white;color:#202020;font-size:10px;line-height:1;transform:translateX(0)}
  button.checked{background:var(--accent);border-color:var(--accent)}button.checked span{transform:translateX(16px)}
  button:disabled{opacity:.45}
</style>
