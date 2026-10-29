import {useEffect,useRef,useState} from 'react';
import {listen} from '@tauri-apps/api/event';
import {useLibraryInvoke} from '../api';
import type {LibraryContext} from '../api';
import '../library-e.css';

interface Summary {cases:number;trash:number;tags:number;projects:number;drafts:number}
interface RestorePreview {token:string;fileName:string;createdAt:number;current:Summary;incoming:Summary;generation:number;revision:number}
interface BackupProgress {operationId:string;phase:string;bytesDone:number;bytesTotal:number;filesDone:number;filesTotal:number}
const fields=[['cases','正常案例'],['trash','回收站案例'],['tags','标签'],['projects','项目']] as const;
function Counts({current,incoming}:{current:Summary;incoming?:Summary}){
  return <div className="v50-list backup-counts">{fields.map(([key,label])=><div key={key}><strong>{current[key]}{incoming&&<> <span className="backup-count-arrow" aria-hidden="true">→</span> {incoming[key]}</>}</strong><span>{label}</span></div>)}</div>;
}
function bytes(value:number){if(value<1024)return `${value} B`;const unit=value<1024*1024?'KB':value<1024*1024*1024?'MB':'GB';const divisor=unit==='KB'?1024:unit==='MB'?1024*1024:1024*1024*1024;return `${(value/divisor).toFixed(1)} ${unit}`}
function phase(value?:string){switch(value){case 'snapshot':return '正在生成一致性快照';case 'hashing':return '正在校验备份内容';case 'archive':case 'packing':case 'exporting':case 'writing':return '正在导出完整备份';case 'extract':case 'extracting':return '正在读取备份文件';case 'validate':case 'validating':case 'inspecting':return '正在检查备份内容';case 'publishing':return '正在完成备份，请稍候';case 'committing':return '正在切换资料库，请稍候';case 'restoring':case 'switching':return '正在恢复完整资料库';default:return '正在处理备份文件'}}
function suggestedBackupName(){const now=new Date();const part=(value:number)=>String(value).padStart(2,'0');return `PromptShelf-backup-${now.getFullYear()}${part(now.getMonth()+1)}${part(now.getDate())}-${part(now.getHours())}${part(now.getMinutes())}${part(now.getSeconds())}.zip`}
function explainError(text:string){
  if(text.includes('backup_publish_failed'))return '备份未保存。目标文件已存在时不会覆盖；请改用新文件名，并确认保存位置可写。';
  if(text.includes('insufficient_space')||text.includes('No space left')||text.includes('disk full'))return '磁盘可用空间不足，请释放空间或选择其他磁盘后重试。';
  if(text.includes('future_or_unknown_schema')||text.includes('future_schema'))return '备份的资料库版本高于当前程序或不受支持，请使用对应版本的程序恢复。';
  if(text.includes('backup_original_hash_mismatch'))return '当前资料库的原图校验失败，备份未完成。请先检查原图文件。';
  if(text.includes('hash_mismatch')||text.includes('file_size_mismatch')||text.includes('actual_bytes_exceed')||text.includes('declared_size_mismatch'))return '备份文件校验失败，内容可能损坏或不完整，无法恢复。请重新导出或选择其他备份。';
  if(text.includes('missing')||text.includes('stored_file_missing'))return '备份或当前资料库缺少必要文件，请检查数据库、清单及原图是否完整。';
  if(text.includes('structure')||text.includes('foreign_keys')||text.includes('integrity')||text.includes('asset_set')||text.includes('file_set')||text.includes('unreferenced_assets'))return '备份的数据库结构或图片引用不完整，无法恢复。请选择其他完整备份。';
  if(text.includes('zip_')||text.includes('manifest')||text.includes('entry_')||text.includes('compression')||text.includes('central_directory')||text.includes('format_not_supported'))return '这份 ZIP 备份格式无效、损坏或不受支持，无法读取。请重新选择完整备份文件。';
  if(text.includes('image_')||text.includes('asset_metadata')||text.includes('asset_path')||text.includes('metadata_invalid')||text.includes('summary_mismatch')||text.includes('settings_not_supported'))return '备份中的图片或配置资料不符合要求，无法恢复。请重新导出完整备份。';
  if(text.includes('restore_preview_expired'))return '恢复预览已失效，请重新选择备份文件。';
  if(text.includes('restore_initialization_failed_rolled_back'))return '恢复后的资料库未能初始化，已切回上一份资料库。';
  if(text.includes('stale_library_generation'))return '资料库已经切换，请在刷新后的窗口重新操作。';
  if(text.includes('backup_job_already_running')||text.includes('maintenance'))return '另一个备份或恢复任务正在进行，请等待完成后重试。';
  if(text.includes('backup_snapshot_busy_retry'))return '资料库正忙，暂时无法生成备份，请稍后重试。';
  return '操作未完成，请核对文件和保存位置后重试。';
}

export function BackupDialog({beforeExport,requestRestore,onClose,canEscape,suspended,onBusyChange}:{beforeExport:()=>Promise<boolean>;requestRestore:(action:()=>void,stay:()=>void)=>void;onClose:()=>void;canEscape:()=>boolean;suspended:boolean;onBusyChange:(busy:boolean)=>void}){
  const invoke=useLibraryInvoke();const panel=useRef<HTMLElement>(null);
  const [current,setCurrent]=useState<Summary|null>(null);const [preview,setPreview]=useState<RestorePreview|null>(null);
  const [busy,setBusy]=useState<'export'|'inspect'|'guard'|'restore'|null>(null);const [progress,setProgress]=useState<BackupProgress|null>(null);
  const [error,setError]=useState('');const [message,setMessage]=useState('');const [canceling,setCanceling]=useState(false);const [committing,setCommitting]=useState(false);const [releasing,setReleasing]=useState(false);
  const operation=useRef<string|null>(null);const alive=useRef(true);const previewToken=useRef<string|null>(null);previewToken.current=preview?.token??null;
  useEffect(()=>{onBusyChange(busy!==null||releasing);return()=>onBusyChange(false)},[busy,releasing]);
  const refreshSummary=async()=>{const next=await invoke<Summary>('backup_summary');if(alive.current)setCurrent(next)};
  useEffect(()=>{alive.current=true;let stop:(()=>void)|undefined;void listen<BackupProgress>('backup-progress',event=>{if(alive.current&&event.payload.operationId===operation.current){setProgress(event.payload);if(event.payload.phase==='committing'||event.payload.phase==='publishing'){setCommitting(true);setCanceling(false)}}}).then(value=>{if(alive.current)stop=value;else value()}).catch(value=>{if(alive.current)setError(`${explainError(String(value))}\n详情：${String(value)}`)});void refreshSummary().catch(value=>{if(alive.current)setError(`${explainError(String(value))}\n详情：${String(value)}`)});return()=>{alive.current=false;stop?.();const token=previewToken.current;previewToken.current=null;if(token)void invoke('backup_release_preview',{token}).catch(()=>{})}},[]);
  useEffect(()=>{if(suspended)return;const previous=document.activeElement as HTMLElement|null;panel.current?.focus();return()=>{if(previous?.isConnected)previous.focus({preventScroll:true})}},[suspended,!!preview]);
  const start=(kind:'export'|'inspect'|'restore')=>{const id=crypto.randomUUID();operation.current=id;setBusy(kind);setProgress(null);setError('');setMessage('');setCanceling(false);setCommitting(false);return id};
  const finish=()=>{operation.current=null;if(alive.current){setBusy(null);setCanceling(false);setCommitting(false)}};
  const failure=(value:unknown)=>{if(!alive.current)return;const text=String(value);if(text.includes('backup_commit_already_started')){setCommitting(true);setCanceling(false)}else if(text.includes('cancelled')||text.includes('canceled'))setMessage('操作已取消，当前资料库未被替换。');else setError(`${explainError(text)}\n详情：${text}`)};
  const exportBackup=async()=>{
    if(busy)return;
    const id=start('export');
    try{if(!await beforeExport()){setError('当前草稿尚未完成保护，请按编辑提示处理后重试。');return}const result=await invoke<{destination:string}|null>('backup_export',{operationId:id,suggestedName:suggestedBackupName()});if(result&&alive.current){setMessage(`完整备份已导出：${result.destination}`);await refreshSummary()}}
    catch(value){failure(value)}finally{finish()}
  };
  const inspect=async()=>{
    if(busy)return;const id=start('inspect');
    try{const result=await invoke<RestorePreview|null>('backup_inspect',{operationId:id});if(result&&alive.current){setPreview(result);setCurrent(result.current)}}catch(value){failure(value)}finally{finish()}
  };
  const restore=async(selected:RestorePreview)=>{
    const id=start('restore');
    try{await invoke<LibraryContext>('backup_restore',{token:selected.token,expectedRevision:selected.revision,operationId:id})}
    catch(value){if(String(value).includes('restore_preview_changed')){try{const next=await invoke<RestorePreview>('backup_refresh_preview',{token:selected.token});if(alive.current){setPreview(next);setCurrent(next.current);setError('当前资料库已变化，数量已更新，请重新核对并确认整体恢复。')}}catch(next){failure(next)}}else failure(value)}finally{finish()}
  };
  const confirm=()=>{if(busy||!preview)return;setBusy('guard');requestRestore(()=>{void restore(preview)},()=>setBusy(null))};
  const cancel=async()=>{const id=operation.current;if(!id||canceling||committing)return;setCanceling(true);try{await invoke('backup_cancel',{operationId:id})}catch(value){failure(value);setCanceling(false)}};
  const releasePreview=async()=>{if(busy||releasing||!preview)return;const token=preview.token;setReleasing(true);setError('');try{await invoke('backup_release_preview',{token});if(alive.current){previewToken.current=null;setPreview(null);setMessage('')}}catch(value){failure(value)}finally{if(alive.current)setReleasing(false)}};
  if(suspended)return null;
  const summary=preview?.current??current;
  return <div className="v50-shade"><section ref={panel} tabIndex={-1} className="v50-modal backup-modal" role="dialog" aria-modal="true" aria-label={preview?'确认恢复这份完整备份？':'备份与恢复'} onKeyDown={event=>{
    if(event.key==='Escape'){event.preventDefault();event.stopPropagation();if(canEscape()&&!busy&&!releasing){if(preview)void releasePreview();else onClose()}}
    if(event.key==='Tab'){const controls=[...event.currentTarget.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];const index=controls.indexOf(document.activeElement as HTMLButtonElement);if(!controls.length)event.preventDefault();else if(event.shiftKey&&index<=0){event.preventDefault();controls.at(-1)?.focus()}else if(!event.shiftKey&&(index<0||index===controls.length-1)){event.preventDefault();controls[0].focus()}}
  }}>
    <h3>{preview?'确认恢复这份完整备份？':'备份与恢复'}</h3>
    <p>{preview?'先核对当前与备份的内容数量，确认后才会替换当前资料库。':'建议定期导出一份完整素材库副本，妥善保存到其他磁盘或电脑。'}</p>
    {preview&&<div className="v50-summary"><b>备份日期：</b>{new Date(preview.createdAt).toLocaleString('zh-CN')}<br/><b>文件：</b>{preview.fileName}</div>}
    {summary?<><Counts current={summary} incoming={preview?.incoming}/><div className="v50-muted backup-drafts">{preview?<>四项数量为“当前 → 备份”。草稿：当前 {summary.drafts} 份，备份 {preview.incoming.drafts} 份。</>:<>当前资料库含 {summary.drafts} 份草稿。导出会包含已落盘草稿。</>}</div></>:<div className="v50-summary">正在读取资料库数量…</div>}
    {preview?<div className="v50-warning">这是整体恢复，不是合并：当前资料库将被替换。上一份完整库及其草稿会保留作回退副本，建议先导出当前备份。</div>:<div className="v50-summary">完整备份包含案例提示词、来源链接、效果图与参考图原图、回收站、草稿、项目、标签、快捷标签顺序、搜索词典及必要设置。</div>}
    {busy&&busy!=='guard'&&<div className="backup-progress" role="status" aria-live="polite"><div>{phase(committing?(busy==='export'?'publishing':'committing'):progress?.phase)}</div><progress max={progress?.bytesTotal||1} value={progress?.bytesDone||0}/><span>{bytes(progress?.bytesDone??0)} / {bytes(progress?.bytesTotal??0)} · {progress?.filesDone??0} / {progress?.filesTotal??0} 个文件</span></div>}
    {error&&<div className="v50-warning backup-error" role="alert">{error}</div>}
    {message&&<div className="v50-positive" role="status">{message}</div>}
    <div className="v50-actions">{busy?<button type="button" className="btn" disabled={busy==='guard'||canceling||committing} onClick={()=>void cancel()}>{committing?(busy==='export'?'正在完成备份，请稍候':'正在切换资料库，请稍候'):canceling?'正在取消…':'取消操作'}</button>:preview?<><button type="button" className="btn" disabled={releasing} onClick={()=>void releasePreview()}>取消恢复</button><button type="button" className="btn primary" disabled={releasing} onClick={confirm}>确认整体恢复</button></>:<><button type="button" className="btn" onClick={onClose}>暂不操作</button><button type="button" className="btn" disabled={!current} onClick={()=>void inspect()}>从备份恢复…</button><button type="button" className="btn primary" disabled={!current} onClick={()=>void exportBackup()}>导出完整备份</button></>}</div>
  </section></div>;
}
