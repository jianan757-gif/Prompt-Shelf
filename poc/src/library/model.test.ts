import test from 'node:test';
import assert from 'node:assert/strict';
import {catalogs,columnCount,libraryCases,safeSource} from './model.ts';
import type {Case} from '../types.ts';
const record=(id:string,createdAt:number,lastUsedAt:number|null):Case=>({id,title:'案例',prompt:'完整提示词',project:'项目',tags:['产品'],favorite:false,used:lastUsedAt?1:0,refs:[],createdAt,lastUsedAt});
test('library dates sort by stored timestamps rather than UUID or call count',()=>{
  const data=[record('zz',100,300),record('aa',200,500),{...record('bb',300,400),used:99}];
  assert.deepEqual(libraryCases(data,'','all','','','new').map(r=>r.id),['bb','aa','zz']);
  assert.deepEqual(libraryCases(data,'','all','','','recent').map(r=>r.id),['aa','bb','zz']);
  assert.deepEqual(data.map(r=>r.id),['zz','aa','bb']);
});
test('browse filtering is read only and unmarked catalog tags never become quick tags',()=>{
  const data=[record('one',100,null),{...record('two',200,300),favorite:true}];
  assert.deepEqual(libraryCases(data,'完整','fav','产品','项目','recent').map(r=>r.id),['two']);
  assert.equal(catalogs({cases:data,assets:[]},'tags')[0].isQuick,false);
  assert.equal(data[0].favorite,false);assert.equal(data[1].used,1);
});
test('explicit name and creation sorts take priority over search relevance',()=>{
  const older={...record('old',100,100),title:'B product',prompt:'product'};
  const newer={...record('new',200,200),title:'A case',prompt:'product'};
  assert.deepEqual(libraryCases([older,newer],'product','all','','','name').map(r=>r.id),['new','old']);
  assert.deepEqual(libraryCases([older,newer],'product','all','','','new').map(r=>r.id),['new','old']);
  assert.deepEqual(libraryCases([older,newer],'product','all','','','recent').map(r=>r.id),['old','new']);
});
test('column thresholds and source URL policy follow the frozen view contract',()=>{
  assert.deepEqual([844,845,1389,1390].map(columnCount),[1,2,2,3]);
  assert.equal(safeSource('javascript:alert(1)'),null);assert.equal(safeSource('file:///a'),null);
  assert.equal(safeSource('https://example.com/source'),'https://example.com/source');
});
