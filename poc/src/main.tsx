import React from 'react';
import {createRoot} from 'react-dom/client';
import {App} from './App';
const library=new URLSearchParams(location.search).get('window')==='library';
await (library?import('./library.css'):import('./quick.css'));
await import('./poc.css');
createRoot(document.getElementById('root')!).render(<App library={library}/>);
