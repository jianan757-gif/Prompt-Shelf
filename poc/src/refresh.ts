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
