<script>
  import Hongguo from '../src/Hongguo.svelte';
  let active=$state(true);
  export function setVisible(value){active=value;}
</script>
<Hongguo {active} showing={active} accountOnly={false} notice={text=>window.hongguoNotice=text} settingsChange={()=>{}}/>
