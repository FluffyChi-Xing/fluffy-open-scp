<script setup lang="ts">
import ProjectConfigTabs from "./ProjectConfigTabs.vue";
import ProjectConfigOverview from "./ProjectConfigOverview.vue";
import { computed, defineAsyncComponent, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import MonacoEditor from "@/components/ui/MonacoEditor.vue";
import { tauriApi } from "@/api";
import { useToast } from "@/composables/useToast";
import MenuBindingForm from "./MenuBindingForm.vue";
import ShiftScheduleSlider from "./ShiftScheduleSlider.vue";
import BehaviorLeafSheet from "./BehaviorLeafSheet.vue";
import { useBehaviorConflicts } from "./useBehaviorConflicts";
import {
  LOGIC_NODE_SPECS,
  addLogicNode,
  connectLogic,
  ensureCoreNode,
  leafSpec,
  removeLogicNode,
  updateLogicParams,
  type BehaviorLeafType,
  type BuildingInfoParams,
  type LeafParams,
  type LogicGraph,
  type LogicNode,
} from "./logicGraph";
import type { WorkbenchMenuBinding } from "./menuBinding";
import VanillaVariantMenuForm from "./VanillaVariantMenuForm.vue";
import VanillaPowerProductionForm from "./VanillaPowerProductionForm.vue";
const PanelProjectEditor = defineAsyncComponent(() => import("./panel-editor/PanelProjectEditor.vue"));

/**
 * package.json 可视化配置（HBuilderX manifest.json 式分类表单）。
 *
 * 点击左栏 package.json 时占据中栏。数据分两处：
 * - 项目清单字段写 package.json；
 * - 菜单/建筑属性/行为来自工作台文档 editor.workbench.menu|logic（保存时同步）。
 * 表单在每页明确标注数据位置，避免"改了但不知道写哪儿"。
 */
const props = defineProps<{
  project: string;
  menu: WorkbenchMenuBinding;
  logic: LogicGraph;
  /** 工作台文档是否有未保存修改。 */
  savedDirty: boolean;
  /** 保存工作台文档（内部会同步 package.json dependencies）。 */
  saveDocument: () => Promise<boolean>;
  /** 文档相对路径（数据位置提示用）。 */
  documentPath: string;
}>();
const emit = defineEmits<{
  "update:menu": [patch: Partial<WorkbenchMenuBinding>];
  "update:logic": [graph: LogicGraph];
  close: [];
}>();

const { t } = useI18n();
const toast = useToast();
const behaviorConflicts = useBehaviorConflicts();

type TabId = "overview" | "basic" | "menu" | "panel" | "building" | "behavior" | "dependencies" | "advanced";
const TABS: { id: TabId; icon: string; labelKey: string }[] = [
  { id: "overview", icon: "ListChecks", labelKey: "studio.assetWorkbench.projectConfig.tabOverview" },
  { id: "basic", icon: "FileText", labelKey: "studio.assetWorkbench.projectConfig.tabBasic" },
  { id: "menu", icon: "Menu", labelKey: "studio.assetWorkbench.projectConfig.tabMenu" },
  { id: "panel", icon: "PanelsTopLeft", labelKey: "panelEditor.title" },
  { id: "building", icon: "Users", labelKey: "studio.assetWorkbench.projectConfig.tabBuilding" },
  { id: "behavior", icon: "Gauge", labelKey: "studio.assetWorkbench.projectConfig.tabBehavior" },
  { id: "dependencies", icon: "Network", labelKey: "studio.assetWorkbench.projectConfig.tabDependencies" },
  { id: "advanced", icon: "Braces", labelKey: "studio.assetWorkbench.projectConfig.tabAdvanced" },
];
const activeTab = ref<TabId>("overview");
const panelEditor = ref<InstanceType<typeof PanelProjectEditor>>();
const panelVisited = ref(false);
const panelDirty = ref(false);
watch(activeTab, value => { if (value === "panel") panelVisited.value = true; });
const isVanillaVariant = computed(() => manifest.value?.mod_type === "vanilla_variant");
const tabs = computed(() => isVanillaVariant.value ? TABS.filter((tab) => tab.id !== "behavior" && tab.id !== "panel") : TABS);

// ---- package.json 读写 ----
const manifest = ref<Record<string, unknown> | null>(null);
const baseline = ref("");
const revision = ref("");
const loading = ref(false);
const loadError = ref("");
const saving = ref(false);
const saveError = ref("");
const utilityLoadFailed = ref(false);
const powerLoadFailed = ref(false);

async function readManifest(): Promise<void> {
  const project = props.project;
  const doc = await tauriApi.workspace.codeReadText(project, "package.json");
  const parsed = JSON.parse(doc.content) as Record<string, unknown>;
  const source = parsed.vanillaVariant as VariantConfig | undefined;
  utilityLoadFailed.value = false;
  if (parsed.mod_type === "vanilla_variant" && source?.unit && !source.utilityFieldsVersion) {
    try {
      const available = await tauriApi.workspace.vanillaVariantUtilityFields(project);
      source.fields = { ...available, ...source.fields };
      source.utilityFieldsVersion = 1;
    } catch {
      if (project === props.project) utilityLoadFailed.value = true;
    }
  }
  powerLoadFailed.value = false;
  if (parsed.mod_type === "vanilla_variant" && source?.unit && source.powerProduction === undefined) {
    try {
      source.powerProduction = await tauriApi.workspace.vanillaVariantPowerFields(project);
    } catch {
      if (project === props.project) powerLoadFailed.value = true;
    }
  }
  if (project !== props.project) return;
  manifest.value = parsed;
  baseline.value = JSON.stringify(parsed);
  revision.value = doc.revision;
  advancedText.value = JSON.stringify(parsed, null, 2);
  advancedDirty.value = false;
}

async function loadManifest(): Promise<void> {
  loading.value = true;
  loadError.value = "";
  try {
    await readManifest();
  } catch (cause) {
    // 缺失清单由 workspace 扫描自动生成（与 CodeWorkbench 同机制），再读一次。
    try {
      await tauriApi.workspace.codeManifest(props.project);
      await readManifest();
    } catch {
      manifest.value = null;
      loadError.value = String(cause);
    }
  } finally {
    loading.value = false;
  }
}
watch(() => props.project, () => void loadManifest(), { immediate: true });

const manifestDirty = computed(
  () => !!manifest.value && JSON.stringify(manifest.value) !== baseline.value,
);
const dirty = computed(
    () => manifestDirty.value || props.savedDirty || advancedDirty.value || panelDirty.value,
);

function stringField(key: string): string {
  return typeof manifest.value?.[key] === "string"
    ? (manifest.value[key] as string)
    : "";
}
function setStringField(key: string, value: string): void {
  if (!manifest.value) return;
  manifest.value = { ...manifest.value, [key]: value };
}

type VariantConfig = {
  title: string;
  description?: string;
  iconPng?: string;
  marqueePng?: string;
  unit: string;
  lot: string;
  utilityFieldsVersion?: number;
  powerProduction?: Awaited<ReturnType<typeof tauriApi.workspace.vanillaVariantPowerFields>>;
  fields: Record<string, { hash: string; original: number; value: number; consumerBound?: boolean }>;
};
const variant = computed(() => manifest.value?.vanillaVariant as VariantConfig | undefined);
function patchPowerField(group: string, field: string, value: number) {
  const entry = variant.value?.powerProduction?.[group]?.[field];
  if (entry) entry.value = value;
}
const utilityGroups = {
  power: ["powerAmount", "powerCapacity", "powerInterval"],
  water: ["waterAmount", "waterCapacity", "waterInterval"],
};
const utilityFields = new Set(Object.values(utilityGroups).flat());
const variantFieldGroups = computed(() => {
  const groups = {
    costs: ["constructionCost", "maintenanceCost"],
    ...utilityGroups,
    labor: ["minimumWorkers", "jobsW1", "jobsW2", "jobsW3", "workersW1", "workersW2", "workersW3"],
    production: ["productionInterval", "productionAmount"],
    education: ["studentCapacity"],
  };
  const fields = Object.entries(variant.value?.fields ?? {});
  const assigned = new Set(Object.values(groups).flat());
  return [
    ...Object.entries(groups).map(([id, keys]) => ({ id, fields: fields.filter(([key]) => keys.includes(key)) })),
    { id: "other", fields: fields.filter(([key]) => !assigned.has(key)) },
  ].filter((group) => group.fields.length > 0);
});
function patchVariantField(field: string, event: Event) {
  const input = event.target as HTMLInputElement;
  const value = input.valueAsNumber;
  const entry = variant.value?.fields[field];
  if (!entry) return;
  if (Number.isFinite(value) && value >= 0 && (!utilityFields.has(field) || input.validity.valid)) {
    entry.value = value;
  } else {
    input.value = String(entry.value);
    if (utilityFields.has(field)) toast.error(t("flow.variant.utilityIntegerRequired"));
  }
}

// ---- 高级 JSON ----
const advancedText = ref("");
const advancedDirty = ref(false);
function onAdvancedChange(value: string): void {
  advancedText.value = value;
  advancedDirty.value = true;
}
function resetAdvanced(): void {
  if (!manifest.value) return;
  advancedText.value = JSON.stringify(manifest.value, null, 2);
  advancedDirty.value = false;
}
// 高级页与基础页不同时编辑：进入高级页总是以当前清单重置文本，
// 避免"改完基础字段后保存被旧的高级文本覆盖"。
watch(activeTab, (tab) => {
  if (tab === "advanced") resetAdvanced();
});

// ---- 建筑属性 ----
const infoNode = computed<LogicNode | null>(
  () => props.logic.nodes.find((node) => node.kind === "building-info") ?? null,
);
const infoParams = computed<BuildingInfoParams | null>(() =>
  infoNode.value
    ? (props.logic.params[infoNode.value.id] as BuildingInfoParams)
    : null,
);
function addInfoNode(): void {
  if (infoNode.value) return;
  const spec = LOGIC_NODE_SPECS.find((item) => item.kind === "building-info");
  if (!spec) return;
  const added = addLogicNode(props.logic, spec, [0, 0]);
  emit(
    "update:logic",
    connectLogic(added.graph, added.node.id, "asset", "info").graph,
  );
}
function patchInfo(key: keyof BuildingInfoParams, event: Event): void {
  if (!infoNode.value) return;
  const raw = (event.target as HTMLInputElement).value.trim();
  const value = raw === "" ? 0 : Number(raw);
  if (!Number.isFinite(value)) return;
  emit(
    "update:logic",
    updateLogicParams(props.logic, infoNode.value.id, { [key]: value }),
  );
}
/** 排班拖动条 → 八班位规模值（0=关班），写入 building-info 节点参数。 */
function patchSchedule(schedule: number[]): void {
  if (!infoNode.value) return;
  emit(
    "update:logic",
    updateLogicParams(props.logic, infoNode.value.id, { schedule }),
  );
}

// ---- 行为 ----
const coreNode = computed<LogicNode | null>(
  () => props.logic.nodes.find((node) => node.kind === "behavior-core") ?? null,
);
const leafNodes = computed(() =>
  props.logic.nodes
    .filter((node) => node.kind === "behavior-leaf")
    .map((node) => {
      const type = (props.logic.params[node.id] as { type?: BehaviorLeafType })
        ?.type;
      return {
        node,
        type: type ?? null,
        labelKey: type ? leafSpec(type).labelKey : node.id,
      };
    }),
);
const leafCatalog = computed(() =>
  LOGIC_NODE_SPECS.filter((spec) => spec.kind === "behavior-leaf").map(
    (spec) => ({
      type: spec.leafType as BehaviorLeafType,
      label: t(spec.labelKey),
    }),
  ),
);
const editingLeafId = ref<string | null>(null);
const editTrigger = ref<HTMLElement | null>(null);
const editingLeaf = computed(() => {
  const leaf = leafNodes.value.find((item) => item.node.id === editingLeafId.value);
  if (!leaf?.type) return null;
  return { ...leaf, params: props.logic.params[leaf.node.id] as LeafParams };
});
function editLeaf(id: string, event: MouseEvent): void {
  editTrigger.value = event.currentTarget as HTMLElement;
  editingLeafId.value = id;
}
function closeLeafEditor(): void {
  editingLeafId.value = null;
}
function applyLeafSettings(patch: Record<string, unknown>): void {
  if (!editingLeaf.value) return;
  emit("update:logic", updateLogicParams(props.logic, editingLeaf.value.node.id, patch));
  closeLeafEditor();
}
function graphWithCore(): { graph: LogicGraph; node: LogicNode } {
  const core = ensureCoreNode(props.logic, [0, 120]);
  let graph = core.graph;
  if (
    !graph.edges.some(
      (edge) => edge.source === core.node.id && edge.target === "asset",
    )
  ) {
    graph = connectLogic(graph, core.node.id, "asset", "behavior").graph;
  }
  return { graph, node: core.node };
}
function addCore(): void {
  if (!coreNode.value) emit("update:logic", graphWithCore().graph);
}
function addLeaf(type: BehaviorLeafType): void {
  if (!behaviorConflicts.canAdd(props.logic, type)) return;
  const core = graphWithCore();
  const added = addLogicNode(core.graph, leafSpec(type), [0, 260]);
  const graph = connectLogic(added.graph, added.node.id, core.node.id, "leaf").graph;
  emit("update:logic", graph);
  void behaviorConflicts.check(graph, added.node.id);
}
function removeNode(id: string): void {
  const result = removeLogicNode(props.logic, id);
  if (result.blocked) {
    toast.warning(t(`studio.assetWorkbench.logicIssue.${result.blocked}`));
    return;
  }
  emit("update:logic", result.graph);
}

// ---- 依赖 ----
const dependencies = computed<Record<string, unknown>>(() => {
  const value = manifest.value?.dependencies;
  return value && typeof value === "object"
    ? (value as Record<string, unknown>)
    : {};
});
const variantDependencies = computed(() => {
  const report = dependencies.value.vanillaVariant as { resources?: { mode?: string }[] } | undefined;
  const rows = Array.isArray(report?.resources) ? report.resources : [];
  return {
    count: rows.length,
    unresolved: rows.filter((row) => row.mode === "unresolved").length,
  };
});
function dependencyResources(): Record<string, unknown> {
  const value = dependencies.value.resources;
  return value && typeof value === "object"
    ? (value as Record<string, unknown>)
    : {};
}
function pathEntries(): string[] {
  const value = dependencyResources().pathEntries;
  return Array.isArray(value) ? value.map(String) : [];
}
function statics(): { path: string; role: string }[] {
  const value = dependencies.value.statics;
  if (!Array.isArray(value)) return [];
  return value
    .filter(
      (item): item is { path: string; role?: string } =>
        typeof item === "object" && item !== null && "path" in item,
    )
    .map((item) => ({ path: String(item.path), role: String(item.role ?? "") }));
}
function nodeCount(): number {
  const value = manifest.value?.nodes;
  return Array.isArray(value) ? value.length : 0;
}
function dependencyCount(): number {
  const value = manifest.value?.internal_dependencies;
  return value && typeof value === "object"
    ? Object.keys(value).length
    : 0;
}

// ---- 保存 ----
async function save(): Promise<void> {
  if (saving.value) return;
  saving.value = true;
  saveError.value = "";
  try {
    if (advancedDirty.value) {
      const parsed = JSON.parse(advancedText.value) as Record<string, unknown>;
      manifest.value = parsed;
      advancedDirty.value = false;
    }
    for (const [amountName, capacityName, intervalName] of Object.values(utilityGroups)) {
      const fields = variant.value?.fields;
      const amount = fields?.[amountName];
      const capacity = fields?.[capacityName];
      const interval = fields?.[intervalName];
      if (amount && capacity && [amount, capacity, interval].some((field) => field && field.value !== field.original)
        && capacity.value < amount.value) {
        saveError.value = t("flow.variant.utilityCapacityTooSmall", { field: t(`flow.variant.${capacityName}`) });
        return;
      }
    }
    if (manifestDirty.value && manifest.value) {
      const written = await tauriApi.workspace.codeWriteText(
        props.project,
        "package.json",
        JSON.stringify(manifest.value, null, 2),
        revision.value,
      );
      revision.value = written.revision;
      baseline.value = JSON.stringify(manifest.value);
    }
    if (panelDirty.value && panelEditor.value && !(await panelEditor.value.save())) return;
    if (props.savedDirty) {
      await props.saveDocument();
    }
    await loadManifest();
    toast.success(t("studio.assetWorkbench.projectConfig.saved"));
  } catch (cause) {
    saveError.value = String(cause);
    toast.error(
      t("studio.assetWorkbench.projectConfig.saveFailed", {
        error: String(cause),
      }),
    );
  } finally {
    saving.value = false;
  }
}

const tabHint = computed(() => {
  if (isVanillaVariant.value && activeTab.value === "building") return t("flow.variant.propertiesHint");
  if (isVanillaVariant.value && activeTab.value === "menu") return t("flow.variant.parkHint");
  switch (activeTab.value) {
    case "panel":
      return t("panelEditor.hint");
    case "overview":
      return t("studio.assetWorkbench.overviewHint");
    case "menu":
      return t("studio.assetWorkbench.projectConfig.menuHint", {
        path: props.documentPath,
      });
    case "building":
    case "behavior":
      return t("studio.assetWorkbench.projectConfig.logicHint", {
        path: props.documentPath,
      });
    case "dependencies":
      return t("studio.assetWorkbench.projectConfig.dependenciesHint");
    case "advanced":
      return t("studio.assetWorkbench.projectConfig.advancedHint");
    default:
      return t("studio.assetWorkbench.projectConfig.basicHint");
  }
});
</script>

<template>
  <section class="ppf" :aria-label="t('studio.assetWorkbench.projectConfig.title')">
    <header class="ppf-head">
      <div class="ppf-title">
        <FIcon name="FileJson" :size="15" aria-label="" />
        <strong>{{ t("studio.assetWorkbench.projectConfig.title") }}</strong>
        <span v-if="dirty" class="ppf-dirty">
          {{ t("studio.assetWorkbench.projectConfig.unsaved") }}
        </span>
      </div>
      <div class="ppf-actions">
        <button type="button" @click="emit('close')">
          <FIcon name="ArrowLeft" :size="12" aria-label="" />
          {{ t("studio.assetWorkbench.projectConfig.back") }}
        </button>
        <button
          type="button"
          class="primary"
          :disabled="saving || loading || !manifest"
          @click="save"
        >
          <FIcon :name="saving ? 'Loader2' : 'Save'" :size="12" aria-label="" />
          {{ t("studio.assetWorkbench.projectConfig.save") }}
        </button>
      </div>
    </header>

    <p v-if="loading" class="ppf-note">
      {{ t("studio.assetWorkbench.projectConfig.loading") }}
    </p>
    <p v-else-if="loadError" class="ppf-error" role="alert">
      {{ t("studio.assetWorkbench.projectConfig.loadFailed", { error: loadError }) }}
    </p>
    <div v-else-if="manifest" class="ppf-body">
      <ProjectConfigTabs v-model="activeTab" :tabs="tabs" :label="t('studio.assetWorkbench.projectConfig.title')" />

      <div class="ppf-content">
        <p class="ppf-hint">{{ tabHint }}</p>
        <PanelProjectEditor v-if="panelVisited" v-show="activeTab === 'panel'" ref="panelEditor" :project="project" :logic="logic" :title="menu.label || stringField('name')" :active="activeTab === 'panel'" @dirty="panelDirty = $event" />

        <!-- 基础配置 -->
        <ProjectConfigOverview v-if="activeTab === 'overview'" :project="project" :manifest="manifest" :menu="menu" :logic="logic" :variant="isVanillaVariant" :advanced-text="advancedDirty ? advancedText : undefined" />
        <div v-else-if="activeTab === 'basic'" class="ppf-fields">
          <label class="ppf-field">
            <span>{{ t("studio.assetWorkbench.projectConfig.name") }}</span>
            <input
              :value="stringField('name')"
              type="text"
              :disabled="saving"
              @input="setStringField('name', ($event.target as HTMLInputElement).value)"
            />
          </label>
          <label class="ppf-field">
            <span>{{ t("studio.assetWorkbench.projectConfig.version") }}</span>
            <input
              :value="stringField('version')"
              type="text"
              :disabled="saving"
              @input="setStringField('version', ($event.target as HTMLInputElement).value)"
            />
          </label>
          <label class="ppf-field wide">
            <span>{{ t("studio.assetWorkbench.projectConfig.description") }}</span>
            <input
              :value="stringField('description')"
              type="text"
              :disabled="saving"
              @input="
                setStringField('description', ($event.target as HTMLInputElement).value)
              "
            />
          </label>
          <label class="ppf-field">
            <span>{{ t("studio.assetWorkbench.projectConfig.author") }}</span>
            <input
              :value="stringField('author')"
              type="text"
              :disabled="saving"
              @input="setStringField('author', ($event.target as HTMLInputElement).value)"
            />
          </label>
          <div class="ppf-readonly">
            <div>
              <span>{{ t("studio.assetWorkbench.projectConfig.modType") }}</span>
              <code>{{ stringField("mod_type") || "—" }}</code>
            </div>
            <div>
              <span>{{ t("studio.assetWorkbench.projectConfig.workflow") }}</span>
              <code>{{ stringField("workflow") || "—" }}</code>
            </div>
            <div>
              <span>{{ t("studio.assetWorkbench.projectConfig.format") }}</span>
              <code>{{ stringField("format") }} / v{{ manifest?.manifest_version ?? "—" }}</code>
            </div>
          </div>
        </div>

        <!-- 菜单 -->
        <div v-else-if="activeTab === 'menu' && isVanillaVariant">
          <VanillaVariantMenuForm
            v-if="variant" :project="project" :unit="variant.unit" :menu="variant" :disabled="saving"
            @update="Object.assign(variant, $event)" />
          <p v-else class="ppf-empty">{{ t("flow.variant.sceneHint") }}</p>
        </div>
        <MenuBindingForm
          v-else-if="activeTab === 'menu'"
          :menu="menu"
          :project="project"
          :disabled="saving"
          @update:menu="emit('update:menu', $event)"
        />

        <!-- 建筑属性 -->
        <div v-else-if="activeTab === 'building'" class="ppf-fields">
          <template v-if="isVanillaVariant">
            <p v-if="utilityLoadFailed" class="ppf-empty" role="status">{{ t("flow.variant.utilityLoadFailed") }}</p>
            <p v-if="powerLoadFailed" class="ppf-empty" role="status">{{ t("flow.variant.powerLoadFailed") }}</p>
            <VanillaPowerProductionForm
              v-if="variant?.powerProduction"
              :groups="variant.powerProduction"
              :disabled="saving"
              @change="patchPowerField" />
            <details v-for="(group, index) in variantFieldGroups" :key="group.id" class="ppf-field-group" :open="index === 0">
              <summary>{{ t("flow.variant.fieldGroup", { name: t(`flow.variant.groups.${group.id}`), count: group.fields.length }) }}</summary>
              <p v-if="group.fields.some(([, field]) => field.consumerBound === false)" class="ppf-utility-hint">
                {{ t("flow.variant.utilityUnbound") }}
              </p>
              <div class="ppf-fields">
                <label v-for="[name, field] in group.fields" :key="name" class="ppf-field">
                  <span>{{ t(`flow.variant.${name}`, name) }}</span>
                  <input
                    type="number" :min="utilityFields.has(name) && name.endsWith('Interval') ? 1 : 0"
                    :max="utilityFields.has(name) ? 16777216 : undefined" :step="utilityFields.has(name) ? 1 : 'any'"
                    :value="field.value" :disabled="saving" @change="patchVariantField(name, $event)" />
                  <small>{{ t("flow.variant.originalValue", { value: field.original }) }} · <code>{{ field.hash }}</code></small>
                </label>
              </div>
            </details>
            <p v-if="!variantFieldGroups.length" class="ppf-empty">{{ t(variant ? "flow.variant.noEditableFields" : "flow.variant.sceneHint") }}</p>
          </template>
          <template v-else-if="infoParams && infoNode">
            <label
              v-for="field in (['w1', 'w2', 'w3', 'minWorkers'] as const)"
              :key="field"
              class="ppf-field"
            >
              <span>{{ t(`studio.assetWorkbench.labor.${field}`) }}</span>
              <input
                type="number"
                min="0"
                step="1"
                :value="infoParams[field]"
                :disabled="saving"
                @change="patchInfo(field, $event)"
              />
            </label>
            <p class="ppf-hint wide">{{ t("studio.assetWorkbench.labor.hint") }}</p>
          </template>
          <div v-else class="ppf-empty">
            <p>{{ t("studio.assetWorkbench.projectConfig.noBuildingInfo") }}</p>
            <button type="button" :disabled="saving" @click="addInfoNode">
              <FIcon name="Plus" :size="12" aria-label="" />
              {{ t("studio.assetWorkbench.projectConfig.addBuildingInfo") }}
            </button>
          </div>
        </div>

        <!-- 行为 -->
        <div v-else-if="activeTab === 'behavior'" class="ppf-behavior">
          <div v-if="infoParams && infoNode" class="ppf-behavior-block">
            <header>
              <span>{{ t("studio.assetWorkbench.behaviorRecruit") }}</span>
            </header>
            <div class="ppf-fields">
              <label
                v-for="field in (['w1', 'w2', 'w3', 'minWorkers'] as const)"
                :key="field"
                class="ppf-field"
              >
                <span>{{ t(`studio.assetWorkbench.labor.${field}`) }}</span>
                <input
                  type="number"
                  min="0"
                  step="1"
                  :value="infoParams[field]"
                  :disabled="saving"
                  @change="patchInfo(field, $event)"
                />
              </label>
              <div class="ppf-field wide">
                <span>{{ t("studio.assetWorkbench.scheduleTitle") }}</span>
                <ShiftScheduleSlider
                  :schedule="
                    Array.isArray(infoParams.schedule) && infoParams.schedule.length === 8
                      ? infoParams.schedule
                      : [6, 6, 7, 7, 6, 6, 7, 6]
                  "
                  :disabled="saving"
                  @update:schedule="patchSchedule($event)"
                />
              </div>
              <p class="ppf-hint wide">{{ t("studio.assetWorkbench.scheduleHint") }}</p>
            </div>
          </div>
          <div class="ppf-behavior-block">
            <header>
              <span>{{ t("studio.assetWorkbench.logicNode.behaviorCore") }}</span>
              <button
                v-if="!coreNode"
                type="button"
                :disabled="saving"
                @click="addCore"
              >
                {{ t("studio.assetWorkbench.projectConfig.addCore") }}
              </button>
            </header>
            <p v-if="!coreNode" class="ppf-note">
              {{ t("studio.assetWorkbench.projectConfig.noCore") }}
            </p>
            <p v-else class="ppf-note">
              {{ t("studio.assetWorkbench.logicNode.behaviorCoreHint") }}
            </p>
          </div>
          <div class="ppf-behavior-block">
            <header>
              <span>{{ t("studio.assetWorkbench.projectConfig.leaves") }}</span>
            </header>
            <ul class="ppf-leaf-list">
              <li v-for="leaf in leafNodes" :key="leaf.node.id">
                <span>{{ t(leaf.labelKey) }}</span>
                <code>{{ leaf.node.id }}</code>
                <button
                  v-if="leaf.type"
                  type="button"
                  :disabled="saving"
                  :title="t('studio.assetWorkbench.behaviorSheet.edit', { behavior: t(leaf.labelKey) })"
                  :aria-label="t('studio.assetWorkbench.behaviorSheet.edit', { behavior: t(leaf.labelKey) })"
                  aria-haspopup="dialog"
                  @click="editLeaf(leaf.node.id, $event)"
                >
                  <FIcon name="Pencil" :size="14" aria-label="" />
                </button>
                <button
                  type="button"
                  :disabled="saving"
                  :title="t('studio.assetWorkbench.logicRemove')"
                  :aria-label="t('studio.assetWorkbench.logicRemove')"
                  @click="removeNode(leaf.node.id)"
                >
                  <FIcon name="Trash2" :size="12" aria-label="" />
                </button>
              </li>
            </ul>
            <p v-if="!leafNodes.length" class="ppf-note">
              {{ t("studio.assetWorkbench.projectConfig.noLeaves") }}
            </p>
            <div class="ppf-add-leaves">
              <button
                v-for="option in leafCatalog"
                :key="option.type"
                type="button"
                :disabled="saving"
                @click="addLeaf(option.type)"
              >
                {{ option.label }}
              </button>
            </div>
          </div>
        </div>

        <!-- 依赖与资源 -->
        <div v-else-if="activeTab === 'dependencies' && isVanillaVariant" class="ppf-deps">
          <div v-if="variant" class="ppf-deps-grid ppf-variant-deps">
            <div><span>{{ t("flow.variant.unit") }}</span><code>{{ variant.unit }}</code></div>
            <div><span>{{ t("flow.variant.lot") }}</span><code>{{ variant.lot }}</code></div>
            <div><span>{{ t("flow.variant.references") }}</span><strong>{{ variantDependencies.count }}</strong></div>
            <div><span>{{ t("flow.variant.unresolvedReferences") }}</span><strong>{{ variantDependencies.unresolved }}</strong></div>
          </div>
          <p v-else class="ppf-empty">{{ t("flow.variant.sceneHint") }}</p>
        </div>
        <div v-else-if="activeTab === 'dependencies'" class="ppf-deps">
          <div class="ppf-deps-grid">
            <div>
              <span>{{ t("studio.assetWorkbench.projectConfig.nodes") }}</span>
              <strong>{{ nodeCount() }}</strong>
            </div>
            <div>
              <span>{{ t("studio.assetWorkbench.projectConfig.links") }}</span>
              <strong>{{ dependencyCount() }}</strong>
            </div>
            <div>
              <span>{{ t("studio.assetWorkbench.projectConfig.pathEntries") }}</span>
              <strong>{{ pathEntries().length }}</strong>
            </div>
            <div>
              <span>{{ t("studio.assetWorkbench.projectConfig.statics") }}</span>
              <strong>{{ statics().length }}</strong>
            </div>
          </div>
          <ul v-if="pathEntries().length" class="ppf-code-list">
            <li v-for="entry in pathEntries()" :key="entry"><code>{{ entry }}</code></li>
          </ul>
          <ul v-if="statics().length" class="ppf-code-list">
            <li v-for="item in statics()" :key="item.path">
              <code>{{ item.path }}</code>
              <span>{{ item.role }}</span>
            </li>
          </ul>
        </div>

        <!-- 高级 JSON -->
        <div v-else-if="activeTab === 'advanced'" class="ppf-advanced">
          <div class="ppf-advanced-head">
            <span>{{ t("studio.assetWorkbench.projectConfig.rawJson") }}</span>
            <button type="button" :disabled="saving" @click="resetAdvanced">
              {{ t("studio.assetWorkbench.projectConfig.resetJson") }}
            </button>
          </div>
          <div class="ppf-json-host">
            <MonacoEditor
              :value="advancedText"
              language="json"
              :editable="!saving"
              fill
              @update:value="onAdvancedChange"
            />
          </div>
        </div>

        <p v-if="saveError" class="ppf-error" role="alert">
          {{ t("studio.assetWorkbench.projectConfig.saveFailed", { error: saveError }) }}
        </p>
      </div>
    </div>
    <BehaviorLeafSheet
      v-if="editingLeaf"
      :key="editingLeaf.node.id"
      :node="editingLeaf.node"
      :params="editingLeaf.params"
      :label-key="editingLeaf.labelKey"
      :trigger="editTrigger"
      @apply="applyLeafSettings"
      @close="closeLeafEditor"
    />
  </section>
</template>

<style scoped>
.ppf {
  background: var(--surface);
  color: var(--foreground);
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.ppf-head {
  align-items: center;
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 12px;
  justify-content: space-between;
  padding: 10px 14px;
}
.ppf-title {
  align-items: center;
  display: flex;
  gap: 7px;
  font-size: 13px;
}
.ppf-dirty {
  background: color-mix(in srgb, var(--primary) 18%, transparent);
  border-radius: 999px;
  color: var(--primary);
  font-size: 10px;
  padding: 1px 8px;
}
.ppf-actions {
  display: flex;
  gap: 7px;
}
.ppf-actions button,
.ppf-empty button,
.ppf-add-leaves button,
.ppf-behavior-block header button,
.ppf-advanced-head button {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11px;
  gap: 4px;
  min-height: 28px;
  padding: 0 9px;
}
.ppf-actions button.primary {
  background: var(--primary);
  border-color: var(--primary);
  color: var(--primary-foreground);
}
.ppf-actions button:disabled,
.ppf-content button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.ppf-body {
  display: flex;
  flex: 1;
  min-height: 0;
}
.ppf-content {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
  overflow-y: auto;
  padding: 14px 18px;
}
.ppf-hint {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 0;
}
.ppf-fields {
  display: grid;
  gap: 12px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  max-width: 760px;
}
.ppf-field {
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 4px;
  min-width: 0;
}
.ppf-field.wide,
.ppf-hint.wide {
  grid-column: 1 / -1;
}
.ppf-field > span {
  color: var(--muted-foreground);
}
.ppf-field input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 12px;
  min-height: 32px;
  min-width: 0;
  padding: 0 9px;
}
.ppf-readonly {
  border-top: 1px dashed var(--border);
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  grid-column: 1 / -1;
  padding-top: 10px;
}
.ppf-readonly div {
  display: flex;
  flex-direction: column;
  font-size: 10.5px;
  gap: 2px;
}
.ppf-readonly span {
  color: var(--muted-foreground);
}
.ppf-empty {
  align-items: flex-start;
  border: 1px dashed var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  flex-direction: column;
  gap: 8px;
  grid-column: 1 / -1;
  padding: 14px;
}
.ppf-empty p {
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 0;
}
.ppf-behavior {
  display: flex;
  flex-direction: column;
  gap: 14px;
  max-width: 760px;
}
.ppf-behavior-block {
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
}
.ppf-behavior-block header {
  align-items: center;
  display: flex;
  font-size: 12px;
  font-weight: 600;
  justify-content: space-between;
}
.ppf-leaf-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.ppf-leaf-list li {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  font-size: 11.5px;
  gap: 7px;
  padding: 5px 8px;
}
.ppf-leaf-list code {
  color: var(--muted-foreground);
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  margin-left: auto;
}
.ppf-leaf-list button {
  background: none;
  border: 0;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 26px;
  min-height: 26px;
  border-radius: var(--radius-sm);
}
.ppf-leaf-list button:hover,
.ppf-leaf-list button:focus-visible {
  background: var(--surface);
  color: var(--foreground);
}
.ppf-add-leaves {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}
.ppf-deps {
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-width: 760px;
}
.ppf-deps-grid {
  display: grid;
  gap: 10px;
  grid-template-columns: repeat(4, minmax(0, 1fr));
}
.ppf-field-group { grid-column: 1 / -1; min-width: 0; border: 1px solid var(--border); border-radius: var(--radius-md); }
.ppf-field-group summary { padding: 10px 12px; font-weight: 600; cursor: pointer; }
.ppf-field-group summary:hover { background: var(--surface-hover); }
.ppf-field-group > .ppf-fields { padding: 0 12px 12px; }
.ppf-utility-hint { padding: 0 12px 10px; color: var(--muted-foreground); font-size: 12px; }
.ppf-variant-deps { grid-template-columns: repeat(auto-fit, minmax(min(100%, 155px), 1fr)); }
.ppf-variant-deps > div { min-width: 0; }
.ppf-variant-deps code { white-space: normal; overflow-wrap: anywhere; }
.ppf-deps-grid div {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  flex-direction: column;
  font-size: 10.5px;
  gap: 4px;
  padding: 9px;
}
.ppf-deps-grid span {
  color: var(--muted-foreground);
}
.ppf-deps-grid strong {
  font-size: 18px;
}
.ppf-code-list {
  display: flex;
  flex-direction: column;
  font-size: 11px;
  gap: 3px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.ppf-code-list li {
  align-items: center;
  display: flex;
  gap: 8px;
}
.ppf-code-list li span {
  color: var(--muted-foreground);
  font-size: 10px;
}
.ppf-advanced {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 6px;
  min-height: 0;
}
.ppf-advanced-head {
  align-items: center;
  display: flex;
  font-size: 11px;
  justify-content: space-between;
}
.ppf-json-host {
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  display: flex;
  flex: 1;
  min-height: 260px;
  overflow: hidden;
}
.ppf-note {
  color: var(--muted-foreground);
  font-size: 11px;
  margin: 0;
}
.ppf-error {
  color: var(--danger, #e5484d);
  font-size: 11px;
  margin: 0;
}
</style>
