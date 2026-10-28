import {useEffect,useRef,useState} from 'react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {getCurrentWebview} from '@tauri-apps/api/webview';
import {attachImages,hasClipboardImage,ImportAuthority,moveReference,parseTags,removeImage} from './editor';
import {DraftClock,contentKey} from './draftSafety';
import type {CaseDocument,EditReceipt,ImportedImage,Slot} from './editor';

export interface EditorState extends EditReceipt {tagsText:string;selected:Slot;pending:boolean;saving:boolean;canceling?:boolean;images:ImportedImage[];dirty:boolean;seq:number;protectedSeq:number;draftError:string;baseline:string}
export function useCaseEditor(notice:(text:string)=>void,onSaved:(id:string)=>Promise<void>){
  const [state,setState]=useState<EditorState|null>(null);const current=useRef<EditorState|null>(null);const opening=useRef(false);const authority=useRef(new ImportAuthority());
  const [confirmation,setConfirmation]=useState(false);const [resolving,setResolving]=useState(false);const resolvingRef=useRef(false);const continuation=useRef<{action:()=>void;stay?:()=>void}|null>(null);
  const clock=useRef<DraftClock|null>(null);const stashRef=useRef<()=>Promise<boolean>>(async()=>false);
  if(!clock.current)clock.current=new DraftClock(()=>{void stashRef.current()});
  const set=(next:EditorState|null)=>{current.current=next;setState(next)};
  const mutate=(next:EditorState,immediate=false)=>{const dirty=contentKey(next.document,next.tagsText)!==next.baseline;set({...next,dirty,seq:next.seq+1,draftError:''});if(dirty||next.protectedSeq>=0){if(immediate)clock.current!.flush();else clock.current!.schedule()}};
  const change=(document:CaseDocument)=>{const active=current.current;if(active&&!active.saving)mutate({...active,document})};
  const install=(receipt:EditReceipt,project?:string,recovered=false)=>{const document={...receipt.document,project:project??receipt.document.project};const tagsText=document.tags.join('，');authority.current.begin(receipt.sessionId);const baseline=recovered?'recovered-draft':contentKey(receipt.document,tagsText);set({...receipt,document,tagsText,selected:'append',pending:false,saving:false,images:[],dirty:recovered||contentKey(document,tagsText)!==baseline,seq:recovered?(receipt.saveSeq??0):(contentKey(document,tagsText)!==baseline?1:0),protectedSeq:recovered?(receipt.saveSeq??0):-1,draftError:'',baseline});if(!recovered&&contentKey(document,tagsText)!==baseline)clock.current!.schedule()};
  const stash=async()=>{const active=current.current;if(!active)return false;if(!active.dirty&&active.protectedSeq<0)return true;
    const snapshot={...active.document,tags:parseTags(active.tagsText)};const seq=active.seq||1;
    try{const ack=await invoke<number>('save_case_draft',{sessionId:active.sessionId,document:snapshot,saveSeq:seq});const latest=current.current;if(latest?.sessionId===active.sessionId)set({...latest,protectedSeq:Math.max(latest.protectedSeq,ack),draftError:''});return true}
    catch(error){const message=String(error);const latest=current.current;if(latest?.sessionId===active.sessionId&&!message.includes('stale_draft_seq')){set({...latest,draftError:message});notice('草稿暂存失败，请保持窗口并重试：'+message)}return message.includes('stale_draft_seq')}
  };stashRef.current=stash;
  const busy=()=>current.current!==null||opening.current;
  const begin=async(id:string|null,project='')=>{
    if(busy()){notice('请先保存或取消当前案例');return}opening.current=true;
    try{const receipt=await invoke<EditReceipt>('begin_case_edit',{caseId:id});install(receipt,id?undefined:project);}
    catch(error){notice(String(error))}finally{opening.current=false}
  };
  const select=(slot:Slot)=>{const active=current.current;if(active&&!active.saving)set({...active,selected:slot})};
  const remove=(slot:Slot)=>{const active=current.current;if(!active||active.saving)return;authority.current.invalidate(slot);mutate({...active,document:removeImage(active.document,slot),selected:active.selected===slot?'append':active.selected},true)};
  const reorder=(from:string,to:string)=>{const active=current.current;if(!active||active.saving)return;const document=moveReference(active.document,from,to);mutate({...active,document,selected:'append'},true);if(document!==active.document)notice('已调整参考图顺序，请核对提示词中的图号（未保存）')};
  const importImages=async(slot?:Slot,paths?:string[],clipboard=false)=>{
    const active=current.current;if(!active)return;if(active.pending||active.saving){notice('请等待当前图片操作完成');return}
    const target=slot??active.selected;if(target!=='append'&&paths&&paths.length>1){notice('替换图片时请选择一个文件');return}
    const ticket=authority.current.ticket(target);set({...active,pending:true});
    try{
      const images=await invoke<ImportedImage[]>('import_case_images',{sessionId:active.sessionId,paths:paths??null,clipboard,multiple:target==='append'});
      const latest=current.current;if(!latest||latest.saving||!authority.current.accepts(ticket))return;
      const document=attachImages(latest.document,target,images);
      if(images.length)mutate({...latest,document,images:[...latest.images,...images],pending:false,selected:'append'},true);else set({...latest,pending:false});
      if(images.length)notice(document===latest.document?'目标图片已不存在，本次导入未附加':'图片已加入当前编辑，尚未正式保存');
    }catch(error){if(current.current?.sessionId===active.sessionId)notice(String(error))}
    finally{const latest=current.current;if(latest?.sessionId===active.sessionId)set({...latest,pending:false})}
  };
  const discard=async()=>{
    const active=current.current;if(!active||active.saving)return false;clock.current!.stop();set({...active,saving:true,canceling:true});
    try{await invoke('cancel_case_edit',{sessionId:active.sessionId});authority.current.end();set(null);notice('已放弃修改，正式案例未变');try{await onSaved(active.document.id)}catch{notice('已放弃修改，列表刷新失败，请重新打开主库')}return true}
    catch(error){set({...active,saving:false,canceling:false});notice(String(error));return false}
  };
  const save=async()=>{
    const active=current.current;if(!active||active.pending||active.saving)return false;
    if(!active.document.title.trim()){notice('请填写案例标题后再保存');return false}
    if(!active.document.prompt.trim()){notice('请填写完整提示词后再保存');return false}
    clock.current!.stop();set({...active,saving:true});
    if(!await stash()){const latest=current.current;if(latest)set({...latest,saving:false});return false;}
    let saved:{id:string};
    try{saved=await invoke<{id:string}>('commit_case',{sessionId:active.sessionId,document:{...active.document,tags:parseTags(active.tagsText)}})}
    catch(error){if(current.current?.sessionId===active.sessionId)set({...current.current,saving:false});const message=String(error);notice(message.includes('revision_conflict')?'正式案例版本已变化，不能覆盖；当前草稿已保留，请继续编辑或明确放弃。':message==='case_title_required'?'请填写案例标题后再保存':message==='case_prompt_required'?'请填写完整提示词后再保存':message);return false}
    authority.current.end();set(null);notice('案例已保存');
    try{await onSaved(saved.id)}catch(error){notice(`案例已保存，但列表刷新失败，请重新打开主库：${String(error)}`)}return true;
  };
  const normalizeSource=async()=>{const active=current.current;if(!active||active.saving)return;const value=active.document.sourceUrl;
    try{const sourceUrl=await invoke<string>('normalize_source_url',{value});const latest=current.current;if(latest?.sessionId===active.sessionId&&latest.document.sourceUrl===value&&!latest.saving)mutate({...latest,document:{...latest.document,sourceUrl}})}catch(error){if(current.current?.sessionId===active.sessionId)notice(String(error))}
  };
  const request=(action:()=>void,stay?:()=>void)=>{if(resolvingRef.current)return;if(opening.current){notice('编辑器正在打开，请稍后重试');stay?.();return}const active=current.current;if(!active){action();return}if(active.saving){notice('正在处理当前编辑，请稍后重试');stay?.();return}if(active.dirty||active.pending){continuation.current={action,stay};setConfirmation(true)}else{void discard().then(ok=>{if(ok)action();else stay?.()})}};
  const resolve=async(choice:'save'|'discard'|'continue')=>{const next=continuation.current;if(!next||resolvingRef.current||current.current?.saving)return;if(choice==='continue'){continuation.current=null;setConfirmation(false);next.stay?.();return}if(choice==='save'&&current.current?.pending){notice('请等待图片导入完成');return}
    resolvingRef.current=true;setResolving(true);try{const ok=choice==='save'?await save():await discard();if(ok){continuation.current=null;setConfirmation(false);next.action()}}finally{resolvingRef.current=false;setResolving(false)}};
  const cancel=()=>request(()=>{});
  const recover=async(draftId:string,asNew=false)=>{if(busy())return false;opening.current=true;try{install(await invoke<EditReceipt>('recover_case_draft',{draftId,asNew}),undefined,true);notice('已恢复草稿，请确认后正式保存');return true}catch(error){notice(String(error));return false}finally{opening.current=false}};
  const handlers=useRef({importImages,cancel});handlers.current={importImages,cancel};
  useEffect(()=>{
    if(!isTauri())return;let disposed=false;let stop:(()=>void)|undefined;
    void getCurrentWebview().onDragDropEvent(event=>{
      if(event.payload.type!=='drop'||!current.current)return;
      const position=event.payload.position.toLogical(devicePixelRatio);const target=document.elementFromPoint(position.x,position.y)?.closest<HTMLElement>('[data-edit-slot]');
      void handlers.current.importImages(target?.dataset.editSlot??'append',event.payload.paths);
    }).then(unlisten=>{if(disposed)unlisten();else stop=unlisten}).catch(error=>notice(String(error)));
    const paste=(event:ClipboardEvent)=>{
      if(!current.current||!event.clipboardData)return;
      if(!hasClipboardImage([...event.clipboardData.types],[...event.clipboardData.items]))return;
      event.preventDefault();void handlers.current.importImages(undefined,undefined,true);
    };
    document.addEventListener('paste',paste,true);
    return()=>{disposed=true;stop?.();document.removeEventListener('paste',paste,true);authority.current.end();clock.current?.stop()};
  },[]);
  return {state,begin,change,select,remove,reorder,importImages,cancel,save,normalizeSource,busy,request,confirmation,resolving,resolve,recover,
    setTagsText:(tagsText:string)=>{const active=current.current;if(active&&!active.saving)mutate({...active,tagsText})}};
}
