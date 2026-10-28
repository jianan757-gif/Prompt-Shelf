/** A trailing debounce plus maximum continuous-input delay; injectable clock for deterministic tests. */
export class DraftClock {
  private quiet:ReturnType<typeof setTimeout>|undefined;private maximum:ReturnType<typeof setTimeout>|undefined;
  private write:()=>void;private timer:typeof setTimeout;private clear:typeof clearTimeout;
  constructor(write:()=>void,timer:typeof setTimeout=(handler,delay,...args)=>globalThis.setTimeout(handler,delay,...args),clear:typeof clearTimeout=handle=>globalThis.clearTimeout(handle)){this.write=write;this.timer=timer;this.clear=clear}
  schedule(){if(this.quiet)this.clear(this.quiet);this.quiet=this.timer(()=>this.flush(),600);if(!this.maximum)this.maximum=this.timer(()=>this.flush(),2000)}
  flush(){this.stop();this.write()}
  stop(){if(this.quiet)this.clear(this.quiet);if(this.maximum)this.clear(this.maximum);this.quiet=undefined;this.maximum=undefined}
}
export const contentKey=(document:unknown,tagsText:string)=>JSON.stringify([document,tagsText]);
