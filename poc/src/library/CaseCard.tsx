import {useEffect,useRef,useState} from 'react';
import {convertFileSrc} from '@tauri-apps/api/core';
import {highlightParts} from '../search';
import {ReferenceCarousel} from '../ReferenceCarousel';
import {Icon} from './Icons';
import {dateLabel,safeSource} from './model';
import type {Asset,Case} from '../types';

export function Mark({text,query}:{text:string;query:string}){
  return <>{highlightParts(text,query).map((part,index)=>part.hit?<mark className="promptmark" key={index}>{part.text}</mark>:<span key={index}>{part.text}</span>)}</>;
}
function LibraryThumbnail({asset,label,result,notice}:{asset?:Asset;label:string;result?:boolean;notice:(text:string)=>void}){
  const element=useRef<HTMLButtonElement>(null);const [visible,setVisible]=useState(false);const [failed,setFailed]=useState(false);
  useEffect(()=>{const node=element.current;if(!node)return;const observer=new IntersectionObserver(entries=>setVisible(entries.some(entry=>entry.isIntersecting)),{threshold:0});observer.observe(node);return()=>observer.disconnect()},[]);
  useEffect(()=>setFailed(false),[asset?.thumbnail]);
  const ratio=asset&&asset.height>0?asset.width/asset.height:1;
  return <button ref={element} type="button" className={result?'result-pic':'ref-pic'} style={{width:result?90:Math.round(90*Math.max(2/3,Math.min(1.5,ratio)))}} title={label} aria-label={label} onClick={()=>notice('原图预览暂未开放')}>
    <div className="thumb-art">{visible&&asset&&!failed?<img src={convertFileSrc(asset.thumbnail)} alt={label} decoding="async" onError={()=>setFailed(true)}/>:<span className="thumb-loading"><Icon name="image" size={20}/><span className="sr-only">{failed?'缩略图加载失败':'等待缩略图'}</span></span>}</div>
    <span className="piclabel">{result?'效果':label}</span>
  </button>;
}
/** Browse only: no editor/draft state or optimistic write is created in this stage. */
export function CaseCard({record,query,assets,positions,notice}:{record:Case;query:string;assets:Map<string,Asset>;positions:Map<string,number>;notice:(text:string)=>void}){
  const source=safeSource(record.sourceUrl);const stamp=dateLabel(record.updatedAt??record.createdAt);
  return <article className="case" data-case-id={record.id} data-case-state="saved" data-editable="false">
    <div className="casehead"><span className="case-title" title={record.title}><Mark text={record.title} query={query}/></span><span className="case-project" title={record.project}>{'· '}<Mark text={record.project||'独立案例'} query={query}/></span>
      <div className="case-tags">{record.tags.map(tag=><span className="tag-chip" key={tag}><Mark text={tag} query={query}/></span>)}</div>
      <div className="card-actions">
        {source?<button type="button" className="iconbutton source-link" title={source} aria-label={`打开 ${record.title} 的来源链接`} onClick={()=>notice('来源链接打开暂未开放')}><Icon name="link" size={15}/></button>:<span className="iconbutton source-link unavailable" title="尚未设置来源链接，请先编辑案例" aria-label="未设置来源链接"><Icon name="link" size={15}/></span>}
        <button type="button" className="iconbutton" title="在卡片内编辑" aria-label={`编辑 ${record.title}`} onClick={()=>notice('案例编辑暂未开放')}><Icon name="pencil"/></button>
        <button type="button" className={`iconbutton ${record.favorite?'faved':''}`} title={record.favorite?'取消收藏':'收藏'} aria-label={`${record.favorite?'取消收藏':'收藏'} ${record.title}`} aria-pressed={record.favorite} onClick={()=>notice('收藏修改暂未开放')}><Icon name="star"/></button>
      </div>
    </div>
    {(record.result||record.refs.length>0)&&<div className="media">
      {record.result&&<LibraryThumbnail result asset={assets.get(record.result)} label="生成效果图" notice={notice}/>}
      {record.result&&record.refs.length>0&&<span className="media-divider"/>}
      {record.refs.length>0&&<ReferenceCarousel caseId={record.id} visible positions={positions} imageKey={JSON.stringify(record.refs.map(id=>[id,assets.get(id)?.width,assets.get(id)?.height]))}>
        {record.refs.map((id,index)=><LibraryThumbnail key={`${id}-${index}`} asset={assets.get(id)} label={`参考${index+1}`} notice={notice}/>)}
      </ReferenceCarousel>}
    </div>}
    <div className="prompt"><Mark text={record.prompt} query={query}/></div>
    <div className="casefoot"><span>{stamp&&`${stamp} · `}{record.refs.length} 张参考图{record.result?' · 1 张效果图':''}</span></div>
  </article>;
}
