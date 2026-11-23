<script setup lang="ts">
import { computed,shallowRef,toRef,watch } from 'vue';
import {useI18n} from 'vue-i18n';
import PanelCanvas from './PanelCanvas.vue';
import PanelLibrary from './PanelLibrary.vue';
import {usePanelProject} from './usePanelProject';
import {availableComponents,insertComponent,type PanelMode} from './model';
import {usesBehaviorContainer,type LogicGraph,type BehaviorLeafType} from '../logicGraph';
const props=defineProps<{project:string;logic:LogicGraph;title:string;active:boolean}>();
const emit=defineEmits<{dirty:[value:boolean]}>();
const {t,te}=useI18n(),state=usePanelProject(toRef(props,'project'));
const {schema,library,busy,error,dirty,undo,redo,saved}=state;
const mode=shallowRef<PanelMode>('click'),closed=shallowRef(false),previewError=shallowRef('');
const selected=shallowRef<string>();
const leaves=computed(()=>[...new Set(props.logic.nodes.filter(n=>n.kind==='behavior-leaf'&&props.logic.edges.some(e=>e.source===n.id&&props.logic.nodes.some(core=>core.id===e.target&&core.kind==='behavior-core'))).map(n=>(props.logic.params[n.id] as {type?:BehaviorLeafType})?.type).filter((type):type is BehaviorLeafType=>!!type))]);
const managed=computed(()=>usesBehaviorContainer(props.logic));
const available=computed(()=>availableComponents(leaves.value,managed.value));
const invalid=computed(()=>{
  const allowed=new Set(available.value.map(c=>c.id));
  return (['hover','click'] as const).flatMap(view=>(schema.value?.[view]??[]).filter(id=>!allowed.has(id)).map(id=>({view,id})));
});
const invalidLabels=computed(()=>invalid.value.map(({view,id})=>{
  const key=`panelEditor.components.${id}`;
  return t('panelEditor.controlLocation',{view:t(`panelEditor.${view}`),name:te(key)?t(key):id});
}).join(t('panelEditor.controlSeparator')));
function insert(id:string,index=schema.value?.[mode.value].length??0){if(!schema.value)return;selected.value=id;if(id==='power'){state.update({...schema.value,power:true});return;}if(!available.value.some(c=>c.id===id))return;state.update(insertComponent(schema.value,mode.value,id,index));}
function remove(id:string){if(!schema.value)return;state.update(id==='power'?{...schema.value,power:false}:{...schema.value,[mode.value]:schema.value[mode.value].filter(v=>v!==id)});}
watch(dirty,value=>emit('dirty',value),{immediate:true});
watch(mode,()=>{selected.value=undefined;});
watch(schema,value=>{if(!value||selected.value==='power'&&!value.power||selected.value&&selected.value!=='power'&&!value[mode.value].includes(selected.value))selected.value=undefined;});
defineExpose({save:state.save});
</script>
<template>
  <section class="panel-project">
    <div class="panel-toolbar">
      <div class="segmented"><button type="button" :class="{active:!schema}" :disabled="busy" @click="state.update(null)">{{t('panelEditor.inherit')}}</button><button type="button" :class="{active:!!schema}" :disabled="busy" @click="state.enable">{{t('panelEditor.custom')}}</button></div>
      <template v-if="schema"><div class="segmented"><button v-for="view in (['click','hover'] as const)" :key="view" type="button" :class="{active:mode===view}" @click="mode=view">{{t(`panelEditor.${view}`)}}</button></div><button type="button" @click="closed=!closed">{{t(closed?'panelEditor.closed':'panelEditor.normal')}}</button><button type="button" :disabled="!undo.length||busy" @click="state.restore('undo')">{{t('panelEditor.undo')}}</button><button type="button" :disabled="!redo.length||busy" @click="state.restore('redo')">{{t('panelEditor.redo')}}</button></template>
    </div>
    <p v-if="busy" role="status">{{t('panelEditor.loading')}}</p>
    <p v-if="error||previewError" class="error" role="alert">{{t('panelEditor.failed',{message:error||previewError})}}</p>
    <p v-if="!schema&&!busy">{{t('panelEditor.inheritHint')}}</p>
    <p v-if="schema" class="file-hint">{{t('panelEditor.fileHint',{path:saved?.schemaPath??'schemas/panel/'})}}</p>
    <p v-if="invalid.length" class="error" role="alert">{{t('panelEditor.unsupportedControls',{items:invalidLabels})}}</p>
    <div v-if="schema&&library" class="panel-workspace" :inert="busy">
      <PanelCanvas v-if="active" :schema="schema" :library="library" :mode="mode" :closed="closed" :title="title" :selected="selected" @insert="insert" @remove="remove" @error="previewError=$event" @select="selected=$event" />
      <PanelLibrary :leaves="leaves" :items="schema[mode]" :power="schema.power" :managed="managed" :selected="selected" @add="insert" @select="selected=$event" @remove="remove" @move="insert" />
    </div>
    <p v-if="schema" class="preview-hint">{{t('panelEditor.previewHint')}}</p>
    <p v-if="schema&&managed" class="preview-hint">{{t('panelEditor.singleUnitHint')}}</p>
    <p v-if="schema" class="preview-hint">{{t('panelEditor.runtimeHint')}}</p>
  </section>
</template>
<style scoped>
.panel-project{display:flex;flex-direction:column;min-height:650px;height:100%;color:inherit}.panel-toolbar{display:flex;gap:10px;align-items:center;flex-wrap:wrap;margin-bottom:10px}.panel-toolbar button{border:1px solid var(--border,#2b3c54);border-radius:4px;background:var(--surface-2,#1b2b43);color:inherit;padding:6px 10px;font:inherit;font-size:12px;cursor:pointer}.panel-toolbar button:disabled{opacity:.45;cursor:default}.segmented{display:flex}.segmented button{border-radius:0}.segmented button.active{background:#17375b;color:#83beff}.panel-workspace{display:flex;flex:1;min-height:520px;border:1px solid var(--border,#2b3c54);border-radius:6px;overflow:hidden}p{font-size:12px;line-height:1.6}.error{color:var(--danger,#ff8b8b)}.file-hint,.preview-hint{color:var(--text-muted,#a1b2c8);overflow-wrap:anywhere}
</style>
