import { computed, shallowRef, watch, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { FlowNode, FlowSchema, FlowState } from "./contracts";

export function useModFlow(project: Ref<string>) {
  const state = shallowRef<FlowState | null>(null);
  const busy = shallowRef(false);
  const error = shallowRef("");
  const artifact = shallowRef("");
  let generation = 0;
  const canBuild = computed(() => !!state.value?.im && !busy.value);
  async function run(action: string, payload: unknown = {}) {
    if (busy.value) return;
    const current = generation;
    busy.value = true;
    error.value = "";
    try {
      const result = await invoke<FlowState | { path: string }>("code_flow", {
        request: { project: project.value, action, payload },
      });
      if (current !== generation) return;
      if ("path" in result) artifact.value = result.path;
      else state.value = result;
      return result;
    } catch (cause) {
      if (current === generation)
        error.value = typeof cause === "string" ? cause : JSON.stringify(cause);
    } finally {
      if (current === generation) busy.value = false;
    }
  }
  const refresh = () => run("inspect");
  const save = (schema: FlowSchema) =>
    run("save-node", { revision: state.value?.revision, schema });
  const saveGraph = (
    nodes: FlowNode[],
    dependencies: Record<string, string[]>,
  ) =>
    run("save-graph", {
      revision: state.value?.revision,
      nodes,
      internal_dependencies: dependencies,
    });
  const build = () =>
    canBuild.value ? run("build", { im: state.value?.im }) : undefined;
  watch(
    project,
    () => {
      generation++;
      busy.value = false;
      state.value = null;
      artifact.value = "";
      void refresh();
    },
    { immediate: true },
  );
  return {
    state,
    busy,
    error,
    artifact,
    canBuild,
    run,
    refresh,
    save,
    saveGraph,
    build,
  };
}
