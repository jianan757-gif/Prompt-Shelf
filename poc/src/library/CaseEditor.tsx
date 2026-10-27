import {useEffect,useLayoutEffect,useRef} from 'react';
import type {PointerEvent} from 'react';
import {convertFileSrc} from '@tauri-apps/api/core';
import {ReferenceCarousel} from '../ReferenceCarousel';
import {Icon} from './Icons';
import type {Asset,CatalogItem} from '../types';
import type {ImageReference,Slot} from './editor';
import type {useCaseEditor} from './useCaseEditor';

type Controller=ReturnType<typeof useCaseEditor>;
function EditingImage({reference,result,index,asset,selected,disabled,pending,select,replace,remove,reorder}:{reference:ImageReference;result?:boolean;index:number;asset?:Asset;selected:boolean;disabled:boolean;pending:boolean;select:()=>void;replace:()=>void;remove:()=>void;reorder:(from:string,to:string)=>void}){
  const drag=useRef<{x:number;y:number;moved:boolean}|null>(null);
  const slot=result?'result':reference.id;
  const start=(event:PointerEvent<HTMLDivElement>)=>{
    if(disabled||(event.target as Element).closest('button'))return;
    select();event.preventDefault();event.currentTarget.focus({preventScroll:true});
    if(!result){drag.current={x:event.clientX,y:event.clientY,moved:false};event.currentTarget.setPointerCapture(event.pointerId)}
  };
  const finish=(event:PointerEvent<HTMLDivElement>)=>{
    if(drag.current?.moved){const target=document.elementFromPoint(event.clientX,event.clientY)?.closest<HTMLElement>('[data-reference-id]');if(target?.dataset.referenceId)reorder(reference.id,target.dataset.referenceId)}drag.current=null;
  };
  const ratio=asset&&asset.height>0?asset.width/asset.height:1;
  return <div className={`${result?'result-pic':'ref-pic'} thumb-select ${selected?'selected':''}`} style={{width:result?90:Math.round(90*Math.max(2/3,Math.min(1.5,ratio)))}} data-edit-slot={slot} data-reference-id={result?undefined:reference.id} role="button" tabIndex={disabled?-1:0} aria-pressed={selected} aria-label={`选择${result?'效果图':`参考图${index+1}`}作为图片导入目标`} title={result?'效果图 · 单击选择替换目标':'参考图 · 拖动调整顺序'}
    onPointerDown={start} onPointerMove={event=>{if(drag.current&&Math.hypot(event.clientX-drag.current.x,event.clientY-drag.current.y)>5)drag.current.moved=true}} onPointerUp={finish} onPointerCancel={()=>{drag.current=null}}
    onKeyDown={event=>{if(!disabled&&(event.key==='Enter'||event.key===' ')){event.preventDefault();select()}}}>
    <div className="thumb-art">{asset?<img src={convertFileSrc(asset.thumbnail)} alt={result?'效果图':`参考图${index+1}`} draggable={false} loading="lazy"/>:<span className="thumb-loading"><Icon name="image"/></span>}</div>
    <span className="piclabel">{result?'效果':`参考${index+1}`}</span>
    <button type="button" className="picedit" disabled={disabled||pending} title="替换图片" onClick={event=>{event.stopPropagation();replace()}}>替换</button>
    <button type="button" className="picremove" disabled={disabled} title="移除图片引用" aria-label={`移除${result?'效果图':`参考图${index+1}`}`} onClick={event=>{event.stopPropagation();remove()}}><Icon name="close" size={12}/></button>
  </div>;
}
export function CaseEditor({editor,assets,positions,projects}:{editor:Controller;assets:Map<string,Asset>;positions:Map<string,number>;projects:CatalogItem[]}){
  const active=editor.state!;const record=active.document;const title=useRef<HTMLInputElement>(null);const prompt=useRef<HTMLTextAreaElement>(null);
  useEffect(()=>{if(document.hasFocus())title.current?.focus({preventScroll:true})},[active.sessionId]);
  useLayoutEffect(()=>{const input=prompt.current;if(input){input.style.height='0px';input.style.height=`${Math.max(95,input.scrollHeight+2)}px`}},[record.prompt]);
  const change=(field:'title'|'project'|'prompt'|'sourceUrl',value:string)=>editor.change({...record,[field]:value});
  const picture=(reference:ImageReference,index:number,result=false)=><EditingImage key={reference.id} reference={reference} index={index} result={result} asset={assets.get(reference.assetId)} selected={active.selected===(result?'result':reference.id)} disabled={active.saving} pending={active.pending} select={()=>editor.select(result?'result':reference.id)} replace={()=>void editor.importImages(result?'result':reference.id)} remove={()=>editor.remove(result?'result':reference.id)} reorder={editor.reorder}/>;
  const slotLabel=active.selected==='append'?'新增参考图':active.selected==='result'?'效果图':`参考图 ${record.references.findIndex(reference=>reference.id===active.selected)+1}`;
  return <article className="case editing" data-case-id={record.id} data-editor-session={active.sessionId} data-case-state="unsaved">
    <div className="case-edit-head"><input ref={title} className="edit-title" aria-label="案例标题" placeholder="案例标题" value={record.title} disabled={active.saving} onChange={event=>change('title',event.target.value)}/><input className="edit-project" aria-label="所属项目" placeholder="所属项目（可空）" list="editor-projects" value={record.project} disabled={active.saving} onChange={event=>change('project',event.target.value)}/><datalist id="editor-projects">{projects.map(project=><option key={project.id} value={project.name}/>)}</datalist>
      <div className="card-actions"><button type="button" className="iconbutton editon" title="保存" aria-label="保存案例" disabled={active.pending||active.saving} onClick={()=>void editor.save()}><Icon name="check"/></button><button type="button" className="iconbutton" title="取消" aria-label="取消编辑" disabled={active.saving} onClick={()=>void editor.cancel()}><Icon name="close"/></button></div>
    </div>
    <div className="tag-editor"><label htmlFor="editor-tags">标签</label><input id="editor-tags" value={active.tagsText} disabled={active.saving} onChange={event=>editor.setTagsText(event.target.value)} placeholder="使用逗号分隔多个标签"/></div>
    <div className="paste-hint"><span>剪贴板图片粘贴至：<strong>{slotLabel}</strong></span><button type="button" className="paste-small" disabled={active.saving} onClick={()=>editor.select('append')}><Icon name="plus" size={13}/>追加参考图</button><button type="button" className="paste-small" disabled={active.saving} onClick={()=>editor.select('result')}>选中效果图</button><span className="notice-paste">单击选择替换目标 · Ctrl+V 粘贴图片 · 拖动参考图调整顺序</span></div>
    <div className="media">
      {record.result?picture(record.result,0,true):<button type="button" className={`blank-pic ${active.selected==='result'?'editor-empty-selected':''}`} data-edit-slot="result" disabled={active.pending||active.saving} onClick={()=>{editor.select('result');void editor.importImages('result')}}><Icon name="plus" size={17}/><span>效果图</span></button>}
      {record.references.length>0&&<span className="media-divider"/>}
      <ReferenceCarousel caseId={record.id} visible positions={positions} imageKey={JSON.stringify(record.references.map(reference=>[reference.id,reference.assetId,assets.get(reference.assetId)?.width,assets.get(reference.assetId)?.height]))}>
        {record.references.map((reference,index)=>picture(reference,index))}
        <button type="button" className="add-img" data-edit-slot="append" disabled={active.pending||active.saving} onClick={()=>{editor.select('append');void editor.importImages('append')}}><Icon name="plus" size={17}/><span>参考图</span></button>
      </ReferenceCarousel>
    </div>
    <textarea ref={prompt} className="edit-prompt" aria-label="完整提示词" value={record.prompt} disabled={active.saving} onChange={event=>change('prompt',event.target.value)} placeholder="完整提示词……"/>
    <div className="source-editor"><label htmlFor="editor-source">来源链接</label><input id="editor-source" type="text" spellCheck={false} value={record.sourceUrl} disabled={active.saving} onChange={event=>change('sourceUrl',event.target.value)} onBlur={()=>void editor.normalizeSource()} placeholder="https://…（可不填）"/><span className="source-hint">点击卡片顶部链接图标溯源</span></div>
    <div className="edit-bottom"><span className="fieldnote" role="status">{active.canceling?'正在取消…':active.saving?'正在保存…':active.pending?'正在导入图片…':'未正式保存'}</span><div className="edit-actions"><button type="button" className="cancel" disabled={active.saving} onClick={()=>void editor.cancel()}>取消</button><button type="button" className="save" disabled={active.pending||active.saving} onClick={()=>void editor.save()}>保存案例</button></div></div>
  </article>;
}
