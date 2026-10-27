<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { VueFlow, type Node, type Edge } from "@vue-flow/core";
import { Background } from "@vue-flow/background";
import "@vue-flow/core/dist/style.css";
import "@vue-flow/core/dist/theme-default.css";
import FIcon from "@/components/extensions/FIcon.vue";
import { Switch } from "@/components/ui/switch";
import type { WorkbenchMenuBinding, WorkbenchSourceKind } from "./useWorkbenchAsset";

/**
 * 中栏底部蓝图画布：资产 → 地块 → 游戏组件 → 菜单绑定 → 打包 的真实
 * 状态管线图。节点数据全部来自工作台活体 schema 文档（非演示数据）；
 * 菜单绑定节点可就地编辑（启用开关用项目 Switch 组件）。
 */
const props = defineProps<{
  sourceKind: WorkbenchSourceKind;
  sourceLabel: string;
  unitCounts: { lights: number; props: number; effects: number; spawners: number; paths: number };
  lotSize: [number, number] | null;
  menu: WorkbenchMenuBinding;
  savedDirty: boolean;
  documentSaved: boolean;
}>();
const emit = defineEmits<{
  "update:menu": [patch: Partial<WorkbenchMenuBinding>];
  "import-model": [];
  "open-tgi": [];
}>();

const { t } = useI18n();

interface WbNodeData {
  icon: string;
  title: string;
  lines: { label: string; value: string; tone?: "ok" | "warn" | "muted" }[];
  slot?: "menu";
}

const nodes = computed<Node<WbNodeData>[]>(() => {
  const sourceReady = props.sourceKind !== "none";
  const sourceValue =
    props.sourceKind === "native"
      ? t("studio.assetWorkbench.pipelineSourceTgi")
      : props.sourceKind === "glb" || props.sourceKind === "lotm"
        ? t("studio.assetWorkbench.pipelineSourceGlb")
        : t("studio.assetWorkbench.pipelineSourceEmpty");
  const unitTotal =
    props.unitCounts.lights +
    props.unitCounts.props +
    props.unitCounts.effects +
    props.unitCounts.spawners +
    props.unitCounts.paths;
  return [
    {
      id: "source",
      type: "wb",
      position: { x: 0, y: 40 },
      data: {
        icon: props.sourceKind === "native" ? "PackageSearch" : "Box",
        title: t("studio.assetWorkbench.pipelineSource"),
        lines: [
          { label: t("studio.assetWorkbench.pipelineSourceKind"), value: sourceValue, tone: sourceReady ? "ok" : "warn" },
          { label: t("studio.assetWorkbench.pipelineSourceName"), value: props.sourceLabel || "—", tone: "muted" },
        ],
      },
    },
    {
      id: "lot",
      type: "wb",
      position: { x: 250, y: 40 },
      data: {
        icon: "LandPlot",
        title: t("studio.assetWorkbench.pipelineLot"),
        lines: [
          {
            label: t("studio.assetWorkbench.pipelineLotSize"),
            value: props.lotSize ? `${props.lotSize[0]} × ${props.lotSize[1]} m` : "—",
            tone: props.lotSize ? "ok" : "warn",
          },
        ],
      },
    },
    {
      id: "units",
      type: "wb",
      position: { x: 500, y: 40 },
      data: {
        icon: "Boxes",
        title: t("studio.assetWorkbench.pipelineUnits"),
        lines: [
          { label: t("package.groupLights"), value: String(props.unitCounts.lights), tone: "muted" },
          { label: t("package.groupProps"), value: String(props.unitCounts.props), tone: "muted" },
          { label: t("package.groupEffects"), value: String(props.unitCounts.effects), tone: "muted" },
          { label: t("package.groupSpawners"), value: String(props.unitCounts.spawners), tone: "muted" },
          { label: t("package.groupPaths"), value: String(props.unitCounts.paths), tone: "muted" },
          { label: t("studio.assetWorkbench.pipelineUnitsTotal"), value: String(unitTotal), tone: unitTotal ? "ok" : "warn" },
        ],
      },
    },
    {
      id: "menu",
      type: "wb",
      position: { x: 750, y: 40 },
      data: {
        icon: "Menu",
        title: t("studio.assetWorkbench.pipelineMenu"),
        lines: [],
        slot: "menu",
      },
    },
    {
      id: "build",
      type: "wb",
      position: { x: 1000, y: 40 },
      data: {
        icon: "PackageCheck",
        title: t("studio.assetWorkbench.pipelineBuild"),
        lines: [
          {
            label: t("studio.assetWorkbench.pipelineDocState"),
            value: props.savedDirty
              ? t("studio.assetWorkbench.pipelineUnsaved")
              : props.documentSaved
                ? t("studio.assetWorkbench.pipelineSaved")
                : t("studio.assetWorkbench.pipelineNoDoc"),
            tone: props.savedDirty || !props.documentSaved ? "warn" : "ok",
          },
          {
            label: t("studio.assetWorkbench.pipelineCompile"),
            value: t("studio.assetWorkbench.pipelineCompileNote"),
            tone: "muted",
          },
        ],
      },
    },
  ];
});

const edges = computed<Edge[]>(() => [
  { id: "e-source-lot", source: "source", target: "lot", animated: !props.sourceKind || props.sourceKind === "none" },
  { id: "e-lot-units", source: "lot", target: "units" },
  { id: "e-units-menu", source: "units", target: "menu" },
  { id: "e-menu-build", source: "menu", target: "build" },
]);

/** VueFlow 插槽入参无类型：节点警示 tone 由行数据推导。 */
function nodeTone(data: unknown): "warn" | "ok" {
  const lines = (data as WbNodeData).lines ?? [];
  return lines.some((line) => line.tone === "warn") ? "warn" : "ok";
}
</script>

<template>
  <div class="blueprint" :aria-label="t('studio.assetWorkbench.pipeline')">
    <VueFlow
      :nodes="nodes"
      :edges="edges"
      :nodes-connectable="false"
      :edges-updatable="false"
      :delete-key-code="null"
      fit-view-on-init
      :max-zoom="1.4"
      :min-zoom="0.3"
      class="blueprint-flow"
    >
      <Background :gap="18" pattern-color="color-mix(in srgb, var(--primary) 16%, transparent)" />
      <template #node-wb="node">
        <div class="wb-node" :data-tone="nodeTone(node.data)">
          <header class="wb-node-head">
            <FIcon :name="node.data.icon" :size="13" aria-label="" />
            <strong>{{ node.data.title }}</strong>
          </header>
          <div v-for="line in node.data.lines" :key="line.label" class="wb-line">
            <span>{{ line.label }}</span>
            <em :data-tone="line.tone ?? 'muted'">{{ line.value }}</em>
          </div>
          <div v-if="node.data.slot === 'menu'" class="wb-menu">
            <label class="wb-menu-row">
              <Switch
                :model-value="menu.enabled"
                :aria-label="t('studio.assetWorkbench.menuEnabled')"
                @update:model-value="emit('update:menu', { enabled: Boolean($event) })"
              />
              <span>{{ t("studio.assetWorkbench.menuEnabled") }}</span>
            </label>
            <input
              :value="menu.label"
              type="text"
              class="wb-menu-input"
              :placeholder="t('studio.assetWorkbench.menuLabelPlaceholder')"
              :aria-label="t('studio.assetWorkbench.menuLabel')"
              @input="emit('update:menu', { label: ($event.target as HTMLInputElement).value })"
            />
            <input
              :value="menu.category"
              type="text"
              class="wb-menu-input"
              :placeholder="t('studio.assetWorkbench.menuCategoryPlaceholder')"
              :aria-label="t('studio.assetWorkbench.menuCategory')"
              @input="emit('update:menu', { category: ($event.target as HTMLInputElement).value })"
            />
          </div>
          <div v-if="node.id === 'source' && sourceKind === 'none'" class="wb-actions">
            <button type="button" @click="emit('import-model')">
              <FIcon name="Upload" :size="11" aria-label="" />
              {{ t("studio.assetWorkbench.importFile") }}
            </button>
            <button type="button" @click="emit('open-tgi')">
              <FIcon name="Hash" :size="11" aria-label="" />
              {{ t("studio.assetWorkbench.tgiParse") }}
            </button>
          </div>
        </div>
      </template>
    </VueFlow>
  </div>
</template>

<style scoped>
.blueprint {
  background: var(--surface);
  height: 100%;
  min-height: 0;
  position: relative;
}
.blueprint-flow {
  height: 100%;
  width: 100%;
}
.blueprint :deep(.vue-flow__node) {
  cursor: default;
}
.blueprint :deep(.vue-flow__edge-path) {
  stroke: color-mix(in srgb, var(--primary) 55%, var(--border));
  stroke-width: 1.6;
}
.wb-node {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-sm);
  display: grid;
  gap: 5px;
  font-size: 10.5px;
  min-width: 190px;
  padding: 9px 11px;
}
.wb-node[data-tone="warn"] {
  border-color: color-mix(in srgb, var(--warning) 55%, var(--border));
}
.wb-node-head {
  align-items: center;
  color: var(--primary);
  display: flex;
  gap: 6px;
}
.wb-node-head strong {
  color: var(--foreground);
  font-size: 11.5px;
}
.wb-line {
  align-items: baseline;
  display: flex;
  gap: 8px;
  justify-content: space-between;
}
.wb-line span {
  color: var(--muted-foreground);
}
.wb-line em {
  font-style: normal;
  font-variant-numeric: tabular-nums;
  max-width: 130px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.wb-line em[data-tone="ok"] {
  color: var(--success);
}
.wb-line em[data-tone="warn"] {
  color: var(--warning);
}
.wb-line em[data-tone="muted"] {
  color: var(--muted-foreground);
}
.wb-menu {
  border-top: 1px solid var(--border);
  display: grid;
  gap: 6px;
  padding-top: 7px;
}
.wb-menu-row {
  align-items: center;
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font-size: 10.5px;
  gap: 6px;
}
.wb-menu-input {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 10.5px;
  min-height: 24px;
  padding: 0 7px;
}
.wb-actions {
  border-top: 1px solid var(--border);
  display: flex;
  gap: 5px;
  padding-top: 7px;
}
.wb-actions button {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  flex: 1;
  font: inherit;
  font-size: 10.5px;
  gap: 4px;
  justify-content: center;
  min-height: 25px;
  padding: 0 7px;
}
.wb-actions button:hover {
  background: var(--surface-hover);
  border-color: var(--primary);
  color: var(--primary);
}
</style>
