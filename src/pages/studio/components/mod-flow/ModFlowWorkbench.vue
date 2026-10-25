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
import MapGenerationNode from "./MapGenerationNode.vue";
import { useMapGraph } from "./useMapGraph";
import FDropdown from "@/components/ui/FDropdown.vue";
import type { MapNodeKind } from "@/lib/map-graph";
import type { FlowSchema } from "./contracts";
import FIcon from "@/components/extensions/FIcon.vue";
const props = defineProps<{ project: string; selectedPath?: string }>();
const emit = defineEmits<{ status: [ready: boolean]; saved: []; focus: [] }>();
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
const generation = useMapGraph(toRef(props, "project"));
const generationMenu = ref(false);
const generationKinds: MapNodeKind[] = ["extent", "perlin", "value", "image", "blend", "grass", "trees", "output"];
const assetMode = computed(() => state.value?.manifest.mod_type === "assets");
const assetWorkflowSteps = [
  { id: "import", icon: "Upload", label: "flow.assetSteps.import" },
  { id: "slots", icon: "Layers", label: "flow.assetSteps.slots" },
  { id: "property", icon: "SlidersHorizontal", label: "flow.assetSteps.property" },
  { id: "bindings", icon: "Boxes", label: "flow.assetSteps.bindings" },
  { id: "menu", icon: "Menu", label: "flow.assetSteps.menu" },
  { id: "build", icon: "Package", label: "flow.assetSteps.build" },
] as const;
function addGeneration(kind: MapNodeKind) { generationMenu.value = false; frameNode(generation.add(kind)); }
function showGeneration() {
  generation.visible.value = !generation.visible.value;
  if (generation.visible.value) requestAnimationFrame(() => requestAnimationFrame(() => { void fitView({nodes: generation.graph.value.nodes.map(n => n.id), padding: 0.12, maxZoom: 1, duration: 200}); }));
}
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
  () => canBuild.value && dirtyNodes.value.size === 0 && !generation.visible.value,
);
function markDirty(id: string, dirty: boolean) {
  const next = new Set(dirtyNodes.value);
  if (dirty) next.add(id);
  else next.delete(id);
  dirtyNodes.value = next;
}
async function arrange() {
  if (generation.visible.value) {
    generation.graph.value.nodes.forEach((node, index) => generation.move(node.id, {x: index * 370, y: -1100}));
    void fitView({nodes: generation.graph.value.nodes.map(n => n.id), padding: 0.12, maxZoom: 1, duration: 200});
    return;
  }
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
    parents.flatMap((source) => {
      const schema = state.value?.schemas.find(s => s.id === target);
      const slots = schema?.kind === 'building-asset' ? Object.entries(schema.config).filter(([k, v]) => /^slot[0-5]$/.test(k) && v === source).map(([k]) => k) : [undefined];
      return slots.map(targetHandle => ({ id: `${source}:${target}:${targetHandle ?? ''}`, source, target, targetHandle }));
    }),
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
  if (connection.source?.startsWith("mapgen-") || connection.target?.startsWith("mapgen-")) { generation.connect(connection); return; }
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
          ["map-source", "static-resource", "texture-input"].includes(n.kind)) ||
        (n.id === connection.source && n.kind === "output"),
    )
  )
    return;
  const targetSchema = state.value.schemas.find(s => s.id === connection.target);
  if (targetSchema?.kind === 'building-asset') {
    if (!/^slot[0-5]$/.test(connection.targetHandle ?? '') || !state.value.nodes.some(n => n.id === connection.source && n.kind === 'texture-input')) return;
    const updated = { ...targetSchema, config: { ...targetSchema.config, [connection.targetHandle!]: connection.source } };
    if (!await save(updated)) return;
    const deps = structuredClone(dependencies.value);
    deps[connection.target] = [...new Set(Object.entries(updated.config).filter(([k]) => /^slot[0-5]$/.test(k)).map(([, v]) => String(v)))];
    await saveGraph(state.value.nodes, deps);
    return;
  }
  const deps = structuredClone(dependencies.value);
  deps[connection.target] = [
    ...new Set([...(deps[connection.target] ?? []), connection.source]),
  ];
  await saveGraph(state.value.nodes, deps);
}
async function move(event: NodeDragEvent) {
  if (event.node.id.startsWith("mapgen-")) { generation.move(event.node.id, event.node.position); return; }
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
  if (id.startsWith("mapgen-")) { generation.remove(id); return; }
  if (!state.value) return;
  const edge = edges.value.find((e) => e.id === id);
  if (!edge) return;
  const target = state.value.schemas.find(s => s.id === edge.target);
  if (target?.kind === "building-asset" && edge.targetHandle) {
    const updated = { ...target, config: { ...target.config, [edge.targetHandle]: "" } };
    if (!await save(updated)) return;
    const deps = structuredClone(dependencies.value);
    deps[edge.target] = [...new Set(Object.entries(updated.config).filter(([k, v]) => /^slot[0-5]$/.test(k) && v).map(([, v]) => String(v)))];
    await saveGraph(state.value.nodes, deps); return;
  }
  const deps = structuredClone(dependencies.value);
  deps[edge.target] = deps[edge.target]!.filter((x) => x !== edge.source);
  await saveGraph(state.value.nodes, deps);
}
defineExpose({
  build: () => (readyToBuild.value ? build() : undefined),
  buildArtifact: async () => {
    if (!readyToBuild.value) return undefined;
    await build();
    return artifact.value || undefined;
  },
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
      <span>{{ t(assetMode ? "flow.assetWorkflowTitle" : "flow.title") }}</span>
      <div>
        <button v-if="!assetMode" :aria-pressed="generation.visible.value" @click="showGeneration">{{ t("mapGraph.title") }}</button>
        <FDropdown v-if="generation.visible.value" v-model:open="generationMenu"><template #trigger><button>{{ t("mapGraph.add") }}</button></template><button v-for="kind in generationKinds" :key="kind" @click="addGeneration(kind)">{{ t(`mapGraph.nodes.${kind}`) }}</button></FDropdown>
        <button v-if="!assetMode" :disabled="busy" @click="arrange">
          {{ t("flow.arrange") }}
        </button>
        <button :disabled="busy" @click="refresh">{{ t("flow.validate") }}</button>
        <button :title="t('flow.focus')" @click="emit('focus')">
          <FIcon name="PanelBottom" :size="14" aria-label="" />
          <span class="sr-only">{{ t("flow.focus") }}</span>
        </button>
        <button :aria-pressed="fullscreen" :title="t('flow.fullscreen')" @click="fullscreen = !fullscreen">
          <FIcon :name="fullscreen ? 'Minimize2' : 'Maximize2'" :size="14" aria-label="" />
          <span class="sr-only">{{ t("flow.fullscreen") }}</span>
        </button>
      </div>
    </header>
    <p v-if="!assetMode" class="flow-hint">
      {{
        t(
          selectedPath && !focused && selectedPath !== "package.json"
            ? "flow.unmapped"
            : "flow.fileHint",
        )
      }}
    </p>
    <div v-if="assetMode" class="asset-workflow-canvas" :aria-label="t('flow.assetWorkflowCanvas')">
      <div class="asset-workflow-track">
        <template v-for="(step, index) in assetWorkflowSteps" :key="step.id">
          <button type="button" class="asset-workflow-step" :class="{ active: step.id === 'property' }" @click="step.id === 'property' && state?.schemas.find((schema) => schema.kind === 'building-asset') && edit(state.schemas.find((schema) => schema.kind === 'building-asset')!.id)">
            <span class="asset-workflow-index">{{ index + 1 }}</span>
            <FIcon :name="step.icon" :size="17" aria-label="" />
            <strong>{{ t(step.label) }}</strong>
            <small>{{ t(`flow.assetSteps.${step.id}Hint`) }}</small>
          </button>
          <span v-if="index < assetWorkflowSteps.length - 1" class="asset-workflow-arrow" aria-hidden="true">→</span>
        </template>
      </div>
      <p class="asset-workflow-note">{{ t('flow.assetWorkflowHint') }}</p>
    </div>
    <VueFlow
      v-else
      :id="graphId"
      :nodes="[...nodes, ...generation.nodes.value]"
      :edges="[...edges, ...generation.edges.value]"
      :nodes-draggable="!busy"
      :nodes-connectable="!busy"
      :delete-key-code="null"
      fit-view-on-init
      @connect="connect"
      @node-drag-stop="move"
      @edge-double-click="({ edge }) => removeEdge(edge.id)"
    >
      <Background :gap="20" pattern-color="var(--border)" />
      <template #node-map-generation="{ data }"><MapGenerationNode :node="data.node" :field="data.field" :error="data.error" @update="generation.update" @remove="generation.remove(data.node.id)" /></template>
      <template #node-workflow="{ id, data }">
        <WorkflowNode
          :kind="data.kind"
          :project="project"
          :schemas="state?.schemas ?? []"
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
      <p v-if="generation.visible.value">{{ t("mapGraph.hint") }}</p>
      <p v-if="generation.error.value" role="alert">{{ t(`mapGraph.errors.${generation.error.value}`) }}</p>
      <p v-if="error && !generation.visible.value" role="alert">{{ error }}</p>
      <p v-if="!generation.visible.value">
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
      <ul v-if="state?.diagnostics.length && !generation.visible.value">
        <li v-for="(diagnostic, i) in state.diagnostics" :key="i">
          <button v-if="diagnostic.node" @click="edit(diagnostic.node)">
            {{ diagnostic.node }}
          </button>
          {{ diagnostic.detail === 'rasterEditor.buildUnsupported' ? t('rasterEditor.buildUnsupported') : diagnostic.detail }}
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
      :project="project"
      :schemas="state?.schemas ?? []"
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
.asset-flow-strip { align-items: center; background: color-mix(in srgb, var(--primary) 7%, var(--surface)); border-bottom: 1px solid var(--border); color: var(--muted-foreground); display: flex; flex-wrap: wrap; font-size: 11px; gap: 7px; padding: 7px 16px; }
.asset-flow-strip span { color: var(--foreground); }
.asset-flow-strip b { color: var(--primary); font-weight: 600; }
.flow-workbench :deep(.vue-flow) {
  flex: 1;
  min-height: 220px;
}
.flow-workbench :deep(.selected .workflow-node) {
  outline: 2px solid var(--primary, #6095dc);
  outline-offset: 3px;
}
.asset-workflow-canvas { background: color-mix(in srgb, var(--surface) 90%, var(--primary)); display: flex; flex: 1; flex-direction: column; justify-content: center; min-height: 220px; overflow: auto; padding: 24px; }
.asset-workflow-track { align-items: stretch; display: flex; justify-content: center; min-width: 820px; }
.asset-workflow-step { align-items: flex-start; background: var(--surface-elevated); border: 1px solid var(--border); border-radius: var(--radius-md); display: flex; flex: 1 1 140px; flex-direction: column; gap: 6px; min-height: 118px; padding: 12px; position: relative; text-align: start; }
.asset-workflow-step:hover, .asset-workflow-step.active { border-color: var(--primary); box-shadow: 0 0 0 1px color-mix(in srgb, var(--primary) 25%, transparent); }
.asset-workflow-step strong { font-size: 12px; }
.asset-workflow-step small { color: var(--muted-foreground); font-size: 10px; line-height: 1.4; }
.asset-workflow-index { align-items: center; background: var(--primary); border-radius: 999px; color: var(--primary-foreground); display: inline-flex; font-size: 10px; height: 19px; justify-content: center; width: 19px; }
.asset-workflow-arrow { align-self: center; color: var(--primary); flex: 0 0 28px; font-size: 18px; text-align: center; }
.asset-workflow-note { color: var(--muted-foreground); font-size: 11px; margin: 22px auto 0; max-width: 650px; text-align: center; }
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
