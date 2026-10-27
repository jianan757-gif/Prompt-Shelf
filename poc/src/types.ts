export type Id = string;
export interface Case {id:Id;title:string;prompt:string;project:string;tags:string[];favorite:boolean;used:number;result?:Id;refs:Id[];sourceUrl?:string;createdAt?:number|null;updatedAt?:number|null;lastUsedAt?:number|null;editRevision?:number}
export interface Asset {id:Id;original:string;thumbnail:string;width:number;height:number}
export interface CatalogItem {id:Id;name:string;createdAt:number;isQuick:boolean;quickOrder:number}
export interface Library {cases:Case[];assets:Asset[];projects?:CatalogItem[];tags?:CatalogItem[];trashCount?:number}
export type Tab='all'|'recent'|'fav';
export interface Progress {requestId:number;caseId:Id;phase:string;done:number;total:number;currentAsset?:string|null;timestampMs:number;error?:string|null}
