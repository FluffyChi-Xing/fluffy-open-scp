<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import MonacoEditor from "@/components/ui/MonacoEditor.vue";
import { tauriApi } from "@/api";
import { useToast } from "@/composables/useToast";
import MenuBindingForm from "./MenuBindingForm.vue";
import ShiftScheduleSlider from "./ShiftScheduleSlider.vue";
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
  type LogicGraph,
  type LogicNode,
} from "./logicGraph";
import type { WorkbenchMenuBinding } from "./menuBinding";

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

type TabId = "basic" | "menu" | "building" | "behavior" | "dependencies" | "advanced";
const TABS: { id: TabId; icon: string; labelKey: string }[] = [
  { id: "basic", icon: "FileText", labelKey: "studio.assetWorkbench.projectConfig.tabBasic" },
  { id: "menu", icon: "Menu", labelKey: "studio.assetWorkbench.projectConfig.tabMenu" },
  { id: "building", icon: "Users", labelKey: "studio.assetWorkbench.projectConfig.tabBuilding" },
  { id: "behavior", icon: "Gauge", labelKey: "studio.assetWorkbench.projectConfig.tabBehavior" },
  { id: "dependencies", icon: "Network", labelKey: "studio.assetWorkbench.projectConfig.tabDependencies" },
  { id: "advanced", icon: "Braces", labelKey: "studio.assetWorkbench.projectConfig.tabAdvanced" },
];
const activeTab = ref<TabId>("basic");

// ---- package.json 读写 ----
const manifest = ref<Record<string, unknown> | null>(null);
const baseline = ref("");
const revision = ref("");
const loading = ref(false);
const loadError = ref("");
const saving = ref(false);
const saveError = ref("");

async function readManifest(): Promise<void> {
  const doc = await tauriApi.workspace.codeReadText(props.project, "package.json");
  const parsed = JSON.parse(doc.content) as Record<string, unknown>;
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
  () => manifestDirty.value || props.savedDirty || advancedDirty.value,
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
function ensureCore(): LogicNode | null {
  const core = ensureCoreNode(props.logic, [0, 120]);
  let graph = core.graph;
  if (
    !graph.edges.some(
      (edge) => edge.source === core.node.id && edge.target === "asset",
    )
  ) {
    graph = connectLogic(graph, core.node.id, "asset", "behavior").graph;
  }
  emit("update:logic", graph);
  return core.node;
}
function addCore(): void {
  if (!coreNode.value) ensureCore();
}
function addLeaf(type: BehaviorLeafType): void {
  const core = coreNode.value ?? ensureCore();
  if (!core) return;
  const added = addLogicNode(props.logic, leafSpec(type), [0, 260]);
  let graph = added.graph;
  graph = connectLogic(graph, added.node.id, core.id, "leaf").graph;
  emit("update:logic", graph);
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
  switch (activeTab.value) {
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
      <nav class="ppf-nav" role="tablist">
        <button
          v-for="tab in TABS"
          :key="tab.id"
          type="button"
          role="tab"
          :aria-selected="activeTab === tab.id"
          :class="{ active: activeTab === tab.id }"
          @click="activeTab = tab.id"
        >
          <FIcon :name="tab.icon" :size="13" aria-label="" />
          {{ t(tab.labelKey) }}
        </button>
      </nav>

      <div class="ppf-content">
        <p class="ppf-hint">{{ tabHint }}</p>

        <!-- 基础配置 -->
        <div v-if="activeTab === 'basic'" class="ppf-fields">
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
        <MenuBindingForm
          v-else-if="activeTab === 'menu'"
          :menu="menu"
          :project="project"
          :disabled="saving"
          @update:menu="emit('update:menu', $event)"
        />

        <!-- 建筑属性 -->
        <div v-else-if="activeTab === 'building'" class="ppf-fields">
          <template v-if="infoParams && infoNode">
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
                  type="button"
                  :disabled="saving"
                  :title="t('studio.assetWorkbench.logicRemove')"
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
        <div v-else class="ppf-advanced">
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
.ppf-nav {
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 168px;
  padding: 10px 8px;
  overflow-y: auto;
}
.ppf-nav button {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font: inherit;
  font-size: 12px;
  gap: 7px;
  min-height: 32px;
  padding: 0 9px;
  text-align: left;
}
.ppf-nav button:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.ppf-nav button.active {
  background: color-mix(in srgb, var(--primary) 14%, transparent);
  color: var(--primary);
  font-weight: 600;
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
