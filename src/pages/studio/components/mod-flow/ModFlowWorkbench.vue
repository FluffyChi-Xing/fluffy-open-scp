<script setup lang="ts">
import { computed, ref, shallowRef, toRef, watch, useId } from "vue";
import { useI18n } from "vue-i18n";
import {
  VueFlow,
  useVueFlow,
  type Connection,
  type NodeDragEvent,
} from "@vue-flow/core";
import { Background } from "@vue-flow/background";
import "@vue-flow/core/dist/style.css";
import "@vue-flow/core/dist/theme-default.css";
import { useModFlow } from "./useModFlow";
import FlowNodeEditor from "./FlowNodeEditor.vue";
import WorkflowNode from "./WorkflowNode.vue";
import type { FlowSchema } from "./contracts";
const props = defineProps<{ project: string; selectedPath?: string }>();
const emit = defineEmits<{ status: [ready: boolean]; saved: [] }>();
const { t } = useI18n();
const {
  state,
  busy,
  error,
  artifact,
  canBuild,
  refresh,
  save,
  saveGraph,
  build,
} = useModFlow(toRef(props, "project"));
const graphId = `mod-flow-${useId()}`;
function frameNode(id: string) {
  updateNodeInternals([id]);
  requestAnimationFrame(() => {
    requestAnimationFrame(() => { void fitView({ nodes: [id], padding: 0.2, maxZoom: 1, duration: 200 }); });
  });
}
const { fitView, getNodes, onNodesInitialized, updateNodeInternals } = useVueFlow({ id: graphId });
let checkedLayout = false;
onNodesInitialized(() => {
  if (checkedLayout || !state.value) return;
  checkedLayout = true;
  const current = getNodes.value;
  if (
    current.some((a, i) =>
      current
        .slice(i + 1)
        .some(
          (b) =>
            a.position.x < b.position.x + b.dimensions.width + 20 &&
            a.position.x + a.dimensions.width + 20 > b.position.x &&
            a.position.y < b.position.y + b.dimensions.height + 20 &&
            a.position.y + a.dimensions.height + 20 > b.position.y,
        ),
    )
  )
    void arrange();
});
const dirtyNodes = ref(new Set<string>());
const readyToBuild = computed(
  () => canBuild.value && dirtyNodes.value.size === 0,
);
function markDirty(id: string, dirty: boolean) {
  const next = new Set(dirtyNodes.value);
  if (dirty) next.add(id);
  else next.delete(id);
  dirtyNodes.value = next;
}
async function arrange() {
  if (!state.value || busy.value) return;
  const width =
    Math.max(420, ...getNodes.value.map((n) => n.dimensions.width)) + 90;
  const height =
    Math.max(320, ...getNodes.value.map((n) => n.dimensions.height)) + 80;
  await saveGraph(
    state.value.nodes.map((n, i) => ({
      ...n,
      position: [40 + (i % 3) * width, 60 + Math.floor(i / 3) * height],
    })),
    dependencies.value,
  );
  void fitView({ padding: 0.12, maxZoom: 1, duration: 200 });
}
const fullscreen = shallowRef(false);
const editorOpen = shallowRef(false);
const edited = ref<FlowSchema | null>(null);
const focused = computed(() =>
  props.selectedPath
    ? state.value?.nodes.find(
        (n) =>
          n.schema === props.selectedPath ||
          state.value?.schemas.find((s) => s.id === n.id)?.config.asset ===
            props.selectedPath,
      )?.id
    : undefined,
);
const nodes = computed(
  () =>
    state.value?.nodes.map((n) => ({
      id: n.id,
      type: "workflow",
      position: { x: n.position[0], y: n.position[1] },
      data: {
        kind: n.kind,
        schema: state.value?.schemas.find((s) => s.id === n.id),
        invalid: state.value?.diagnostics.some((d) => d.node === n.id),
      },
      selected: n.id === focused.value,
    })) ?? [],
);
const dependencies = computed(
  () =>
    (state.value?.manifest.internal_dependencies ?? {}) as Record<
      string,
      string[]
    >,
);
const edges = computed(() =>
  Object.entries(dependencies.value).flatMap(([target, parents]) =>
    parents.map((source) => ({ id: `${source}:${target}`, source, target })),
  ),
);
watch(readyToBuild, (value) => emit("status", value), { immediate: true });
watch(focused, (id) => {
  if (id)
    void fitView({ nodes: [id], padding: 1.4, maxZoom: 1.1, duration: 250 });
});
function edit(id: string) {
  edited.value = state.value?.schemas.find((s) => s.id === id) ?? null;
  editorOpen.value = !!edited.value;
}
async function saveNode(schema: FlowSchema) {
  const result = await save(schema);
  if (result) {
    editorOpen.value = false;
    emit("saved");
  }
}
async function connect(connection: Connection) {
  if (
    !state.value ||
    !connection.source ||
    !connection.target ||
    connection.source === connection.target
  )
    return;
  if (
    state.value.nodes.some(
      (n) =>
        (n.id === connection.target &&
          ["map-source", "static-resource"].includes(n.kind)) ||
        (n.id === connection.source && n.kind === "output"),
    )
  )
    return;
  const deps = structuredClone(dependencies.value);
  deps[connection.target] = [
    ...new Set([...(deps[connection.target] ?? []), connection.source]),
  ];
  await saveGraph(state.value.nodes, deps);
}
async function move(event: NodeDragEvent) {
  if (!state.value) return;
  await saveGraph(
    state.value.nodes.map((n) =>
      n.id === event.node.id
        ? { ...n, position: [event.node.position.x, event.node.position.y] }
        : n,
    ),
    dependencies.value,
  );
}
async function removeEdge(id: string) {
  if (!state.value) return;
  const edge = edges.value.find((e) => e.id === id);
  if (!edge) return;
  const deps = structuredClone(dependencies.value);
  deps[edge.target] = deps[edge.target]!.filter((x) => x !== edge.source);
  await saveGraph(state.value.nodes, deps);
}
defineExpose({
  build: () => (readyToBuild.value ? build() : undefined),
  refresh,
});
</script>
<template>
  <section
    class="flow-workbench"
    :class="{ fullscreen }"
    :aria-label="t('flow.title')"
  >
    <header class="flow-toolbar">
      <span>{{ t("flow.title") }}</span>
      <div>
        <button :disabled="busy" @click="arrange">
          {{ t("flow.arrange") }}
        </button>
        <button :disabled="busy" @click="refresh">
          {{ t("flow.validate") }}</button
        ><button :aria-pressed="fullscreen" @click="fullscreen = !fullscreen">
          {{ t("flow.fullscreen") }}
        </button>
      </div>
    </header>
    <p class="flow-hint">
      {{
        t(
          selectedPath && !focused && selectedPath !== "package.json"
            ? "flow.unmapped"
            : "flow.fileHint",
        )
      }}
    </p>
    <VueFlow
      :id="graphId"
      :nodes="nodes"
      :edges="edges"
      :nodes-draggable="!busy"
      :nodes-connectable="!busy"
      :delete-key-code="null"
      fit-view-on-init
      @connect="connect"
      @node-drag-stop="move"
      @edge-double-click="({ edge }) => removeEdge(edge.id)"
    >
      <Background :gap="20" pattern-color="var(--border)" />
      <template #node-workflow="{ id, data }">
        <WorkflowNode
          :kind="data.kind"
          :schema="data.schema"
          :invalid="data.invalid"
          :busy="busy"
          @edit="edit(id)"
          @save="saveNode"
          @dirty="markDirty(id, $event)"
          @resize="frameNode(id)"
        />
      </template>
    </VueFlow>
    <footer class="flow-result" aria-live="polite">
      <p v-if="error" role="alert">{{ error }}</p>
      <p>
        {{
          t(
            busy
              ? "flow.busy"
              : dirtyNodes.size
                ? "flow.unsaved"
                : canBuild
                  ? "flow.ready"
                  : "flow.blocked",
          )
        }}
      </p>
      <ul v-if="state?.diagnostics.length">
        <li v-for="(diagnostic, i) in state.diagnostics" :key="i">
          <button v-if="diagnostic.node" @click="edit(diagnostic.node)">
            {{ diagnostic.node }}
          </button>
          {{ diagnostic.detail }}
        </li>
      </ul>
      <p v-if="artifact">
        {{ t("flow.artifact") }}: <code>{{ artifact }}</code>
      </p>
    </footer>
    <FlowNodeEditor
      v-if="editorOpen"
      v-model:open="editorOpen"
      :schema="edited"
      :busy="busy"
      @save="saveNode"
    />
  </section>
</template>
<style scoped>
.flow-workbench {
  height: 100%;
  min-height: 400px;
  display: flex;
  flex-direction: column;
  background: var(--surface);
  color: var(--foreground);
}
.flow-workbench.fullscreen {
  position: fixed;
  inset: 12px;
  z-index: 70;
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: var(--shadow-md);
}
.flow-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
  gap: 12px;
}
.flow-toolbar div {
  display: flex;
  gap: 8px;
}
.flow-hint {
  font-size: 12px;
  color: var(--muted-foreground);
  margin: 10px 16px;
}
.flow-workbench :deep(.vue-flow) {
  flex: 1;
  min-height: 220px;
}
.flow-workbench :deep(.selected .workflow-node) {
  outline: 2px solid var(--primary, #6095dc);
  outline-offset: 3px;
}
.flow-workbench button {
  border: 1px solid var(--border);
  background: var(--surface);
  color: inherit;
  border-radius: 5px;
  min-height: 26px;
  padding: 0 8px;
  cursor: pointer;
  font-size: 11px;
}
.flow-workbench button:disabled {
  opacity: 0.5;
  cursor: wait;
}
.flow-result {
  padding: 10px 16px;
  border-top: 1px solid var(--border);
  max-height: 150px;
  overflow: auto;
  font-size: 12px;
}
.flow-result p {
  margin: 4px 0;
}
.flow-result ul {
  padding-left: 16px;
}
.flow-result li {
  margin: 6px 0;
}
.flow-result code {
  overflow-wrap: anywhere;
}
</style>
