const icons=import.meta.glob<string>('../../../assets/source-icons/*.{png,ico}',{eager:true,query:'?url',import:'default'});
const colors:Record<string,string>={
  weibo:'#e84545',zhihu:'#1675e8',bilibili:'#ed6597',baidu:'#4844df',xiaohongshu:'#e83d52',douyin:'#d63d67',
  wallstreetcn:'#267ac8',jin10:'#b58c2a',v2ex:'#607d69',zaobao:'#ce3939',coolapk:'#26a15d',mktnews:'#3d8b82',
  '36kr':'#3174d9',hupu:'#d64141',dongqiudi:'#23a267',aihot:'#8a59ca',tieba:'#3383e1',toutiao:'#dd4242',
  ithome:'#d63d42',thepaper:'#42739b',sputniknewscn:'#c28123',cankaoxiaoxi:'#b73e3e',pcbeta:'#3389bf',
  cls:'#cf343b',xueqiu:'#277ac5',gelonghui:'#b08e35',fastbull:'#4482bd',solidot:'#61874a',hackernews:'#d47528',
  producthunt:'#d45a42',github:'#6a66ad',kuaishou:'#d96e25',kaopu:'#617d94',nowcoder:'#28a17f',sspai:'#cc5147',
  juejin:'#3079de',ifeng:'#c84747',chongbuluo:'#72914c',douban:'#329262',steam:'#3d7199',tencent:'#2c8dc4',
  freebuf:'#459e80',qqvideo:'#d29527',iqiyi:'#35a45c'
};
export function sourceStyle(id:string){
  const platform=id.split('-')[0];
  const path=`../../../assets/source-icons/${platform}.${platform==='xiaohongshu'?'ico':'png'}`;
  return {color:colors[platform]??'#527d6d',icon:icons[path]??icons['../../../assets/source-icons/default.png']};
}
