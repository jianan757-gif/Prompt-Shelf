import {useEffect,useLayoutEffect,useRef} from 'react';
import {EditorImages} from './EditorImages';
import {Icon} from './Icons';
import type {Asset,CatalogItem} from '../types';
import type {useCaseEditor} from './useCaseEditor';

type Controller=ReturnType<typeof useCaseEditor>;
export function CaseEditor({editor,assets,positions,projects}:{editor:Controller;assets:Map<string,Asset>;positions:Map<string,number>;projects:CatalogItem[]}){
  const active=editor.state!;const record=active.document;const title=useRef<HTMLInputElement>(null);const prompt=useRef<HTMLTextAreaElement>(null);
  useEffect(()=>{if(document.hasFocus())title.current?.focus({preventScroll:true})},[active.sessionId]);
  useLayoutEffect(()=>{const input=prompt.current;if(input){input.style.height='0px';input.style.height=`${Math.max(95,input.scrollHeight+2)}px`}},[record.prompt]);
  const change=(field:'title'|'project'|'prompt'|'sourceUrl',value:string)=>editor.change({...record,[field]:value});
  return <article className="case editing" data-case-id={record.id} data-editor-session={active.sessionId} data-case-state={active.dirty?'unsaved':'unchanged'}>
    <div className="case-edit-head"><input ref={title} className="edit-title" aria-label="案例标题" placeholder="案例标题" value={record.title} disabled={active.saving} onChange={event=>change('title',event.target.value)}/><input className="edit-project" aria-label="所属项目" placeholder="所属项目（可空）" list="editor-projects" value={record.project} disabled={active.saving} onChange={event=>change('project',event.target.value)}/><datalist id="editor-projects">{projects.map(project=><option key={project.id} value={project.name}/>)}</datalist>
      <div className="card-actions"><button type="button" className="iconbutton editon" title="保存" aria-label="保存案例" disabled={active.pending||active.saving} onClick={()=>void editor.save()}><Icon name="check"/></button><button type="button" className="iconbutton" title="取消" aria-label="取消编辑" disabled={active.saving} onClick={()=>void editor.cancel()}><Icon name="close"/></button></div>
    </div>
    <div className="tag-editor"><label htmlFor="editor-tags">标签</label><input id="editor-tags" value={active.tagsText} disabled={active.saving} onChange={event=>editor.setTagsText(event.target.value)} placeholder="使用逗号分隔多个标签"/></div>
    <EditorImages caseId={record.id} result={record.result} references={record.references} assets={assets} positions={positions} selected={active.selected} disabled={active.saving} pending={active.pending} select={editor.select} replace={slot=>void editor.importImages(slot)} remove={editor.remove} reorder={editor.reorder}/>
    <textarea ref={prompt} className="edit-prompt" aria-label="完整提示词" value={record.prompt} disabled={active.saving} onChange={event=>change('prompt',event.target.value)} placeholder="完整提示词……"/>
    <div className="source-editor"><label htmlFor="editor-source">来源链接</label><input id="editor-source" type="text" spellCheck={false} value={record.sourceUrl} disabled={active.saving} onChange={event=>change('sourceUrl',event.target.value)} onBlur={()=>void editor.normalizeSource()} placeholder="https://…（可不填）"/><span className="source-hint">点击卡片顶部链接图标溯源</span></div>
    <div className="edit-bottom"><span className="fieldnote" role="status">{active.canceling?'正在取消…':active.saving?'正在保存…':active.pending?'正在导入图片…':<span className="v50-draft-status"><i/>{active.draftError?'未正式保存 · 草稿暂存失败':!active.dirty?'尚未修改':active.protectedSeq>=active.seq?'未正式保存 · 自动保护草稿':'未正式保存 · 草稿暂存中…'}</span>}</span><div className="edit-actions"><button type="button" className="cancel" disabled={active.saving} onClick={()=>void editor.cancel()}>取消</button><button type="button" className="save" disabled={active.pending||active.saving} onClick={()=>void editor.save()}>保存案例</button></div></div>
  </article>;
}
