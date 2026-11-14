export type Story={title:string;url:string;badge?:string};
export type Source={id:string;name:string};
export type Preferences={auto_refresh:boolean;refresh_minutes:number;followed:string[];hidden:string[];order:string[];default_followed:boolean};
export function ordered(sources:Source[],preferences:Preferences,followed:boolean):Source[]{
  return sources.filter(s=>!preferences.hidden.includes(s.id)&&(!followed||preferences.followed.includes(s.id)))
    .sort((a,b)=>{const index=(id:string)=>{const i=preferences.order.indexOf(id);return i<0?Number.MAX_SAFE_INTEGER:i;};return index(a.id)-index(b.id);});
}
export function batches<T>(items:T[]):T[][]{return Array.from({length:Math.ceil(items.length/4)},(_,i)=>items.slice(i*4,i*4+4));}
