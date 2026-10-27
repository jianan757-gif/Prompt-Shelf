import {useEffect,useRef,useState} from 'react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {getCurrentWebview} from '@tauri-apps/api/webview';
import {attachImages,hasClipboardImage,ImportAuthority,moveReference,parseTags,removeImage} from './editor';
import type {CaseDocument,EditReceipt,ImportedImage,Slot} from './editor';

export interface EditorState extends EditReceipt {tagsText:string;selected:Slot;pending:boolean;saving:boolean;canceling?:boolean;images:ImportedImage[]}
export function useCaseEditor(notice:(text:string)=>void,onSaved:(id:string)=>Promise<void>){
  const [state,setState]=useState<EditorState|null>(null);const current=useRef<EditorState|null>(null);const opening=useRef(false);const authority=useRef(new ImportAuthority());
  const set=(next:EditorState|null)=>{current.current=next;setState(next)};
  const change=(document:CaseDocument)=>{const active=current.current;if(active&&!active.saving)set({...active,document})};
  const busy=()=>current.current!==null||opening.current;
  const begin=async(id:string|null,project='')=>{
    if(busy()){notice('请先保存或取消当前案例');return}opening.current=true;
    try{const receipt=await invoke<EditReceipt>('begin_case_edit',{caseId:id});const document={...receipt.document,project:id?receipt.document.project:project};authority.current.begin(receipt.sessionId);set({...receipt,document,tagsText:document.tags.join('，'),selected:'append',pending:false,saving:false,images:[]});}
    catch(error){notice(String(error))}finally{opening.current=false}
  };
  const select=(slot:Slot)=>{const active=current.current;if(active&&!active.saving)set({...active,selected:slot})};
  const remove=(slot:Slot)=>{const active=current.current;if(!active||active.saving)return;authority.current.invalidate(slot);set({...active,document:removeImage(active.document,slot),selected:active.selected===slot?'append':active.selected})};
  const reorder=(from:string,to:string)=>{const active=current.current;if(!active||active.saving)return;const document=moveReference(active.document,from,to);set({...active,document,selected:'append'});if(document!==active.document)notice('已调整参考图顺序，请核对提示词中的图号（未保存）')};
  const importImages=async(slot?:Slot,paths?:string[],clipboard=false)=>{
    const active=current.current;if(!active)return;if(active.pending||active.saving){notice('请等待当前图片操作完成');return}
    const target=slot??active.selected;if(target!=='append'&&paths&&paths.length>1){notice('替换图片时请选择一个文件');return}
    const ticket=authority.current.ticket(target);set({...active,pending:true});
    try{
      const images=await invoke<ImportedImage[]>('import_case_images',{sessionId:active.sessionId,paths:paths??null,clipboard,multiple:target==='append'});
      const latest=current.current;if(!latest||!authority.current.accepts(ticket))return;
      const document=attachImages(latest.document,target,images);
      set({...latest,document,images:[...latest.images,...images],pending:false,selected:images.length?'append':latest.selected});
      if(images.length)notice(document===latest.document?'目标图片已不存在，本次导入未附加':'图片已加入当前编辑，尚未正式保存');
    }catch(error){if(current.current?.sessionId===active.sessionId)notice(String(error))}
    finally{const latest=current.current;if(latest?.sessionId===active.sessionId)set({...latest,pending:false})}
  };
  const cancel=async()=>{
    const active=current.current;if(!active||active.saving)return;authority.current.end();set({...active,saving:true,canceling:true});
    try{await invoke('cancel_case_edit',{sessionId:active.sessionId});set(null);notice('已取消修改，正式案例未变')}
    catch(error){if(String(error).includes('edit_session_expired'))set(null);else{set({...active,saving:false});authority.current.begin(active.sessionId);notice(String(error))}}
  };
  const save=async()=>{
    const active=current.current;if(!active||active.pending||active.saving)return;
    if(!active.document.title.trim()){notice('请填写案例标题后再保存');return}
    if(!active.document.prompt.trim()){notice('请填写完整提示词后再保存');return}
    set({...active,saving:true});
    let saved:{id:string};
    try{saved=await invoke<{id:string}>('commit_case',{sessionId:active.sessionId,document:{...active.document,tags:parseTags(active.tagsText)}})}
    catch(error){if(current.current?.sessionId===active.sessionId)set({...current.current,saving:false});const message=String(error);notice(message==='case_title_required'?'请填写案例标题后再保存':message==='case_prompt_required'?'请填写完整提示词后再保存':message);return}
    authority.current.end();set(null);notice('案例已保存');
    try{await onSaved(saved.id)}catch(error){notice(`案例已保存，但列表刷新失败，请重新打开主库：${String(error)}`)}
  };
  const normalizeSource=async()=>{const active=current.current;if(!active||active.saving)return;const value=active.document.sourceUrl;
    try{const sourceUrl=await invoke<string>('normalize_source_url',{value});const latest=current.current;if(latest?.sessionId===active.sessionId&&latest.document.sourceUrl===value)set({...latest,document:{...latest.document,sourceUrl}})}catch(error){if(current.current?.sessionId===active.sessionId)notice(String(error))}
  };
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
    return()=>{disposed=true;stop?.();document.removeEventListener('paste',paste,true);authority.current.end()};
  },[]);
  return {state,begin,change,select,remove,reorder,importImages,cancel,save,normalizeSource,busy,
    setTagsText:(tagsText:string)=>{const active=current.current;if(active&&!active.saving)set({...active,tagsText})}};
}
