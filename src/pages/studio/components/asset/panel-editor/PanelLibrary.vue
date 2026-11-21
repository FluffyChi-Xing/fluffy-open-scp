<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { availableComponents } from './model';
const props=defineProps<{leaves:string[];items:string[];power:boolean;selected?:string}>();
const emit=defineEmits<{add:[id:string];select:[id:string];remove:[id:string];move:[id:string,index:number]}>();const {t}=useI18n();
const groups=computed(()=>['common',...props.leaves].map(group=>({group,items:availableComponents(props.leaves).filter(c=>c.group===group)})));
function drag(e:DragEvent,id:string){e.dataTransfer?.setData('application/openscp-panel-component',id);if(e.dataTransfer)e.dataTransfer.effectAllowed='copyMove';}
</script>
<template>
  <aside class="panel-library">
    <header><strong>{{t('panelEditor.library')}}</strong><p>{{t('panelEditor.libraryHint')}}</p></header>
    <details v-for="group in groups" :key="group.group" open>
      <summary>{{t(`panelEditor.groups.${group.group}`)}}</summary>
      <div class="cards">
        <button v-for="item in group.items" :key="item.id" type="button" draggable="true" @dragstart="drag($event,item.id)" @click="items.includes(item.id)?emit('select',item.id):emit('add',item.id)">
          <span aria-hidden="true">⠿</span><span>{{t(`panelEditor.components.${item.id}`)}}</span><small v-if="items.includes(item.id)">{{t('panelEditor.added')}}</small>
        </button>
        <button v-if="group.group==='common'" type="button" draggable="true" @dragstart="drag($event,'power')" @click="power?emit('select','power'):emit('add','power')"><span aria-hidden="true">⠿</span>{{t('panelEditor.power')}}<small v-if="power">{{t('panelEditor.added')}}</small></button>
        <p v-if="!group.items.length" class="empty">{{t('panelEditor.noComponents')}}</p>
      </div>
    </details>
    <section v-if="selected" class="selection"><strong>{{t('panelEditor.selection')}}</strong><p>{{t(selected==='power'?'panelEditor.power':`panelEditor.components.${selected}`)}}</p><div><template v-if="selected!=='power'"><button type="button" :disabled="items.indexOf(selected)<=0" @click="emit('move',selected,items.indexOf(selected)-1)">{{t('panelEditor.up')}}</button><button type="button" :disabled="items.indexOf(selected)>=items.length-1" @click="emit('move',selected,items.indexOf(selected)+2)">{{t('panelEditor.down')}}</button></template><button type="button" @click="emit('remove',selected)">{{t('panelEditor.remove')}}</button></div></section>
  </aside>
</template>
<style scoped>
.panel-library{width:280px;flex-shrink:0;overflow:auto;background:var(--surface-1,#152237);border-left:1px solid var(--border,#2b3c54)}header{padding:16px}p{font-size:12px;line-height:1.6;color:var(--text-muted,#a1b2c8)}details{border-top:1px solid var(--border,#2b3c54)}summary{padding:12px 16px;cursor:pointer;font-weight:600}.cards{display:grid;gap:7px;padding:0 12px 14px}.cards button{display:flex;gap:8px;align-items:center;min-height:38px;padding:8px;border:1px solid var(--border,#2b3c54);border-radius:5px;background:var(--surface-2,#1b2a40);text-align:left;color:inherit;cursor:grab;font:inherit;font-size:12px}.cards small{margin-left:auto;font-size:10px;color:var(--text-muted,#95bddf)}.cards button:hover,.cards button:focus-visible{outline:1px solid var(--primary,#5daaff)}.empty{margin:0}@media(max-width:1000px){.panel-library{width:230px}}
</style>
<style scoped>.selection{position:sticky;bottom:0;background:var(--surface-1,#152237);padding:14px;border-top:1px solid var(--border,#2b3c54)}.selection>div{display:flex;gap:5px;flex-wrap:wrap}.selection button{padding:5px 8px;border:1px solid var(--border,#2b3c54);border-radius:4px;background:var(--surface-2,#1b2b43);color:inherit;font:inherit;font-size:12px;cursor:pointer}.selection button:disabled{opacity:.45;cursor:default}</style>
