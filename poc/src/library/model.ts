import {dictionary,filterCases} from '../search.ts';
import type {Case,CatalogItem,Library,Tab} from '../types.ts';

export type CaseSort='recent'|'name'|'new';
export type Layout='regular'|'masonry';
export const columnCount=(width:number)=>width>=1390?3:width>=845?2:1;
export function libraryCases(data:Case[],query:string,view:Tab,tag:string,project:string,sort:CaseSort,groups:readonly string[][]=dictionary):Case[]{
  return filterCases(data,query,view,tag,project,sort,groups);
}
export function catalogs(data:Library,kind:'projects'|'tags'):CatalogItem[]{
  // Older DTOs can still be displayed; unknown dates remain unknown (zero), never inferred from UUID.
  return data[kind]??[...new Set(data.cases.flatMap(r=>kind==='tags'?r.tags:r.project?[r.project]:[]))]
    .map(name=>({id:name,name,createdAt:0,isQuick:false,quickOrder:0}));
}
export const catalogCount=(records:Case[],kind:'projects'|'tags',id:string)=>records.filter(r=>kind==='tags'?(r.tagIds?r.tagIds.includes(id):r.tags.includes(id)):(r.projectId!==undefined?r.projectId===id:r.project===id)).length;
export function safeSource(value?:string):string|null{
  if(!value?.trim())return null;
  try{const url=new URL(value);return ['http:','https:'].includes(url.protocol)?url.href:null}catch{return null}
}
export function dateLabel(timestamp?:number|null):string{
  if(timestamp==null||!Number.isFinite(timestamp))return '';
  if(new Date(timestamp).toDateString()===new Date().toDateString())return '今天';
  return new Intl.DateTimeFormat('zh-CN',{month:'long',day:'numeric'}).format(timestamp);
}
