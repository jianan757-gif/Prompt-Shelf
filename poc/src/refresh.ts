/** A later read supersedes an older response, including the first boot snapshot. */
export class LatestRead {
  private sequence=0;
  begin(){return ++this.sequence}
  accepts(token:number){return token===this.sequence}
  invalidate(){this.sequence++}
}
export function selectionAfterRefresh(ids:string[],selectedId:string|null,previousIndex:number):number{
  const index=selectedId===null?-1:ids.indexOf(selectedId);
  return index>=0?index:Math.max(0,Math.min(previousIndex,ids.length-1));
}
/** Automatic snapshots retain the current session's order; a view change explicitly starts a new order. */
export function retainSessionOrder<T extends {id:string}>(records:T[],previousIds:string[]):T[]{
  const current=new Map(records.map(record=>[record.id,record]));const output:T[]=[];
  for(const id of previousIds){const record=current.get(id);if(record){output.push(record);current.delete(id)}}
  return [...output,...records.filter(record=>current.has(record.id))];
}
