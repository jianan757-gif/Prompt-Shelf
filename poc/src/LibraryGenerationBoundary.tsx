import {Fragment,useEffect,useState} from 'react';
import type {ReactNode} from 'react';
import {isTauri} from '@tauri-apps/api/core';
import {initializeLibrary,subscribeLibraryGeneration} from './api';

export function LibraryGenerationBoundary({children}:{children:ReactNode}){
  const [generation,setGeneration]=useState<number|null>(isTauri()?null:0);
  const [error,setError]=useState('');
  const load=()=>{setError('');void initializeLibrary().then(value=>setGeneration(value.generation)).catch(value=>setError(String(value)))};
  useEffect(()=>{if(!isTauri())return;const stop=subscribeLibraryGeneration(value=>{setGeneration(value.generation);setError('')});load();return stop},[]);
  if(error)return <div className="empty-results" role="alert">资料库暂时无法读取<br/>{error}<br/><button type="button" className="btn" onClick={load}>重试</button></div>;
  if(generation===null)return <div className="empty-results" role="status">正在加载资料库…</div>;
  return <Fragment key={generation}>{children}</Fragment>;
}
