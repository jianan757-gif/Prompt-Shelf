import test from 'node:test';
import assert from 'node:assert/strict';
import {attachImages,hasClipboardImage,ImportAuthority,moveReference,parseTags,removeImage} from './editor.ts';
import type {CaseDocument,ImportedImage} from './editor.ts';
import {LatestRead,selectionAfterRefresh} from '../refresh.ts';
const image=(id:string):ImportedImage=>({id,original:'private-original',thumbnail:'cache',width:100,height:100,originalName:`${id}.png`});
const document=():CaseDocument=>({id:'case',baseRevision:1,title:'正式标题',prompt:'正式正文',project:'',tags:[],sourceUrl:'',result:null,references:[{id:'row-a',assetId:'shared',originalName:'a.png'},{id:'row-b',assetId:'other',originalName:'b.png'}]});
test('late replacement follows stable reference id after a reorder, not the old index',()=>{
  const authority=new ImportAuthority();authority.begin('session');const ticket=authority.ticket('row-a');
  const original=document(),reordered=moveReference(original,'row-a','row-b');
  assert.ok(authority.accepts(ticket));const result=attachImages(reordered,ticket.slot,[image('replacement')],()=> 'new-row');
  assert.equal(result.references[0].id,'row-b');assert.equal(result.references[0].assetId,'other');
  assert.equal(result.references[1].id,'row-a');assert.equal(result.references[1].assetId,'replacement');
  assert.equal(original.references[0].assetId,'shared');
});
test('removed slots, canceled editors and later sessions reject old image receipts',()=>{
  const authority=new ImportAuthority();authority.begin('first');const ticket=authority.ticket('result');
  authority.invalidate('result');assert.equal(authority.accepts(ticket),false);
  const fresh=authority.ticket('row-a');authority.end();assert.equal(authority.accepts(fresh),false);
  authority.begin('second');assert.equal(authority.accepts(fresh),false);
  const second=authority.ticket('row-a');assert.ok(authority.accepts(second));
  const removed=removeImage(document(),'row-a');assert.equal(attachImages(removed,'row-a',[image('x')]),removed);
  assert.deepEqual(removed.references.map(reference=>reference.id),['row-b']);
});
test('shared assets keep distinct occurrence ids and replacement does not collapse references',()=>{
  let sequence=0;const original=document();const result=attachImages(original,'append',[image('shared'),image('shared')],()=>`new-${++sequence}`);
  assert.equal(result.references.length,4);assert.notEqual(result.references[2].id,result.references[3].id);
  assert.equal(result.references[2].assetId,result.references[3].assetId);
  assert.equal(original.references.length,2);
  assert.equal(attachImages(original,'row-a',[image('one'),image('two')]),original);
});
test('image paste detection leaves plain text paste and free comma tag input intact',()=>{
  assert.equal(hasClipboardImage(['text/plain'],[{kind:'string',type:'text/plain'}]),false);
  assert.equal(hasClipboardImage(['Files'],[]),true);
  assert.equal(hasClipboardImage([], [{kind:'file',type:'image/png'}]),true);
  assert.deepEqual(parseTags(' 首个，第二个, , third，'),['首个','第二个','third']);
});
test('first-read and focus refresh responses cannot overwrite a newer library change',()=>{
  const reads=new LatestRead();const initial=reads.begin(),changed=reads.begin();let displayed='old';
  if(reads.accepts(changed))displayed='saved';if(reads.accepts(initial))displayed='stale-initial';assert.equal(displayed,'saved');
  const pending=reads.begin();reads.invalidate();assert.equal(reads.accepts(pending),false);
});
test('quick refresh keeps the selected case id across insertions and safely clamps a removed selection',()=>{
  assert.equal(selectionAfterRefresh(['new','selected','last'],'selected',0),1);
  assert.equal(selectionAfterRefresh(['only'],'removed',8),0);
  assert.equal(selectionAfterRefresh([],'removed',8),0);
});
