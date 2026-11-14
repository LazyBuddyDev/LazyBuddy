export type Item = {id:string;title:string;episodes:number|null;has_cover?:boolean};
export function playerPreferences(value:unknown){
  const p=value&&typeof value==='object'?value as Record<string,unknown>:{};
  const number=(value:unknown,min:number,max:number,fallback:number)=>typeof value==='number'&&Number.isFinite(value)&&value>=min&&value<=max?value:fallback;
  return {volume:number(p.volume,0,1,.5),speed:number(p.speed,.25,4,1),mute:typeof p.mute==='boolean'?p.mute:true,quality:typeof p.quality==='string'&&['360p','480p','720p','1080p'].includes(p.quality)?p.quality:'720p',favorites:p.favorites===true};
}
export type Account = {state:string;name:string;qr_png?:string;error?:string|null;favorites_error?:string|null;favorites?:Item[];operation?:string|null};
export type Playback = {session:number;loading:boolean;prepared:boolean;suspended:boolean;resource:{url:string;mime:string}|null;episodes?:number[];available_qualities?:number[];error?:string|null;progress_error?:string|null};
export function qualityLabels(values:number[]):string[] {
  return values.filter(value=>[360,480,720,1080].includes(value)).map(value=>`${value}p`);
}
export function favoriteValue(items:Item[],pending:{id:string;value:boolean}|null,id:string|undefined):boolean {
  return pending&&pending.id===id?pending.value:items.some(item=>item.id===id);
}
export function loginPending(state:string):boolean {return ['preparing','pending','scanned','verifying'].includes(state);}
export function adjacent(episodes:number[],current:number,delta:number):number|undefined {
  const index=episodes.indexOf(current);
  return index<0?undefined:episodes[index+delta];
}
export function message(error:unknown):string {
  const value=error instanceof Error?error.message:String(error);
  return ({login_required:'请先登录红果账号',login_step_failed:'扫码登录失败，请重试',credential_save_failed:'登录信息安全保存失败',
    saved_session_verification_failed:'已保存的登录状态核对失败，请重新登录',favorites_unavailable:'收藏列表读取失败',favorites_sync_failed:'收藏同步失败',
    app_access_denied:'当前账号无法访问该媒体',episode_unavailable:'当前集不可播放',quality_unavailable:'当前清晰度不可用',
    app_playback_unavailable:'App 播放接口暂不可用，尚未获取视频资源',app_playback_empty_response:'App 播放接口返回空内容（HTTP 200），尚未获取视频资源',
    media_access_denied:'媒体访问被拒绝',media_network_error:'媒体网络请求失败',app_source_network_error:'取源网络请求失败',
    browser_format_unsupported:'浏览器不支持该视频编码或封装',browser_decode_failed:'视频解码失败',
    browser_media_error:'浏览器媒体加载失败，具体原因请结合日志定位',
    media_redirect_rejected:'媒体跳转校验失败',media_redirect_target_rejected:'媒体跳转被拒：目标不在允许范围',
    media_redirect_https_required:'媒体跳转被拒：目标不是 HTTPS',media_redirect_missing_location:'媒体服务器未提供跳转地址',media_redirect_limit:'媒体服务器连续跳转超过上限',
    account_request_failed:'账号请求失败，请查看本地日志',search_failed:'搜索请求失败，请查看本地日志',invalid_quality:'清晰度参数无效，请重新选择档位',
    app_source_timeout:'取源超时',playback_service_unavailable:'红果服务连接已断开，请重试'} as Record<string,string>)[value]??(/^[a-z_]{1,80}$/.test(value)?`操作失败，请查看日志（${value}）`:'操作失败，请查看本地日志');
}
export function playbackDetails(error:unknown,context:{title:string;episode:number;quality:string;direct:boolean;stage:string}):string {
  const value=error instanceof Error?error.message:String(error);
  const code=/^[a-z_]{1,80}$/.test(value)?value:'browser_media_error';
  const translated=message(code);
  const reason=translated===code?'媒体加载或播放失败，具体原因请结合日志定位':translated;
  const action=code==='quality_unavailable'?'请选择其他清晰度后重试。':code==='media_access_denied'?'媒体访问被拒绝，请核对账号权限；不会自动重试。':'可点击视频中央“重试”重新连接当前集。';
  return `${context.title} 第${context.episode}集：${reason}\n阶段：${context.stage}；请求清晰度：${context.quality}；连接：${context.direct?'直连':'系统代理'}\n${action}\n错误码：${code}。准确定位请查看本地日志。`;
}
