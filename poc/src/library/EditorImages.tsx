import {useEffect,useLayoutEffect,useRef,useState} from 'react';
import type {PointerEvent} from 'react';
import {createPortal} from 'react-dom';
import {convertFileSrc} from '@tauri-apps/api/core';
import {ReferenceCarousel} from '../ReferenceCarousel';
import {Icon} from './Icons';
import type {Asset} from '../types';
import type {ImageReference,Slot} from './editor';

type Props={caseId:string;result:ImageReference|null;references:ImageReference[];assets:Map<string,Asset>;positions:Map<string,number>;selected:Slot;disabled:boolean;pending:boolean;select:(slot:Slot)=>void;replace:(slot:Slot)=>void;remove:(slot:Slot)=>void;reorder:(from:string,to:string)=>void};
type Drag={id:string;pointer:number;startX:number;startY:number;offsetX:number;offsetY:number;width:number;original:string[];order:string[];active:boolean};
type Ghost={id:string;x:number;y:number;width:number;settling:boolean};
const imageWidth=(asset?:Asset)=>Math.round(90*Math.max(2/3,Math.min(1.5,asset&&asset.height>0?asset.width/asset.height:1)));

/** Preview order stays local; one stable-ID reorder reaches the editor only on release. */
export function EditorImages(props:Props){
  const {result,references,assets,disabled,pending,selected}=props;
  const media=useRef<HTMLDivElement>(null);const nodes=useRef(new Map<string,HTMLDivElement>());const ghostNode=useRef<HTMLDivElement>(null);
  const drag=useRef<Drag|null>(null);const settling=useRef(false);const epoch=useRef(0);const previous=useRef(new Map<string,DOMRect>());
  const [preview,setPreview]=useState<string[]|null>(null);const [ghost,setGhost]=useState<Ghost|null>(null);
  const live=useRef(props);live.current=props;
  const ordered=(preview??references.map(r=>r.id)).map(id=>references.find(r=>r.id===id)).filter((r):r is ImageReference=>!!r);
  const remember=()=>{previous.current=new Map([...nodes.current].map(([id,node])=>[id,node.getBoundingClientRect()]));for(const node of nodes.current.values())node.getAnimations().forEach(animation=>animation.cancel())};
  const finish=(cancel=false)=>{
    const d=drag.current;if(!d)return;drag.current=null;
    if(d.active){remember();if(!cancel){const index=d.order.indexOf(d.id);if(index!==d.original.indexOf(d.id))live.current.reorder(d.id,d.original[index])}setPreview(null);settling.current=true;setGhost(current=>current?{...current,settling:true}:null)}
    if(media.current?.hasPointerCapture(d.pointer))media.current.releasePointerCapture(d.pointer);
  };
  useLayoutEffect(()=>{
    const reduced=matchMedia('(prefers-reduced-motion: reduce)').matches;
    for(const [id,node] of nodes.current){const before=previous.current.get(id),after=node.getBoundingClientRect();if(before&&id!==ghost?.id&&(Math.abs(before.left-after.left)>.5||Math.abs(before.top-after.top)>.5))node.animate([{transform:`translate(${before.left-after.left}px,${before.top-after.top}px)`},{transform:'translate(0,0)'}],{duration:reduced?0:210,easing:'cubic-bezier(.22,.8,.3,1)'})}previous.current.clear();
    if(ghost?.settling&&ghostNode.current){const target=nodes.current.get(ghost.id)?.getBoundingClientRect();const node=ghostNode.current;const token=++epoch.current;
      const done=()=>{if(epoch.current===token){settling.current=false;setGhost(null)}};
      if(!target){done();return}const animation=node.animate([{transform:`translate(${ghost.x}px,${ghost.y}px)`,opacity:.65},{transform:`translate(${target.left}px,${target.top}px)`,opacity:1}],{duration:reduced?0:160,easing:'cubic-bezier(.22,.8,.3,1)',fill:'forwards'});void animation.finished.then(done,done);
    }
  },[preview,references,ghost?.settling]);
  useEffect(()=>{if(drag.current)finish(true)},[references]);
  useEffect(()=>{
    const key=(event:KeyboardEvent)=>{if(event.key==='Escape'&&drag.current){event.preventDefault();event.stopPropagation();event.stopImmediatePropagation();finish(true)}};
    const blur=()=>finish(true);
    document.addEventListener('keydown',key,true);window.addEventListener('blur',blur);return()=>{document.removeEventListener('keydown',key,true);window.removeEventListener('blur',blur);epoch.current++;drag.current=null};
  },[]);
  const start=(event:PointerEvent<HTMLButtonElement>,reference:ImageReference)=>{
    if(event.button!==0||!event.isPrimary||disabled||pending||drag.current||settling.current)return;
    props.select(reference.id);event.preventDefault();event.currentTarget.focus({preventScroll:true});const bounds=nodes.current.get(reference.id)!.getBoundingClientRect();
    drag.current={id:reference.id,pointer:event.pointerId,startX:event.clientX,startY:event.clientY,offsetX:event.clientX-bounds.left,offsetY:event.clientY-bounds.top,width:bounds.width,original:references.map(r=>r.id),order:references.map(r=>r.id),active:false};media.current!.setPointerCapture(event.pointerId);
  };
  const move=(event:PointerEvent<HTMLDivElement>)=>{
    const d=drag.current;if(!d||event.pointerId!==d.pointer)return;if(!d.active&&Math.hypot(event.clientX-d.startX,event.clientY-d.startY)<5)return;
    d.active=true;setGhost({id:d.id,x:event.clientX-d.offsetX,y:event.clientY-d.offsetY,width:d.width,settling:false});
    const node=nodes.current.get(d.id);const track=node?.parentElement;if(!node||!track)return;const vp=track.parentElement!;const edge=vp.getBoundingClientRect();if(event.clientX<edge.left+20)vp.scrollLeft-=12;else if(event.clientX>edge.right-20)vp.scrollLeft+=12;
    const origin=track.getBoundingClientRect();const center=event.clientX-d.offsetX+d.width/2;let closest=d.id,distance=Infinity;
    for(const [id,other] of nodes.current){const delta=Math.abs(center-origin.left-other.offsetLeft-other.offsetWidth/2);if(delta<distance){closest=id;distance=delta}}
    if(closest!==d.id){const next=[...d.order];const from=next.indexOf(d.id),to=next.indexOf(closest);if(from<0||to<0)return;next.splice(to,0,...next.splice(from,1));remember();d.order=next;setPreview(next)}
  };
  const art=(reference:ImageReference,index:number,effect=false)=>{const asset=assets.get(reference.assetId);return <><span className="thumb-art">{asset?<img src={convertFileSrc(asset.thumbnail)} alt={effect?'效果图':`参考图${index+1}`} draggable={false} loading="lazy"/>:<span className="thumb-loading"><Icon name="image"/></span>}</span><span className="editor-image-number">{effect?'效果':`参考${index+1}`}</span><span className="editor-image-check" aria-hidden="true"><Icon name="check" size={12}/></span></>};
  const picture=(reference:ImageReference,index:number,effect=false)=>{
    const slot=effect?'result':reference.id;const label=effect?'效果图':`参考图${index+1}`;
    return <div key={reference.id} ref={node=>{if(!effect){if(node)nodes.current.set(reference.id,node);else nodes.current.delete(reference.id)}}} className={`${effect?'result-pic':'ref-pic'} editor-thumb ${selected===slot?'selected':''} ${ghost?.id===reference.id?'is-dragging':''}`} style={{width:effect?90:imageWidth(assets.get(reference.assetId))}} data-edit-slot={slot} data-reference-id={effect?undefined:reference.id}>
      <button type="button" className="editor-image-select" disabled={disabled||pending} aria-label={`选择${label}${effect?'':'，可拖动排序'}`} aria-pressed={selected===slot} title={label} onClick={()=>props.select(slot)} onPointerDown={effect?undefined:event=>start(event,reference)} onKeyDown={event=>{if(!effect&&event.altKey&&(event.key==='ArrowLeft'||event.key==='ArrowRight')){event.preventDefault();const to=index+(event.key==='ArrowLeft'?-1:1);if(references[to])props.reorder(reference.id,references[to].id)}}}>{art(reference,index,effect)}</button>
      <div className="editor-image-tools"><button type="button" disabled={disabled||pending||!!ghost} title="替换图片" aria-label={`替换${label}`} onClick={()=>{props.select(slot);props.replace(slot)}}><Icon name="unlink"/></button><button type="button" disabled={disabled||!!ghost} title="移除引用" aria-label={`移除${label}`} onClick={()=>props.remove(slot)}><Icon name="trash-2"/></button></div>
    </div>;
  };
  const ghostReference=ghost&&references.find(r=>r.id===ghost.id);
  return <div className="media editor-media" ref={media} data-drag-active={!!ghost} onPointerMove={move} onPointerUp={event=>{if(event.pointerId===drag.current?.pointer)finish()}} onPointerCancel={event=>{if(event.pointerId===drag.current?.pointer)finish(true)}} onLostPointerCapture={event=>{if(event.pointerId===drag.current?.pointer)finish(true)}}>
    {result?picture(result,0,true):<button type="button" className={`blank-pic ${selected==='result'?'editor-empty-selected':''}`} data-edit-slot="result" disabled={pending||disabled} title="添加效果图" aria-label="添加效果图" onClick={()=>{props.select('result');props.replace('result')}}><Icon name="plus" size={17}/><span>效果图</span></button>}
    {references.length>0&&<span className="media-divider"/>}
    <ReferenceCarousel caseId={props.caseId} visible positions={props.positions} imageKey={JSON.stringify(references.map(r=>[r.id,r.assetId,assets.get(r.assetId)?.width,assets.get(r.assetId)?.height]))}>
      {ordered.map((reference,index)=>picture(reference,index))}<button type="button" className="add-img" data-edit-slot="append" disabled={pending||disabled||!!ghost} aria-label="添加参考图" onClick={()=>{props.select('append');props.replace('append')}}><Icon name="plus" size={17}/><span>参考图</span></button>
    </ReferenceCarousel>
    {ghost&&ghostReference&&createPortal(<div ref={ghostNode} className="editor-drag-ghost editor-thumb selected" aria-hidden="true" style={{width:ghost.width,transform:`translate(${ghost.x}px,${ghost.y}px)`}}>{art(ghostReference,ordered.findIndex(r=>r.id===ghost.id))}</div>,document.body)}
  </div>;
}
