<script lang="ts">
  import {onDestroy} from 'svelte';
  import {invoke} from '@tauri-apps/api/core';
  let {label,save,notice,active=true}:{active?:boolean;label:string;save:(value:{modifiers:number;key:number})=>Promise<boolean>;notice:(text:string)=>void}=$props();
  let recording=$state(false);let arming=false;let saving=false;let ticket=0;let button:HTMLButtonElement;
  async function stop(){ticket++;recording=false;try{await invoke('capture_shortcut',{enabled:false});}catch(error){notice(String(error));}}
  async function begin(){
    if(recording||arming||saving)return;
    const current=++ticket;arming=true;
    try{
      await invoke('capture_shortcut',{enabled:true});
      if(current===ticket&&document.activeElement===button)recording=true;
      else await stop();
    }catch(error){notice(String(error));}finally{arming=false;}
  }
  async function keydown(event:KeyboardEvent){
    if(!recording)return;
    if(event.code==='Tab'){void stop();return;}
    event.preventDefault();event.stopPropagation();
    if(event.code==='Escape'){void stop();return;}
    if(event.repeat||saving)return;
    const key=event.code.startsWith('Key')?event.code.charCodeAt(3):event.code.startsWith('Digit')?event.code.charCodeAt(5):/^F\d+$/.test(event.code)?111+Number(event.code.slice(1)):0;
    if(!key)return;
    saving=true;
    try{await save({modifiers:(event.altKey?1:0)|(event.ctrlKey?2:0)|(event.shiftKey?4:0),key});await stop();}
    finally{saving=false;}
  }
  function blur(){if(recording||arming)void stop();}
  $effect(()=>{if(!active&&(recording||arming))void stop();});
  onDestroy(()=>{if(recording||arming)void stop();});
</script>
<svelte:window onblur={blur}/>
<button bind:this={button} aria-label="显示隐藏快捷键" aria-pressed={recording} title={recording?'按下组合键，Esc取消':'点击后按下新的组合键'} onclick={begin} onkeydown={keydown} onblur={blur}>{recording?'请按快捷键（Esc 取消）':label}</button>
