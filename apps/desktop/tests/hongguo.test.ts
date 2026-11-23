import {test} from 'node:test';
import assert from 'node:assert/strict';
import {coverPixels,watchedLabel,filterFavorites,loginPending,adjacent,message,qualityLabels,favoriteValue,playbackDetails,playerPreferences,danmakuTimeline} from '../src/hongguo.ts';
test('danmaku assigns four lanes in time order and reuses a lane once clear',()=>{
  const rows=[5000,0,0,0,0,1].map((offset_ms,index)=>({offset_ms,text:String(index)}));
  assert.deepEqual(danmakuTimeline(rows).map(({text,lane})=>[text,lane]),[['1',0],['2',1],['3',2],['4',3],['0',0]]);
  assert.equal(rows[0].offset_ms,5000);
});
test('player preferences preserve mute independently of volume and reject damaged storage',()=>{
  assert.deepEqual(playerPreferences({volume:0,mute:false,muteChosen:true,speed:1.5,quality:'1080p',favorites:true}),{volume:0,mute:false,muteChosen:true,speed:1.5,quality:'1080p',favorites:true});
  assert.deepEqual(playerPreferences({volume:2,speed:NaN,quality:720}),playerPreferences(null));
});
test('error details explain the failure without echoing raw transport secrets',()=>{
  const context={title:'测试短剧',episode:3,quality:'720p',direct:true,stage:'取源'};
  const detail=playbackDetails(new Error('media_redirect_limit'),context);
  assert.match(detail,/测试短剧 第3集/);assert.match(detail,/连续跳转超过上限/);assert.match(detail,/请求清晰度：720p/);assert.match(detail,/直连/);
  const unknown=playbackDetails(new Error('https://private.invalid/secret?token=SECRET'),context);
  assert.ok(!unknown.includes('SECRET'));assert.ok(!unknown.includes('https://'));
});
test('favorite feedback rolls back to confirmed list and never crosses items',()=>{
  const items=[{id:'1',title:'fixture',episodes:1}];
  assert.equal(favoriteValue(items,null,'1'),true);
  assert.equal(favoriteValue(items,{id:'1',value:false},'1'),false);
  assert.equal(favoriteValue(items,{id:'2',value:true},'1'),true);
  assert.equal(favoriteValue(items,{id:'2',value:true},'2'),true);
  assert.equal(favoriteValue(items,null,'2'),false);
  assert.equal(favoriteValue([],null,undefined),false);
});
test('numeric service qualities become canonical playback request strings',()=>{
  assert.deepEqual(qualityLabels([360,480,720,1080]),['360p','480p','720p','1080p']);
  assert.deepEqual(qualityLabels([1080]),['1080p']);
  assert.deepEqual(qualityLabels([2160]),[]);
});
test('login polling ends after confirmation, expiry or failure',()=>{
  assert.ok(loginPending('verifying'));assert.ok(loginPending('scanned'));
  for(const state of ['signed_in','expired','failed','idle'])assert.equal(loginPending(state),false);
});
test('episode navigation follows only the confirmed available list',()=>{
  assert.equal(adjacent([1,3,7],3,1),7);assert.equal(adjacent([1,3,7],1,-1),undefined);
  assert.equal(adjacent([1,3,7],2,1),undefined);assert.equal(message('login_required'),'请先登录红果账号');
});
test('playback failures stay readable when wrapped as Error objects',()=>{
  assert.equal(message(new Error('app_playback_empty_response')),'App 播放接口返回空内容（HTTP 200），尚未获取视频资源');
  assert.equal(message(new Error('app_access_denied')),'当前账号无法访问该媒体');
  assert.equal(message(new Error('media_redirect_target_rejected')),'媒体跳转被拒：目标不在允许范围');
  assert.equal(message(new Error('media_redirect_limit')),'媒体服务器连续跳转超过上限');
});

test('first playback is audible and explicit mute remains independent of volume',()=>{
  assert.equal(playerPreferences(null).mute,false);
  assert.equal(playerPreferences({mute:true,volume:.7}).mute,false,'old implicit mute must not override the new default');
  assert.equal(playerPreferences({mute:'true'}).mute,false);
  assert.equal(playerPreferences(null).volume,.5);
  assert.equal(playerPreferences({mute:true,muteChosen:true,volume:.7}).mute,true);
  assert.equal(playerPreferences({mute:false,muteChosen:true,volume:0}).mute,false);
});

test('favorites filter uses titles without changing confirmed collection',()=>{
 const items=[{id:'1',title:'春日',episodes:10},{id:'2',title:'Summer',episodes:20}];
 assert.deepEqual(filterFavorites(items,' 春 '),[items[0]]);
 assert.deepEqual(filterFavorites(items,'SUM'),[items[1]]);
 assert.equal(filterFavorites(items,'  '),items);
 assert.deepEqual(filterFavorites(items,'未找到'),[]);assert.equal(items.length,2);
});

test('cover pixels are bounded and viewing labels use actual episode records',()=>{
 assert.deepEqual(coverPixels({width:1,height:1,rgba:'/wAA/w=='}),new Uint8ClampedArray([255,0,0,255]));
 for(const data of [{width:241,height:1,rgba:''},{width:1,height:361,rgba:''},{width:1,height:1,rgba:'AA=='},{width:1,height:1,rgba:'invalid'}])assert.equal(coverPixels(data),undefined);
 assert.equal(watchedLabel({episode:3}),'看到第 3 集');assert.equal(watchedLabel(),'暂无观看记录');assert.equal(watchedLabel({episode:0}),'暂无观看记录');
});
