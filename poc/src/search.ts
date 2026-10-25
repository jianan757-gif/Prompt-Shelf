import type {Case,Tab} from './types.ts';
export const dictionary=[['换色','色调迁移','配色替换'],['保构图','构图锁定','结构锁定'],['精修','高清精修','商业精修'],['扩图','画面扩展','图像扩展','外扩'],['去背','去背景','背景移除'],['产品渲染','商品渲染','商业CG']];
export const terms=(query:string)=>query.trim().toLocaleLowerCase().split(/\s+/u).filter(Boolean);
export const expand=(term:string)=>dictionary.find(group=>group.some(x=>x.toLocaleLowerCase()===term))??[term];
export function score(record:Case,query:string):number {
  let total=0;
  for(const term of terms(query)) {
    let best=0;
    for(const [text,weight] of [[record.title,12],[record.prompt,6],[record.project,2],[record.tags.join(' '),2]] as const) {
      const value=text.toLocaleLowerCase();
      if(value.includes(term))best=Math.max(best,30+weight*3+(value===term?12:0));
      else if(expand(term).some(x=>value.includes(x.toLocaleLowerCase())))best=Math.max(best,10+weight);
    }
    if(!best)return -1;
    total+=best;
  }
  return total;
}
export function filterCases(records:Case[],query:string,tab:Tab,tag='',project='',sort='recent') {
  return records.filter(r=>(tab!=='fav'||r.favorite)&&(tab!=='recent'||r.used>0)&&(!tag||r.tags.includes(tag))&&(!project||r.project===project))
    .map(r=>({r,s:score(r,query)})).filter(x=>x.s>=0)
    .sort((a,b)=>query.trim()?b.s-a.s||b.r.used-a.r.used:sort==='name'?a.r.title.localeCompare(b.r.title,'zh-CN'):sort==='new'?String(b.r.id).localeCompare(String(a.r.id),undefined,{numeric:true}):b.r.used-a.r.used).map(x=>x.r);
}
// Split the raw text before React escapes it. Never construct HTML from query or content.
export function highlightParts(text:string,query:string):{text:string;hit:boolean}[] {
  const needles=[...new Set(terms(query).flatMap(expand))].sort((a,b)=>b.length-a.length);
  if(!needles.length)return [{text,hit:false}];
  const safe=needles.map(x=>x.replace(/[.*+?^${}()|[\]\\]/g,'\\$&'));
  const re=new RegExp(safe.join('|'),'giu');let cursor=0;const output=[];
  for(const match of text.matchAll(re)) {
    const at=match.index!;
    if(at>cursor)output.push({text:text.slice(cursor,at),hit:false});
    output.push({text:match[0],hit:true});cursor=at+match[0].length;
  }
  if(cursor<text.length)output.push({text:text.slice(cursor),hit:false});
  return output;
}
