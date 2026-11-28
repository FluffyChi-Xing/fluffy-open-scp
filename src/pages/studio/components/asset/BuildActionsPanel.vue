<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import { tauriApi } from "@/api";
import { subscribe } from "@/api/tauri";
import type {
  BuildJobDto,
  BuildQueueEventPayload,
  StageEventDto,
} from "@/api/tauri";
import { useBuildActivityStore } from "@/stores/buildActivity";
import { buildQueueErrorKey } from "@/lib/buildQueueError";
import { buildStageLabel } from "@/lib/buildStageLabel";
import PanelRuntimeNotice from './PanelRuntimeNotice.vue';
import { unsupportedPanelItems } from './panel-editor/panelBuildDiagnostic';
import { describeBehaviorConflict } from './behaviorBuildDiagnostics';

/**
 * OpenSCP Actions：构建历史 + GitHub Actions 风格步骤耗时/详情日志。
 * 生产入队入口在场景编辑器「构建」；本面板也可一键构建当前项目。
 */
const props = defineProps<{
  project: string;
  road?: boolean;
  /** 场景编辑器刚入队的任务；打开面板时自动展开详情。 */
  focusJobId?: string | null;
}>();
const { t, te } = useI18n();
const buildActivity = useBuildActivityStore();

interface StepView {
  stage: string;
  status: "running" | "finished" | "failed";
  durationMs: number | null;
  detail: unknown;
}

const jobs = ref<BuildJobDto[]>([]);
const selectedId = ref<string | null>(null);
const expandedStage = ref<string | null>(null);
const loading = ref(false);
let refreshRequest = 0;
const projectBuildBusy = ref(false);
const projectBuildError = ref("");
const projectSchema = ref("");
const projectLogic = ref<{
  menu: boolean;
  labor: boolean;
  behaviors: number;
  warnings: string[];
} | null>(null);

const selectedJob = computed(
  () => jobs.value.find((job) => job.id === selectedId.value) ?? null,
);

const selectedSteps = computed(() =>
  selectedJob.value ? stepsFromEvents(selectedJob.value.events) : [],
);

function stageLabel(stage: string) {
  return buildStageLabel(stage, t);
}

function stepsFromEvents(events: StageEventDto[]): StepView[] {
  const order: string[] = [];
  const map = new Map<string, StepView>();
  for (const event of events) {
    if (!map.has(event.stage)) {
      order.push(event.stage);
      map.set(event.stage, {
        stage: event.stage,
        status: "running",
        durationMs: null,
        detail: event.detail,
      });
    }
    const step = map.get(event.stage)!;
    if (event.kind.type === "started") {
      step.status = "running";
      step.detail = event.detail;
    } else if (event.kind.type === "finished") {
      step.status = "finished";
      step.durationMs = event.durationMs;
      step.detail = event.detail;
    } else {
      step.status = "failed";
      step.durationMs = event.durationMs;
      step.detail = event.detail;
    }
  }
  return order.map((stage) => map.get(stage)!);
}

function formatDetail(detail: unknown): string {
  if (detail == null) return "";
  if (typeof detail === "string") return detail;
  try {
    return JSON.stringify(detail, null, 2);
  } catch {
    return String(detail);
  }
}

async function refresh() {
  const request = ++refreshRequest;
  const project = props.project;
  if (!project) {
    jobs.value = [];
    loading.value = false;
    return;
  }
  loading.value = true;
  try {
    const nextJobs = await tauriApi.buildQueue.list(project);
    if (request !== refreshRequest) return;
    jobs.value = nextJobs;
    const focus = props.focusJobId;
    if (focus && jobs.value.some((job) => job.id === focus)) {
      selectedId.value = focus;
    } else if (
      selectedId.value &&
      !jobs.value.some((job) => job.id === selectedId.value)
    ) {
      selectedId.value = null;
      expandedStage.value = null;
    }
  } finally {
    if (request === refreshRequest) loading.value = false;
  }
}

watch(
  () => props.project,
  () => {
    selectedId.value = props.focusJobId ?? null;
    expandedStage.value = null;
    void refresh();
  },
  { immediate: true },
);

watch(
  () => props.focusJobId,
  (jobId) => {
    if (jobId) {
      selectedId.value = jobId;
      void refresh();
    }
  },
);

const unlistenPromise = subscribe<BuildQueueEventPayload>(
  "build-queue:event",
  (payload) => {
    if (payload.event && selectedId.value === payload.jobId) {
      const job = jobs.value.find((item) => item.id === payload.jobId);
      if (job) {
        job.events.push(payload.event);
        if (job.status === "queued") job.status = "running";
      }
      return;
    }
    void refresh();
  },
).catch(() => null);
onUnmounted(() => {
  void unlistenPromise.then((off) => off?.());
});

const statusIcon = (status: BuildJobDto["status"]) =>
  status === "finished"
    ? "Check"
    : status === "failed"
      ? "X"
      : status === "running"
        ? "LoaderCircle"
        : "Clock";
const statusLabel = (status: BuildJobDto["status"]) =>
  t(`studio.assetWorkbench.actionsStatus.${status}`);

function duration(job: BuildJobDto): string {
  const elapsed =
    job.durationMs ??
    Math.max(0, Date.now() - (job.startedAtMs ?? job.submittedAtMs));
  return formatDuration(elapsed);
}

function formatDuration(milliseconds: number | null): string {
  if (milliseconds === null) return "—";
  if (milliseconds < 1000) return `${milliseconds}ms`;
  if (milliseconds < 60_000) return `${(milliseconds / 1000).toFixed(2)}s`;
  const totalSeconds = Math.floor(milliseconds / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}m ${seconds}s`;
}

function select(job: BuildJobDto) {
  if (selectedId.value === job.id) {
    selectedId.value = null;
    expandedStage.value = null;
    return;
  }
  selectedId.value = job.id;
  expandedStage.value = null;
}

function toggleStep(stage: string) {
  expandedStage.value = expandedStage.value === stage ? null : stage;
}

function buildError(message: string): string {
  if (/\broad_frontage_donor_missing\b/.test(message)) return t("roadAsset.frontage.donorMissing");
  if (/\broad_unit_cost_invalid\b/.test(message)) return t("roadAsset.project.unitCostInvalid");
  if (/\broad_unit_cost_resource_missing\b/.test(message)) return t("roadAsset.project.unitCostResourceMissing");
  if (/\broad_draft_end_cap_missing\b/.test(message)) return t("roadAsset.project.endCapMissing");
  const roadDiagnostic = message.match(/\broad_draft_(layout_unsupported|empty|base_invalid)\b/);
  if (roadDiagnostic?.[1] === "layout_unsupported") return t("roadAsset.draft.unsupported");
  if (roadDiagnostic?.[1] === "empty") return t("roadAsset.draft.emptyBuild");
  if (roadDiagnostic?.[1] === "base_invalid") return t("roadAsset.draft.chooseBase");
  const pair = message.match(/\bBEHAVIOR_LEAF_CONFLICT:\s*([\w-]+)\/([\w-]+)/);
  if (pair) return describeBehaviorConflict(pair[1], pair[2], t);
  const behaviorDiagnostic = message.match(/\b(HOTEL_(?:ROOMS|WEALTH|ROAD_ACCESS|RESOURCE|BUNDLE|FIELD)|FUSION_(?:COMPOSITION|PARAMETER|EP1|MENU_REQUIRED|RESOURCE|BUNDLE|FIELD)|CONTROLNET_(?:COMPOSITION|PARAMETER|EP1|MENU_REQUIRED|RESOURCE|BUNDLE|FIELD)|BEHAVIOR_LEAF_(?:LIMIT|CONFLICT)|BEHAVIOR_COPAN_(?:BASE|SERVICES|RESIDENCE))\b/);
  if (behaviorDiagnostic) return t(`studio.assetWorkbench.behaviorSheet.buildError.${behaviorDiagnostic[1]}`);
  const missingTowerLayout = message.match(/\bPANEL_TOWER_LAYOUT_MISSING:([\w-]+):([\w:]+)/);
  if (missingTowerLayout) {
    const [, id, resource] = missingTowerLayout;
    const key = `panelEditor.components.${id}`;
    return t('panelEditor.towerLayoutMissing', { name: te(key) ? t(key) : id, resource });
  }
  if (/\bPANEL_TOWER_OPERATIONS_REQUIRE_COMPONENTS\b/.test(message)) return t('panelEditor.towerOperationsRequireComponents');
  const runtimeDiagnostic = message.match(/\b(PANEL_RUNTIME_(?:PATCH_IDENTITY|COMPOSITION_CONFLICT|PATCH_DEPENDENCY|CONTRACT|CAPABILITY|SOURCE))\b/);
  if (runtimeDiagnostic) return t(`panelEditor.runtimeErrors.${runtimeDiagnostic[1]}`);
  const containerPanelDiagnostic = message.match(/\b(PANEL_CONTAINER_QUERY_UNSUPPORTED|PANEL_CONTAINER_BINDING|PANEL_CONTAINER_POWER_REQUIRED)\b/);
  const unsupportedItems = unsupportedPanelItems(message);
  if (unsupportedItems.length) {
    const items = unsupportedItems.map(({ view, id }) => {
      const key = `panelEditor.components.${id}`;
      const name = te(key) ? t(key) : id;
      return view ? t('panelEditor.controlLocation', { view: t(`panelEditor.${view}`), name }) : name;
    });
    return t('panelEditor.unsupportedControls', { items: items.join(t('panelEditor.controlSeparator')) });
  }
  if (containerPanelDiagnostic) return t(`studio.assetWorkbench.behaviorSheet.buildError.${containerPanelDiagnostic[1]}`);
  const queueError = buildQueueErrorKey(message);
  if (queueError) return t(queueError);
  const vehicleDiagnostic = message.match(/\bVEHICLE_PATH_(RECEIVER|BINDING)\b/);
  if (vehicleDiagnostic) {
    return t(`studio.assetWorkbench.behaviorSheet.buildError.VEHICLE_PATH_${vehicleDiagnostic[1]}`);
  }
  const decalDiagnostic = message.match(/\bDECAL_(CATEGORY|FIELD|READBACK|SOURCE)\b(?:: ([^\r\n]+))?/);
  if (decalDiagnostic) {
    const [, code, detail] = decalDiagnostic;
    const label = t(`studio.assetWorkbench.decalBuildError.${code}`);
    return detail ? `${label} (${detail})` : label;
  }
  const templateDiagnostic = message.match(/\b(BEHAVIOR_TEMPLATE_(?:PROFILE|PARAMETER|RESOURCE|BUNDLE|FIELD)|RESIDENCE_(?:PATH|FILL_BUNDLE|FILL_INTAKE|GROUND_(?:PROFILE|CAPACITY|VACANT|BUNDLE|ALTERNATE))|OFFICE_(?:PARAMETER|CONFLICT|RESOURCE|FIELD|BUNDLE|LABOR)|COMMERCIAL_(?:PARAMETER|CONFLICT|RESOURCE|FIELD|BUNDLE|ROAD_ACCESS))\b(?:: ([^\r\n]+))?/);
  if (templateDiagnostic) {
    const [, code, detail] = templateDiagnostic;
    const label = t(`studio.assetWorkbench.behaviorSheet.buildError.${code}`);
    return detail ? `${label} (${detail})` : label;
  }
  const diagnostic = message.match(/\bTOURISM_(PROFILE|PARAMETER|QUOTA|MENU|RESOURCE|BUNDLE|ROAD_ACCESS)\b(?:: ([^\r\n]+))?/);
  if (!diagnostic) return message;
  const [, code, detail] = diagnostic;
  const label = t(`studio.assetWorkbench.behaviorSheet.buildError.${code}`);
  return detail ? `${label} (${detail})` : label;
}

const stepIcon = (status: StepView["status"]) =>
  status === "finished"
    ? "Check"
    : status === "failed"
      ? "X"
      : "LoaderCircle";

async function enqueueProject() {
  if (projectBuildBusy.value || !props.project) return;
  projectBuildBusy.value = true;
  projectBuildError.value = "";
  try {
    const submitted = await tauriApi.buildQueue.enqueueProject(props.project);
    projectSchema.value = submitted.schema;
    projectLogic.value = submitted.logic ?? null;
    selectedId.value = submitted.jobId;
    buildActivity.rememberProject(
      props.project,
      buildActivity.lastProjectLabel,
      submitted.jobId,
    );
    await refresh();
  } catch (cause) {
    const message =
      cause &&
      typeof cause === "object" &&
      "message" in cause &&
      typeof (cause as { message: unknown }).message === "string"
        ? (cause as { message: string }).message
        : String(cause);
    projectBuildError.value = message;
    await refresh();
    const failed = jobs.value.filter(job => job.status === "failed" && job.error === message).at(-1);
    if (failed) {
      selectedId.value = failed.id;
      expandedStage.value = "project-preflight";
    }
  } finally {
    projectBuildBusy.value = false;
  }
}

</script>

<template>
  <div class="actions-panel">
    <section class="hero">
      <div class="hero-header">
        <div class="hero-copy">
          <span class="hero-kicker">{{
            t("studio.assetWorkbench.actionsProductionKicker")
          }}</span>
          <p class="hero-note">
            {{
              projectSchema
                ? t("studio.assetWorkbench.actionsBuildProjectSchema", {
                    path: projectSchema,
                  })
                : t(road ? "roadAsset.actionsHint" : "studio.assetWorkbench.actionsBuildProjectHint")
            }}
          </p>
        </div>
        <button
          type="button"
          class="build-button"
          :disabled="projectBuildBusy"
          @click="enqueueProject"
        >
          <span class="build-icon" aria-hidden="true">
            <FIcon
              :name="projectBuildBusy ? 'LoaderCircle' : 'Hammer'"
              :size="16"
              aria-label=""
            />
          </span>
          <span class="build-text">
            <strong>{{
              projectBuildBusy
                ? t("studio.assetWorkbench.buildEnqueueing")
                : t("studio.assetWorkbench.actionsBuildProject")
            }}</strong>
            <small>{{
              t(road ? "roadAsset.compileStage" : "studio.assetWorkbench.actionsBuildProjectCaption")
            }}</small>
          </span>
        </button>
      </div>
      <div v-if="projectLogic || projectBuildError" class="hero-details">
        <p v-if="projectLogic" class="hero-logic">
          {{
            t("studio.assetWorkbench.actionsBuildLogic", {
              menu: projectLogic.menu ? "✓" : "—",
              labor: projectLogic.labor ? "✓" : "—",
              behaviors: projectLogic.behaviors,
            })
          }}
        </p>
        <p
          v-for="warning in projectLogic?.warnings ?? []"
          :key="warning"
          class="hero-warning"
        >
          {{ warning }}
        </p>
        <p v-if="projectBuildError" class="enqueue-error">
          {{ buildError(projectBuildError) }}
        </p>
        <details v-if="projectBuildError && buildError(projectBuildError) !== projectBuildError" class="diagnostic">
          <summary>{{ t("assetCapture.rawBuildError") }}</summary>
          <pre class="diagnostic-body">{{ projectBuildError }}</pre>
        </details>
      </div>
    </section>

    <div class="runs-block">
      <header class="runs-head">
        <strong>{{ t("studio.assetWorkbench.actionsTab") }}</strong>
        <span class="runs-count">{{ jobs.length }}</span>
      </header>

      <p v-if="loading && !jobs.length" class="panel-note">
        {{ t("common.loading") }}
      </p>
      <div v-else-if="!jobs.length" class="empty-state">
        <span class="empty-icon" aria-hidden="true">
          <FIcon name="ClipboardList" :size="18" aria-label="" />
        </span>
        <p>{{ t("studio.assetWorkbench.actionsEmpty") }}</p>
      </div>

      <ul v-else class="run-list">
        <li v-for="job in jobs" :key="job.id" class="run-item">
          <button
            type="button"
            class="run-row"
            :class="[`status-${job.status}`, { selected: selectedId === job.id }]"
            @click="select(job)"
          >
            <span class="status-dot" :data-status="job.status" aria-hidden="true">
              <FIcon :name="statusIcon(job.status)" :size="12" aria-label="" />
            </span>
            <span class="run-main">
              <span class="run-id">{{ job.label || job.id }}</span>
              <span class="run-meta">
                <span class="status-pill" :data-status="job.status">{{
                  statusLabel(job.status)
                }}</span>
                <span>{{ new Date(job.submittedAtMs).toLocaleString() }}</span>
              </span>
            </span>
            <span class="run-duration">{{ duration(job) }}</span>
          </button>

          <div v-if="selectedId === job.id" class="job-detail">
            <header class="job-detail-head">
              <strong>{{ t("studio.assetWorkbench.actionsViewDetail") }}</strong>
              <span v-if="job.durationMs != null">{{
                formatDuration(job.durationMs)
              }}</span>
            </header>
            <ol class="stage-list">
              <li
                v-for="step in selectedSteps"
                :key="step.stage"
                class="stage-item"
              >
                <button
                  type="button"
                  class="stage-row"
                  @click="toggleStep(step.stage)"
                >
                  <FIcon
                    :name="stepIcon(step.status)"
                    :size="12"
                    aria-label=""
                  />
                  <span class="stage-name">{{ stageLabel(step.stage) }}</span>
                  <span class="stage-duration">{{
                    formatDuration(step.durationMs)
                  }}</span>
                </button>
                <pre
                  v-if="expandedStage === step.stage && formatDetail(step.detail)"
                  class="stage-log"
                  >{{ formatDetail(step.detail) }}</pre
                >
              </li>
            </ol>
            <p v-if="job.error" class="stage-error">{{ buildError(job.error) }}</p>
            <p v-else-if="job.result" class="stage-result">
              {{ t("studio.assetWorkbench.actionsResources") }}:
              {{ job.result.resourceCount }} · sha256
              {{ String(job.result.sha256 ?? "").slice(0, 12) }}…
              <template v-if="String(job.result.scope ?? '').startsWith('map_resources')">
                <br />{{ t("mapResources.output") }} <code class="break-all">{{ job.result.path }}</code>
              </template>
            </p>
            <PanelRuntimeNotice v-if="job.result" :legacy-path="job.result.panelRuntime" :required="job.result.requiresPanelRuntime === true" />
          </div>
        </li>
      </ul>
    </div>

  </div>
</template>

<style scoped>
.actions-panel {
  display: flex;
  flex-direction: column;
  gap: 14px;
  overflow: auto;
  padding: 4px 2px 8px;
}
.hero {
  background:
    radial-gradient(
      120% 80% at 100% 0%,
      color-mix(in srgb, var(--primary, #2563eb) 18%, transparent),
      transparent 55%
    ),
    linear-gradient(
      160deg,
      color-mix(in srgb, var(--surface-elevated) 92%, #1e293b),
      color-mix(in srgb, var(--surface) 88%, transparent)
    );
  border: 1px solid color-mix(in srgb, var(--primary, #2563eb) 22%, var(--border));
  border-radius: 12px;
  display: grid;
  gap: 14px;
  padding: 14px 16px;
}
.hero-header {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 10px 14px;
}
.hero-copy {
  flex: 1 1 14rem;
  min-inline-size: 0;
}
.hero-details {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 8px;
  display: grid;
  gap: 4px;
  min-inline-size: 0;
  padding: 8px 10px;
}
.hero-kicker {
  color: color-mix(in srgb, var(--primary, #60a5fa) 85%, white);
  display: inline-block;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.08em;
  margin-bottom: 6px;
  text-transform: uppercase;
}
.hero-note,
.hero-logic,
.hero-warning {
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.5;
  margin: 0;
  overflow-wrap: anywhere;
}
.hero-warning {
  color: #fbbf24;
}
.build-button {
  align-items: center;
  background: linear-gradient(
    180deg,
    color-mix(in srgb, var(--primary, #3b82f6) 92%, white),
    var(--primary, #2563eb)
  );
  border: 1px solid color-mix(in srgb, var(--primary, #2563eb) 70%, #0f172a);
  border-radius: 10px;
  box-shadow:
    0 1px 0 color-mix(in srgb, white 18%, transparent) inset,
    0 8px 20px color-mix(in srgb, var(--primary, #2563eb) 28%, transparent);
  color: #fff;
  cursor: pointer;
  display: inline-flex;
  gap: 10px;
  min-height: 48px;
  margin-inline-start: auto;
  max-inline-size: 100%;
  min-inline-size: min(168px, 100%);
  padding: 8px 14px;
  text-align: start;
}
.build-button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
.build-button:not(:disabled):hover {
  filter: brightness(1.05);
}
.build-icon {
  align-items: center;
  background: color-mix(in srgb, white 16%, transparent);
  border-radius: 8px;
  display: inline-flex;
  flex-shrink: 0;
  height: 30px;
  justify-content: center;
  width: 30px;
}
.build-text {
  display: flex;
  min-inline-size: 0;
  overflow-wrap: anywhere;
  flex-direction: column;
  gap: 1px;
  line-height: 1.2;
}
.build-text strong {
  font-size: 13px;
  font-weight: 600;
}
.build-text small {
  color: color-mix(in srgb, white 72%, transparent);
  font-size: 10px;
}
.runs-block {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.runs-head {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 11px;
  gap: 8px;
  justify-content: space-between;
  letter-spacing: 0.02em;
}
.runs-count {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: 999px;
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  min-width: 22px;
  padding: 1px 7px;
  text-align: center;
}
.empty-state {
  align-items: center;
  background: color-mix(in srgb, var(--surface-elevated) 55%, transparent);
  border: 1px dashed var(--border);
  border-radius: 10px;
  color: var(--subtle-foreground);
  display: flex;
  flex-direction: column;
  gap: 8px;
  justify-content: center;
  min-height: 96px;
  padding: 18px 16px;
  text-align: center;
}
.empty-icon {
  align-items: center;
  background: color-mix(in srgb, var(--surface) 70%, transparent);
  border: 1px solid var(--border);
  border-radius: 999px;
  display: inline-flex;
  height: 34px;
  justify-content: center;
  width: 34px;
}
.empty-state p,
.panel-note {
  color: var(--subtle-foreground);
  font-size: 12px;
  line-height: 1.45;
  margin: 0;
}
.run-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.run-item {
  display: flex;
  flex-direction: column;
}
.run-row {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: 10px;
  color: var(--foreground);
  cursor: pointer;
  display: grid;
  font: inherit;
  gap: 10px;
  grid-template-columns: 28px 1fr auto;
  min-height: 44px;
  padding: 8px 12px;
  scale: none;
  text-align: start;
  transition:
    border-color 0.15s ease,
    background 0.15s ease;
  width: 100%;
}
.run-row:hover {
  border-color: color-mix(in srgb, var(--primary, #2563eb) 35%, var(--border));
}
.run-row.selected {
  background: color-mix(in srgb, var(--primary, #2563eb) 8%, var(--surface-elevated));
  border-color: color-mix(in srgb, var(--primary, #2563eb) 55%, var(--border));
  border-bottom-left-radius: 0;
  border-bottom-right-radius: 0;
}
.status-dot {
  align-items: center;
  border-radius: 999px;
  display: inline-flex;
  height: 26px;
  justify-content: center;
  width: 26px;
}
.status-dot[data-status="finished"] {
  background: color-mix(in srgb, #22c55e 18%, transparent);
  color: #4ade80;
}
.status-dot[data-status="failed"] {
  background: color-mix(in srgb, #ef4444 18%, transparent);
  color: #f87171;
}
.status-dot[data-status="running"] {
  background: color-mix(in srgb, #3b82f6 18%, transparent);
  color: #60a5fa;
}
.status-dot[data-status="queued"] {
  background: color-mix(in srgb, #94a3b8 16%, transparent);
  color: #94a3b8;
}
.run-main {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}
.run-id {
  font-size: 12px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.run-meta {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  flex-wrap: wrap;
  font-size: 11px;
  gap: 6px;
}
.status-pill {
  border-radius: 999px;
  font-size: 10px;
  padding: 1px 7px;
}
.status-pill[data-status="finished"] {
  background: color-mix(in srgb, #22c55e 16%, transparent);
  color: #86efac;
}
.status-pill[data-status="failed"] {
  background: color-mix(in srgb, #ef4444 16%, transparent);
  color: #fca5a5;
}
.status-pill[data-status="running"] {
  background: color-mix(in srgb, #3b82f6 16%, transparent);
  color: #93c5fd;
}
.status-pill[data-status="queued"] {
  background: color-mix(in srgb, #94a3b8 14%, transparent);
  color: #cbd5e1;
}
.run-duration {
  color: var(--muted-foreground);
  font-family: var(--font-mono, monospace);
  font-size: 11px;
}
.job-detail {
  background: color-mix(in srgb, var(--surface-elevated) 65%, #0b1220);
  border: 1px solid color-mix(in srgb, var(--primary, #2563eb) 35%, var(--border));
  border-radius: 0 0 10px 10px;
  border-top: 0;
  margin: 0;
  padding: 10px 12px 12px;
}
.job-detail-head {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 11px;
  justify-content: space-between;
  margin-bottom: 6px;
}
.stage-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.stage-row {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: 6px;
  color: var(--foreground);
  cursor: pointer;
  display: grid;
  font: inherit;
  font-size: 11px;
  gap: 8px;
  grid-template-columns: 14px 1fr auto;
  min-height: 26px;
  padding: 4px 6px;
  text-align: start;
  width: 100%;
}
.stage-row:hover {
  background: var(--surface-hover);
}
.stage-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.stage-duration {
  color: var(--muted-foreground);
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  min-width: 48px;
  text-align: end;
}
.stage-log {
  background: #0b1220;
  border-radius: 8px;
  color: #d7e0ea;
  font-family: var(--font-mono, monospace);
  font-size: 10.5px;
  line-height: 1.45;
  margin: 0 0 4px 22px;
  max-height: 220px;
  overflow: auto;
  padding: 8px 10px;
  white-space: pre-wrap;
  word-break: break-word;
}
.stage-error {
  color: var(--danger, #ef4444);
  font-size: 11px;
  margin: 8px 0 0;
  word-break: break-all;
}
.stage-result {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: 8px 0 0;
}
.diagnostic {
  border-top: 1px solid var(--border);
  padding-top: 4px;
}
.diagnostic-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 0 0 4px;
}
.enqueue-error {
  color: var(--danger, #ef4444);
  font-size: 11px;
  margin: 0;
  word-break: break-all;
}
</style>
