import {test} from 'node:test';
import assert from 'node:assert/strict';
import {score,filterCases,highlightParts} from './search.ts';
import type {Case} from './types.ts';
const sample:Case={id:'1',title:'透明玻璃精修',prompt:'保持标签与结构，商业摄影。',project:'精华液',tags:['产品'],favorite:true,used:1,refs:[]};
test('AND spans fields and rejects absent terms',()=>{assert.ok(score(sample,'玻璃 产品')>=0);assert.equal(score(sample,'玻璃 猫'),-1)});
test('title ranks above body and project',()=>{assert.ok(score(sample,'玻璃')>score({...sample,title:'其他',prompt:'玻璃'},'玻璃'));assert.ok(score({...sample,title:'其他',prompt:'玻璃'},'玻璃')>score({...sample,title:'其他',prompt:'其他',project:'玻璃'},'玻璃'))});
test('dictionary is bidirectional and tabs filter',()=>{assert.ok(score({...sample,title:'商业精修'},'精修')>=0);assert.ok(score({...sample,title:'背景移除'},'去背')>=0);assert.ok(score({...sample,title:'去背'},'背景移除')>=0);assert.equal(filterCases([sample,{...sample,id:'2',favorite:false,used:0}],'','fav').length,1);assert.equal(filterCases([{...sample,used:0}],'','recent').length,0)});
test('highlight preserves hostile HTML and regex query literally',()=>{const source='<img onerror="alert(1)"> & [a]';const parts=highlightParts(source,'[a]');assert.equal(parts.map(x=>x.text).join(''),source);assert.equal(parts.filter(x=>x.hit)[0].text,'[a]');assert.equal(highlightParts('图像扩展','扩图').some(x=>x.hit),true)});
