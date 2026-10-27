import type {Asset} from '../types.ts';
export interface ImageReference {id:string;assetId:string;originalName:string}
export interface CaseDocument {id:string;baseRevision:number;title:string;prompt:string;project:string;tags:string[];sourceUrl:string;result:ImageReference|null;references:ImageReference[]}
export interface EditReceipt {sessionId:string;document:CaseDocument;draftId?:string;saveSeq?:number}
export interface DraftSummary {id:string;caseId:string|null;title:string;prompt:string;referenceCount:number;resultCount:number;updatedAt:number;targetStatus:'new'|'live'|'missing'|'deleted';conflict:boolean}
export interface ImportedImage extends Asset {originalName:string}
export type Slot='append'|'result'|string;
export interface ImportTicket {sessionId:string;slot:Slot;version:number}
/** In-memory only. A slot version invalidates a removed/replaced target without using array indexes. */
export class ImportAuthority {
  private sessionId:string|null=null;private versions=new Map<Slot,number>();
  begin(sessionId:string){this.sessionId=sessionId;this.versions.clear()}
  end(){this.sessionId=null;this.versions.clear()}
  invalidate(slot:Slot){this.versions.set(slot,(this.versions.get(slot)??0)+1)}
  ticket(slot:Slot):ImportTicket {this.invalidate(slot);return {sessionId:this.sessionId??'',slot,version:this.versions.get(slot)!}}
  accepts(ticket:ImportTicket){return this.sessionId===ticket.sessionId&&this.versions.get(ticket.slot)===ticket.version}
}
export const parseTags=(text:string)=>text.split(/[,，]/u).map(name=>name.trim()).filter(Boolean);
export function attachImages(document:CaseDocument,slot:Slot,images:ImportedImage[],newId=()=>crypto.randomUUID()):CaseDocument{
  const reference=(image:ImportedImage):ImageReference=>({id:newId(),assetId:image.id,originalName:image.originalName});
  if(!images.length)return document;
  if(slot==='append')return {...document,references:[...document.references,...images.map(reference)]};
  if(images.length!==1)return document;
  if(slot==='result'){const replacement=reference(images[0]);return {...document,result:{...replacement,id:document.result?.id??replacement.id}}}
  if(!document.references.some(item=>item.id===slot))return document;
  return {...document,references:document.references.map(item=>item.id===slot?{...item,assetId:images[0].id,originalName:images[0].originalName}:item)};
}
export function removeImage(document:CaseDocument,slot:Slot):CaseDocument{return slot==='result'?{...document,result:null}:{...document,references:document.references.filter(item=>item.id!==slot)}}
export function moveReference(document:CaseDocument,from:string,to:string):CaseDocument{
  const a=document.references.findIndex(item=>item.id===from),b=document.references.findIndex(item=>item.id===to);if(a<0||b<0||a===b)return document;
  const references=[...document.references];const [item]=references.splice(a,1);references.splice(b,0,item);return {...document,references};
}
export function hasClipboardImage(types:readonly string[],items:readonly {type:string;kind:string}[]):boolean{
  return types.includes('Files')||items.some(item=>item.kind==='file'&&item.type.startsWith('image/'));
}
