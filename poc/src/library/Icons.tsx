// Static SVG paths from frozen V4.11 and approved image controls; never supplied by library content.
const paths={
  "unlink": '<path d="m18.84 12.25 1.72-1.71a6 6 0 0 0-8.49-8.49l-1.71 1.71M5.16 11.75l-1.72 1.71a6 6 0 0 0 8.49 8.49l1.71-1.71M8 2v3M2 8h3M16 19v3M19 16h3"/>',
  "trash-2": '<path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M10 11v6M14 11v6"/>',
  "book": "<path d=\"M12 6.4c-2.4-1.9-5.4-2.1-8.5-1.5v12.4c3.3-.6 6.2-.3 8.5 1.7m0-12.6c2.4-1.9 5.4-2.1 8.5-1.5v12.4c-3.3-.6-6.2-.3-8.5 1.7m0-12.6V19\"/>",
  "archive": "<rect x=\"3.5\" y=\"4.5\" width=\"17\" height=\"5\" rx=\"1.6\"/><path d=\"M5.5 9.5v9.2c0 .9.7 1.6 1.6 1.6h9.8c.9 0 1.6-.7 1.6-1.6V9.5M9.5 13.5h5\"/>",
  "plus": "<path d=\"M12 5v14M5 12h14\"/>",
  "pencil": "<path d=\"m4.6 19.5 4.4-1 10.2-10.2a2 2 0 0 0 0-2.9l-.6-.6a2 2 0 0 0-2.9 0L5.5 15zM13.8 6.6l3.6 3.6\"/>",
  "close": "<path d=\"M6 6l12 12M18 6 6 18\"/>",
  "check": "<path d=\"m4.5 12.5 5 4.8 10-10.2\"/>",
  "trash": "<path d=\"M4.5 7h15M9.5 7V4.8h5V7M7 7.5l.8 11.3c.1.9.8 1.6 1.7 1.6h5c.9 0 1.6-.7 1.7-1.6L17 7.5M10 10.5v6M14 10.5v6\"/>",
  "pin": "<g class=\"ps-pin-glyph\" transform=\"rotate(40 12 12)\"><path class=\"ps-pin-body\" d=\"M9.3 5.3h5.4c.75 0 1.3.55 1.3 1.3v1.15c0 .55-.3.93-.76 1.12v4.05l1.72 2.32c.44.62.03 1.44-.82 1.44H7.86c-.85 0-1.26-.82-.82-1.44l1.72-2.32V8.87C8.3 8.68 8 8.3 8 7.75V6.6c0-.75.55-1.3 1.3-1.3Z\"/><path class=\"ps-pin-needle\" d=\"M12 16.6v3.1\"/></g>",
  "star": "<path d=\"m12 3.3 2.8 5.8 6.4.9-4.6 4.6 1.1 6.4L12 18l-5.7 3 1.1-6.4L2.8 10l6.4-.9z\"/>",
  "link": "<path d=\"M10 13.5a5 5 0 0 0 7.4.6l3-3a5 5 0 0 0-7.1-7.1l-1.6 1.6M14 10.5a5 5 0 0 0-7.4-.6l-3 3a5 5 0 1 0 7.1 7.1l1.6-1.6\"/>",
  "search": "<circle cx=\"10.6\" cy=\"10.6\" r=\"6.2\"/><path d=\"m15.2 15.2 5 5\"/>",
  "restore": "<path d=\"M4 10a8 8 0 1 1 1.6 7.1M4 5v5.7h5.7\"/>",
  "more": "<circle cx=\"5\" cy=\"12\" r=\"1.2\" fill=\"currentColor\" stroke=\"none\"/><circle cx=\"12\" cy=\"12\" r=\"1.2\" fill=\"currentColor\" stroke=\"none\"/><circle cx=\"19\" cy=\"12\" r=\"1.2\" fill=\"currentColor\" stroke=\"none\"/>",
  "image": "<rect x=\"3.3\" y=\"4.1\" width=\"17.4\" height=\"15.7\" rx=\"2.2\"/><circle cx=\"8.6\" cy=\"9.3\" r=\"1.5\"/><path d=\"m5.5 17 4.1-4 2.7 2.5 2.3-2.1 3.9 3.6\"/>",
  "nav-all": "<rect x=\"3.3\" y=\"3.5\" width=\"17.4\" height=\"17\" rx=\"4\"/><path d=\"M3.4 12.4h4.2l1.5 2.2h5.8l1.5-2.2h4.2\"/><path d=\"M8 7.6h8\" opacity=\".60\"/>",
  "nav-fav": "<path d=\"M12 20.4s-7.8-4.7-8.65-9.5C2.65 6.85 5.1 4.6 7.8 4.6c1.88 0 3.22.98 4.2 2.43.98-1.45 2.32-2.43 4.2-2.43 2.7 0 5.15 2.25 4.45 6.3C19.8 15.7 12 20.4 12 20.4z\"/>",
  "nav-recent": "<circle cx=\"12\" cy=\"12\" r=\"8.5\"/><path d=\"M12 7.25v5.2l3.6 2.1\"/>",
  "nav-projects": "<path d=\"M3.5 7.3V6a2.25 2.25 0 0 1 2.25-2.25h4.2l2.05 2.2h6.25A2.25 2.25 0 0 1 20.5 8.2v9.5a2.25 2.25 0 0 1-2.25 2.25H5.75A2.25 2.25 0 0 1 3.5 17.7V7.3z\"/><path d=\"M3.5 9.1h17\" opacity=\".68\"/>",
  "nav-tags": "<path d=\"M4 5.9A2 2 0 0 1 6 3.9h6.25c.6 0 1.18.24 1.6.67l6.2 6.2a2.25 2.25 0 0 1 0 3.18l-6.05 6.05a2.25 2.25 0 0 1-3.18 0l-6.15-6.2A2.25 2.25 0 0 1 4 12.2z\"/><circle cx=\"8.55\" cy=\"8.55\" r=\"1.25\" fill=\"currentColor\" stroke=\"none\"/>",
  "backup": "<path d=\"M6 3.5h9l4 4v13H6zM9 3.5v6h7v-5M9 20.5v-7h7v7\"/>",
  "minimize": "<path d=\"M5 17h14\"/>",
  "maximize": "<rect x=\"6\" y=\"6\" width=\"12\" height=\"12\" rx=\"1.2\"/>",
  "regular": "<rect x=\"2\" y=\"2.5\" width=\"7\" height=\"7\" rx=\"1\"/><rect x=\"11\" y=\"2.5\" width=\"7\" height=\"7\" rx=\"1\"/><rect x=\"2\" y=\"11\" width=\"7\" height=\"7\" rx=\"1\"/><rect x=\"11\" y=\"11\" width=\"7\" height=\"7\" rx=\"1\"/>",
  "masonry": "<rect x=\"2\" y=\"2.5\" width=\"7\" height=\"10\" rx=\"1\"/><rect x=\"11\" y=\"2.5\" width=\"7\" height=\"6\" rx=\"1\"/><rect x=\"11\" y=\"10.5\" width=\"7\" height=\"7\" rx=\"1\"/><rect x=\"2\" y=\"14.5\" width=\"7\" height=\"3\" rx=\"1\"/>"
} as const;
export type IconName=keyof typeof paths;
export function Icon({name,size=16}:{name:IconName;size?:number}){
 return <svg className="ui-icon" width={size} height={size} viewBox={name==='regular'||name==='masonry'?'0 0 20 20':'0 0 24 24'} fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" dangerouslySetInnerHTML={{__html:paths[name]}}/>;
}
