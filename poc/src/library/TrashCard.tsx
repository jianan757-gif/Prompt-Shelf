import {useState} from 'react';
import {convertFileSrc} from '@tauri-apps/api/core';
import type {Asset,TrashCase} from '../types';
import {Icon} from './Icons';
import {Mark} from './CaseCard';

function TrashThumbnail({asset,result}:{asset?:Asset;result:boolean}){
  const [failed,setFailed]=useState(false);
  return <div className={`tr-pic ${result?'result':''}`} title={result?'生成效果图':'参考图'}>{asset&&!failed?<img src={convertFileSrc(asset.thumbnail)} alt={result?'生成效果图':'参考图'} decoding="async" loading="lazy" onError={()=>setFailed(true)}/>:<Icon name="image" size={22}/>}</div>;
}
export function TrashCard({record,assets,query,dictionary,busy,onRestore,onPermanent}:{record:TrashCase;assets:Map<string,Asset>;query:string;dictionary:string[][];busy:boolean;onRestore:()=>void;onPermanent:()=>void}){
  const images=[...(record.result?[{id:record.result,result:true}]:[]),...record.refs.map(id=>({id,result:false}))];
  return <article className="case trashcase" data-case-id={record.id} data-case-state="trash">
    <div className="tr-head"><div className="tr-heading"><div className="tr-title" title={record.title}><Mark text={record.title} query={query} dictionary={dictionary}/></div><div className="tr-project">原项目：{record.deletedProjectName||'独立案例'} · {record.refs.length} 张参考图</div></div><div className="tr-pics">{images.slice(0,3).map((image,index)=><TrashThumbnail key={`${image.id}-${index}`} asset={assets.get(image.id)} result={image.result}/>)}{images.length>3&&<span className="tr-more">+{images.length-3}</span>}</div></div>
    <div className="tr-text" title={record.prompt}><Mark text={record.prompt||'暂无提示词内容'} query={query} dictionary={dictionary}/></div>
    <div className="tr-foot"><span className="tr-shared">{record.sharedImageCount?`其中 ${record.sharedImageCount} 张图片仍被其他案例使用`:'原始图片暂保留'}</span><div className="tr-buttons"><button type="button" className="btn sm" disabled={busy} aria-label={`恢复 ${record.title}`} onClick={onRestore}><Icon name="restore" size={14}/> 恢复</button><button type="button" className="quiet-delete" disabled={busy} aria-label={`永久删除 ${record.title}`} onClick={onPermanent}><Icon name="trash" size={13}/> 永久删除</button></div></div>
  </article>;
}
