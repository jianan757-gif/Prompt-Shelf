import {useLayoutEffect,useRef,useState} from 'react';
import type {ReactNode} from 'react';
import {nearestReference,railEdges,restoreReference,stepReference} from './carousel';
import type {RailDirection} from './carousel';
import './carousel.css';

/** V4.11 rail: fixed 90px images, actual image boundaries, no native bars. */
export function ReferenceCarousel({caseId,visible,positions,imageKey,children}:{caseId:string;visible:boolean;positions:Map<string,number>;imageKey:string;children:ReactNode}) {
  const viewport=useRef<HTMLDivElement>(null);const track=useRef<HTMLDivElement>(null);
  const pending=useRef<number|null>(null);const [edges,setEdges]=useState({prev:false,next:false});
  const geometry=()=>{
    const vp=viewport.current,strip=track.current;
    if(!vp||!strip)return null;
    const items=Array.from(strip.children) as HTMLElement[];
    const origin=items[0]?.offsetLeft??0;
    return {vp,offsets:items.map(item=>item.offsetLeft-origin),max:Math.max(0,vp.scrollWidth-vp.clientWidth)};
  };
  const update=()=>{
    const shape=geometry();if(!shape)return;
    const next=railEdges(shape.vp.scrollLeft,shape.max);
    setEdges(previous=>previous.prev===next.prev&&previous.next===next.next?previous:next);
    positions.set(caseId,nearestReference(shape.offsets,shape.vp.scrollLeft));
    if(pending.current!==null&&Math.abs(shape.vp.scrollLeft-pending.current)<1.5)pending.current=null;
  };
  useLayoutEffect(()=>{
    pending.current=null;
    if(!visible){setEdges({prev:false,next:false});return}
    const vp=viewport.current,strip=track.current;if(!vp||!strip)return;
    let disposed=false;
    const restore=()=>{
      if(disposed)return;
      const shape=geometry();if(!shape)return;
      pending.current=null;
      shape.vp.scrollLeft=restoreReference(shape.offsets,positions.get(caseId)??0,shape.max);
      update();
    };
    const onScroll=()=>{if(!disposed)update()};
    const onScrollEnd=()=>{pending.current=null;onScroll()};
    const observer=new ResizeObserver(restore);observer.observe(vp);observer.observe(strip);
    vp.addEventListener('scroll',onScroll,{passive:true});
    vp.addEventListener('scrollend',onScrollEnd);
    restore();const raf=requestAnimationFrame(restore);
    return()=>{disposed=true;cancelAnimationFrame(raf);observer.disconnect();vp.removeEventListener('scroll',onScroll);vp.removeEventListener('scrollend',onScrollEnd)};
  // Parent queue/search status rerenders recreate children, but must not reset
  // smooth motion. Only actual image identity/geometry changes restore position.
  },[caseId,visible,positions,imageKey]);
  const turn=(direction:RailDirection)=>{
    const shape=geometry();if(!shape)return;
    // Repeated clicks during smooth motion advance from the requested boundary.
    const from=pending.current??shape.vp.scrollLeft;
    const left=stepReference(shape.offsets,from,shape.max,direction);
    pending.current=left;
    positions.set(caseId,nearestReference(shape.offsets,left));
    shape.vp.scrollTo({left,behavior:'smooth'});
  };
  return <div className="reference-carousel" role="group" aria-label="参考图逐张浏览" data-carousel-case={caseId}>
    <div className="reference-viewport" ref={viewport} onFocusCapture={()=>{pending.current=null}}><div className="reference-track" ref={track}>{children}</div></div>
    {(['prev','next'] as const).map(direction=><button key={direction} type="button" className={`reference-edge ${direction} ${edges[direction]?'available':''}`} hidden={!edges[direction]} aria-label={direction==='prev'?'上一张参考图':'下一张参考图'} title={direction==='prev'?'上一张参考图':'下一张参考图'} onMouseDown={event=>event.stopPropagation()} onClick={event=>{event.preventDefault();event.stopPropagation();turn(direction)}}>
      <span className="reference-arrow"><svg viewBox="0 0 24 24" aria-hidden="true"><path d={direction==='prev'?'m14.5 5.5-6.5 6.5 6.5 6.5':'m9.5 5.5 6.5 6.5-6.5 6.5'}/></svg></span>
    </button>)}
  </div>;
}
