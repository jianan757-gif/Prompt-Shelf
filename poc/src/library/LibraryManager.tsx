import {useEffect,useMemo,useRef,useState} from 'react';
import type {CSSProperties,PointerEvent} from 'react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import {dictionary} from '../search';
import {Icon} from './Icons';
import {CaseCard} from './CaseCard';
import {CaseEditor} from './CaseEditor';
import {EditSafetyDialog,DraftRecoveryDialog} from './EditSafetyDialogs';
import type {DraftSummary} from './editor';
import {useCaseEditor} from './useCaseEditor';
import {LatestRead} from '../refresh';
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
/** Native library; persistent draft state is owned by the editor and Rust. */
export function LibraryManager(){
  const [data,setData]=useState<Library>({cases:[],assets:[]});const [loaded,setLoaded]=useState(false);const [loadError,setLoadError]=useState('');
  const [platform,setPlatform]=useState('');const [view,setView]=useState<Tab>('all');const [query,setQuery]=useState('');const [tag,setTag]=useState('');const [project,setProject]=useState('');
  const [expanded,setExpanded]=useState<CatalogKind|null>(null);const [layout,setLayout]=useState<Layout>('regular');const [sort,setSort]=useState<CaseSort>('recent');
  const [railWidth,setRailWidth]=useState(224);const [secondaryWidth,setSecondaryWidth]=useState(320);const [columns,setColumns]=useState(1);
  const [drafts,setDrafts]=useState<DraftSummary[]>([]);const [recoveryOpen,setRecoveryOpen]=useState(false);const [recoveryBusy,setRecoveryBusy]=useState(false);
  const [dictionaryOpen,setDictionaryOpen]=useState(false);const [toast,setToast]=useState('');
  const toastTimer=useRef<ReturnType<typeof setTimeout>|null>(null);const globalEscape=useRef(false);const liveEscape=useRef(false);
  const reads=useRef(new LatestRead());const refreshLibrary=useRef<()=>Promise<void>>(async()=>{});
  const search=useRef<HTMLInputElement>(null);const scrollArea=useRef<HTMLDivElement>(null);const positions=useRef(new Map<string,number>());
  const tags=useMemo(()=>catalogs(data,'tags'),[data]);const projects=useMemo(()=>catalogs(data,'projects'),[data]);
  const quickTags=useMemo(()=>tags.filter(item=>item.isQuick).slice().sort((a,b)=>a.quickOrder-b.quickOrder||a.id.localeCompare(b.id)),[tags]);
  const records=useMemo(()=>libraryCases(data.cases,query,view,tag,project,sort),[data.cases,query,view,tag,project,sort]);
  const notice=(message:string)=>{setToast(message);if(toastTimer.current)clearTimeout(toastTimer.current);toastTimer.current=setTimeout(()=>setToast(''),2200)};
  const command=async(name:string,args?:Record<string,unknown>)=>{try{await invoke(name,args)}catch(error){notice(String(error))}};
  const editor=useCaseEditor(notice,async()=>{await refreshLibrary.current();await refreshDrafts()});
  const assets=useMemo(()=>new Map([...data.assets,...(editor.state?.images??[])].map(asset=>[asset.id,asset])),[data.assets,editor.state?.images]);
  const guard=(action:()=>void)=>editor.request(action);
  const editorLive=useRef(editor);editorLive.current=editor;
  const closeRequest=useRef<(request:{token:number;action:string})=>void>(()=>{});
  closeRequest.current=request=>editorLive.current.request(()=>void command('finish_library_close',{token:request.token}),()=>void command('cancel_library_close',{token:request.token}));
  const refreshDrafts=async()=>{const rows=await invoke<DraftSummary[]>('list_case_drafts');setDrafts(rows);return rows};
  const recover=async(id:string,asNew:boolean)=>{setRecoveryBusy(true);try{if(await editor.recover(id,asNew)){setRecoveryOpen(false);setQuery('');setTag('');setProject('');setView('all');await refreshLibrary.current();await refreshDrafts();requestAnimationFrame(()=>document.querySelector(`[data-case-id="${editorLive.current.state?.document.id}"]`)?.scrollIntoView({block:'nearest'}))}}catch(error){notice(String(error))}finally{setRecoveryBusy(false)}};
  const discardDraft=async(id:string)=>{setRecoveryBusy(true);try{await invoke('discard_case_draft',{draftId:id});if(!(await refreshDrafts()).length)setRecoveryOpen(false)}catch(error){notice(String(error))}finally{setRecoveryBusy(false)}};
  useEffect(()=>{
    let disposed=false;const unlisten:(()=>void)[]=[];
    if(!isTauri()){setLoadError('请通过桌面程序打开资料库');return}
    const refresh=async()=>{const token=reads.current.begin();const library=await invoke<Library>('get_library');if(!disposed&&reads.current.accepts(token)){setData(library);setLoaded(true);setLoadError('')}};
    refreshLibrary.current=refresh;
    const refreshQuietly=()=>{void refresh().catch(error=>{if(!disposed)notice(String(error))})};
    void(async()=>{
      try{
        const stop=await listen<{registered:boolean}>('escape-status',event=>{liveEscape.current=true;globalEscape.current=event.payload.registered});if(disposed){stop();return}unlisten.push(stop);
        const closing=await listen<{token:number;action:string}>('edit-close-request',event=>closeRequest.current(event.payload));if(disposed){closing();return}unlisten.push(closing);
        const pending=await invoke<{token:number;action:string}|null>('pending_library_close');if(pending)closeRequest.current(pending);
        const changed=await listen('library-changed',refreshQuietly);if(disposed){changed();return}unlisten.push(changed);
        window.addEventListener('focus',refreshQuietly);
        const [,boot]=await Promise.all([refresh(),invoke<Boot>('get_boot')]);if(disposed)return;
        const hasRecovery=(await refreshDrafts()).length>0;if(hasRecovery)setRecoveryOpen(true);
        setPlatform(boot.platform);if(!liveEscape.current)globalEscape.current=!!boot.escapeRegistered;
        requestAnimationFrame(()=>{if(disposed||!document.hasFocus())return;if(!hasRecovery)search.current?.focus({preventScroll:true});void command('ready',{token:0,kind:'library',interactive:document.hasFocus()})});
      }catch(error){if(!disposed)setLoadError(String(error))}
    })();
    return()=>{disposed=true;reads.current.invalidate();unlisten.forEach(stop=>stop());window.removeEventListener('focus',refreshQuietly);if(toastTimer.current)clearTimeout(toastTimer.current)};
  },[]);
  useEffect(()=>{
    const area=scrollArea.current;if(!area)return;
    const observer=new ResizeObserver(entries=>{const next=columnCount(entries[0]?.contentRect.width??0);setColumns(previous=>previous===next?previous:next)});observer.observe(area);return()=>observer.disconnect();
  },[]);
  useEffect(()=>{
    const key=(event:KeyboardEvent)=>{
      if(event.key==='Escape'){
        event.preventDefault();if(globalEscape.current||event.repeat||event.isComposing)return;
        if(editorLive.current.confirmation){void editorLive.current.resolve('continue');return}if(recoveryOpen){setRecoveryOpen(false);return}if(dictionaryOpen)setDictionaryOpen(false);else if(editorLive.current.busy())editorLive.current.cancel();else void command('close_library');return;
      }
      if(event.isComposing||editorLive.current.confirmation||recoveryOpen)return;
      if(((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='k')||(event.key==='/'&&!['INPUT','TEXTAREA'].includes((event.target as HTMLElement).tagName))){event.preventDefault();search.current?.focus();search.current?.select()}
    };
    document.addEventListener('keydown',key);return()=>document.removeEventListener('keydown',key);
  },[dictionaryOpen,recoveryOpen]);
  const chooseTag=(name:string)=>guard(()=>setTag(previous=>previous===name?'':name));
  const chooseProject=(name:string)=>guard(()=>setProject(previous=>previous===name?'':name));
  const toggle=(kind:CatalogKind)=>setExpanded(previous=>previous===kind?null:kind);
  const heading=project||tag||(view==='fav'?'收藏的案例':view==='recent'?'最近使用':'我的素材资料库');
  const styles={'--rail-w':`${railWidth}px`,'--sub-w':`${secondaryWidth}px`} as CSSProperties;
  const cardIds=records.map(record=>record.id);if(editor.state&&!cardIds.includes(editor.state.document.id))cardIds.unshift(editor.state.document.id);
  const cards=cardIds.map(id=>editor.state?.document.id===id?<CaseEditor key={id} editor={editor} assets={assets} positions={positions.current} projects={projects}/>:<CaseCard key={id} record={records.find(record=>record.id===id)!} query={query} assets={assets} positions={positions.current} notice={notice} onEdit={()=>guard(()=>void editor.begin(id))} onSource={value=>void command('open_source_url',{value})}/>);
  const newCase=()=>guard(()=>{setView('all');setQuery('');setTag('');void editor.begin(null,project).then(()=>requestAnimationFrame(()=>scrollArea.current?.scrollTo({top:0}))) });
  return <div className="library-root" style={styles} data-library-mode={editor.state?'editing':'browse'} data-draft-state={editor.state?.dirty?editor.state.draftError?'failed':editor.state.protectedSeq>=editor.state.seq?'protected':'pending':'none'}><main className="workspace">
    <header className={`topbar ${platform==='macos'?'mac-titlebar':''}`} data-tauri-drag-region><div className="brand" data-tauri-drag-region><span className="brandicon">✦</span>Prompt Shelf <span className="breadcrumb">/ 素材管理 / 图文资料库</span></div>
      <div className="top-actions">{drafts.length>0&&!editor.state&&<button type="button" className="btn sm" onClick={()=>void refreshDrafts().then(()=>setRecoveryOpen(true))}>未完成草稿 {drafts.length}</button>}<button type="button" className="btn primary sm" onClick={newCase}><Icon name="plus"/> 新建案例</button><button type="button" className="btn sm" onClick={()=>notice('回收站暂未开放')}><Icon name="archive"/> 回收站 <span id="trashBadge">{data.trashCount??'—'}</span></button><button type="button" className="btn sm" onClick={()=>setDictionaryOpen(true)}><Icon name="book"/> 搜索词典</button><button type="button" className="btn sm" title="备份与恢复" onClick={()=>notice('备份与恢复暂未开放')}><Icon name="backup" size={15}/> 备份与恢复</button>
        <span className="window-divider" aria-hidden="true"/><div className="window-controls" role="group" aria-label="窗口控制">
          <button type="button" className="window-btn" title="最小化窗口" aria-label="最小化窗口" onClick={()=>void command('window_action',{action:'minimize'})}><Icon name="minimize"/></button>
          <button type="button" className="window-btn" title="最大化/窗口化" aria-label="最大化或还原窗口" onClick={()=>void command('window_action',{action:'maximize'})}><Icon name="maximize"/></button>
          <button type="button" className="window-btn window-close" title="关闭窗口" aria-label="关闭窗口" onClick={()=>void command('close_library')}><Icon name="close"/></button>
        </div>
      </div>
    </header>
    <div className="body"><LibrarySidebar records={data.cases} view={view} setView={value=>guard(()=>setView(value))} expanded={expanded} toggle={toggle} quickTags={quickTags} activeTag={tag} setTag={chooseTag} notice={notice}/>
      <Splitter name="rail" width={railWidth} change={width=>setRailWidth(Math.max(172,Math.min(340,width)))}/>
      {expanded&&<><CatalogPanel key={expanded} kind={expanded} entries={expanded==='tags'?tags:projects} records={data.cases} active={expanded==='tags'?tag:project} choose={expanded==='tags'?chooseTag:chooseProject} notice={notice} canHandleEscape={()=>!globalEscape.current}/><Splitter name="secondary" width={secondaryWidth} change={width=>setSecondaryWidth(Math.max(278,Math.min(680,width)))}/></>}
      <section className="library" aria-label="案例资料库"><div className="libraryhead"><div className="headrow"><div className="heading-wrap"><div className="libtitle">{heading}</div><div className="libdesc">图文直览 · 原卡片内编辑 · 网格与瀑布流自由切换</div></div>
        <div className="head-controls v410-toolbar"><div className="view-toggle" role="group" aria-label="卡片布局">{(['regular','masonry'] as const).map(mode=><button type="button" key={mode} className={layout===mode?'active':''} aria-pressed={layout===mode} title={mode==='regular'?'常规网格：同排等高':'瀑布流：按列紧凑排列'} onClick={()=>setLayout(mode)}><Icon name={mode}/>{mode==='regular'?'常规':'瀑布流'}</button>)}</div><div className="sort-toggle" role="group" aria-label="全部案例排序">{([['recent','最近使用'],['name','按名称'],['new','最新创建']] as const).map(([value,label])=><button type="button" key={value} className={sort===value?'active':''} aria-pressed={sort===value} onClick={()=>setSort(value)}>{label}</button>)}</div></div>
      </div><div className="searchrow"><label className="global-search"><Icon name="search" size={15}/><input ref={search} value={query} onChange={event=>{const value=event.target.value;guard(()=>setQuery(value))}} placeholder="搜索案例标题、提示词正文、项目、标签…" aria-label="搜索案例" autoComplete="off"/></label></div>
      <div className="quick-filter-row"><div className="quickbar"><span className="qlabel">快捷标签</span><button type="button" className={`quicktag ${!tag?'active':''}`} onClick={()=>guard(()=>setTag(''))}>全部场景</button>{quickTags.map(item=><button type="button" key={item.id} className={`quicktag ${tag===item.name?'active':''}`} onClick={()=>chooseTag(item.name)}>{item.name}</button>)}</div><button type="button" className="btn sm toolbar-clear" title="清除搜索和筛选" onClick={()=>guard(()=>{setQuery('');setTag('');setProject('');setView('all')})}>清除筛选</button></div>
      <div className="statsrow"><b>{records.length} 条案例</b><span>{layout==='regular'?'常规网格 · 同排等高':'瀑布流 · 各列独立排列'}{tag&&` · 标签：${tag}`}{project&&` · 项目：${project}`}</span></div></div>
      <div className="scrollarea" ref={scrollArea}><div className={`gallery ${layout}`} data-columns={columns}>
        {cards.length?layout==='masonry'?Array.from({length:columns},(_,column)=><div className="masonry-column" key={column}>{cards.filter((_,index)=>index%columns===column)}</div>):cards:<div className="empty-results">{loadError?<><strong>资料库暂时无法读取</strong><br/>{loadError}</>:!loaded?'正在加载资料库…':data.cases.length?<>没有符合筛选条件的案例<br/>请调整搜索词、标签或项目。</>:'资料库暂无案例'}</div>}
      </div></div></section>
    </div>
  </main>
  {editor.confirmation&&<EditSafetyDialog canEscape={()=>!globalEscape.current} busy={editor.resolving||!!editor.state?.saving} resolve={choice=>void editor.resolve(choice)}/>}
  {recoveryOpen&&!editor.state&&!editor.confirmation&&<DraftRecoveryDialog drafts={drafts} busy={recoveryBusy} recover={(id,asNew)=>void recover(id,asNew)} discard={id=>void discardDraft(id)} later={()=>setRecoveryOpen(false)}/>}
  {toast&&<div className="library-toast" role="status" aria-live="polite">{toast}</div>}
  {dictionaryOpen&&<div className="dictionary-shade" onClick={()=>setDictionaryOpen(false)}><section className="dictionary-panel" role="dialog" aria-modal="true" aria-label="搜索词典" onClick={event=>event.stopPropagation()}><h2>领域近似词词典 · 规则预览</h2><p>这是独立于案例标签的搜索扩展规则。直接命中优先，同组近似表达也会参与搜索。</p><div className="dictionary-rows">{dictionary.map(group=><div style={{display:'contents'}} key={group[0]}><div><b>{group[0]}</b></div><div>{group.slice(1).join('、')}</div></div>)}</div><button type="button" className="btn primary" onClick={()=>setDictionaryOpen(false)}>关闭</button></section></div>}
  </div>;
}
