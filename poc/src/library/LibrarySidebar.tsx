import {Icon} from './Icons';
import {catalogCount} from './model';
import type {Case,CatalogItem,Tab} from '../types';

export type CatalogKind='projects'|'tags';
export function LibrarySidebar({records,view,setView,expanded,toggle,quickTags,activeTag,setTag,notice}:{records:Case[];view:Tab;setView:(view:Tab)=>void;expanded:CatalogKind|null;toggle:(kind:CatalogKind)=>void;quickTags:CatalogItem[];activeTag:string;setTag:(name:string)=>void;notice:(message:string)=>void}){
  return <aside className="rail" aria-label="资料库导航"><div className="rail-inner">
    <section className="rail-group"><div className="section-overline">常用集合</div>
      {([['all','全部'],['fav','收藏'],['recent','最近']] as const).map(([id,label])=><button type="button" className={`navitem ${view===id?'active':''}`} data-view={id} key={id} onClick={()=>setView(id)}><span className="navicon"><Icon name={`nav-${id}`} size={20}/></span><span>{label}</span><span className="count">{records.filter(record=>id==='all'||(id==='fav'?record.favorite:record.used>0)).length}</span></button>)}
    </section>
    <div className="navdivider"/>
    <section className="rail-group"><div className="section-overline">分类管理</div>
      {([['projects','项目归档'],['tags','标签']] as const).map(([kind,label])=><button type="button" key={kind} className={`navitem ${kind==='projects'?'project-navigation':'tag-navigation'} ${expanded===kind?'active':''}`} title={`展开或收起${label}`} aria-expanded={expanded===kind} onClick={()=>toggle(kind)}><span className="navicon"><Icon name={`nav-${kind}`} size={20}/></span><span>{label}</span></button>)}
    </section>
    <div className="navdivider"/>
    <section className="rail-group"><div className="section-overline">快捷标签</div><div id="quickTagList">
      {quickTags.map(item=><div className="tag-row" key={item.id}><button type="button" className="tag-grip" aria-label={`调整 ${item.name} 的顺序`} title="调整快捷标签顺序" onClick={()=>notice('快捷标签排序暂未开放')}/><button type="button" className={`taglink ${activeTag===item.name?'active':''}`} onClick={()=>setTag(item.name)}>{item.name}</button><span className="countplain">{catalogCount(records,'tags',item.name)}</span></div>)}
      {!quickTags.length&&<p className="smallnote">暂无快捷标签</p>}
    </div></section>
    <div className="navdivider"/><p className="smallnote">项目归档为低频入口；常用标签可以直接在左侧筛选。</p>
  </div></aside>;
}
