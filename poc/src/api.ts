import {useEffect,useMemo,useState} from 'react';
import {invoke as nativeInvoke,isTauri} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';

export interface LibraryContext {generation:number;revision:number;maintenance:boolean;restoreWarning?:string}
type Invoke=<T=unknown>(command:string,args?:Record<string,unknown>)=>Promise<T>;
let context:LibraryContext|undefined;
let setup:Promise<void>|undefined;
let reading:Promise<LibraryContext>|undefined;
const changed=new Set<(context:LibraryContext)=>void>();

function accept(next:LibraryContext){
  if(context&&next.generation<context.generation)return;
  const different=context?.generation!==next.generation;
  const noticeChanged=context?.restoreWarning!==next.restoreWarning;
  context=next;
  if(different||noticeChanged)changed.forEach(callback=>callback(next));
}
async function connect(){
  if(!setup)setup=(async()=>{
    await listen<LibraryContext>('library-reinitialized',event=>accept(event.payload));
    window.addEventListener('focus',()=>{void refreshLibraryContext().catch(()=>{})});
  })().catch(error=>{setup=undefined;throw error});
  await setup;
}
export async function refreshLibraryContext():Promise<LibraryContext>{
  await connect();
  if(!reading)reading=nativeInvoke<LibraryContext>('get_library_context').then(next=>{accept(next);return context!}).finally(()=>{reading=undefined});
  return reading;
}
export async function initializeLibrary(){
  if(!isTauri())throw new Error('请通过桌面程序打开资料库');
  await connect();
  return context??refreshLibraryContext();
}
export function subscribeLibraryGeneration(callback:(context:LibraryContext)=>void){changed.add(callback);return()=>{changed.delete(callback)}}
export function isCurrentLibraryGeneration(generation:number){return generation===context?.generation}
export function useRestoreWarning(){
  const [warning,setWarning]=useState(context?.restoreWarning);
  useEffect(()=>{const stop=subscribeLibraryGeneration(next=>setWarning(next.restoreWarning));setWarning(context?.restoreWarning);return stop},[]);
  return warning;
}

/** Bind once per mounted component: a queued continuation must keep its original library. */
function libraryClient():Invoke{
  let generation=context?.generation;
  return async<T,>(command:string,args?:Record<string,unknown>):Promise<T>=>{
    const current=await initializeLibrary();
    generation??=current.generation;
    if(generation!==current.generation)throw new Error('stale_library_generation');
    try{
      const value=await nativeInvoke<T>(command,{...args,generation});
      if(command==='backup_restore'){
        accept(value as LibraryContext);
        return value;
      }
      if(generation!==context?.generation)throw new Error('stale_library_generation');
      return value;
    }catch(error){
      if(String(error).includes('stale_library_generation'))await refreshLibraryContext().catch(()=>{});
      throw error;
    }
  };
}
export function useLibraryInvoke():Invoke{return useMemo(libraryClient,[])}
