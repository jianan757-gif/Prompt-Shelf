import type {Progress,Tab} from './types.ts';
export const terminalPhase=(phase:string)=>['done','complete','completed','cancelled','canceled','error','failed'].includes(phase.toLowerCase());
export function progressRouting(expected:number|null,locked:boolean,event:Progress):'apply'|'defer'|'ignore'{
  if(expected===null)return locked?'defer':'ignore';
  return event.requestId===expected?'apply':'ignore';
}
export function rememberProgress(cache:Map<number,Progress>,event:Progress):void{
  const previous=cache.get(event.requestId);
  if(previous&&terminalPhase(previous.phase)&&!terminalPhase(event.phase))return;
  cache.set(event.requestId,event);
}

export interface SessionView {tab:Tab;query:string;tag:string;project:string;selected:number;scrollTop:number}
export type SavedView=Pick<SessionView,'selected'|'scrollTop'>;
/** A wake changes the requested collection, never the active search/filter. */
export function wakeView(current:SessionView,tab:Tab,saved:Map<Tab,SavedView>):SessionView {
  if(current.tab===tab)return current;
  saved.set(current.tab,{selected:current.selected,scrollTop:current.scrollTop});
  const destination=saved.get(tab)??{selected:0,scrollTop:0};
  return {...current,tab,...destination};
}
export function panelIntent(key:'escape'|'close',busy:boolean):'cancel'|'hide' {
  return key==='escape'&&busy?'cancel':'hide';
}
