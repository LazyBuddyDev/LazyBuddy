export type Position = {book_id:string;content_version:string;chapter_id:string;block_id:string;offset:number};
export type Source = {kind:string;id:string;format:string;encoding:string};
export type Entry = {id:string;title:string;author:string;source:Source;content_version:string;chapters:number;position:Position|null;revision:number;updated_ms:number};
export type Chapter = {id:string;title:string;blocks:number};
export type Book = {id:string;title:string;author:string;content_version:string;chapters:Chapter[];warnings:string[]};
export type Block = {id:string;kind:string;text:string;runs:{text:string;strong:boolean;emphasis:boolean;link?:string}[];resource:string|null};
export type ReadingPreferences = {font_size:number;line_height:number;width:number};
export const pageSize=96;
export function pageStart(blockId:string):number { return Math.floor(Number(blockId.slice(1))/pageSize)*pageSize; }
export function progressLabel(entry:Entry):string {
  if(!entry.position)return '未开始';
  return `第 ${Number(entry.position.chapter_id.slice(1))+1} / ${entry.chapters} 章`;
}
export function readerError(error:unknown):string {
  const code=String(error);
  const messages:Record<string,string>={encoding_required:'无法确定文本编码，请选择编码后重新导入',invalid_text_encoding:'所选编码无法正确读取文件，请更换编码',file_too_large:'文件超过32MB上限',archive_too_large:'EPUB解压内容超过上限',encrypted_epub:'暂不支持加密的EPUB',encrypted_or_invalid_epub:'EPUB已加密或文件损坏',fixed_layout_epub:'暂不支持固定版式EPUB',invalid_epub:'EPUB结构无效',invalid_xml:'EPUB正文格式无效',unsafe_xml:'EPUB含不受支持的实体声明',unsafe_archive:'EPUB包含不安全的资源路径',unsafe_resource:'EPUB资源地址不受支持',unsupported_format:'请选择TXT或EPUB文件',empty_book:'未找到可阅读的正文',file_unreadable:'文件无法读取，请检查权限或重新选择',save_failed:'保存失败，请检查磁盘空间与目录权限',invalid_saved_data:'书库数据损坏，已保留原文件',read_failed:'读取书库失败，已保留原文件',reader_disconnected:'阅读服务未能连接，请重新进入小说页面',content_version_changed:'正文已变化，请重新打开书籍',import_busy:'上一项导入仍在收尾，请稍后重试',removal_cleanup_pending:'书籍已移除，副本清理未完成',book_not_found:'书籍不存在，请返回书库',unsupported_xml_encoding:'EPUB内部文本需使用UTF-8编码',unsupported_spine_format:'EPUB含不受支持的正文格式'};
  return messages[code]??`操作未完成（${code.slice(0,100)}）`;
}
export function highlights(text:string,query:string):{text:string;match:boolean}[] {
  if(!query)return [{text,match:false}];
  const result:{text:string;match:boolean}[]=[];let start=0;let i:number;
  while((i=text.indexOf(query,start))>=0){result.push({text:text.slice(start,i),match:false},{text:query,match:true});start=i+query.length;}
  result.push({text:text.slice(start),match:false});return result;
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
  const top=container.getBoundingClientRect().top+12;
  const block=Array.from(container.querySelectorAll<HTMLElement>('[data-block]')).find(b=>b.getBoundingClientRect().bottom>top);
  if(!block)return null;
  const length=Array.from(block.textContent??'').length;
  let low=0,high=length;
  if(block.dataset.kind!=='image')while(low<high){const mid=Math.floor((low+high)/2);const rect=textRange(block,mid)?.getBoundingClientRect();if(rect&&rect.bottom<=top)low=mid+1;else high=mid;}
  return {block_id:block.dataset.block!,offset:block.dataset.kind==='image'?0:low};
}
export function restoreAnchor(container:HTMLElement,position:Pick<Position,'block_id'|'offset'>):boolean {
  const block=Array.from(container.querySelectorAll<HTMLElement>('[data-block]')).find(b=>b.dataset.block===position.block_id);
  if(!block)return false;
  const rect=position.offset?textRange(block,position.offset)?.getBoundingClientRect():block.getBoundingClientRect();
  if(!rect)return false;
  container.scrollTop+=rect.top-container.getBoundingClientRect().top-12;return true;
}
