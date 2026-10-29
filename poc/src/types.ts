export type Id = string;
export interface Case {id:Id;title:string;prompt:string;project:string;projectId?:Id|null;tags:string[];tagIds?:Id[];favorite:boolean;used:number;result?:Id;refs:Id[];sourceUrl?:string;createdAt?:number|null;updatedAt?:number|null;lastUsedAt?:number|null;editRevision?:number}
export interface TrashCase extends Case {deletedAt:number;deletedProjectName:string|null;sharedImageCount:number;unfinishedDraftId:string|null}
export interface Asset {id:Id;original:string;thumbnail:string;width:number;height:number}
export interface CatalogItem {id:Id;name:string;createdAt:number;isQuick:boolean;quickOrder:number;sortOrder?:number}
export interface UiPreferences {libraryLayout:'regular'|'masonry';librarySort:'recent'|'name'|'new';catalogColumns:'single'|'double';tagSort:'time'|'name';projectSort:'time'|'name';railWidth:number;secondaryWidth:number;expandedCatalog:'tags'|'projects'|null;quickIntervalMs:number}
export interface Library {cases:Case[];assets:Asset[];projects?:CatalogItem[];tags?:CatalogItem[];trashCount?:number;uiPreferences?:UiPreferences;dictionary?:string[][]}
export type Tab='all'|'recent'|'fav';
export interface Progress {requestId:number;caseId:Id;phase:string;done:number;total:number;currentAsset?:string|null;timestampMs:number;error?:string|null}
