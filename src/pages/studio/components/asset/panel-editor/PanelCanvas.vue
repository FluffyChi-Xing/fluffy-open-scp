<script setup lang="ts">
import { computed,onBeforeUnmount,onMounted,shallowRef,watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { clipSlot,itemName,nameHash,type PanelSchema,type PanelLibrary as Library,type PanelMode,type PreviewReport,type Rect } from './model';
import { previewDocument } from './previewDocument';
const props=defineProps<{schema:PanelSchema;library:Library;mode:PanelMode;closed:boolean;title:string;selected?:string}>();
const emit=defineEmits<{insert:[id:string,index:number];remove:[id:string];error:[message:string];select:[id:string|undefined]}>();
const {t,locale}=useI18n();const viewport=shallowRef<HTMLDivElement>(),frame=shallowRef<HTMLIFrameElement>();
const document=shallowRef(''),report=shallowRef<PreviewReport>(),ready=shallowRef(false);
const zoom=shallowRef(1),pan=shallowRef({x:90,y:50});let revision=0;
let drag:{id?:string;x:number;y:number;pan:{x:number;y:number};moved:boolean}|undefined;
const names=computed(()=>Object.fromEntries(props.library.components.map(c=>[nameHash(itemName(c,props.schema.missingFeedback==='hide')),c.id])));
const overlays=computed(()=>{
  const r=report.value;if(!r)return [];
  const list=r.items.filter(i=>i.visible&&names.value[i.id]).map(i=>({id:names.value[i.id],rect:clipSlot(i.rect,r.contentBounds)})).filter(i=>i.rect.width>0&&i.rect.height>0);
  if(props.schema.power&&props.mode==='click'&&r.powerRect)list.push({id:'power',rect:r.powerRect});return list;
});
const worldStyle=computed(()=>({transform:`translate(${pan.value.x}px,${pan.value.y}px) scale(${zoom.value})`,'--panel-zoom':zoom.value}));
const frameStyle=computed(()=>({width:(report.value?.dimensions[0]??278)+48+'px',height:(report.value?.dimensions[1]??350)+48+'px'}));
function rectStyle(r:Rect){return{left:r.x+'px',top:r.y+'px',width:r.width+'px',height:r.height+'px'};}
function send(){
 if(!ready.value||!frame.value?.contentWindow)return;
 const schema=props.schema,all=props.library.components,hide=schema.missingFeedback==='hide';
 const ids=(list:string[])=>list.length?list.map(id=>{const c=all.find(c=>c.id===id);if(!c)throw Error('PANEL_COMPONENT_UNKNOWN:'+id);return nameHash(itemName(c,hide));}):[nameHash('OpenSCPPanel_empty')];
 const shell=schema.shell.split(':').map(v=>parseInt(v,16));
 const context=['residence','education','park','tourism'].find(k=>[...schema.hover,...schema.click].some(id=>all.find(c=>c.id===id)?.group===k))??'park';
 const names=Object.fromEntries([...all.map(c=>[nameHash(itemName(c,hide)),itemName(c,hide)]),[nameHash('OpenSCPPanel_empty'),'OpenSCPPanel_empty']]);
 frame.value.contentWindow.postMessage({type:'panel-render',revision:++revision,mode:props.mode==='hover'?'base':'extended',options:{__compose:true,__panel:{layout:{type_id:shell[0],group:shell[1],instance:shell[2]},icon:props.library.contexts.park?.[212579023],key:'panel-preview',names:[props.title],base:ids(schema.hover),extended:ids(schema.click),popup:schema.power?[nameHash('kOnOffToggle')]:[],flat:props.library.contexts[context]??{}},__names:names,__state:props.closed?'closed':'normal',__nativeLocale:props.library.nativeLocales?.[locale.value]??(locale.value==='en-US'?'en-us':'zh-cn')}},'*');
}
function fit(){if(!viewport.value||!report.value)return;const [w,h]=report.value.dimensions;zoom.value=Math.max(.25,Math.min(1.25,(viewport.value.clientWidth-80)/w,(viewport.value.clientHeight-90)/h));pan.value={x:(viewport.value.clientWidth-w*zoom.value)/2,y:Math.max(36,(viewport.value.clientHeight-h*zoom.value)/2)};}
function zoomAt(next:number,x=(viewport.value?.clientWidth??0)/2,y=(viewport.value?.clientHeight??0)/2){const z=Math.max(.25,Math.min(3,next));pan.value={x:x-(x-pan.value.x)*z/zoom.value,y:y-(y-pan.value.y)*z/zoom.value};zoom.value=z;}
function wheel(e:WheelEvent){const b=viewport.value!.getBoundingClientRect();zoomAt(zoom.value*Math.exp(-e.deltaY*.0015),e.clientX-b.left,e.clientY-b.top);}
function indexAt(clientY:number){const b=viewport.value!.getBoundingClientRect(),y=(clientY-b.top-pan.value.y)/zoom.value;const item=overlays.value.find(i=>i.id!=='power'&&y<i.rect.y+i.rect.height/2);return item?props.schema[props.mode].indexOf(item.id):props.schema[props.mode].length;}
function start(e:PointerEvent,id?:string){if(e.button!==0)return;e.preventDefault();emit('select',id);drag={id,x:e.clientX,y:e.clientY,pan:{...pan.value},moved:false};}
function pointerMove(e:PointerEvent){if(!drag)return;if(Math.hypot(e.clientX-drag.x,e.clientY-drag.y)>5)drag.moved=true;if(!drag.id)pan.value={x:drag.pan.x+e.clientX-drag.x,y:drag.pan.y+e.clientY-drag.y};}
function pointerUp(e:PointerEvent){if(!drag)return;const d=drag;drag=undefined;if(d.id&&d.id!=='power'&&d.moved)emit('insert',d.id,indexAt(e.clientY));}
function drop(e:DragEvent){const id=e.dataTransfer?.getData('application/openscp-panel-component');if(id)emit('insert',id,indexAt(e.clientY));}
function key(e:KeyboardEvent,id:string){if(e.target!==e.currentTarget)return;if(e.key==='Delete'||e.key==='Backspace'){e.preventDefault();emit('remove',id);}else if(e.key==='Enter'||e.key===' '){e.preventDefault();emit('select',id);}else if(['ArrowUp','ArrowDown'].includes(e.key)&&id!=='power'){e.preventDefault();const at=props.schema[props.mode].indexOf(id);emit('insert',id,e.key==='ArrowUp'?Math.max(0,at-1):Math.min(props.schema[props.mode].length,at+2));}}
function message(e:MessageEvent){if(e.source!==frame.value?.contentWindow)return;if(e.data?.type==='panel-ready'){ready.value=true;send();}if(e.data?.type==='panel-report'&&e.data.revision===revision){const r=e.data.report as PreviewReport;const failure=r.error||r.items?.find(i=>i.error)?.error;if(failure){emit('error',failure);return;}const first=!report.value;report.value=r;if(first)fit();}}
watch(()=>props.library,value=>{ready.value=false;report.value=undefined;document.value=previewDocument(value);},{immediate:true});
watch(()=>[props.schema,props.mode,props.closed,props.title,locale.value],send);
onMounted(()=>{window.addEventListener('message',message);window.addEventListener('pointermove',pointerMove);window.addEventListener('pointerup',pointerUp);});
onBeforeUnmount(()=>{window.removeEventListener('message',message);window.removeEventListener('pointermove',pointerMove);window.removeEventListener('pointerup',pointerUp);});
</script>
<template>
  <section class="panel-canvas">
    <div class="canvas-tools"><span>{{t('panelEditor.canvas')}}</span><div><button type="button" :aria-label="t('panelEditor.zoomOut')" @click="zoomAt(zoom/1.2)">−</button><output>{{Math.round(zoom*100)}}%</output><button type="button" :aria-label="t('panelEditor.zoomIn')" @click="zoomAt(zoom*1.2)">＋</button><button type="button" @click="fit">{{t('panelEditor.fit')}}</button></div></div>
    <div ref="viewport" class="viewport" :style="{backgroundSize:`${20*zoom}px ${20*zoom}px`,backgroundPosition:`${pan.x}px ${pan.y}px`}" tabindex="0" :aria-label="t('panelEditor.canvas')" @wheel.prevent="wheel" @pointerdown.self="start($event)" @dragover.prevent @drop.prevent="drop">
      <div class="world" :style="worldStyle">
        <iframe ref="frame" :srcdoc="document" :title="t('panelEditor.preview')" sandbox="allow-scripts allow-same-origin" :style="frameStyle" />
        <div v-for="item in overlays" :key="item.id" class="module" :class="{selected:selected===item.id}" :style="rectStyle(item.rect)" tabindex="0" role="group" :aria-label="t(item.id==='power'?'panelEditor.power':`panelEditor.components.${item.id}`)" @pointerdown.stop="start($event,item.id)" @keydown="key($event,item.id)" @click="emit('select',item.id)">
          <button v-if="selected===item.id" class="remove" type="button" :aria-label="t('panelEditor.remove')" @pointerdown.stop @click.stop="emit('remove',item.id)">×</button>
        </div>
      </div>
      <span class="canvas-hint">{{t('panelEditor.canvasHint')}}</span>
    </div>
  </section>
</template>
<style scoped>
.panel-canvas{display:flex;flex:1;min-width:0;flex-direction:column}.canvas-tools{display:flex;align-items:center;justify-content:space-between;padding:8px 12px;border-bottom:1px solid var(--border,#2b3c54);font-size:12px}.canvas-tools>div{display:flex;gap:5px;align-items:center}.canvas-tools button{padding:5px 9px;min-height:30px;border:1px solid var(--border,#2b3c54);border-radius:4px;background:var(--surface-2,#1b2b43);color:inherit;font:inherit;cursor:pointer}output{min-width:48px;text-align:center}.viewport{position:relative;flex:1;min-height:440px;overflow:hidden;background-color:#101722;background-image:radial-gradient(#354052 1px,transparent 1px);background-size:20px 20px;touch-action:none;cursor:grab}.world{position:absolute;transform-origin:0 0}.world iframe{position:absolute;left:-24px;top:-24px;border:0;pointer-events:none;filter:drop-shadow(0 10px 18px #0006)}.module{position:absolute;box-sizing:border-box;border:1px dashed transparent;cursor:grab;touch-action:none}.module:hover,.module.selected{border:2px dashed var(--primary,#5daaff);background:#4397f517}.module:focus-visible{outline:2px solid var(--primary,#5daaff)}.remove{position:absolute;right:-8px;top:-8px;width:16px;height:16px;min-height:16px;padding:0;border:1px solid #ff8484;border-radius:50%;background:#c52f3e;color:white;font-size:13px;line-height:14px;cursor:pointer;transform:scale(calc(1 / var(--panel-zoom,1)));transform-origin:center;z-index:3}.remove::after{content:'';position:absolute;inset:-4px}.canvas-hint{position:absolute;bottom:15px;left:15px;color:#aebed0;background:#101a2bd9;padding:6px 9px;border-radius:4px;font-size:11px;pointer-events:none}
</style>
