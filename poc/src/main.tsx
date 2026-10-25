import React from 'react';
import {createRoot} from 'react-dom/client';
const library=new URLSearchParams(location.search).get('window')==='library';
document.documentElement.dataset.window=library?'library':'quick';
const root=createRoot(document.getElementById('root')!);
// Keep explicit branches: the production preloader must retain each window's CSS.
if(library){
  await import('./library.css');
  const {LibraryManager}=await import('./library/LibraryManager');
  root.render(<LibraryManager/>);
}else{
  await import('./quick.css');
  await import('./poc.css');
  const {App}=await import('./App');
  root.render(<App library={false}/>);
}
