import test from 'node:test';
import assert from 'node:assert/strict';
import {panelIntent,progressRouting,rememberProgress,wakeView} from './session.ts';
import type {Progress} from './types.ts';
import type {SessionView,SavedView} from './session.ts';
import type {Tab} from './types.ts';

test('same-collection wake preserves query, filters, selection and scroll',()=>{
  const current:SessionView={tab:'all',query:'玻璃 精修',tag:'产品',project:'B5',selected:8,scrollTop:930};
  assert.equal(wakeView(current,'all',new Map()),current);
});
test('hotkey collections restore their own position without clearing search or filters',()=>{
  const saved=new Map<Tab,SavedView>();
  const original:SessionView={tab:'all',query:'保构图',tag:'商业CG',project:'项目1',selected:7,scrollTop:850};
  const favorite=wakeView(original,'fav',saved);
  assert.equal(favorite.query,original.query);assert.equal(favorite.tag,original.tag);assert.equal(favorite.project,original.project);
  assert.equal(favorite.selected,0);assert.equal(favorite.scrollTop,0);
  const restored=wakeView({...favorite,selected:2,scrollTop:180},'all',saved);
  assert.deepEqual(restored,original);
  assert.deepEqual(saved.get('fav'),{selected:2,scrollTop:180});
});
test('active Escape cancels and retains panel; active close always hides',()=>{
  assert.equal(panelIntent('escape',true),'cancel');assert.equal(panelIntent('escape',false),'hide');
  assert.equal(panelIntent('close',true),'hide');assert.equal(panelIntent('close',false),'hide');
});
test('early failure waits for request receipt while stale terminal cannot unlock a new call',()=>{
  const event={requestId:3,caseId:'c',phase:'failed',done:0,total:1,timestampMs:1};
  assert.equal(progressRouting(null,true,event),'defer');
  assert.equal(progressRouting(3,true,event),'apply');
  assert.equal(progressRouting(4,true,event),'ignore');
  assert.equal(progressRouting(null,false,event),'ignore');
});
test('late nonterminal stages cannot replace the active cancel id or reopen a finished request',()=>{
  let expected:number|null=4;let locked=true;
  for(const phase of ['preparing','restoringFocus','waiting','failed','completed']){
    const old={requestId:3,caseId:'old',phase,done:0,total:1,timestampMs:1};
    const route=progressRouting(expected,locked,old);
    assert.equal(route,'ignore');
    if(route==='apply'){expected=old.requestId;locked=phase!=='completed'&&phase!=='failed'}
  }
  assert.equal(expected,4);assert.equal(locked,true);
  const cancelPayload={requestId:expected};assert.equal(cancelPayload.requestId,4);
  const fresh={requestId:4,caseId:'new',phase:'preparing',done:0,total:1,timestampMs:2};
  assert.equal(progressRouting(null,true,fresh),'defer');
  assert.equal(progressRouting(4,true,fresh),'apply');
  assert.equal(progressRouting(null,false,fresh),'ignore');
});
test('pending events are isolated by id and a terminal cannot be overwritten by an earlier stage',()=>{
  const pending=new Map<number,Progress>();
  const failure={requestId:4,caseId:'new',phase:'failed',done:0,total:1,timestampMs:2};
  rememberProgress(pending,failure);
  rememberProgress(pending,{...failure,phase:'preparing',timestampMs:1});
  rememberProgress(pending,{...failure,requestId:3,phase:'waiting'});
  assert.equal(pending.get(4),failure);assert.equal(pending.get(3)?.phase,'waiting');
});
