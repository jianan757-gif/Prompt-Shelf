// Pure search baseline only: excludes WebView/React rendering, IPC and image loading.
import fs from 'node:fs';
import {performance} from 'node:perf_hooks';
import {filterCases} from '../poc/src/search.ts';
const fixture=JSON.parse(fs.readFileSync(new URL('../fixtures/100x500/manifest.json',import.meta.url),'utf8'));
const queries=['','001','产品','透明 玻璃','摄影','无匹配内容'];
const records=fixture.cases.map(c=>({...c,favorite:c.favorite??c.fav,tagIds:[],projectId:null}));
for(let i=0;i<20;i++)for(const q of queries)filterCases(records,q,'all');
const samples=[];
for(let i=0;i<100;i++)for(const q of queries){const start=performance.now();filterCases(records,q,'all');samples.push(performance.now()-start)}
samples.sort((a,b)=>a-b);
console.log(JSON.stringify({cases:records.length,assets:fixture.assets.length,samples:samples.length,p50Ms:samples[Math.ceil(samples.length*.5)-1],p95Ms:samples[Math.ceil(samples.length*.95)-1],maxMs:samples.at(-1),method:'Node production filterCases, synthetic fixture, warmed, 100 rounds x 6 queries; pure computation only',uiLatency:'NOT_MEASURED',threshold:'NOT_APPROVED'},null,2));
