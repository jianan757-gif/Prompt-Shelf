import {useEffect,useRef} from 'react';
import type {DraftSummary} from './editor';
export function EditSafetyDialog({busy,resolve,canEscape}:{busy:boolean;canEscape:()=>boolean;resolve:(choice:'save'|'discard'|'continue')=>void}){
  const panel=useRef<HTMLElement>(null);
  useEffect(()=>{const previous=document.activeElement as HTMLElement|null;panel.current?.focus();return()=>{if(previous?.isConnected)previous.focus({preventScroll:true})}},[]);
  return <div className="v50-shade"><section ref={panel} tabIndex={-1} className="v50-modal" role="dialog" aria-modal="true" aria-label="当前案例尚未保存" onKeyDown={event=>{if(event.key==='Escape'&&canEscape()){event.preventDefault();event.stopPropagation();if(!busy)resolve('continue')}if(event.key==='Tab'){const buttons=[...event.currentTarget.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];if(!buttons.length){event.preventDefault();return}const index=buttons.indexOf(document.activeElement as HTMLButtonElement);if(event.shiftKey&&index<=0){event.preventDefault();buttons.at(-1)?.focus()}else if(!event.shiftKey&&(index<0||index===buttons.length-1)){event.preventDefault();buttons[0].focus()}}}}>
    <h3>当前案例尚未保存</h3><p>离开编辑之前，要如何处理现在的修改？</p><div className="v50-warning">保存成功后才会继续。放弃会清除本次修改及自动保护草稿，正式内容保持上次保存的版本。</div>
    <div className="v50-actions"><button type="button" className="btn" disabled={busy} onClick={()=>resolve('continue')}>继续编辑</button><button type="button" className="btn" disabled={busy} onClick={()=>resolve('discard')}>放弃修改</button><button type="button" className="btn primary" disabled={busy} onClick={()=>resolve('save')}>保存并继续</button></div>
  </section></div>;
}
export function DraftRecoveryDialog({drafts,busy,recover,discard,later}:{drafts:DraftSummary[];busy:boolean;recover:(id:string,asNew:boolean)=>void;discard:(id:string)=>void;later:()=>void}){
  const panel=useRef<HTMLElement>(null);useEffect(()=>{panel.current?.querySelector<HTMLButtonElement>('button')?.focus()},[]);
  const draft=drafts[0];if(!draft)return null;
  const unavailable=draft.targetStatus==='missing'||draft.targetStatus==='deleted';
  return <div className="v50-shade"><section ref={panel} className="v50-modal" role="dialog" aria-modal="true" aria-label="发现未完成的案例草稿" onKeyDown={event=>{if(event.key==='Tab'){const buttons=[...event.currentTarget.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];const index=buttons.indexOf(document.activeElement as HTMLButtonElement);if(!buttons.length){event.preventDefault()}else if(event.shiftKey&&index<=0){event.preventDefault();buttons.at(-1)?.focus()}else if(!event.shiftKey&&index===buttons.length-1){event.preventDefault();buttons[0].focus()}}}}>
    <h3>发现未完成的案例草稿</h3><p>恢复仅继续编辑，不会覆盖正式案例。还有 {drafts.length} 份未完成草稿。</p>
    <div className="v50-summary"><b>{draft.title||'未命名案例'}</b><div>提示词：{draft.prompt||'（尚未填写）'}</div><div>参考图 {draft.referenceCount} 张 · 效果图 {draft.resultCount} 张</div><span className="v50-muted">暂存于 {new Date(draft.updatedAt).toLocaleString('zh-CN')}</span></div>
    {unavailable&&<div className="v50-warning">原案例不存在或已在回收站。请先恢复原案例，或明确选择作为新草稿继续。</div>}
    {draft.conflict&&<div className="v50-warning">正式案例版本已经变化。恢复将保留草稿原版本，保存时会拒绝覆盖；也可作为新草稿继续。</div>}
    <div className="v50-actions"><button type="button" className="btn" disabled={busy} onClick={later}>稍后处理</button><button type="button" className="btn" disabled={busy} onClick={()=>discard(draft.id)}>放弃草稿</button>{(unavailable||draft.conflict)&&<button type="button" className="btn" disabled={busy} onClick={()=>recover(draft.id,true)}>作为新草稿</button>}{!unavailable&&<button type="button" className="btn primary" disabled={busy} onClick={()=>recover(draft.id,false)}>恢复并继续编辑</button>}</div>
  </section></div>;
}
