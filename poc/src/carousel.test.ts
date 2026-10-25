import test from 'node:test';
import assert from 'node:assert/strict';
import {nearestReference,railEdges,restoreReference,stepReference} from './carousel.ts';

test('mixed image widths advance to actual boundaries and clamp the final step',()=>{
  const offsets=[0,66,207,303];
  assert.equal(stepReference(offsets,0,260,'next'),66);
  assert.equal(stepReference(offsets,66,260,'next'),207);
  assert.equal(stepReference(offsets,207,260,'next'),260);
  assert.equal(stepReference(offsets,260,260,'next'),260);
  assert.equal(stepReference(offsets,260,260,'prev'),207);
  assert.equal(stepReference(offsets,207,260,'prev'),66);
  assert.equal(stepReference(offsets,66,260,'prev'),0);
});
test('empty and nonoverflowing rails have no movable direction',()=>{
  assert.deepEqual(railEdges(0,0),{prev:false,next:false});
  assert.equal(stepReference([],0,0,'next'),0);
  assert.equal(stepReference([0,100],0,0,'next'),0);
});
test('fractional boundaries avoid repeated tiny steps and expose only available edges',()=>{
  assert.equal(stepReference([0,66,207],65.8,260,'next'),207);
  assert.equal(stepReference([0,66,207],66.2,260,'prev'),0);
  assert.deepEqual(railEdges(0,260),{prev:false,next:true});
  assert.deepEqual(railEdges(259.5,260),{prev:true,next:false});
  assert.deepEqual(railEdges(66,260),{prev:true,next:true});
});
test('resize and filtered image counts restore a valid saved image boundary',()=>{
  assert.equal(restoreReference([0,66,207,303],3,260),260);
  assert.equal(restoreReference([0,66,207,303],3,400),303);
  assert.equal(restoreReference([0,66],3,40),40);
  assert.equal(restoreReference([],3,0),0);
});
test('nearest image and rapid requested steps preserve one-image navigation',()=>{
  assert.equal(nearestReference([0,66,207],80),1);
  const first=stepReference([0,66,207,303],0,260,'next');
  assert.equal(stepReference([0,66,207,303],first,260,'next'),207);
  assert.equal(nearestReference([],0),0);
});
