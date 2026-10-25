export type RailDirection='prev'|'next';
const clamp=(value:number,max:number)=>Math.max(0,Math.min(Math.max(0,max),value));
export function railEdges(current:number,max:number) {
  const left=clamp(current,max);return {prev:left>1.5,next:Math.max(0,max)-left>1.5};
}
export function nearestReference(offsets:readonly number[],current:number):number {
  let nearest=0,best=Infinity;
  offsets.forEach((left,index)=>{const distance=Math.abs(left-current);if(distance<best){nearest=index;best=distance}});
  return nearest;
}
export function restoreReference(offsets:readonly number[],index:number,max:number):number {
  return clamp(offsets[Math.max(0,Math.min(offsets.length-1,index))]??0,max);
}
export function stepReference(offsets:readonly number[],current:number,max:number,direction:RailDirection):number {
  if(!offsets.length)return 0;
  const left=clamp(current,max);
  if(direction==='next')return clamp(offsets.find(value=>value>left+2)??max,max);
  let previous=0;for(const value of offsets){if(value<left-2)previous=value;else break}
  return clamp(previous,max);
}
