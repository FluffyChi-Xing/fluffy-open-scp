<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { VueFlow, Handle, Position, type Node, type Edge } from "@vue-flow/core";
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
      <Background :gap="20" pattern-color="#333333" />
      <template #node-wb="node">
        <div class="wb-node" :data-role="node.id" :data-tone="nodeTone(node.data)">
          <Handle type="target" :position="Position.Left" />
          <Handle type="source" :position="Position.Right" />
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
/* Blender node-editor look: fixed gray-black palette, independent of the
   app theme. Shared tokens live in styles/flow-presentation.css. */
.blueprint {
  background: var(--flow-bg, #1d1d1d);
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
.wb-node {
  background: var(--flow-node, #282828);
  border: 1px solid var(--flow-border, #141414);
  border-radius: 8px;
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.4);
  color: var(--flow-text, #e6e6e6);
  display: grid;
  gap: 5px;
  font-size: 10.5px;
  min-width: 190px;
  padding: 8px 11px;
}
.wb-node[data-tone="warn"] {
  border-color: #8a6a2a;
}
.wb-node-head {
  align-items: center;
  background: var(--flow-head-process, #2e6b2e);
  border-radius: 7px 7px 0 0;
  color: #f2f2f2;
  display: flex;
  gap: 6px;
  margin: -8px -11px 1px;
  padding: 6px 10px;
}
.wb-node[data-role="source"] .wb-node-head {
  background: var(--flow-head-input, #6e4a2b);
}
.wb-node[data-role="lot"] .wb-node-head {
  background: var(--flow-head-lot, #3d5a6b);
}
.wb-node[data-role="menu"] .wb-node-head {
  background: var(--flow-head-menu, #6b5b2e);
}
.wb-node[data-role="build"] .wb-node-head {
  background: var(--flow-head-output, #7f3030);
}
.wb-node-head strong {
  font-size: 11.5px;
}
.wb-line {
  align-items: baseline;
  display: flex;
  gap: 8px;
  justify-content: space-between;
}
.wb-line span {
  color: var(--flow-text-dim, #9b9b9b);
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
  color: #58a05e;
}
.wb-line em[data-tone="warn"] {
  color: #d9a53d;
}
.wb-line em[data-tone="muted"] {
  color: var(--flow-text-dim, #9b9b9b);
}
.wb-menu {
  border-top: 1px solid var(--flow-field-border, #3a3a3a);
  display: grid;
  gap: 6px;
  padding-top: 7px;
}
.wb-menu-row {
  align-items: center;
  color: var(--flow-text-dim, #9b9b9b);
  cursor: pointer;
  display: flex;
  font-size: 10.5px;
  gap: 6px;
}
.wb-menu-input {
  border: 1px solid var(--flow-field-border, #3a3a3a);
  border-radius: 4px;
  font: inherit;
  font-size: 10.5px;
  min-height: 24px;
  padding: 0 7px;
}
.wb-actions {
  border-top: 1px solid var(--flow-field-border, #3a3a3a);
  display: flex;
  gap: 5px;
  padding-top: 7px;
}
.wb-actions button {
  align-items: center;
  background: var(--flow-field, #1a1a1a);
  border: 1px solid var(--flow-field-border, #3a3a3a);
  border-radius: 4px;
  color: var(--flow-text, #e6e6e6);
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
  background: #333333;
  border-color: #6a6a6a;
}
</style>
