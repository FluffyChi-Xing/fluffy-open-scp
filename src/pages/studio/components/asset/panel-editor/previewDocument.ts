import { convertFileSrc } from '@tauri-apps/api/core';
import { extractBindings } from './extractBindings';
import type { PanelLibrary } from './model';
import runtime from './native/runtime.js?raw';
import fixture from './native/fixture.js?raw';
import preview from './native/preview.js?raw';
const script=(text:string)=>`<script>${text.replace(/<\/script/gi,'<\\/script')}</script>`;

export function previewDocument(library:PanelLibrary):string {
  const rows=library.resources.map(r=>({...r,url:convertFileSrc(r.path)}));
  const transport=`
  const panelResources=${JSON.stringify(rows).replaceAll('<','\\u003c')};
  function panelAsset(path){
    if(!path||/^(data:|blob:|asset:|https?:)/.test(path))return path;
    path=decodeURIComponent(path).replace(/^\\//,'');
    let full=/raw\\/([A-F0-9]{8})_([A-F0-9]{8})_([A-F0-9]{8})/i.exec(path),base=path.split('/').pop(),ext=base.split('.').pop().toLowerCase(),stem=base.slice(0,-ext.length-1),h=2166136261;
    for(const c of stem.toLowerCase())h=(Math.imul(h,16777619)^c.charCodeAt(0))>>>0;
    let id=/^0x[0-9a-f]+$/i.test(stem)?parseInt(stem,16):h;
    let type=path.includes('Locale/')?0x0A98EAF0:ext==='js'?0x67771F5C:ext==='css'?0x2C978DB6:ext==='gif'?0x2F7D0007:0x2F7D0004;
    let candidates=panelResources.filter(r=>full?r.type===parseInt(full[1],16)&&r.group===parseInt(full[2],16)&&r.instance===parseInt(full[3],16):r.type===type&&r.instance===id);
    let lang=path.includes('Locale/')?path.split('/')[1]:'en-us';
    let selected=path.includes('Locale/')?candidates.find(r=>r.locale===lang)||candidates.at(-1):candidates.at(-1);
    return selected?selected.url:path;
  }
  const open=XMLHttpRequest.prototype.open;XMLHttpRequest.prototype.open=function(method,url,...args){return open.call(this,method,panelAsset(String(url)),...args);};
  for(const [type,prop]of [[HTMLImageElement,'src'],[HTMLLinkElement,'href']]){const d=Object.getOwnPropertyDescriptor(type.prototype,prop);Object.defineProperty(type.prototype,prop,{...d,set(value){if(type===HTMLImageElement)this.setAttribute('data-panel-original-source',value);d.set.call(this,panelAsset(value));}});}
  `;
  const css=library.css.replace(/url\((['"]?)([^)'"\s]+)\1\)/g,(_,quote,url)=>{
    const filename=String(url).split('/').at(-1)!,stem=filename.replace(/\.[^.]*$/,'');let h=2166136261;for(const c of stem.toLowerCase())h=(Math.imul(h,16777619)^c.charCodeAt(0))>>>0;
    const match=[...rows].reverse().find(r=>r.instance===h);return match?`url("${match.url}")`:`url(${quote}${url}${quote})`;
  });
  const receive=`
  window.addEventListener('message',async e=>{
   if(e.source!==parent||e.data?.type!=='panel-render')return;
   const d=e.data, r=await run('park',d.mode,d.options);
   const c=window.panel?.FindControlByID(181820548);if(c?.mIsVisible){const a=c.mRootElement.getBoundingClientRect(),b=panel.mRootElement.getBoundingClientRect();r.powerRect={x:a.left-b.left,y:a.top-b.top,width:a.width,height:a.height};}
   parent.postMessage({type:'panel-report',revision:d.revision,report:r},'*');
  });
  parent.postMessage({type:'panel-ready'},'*');`;
  return `<!doctype html><html><head><meta charset="utf-8"><style>${css.replace(/<\/style/gi,'<\\/style')}</style><style>:root{color-scheme:dark}html,body{margin:0;background:transparent;overflow:hidden}#host{position:absolute;left:24px;top:24px;width:600px;height:1800px}#caption,#notes{display:none}#host img[data-panel-original-source$="/rlvr-Zot_Header.png"]{display:none}</style></head><body><div id="caption"></div><div id="host"></div><pre id="notes"></pre>${script(transport)}${script(library.scruiScript)}${script(extractBindings(library.gameScript))}${script(`window.OpenSCPPanelComponents=${JSON.stringify(library.components)};`)}${script(runtime)}${script(fixture)}${script(preview)}${script('scrui.PostGameCommand=function(){};')}${script(receive)}</body></html>`;
}
