import React,{useEffect,useRef,useState} from 'react';
import {createRoot} from 'react-dom/client';
import {CaseEditor} from '../../src/library/CaseEditor';
import {useCaseEditor} from '../../src/library/useCaseEditor';
import {DraftClock} from '../../src/library/draftSafety';
import '../../src/library.css';
const runtime=window as any;
runtime.isTauri=true;const callbacks=new Map<number,Function>();const listeners=new Map<number,{event:string;handler:number}>();let id=0;
runtime.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'library_manager'},currentWebview:{label:'library_manager'}},transformCallback:(fn:Function)=>{callbacks.set(++id,fn);return id},unregisterCallback:(id:number)=>callbacks.delete(id),convertFileSrc:(path:string)=>'/__test_asset?path='+encodeURIComponent(path),invoke:async(command:string,args:any)=>{if(command==='plugin:event|listen'){const eventId=++id;listeners.set(eventId,args);return eventId}if(command==='plugin:event|unlisten'){listeners.delete(args.eventId);return}return runtime.backend(command,args)}};
runtime.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
runtime.emitTestDrop=(paths:string[],x:number,y:number)=>{for(const [id,event] of listeners)if(event.event==='tauri://drag-drop')callbacks.get(event.handler)?.({id,event:event.event,payload:{paths,position:{x:x*devicePixelRatio,y:y*devicePixelRatio}}})};
runtime.DraftClock=DraftClock;
function Harness(){
 const [library,setLibrary]=useState<any>({cases:[],assets:[]});const [notices,setNotices]=useState<string[]>([]);const positions=useRef(new Map<string,number>());
 const refresh=async()=>setLibrary(await runtime.backend('get_library',{}));
 const editor=useCaseEditor(text=>setNotices(previous=>[...previous,text]),refresh);
 const live=useRef(editor);live.current=editor;
 useEffect(()=>{void refresh();const key=(event:KeyboardEvent)=>{if(event.key==='Escape'){runtime.escapeBubbled++;live.current.cancel()}};document.addEventListener('keydown',key);return()=>document.removeEventListener('keydown',key)},[]);
 runtime.test={editor,state:editor.state,notices,library,refresh};runtime.escapeBubbled??=0;
 const assets=new Map([...library.assets,...(editor.state?.images??[])].map((asset:any)=>[asset.id,asset]));
 return <div className="library-root" style={{padding:16,display:'block',overflow:'auto'}}><div id="card-host" style={{width:720,maxWidth:'100%'}}>{editor.state?<CaseEditor editor={editor} assets={assets as any} positions={positions.current} projects={library.projects??[]}/>:<div id="browser-ready">编辑器未打开</div>}</div><output id="test-state" hidden>{JSON.stringify({state:editor.state,confirmation:editor.confirmation})}</output></div>;
}
document.documentElement.dataset.window='library';createRoot(document.getElementById('root')!).render(<Harness/>);
