export type Story={title:string;url:string;badge?:string|null};
export type Source={id:string;name:string;subtitle?:string|null};
export type Preferences={auto_refresh:boolean;refresh_minutes:number;followed:string[];hidden:string[];order:string[];default_followed:boolean};
export function ordered(sources:Source[],preferences:Preferences,followed:boolean):Source[]{
  return sources.filter(s=>!preferences.hidden.includes(s.id)&&(!followed||preferences.followed.includes(s.id)))
    .sort((a,b)=>{const index=(id:string)=>{const i=preferences.order.indexOf(id);return i<0?Number.MAX_SAFE_INTEGER:i;};return index(a.id)-index(b.id);});
}
export function batches<T>(items:T[]):T[][]{return Array.from({length:Math.ceil(items.length/4)},(_,i)=>items.slice(i*4,i*4+4));}

export function refreshSources(sources:Source[],preferences:Preferences,followed:boolean,cache:Record<string,{updated_ms:number}>,force:boolean,sourceId?:string,now=Date.now()):string[]{
  return ordered(sources,preferences,followed)
    .filter(s=>(sourceId===undefined||s.id===sourceId)&&(force||!cache[s.id]||now-cache[s.id].updated_ms>=preferences.refresh_minutes*60000))
    .map(s=>s.id);
}

const badgeLabels:Record<string,string>={New:'新',Hot:'热',Boiling:'沸',Explosive:'爆',Meme:'梗',Exclusive:'独家',Recommended:'荐',FirstRelease:'首发'};
export function badgeLabel(badge?:string|null):string|undefined{return badge?badgeLabels[badge]:undefined;}

export type SourceReply={id:string;ok:boolean;stories:Story[];updated_ms:number;error?:{code:string;detail?:string}};
export function sourceFailure(row:SourceReply,sources:Source[]):string{
  const name=sources.find(source=>source.id===row.id)?.name??row.id;
  const detail=row.error?.detail;
  const reason=!detail?'暂时无法获取':detail.includes('kind=timeout')?'请求超时':/status=(401|403)\b/.test(detail)?'上游拒绝访问':detail.includes('kind=no_usable_stories')?'上游未返回可用条目':detail.includes('stage=parse')?'上游内容解析失败':detail.includes('kind=tls')?'安全连接失败':'获取失败';
  return `${name}：${reason}，保留已有内容${detail?`\n来源请求详情：${detail}`:''}`;
}
