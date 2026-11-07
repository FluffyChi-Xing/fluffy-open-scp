<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { command } from "@/api/tauri";
import { tauriApi } from "@/api";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  VueFlow,
  Handle,
  Position,
  useVueFlow,
  type Connection,
  type Edge,
  type Node,
  type NodeDragEvent,
  type NodeMouseEvent,
} from "@vue-flow/core";
import { Background } from "@vue-flow/background";
import "@vue-flow/core/dist/style.css";
import "@vue-flow/core/dist/theme-default.css";
import FIcon from "@/components/extensions/FIcon.vue";
import FDropdownSelect from "@/components/ui/FSelect.vue";
import {
  menuBindingIssues,
  menuTargetsFor,
  newMenuEntry,
  type MenuDimension,
  type WorkbenchMenuBinding,
  type WorkbenchMenuEntry,
} from "./menuBinding";
import type { WorkbenchSourceKind } from "./useWorkbenchAsset";
import LogicNodeCard from "./LogicNodeCard.vue";
import { useToast } from "@/composables/useToast";
import {
  LOGIC_NODE_SPECS,
  addLogicNode,
  connectLogic,
  duplicateLogicNode,
  ensureCoreNode,
  leafSpec,
  logicIssues,
  removeLogicNode,
  updateLogicParams,
  type BehaviorLeafType,
  type LogicGraph,
  type LogicNodeSpec,
} from "./logicGraph";

/**
 * 中栏底部蓝图画布：资产 → 地块 → 游戏组件 → 菜单绑定 → 打包 的真实
 * 状态管线图。节点数据全部来自工作台活体 schema 文档（非演示数据）；
 * 菜单绑定节点可就地编辑（启用开关用项目 Switch 组件）。启用后按
 * menu.entries 派生一列菜单编辑节点（每个目标菜单容器一个，只决定
 * 「放在哪儿」），并从菜单绑定节点的 source 端口连线。
 */
const props = defineProps<{
  sourceKind: WorkbenchSourceKind;
  sourceLabel: string;
  unitCounts: { lights: number; props: number; effects: number; spawners: number; paths: number };
  lotSize: [number, number] | null;
  menu: WorkbenchMenuBinding;
  /** 逻辑编排图（建筑信息/行为聚合/行为叶子）；画布是唯一编辑器。 */
  logic: LogicGraph;
  savedDirty: boolean;
  documentSaved: boolean;
  /** 项目根绝对路径：菜单图标/背景图预览用（convertFileSrc 拼接）。 */
  projectRoot?: string;
  /** 项目（code_flow 导入用）。 */
  project?: string;
}>();
const emit = defineEmits<{
  "update:menu": [patch: Partial<WorkbenchMenuBinding>];
  /** 整图回写（纯函数结果）；工作台负责持久化到 editor.workbench.logic。 */
  "update:logic": [graph: LogicGraph];
  "import-model": [];
  "open-tgi": [];
}>();

const { t } = useI18n();
const toast = useToast();
const { screenToFlowCoordinate } = useVueFlow();

interface WbNodeData {
  icon: string;
  title: string;
  lines: { label: string; value: string; tone?: "ok" | "warn" | "muted" }[];
  slot?: "menu";
}
interface WbEntryNodeData {
  entry: WorkbenchMenuEntry;
}

const nodes = computed<Node[]>(() => {
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
  const menuIssues = menuBindingIssues(props.menu);
  const menuStateLine: WbNodeData["lines"][number] = !props.menu.enabled
    ? { label: t("studio.assetWorkbench.pipelineMenuState"), value: t("studio.assetWorkbench.menuStateOff"), tone: "muted" }
    : menuIssues.length
      ? { label: t("studio.assetWorkbench.pipelineMenuState"), value: t("studio.assetWorkbench.menuStateIncomplete"), tone: "warn" }
      : {
          label: t("studio.assetWorkbench.pipelineMenuState"),
          value: t("studio.assetWorkbench.menuStateEntries", { n: props.menu.entries.length }),
          tone: "ok",
        };
  const pipelineNodes: Node<WbNodeData>[] = [
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
          menuStateLine,
          {
            label: t("studio.assetWorkbench.pipelineCompile"),
            value: t("studio.assetWorkbench.pipelineCompileNote"),
            tone: "muted",
          },
        ],
      },
    },
  ];
  // 菜单编辑节点组：每个目标菜单容器一个节点，排在菜单绑定节点下方一列。
  const entryNodes: Node<WbEntryNodeData>[] = props.menu.enabled
    ? props.menu.entries.map((entry, index) => ({
        id: `entry-${entry.id}`,
        type: "menu-entry",
        position: { x: 750, y: 340 + index * 210 },
        data: { entry },
      }))
    : [];
  const logicNodes: Node[] = props.logic.nodes.map((node) => ({
    id: node.id,
    type: "logic",
    position: { x: node.position[0], y: node.position[1] },
    draggable: true,
    data: { node, params: props.logic.params[node.id] },
  }));
  return [
    ...pipelineNodes,
    ...entryNodes,
    ...logicNodes,
  ];
});

const edges = computed<Edge[]>(() => [
  { id: "e-source-lot", source: "source", target: "lot", animated: !props.sourceKind || props.sourceKind === "none" },
  { id: "e-lot-units", source: "lot", target: "units" },
  { id: "e-units-menu", source: "units", target: "menu" },
  { id: "e-menu-build", source: "menu", target: "build" },
  ...(props.menu.enabled
    ? props.menu.entries.map((entry) => ({
        id: `e-menu-${entry.id}`,
        source: "menu",
        target: `entry-${entry.id}`,
      }))
    : []),
  ...props.logic.edges.map((edge) => ({
    id: edge.id,
    source: edge.source,
    // 文档以 "asset" 作为逻辑锚点；画布上的资产节点 id 是 "units"。
    target: edge.target === "asset" ? "units" : edge.target,
    targetHandle: edge.targetHandle,
    sourceHandle: "out",
  })),
]);


function emitAddEntry() {
  emit("update:menu", { entries: [...props.menu.entries, newMenuEntry(props.menu.entries)] });
}

function emitRemoveEntry(id: string) {
  emit("update:menu", { entries: props.menu.entries.filter((entry) => entry.id !== id) });
}

function emitEntryPatch(id: string, patch: Partial<WorkbenchMenuEntry>) {
  emit("update:menu", {
    entries: props.menu.entries.map((entry) => (entry.id === id ? { ...entry, ...patch } : entry)),
  });
}

/** 切维度时重置分类（两个维度的目标目录不同）。 */
function onEntryDimension(entry: WorkbenchMenuEntry, dimension: string) {
  if (dimension === entry.dimension) return;
  emitEntryPatch(entry.id, { dimension: dimension as MenuDimension, categoryInstance: "" });
}

const dimensionOptions = computed(() => [
  { value: "city", label: t("studio.workbench.dimCity") },
  { value: "development", label: t("studio.workbench.dimDevelopment") },
]);

function entryTargetOptions(dimension: MenuDimension) {
  return [
    { value: "", label: t("studio.assetWorkbench.menuCategoryRequired") },
    ...menuTargetsFor(dimension).map((option) => ({
      value: option.id,
      label: t(option.labelKey),
    })),
  ];
}

async function uploadMenuImage(kind: "icon" | "marquee"): Promise<void> {
  const path = await tauriApi.workspace.pickImageFile(
    t(kind === "icon" ? "studio.assetWorkbench.menuIcon" : "studio.assetWorkbench.menuMarquee"),
  );
  if (typeof path !== "string") return;
  const imported = await command<{ asset: string }>("code_flow", {
    request: { project: props.project, action: "import-menu-image", payload: { path, kind } },
  });
  emit("update:menu", { [kind === "icon" ? "iconPng" : "marqueePng"]: imported.asset });
}
const menuImageData = reactive<Record<string, string>>({});
const menuImagePreview = (relative: string): string => {
  if (!relative) return "";
  return menuImageData[relative] ?? "";
};
watch(
  () => [props.menu.iconPng, props.menu.marqueePng, props.project] as const,
  async () => {
    if (!props.project) return;
    for (const kind of ["icon", "marquee"] as const) {
      const relative = kind === "icon" ? props.menu.iconPng : props.menu.marqueePng;
      if (!relative || menuImageData[relative]) continue;
      try {
        const result = await command<{ data: string | null }>("code_flow", {
          request: {
            project: props.project,
            action: "read-menu-image",
            payload: { kind },
          },
        });
        if (result.data) menuImageData[relative] = result.data;
      } catch (cause) {
        console.warn("[menu] image preview failed:", cause);
      }
    }
  },
  { immediate: true },
);
function onMenuPosition(event: Event) {
  const raw = (event.target as HTMLInputElement).value.trim();
  const position = raw === "" ? null : Number(raw);
  emit("update:menu", { uiPosition: position !== null && Number.isFinite(position) ? position : null });
}
function onMenuCost(
  field: "constructionCost" | "maintenanceCost",
  event: Event,
) {
  const raw = (event.target as HTMLInputElement).value.trim();
  const value = raw === "" ? null : Number(raw);
  emit("update:menu", {
    [field]: value !== null && Number.isFinite(value) ? value : null,
  });
}

// ---- 逻辑编排图：右键添加、连线、删除、参数回写（纯函数在 logicGraph.ts）----

const issuesByNode = computed(() => {
  const map = new Map<string, string[]>();
  for (const issue of logicIssues(props.logic)) {
    if (!issue.nodeId) continue;
    const list = map.get(issue.nodeId) ?? [];
    list.push(issue.code);
    map.set(issue.nodeId, list);
  }
  return map;
});
function issuesFor(id: string): string[] {
  return issuesByNode.value.get(id) ?? [];
}

/** 右键菜单（pane = 画布空白；node = 具体节点）。 */
interface CanvasMenu {
  x: number;
  y: number;
  flow: { x: number; y: number };
  nodeId?: string;
}
const canvasMenu = ref<CanvasMenu | null>(null);

function onPaneContextMenu(event: MouseEvent): void {
  if (event.defaultPrevented) return;
  event.preventDefault();
  canvasMenu.value = {
    x: event.clientX,
    y: event.clientY,
    flow: screenToFlowCoordinate({ x: event.clientX, y: event.clientY }),
  };
}
/** 鼠标/触摸事件统一取坐标（VueFlow 的 NodeMouseEvent 是联合类型）。 */
function eventPoint(event: MouseEvent | TouchEvent): { x: number; y: number } {
  if ("clientX" in event) return { x: event.clientX, y: event.clientY };
  const touch = event.touches[0] ?? event.changedTouches[0];
  return { x: touch?.clientX ?? 0, y: touch?.clientY ?? 0 };
}
function onNodeContextMenu(payload: NodeMouseEvent): void {
  if (!props.logic.nodes.some((node) => node.id === payload.node.id)) return;
  payload.event.preventDefault();
  const point = eventPoint(payload.event);
  canvasMenu.value = {
    x: point.x,
    y: point.y,
    flow: screenToFlowCoordinate(point),
    nodeId: payload.node.id,
  };
}
function closeCanvasMenu(): void {
  canvasMenu.value = null;
}

/** 新增节点：info/core 自动挂到资产锚点；leaf 自动补聚合节点并连线。 */
function addNode(spec: LogicNodeSpec, position: [number, number]): void {
  let graph = props.logic;
  if (spec.kind === "behavior-leaf") {
    const core = ensureCoreNode(graph, [position[0] - 360, position[1]]);
    graph = core.graph;
    const added = addLogicNode(graph, spec, position);
    graph = added.graph;
    graph = connectLogic(graph, added.node.id, core.node.id, "leaf").graph;
    if (
      !graph.edges.some(
        (edge) => edge.source === core.node.id && edge.target === "asset",
      )
    ) {
      graph = connectLogic(graph, core.node.id, "asset", "behavior").graph;
    }
  } else {
    const added = addLogicNode(graph, spec, position);
    graph = added.graph;
    graph = connectLogic(
      graph,
      added.node.id,
      "asset",
      spec.kind === "building-info" ? "info" : "behavior",
    ).graph;
  }
  emit("update:logic", graph);
}
function addNodeFromMenu(spec: LogicNodeSpec): void {
  const menu = canvasMenu.value;
  if (!menu) return;
  addNode(spec, [menu.flow.x, menu.flow.y]);
  closeCanvasMenu();
}

/** 聚合节点上的「添加行为」：叶子建在右侧并自动连线。 */
function onAddLeaf(coreId: string, type: BehaviorLeafType): void {
  const core = props.logic.nodes.find((node) => node.id === coreId);
  const spec = leafSpec(type);
  const siblings = props.logic.edges.filter(
    (edge) => edge.target === coreId,
  ).length;
  const position: [number, number] = core
    ? [core.position[0] + 390, core.position[1] + siblings * 220]
    : [0, 0];
  let graph = addLogicNode(props.logic, spec, position).graph;
  const added = graph.nodes[graph.nodes.length - 1];
  graph = connectLogic(graph, added.id, coreId, "leaf").graph;
  emit("update:logic", graph);
}

function onRemoveNode(id: string): void {
  const result = removeLogicNode(props.logic, id);
  if (result.blocked) {
    toast.warning(
      t(`studio.assetWorkbench.logicIssue.${result.blocked}`),
    );
    return;
  }
  emit("update:logic", result.graph);
}
function onDuplicateNode(id: string): void {
  const result = duplicateLogicNode(props.logic, id);
  if (result.node) emit("update:logic", result.graph);
}
function onUpdateParams(id: string, patch: Record<string, unknown>): void {
  emit("update:logic", updateLogicParams(props.logic, id, patch));
}
function onNodeDragStop(event: NodeDragEvent): void {
  const node = props.logic.nodes.find((item) => item.id === event.node.id);
  if (!node) return;
  emit("update:logic", {
    ...props.logic,
    nodes: props.logic.nodes.map((item) =>
      item.id === node.id
        ? {
            ...item,
            position: [event.node.position.x, event.node.position.y],
          }
        : item,
    ),
  });
}

/** 画布连线：只接受逻辑节点发起的合法连线，其余静默拒绝并提示。 */
function onConnect(connection: Connection): void {
  const source = connection.source;
  if (!source || !props.logic.nodes.some((node) => node.id === source)) return;
  const target =
    connection.target === "units" ? "asset" : connection.target ?? "";
  const handle = connection.targetHandle;
  if (handle !== "info" && handle !== "behavior" && handle !== "leaf") return;
  const result = connectLogic(props.logic, source, target, handle);
  if (result.error) {
    toast.warning(t(`studio.assetWorkbench.logicIssue.${result.error}`));
    return;
  }
  emit("update:logic", result.graph);
}

/** 双击删除只对逻辑边生效；固定管线边不可删。 */
function onEdgeDoubleClick(payload: { edge: Edge }): void {
  if (!props.logic.edges.some((edge) => edge.id === payload.edge.id)) return;
  emit("update:logic", {
    ...props.logic,
    edges: props.logic.edges.filter((edge) => edge.id !== payload.edge.id),
  });
}

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
      :nodes-connectable="true"
      :nodes-draggable="true"
      :edges-updatable="false"
      :delete-key-code="null"
      fit-view-on-init
      :max-zoom="1.4"
      :min-zoom="0.3"
      class="blueprint-flow"
      @connect="onConnect"
      @node-drag-stop="onNodeDragStop"
      @edge-double-click="onEdgeDoubleClick"
      @pane-context-menu="onPaneContextMenu"
      @node-context-menu="onNodeContextMenu"
    >
      <Background :gap="20" pattern-color="#333333" />
      <template #node-wb="node">
        <div class="wb-node" :data-role="node.id" :data-tone="nodeTone(node.data)">
          <Handle type="target" :position="Position.Left" />
          <Handle type="source" :position="Position.Right" />
          <template v-if="node.id === 'units'">
            <Handle
              id="info"
              type="target"
              :position="Position.Left"
              :style="{ top: '78px', background: '#c9a227' }"
            />
            <Handle
              id="behavior"
              type="target"
              :position="Position.Left"
              :style="{ top: '104px', background: '#58a05e' }"
            />
            <span class="wb-port-hint" :style="{ top: '70px' }">info</span>
            <span class="wb-port-hint" :style="{ top: '96px' }">behavior</span>
          </template>
          <header class="wb-node-head">
            <FIcon :name="node.data.icon" :size="13" aria-label="" />
            <strong>{{ node.data.title }}</strong>
          </header>
          <div v-for="line in node.data.lines" :key="line.label" class="wb-line">
            <span>{{ line.label }}</span>
            <em :data-tone="line.tone ?? 'muted'">{{ line.value }}</em>
          </div>
          <div v-if="node.data.slot === 'menu'" class="wb-menu">
              <input
                :value="menu.label"
                type="text"
                class="wb-menu-input"
                :placeholder="t('studio.assetWorkbench.menuLabelPlaceholder')"
                :aria-label="t('studio.assetWorkbench.menuLabel')"
                @input="emit('update:menu', { label: ($event.target as HTMLInputElement).value })"
              />
              <input
                :value="menu.description"
                type="text"
                class="wb-menu-input"
                :placeholder="t('studio.assetWorkbench.menuDescriptionPlaceholder')"
                :aria-label="t('studio.assetWorkbench.menuDescription')"
                @input="emit('update:menu', { description: ($event.target as HTMLInputElement).value })"
              />
              <input
                :value="menu.uiPosition ?? ''"
                type="number"
                class="wb-menu-input"
                :placeholder="t('studio.assetWorkbench.menuPositionPlaceholder')"
                :aria-label="t('studio.assetWorkbench.menuPosition')"
                @input="onMenuPosition"
              />
              <div class="wb-menu-images">
                <button
                  type="button"
                  class="wb-menu-image nodrag"
                  :class="{ filled: menu.iconPng }"
                  :title="t('studio.assetWorkbench.menuIconUpload')"
                  @click="uploadMenuImage('icon')"
                >
                  <img
                    v-if="menuImagePreview(menu.iconPng)"
                    :src="menuImagePreview(menu.iconPng)"
                    :alt="t('studio.assetWorkbench.menuIconPreview')"
                  />
                  <span v-else>{{ t("studio.assetWorkbench.menuIconUpload") }}</span>
                </button>
                <button
                  type="button"
                  class="wb-menu-image marquee nodrag"
                  :class="{ filled: menu.marqueePng }"
                  :title="t('studio.assetWorkbench.menuMarqueeUpload')"
                  @click="uploadMenuImage('marquee')"
                >
                  <img
                    v-if="menuImagePreview(menu.marqueePng)"
                    :src="menuImagePreview(menu.marqueePng)"
                    :alt="t('studio.assetWorkbench.menuMarqueePreview')"
                  />
                  <span v-else>{{ t("studio.assetWorkbench.menuMarqueeUpload") }}</span>
                </button>
              </div>
              <div class="wb-menu-costs">
                <input
                  :value="menu.constructionCost ?? ''"
                  type="number"
                  class="wb-menu-input"
                  :placeholder="t('studio.assetWorkbench.menuCostPlaceholder')"
                  :aria-label="t('studio.assetWorkbench.menuCost')"
                  @input="onMenuCost('constructionCost', $event)"
                />
                <input
                  :value="menu.maintenanceCost ?? ''"
                  type="number"
                  class="wb-menu-input"
                  :placeholder="t('studio.assetWorkbench.menuUpkeepPlaceholder')"
                  :aria-label="t('studio.assetWorkbench.menuUpkeep')"
                  @input="onMenuCost('maintenanceCost', $event)"
                />
              </div>
              <button type="button" class="wb-menu-add" @click="emitAddEntry">
                <FIcon name="Plus" :size="11" aria-label="" />
                {{ t("studio.assetWorkbench.menuAddEntry") }}
              </button>
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
      <template #node-logic="node">
        <LogicNodeCard
          :node="node.data.node"
          :params="node.data.params"
          :issues="issuesFor(node.id)"
          :disabled="false"
          @update-params="onUpdateParams(node.id, $event)"
          @remove="onRemoveNode(node.id)"
          @duplicate="onDuplicateNode(node.id)"
          @add-leaf="onAddLeaf(node.id, $event)"
        />
      </template>
      <template #node-menu-entry="node">
        <div class="wb-entry">
          <Handle type="target" :position="Position.Left" />
          <header class="wb-entry-head">
            <FIcon name="Menu" :size="12" aria-label="" />
            <strong>{{ t("studio.assetWorkbench.menuEntryTitle") }}</strong>
            <button
              type="button"
              class="wb-entry-remove"
              :aria-label="t('studio.assetWorkbench.menuRemoveEntry')"
              @click="emitRemoveEntry(node.data.entry.id)"
            >
              ×
            </button>
          </header>
          <label class="wb-entry-field">
            <span>{{ t("studio.assetWorkbench.menuDimension") }}</span>
            <FDropdownSelect
              :model-value="node.data.entry.dimension"
              :options="dimensionOptions"
              @update:model-value="onEntryDimension(node.data.entry, $event)"
            />
          </label>
          <label class="wb-entry-field">
            <span>{{ t("studio.assetWorkbench.menuTargetCategory") }}</span>
            <FDropdownSelect
              :model-value="node.data.entry.categoryInstance"
              :options="entryTargetOptions(node.data.entry.dimension)"
              :invalid="!node.data.entry.categoryInstance"
              @update:model-value="emitEntryPatch(node.data.entry.id, { categoryInstance: $event })"
            />
          </label>
        </div>
      </template>
    </VueFlow>
    <div v-if="canvasMenu" class="ctx-backdrop" @pointerdown.self="closeCanvasMenu">
      <div
        class="ctx-menu"
        :style="{ left: `${canvasMenu.x}px`, top: `${canvasMenu.y}px` }"
        role="menu"
      >
        <template v-if="canvasMenu.nodeId">
          <button
            type="button"
            role="menuitem"
            @click="
              onDuplicateNode(canvasMenu.nodeId!);
              closeCanvasMenu();
            "
          >
            {{ t("studio.assetWorkbench.logicDuplicate") }}
          </button>
          <button
            type="button"
            role="menuitem"
            class="danger"
            @click="
              onRemoveNode(canvasMenu.nodeId!);
              closeCanvasMenu();
            "
          >
            {{ t("studio.assetWorkbench.logicRemove") }}
          </button>
        </template>
        <template v-else>
          <p class="ctx-title">{{ t("studio.assetWorkbench.logicAddNode") }}</p>
          <button
            v-for="spec in LOGIC_NODE_SPECS"
            :key="`${spec.kind}:${spec.leafType ?? ''}`"
            type="button"
            role="menuitem"
            :class="{ 'ctx-group-start': spec.kind === 'behavior-leaf' && spec.leafType === 'education' }"
            @click="addNodeFromMenu(spec)"
          >
            <span>{{ t(spec.labelKey) }}</span>
          </button>
        </template>
      </div>
    </div>
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
.wb-menu-add {
  align-items: center;
  background: var(--flow-field, #1a1a1a);
  border: 1px solid var(--flow-field-border, #3a3a3a);
  border-radius: 4px;
  color: var(--flow-text, #e6e6e6);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 10.5px;
  gap: 4px;
  justify-content: center;
  min-height: 25px;
  padding: 0 7px;
}
.wb-menu-add:hover {
  background: #333333;
  border-color: #6a6a6a;
}
/* 菜单编辑节点：每个目标菜单容器一个，只决定「放在哪儿」。 */
.wb-entry {
  background: var(--flow-node, #282828);
  border: 1px solid var(--flow-border, #141414);
  border-radius: 8px;
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.4);
  color: var(--flow-text, #e6e6e6);
  display: grid;
  gap: 6px;
  font-size: 10.5px;
  min-width: 200px;
  padding: 8px 11px;
}
.wb-entry-head {
  align-items: center;
  background: var(--flow-head-menu, #6b5b2e);
  border-radius: 7px 7px 0 0;
  color: #f2f2f2;
  display: flex;
  gap: 6px;
  margin: -8px -11px 1px;
  padding: 6px 8px 6px 10px;
}
.wb-entry-head strong {
  flex: 1;
  font-size: 11.5px;
}
.wb-entry-remove {
  background: transparent;
  border: 0;
  border-radius: 4px;
  color: rgba(255, 255, 255, 0.65);
  cursor: pointer;
  font-size: 13px;
  line-height: 1;
  min-height: 20px;
  padding: 0 5px;
}
.wb-entry-remove:hover {
  background: rgba(255, 255, 255, 0.14);
  color: #ffffff;
}
.wb-entry-field {
  align-items: center;
  display: grid;
  grid-template-columns: 52px minmax(0, 1fr);
  gap: 8px;
}
.wb-entry-field > span {
  color: var(--flow-text-dim, #9b9b9b);
}
/* FSelect 落在 Blender 暗色节点内：覆盖其浅色默认（下拉面板保持应用主题）。 */
.wb-entry :deep(.f-select) {
  background: var(--flow-field, #1a1a1a);
  border-color: var(--flow-field-border, #3a3a3a);
  border-radius: 4px;
  color: var(--flow-text, #e6e6e6);
  font-size: 10.5px;
  gap: 4px;
  min-height: 24px;
  padding: 0 5px;
}
.wb-entry :deep(.f-select:focus) {
  border-color: #6a6a6a;
  box-shadow: none;
}
.wb-entry :deep(.f-select-label.placeholder) {
  color: var(--flow-text-dim, #9b9b9b);
}
.wb-port-hint {
  color: var(--flow-text-dim, #9b9b9b);
  font-family: var(--font-mono, monospace);
  font-size: 8px;
  left: -38px;
  position: absolute;
  width: 34px;
}
.wb-menu-costs {
  display: grid;
  gap: 4px;
  grid-template-columns: 1fr 1fr;
}
.ctx-backdrop {
  inset: 0;
  position: fixed;
  z-index: 80;
}
.ctx-menu {
  background: var(--flow-node, #282828);
  border: 1px solid var(--flow-border, #141414);
  border-radius: 6px;
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.5);
  color: var(--flow-text, #e6e6e6);
  display: flex;
  flex-direction: column;
  min-width: 190px;
  padding: 4px;
  position: fixed;
}
.ctx-title {
  color: var(--flow-text-dim, #9b9b9b);
  font-size: 10px;
  margin: 2px 6px 4px;
}
.ctx-menu button {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 4px;
  color: inherit;
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 11px;
  gap: 6px;
  padding: 5px 7px;
  text-align: left;
}
.ctx-menu button:hover {
  background: rgba(255, 255, 255, 0.1);
}
.ctx-menu button.danger:hover {
  background: rgba(211, 60, 60, 0.35);
}
.ctx-menu button.ctx-group-start {
  border-top: 1px solid rgba(255, 255, 255, 0.12);
  margin-top: 3px;
  padding-top: 7px;
}
</style>
