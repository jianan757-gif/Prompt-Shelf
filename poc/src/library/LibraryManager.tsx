import {useEffect,useMemo,useRef,useState} from 'react';
import type {CSSProperties,PointerEvent} from 'react';
import {isTauri} from '@tauri-apps/api/core';
import {useLibraryInvoke,useRestoreWarning} from '../api';
import {listen} from '@tauri-apps/api/event';
import {dictionary,score} from '../search';
import {Icon} from './Icons';
import {CaseCard} from './CaseCard';
import {TrashCard} from './TrashCard';
import {TrashDialog} from './TrashDialogs';
import type {TrashDecision} from './TrashDialogs';
import {CaseEditor} from './CaseEditor';
import {EditSafetyDialog,DraftRecoveryDialog} from './EditSafetyDialogs';
import type {DraftSummary} from './editor';
import {useCaseEditor} from './useCaseEditor';
import {LatestRead} from '../refresh';
import {LibrarySidebar} from './LibrarySidebar';
import type {CatalogKind} from './LibrarySidebar';
import {CatalogPanel} from './CatalogPanel';
import {DictionaryEditor} from './DictionaryEditor';
import {BackupDialog} from './BackupDialog';
import '../library-c.css';
import '../library-d.css';
import {catalogs,columnCount,libraryCases} from './model';
import type {CaseSort,Layout} from './model';
import type {Asset,Case,Library,Tab,TrashCase,UiPreferences} from '../types';

type Boot={platform:string;escapeRegistered?:boolean};
const defaults:UiPreferences={libraryLayout:'regular',librarySort:'recent',catalogColumns:'single',tagSort:'time',projectSort:'time',railWidth:224,secondaryWidth:320,expandedCatalog:null,quickIntervalMs:700};
function Splitter({name,width,change,commit}:{name:'rail'|'secondary';width:number;change:(width:number)=>void;commit:(width:number)=>void}){
  const drag=useRef<{x:number;width:number;next:number}|null>(null);const [active,setActive]=useState(false);
  const begin=(event:PointerEvent<HTMLDivElement>)=>{event.preventDefault();event.currentTarget.setPointerCapture(event.pointerId);drag.current={x:event.clientX,width,next:width};setActive(true)};
  const finish=(cancel=false)=>{const current=drag.current;if(!current)return;drag.current=null;setActive(false);if(cancel)change(current.width);else commit(current.next)};
  return <div className={`splitter ${active?'dragging':''}`} role="separator" aria-label={name==='rail'?'左侧导航宽度':'二级栏宽度'} aria-orientation="vertical" aria-valuenow={width} tabIndex={0} title={name==='rail'?'拖动调整左侧导航宽度':'拖动调整二级栏宽度'} onPointerDown={begin} onPointerMove={event=>{if(drag.current){drag.current.next=drag.current.width+event.clientX-drag.current.x;change(drag.current.next)}}} onPointerUp={()=>finish()} onPointerCancel={()=>finish(true)} onLostPointerCapture={()=>finish(true)} onKeyDown={event=>{if(event.key==='ArrowLeft'||event.key==='ArrowRight'){event.preventDefault();commit(width+(event.key==='ArrowRight'?10:-10))}}}/>;
}
/** Native library; persistent draft state is owned by the editor and Rust. */
export function LibraryManager(){
  const invoke=useLibraryInvoke();const restoreWarning=useRestoreWarning();
  const [data,setData]=useState<Library>({cases:[],assets:[]});const [loaded,setLoaded]=useState(false);const [loadError,setLoadError]=useState('');
  const [platform,setPlatform]=useState('');const [view,setView]=useState<Tab>('all');const [query,setQuery]=useState('');const [tag,setTag]=useState('');const [project,setProject]=useState('');
  const [expanded,setExpanded]=useState<CatalogKind|null>(null);const [layout,setLayout]=useState<Layout>('regular');const [sort,setSort]=useState<CaseSort>('recent');
  const [railWidth,setRailWidth]=useState(224);const [secondaryWidth,setSecondaryWidth]=useState(320);const [columns,setColumns]=useState(1);
  const [drafts,setDrafts]=useState<DraftSummary[]>([]);const [recoveryOpen,setRecoveryOpen]=useState(false);const [recoveryBusy,setRecoveryBusy]=useState(false);
  const [preferences,setPreferences]=useState<UiPreferences>(defaults);const prefCurrent=useRef(defaults);const confirmedPreferences=useRef(defaults);const preferenceQueue=useRef(Promise.resolve());const pendingPreferences=useRef(0);
  const [assetCache,setAssetCache]=useState(new Map<string,Asset>());const assetReads=useRef(new LatestRead());const cachedCaseVersions=useRef(new Map<string,string>());const [renderLimit,setRenderLimit]=useState(60);const loadMore=useRef<HTMLDivElement>(null);
  const [favoriteOverrides,setFavoriteOverrides]=useState(new Map<string,boolean>());const favoriteInFlight=useRef(new Set<string>());const catalogInFlight=useRef(false);
  const [dictionaryOpen,setDictionaryOpen]=useState(false);const [backupOpen,setBackupOpen]=useState(false);const [backupBusy,setBackupBusy]=useState(false);const [toast,setToast]=useState('');
  const [trashOpen,setTrashOpen]=useState(false);const [trashCases,setTrashCases]=useState<TrashCase[]>([]);const [trashDecision,setTrashDecision]=useState<TrashDecision|null>(null);const [trashBusy,setTrashBusy]=useState(false);const trashInFlight=useRef(false);const [trashError,setTrashError]=useState('');
  const [undo,setUndo]=useState<{caseId:string;title:string}|null>(null);const undoTimer=useRef<ReturnType<typeof setTimeout>|null>(null);
  const toastTimer=useRef<ReturnType<typeof setTimeout>|null>(null);const globalEscape=useRef(false);const liveEscape=useRef(false);
  const reads=useRef(new LatestRead());const refreshLibrary=useRef<()=>Promise<void>>(async()=>{});
  const search=useRef<HTMLInputElement>(null);const scrollArea=useRef<HTMLDivElement>(null);const positions=useRef(new Map<string,number>());
  const tags=useMemo(()=>catalogs(data,'tags'),[data]);const projects=useMemo(()=>catalogs(data,'projects'),[data]);
  const quickTags=useMemo(()=>tags.filter(item=>item.isQuick).slice().sort((a,b)=>a.quickOrder-b.quickOrder||a.id.localeCompare(b.id)),[tags]);
  const cases=useMemo(()=>data.cases.map(record=>favoriteOverrides.has(record.id)?{...record,favorite:favoriteOverrides.get(record.id)!}:record),[data.cases,favoriteOverrides]);
  const groups=data.dictionary??dictionary;
  const records=useMemo(()=>trashOpen?trashCases.filter(record=>score({...record,project:record.deletedProjectName||record.project},query,groups)>=0):libraryCases(cases,query,view,tag,project,sort,groups),[cases,trashCases,trashOpen,query,view,tag,project,sort,groups]);
  const visibleRecords=records.slice(0,renderLimit);
  const notice=(message:string)=>{setToast(message);if(toastTimer.current)clearTimeout(toastTimer.current);toastTimer.current=setTimeout(()=>setToast(''),2200)};
  const command=async(name:string,args?:Record<string,unknown>)=>{try{await invoke(name,args)}catch(error){notice(String(error))}};
  const editor=useCaseEditor(notice,async()=>{await refreshLibrary.current();await refreshDrafts()},backupOpen);
  const assets=useMemo(()=>new Map([...assetCache.values(),...data.assets,...(editor.state?.images??[])].map(asset=>[asset.id,asset])),[assetCache,data.assets,editor.state?.images]);
  const guard=(action:()=>void)=>editor.request(action);
  const applyPreferences=(next:UiPreferences)=>{prefCurrent.current=next;setPreferences(next);setLayout(next.libraryLayout);setSort(next.librarySort);setRailWidth(next.railWidth);setSecondaryWidth(next.secondaryWidth);setExpanded(next.expandedCatalog)};
  const updatePreference=(patch:Partial<UiPreferences>)=>{if(!loaded){notice('资料库加载后再调整显示偏好');return}applyPreferences({...prefCurrent.current,...patch});pendingPreferences.current++;preferenceQueue.current=preferenceQueue.current.catch(()=>{}).then(async()=>{try{await invoke('set_ui_preferences',{patch});confirmedPreferences.current={...confirmedPreferences.current,...patch}}catch(error){const rollback=Object.fromEntries(Object.keys(patch).filter(key=>prefCurrent.current[key as keyof UiPreferences]===patch[key as keyof UiPreferences]).map(key=>[key,confirmedPreferences.current[key as keyof UiPreferences]])) as Partial<UiPreferences>;applyPreferences({...prefCurrent.current,...rollback});notice(`显示偏好保存失败，已恢复：${String(error)}`)}finally{pendingPreferences.current--;if(!pendingPreferences.current)await refreshLibrary.current().catch(error=>notice(String(error)))}})};
  const mutateCatalog=(name:string,args:Record<string,unknown>)=>new Promise<boolean>(resolve=>{if(catalogInFlight.current){notice('请等待当前分类操作完成');resolve(false);return}catalogInFlight.current=true;const done=(value:boolean)=>{catalogInFlight.current=false;resolve(value)};editor.request(()=>{void(async()=>{let committed=false;try{await invoke(name,{...args});committed=true;await refreshLibrary.current();await refreshDrafts();done(true)}catch(error){notice(committed?`分类操作已保存，列表刷新失败：${String(error)}`:String(error));done(committed)}})()},()=>done(false))});
  const favorite=async(record:Case)=>{if(favoriteInFlight.current.has(record.id))return;favoriteInFlight.current.add(record.id);setFavoriteOverrides(previous=>new Map(previous).set(record.id,!record.favorite));let committed=false;try{await invoke('set_favorite',{caseId:record.id,favorite:!record.favorite});committed=true;setData(previous=>({...previous,cases:previous.cases.map(item=>item.id===record.id?{...item,favorite:!record.favorite}:item)}));await refreshLibrary.current()}catch(error){notice(committed?`收藏已保存，列表刷新失败：${String(error)}`:`收藏保存失败：${String(error)}`)}finally{favoriteInFlight.current.delete(record.id);setFavoriteOverrides(previous=>{const next=new Map(previous);next.delete(record.id);return next})}};
  const saveDictionary=async(next:string[][])=>{let committed=false;try{await invoke('save_dictionary',{groups:next});committed=true;await refreshLibrary.current();return true}catch(error){notice(committed?`词典已保存，列表刷新失败：${String(error)}`:String(error));return committed}};
  const clearUndo=()=>{if(undoTimer.current)clearTimeout(undoTimer.current);undoTimer.current=null;setUndo(null)};
  const showUndo=(record:{id:string;title:string})=>{clearUndo();setUndo({caseId:record.id,title:record.title});undoTimer.current=setTimeout(()=>{setUndo(null);undoTimer.current=null},4500)};
  const mutateTrash=async(name:string,caseId:string,title:string)=>{if(trashInFlight.current)return false;trashInFlight.current=true;setTrashBusy(true);setTrashError('');let committed=false;try{await invoke(name,{caseId});committed=true;if(name==='soft_delete_case')showUndo({id:caseId,title});else if(undo?.caseId===caseId)clearUndo();setTrashDecision(null);await refreshLibrary.current();await refreshDrafts();if(name==='restore_case')notice('案例已恢复，提示词和图片关联完整保留');if(name==='permanently_delete_case')notice('案例已永久删除，仍被使用的图片继续保留');return true}catch(error){const message=committed?`操作已保存，列表刷新失败：${String(error)}`:String(error);setTrashError(message);notice(message);return committed}finally{trashInFlight.current=false;setTrashBusy(false)}};
  const requestDelete=(record:Case,kind:'soft'|'permanent')=>guard(()=>{if(trashInFlight.current)return;trashInFlight.current=true;setTrashBusy(true);setTrashError('');void(async()=>{let deleteNow=false;try{const draftId=await invoke<string|null>('get_case_delete_draft',{caseId:record.id});if(draftId)setTrashDecision({caseId:record.id,title:record.title,kind,step:'draft',draftId});else if(kind==='permanent')setTrashDecision({caseId:record.id,title:record.title,kind,step:'permanent'});else deleteNow=true}catch(error){setTrashError(String(error));notice(String(error))}finally{trashInFlight.current=false;setTrashBusy(false)}if(deleteNow)await mutateTrash('soft_delete_case',record.id,record.title)})()});
  const restore=(record:{id:string;title:string})=>guard(()=>void mutateTrash('restore_case',record.id,record.title));
  const discardDeleteDraft=async()=>{const decision=trashDecision;if(!decision?.draftId||trashInFlight.current)return;trashInFlight.current=true;setTrashBusy(true);setTrashError('');try{await invoke('discard_case_draft',{draftId:decision.draftId});await refreshDrafts();if(decision.kind==='permanent')setTrashDecision({...decision,step:'permanent',draftId:undefined});else{trashInFlight.current=false;await mutateTrash('soft_delete_case',decision.caseId,decision.title)}}catch(error){setTrashError(String(error))}finally{trashInFlight.current=false;setTrashBusy(false)}};
  const recoverDeleteDraft=()=>{const decision=trashDecision;if(!decision?.draftId||trashBusy)return;setTrashDecision(null);void recover(decision.draftId,decision.kind==='permanent')};
  const wanted=visibleRecords.slice();if(editor.state&&!wanted.some(record=>record.id===editor.state!.document.id)){const record=data.cases.find(item=>item.id===editor.state!.document.id);if(record)wanted.push(record)}
  const assetKey=JSON.stringify([trashOpen,wanted.map(record=>[record.id,record.editRevision,record.result,record.refs])]);
  useEffect(()=>{let disposed=false;const token=assetReads.current.begin();const cacheId=(id:string)=>`${trashOpen?'trash':'live'}:${id}`;const pending=wanted.filter(record=>cachedCaseVersions.current.get(cacheId(record.id))!==JSON.stringify([record.editRevision,record.result,record.refs]));if(pending.length)void(async()=>{try{const batches:Case[][]=[];for(let i=0;i<pending.length;i+=120)batches.push(pending.slice(i,i+120));const results=await Promise.all(batches.map(batch=>invoke<Asset[]>(trashOpen?'get_trash_assets':'get_case_assets',{caseIds:batch.map(record=>record.id)})));if(disposed||!assetReads.current.accepts(token))return;setAssetCache(previous=>new Map([...previous,...results.flat().map(asset=>[asset.id,asset] as const)]));pending.forEach(record=>cachedCaseVersions.current.set(cacheId(record.id),JSON.stringify([record.editRevision,record.result,record.refs])))}catch(error){if(!disposed)notice(String(error))}})();return()=>{disposed=true;assetReads.current.invalidate()}},[assetKey]);
  useEffect(()=>{setRenderLimit(60)},[query,view,trashOpen,tag,project,sort]);
  useEffect(()=>{const sentinel=loadMore.current;if(!sentinel||renderLimit>=records.length)return;const observer=new IntersectionObserver(entries=>{if(entries.some(entry=>entry.isIntersecting))setRenderLimit(previous=>Math.min(records.length,previous+60))},{root:scrollArea.current,rootMargin:'180px'});observer.observe(sentinel);return()=>observer.disconnect()},[records.length,renderLimit]);
  const editorLive=useRef(editor);editorLive.current=editor;
  const closeRequest=useRef<(request:{token:number;action:string})=>void>(()=>{});
  closeRequest.current=request=>{if(backupBusy){notice('备份或恢复正在进行，请先取消操作或等待完成');void command('cancel_library_close',{token:request.token});return}editorLive.current.request(()=>void command('finish_library_close',{token:request.token}),()=>void command('cancel_library_close',{token:request.token}))};
  const refreshDrafts=async()=>{const rows=await invoke<DraftSummary[]>('list_case_drafts');setDrafts(rows);return rows};
  const recover=async(id:string,asNew:boolean)=>{setRecoveryBusy(true);try{if(await editor.recover(id,asNew)){setRecoveryOpen(false);setTrashOpen(false);setQuery('');setTag('');setProject('');setView('all');await refreshLibrary.current();const legacy=await invoke<Library>('get_library');setAssetCache(previous=>new Map([...previous,...legacy.assets.map(asset=>[asset.id,asset] as const)]));await refreshDrafts();requestAnimationFrame(()=>document.querySelector(`[data-case-id="${editorLive.current.state?.document.id}"]`)?.scrollIntoView({block:'nearest'}))}}catch(error){notice(String(error))}finally{setRecoveryBusy(false)}};
  const discardDraft=async(id:string)=>{setRecoveryBusy(true);try{await invoke('discard_case_draft',{draftId:id});if(!(await refreshDrafts()).length)setRecoveryOpen(false)}catch(error){notice(String(error))}finally{setRecoveryBusy(false)}};
  useEffect(()=>{
    let disposed=false;const unlisten:(()=>void)[]=[];
    if(!isTauri()){setLoadError('请通过桌面程序打开资料库');return}
    const refresh=async()=>{const token=reads.current.begin();const [library,deleted]=await Promise.all([invoke<Library>('get_library_index'),invoke<TrashCase[]>('get_trash_index')]);if(!disposed&&reads.current.accepts(token)){setData(library);setTrashCases(deleted);if(library.uiPreferences&&!pendingPreferences.current){confirmedPreferences.current=library.uiPreferences;applyPreferences(library.uiPreferences)}if(library.tags)setTag(previous=>previous&&!library.tags!.some(item=>item.id===previous)?'':previous);if(library.projects)setProject(previous=>previous&&!library.projects!.some(item=>item.id===previous)?'':previous);setLoaded(true);setLoadError('')}};
    refreshLibrary.current=refresh;
    const refreshQuietly=()=>{void refresh().catch(error=>{if(!disposed)notice(String(error))})};
    void(async()=>{
      try{
        const stop=await listen<{registered:boolean}>('escape-status',event=>{liveEscape.current=true;globalEscape.current=event.payload.registered});if(disposed){stop();return}unlisten.push(stop);
        const closing=await listen<{token:number;action:string}>('edit-close-request',event=>closeRequest.current(event.payload));if(disposed){closing();return}unlisten.push(closing);
        const pending=await invoke<{token:number;action:string}|null>('pending_library_close');if(pending)closeRequest.current(pending);
        const failedWrite=await listen<{kind:string;error:string}>('library-write-failed',event=>notice(`调用已派发，但最近使用记录保存失败：${event.payload.error}`));if(disposed){failedWrite();return}unlisten.push(failedWrite);
        const changed=await listen('library-changed',refreshQuietly);if(disposed){changed();return}unlisten.push(changed);
        window.addEventListener('focus',refreshQuietly);
        const [,boot]=await Promise.all([refresh(),invoke<Boot>('get_boot')]);if(disposed)return;
        const hasRecovery=(await refreshDrafts()).length>0;if(hasRecovery)setRecoveryOpen(true);
        setPlatform(boot.platform);if(!liveEscape.current)globalEscape.current=!!boot.escapeRegistered;
        requestAnimationFrame(()=>{if(disposed||!document.hasFocus())return;if(!hasRecovery)search.current?.focus({preventScroll:true});void command('ready',{token:0,kind:'library',interactive:document.hasFocus()})});
      }catch(error){if(!disposed)setLoadError(String(error))}
    })();
    return()=>{disposed=true;reads.current.invalidate();unlisten.forEach(stop=>stop());window.removeEventListener('focus',refreshQuietly);if(toastTimer.current)clearTimeout(toastTimer.current);if(undoTimer.current)clearTimeout(undoTimer.current)};
  },[]);
  useEffect(()=>{
    const area=scrollArea.current;if(!area)return;
    const observer=new ResizeObserver(entries=>{const next=columnCount(entries[0]?.contentRect.width??0);setColumns(previous=>previous===next?previous:next)});observer.observe(area);return()=>observer.disconnect();
  },[]);
  useEffect(()=>{
    const key=(event:KeyboardEvent)=>{
      if(event.key==='Escape'){
        event.preventDefault();if(globalEscape.current||event.repeat||event.isComposing)return;
        if(editorLive.current.confirmation){void editorLive.current.resolve('continue');return}if(backupOpen)return;if(trashDecision){if(!trashBusy)setTrashDecision(null);return}if(recoveryOpen){setRecoveryOpen(false);return}if(dictionaryOpen)setDictionaryOpen(false);else if(editorLive.current.busy())editorLive.current.cancel();else void command('close_library');return;
      }
      if(event.isComposing||editorLive.current.confirmation||recoveryOpen||dictionaryOpen||trashDecision||backupOpen)return;
      if(((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='k')||(event.key==='/'&&!['INPUT','TEXTAREA'].includes((event.target as HTMLElement).tagName))){event.preventDefault();search.current?.focus();search.current?.select()}
    };
    document.addEventListener('keydown',key);return()=>document.removeEventListener('keydown',key);
  },[dictionaryOpen,recoveryOpen,trashDecision,trashBusy,backupOpen]);
  const chooseTag=(name:string)=>guard(()=>{setTrashOpen(false);setTag(previous=>previous===name?'':name)});
  const chooseProject=(name:string)=>guard(()=>{setTrashOpen(false);setProject(previous=>previous===name?'':name)});
  const toggle=(kind:CatalogKind)=>updatePreference({expandedCatalog:expanded===kind?null:kind});
  const projectName=projects.find(item=>item.id===project)?.name??'';const tagName=tags.find(item=>item.id===tag)?.name??'';
  const heading=trashOpen?'回收站':projectName||tagName||(view==='fav'?'收藏的案例':view==='recent'?'最近使用':'我的素材资料库');
  const styles={'--rail-w':`${railWidth}px`,'--sub-w':`${secondaryWidth}px`} as CSSProperties;
  const cardIds=visibleRecords.map(record=>record.id);if(editor.state&&!cardIds.includes(editor.state.document.id))cardIds.unshift(editor.state.document.id);
  const cards=cardIds.map(id=>editor.state?.document.id===id?<CaseEditor key={id} editor={editor} assets={assets} positions={positions.current} projects={projects}/>:trashOpen?<TrashCard key={id} record={trashCases.find(record=>record.id===id)!} query={query} assets={assets} dictionary={groups} busy={trashBusy} onRestore={()=>restore(trashCases.find(record=>record.id===id)!)} onPermanent={()=>requestDelete(trashCases.find(record=>record.id===id)!,'permanent')}/>:<CaseCard key={id} record={records.find(record=>record.id===id)!} query={query} assets={assets} positions={positions.current} notice={notice} onEdit={()=>guard(()=>void editor.begin(id))} onSource={value=>void command('open_source_url',{value})} onFavorite={()=>void favorite(records.find(record=>record.id===id)!)} onDelete={()=>requestDelete(records.find(record=>record.id===id)!,'soft')} favoritePending={favoriteOverrides.has(id)} dictionary={groups}/>);
  const newCase=()=>guard(()=>{setTrashOpen(false);setView('all');setQuery('');setTag('');void editor.begin(null,projectName).then(()=>requestAnimationFrame(()=>scrollArea.current?.scrollTo({top:0}))) });
  return <div className="library-root" style={styles} data-library-mode={editor.state?'editing':'browse'} data-draft-state={editor.state?.dirty?editor.state.draftError?'failed':editor.state.protectedSeq>=editor.state.seq?'protected':'pending':'none'}><main className={`workspace ${trashOpen?'trash-view':''}`}>
    <header className={`topbar ${platform==='macos'?'mac-titlebar':''}`} data-tauri-drag-region><div className="brand" data-tauri-drag-region><span className="brandicon">✦</span>Prompt Shelf <span className="breadcrumb">/ 素材管理 / 图文资料库</span></div>
      <div className="top-actions">{drafts.length>0&&!editor.state&&<button type="button" className="btn sm" onClick={()=>void refreshDrafts().then(()=>setRecoveryOpen(true)).catch(error=>notice(String(error)))}>未完成草稿 {drafts.length}</button>}<button type="button" className="btn primary sm" onClick={newCase}><Icon name="plus"/> 新建案例</button><button type="button" className={`btn sm ${trashOpen?'active':''}`} aria-pressed={trashOpen} onClick={()=>guard(()=>{setTrashOpen(!trashOpen);setQuery('');setTag('');setProject('')})}><Icon name="archive"/> 回收站 <span id="trashBadge">{data.trashCount??'—'}</span></button><button type="button" className="btn sm" onClick={()=>setDictionaryOpen(true)}><Icon name="book"/> 搜索词典</button><button type="button" className="btn sm" title="备份与恢复" onClick={()=>setBackupOpen(true)}><Icon name="backup" size={15}/> 备份与恢复</button>
        <span className="window-divider" aria-hidden="true"/><div className="window-controls" role="group" aria-label="窗口控制">
          <button type="button" className="window-btn" title="最小化窗口" aria-label="最小化窗口" onClick={()=>void command('window_action',{action:'minimize'})}><Icon name="minimize"/></button>
          <button type="button" className="window-btn" title="最大化/窗口化" aria-label="最大化或还原窗口" onClick={()=>void command('window_action',{action:'maximize'})}><Icon name="maximize"/></button>
          <button type="button" className="window-btn window-close" title="关闭窗口" aria-label="关闭窗口" onClick={()=>void command('close_library')}><Icon name="close"/></button>
        </div>
      </div>
    </header>
    <div className="body"><LibrarySidebar records={cases} view={view} setView={value=>guard(()=>{setTrashOpen(false);setView(value)})} expanded={expanded} toggle={toggle} quickTags={quickTags} activeTag={tag} setTag={chooseTag} reorderQuick={ids=>void mutateCatalog('reorder_quick_tags',{ids})}/>
      <Splitter name="rail" width={railWidth} change={width=>setRailWidth(Math.max(172,Math.min(340,width)))} commit={width=>updatePreference({railWidth:Math.max(172,Math.min(340,width))})}/>
      {expanded&&<><CatalogPanel key={expanded} kind={expanded} entries={expanded==='tags'?tags:projects} records={cases} active={expanded==='tags'?tag:project} choose={expanded==='tags'?chooseTag:chooseProject} notice={notice} canHandleEscape={()=>!globalEscape.current&&!editor.confirmation&&!dictionaryOpen&&!recoveryOpen&&!trashDecision} mutate={mutateCatalog} sort={expanded==='tags'?preferences.tagSort:preferences.projectSort} columns={preferences.catalogColumns} onPreference={patch=>updatePreference(patch as Partial<UiPreferences>)}/><Splitter name="secondary" width={secondaryWidth} change={width=>setSecondaryWidth(Math.max(278,Math.min(680,width)))} commit={width=>updatePreference({secondaryWidth:Math.max(278,Math.min(680,width))})}/></>}
      <section className="library" aria-label="案例资料库"><div className="libraryhead"><div className="headrow"><div className="heading-wrap"><div className="libtitle">{heading}</div><div className="libdesc">{trashOpen?'完整保留提示词与关联图片 · 恢复或永久删除':'图文直览 · 原卡片内编辑 · 网格与瀑布流自由切换'}</div></div>
        <div className="head-controls v410-toolbar"><div className="view-toggle" role="group" aria-label="卡片布局">{(['regular','masonry'] as const).map(mode=><button type="button" key={mode} className={layout===mode?'active':''} aria-pressed={layout===mode} title={mode==='regular'?'常规网格：同排等高':'瀑布流：按列紧凑排列'} onClick={()=>updatePreference({libraryLayout:mode})}><Icon name={mode}/>{mode==='regular'?'常规':'瀑布流'}</button>)}</div>{!trashOpen&&<div className="sort-toggle" role="group" aria-label="全部案例排序">{([['recent','最近使用'],['name','按名称'],['new','最新创建']] as const).map(([value,label])=><button type="button" key={value} className={sort===value?'active':''} aria-pressed={sort===value} onClick={()=>updatePreference({librarySort:value})}>{label}</button>)}</div>}</div>
      </div><div className="searchrow"><label className="global-search"><Icon name="search" size={15}/><input ref={search} value={query} onChange={event=>{const value=event.target.value;guard(()=>setQuery(value))}} placeholder={trashOpen?'搜索已删除案例':'搜索案例标题、提示词正文、项目、标签…'} aria-label={trashOpen?'搜索已删除案例':'搜索案例'} autoComplete="off"/></label></div>
      {!trashOpen&&<div className="quick-filter-row"><div className="quickbar"><span className="qlabel">快捷标签</span><button type="button" className={`quicktag ${!tag?'active':''}`} onClick={()=>guard(()=>setTag(''))}>全部场景</button>{quickTags.map(item=><button type="button" key={item.id} className={`quicktag ${tag===item.id?'active':''}`} onClick={()=>chooseTag(item.id)}>{item.name}</button>)}</div><button type="button" className="btn sm toolbar-clear" title="清除搜索和筛选" onClick={()=>guard(()=>{setQuery('');setTag('');setProject('');setView('all')})}>清除筛选</button></div>}
      <div className="statsrow"><b>{records.length} {trashOpen?'个已删除案例':'条案例'}</b><span>{trashOpen?'恢复后回到资料库；永久删除不可撤销':layout==='regular'?'常规网格 · 同排等高':'瀑布流 · 各列独立排列'}{!trashOpen&&tagName&&` · 标签：${tagName}`}{!trashOpen&&projectName&&` · 项目：${projectName}`}</span></div>{restoreWarning&&<div className="v50-warning" role="status">{restoreWarning}</div>}{trashError&&!trashDecision&&<div className="trash-error-banner" role="alert"><span>{trashError}</span><button type="button" className="iconbutton" aria-label="关闭操作错误提示" onClick={()=>setTrashError('')}><Icon name="close" size={14}/></button></div>}</div>
      <div className="scrollarea" ref={scrollArea}><div className={`gallery ${layout}`} data-columns={columns}>
        {cards.length?layout==='masonry'?Array.from({length:columns},(_,column)=><div className="masonry-column" key={column}>{cards.filter((_,index)=>index%columns===column)}</div>):cards:<div className="empty-results">{loadError?<><strong>资料库暂时无法读取</strong><br/>{loadError}</>:!loaded?'正在加载资料库…':trashOpen?trashCases.length?'没有符合搜索条件的已删除案例':'回收站为空':data.cases.length?<>没有符合筛选条件的案例<br/>请调整搜索词、标签或项目。</>:'资料库暂无案例'}</div>}
      </div>{renderLimit<records.length&&<div ref={loadMore} className="library-load-more" role="status">正在加载更多案例…</div>}</div></section>
    </div>
  </main>
  {editor.confirmation&&<EditSafetyDialog canEscape={()=>!globalEscape.current} busy={editor.resolving||!!editor.state?.saving} resolve={choice=>void editor.resolve(choice)}/>}
  {recoveryOpen&&!editor.state&&!editor.confirmation&&!trashDecision&&<DraftRecoveryDialog drafts={drafts} busy={recoveryBusy} recover={(id,asNew)=>void recover(id,asNew)} discard={id=>void discardDraft(id)} later={()=>setRecoveryOpen(false)}/>}
  {trashDecision&&<TrashDialog decision={trashDecision} busy={trashBusy} error={trashError} canEscape={()=>!globalEscape.current} onClose={()=>setTrashDecision(null)} onDiscardDraft={()=>void discardDeleteDraft()} onRecoverDraft={recoverDeleteDraft} onDelete={()=>guard(()=>void mutateTrash('permanently_delete_case',trashDecision.caseId,trashDecision.title))}/>}
  {undo?<div className="library-toast trash-undo" role="status" aria-live="polite"><span>案例已移入回收站</span><button type="button" disabled={trashBusy} aria-label={`撤销删除 ${undo.title}`} onClick={()=>restore({id:undo.caseId,title:undo.title})}>撤销</button></div>:toast&&<div className="library-toast" role="status" aria-live="polite">{toast}</div>}
  {dictionaryOpen&&<DictionaryEditor groups={groups} onSave={saveDictionary} onClose={()=>setDictionaryOpen(false)} canHandleEscape={()=>!globalEscape.current}/>}
  {backupOpen&&<BackupDialog beforeExport={editor.flushForBackup} requestRestore={(action,stay)=>editor.request(action,stay)} onClose={()=>setBackupOpen(false)} canEscape={()=>!globalEscape.current} suspended={editor.confirmation} onBusyChange={setBackupBusy}/>}

  </div>;
}
