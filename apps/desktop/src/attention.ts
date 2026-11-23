/** Host-owned notification routing. Sources publish facts; targets only acknowledge snapshots. */
export type AttentionSignal={id:string;revision:string;targets:string[]};
export type AttentionItem=AttentionSignal&{key:string};
export type AttentionReceipts=Record<string,string>;
export class AttentionCenter {
  private sources=new Map<string,{items:AttentionSignal[]}>();
  private changed:(items:AttentionItem[])=>void;
  constructor(changed:(items:AttentionItem[])=>void=()=>{}){this.changed=changed;}
  register(source:string){
    if(this.sources.has(source))throw Error(`Duplicate attention source: ${source}`);
    const owner={items:[] as AttentionSignal[]};this.sources.set(source,owner);
    return {
      publish:(items:AttentionSignal[])=>{if(this.sources.get(source)===owner){owner.items=items;this.changed(this.items());}},
      dispose:()=>{if(this.sources.get(source)===owner){this.sources.delete(source);this.changed(this.items());}},
    };
  }
  items():AttentionItem[]{return [...this.sources].flatMap(([source,owner])=>owner.items.map(item=>({...item,key:`${source}/${item.id}`})));}
}
export function unreadAttention(items:AttentionItem[],receipts:AttentionReceipts,target:string){
  return items.filter(item=>item.targets.includes(target)&&receipts[item.key]!==item.revision);
}
export function acknowledgeAttention(receipts:AttentionReceipts,items:AttentionItem[]):AttentionReceipts{
  const next={...receipts};for(const item of items){delete next[item.key];next[item.key]=item.revision;}
  return Object.fromEntries(Object.entries(next).slice(-2048));
}
export function moduleAttention(modules:{id:string;version:string;available_version?:string;installed:boolean;update:boolean}[]):AttentionSignal[]{
  return modules.flatMap(module=>{
    const targets=['modules',`module:${module.id}`];
    if(!module.installed)return [{id:`new:${module.id}`,revision:'available',targets}];
    return module.update?[{id:`update:${module.id}`,revision:module.available_version??module.version,targets}]:[];
  });
}
export function updateAttention(build:string|null):AttentionSignal[]{
  return build?[{id:'prepared',revision:build,targets:['settings','settings.system','app.update']}]:[];
}
/** Re-read parameters on interaction, so a stale DOM handler cannot acknowledge a future revision. */
export function attentionTarget(node:HTMLElement,initial:{ack:()=>void;enabled:boolean}){
  let options=initial;
  const pointer=(event:PointerEvent)=>{if(event.pointerType!=='touch'&&options.enabled)options.ack();};
  const focus=()=>{if(options.enabled)options.ack();};
  node.addEventListener('pointerenter',pointer);node.addEventListener('focusin',focus);
  return {update:(value:typeof initial)=>{options=value;},destroy:()=>{node.removeEventListener('pointerenter',pointer);node.removeEventListener('focusin',focus);}};
}
