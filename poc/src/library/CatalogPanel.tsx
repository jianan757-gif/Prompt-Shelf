import {useEffect,useMemo,useState} from 'react';
import {createPortal} from 'react-dom';
import {Icon} from './Icons';
import {catalogCount} from './model';
import type {CatalogKind} from './LibrarySidebar';
import type {Case,CatalogItem} from '../types';

export function CatalogPanel({kind,entries,records,active,choose,notice,canHandleEscape}:{kind:CatalogKind;entries:CatalogItem[];records:Case[];active:string;choose:(value:string)=>void;notice:(message:string)=>void;canHandleEscape:()=>boolean}){
  const [query,setQuery]=useState('');const [sort,setSort]=useState<'time'|'name'>('time');const [columns,setColumns]=useState<'single'|'double'>('single');
  const [menu,setMenu]=useState<{id:string;left:number;top:number}|null>(null);
  useEffect(()=>{
    if(!menu)return;
    const outside=(event:globalThis.PointerEvent)=>{if(!(event.target as Element).closest('[data-catalog-menu],.catalog-menu'))setMenu(null)};
    const escape=(event:KeyboardEvent)=>{if(event.key==='Escape'&&canHandleEscape()){event.preventDefault();event.stopImmediatePropagation();setMenu(null)}};
    document.addEventListener('pointerdown',outside,true);document.addEventListener('keydown',escape,true);
    return()=>{document.removeEventListener('pointerdown',outside,true);document.removeEventListener('keydown',escape,true)};
  },[menu,canHandleEscape]);
  const items=useMemo(()=>entries.filter(item=>item.name.toLocaleLowerCase().includes(query.toLocaleLowerCase())).slice().sort((a,b)=>sort==='name'?a.name.localeCompare(b.name,'zh-CN'):b.createdAt-a.createdAt||a.name.localeCompare(b.name,'zh-CN')),[entries,query,sort]);
  const label=kind==='tags'?'标签':'项目';
  return <aside className="secondary" aria-label={kind==='tags'?'全部标签':'项目归档'}>
    <div className="secondary-head"><div className="secondary-title"><span>{kind==='tags'?'全部标签':'项目归档'}</span><div className="catalog-tools"><button type="button" className="catalog-btn primary" onClick={()=>notice(`${label}新增暂未开放`)}><Icon name="plus" size={14}/> 新增</button><button type="button" className="catalog-btn" onClick={()=>notice(`${label}编辑暂未开放`)}><Icon name="pencil" size={14}/> 编辑</button></div></div>
      <label className="catalog-row-search"><Icon name="search" size={14}/><input value={query} onChange={event=>setQuery(event.target.value)} placeholder={`搜索${label}…`} aria-label={`搜索${label}`}/></label>
      <div className="catalog-controls"><div className="catalog-sort" role="group" aria-label={`${label}排序`}>{(['time','name'] as const).map(value=><button type="button" key={value} className={sort===value?'active':''} aria-pressed={sort===value} onClick={()=>setSort(value)}>{value==='time'?'时间':'名称'}</button>)}</div><div className="catalog-column-toggle" role="group" aria-label="标签和项目列表列数">{(['single','double'] as const).map(value=><button type="button" key={value} className={columns===value?'active':''} aria-pressed={columns===value} onClick={()=>setColumns(value)}><Icon name={value==='single'?'archive':'image'} size={13}/>{value==='single'?'单列':'双列'}</button>)}</div></div>
    </div>
    <div className="secondary-list" onScroll={()=>setMenu(null)}><div className={`catalog-grid ${columns==='single'?'one-col':'two-cols'}`}>
      {items.map(item=><div className={`catalog-item ${active===item.name?'selected':''}`} title={item.name} key={item.id}>
        <button type="button" className="catalog-name" title={`筛选${label}：${item.name}`} onClick={()=>choose(item.name)}><span className="catalog-text">{item.name}</span><small>{catalogCount(records,kind,item.name)}</small></button>
        <div className="catalog-actions"><button type="button" className="catalog-icon-btn" title="更多操作" aria-label={`${item.name} 更多操作`} aria-haspopup="menu" aria-expanded={menu?.id===item.id} data-catalog-menu={item.id} onClick={event=>{const rect=event.currentTarget.getBoundingClientRect();const height=kind==='tags'?118:76;setMenu(menu?.id===item.id?null:{id:item.id,left:Math.max(8,Math.min(innerWidth-153,rect.right-145)),top:rect.bottom+height+6>innerHeight?Math.max(7,rect.top-height-5):rect.bottom+5})}}><Icon name="more"/></button></div>
        {menu?.id===item.id&&createPortal(<div className="catalog-menu" role="menu" style={{left:menu.left,top:menu.top}}>{kind==='tags'&&<><button role="menuitem" type="button" onClick={()=>{setMenu(null);notice('快捷标签设置暂未开放')}}><span className={`menu-pin ${item.isQuick?'quick-enabled':''}`}><Icon name="pin" size={15}/></span>{item.isQuick?'取消快捷标签':'设为快捷标签'}</button><hr/></>}<button role="menuitem" type="button" onClick={()=>{setMenu(null);notice(`${label}重命名暂未开放`)}}><Icon name="pencil" size={15}/>重命名</button><button role="menuitem" type="button" className="danger" onClick={()=>{setMenu(null);notice(`${label}删除暂未开放`)}}><Icon name="trash" size={15}/>删除</button></div>,document.body)}
      </div>)}
      {!items.length&&<p className="catalog-note">没有匹配内容</p>}
    </div></div>
  </aside>;
}
