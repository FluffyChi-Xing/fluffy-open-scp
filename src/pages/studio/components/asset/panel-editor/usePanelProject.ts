import { computed, shallowRef, watch, type Ref } from 'vue';
import { readPanelProject,loadPanelLibrary,writePanelProject } from './api';
import type { PanelLibrary,PanelProject,PanelSchema } from './model';

export function usePanelProject(project:Ref<string>) {
  const saved=shallowRef<PanelProject>(),schema=shallowRef<PanelSchema|null>(null),library=shallowRef<PanelLibrary>();
  const busy=shallowRef(false),error=shallowRef(''),undo=shallowRef<Array<PanelSchema|null>>([]),redo=shallowRef<Array<PanelSchema|null>>([]);
  const dirty=computed(()=>!!saved.value&&JSON.stringify(schema.value)!==JSON.stringify(saved.value.schema));
  function update(next:PanelSchema|null){undo.value=[...undo.value,schema.value].slice(-80);redo.value=[];schema.value=next;}
  function restore(direction:'undo'|'redo'){const from=direction==='undo'?undo:redo,to=direction==='undo'?redo:undo;if(!from.value.length)return;to.value=[...to.value,schema.value];schema.value=from.value.at(-1)!;from.value=from.value.slice(0,-1);}
  watch(project,async(value,_,onCleanup)=>{
    let cancelled=false;onCleanup(()=>{cancelled=true;});busy.value=true;error.value='';library.value=undefined;saved.value=undefined;schema.value=null;undo.value=[];redo.value=[];
    try{const state=await readPanelProject(value);if(cancelled)return;saved.value=state;schema.value=state.schema;if(state.schema){const lib=await loadPanelLibrary(value);if(!cancelled)library.value=lib;}}
    catch(e){if(!cancelled)error.value=String(e instanceof Error?e.message:JSON.stringify(e));}
    finally{if(!cancelled)busy.value=false;}
  },{immediate:true});
  async function enable(){if(busy.value)return;const current=project.value;busy.value=true;error.value='';try{const lib=library.value??await loadPanelLibrary(current);if(project.value!==current)return;library.value=lib;update(saved.value?.schema??lib.schema);}catch(e){error.value=String(e instanceof Error?e.message:JSON.stringify(e));}finally{if(project.value===current)busy.value=false;}}
  async function save():Promise<boolean>{
    if(!dirty.value)return true;if(!saved.value||busy.value)return false;const current=project.value;busy.value=true;error.value='';
    try{
      // Unrelated package.json edits may merge; changed panel ownership may not.
      const latest=await readPanelProject(current);
      if(latest.schemaRevision!==saved.value.schemaRevision||JSON.stringify(latest.dependency)!==JSON.stringify(saved.value.dependency))throw Error('PANEL_REVISION_CONFLICT');
      const next=await writePanelProject(current,{schema:schema.value,dependency:library.value?.dependency??saved.value.dependency,manifestRevision:latest.manifestRevision,schemaRevision:saved.value.schemaRevision});
      if(current!==project.value)return false;saved.value=next;schema.value=next.schema;undo.value=[];redo.value=[];return true;
    }catch(e){error.value=String(e instanceof Error?e.message:JSON.stringify(e));return false;}finally{if(current===project.value)busy.value=false;}
  }
  return{schema,library,saved,busy,error,dirty,undo,redo,update,restore,enable,save};
}
