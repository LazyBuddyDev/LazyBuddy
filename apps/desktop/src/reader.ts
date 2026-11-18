export type Position = {book_id:string;content_version:string;chapter_id:string;block_id:string;offset:number;chapter_version?:string};
export type Source = {kind:string;id:string;format:string;encoding:string};
export type Entry = {id:string;title:string;author:string;source:Source;content_version:string;chapters:number;position:Position|null;revision:number;updated_ms:number;in_shelf?:boolean;visited_ms?:number};
export type Chapter = {id:string;title:string;blocks:number};
export type Book = {id:string;title:string;author:string;source:Source;description?:string;content_version:string;chapters:Chapter[];warnings:string[]};
export type OnlineEntry = {id:string;title:string;author:string;description:string};
export type OnlineSource = 'fanqie'|'qimao'|'shuqi';
export const onlineSources:Record<OnlineSource,string>={fanqie:'番茄小说',qimao:'七猫中文网',shuqi:'书旗小说'};
export type OnlinePage = {source:OnlineSource;entries:OnlineEntry[];cursor:number;next_cursor:number|null};
export type OnlineBook = OnlineEntry & {source:OnlineSource;chapters:{id:string;title:string}[]};
export type Block = {id:string;kind:string;text:string;runs:{text:string;strong:boolean;emphasis:boolean;link?:string}[];resource:string|null};
export type PageMode = 'horizontal'|'vertical'|'scroll';
export type ReadingPreferences = {font_size:number;line_height:number;width:number;page_mode:PageMode;history_limit?:number};
export function viewportStep(height:number,line:number):number { return Math.max(line,Math.floor((height-32)/line)*line); }
export function pageOffset(offset:number,step:number,extent:number,direction:number):number { return Math.max(0,Math.min(extent,offset+direction*step)); }
export const pageSize=96;
export function matchOffsets(text:string,query:string):number[]{
  if(!query)return [];const offsets:number[]=[];let start=0,index:number;
  while((index=text.indexOf(query,start))>=0){offsets.push(index);start=index+query.length;}return offsets;
}
export function wrapMatch(index:number,total:number):number{return total?((index%total)+total)%total:-1;}
export function markedRuns(block:Block,query:string,currentOffset=-1){
  const matches=matchOffsets(block.text,query);
  const runs=block.runs.length?block.runs:[{text:block.text,strong:false,emphasis:false}];
  const parts:{text:string;strong:boolean;emphasis:boolean;match:boolean;current:boolean}[]=[];let base=0,matchIndex=0;
  for(const run of runs){
    let local=0;
    while(local<run.text.length){
      const at=base+local;while(matchIndex<matches.length&&matches[matchIndex]+query.length<=at)matchIndex++;
      const offset=matches[matchIndex];const hit=offset!==undefined&&offset<=at;
      const end=Math.min(base+run.text.length,offset===undefined?Infinity:hit?offset+query.length:offset);
      parts.push({text:run.text.slice(local,end-base),strong:run.strong,emphasis:run.emphasis,match:hit,current:hit&&offset===currentOffset});local=end-base;
    }
    base+=run.text.length;
  }
  return parts;
}
export function pageStart(blockId:string):number { return Math.floor(Number(blockId.slice(1))/pageSize)*pageSize; }
export function progressLabel(entry:Entry):string {
  if(entry.source.format==='online')return entry.position?`继续阅读 · 共 ${entry.chapters} 章`:`共 ${entry.chapters} 章`;
  if(!entry.position)return '未开始';
  return `第 ${Number(entry.position.chapter_id.slice(1))+1} / ${entry.chapters} 章`;
}
export function readerError(error:unknown):string {
  const code=String(error);
  const bodyErrors:Record<string,string>={source_chapter_locked:'该章节未向访客免费开放，请在官方平台阅读',source_helper_missing:'正文组件缺失，请通过 start.bat 更新小说功能',source_runtime_failed:'正文组件未能启动，请重新打开书籍',source_decode_failed:'本章正文解析失败，请稍后手动重试',source_retry_stopped:'本章请求已停止，请稍后手动重试',source_empty_chapter:'本章未返回可读正文',chapter_not_loaded:'章节尚未载入，请重新打开书籍',credential_save_failed:'设备身份保存失败，请检查 Windows 凭据管理器',credential_read_failed:'设备身份读取失败，原有凭据已保留',invalid_saved_identity:'已保存的设备身份无效，原有凭据已保留',invalid_credential_scope:'阅读器凭据作用域无效'};
  if(bodyErrors[code])return bodyErrors[code];
  if(code==='source_body_unavailable')return '来源正文暂不可用，书架与目录已保留';
  if(code==='source_detail_required')return '请重新载入目录后加入书架';
  if(code==='incomplete_source_directory')return '来源目录不完整，请重新载入，已有数据已保留';
  if(code==='invalid_source_book')return '来源书籍标识无效，请重新搜索';
  const onlineMessages:Record<string,string>={search_busy:'上一项在线请求仍在收尾，请稍后重试',source_verification_required:'来源要求完成官方验证，本次请求已停止',source_access_denied:'来源拒绝访问，本次请求已停止',source_rate_limited:'请求过于频繁，请稍后再试',source_timeout:'来源响应超时，请稍后重试',source_network_failed:'无法连接来源，请检查网络后重试',source_empty_response:'来源返回空响应，请稍后重试',source_request_rejected:'来源未接受本次请求',invalid_source_response:'来源响应格式异常，请稍后重试',source_response_too_large:'来源响应超过大小限制',source_text_obfuscated:'来源返回了无法直接显示的文字，本次请求已停止',invalid_search_query:'请输入1至100个字符的书名或作者',invalid_search_cursor:'搜索分页位置无效，请重新搜索',unsupported_source:'暂不支持此在线来源',search_worker_failed:'在线请求未能完成，请重试'};
  if(onlineMessages[code])return onlineMessages[code];
  const messages:Record<string,string>={encoding_required:'无法确定文本编码，请选择编码后重新导入',invalid_text_encoding:'所选编码无法正确读取文件，请更换编码',file_too_large:'文件超过32MB上限',archive_too_large:'EPUB解压内容超过上限',encrypted_epub:'暂不支持加密的EPUB',encrypted_or_invalid_epub:'EPUB已加密或文件损坏',fixed_layout_epub:'暂不支持固定版式EPUB',invalid_epub:'EPUB结构无效',invalid_xml:'EPUB正文格式无效',unsafe_xml:'EPUB含不受支持的实体声明',unsafe_archive:'EPUB包含不安全的资源路径',unsafe_resource:'EPUB资源地址不受支持',unsupported_format:'请选择TXT或EPUB文件',empty_book:'未找到可阅读的正文',file_unreadable:'文件无法读取，请检查权限或重新选择',save_failed:'保存失败，请检查磁盘空间与目录权限',invalid_saved_data:'书库数据损坏，已保留原文件',read_failed:'读取书库失败，已保留原文件',reader_disconnected:'阅读服务未能连接，请重新进入小说页面',content_version_changed:'正文已变化，请重新打开书籍',import_busy:'上一项导入仍在收尾，请稍后重试',removal_cleanup_pending:'书籍已移除，副本清理未完成',book_not_found:'书籍不存在，请返回书库',unsupported_xml_encoding:'EPUB内部文本需使用UTF-8编码',unsupported_spine_format:'EPUB含不受支持的正文格式'};
  return messages[code]??`操作未完成（${code.slice(0,100)}）`;
}
// Offsets use Unicode scalar values in the service, while DOM ranges use UTF-16.
export function textRange(element:HTMLElement,offset:number):Range|null {
  let remaining=Array.from(element.textContent??'').slice(0,offset).join('').length;
  const walker=document.createTreeWalker(element,NodeFilter.SHOW_TEXT);let node:Node|null;
  while((node=walker.nextNode())){
    if(remaining<=(node.textContent?.length??0)){const range=document.createRange();range.setStart(node,remaining);range.setEnd(node,Math.min(remaining+1,node.textContent?.length??0));return range;}
    remaining-=node.textContent?.length??0;
  }
  return null;
}
export function visibleAnchor(container:HTMLElement):{block_id:string;offset:number}|null {
  const viewport=container.getBoundingClientRect(),top=viewport.top+12;
  const horizontal=container.dataset.pageMode==='horizontal';
  const block=Array.from(container.querySelectorAll<HTMLElement>('[data-block]')).find(b=>{
    const range=document.createRange();range.selectNodeContents(b);
    const rects=b.dataset.kind==='image'?b.getClientRects():range.getClientRects();
    return Array.from(rects).some(r=>r.bottom>top&&r.top<viewport.bottom&&(!horizontal||r.right>viewport.left+1&&r.left<viewport.right));
  });
  if(!block)return null;
  const length=Array.from(block.textContent??'').length;
  let low=0,high=length;
  if(block.dataset.kind!=='image')while(low<high){
    const mid=Math.floor((low+high)/2),rect=textRange(block,mid)?.getBoundingClientRect();
    const before=rect&&(horizontal?(rect.right<=viewport.left+1||rect.left<viewport.right&&rect.bottom<=top):rect.bottom<=top);
    if(before)low=mid+1;else high=mid;
  }
  return {block_id:block.dataset.block!,offset:block.dataset.kind==='image'?0:low};
}
export function restoreAnchor(container:HTMLElement,position:Pick<Position,'block_id'|'offset'>):boolean {
  const block=Array.from(container.querySelectorAll<HTMLElement>('[data-block]')).find(b=>b.dataset.block===position.block_id);
  if(!block)return false;
  const rect=block.dataset.kind==='image'?block.getBoundingClientRect():textRange(block,position.offset)?.getBoundingClientRect();
  if(!rect)return false;
  if(container.dataset.pageMode==='horizontal'){
    const page=Math.floor((rect.left-container.getBoundingClientRect().left+container.scrollLeft)/container.getBoundingClientRect().width);
    container.scrollTop=0;container.scrollLeft=page*container.getBoundingClientRect().width;
  }else{container.scrollLeft=0;container.scrollTop+=rect.top-container.getBoundingClientRect().top-12;}
  return true;
}

export function hasChapterTitle(blocks:Block[],title:string):boolean {
  const expected=title.replace(/[\s\u200B-\u200D\u2060\uFEFF]/g,'');
  if(!expected||blocks[0]?.id!=='b0')return false;
  let text='';
  for(const block of blocks.slice(0,3)){
    if(block.kind==='image')return false;
    text+=block.text.replace(/[\s\u200B-\u200D\u2060\uFEFF]/g,'');
    if(text===expected)return true;
    if(!expected.startsWith(text))return false;
  }
  return false;
}
