import {useEffect,useMemo,useRef,useState} from 'react';
import type {CSSProperties,PointerEvent} from 'react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import {dictionary} from '../search';
import {Icon} from './Icons';
import {CaseCard} from './CaseCard';
import {LibrarySidebar} from './LibrarySidebar';
import type {CatalogKind} from './LibrarySidebar';
import {CatalogPanel} from './CatalogPanel';
import {catalogs,columnCount,libraryCases} from './model';
import type {CaseSort,Layout} from './model';
import type {Library,Tab} from '../types';

type Boot={platform:string;escapeRegistered?:boolean};
function Splitter({name,width,change}:{name:'rail'|'secondary';width:number;change:(width:number)=>void}){
  const drag=useRef<{x:number;width:number}|null>(null);const [active,setActive]=useState(false);
  const begin=(event:PointerEvent<HTMLDivElement>)=>{event.preventDefault();event.currentTarget.setPointerCapture(event.pointerId);drag.current={x:event.clientX,width};setActive(true)};
  return <div className={`splitter ${active?'dragging':''}`} role="separator" aria-label={name==='rail'?'左侧导航宽度':'二级栏宽度'} aria-orientation="vertical" aria-valuenow={width} tabIndex={0} title={name==='rail'?'拖动调整左侧导航宽度':'拖动调整二级栏宽度'} onPointerDown={begin} onPointerMove={event=>{if(drag.current)change(drag.current.width+event.clientX-drag.current.x)}} onPointerUp={()=>{drag.current=null;setActive(false)}} onPointerCancel={()=>{drag.current=null;setActive(false)}} onKeyDown={event=>{if(event.key==='ArrowLeft'||event.key==='ArrowRight'){event.preventDefault();change(width+(event.key==='ArrowRight'?10:-10))}}}/>;
}
/** Native library browse surface. No draft state, save command or fake data exists here. */
export function LibraryManager(){
  const [data,setData]=useState<Library>({cases:[],assets:[]});const [loaded,setLoaded]=useState(false);const [loadError,setLoadError]=useState('');
  const [platform,setPlatform]=useState('');const [view,setView]=useState<Tab>('all');const [query,setQuery]=useState('');const [tag,setTag]=useState('');const [project,setProject]=useState('');
  const [expanded,setExpanded]=useState<CatalogKind|null>(null);const [layout,setLayout]=useState<Layout>('regular');const [sort,setSort]=useState<CaseSort>('recent');
  const [railWidth,setRailWidth]=useState(224);const [secondaryWidth,setSecondaryWidth]=useState(320);const [columns,setColumns]=useState(1);
  const [dictionaryOpen,setDictionaryOpen]=useState(false);const [toast,setToast]=useState('');
  const toastTimer=useRef<ReturnType<typeof setTimeout>|null>(null);const globalEscape=useRef(false);const liveEscape=useRef(false);
  const search=useRef<HTMLInputElement>(null);const scrollArea=useRef<HTMLDivElement>(null);const positions=useRef(new Map<string,number>());
  const assets=useMemo(()=>new Map(data.assets.map(asset=>[asset.id,asset])),[data.assets]);
  const tags=useMemo(()=>catalogs(data,'tags'),[data]);const projects=useMemo(()=>catalogs(data,'projects'),[data]);
  const quickTags=useMemo(()=>tags.filter(item=>item.isQuick).slice().sort((a,b)=>a.quickOrder-b.quickOrder||a.id.localeCompare(b.id)),[tags]);
  const records=useMemo(()=>libraryCases(data.cases,query,view,tag,project,sort),[data.cases,query,view,tag,project,sort]);
  const notice=(message:string)=>{setToast(message);if(toastTimer.current)clearTimeout(toastTimer.current);toastTimer.current=setTimeout(()=>setToast(''),2200)};
  const command=async(name:string,args?:Record<string,unknown>)=>{try{await invoke(name,args)}catch(error){notice(String(error))}};
  useEffect(()=>{
    let disposed=false;const unlisten:(()=>void)[]=[];
    if(!isTauri()){setLoadError('请通过桌面程序打开资料库');return}
    void(async()=>{
      try{
        const stop=await listen<{registered:boolean}>('escape-status',event=>{liveEscape.current=true;globalEscape.current=event.payload.registered});if(disposed){stop();return}unlisten.push(stop);
        const [library,boot]=await Promise.all([invoke<Library>('get_library'),invoke<Boot>('get_boot')]);if(disposed)return;
        setData(library);setPlatform(boot.platform);setLoaded(true);if(!liveEscape.current)globalEscape.current=!!boot.escapeRegistered;
        requestAnimationFrame(()=>{if(disposed||!document.hasFocus())return;search.current?.focus({preventScroll:true});void command('ready',{token:0,kind:'library',interactive:document.activeElement===search.current&&document.hasFocus()})});
      }catch(error){if(!disposed)setLoadError(String(error))}
    })();
    return()=>{disposed=true;unlisten.forEach(stop=>stop());if(toastTimer.current)clearTimeout(toastTimer.current)};
  },[]);
  useEffect(()=>{
    const area=scrollArea.current;if(!area)return;
    const observer=new ResizeObserver(entries=>{const next=columnCount(entries[0]?.contentRect.width??0);setColumns(previous=>previous===next?previous:next)});observer.observe(area);return()=>observer.disconnect();
  },[]);
  useEffect(()=>{
    const key=(event:KeyboardEvent)=>{
      if(event.key==='Escape'){
        event.preventDefault();if(globalEscape.current||event.repeat||event.isComposing)return;
        if(dictionaryOpen)setDictionaryOpen(false);else void command('close_library');return;
      }
      if(event.isComposing)return;
      if(((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='k')||(event.key==='/'&&!['INPUT','TEXTAREA'].includes((event.target as HTMLElement).tagName))){event.preventDefault();search.current?.focus();search.current?.select()}
    };
    document.addEventListener('keydown',key);return()=>document.removeEventListener('keydown',key);
  },[dictionaryOpen]);
  const chooseTag=(name:string)=>setTag(previous=>previous===name?'':name);
  const chooseProject=(name:string)=>setProject(previous=>previous===name?'':name);
  const toggle=(kind:CatalogKind)=>setExpanded(previous=>previous===kind?null:kind);
  const heading=project||tag||(view==='fav'?'收藏的案例':view==='recent'?'最近使用':'我的素材资料库');
  const styles={'--rail-w':`${railWidth}px`,'--sub-w':`${secondaryWidth}px`} as CSSProperties;
  const cards=records.map(record=><CaseCard key={record.id} record={record} query={query} assets={assets} positions={positions.current} notice={notice}/>);
  return <div className="library-root" style={styles} data-library-mode="browse" data-draft-state="none"><main className="workspace">
    <header className={`topbar ${platform==='macos'?'mac-titlebar':''}`} data-tauri-drag-region><div className="brand" data-tauri-drag-region><span className="brandicon">✦</span>Prompt Shelf <span className="breadcrumb">/ 素材管理 / 图文资料库</span></div>
      <div className="top-actions"><button type="button" className="btn primary sm" onClick={()=>notice('案例新建暂未开放')}><Icon name="plus"/> 新建案例</button><button type="button" className="btn sm" onClick={()=>notice('回收站暂未开放')}><Icon name="archive"/> 回收站 <span id="trashBadge">{data.trashCount??'—'}</span></button><button type="button" className="btn sm" onClick={()=>setDictionaryOpen(true)}><Icon name="book"/> 搜索词典</button><button type="button" className="btn sm" title="备份与恢复" onClick={()=>notice('备份与恢复暂未开放')}><Icon name="backup" size={15}/> 备份与恢复</button>
        <span className="window-divider" aria-hidden="true"/><div className="window-controls" role="group" aria-label="窗口控制">
          <button type="button" className="window-btn" title="最小化窗口" aria-label="最小化窗口" onClick={()=>void command('window_action',{action:'minimize'})}><Icon name="minimize"/></button>
          <button type="button" className="window-btn" title="最大化/窗口化" aria-label="最大化或还原窗口" onClick={()=>void command('window_action',{action:'maximize'})}><Icon name="maximize"/></button>
          <button type="button" className="window-btn window-close" title="关闭窗口" aria-label="关闭窗口" onClick={()=>void command('close_library')}><Icon name="close"/></button>
        </div>
      </div>
    </header>
    <div className="body"><LibrarySidebar records={data.cases} view={view} setView={setView} expanded={expanded} toggle={toggle} quickTags={quickTags} activeTag={tag} setTag={chooseTag} notice={notice}/>
      <Splitter name="rail" width={railWidth} change={width=>setRailWidth(Math.max(172,Math.min(340,width)))}/>
      {expanded&&<><CatalogPanel key={expanded} kind={expanded} entries={expanded==='tags'?tags:projects} records={data.cases} active={expanded==='tags'?tag:project} choose={expanded==='tags'?chooseTag:chooseProject} notice={notice} canHandleEscape={()=>!globalEscape.current}/><Splitter name="secondary" width={secondaryWidth} change={width=>setSecondaryWidth(Math.max(278,Math.min(680,width)))}/></>}
      <section className="library" aria-label="案例资料库"><div className="libraryhead"><div className="headrow"><div className="heading-wrap"><div className="libtitle">{heading}</div><div className="libdesc">图文直览 · 原卡片内编辑 · 网格与瀑布流自由切换</div></div>
        <div className="head-controls v410-toolbar"><div className="view-toggle" role="group" aria-label="卡片布局">{(['regular','masonry'] as const).map(mode=><button type="button" key={mode} className={layout===mode?'active':''} aria-pressed={layout===mode} title={mode==='regular'?'常规网格：同排等高':'瀑布流：按列紧凑排列'} onClick={()=>setLayout(mode)}><Icon name={mode}/>{mode==='regular'?'常规':'瀑布流'}</button>)}</div><div className="sort-toggle" role="group" aria-label="全部案例排序">{([['recent','最近使用'],['name','按名称'],['new','最新创建']] as const).map(([value,label])=><button type="button" key={value} className={sort===value?'active':''} aria-pressed={sort===value} onClick={()=>setSort(value)}>{label}</button>)}</div></div>
      </div><div className="searchrow"><label className="global-search"><Icon name="search" size={15}/><input ref={search} value={query} onChange={event=>setQuery(event.target.value)} placeholder="搜索案例标题、提示词正文、项目、标签…" aria-label="搜索案例" autoComplete="off"/></label></div>
      <div className="quick-filter-row"><div className="quickbar"><span className="qlabel">快捷标签</span><button type="button" className={`quicktag ${!tag?'active':''}`} onClick={()=>setTag('')}>全部场景</button>{quickTags.map(item=><button type="button" key={item.id} className={`quicktag ${tag===item.name?'active':''}`} onClick={()=>chooseTag(item.name)}>{item.name}</button>)}</div><button type="button" className="btn sm toolbar-clear" title="清除搜索和筛选" onClick={()=>{setQuery('');setTag('');setProject('');setView('all')}}>清除筛选</button></div>
      <div className="statsrow"><b>{records.length} 条案例</b><span>{layout==='regular'?'常规网格 · 同排等高':'瀑布流 · 各列独立排列'}{tag&&` · 标签：${tag}`}{project&&` · 项目：${project}`}</span></div></div>
      <div className="scrollarea" ref={scrollArea}><div className={`gallery ${layout}`} data-columns={columns}>
        {cards.length?layout==='masonry'?Array.from({length:columns},(_,column)=><div className="masonry-column" key={column}>{cards.filter((_,index)=>index%columns===column)}</div>):cards:<div className="empty-results">{loadError?<><strong>资料库暂时无法读取</strong><br/>{loadError}</>:!loaded?'正在加载资料库…':data.cases.length?<>没有符合筛选条件的案例<br/>请调整搜索词、标签或项目。</>:'资料库暂无案例'}</div>}
      </div></div></section>
    </div>
  </main>
  {toast&&<div className="library-toast" role="status" aria-live="polite">{toast}</div>}
  {dictionaryOpen&&<div className="dictionary-shade" onClick={()=>setDictionaryOpen(false)}><section className="dictionary-panel" role="dialog" aria-modal="true" aria-label="搜索词典" onClick={event=>event.stopPropagation()}><h2>领域近似词词典 · 规则预览</h2><p>这是独立于案例标签的搜索扩展规则。直接命中优先，同组近似表达也会参与搜索。</p><div className="dictionary-rows">{dictionary.map(group=><div style={{display:'contents'}} key={group[0]}><div><b>{group[0]}</b></div><div>{group.slice(1).join('、')}</div></div>)}</div><button type="button" className="btn primary" onClick={()=>setDictionaryOpen(false)}>关闭</button></section></div>}
  </div>;
}
