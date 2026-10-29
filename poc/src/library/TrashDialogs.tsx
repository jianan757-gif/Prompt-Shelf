import {useEffect,useRef} from 'react';
export interface TrashDecision {caseId:string;title:string;kind:'soft'|'permanent';step:'draft'|'permanent';draftId?:string}
export function TrashDialog({decision,busy,error,canEscape,onClose,onDiscardDraft,onRecoverDraft,onDelete}:{decision:TrashDecision;busy:boolean;error:string;canEscape:()=>boolean;onClose:()=>void;onDiscardDraft:()=>void;onRecoverDraft:()=>void;onDelete:()=>void}){
  const panel=useRef<HTMLElement>(null);const draft=decision.step==='draft';
  useEffect(()=>{const previous=document.activeElement as HTMLElement|null;panel.current?.focus();return()=>{if(previous?.isConnected)previous.focus({preventScroll:true})}},[decision.step]);
  const title=draft?'先处理未完成草稿':'永久删除案例？';
  return <div className="v50-shade"><section ref={panel} tabIndex={-1} className="v50-modal" role="dialog" aria-modal="true" aria-label={title} onKeyDown={event=>{if(event.key==='Escape'&&canEscape()){event.preventDefault();event.stopPropagation();if(!busy)onClose()}if(event.key==='Tab'){const buttons=[...event.currentTarget.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];const index=buttons.indexOf(document.activeElement as HTMLButtonElement);if(!buttons.length){event.preventDefault()}else if(event.shiftKey&&index<=0){event.preventDefault();buttons.at(-1)?.focus()}else if(!event.shiftKey&&(index<0||index===buttons.length-1)){event.preventDefault();buttons[0].focus()}}}}>
    <h3>{title}</h3><p>案例「{decision.title}」</p>
    {draft?<><div className="v50-warning">此案例还有未完成草稿。请先继续编辑或明确放弃草稿，再{decision.kind==='soft'?'移入回收站':'永久删除'}。放弃草稿会清除草稿中的修改及图片关联。</div><div className="v50-actions"><button type="button" className="btn" disabled={busy} onClick={onClose}>取消</button><button type="button" className="btn" disabled={busy} onClick={onDiscardDraft}>放弃草稿并继续</button><button type="button" className="btn primary" disabled={busy} onClick={onRecoverDraft}>{decision.kind==='permanent'?'作为新草稿继续':'恢复并继续编辑'}</button></div></>:<><div className="v50-warning">此操作不可撤销。案例内容及图片关联将被移除；其他案例、回收站案例或草稿仍在使用的原图会保留。</div><div className="v50-actions"><button type="button" className="btn" disabled={busy} onClick={onClose}>取消</button><button type="button" className="btn danger" disabled={busy} onClick={onDelete}>确认永久删除</button></div></>}
    {error&&<p className="trash-error" role="alert">{error}</p>}
  </section></div>;
}
