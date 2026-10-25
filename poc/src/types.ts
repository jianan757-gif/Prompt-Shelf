export type Id = string;
export interface Case {id:Id;title:string;prompt:string;project:string;tags:string[];favorite:boolean;used:number;result?:Id;refs:Id[]}
export interface Asset {id:Id;original:string;thumbnail:string;width:number;height:number}
export interface Library {cases:Case[];assets:Asset[]}
export type Tab='all'|'recent'|'fav';
export interface Progress {requestId:number;caseId:Id;phase:string;done:number;total:number;currentAsset?:string|null;timestampMs:number;error?:string|null}
