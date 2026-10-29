import {createContext,useContext,useEffect,useLayoutEffect,useMemo,useRef,useState} from 'react';
import {convertFileSrc,isTauri} from '@tauri-apps/api/core';
import {isCurrentLibraryGeneration,useLibraryInvoke,useRestoreWarning} from './api';
import {listen} from '@tauri-apps/api/event';
import {LatestRead,selectionAfterRefresh,retainSessionOrder} from './refresh';
import {dictionary,filterCases,highlightParts} from './search';
import {ReferenceCarousel} from './ReferenceCarousel';
import {progressRouting,rememberProgress,terminalPhase as terminal,wakeView} from './session';
import type {SavedView,SessionView} from './session';
import type {Asset,Case,Library,Progress,Tab} from './types';

const WindowVisible=createContext(false);
const SearchDictionary=createContext<readonly string[][]>(dictionary);
type Wake={tab:Tab;token:number;warning?:string|null;targetBound?:boolean};
type Boot={platform:string;logPath:string;pendingWake?:Wake|null;panelVisible?:boolean;targetBound?:boolean;escapeRegistered?:boolean;escapeError?:string|null};
const frame=()=>new Promise<void>(resolve=>requestAnimationFrame(()=>resolve()));
const tabs:[Tab,string][]=[['all','全部案例'],['recent','最近使用'],['fav','我的收藏']];
function Mark({text,query}:{text:string;query:string}) {return <>{highlightParts(text,query,useContext(SearchDictionary)).map((p,i)=>p.hit?<mark key={i} className="highlight promptmark">{p.text}</mark>:<span key={i}>{p.text}</span>)}</>}

// A button shell occupies the original geometry; only intersecting images mount.
function Thumb({asset,label,result,onClick,disabled}:{asset?:Asset;label:string;result?:boolean;onClick?:()=>void;disabled?:boolean}) {
  const windowVisible=useContext(WindowVisible);
  const element=useRef<HTMLButtonElement>(null);const [visible,setVisible]=useState(false);const [failed,setFailed]=useState(false);
  useEffect(()=>{setVisible(false);const node=element.current;if(!node||!windowVisible)return;const observer=new IntersectionObserver(entries=>setVisible(entries.some(x=>x.isIntersecting)),{root:null,rootMargin:'0px',threshold:0});observer.observe(node);return()=>observer.disconnect()},[windowVisible]);
  useEffect(()=>setFailed(false),[asset?.thumbnail]);
  const ratio=asset&&asset.height>0?asset.width/asset.height:1;
  return <button ref={element} data-asset-id={asset?.id} data-paste-kind={result?'result':'reference'} className={`photo ${result?'result':''} ${!asset?'placeholder':''}`} style={{width:result?90:Math.round(90*Math.max(2/3,Math.min(1.5,ratio)))}} disabled={disabled||!asset} onClick={onClick} title={asset?`${label} · 粘贴原图`:`${label} · 无图片`} aria-label={label}>
    {windowVisible&&visible&&asset&&!failed?<img data-asset-id={asset.id} src={convertFileSrc(asset.thumbnail)} alt={label} onError={()=>setFailed(true)}/>:<span className="thumb-placeholder">{failed?'缩略图加载失败':'▧'}</span>}<span className="piclabel">{label}</span>
  </button>;
}

export function App({library}:{library:boolean}) {
  const invoke=useLibraryInvoke();const restoreWarning=useRestoreWarning();
  const [panelVisible,setPanelVisible]=useState(false);
  const [targetBound,setTargetBound]=useState(false);
  const [data,setData]=useState<Library>({cases:[],assets:[]});const [loaded,setLoaded]=useState(false);
  const [query,setQuery]=useState('');const [tab,setTab]=useState<Tab>('all');const [tag,setTag]=useState('');const [project,setProject]=useState('');
  const [selected,setSelected]=useState(0);const [interval,setIntervalMs]=useState(700);const [intervalText,setIntervalText]=useState('700');const [progress,setProgress]=useState<Progress|null>(null);
  const [metadataError,setMetadataError]=useState('');const favoritePending=useRef(new Set<string>());const intervalEditing=useRef(false);const intervalSaving=useRef(false);
  const [busy,setBusy]=useState(false);const locked=useRef(false);const [message,setMessage]=useState('');const [dict,setDict]=useState(false);
  const activeRequest=useRef<number|null>(null);const pendingInteraction=useRef(false);const requestGeneration=useRef(0);
  const earlyProgress=useRef(new Map<number,Progress>());
  const libraryReads=useRef(new LatestRead());const refreshLibrary=useRef<()=>Promise<void>>(async()=>{});
  const pendingSelection=useRef<{id:string|null;index:number;context:string}|null>(null);const skipSelectionScroll=useRef(false);
  const liveWindowEvent=useRef(false);
  const globalEscape=useRef(false);const liveEscapeEvent=useRef(false);const [escapeError,setEscapeError]=useState<string|null>(null);
  const [queueCaseId,setQueueCaseId]=useState<string|null>(null);
  const [wake,setWake]=useState<Wake|null>(null);const [boot,setBoot]=useState<Boot>({platform:'',logPath:''});
  const [sort,setSort]=useState('recent');const [layout,setLayout]=useState('regular');const [catalog,setCatalog]=useState<'tags'|'projects'|null>(null);
  const search=useRef<HTMLInputElement>(null);const list=useRef<HTMLDivElement>(null);const readySequence=useRef(0);
  const railPositions=useRef(new Map<string,number>());
  const savedViews=useRef(new Map<Tab,SavedView>());const pendingScroll=useRef<number|null>(null);
  const sessionView=useRef<SessionView>({tab,query,tag,project,selected,scrollTop:0});
  sessionView.current={tab,query,tag,project,selected,scrollTop:pendingScroll.current??list.current?.scrollTop??0};
  const searchSequence=useRef(0);const searchSamples=useRef<{sequence:number;started:number}[]>([]);const [searchInputSequence,setSearchInputSequence]=useState(0);
  const sessionOrder=useRef<{key:string;ids:string[]}>({key:'',ids:[]});const [orderEpoch,setOrderEpoch]=useState(0);
  const assets=useMemo(()=>new Map(data.assets.map(a=>[a.id,a])),[data.assets]);
  const matches=useMemo(()=>{const fresh=filterCases(data.cases,query,tab,tag,project,sort,data.dictionary??dictionary);const key=JSON.stringify([query,tab,tag,project,sort,orderEpoch]);const ordered=!library&&sessionOrder.current.key===key?retainSessionOrder(fresh,sessionOrder.current.ids):fresh;sessionOrder.current={key,ids:ordered.map(record=>record.id)};return ordered},[data,query,tab,tag,project,sort,orderEpoch,library]);
  const items=useMemo(()=>library?matches:matches.slice(0,50),[library,matches]);
  const latestItems=useRef<Case[]>([]);latestItems.current=items;const latestItemContext=useRef('');latestItemContext.current=JSON.stringify([tab,query,tag,project]);
  useLayoutEffect(()=>{const snapshot=pendingSelection.current;if(!snapshot)return;pendingSelection.current=null;if(snapshot.context!==JSON.stringify([tab,query,tag,project]))return;const next=selectionAfterRefresh(items.map(record=>record.id),snapshot.id,snapshot.index);if(next!==sessionView.current.selected){skipSelectionScroll.current=true;setSelected(next)}},[items]);
  const tags=useMemo(()=>(data.tags??[]).filter(item=>item.isQuick).slice().sort((a,b)=>a.quickOrder-b.quickOrder||a.id.localeCompare(b.id)),[data.tags]);
  const projects=useMemo(()=>[...new Set(data.cases.map(c=>c.project).filter(Boolean))],[data]);
  const run=async(command:string,args?:Record<string,unknown>)=>{try{return await invoke(command,args)}catch(e){setMessage(String(e));return undefined}};
  const changeTab=(next:Tab,explicit=false)=>{
    if(explicit&&next===sessionView.current.tab){pendingSelection.current={id:latestItems.current[sessionView.current.selected]?.id??null,index:sessionView.current.selected,context:JSON.stringify([tab,query,tag,project])};pendingScroll.current??=list.current?.scrollTop??0;setOrderEpoch(value=>value+1);void refreshLibrary.current().catch(error=>setMessage(String(error)))}else if(next!==sessionView.current.tab){pendingSelection.current=null}
    const current={...sessionView.current,scrollTop:pendingScroll.current??list.current?.scrollTop??0};
    const nextView=wakeView(current,next,savedViews.current);
    if(nextView===current)return;
    sessionView.current=nextView;pendingScroll.current=nextView.scrollTop;
    setTab(nextView.tab);setSelected(nextView.selected);
  };
  useLayoutEffect(()=>{if(pendingScroll.current!==null&&list.current){list.current.scrollTop=pendingScroll.current;pendingScroll.current=null}},[tab,items]);
  const interruptForPanelInteraction=()=>{
    if(library||!locked.current)return;
    pendingInteraction.current=true;
    if(activeRequest.current!==null){pendingInteraction.current=false;void run('panel_interaction',{requestId:activeRequest.current})}
  };
  const acceptProgress=(p:Progress)=>{
    const route=progressRouting(activeRequest.current,locked.current,p);
    if(route==='defer'){rememberProgress(earlyProgress.current,p);return}
    if(route==='ignore')return;
    const active=!terminal(p.phase);
    if(active){activeRequest.current=p.requestId;if(pendingInteraction.current){pendingInteraction.current=false;void run('panel_interaction',{requestId:p.requestId})}}
    else{activeRequest.current=null;pendingInteraction.current=false}
    setProgress(p);locked.current=active;setBusy(active);setMessage(p.error??`${active?'队列执行中':p.phase==='cancelled'?'已停止':'执行结束'} · 已触发 ${p.done}/${p.total}（不代表目标接收）`);
  };
  const onSearchInput=(value:string)=>{
    const sequence=++searchSequence.current;
    searchSamples.current.push({sequence,started:performance.now()});
    setQuery(value);setSearchInputSequence(sequence);
  };
  useLayoutEffect(()=>{
    // This effect runs after the result DOM commit. Only actual input handlers
    // enqueue samples; tab/filter changes and programmatic resets do not.
    const samples=searchSamples.current.splice(0);const resultCount=items.length;
    for(const sample of samples)void(async()=>{
      await frame();await frame();
      const elapsedMs=performance.now()-sample.started;
      const superseded=sample.sequence!==searchSequence.current;
      if(isTauri())await run('search_ready',{elapsedMs,resultCount,sequence:sample.sequence,superseded});
    })();
  },[items,searchInputSequence]);
  useEffect(()=>{
    let disposed=false;const unlisten:(()=>void)[]=[];
    if(!isTauri()){setMessage('浏览器预览 · 系统粘贴、资源与窗口功能需启动 Tauri PoC');return}
    const refresh=async()=>{
      const token=libraryReads.current.begin();const before=sessionView.current;const selection={id:latestItems.current[before.selected]?.id??null,index:before.selected,context:JSON.stringify([before.tab,before.query,before.tag,before.project])};
      try{const catalog=await invoke<Library>('get_library_index');if(disposed||!libraryReads.current.accepts(token))return;
        const context=JSON.stringify([sessionView.current.tab,sessionView.current.query,sessionView.current.tag,sessionView.current.project]);const index=sessionView.current.selected;pendingSelection.current=selection.context===context&&latestItemContext.current===context?{id:latestItems.current[index]?.id??null,index,context}:null;pendingScroll.current??=list.current?.scrollTop??0;setData(previous=>({...catalog,assets:previous.assets}));if(!intervalEditing.current&&!intervalSaving.current&&catalog.uiPreferences){setIntervalMs(catalog.uiPreferences.quickIntervalMs);setIntervalText(String(catalog.uiPreferences.quickIntervalMs))}setTag(value=>value&&!(catalog.tags??[]).some(item=>item.id===value)?'':value);setProject(value=>value&&!(catalog.projects??[]).some(item=>item.id===value)?'':value);setLoaded(true);
      }catch(error){if(!disposed&&libraryReads.current.accepts(token))throw error}
    };
    refreshLibrary.current=refresh;
    const refreshQuietly=()=>{void refresh().catch(error=>setMessage(String(error)))};
    const attach=async()=>{
      try {
        const a=await listen<Wake>('wake',e=>{liveWindowEvent.current=true;setPanelVisible(true);changeTab(e.payload.tab);setWake(e.payload);if(typeof e.payload.targetBound==='boolean')setTargetBound(e.payload.targetBound);if(e.payload.warning)setMessage(e.payload.warning)});
        if(disposed){a();return}unlisten.push(a);
        const visibility=await listen<{visible:boolean}>('panel-visibility',e=>{if(!disposed&&!library){liveWindowEvent.current=true;setPanelVisible(e.payload.visible)}});
        if(disposed){visibility();return}unlisten.push(visibility);
        const esc=await listen<{registered:boolean;error:string|null}>('escape-status',e=>{if(!disposed){liveEscapeEvent.current=true;globalEscape.current=e.payload.registered;setEscapeError(e.payload.error)}});
        if(disposed){esc();return}unlisten.push(esc);
        const b=await listen<Progress&{generation:number}>('queue-progress',e=>{if(!disposed&&isCurrentLibraryGeneration(e.payload.generation))acceptProgress(e.payload)});
        if(disposed){b();return}unlisten.push(b);
        const writeFailed=await listen<{kind:string;error:string}>('library-write-failed',event=>setMetadataError(event.payload.error));if(disposed){writeFailed();return}unlisten.push(writeFailed);
        const changed=await listen('library-changed',refreshQuietly);if(disposed){changed();return}unlisten.push(changed);
        window.addEventListener('focus',refreshQuietly);
        const [,info]=await Promise.all([refresh(),invoke<typeof boot>('get_boot')]);
        if(!disposed){
          setBoot(info);
          if(!liveEscapeEvent.current){globalEscape.current=!!info.escapeRegistered;setEscapeError(info.escapeError??null)}
          // A delayed boot snapshot cannot overwrite an already handled wake,
          // rebind or explicit close. Pending wake only initializes a quiet UI.
          if(!liveWindowEvent.current){
            setTargetBound(!!info.targetBound);if(!library)setPanelVisible(!!info.panelVisible);
            if(info.pendingWake){const pending=info.pendingWake;setWake(pending);setPanelVisible(true);changeTab(pending.tab);if(pending.warning)setMessage(pending.warning)}
          }
          if(info.platform!=='windows')setMessage('非 Windows 平台的系统粘贴尚未实现；当前仅预留跨平台界面边界。')
        }
      }catch(error){if(!disposed)setMessage(String(error))}
    };void attach();return()=>{disposed=true;libraryReads.current.invalidate();unlisten.forEach(fn=>fn());window.removeEventListener('focus',refreshQuietly)};
  },[]);
  useEffect(()=>{if(!loaded||!isTauri())return;let disposed=false;void invoke<Asset[]>('get_case_assets',{caseIds:items.map(record=>record.id)}).then(incoming=>{if(!disposed)setData(previous=>({...previous,assets:[...new Map([...previous.assets,...incoming].map(asset=>[asset.id,asset])).values()]}))}).catch(error=>{if(!disposed)setMessage(String(error))});return()=>{disposed=true}},[loaded,JSON.stringify(items.map(record=>[record.id,record.result,record.refs]))]);
  const toggleFavorite=async(record:Case)=>{if(favoritePending.current.has(record.id))return;favoritePending.current.add(record.id);const favorite=!record.favorite;setData(previous=>({...previous,cases:previous.cases.map(item=>item.id===record.id?{...item,favorite}:item)}));try{await invoke('set_favorite',{caseId:record.id,favorite})}catch(error){setData(previous=>({...previous,cases:previous.cases.map(item=>item.id===record.id?{...item,favorite:record.favorite}:item)}));setMessage('收藏保存失败：'+String(error));void refreshLibrary.current().catch(()=>{})}finally{favoritePending.current.delete(record.id)}};
  const persistInterval=async()=>{intervalEditing.current=false;const value=Math.max(50,Math.min(10000,Number(intervalText)||data.uiPreferences?.quickIntervalMs||700));setIntervalMs(value);setIntervalText(String(value));intervalSaving.current=true;try{await invoke('set_ui_preferences',{patch:{quickIntervalMs:value}})}catch(error){setMessage('间隔保存失败：'+String(error))}finally{intervalSaving.current=false;void refreshLibrary.current().catch(()=>{})}};
  useEffect(()=>{
    if(!loaded||(!library&&(!wake||!panelVisible)))return;
    const sequence=++readySequence.current;
    void(async()=>{await frame();if(sequence!==readySequence.current||locked.current)return;search.current?.focus({preventScroll:true});await frame();await frame();
      if(sequence!==readySequence.current)return;
      const interactive=document.activeElement===search.current&&document.hasFocus();
      if(!interactive)setMessage('搜索框尚未真正获得窗口焦点，本次未记为可交互。请重新激活窗口。');
      await run('ready',{token:wake?.token??0,kind:library?'library':'quick',interactive});
    })();
    return()=>{readySequence.current++};
  },[loaded,wake,library,panelVisible]);
  useEffect(()=>{setSelected(0)},[query,tag,project]);
  useEffect(()=>{setSelected(index=>Math.max(0,Math.min(index,items.length-1)))},[items]);
  const paste=async(record:Case,kind:'text'|'result'|'reference'|'queue',index?:number)=>{
    if(locked.current){setMessage('已有粘贴请求执行中，请等待或按 Esc 取消');return}
    if(!isTauri()){setMessage('系统粘贴需启动 Tauri PoC');return}
    if(!targetBound){setMessage('请先激活目标输入框，再按 Ctrl+Alt+1 绑定；面板搜索与点击不会更换目标');return}
    const generation=++requestGeneration.current;
    readySequence.current++; // A pending wake rAF must not focus after paste starts.
    locked.current=true;activeRequest.current=null;pendingInteraction.current=false;earlyProgress.current.clear();setBusy(true);setQueueCaseId(kind==='queue'?record.id:null);setProgress(null);setMessage('正在准备粘贴请求…');
    try{const id=await invoke<number>('paste',{caseId:record.id,kind,index:index??null,intervalMs:interval,expectedRevision:record.editRevision??0});
      if(generation===requestGeneration.current&&locked.current){activeRequest.current=id;const early=earlyProgress.current.get(id);earlyProgress.current.clear();if(early){acceptProgress(early)}else if(pendingInteraction.current){pendingInteraction.current=false;void run('panel_interaction',{requestId:id})}}
    }
    catch(error){if(generation===requestGeneration.current){locked.current=false;activeRequest.current=null;pendingInteraction.current=false;setBusy(false);if(String(error).includes('case_changed_refresh_and_retry')){setMessage('案例已更新，请待列表刷新后重新选择并重试');void refreshLibrary.current().catch(next=>setMessage(String(next)))}else setMessage(String(error))}}
  };
  const cancel=async()=>{await run('cancel_queue')};
  const dismiss=async()=>{await run(library?'close_library':'hide_panel')};
  useEffect(()=>{
    const handle=(e:KeyboardEvent)=>{
      if(e.key==='Escape'){
        e.preventDefault();
        if(!library){void run('panel_escape',{pressed:true,repeat:e.repeat});return}
        if(globalEscape.current||e.repeat||e.isComposing)return;
        if(dict)setDict(false);else void dismiss();return
      }
      if(e.isComposing)return;
      if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='k'){e.preventDefault();search.current?.focus();search.current?.select();return}
      if(document.activeElement!==search.current)return;
      if(e.key==='ArrowDown'||e.key==='ArrowUp'){e.preventDefault();setSelected(i=>Math.max(0,Math.min(items.length-1,i+(e.key==='ArrowDown'?1:-1))));return}
      if(e.key==='Enter'&&!library){const record=items[selected];if(record){e.preventDefault();void paste(record,'text')}}
    };
    const release=(e:KeyboardEvent)=>{if(e.key==='Escape'&&!library){e.preventDefault();void run('panel_escape',{pressed:false,repeat:false})}};
    document.addEventListener('keydown',handle);document.addEventListener('keyup',release);
    return()=>{document.removeEventListener('keydown',handle);document.removeEventListener('keyup',release)};
  },[items,selected,interval,dict,library,targetBound]);
  useEffect(()=>{if(skipSelectionScroll.current){skipSelectionScroll.current=false;return}list.current?.querySelector('[data-selected="true"]')?.scrollIntoView({block:'nearest'})},[selected]);
  const nav=<>{tabs.map(([id,name],i)=><button key={id} className={`${library?'navitem':'tab'} ${tab===id?'active':''}`} title={`Ctrl + Alt + ${i+1}；点击刷新该集合排序`} onClick={()=>changeTab(id,true)}>{name}{library&&<span className="count">{data.cases.filter(r=>id==='all'||(id==='fav'?r.favorite:r.used>0)).length}</span>}</button>)}</>;
  const searchbox=<label className={library?'global-search':'search-wrap'}><span className="mag">⌕</span><input ref={search} value={query} onChange={e=>onSearchInput(e.target.value)} placeholder="搜索案例名称、提示词、项目、标签…" autoComplete="off" aria-label="搜索案例"/><button className="clear" title="清空关键词" onClick={()=>{setQuery('');search.current?.focus()}}>×</button></label>;
  const card=(r:Case,i:number)=>{
    const p=queueCaseId===r.id&&progress?.caseId===r.id?progress:null;const active=p&&!terminal(p.phase);const complete=p&&p.done===p.total&&p.total>0;
    return <article key={r.id} className={`case ${selected===i?'selected':''}`} data-case-id={r.id} data-selected={selected===i} onMouseDown={()=>{if(selected!==i)skipSelectionScroll.current=true;setSelected(i)}}>
      <div className="casehead"><span className={library?'case-title':'title'} title={r.title}><Mark text={r.title} query={query}/></span><span className={library?'case-project':'project'} title={r.project}>· <Mark text={r.project||'独立案例'} query={query}/></span>{r.tags.slice(0,3).map(t=><span key={t} className={library?'tag-chip':'tag'}><Mark text={t} query={query}/></span>)}<button type="button" className={`star ${r.favorite?'on':''}`} title={r.favorite?'取消收藏':'收藏案例'} aria-label={`${r.favorite?'取消收藏':'收藏'} ${r.title}`} aria-pressed={r.favorite} onClick={event=>{event.stopPropagation();void toggleFavorite(r)}}>{r.favorite?'★':'☆'}</button></div>
      {(r.result||r.refs.length>0)&&<div className="imgline"><Thumb result asset={r.result?assets.get(r.result):undefined} label={r.result?'效果图':'暂无效果'} disabled={library||busy} onClick={()=>void paste(r,'result')}/>{r.refs.length>0&&<><span className="divide"/>{!library&&<button data-paste-kind="queue" data-case-id={r.id} data-request-id={p?.requestId} data-phase={p?.phase} data-done={p?.done} data-total={p?.total} data-current-asset={p?.currentAsset??undefined} className={`queue ${active?'progress':''} ${complete?'complete':''}`} disabled={busy} onClick={()=>void paste(r,'queue')} aria-label={`顺序粘贴 ${r.refs.length} 张参考图`}><span className="fill" style={{height:`${p&&p.total>0?Math.min(100,p.done/p.total*100):0}%`}}/><span className="ins"><span className="symbol">{complete?'✓':'⇧'}</span><span>{active?<>正在<br/>粘贴</>:complete?<>已<br/>触发</>:<>顺序<br/>粘贴</>}</span><span>{p?`${p.done}/${p.total}`:`${r.refs.length}张`}</span></span></button>}<ReferenceCarousel caseId={r.id} visible={library||panelVisible} positions={railPositions.current} imageKey={JSON.stringify(r.refs.map(id=>[id,assets.get(id)?.width,assets.get(id)?.height]))}>{r.refs.map((id,index)=><Thumb key={`${id}-${index}`} asset={assets.get(id)} label={`参考 ${index+1}`} disabled={library||busy} onClick={()=>void paste(r,'reference',index)}/>)}</ReferenceCarousel></>}</div>}
      {library?<div className="prompt"><Mark text={r.prompt} query={query}/></div>:<button className="prompt" data-paste-kind="text" data-case-id={r.id} disabled={busy} onClick={()=>void paste(r,'text')} title="粘贴完整提示词"><span className="clamp"><Mark text={r.prompt} query={query}/></span></button>}
      {library&&<div className="casefoot">{r.refs.length} 张参考图 · 只读 PoC</div>}
    </article>;
  };
  const content=<div ref={list} className={library?'scrollarea':'list'}><div className={library?`gallery ${layout}`:''}>{items.length?items.map(card):<div className="none empty-results"><strong>{loaded?'没有找到匹配的案例':'等待加载资料库'}</strong><div>{loaded?'尝试减少关键词或切换至全部案例。':message||'由 Rust 读取 JSON manifest。'}</div></div>}</div></div>;
  return <SearchDictionary.Provider value={data.dictionary??dictionary}><WindowVisible.Provider value={library||panelVisible}><div onPointerDownCapture={interruptForPanelInteraction} onInputCapture={interruptForPanelInteraction} onKeyDownCapture={event=>{if(event.key!=='Escape')interruptForPanelInteraction()}} data-target-bound={targetBound} data-panel-visible={library||panelVisible} data-request-id={progress?.requestId} data-phase={progress?.phase} data-current-asset={progress?.currentAsset??undefined} data-event-timestamp={progress?.timestampMs} className={library?'library-root':'quick-root'}>
    <section className="panel" aria-label="提示词快捷调用面板"><header className={`bar ${boot.platform==='macos'?'mac-titlebar':''}`}><span className="logo">✦</span><span className="brand" data-tauri-drag-region>Prompt Shelf</span><small>提示词速用 · PoC</small><span className="bar-end"><button className="library-link" onClick={()=>void run('open_library')}>打开主库</button><kbd>Ctrl + Alt + {tab==='recent'?2:tab==='fav'?3:1}</kbd><button className="x" aria-label="隐藏面板" onClick={()=>void dismiss()}>×</button></span></header><div className="controls">{searchbox}<div className="nav">{nav}<span className="line"/>{tags.slice(0,3).map(t=><button key={t.id} className={`category ${tag===t.id?'active':''}`} onClick={()=>setTag(tag===t.id?'':t.id)}>{t.name}</button>)}<span className="count">{matches.length>items.length?`前 ${items.length} / ${matches.length}`:items.length} 条</span></div></div>{content}<div className="queue-settings"><label>图间隔 <input type="number" min={50} max={10000} step={100} value={intervalText} disabled={busy} onFocus={()=>{intervalEditing.current=true}} onBlur={()=>void persistInterval()} onChange={e=>{setIntervalText(e.target.value);const value=Number(e.target.value);if(value>=50&&value<=10000)setIntervalMs(value)}}/> ms</label>{busy&&<button className="cancel-queue" onClick={()=>void cancel()}>Esc 停止</button>}<span title="外部输入框按 Ctrl+Alt+1/2/3 可重新绑定；搜索和点击面板不换目标">{targetBound?'目标已绑定':'未绑定目标'} · ↑↓ 选择 · Enter 全文</span></div><div className="legal" role="status" aria-live="polite">{busy&&<span className="paste-spinner" aria-hidden="true"/>}{restoreWarning&&<span>{restoreWarning}<br/></span>}{escapeError||metadataError||message||'点击提示词或图片粘贴至唤醒前目标 · 队列进度表示已触发指令，不代表目标已接收'}</div></section>
    {dict&&<div className="demo-dialog" onClick={()=>setDict(false)}><section className="demo-dialog-panel" role="dialog" aria-modal="true" aria-label="搜索词典" onClick={e=>e.stopPropagation()}><h2>搜索词典</h2><p>多词 AND；标题权重高于提示词正文。同组词双向匹配。</p><div className="demo-dict">{(data.dictionary??dictionary).map(group=><div key={group[0]}>{group.join(' / ')}</div>)}</div><button className="btn" onClick={()=>setDict(false)}>关闭</button></section></div>}
  </div></WindowVisible.Provider></SearchDictionary.Provider>;
}
