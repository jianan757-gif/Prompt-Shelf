import {useEffect,useRef,useState} from 'react';
import {Icon} from './Icons';
import {catalogCount} from './model';
import type {Case,CatalogItem,Tab} from '../types';

export type CatalogKind='projects'|'tags';
export function LibrarySidebar({records,view,setView,expanded,toggle,quickTags,activeTag,setTag,reorderQuick}:{records:Case[];view:Tab;setView:(view:Tab)=>void;expanded:CatalogKind|null;toggle:(kind:CatalogKind)=>void;quickTags:CatalogItem[];activeTag:string;setTag:(id:string)=>void;reorderQuick:(ids:string[])=>void}){
  const root=useRef<HTMLElement>(null);const dragging=useRef<{id:string;pointer:number;x:number;y:number;moved:boolean}|null>(null);const [over,setOver]=useState<string|null>(null);const overId=useRef<string|null>(null);const live=useRef({quickTags,reorderQuick});live.current={quickTags,reorderQuick};
  const move=(from:string,to:string)=>{const ids=live.current.quickTags.map(item=>item.id),start=ids.indexOf(from),end=ids.indexOf(to);if(start<0||end<0||start===end)return;ids.splice(end,0,...ids.splice(start,1));live.current.reorderQuick(ids)};
  const finish=(cancel=false)=>{const d=dragging.current;if(!d)return;dragging.current=null;const target=overId.current;overId.current=null;setOver(null);if(root.current?.hasPointerCapture(d.pointer))root.current.releasePointerCapture(d.pointer);if(!cancel&&d.moved&&target)move(d.id,target)};
  useEffect(()=>{const key=(event:KeyboardEvent)=>{if(event.key==='Escape'&&dragging.current){event.preventDefault();event.stopImmediatePropagation();finish(true)}};const blur=()=>finish(true);document.addEventListener('keydown',key,true);window.addEventListener('blur',blur);return()=>{document.removeEventListener('keydown',key,true);window.removeEventListener('blur',blur);dragging.current=null}},[]);
  return <aside ref={root} className="rail" aria-label="资料库导航" onPointerMove={event=>{const d=dragging.current;if(!d||event.pointerId!==d.pointer)return;if(Math.hypot(event.clientX-d.x,event.clientY-d.y)<5&&!d.moved)return;d.moved=true;const row=document.elementFromPoint(event.clientX,event.clientY)?.closest<HTMLElement>('[data-quick-tag-id]');const id=row&&root.current?.contains(row)?row.dataset.quickTagId??null:null;overId.current=id;setOver(id)}} onPointerUp={event=>{if(event.pointerId===dragging.current?.pointer)finish()}} onPointerCancel={event=>{if(event.pointerId===dragging.current?.pointer)finish(true)}} onLostPointerCapture={event=>{if(event.pointerId===dragging.current?.pointer)finish(true)}}><div className="rail-inner">
    <section className="rail-group"><div className="section-overline">常用集合</div>
      {([['all','全部'],['fav','收藏'],['recent','最近']] as const).map(([id,label])=><button type="button" className={`navitem ${view===id?'active':''}`} data-view={id} key={id} onClick={()=>setView(id)}><span className="navicon"><Icon name={`nav-${id}`} size={20}/></span><span>{label}</span><span className="count">{records.filter(record=>id==='all'||(id==='fav'?record.favorite:record.used>0)).length}</span></button>)}
    </section>
    <div className="navdivider"/>
    <section className="rail-group"><div className="section-overline">分类管理</div>
      {([['projects','项目归档'],['tags','标签']] as const).map(([kind,label])=><button type="button" key={kind} className={`navitem ${kind==='projects'?'project-navigation':'tag-navigation'} ${expanded===kind?'active':''}`} title={`展开或收起${label}`} aria-expanded={expanded===kind} onClick={()=>toggle(kind)}><span className="navicon"><Icon name={`nav-${kind}`} size={20}/></span><span>{label}</span></button>)}
    </section>
    <div className="navdivider"/>
    <section className="rail-group"><div className="section-overline">快捷标签</div><div id="quickTagList">
      {quickTags.map((item,index)=><div className={`tag-row ${over===item.id?'drag-over':''}`} key={item.id} data-quick-tag-id={item.id}><button type="button" className="tag-grip" aria-label={`调整 ${item.name} 的顺序`} title="拖动调整顺序；Alt+上下方向键移动" onPointerDown={event=>{if(event.button!==0||!event.isPrimary||dragging.current)return;event.preventDefault();event.currentTarget.focus({preventScroll:true});dragging.current={id:item.id,pointer:event.pointerId,x:event.clientX,y:event.clientY,moved:false};root.current?.setPointerCapture(event.pointerId)}} onKeyDown={event=>{if(event.altKey&&(event.key==='ArrowUp'||event.key==='ArrowDown')){event.preventDefault();const target=quickTags[index+(event.key==='ArrowUp'?-1:1)];if(target)move(item.id,target.id)}}}/><button type="button" className={`taglink ${activeTag===item.id?'active':''}`} onClick={()=>setTag(item.id)}>{item.name}</button><span className="countplain">{catalogCount(records,'tags',item.id)}</span></div>)}
      {!quickTags.length&&<p className="smallnote">暂无快捷标签</p>}
    </div></section>
    <div className="navdivider"/><p className="smallnote">项目归档为低频入口；常用标签可以直接在左侧筛选。</p>
  </div></aside>;
}
