<script setup lang="ts">
import { computed, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import { tauriApi } from "@/api";
import { subscribe } from "@/api/tauri";
import type {
  BuildJobDto,
  BuildQueueEventPayload,
  StageEventDto,
} from "@/api/tauri";

/**
 * OpenSCP Actions 页(P2,2026-11-04):构建队列的运行列表 + 单次运行
 * 的阶段详情(对标 GitHub Actions 列表/job log 两级视图)。任务由
 * 后台 build-queue 单线程串行执行;阶段边界事件经 build-queue:event
 * 实时推送,本面板订阅后刷新列表/详情。入队入口 = 手动粘贴 schema
 * JSON(P3 将由场景编辑器的"打包"按钮自动入队)。
 */
const props = defineProps<{ project: string }>();
const { t } = useI18n();

const jobs = ref<BuildJobDto[]>([]);
const selectedId = ref<string | null>(null);
const loading = ref(false);
const schemaText = ref("");
const enqueueBusy = ref(false);
const enqueueError = ref("");

const selected = computed(
  () => jobs.value.find((job) => job.id === selectedId.value) ?? null,
);

async function refresh() {
  loading.value = true;
  try {
    jobs.value = await tauriApi.buildQueue.list(props.project);
    if (selectedId.value && !jobs.value.some((job) => job.id === selectedId.value)) {
      selectedId.value = null;
    }
  } finally {
    loading.value = false;
  }
}
void refresh();

const unlisten = subscribe<BuildQueueEventPayload>(
  "build-queue:event",
  (payload) => {
    // 运行中任务的阶段事件 → 就地更新详情行;状态类事件 → 全量刷新。
    if (payload.event && selectedId.value === payload.jobId) {
      const job = jobs.value.find((item) => item.id === payload.jobId);
      job?.events.push(payload.event);
      return;
    }
    void refresh();
  },
);
onUnmounted(() => void unlisten.then((off) => off()));

const statusIcon = (status: BuildJobDto["status"]) =>
  status === "finished"
    ? "Check"
    : status === "failed"
      ? "X"
      : status === "running"
        ? "Loader"
        : "Clock";
const statusLabel = (status: BuildJobDto["status"]) =>
  t(`studio.assetWorkbench.actionsStatus.${status}`);

function duration(job: BuildJobDto): string {
  const elapsed = job.durationMs ?? Math.max(0, Date.now() - (job.startedAtMs ?? job.submittedAtMs));
  return formatDuration(elapsed);
}

function formatDuration(milliseconds: number | null): string {
  if (milliseconds === null) return "—";
  if (milliseconds < 1000) return String(milliseconds) + "ms";
  return (milliseconds / 1000).toFixed(2) + "s";
}

function eventTime(event: StageEventDto): string {
  return new Date(event.timestampMs).toLocaleTimeString();
}

function select(job: BuildJobDto) {
  selectedId.value = selectedId.value === job.id ? null : job.id;
}

async function enqueueSchema() {
  enqueueError.value = "";
  const schema = schemaText.value.trim();
  if (!schema) return;
  try {
    JSON.parse(schema);
  } catch (cause) {
    enqueueError.value = String(cause);
    return;
  }
  enqueueBusy.value = true;
  try {
    await tauriApi.buildQueue.enqueue(props.project, schema);
    schemaText.value = "";
    await refresh();
  } catch (cause) {
    enqueueError.value = String(cause);
  } finally {
    enqueueBusy.value = false;
  }
}

const stageKindIcon = (event: StageEventDto) =>
  event.kind.type === "finished"
    ? "Check"
    : event.kind.type === "failed"
      ? "X"
      : "Loader";
</script>

<template>
  <div class="actions-panel">
    <!-- 手动入队(P3 前的过渡入口:粘贴 schema JSON) -->
    <details class="enqueue-box">
      <summary>{{ t("studio.assetWorkbench.actionsEnqueueTitle") }}</summary>
      <textarea
        v-model="schemaText"
        class="schema-input"
        :placeholder="t('studio.assetWorkbench.actionsEnqueueHint')"
        spellcheck="false"
      />
      <button
        type="button"
        class="enqueue-button"
        :disabled="enqueueBusy || !schemaText.trim()"
        @click="enqueueSchema"
      >
        {{ t("studio.assetWorkbench.actionsEnqueueButton") }}
      </button>
      <p v-if="enqueueError" class="enqueue-error">{{ enqueueError }}</p>
    </details>

    <p v-if="loading && !jobs.length" class="panel-note">
      {{ t("common.loading") }}
    </p>
    <p v-else-if="!jobs.length" class="panel-note">
      {{ t("studio.assetWorkbench.actionsEmpty") }}
    </p>
    <ul v-else class="run-list">
      <li v-for="job in jobs" :key="job.id">
        <button
          type="button"
          class="run-row"
          :class="{ selected: selectedId === job.id }"
          @click="select(job)"
        >
          <FIcon
            :name="statusIcon(job.status)"
            :size="14"
            aria-label=""
          />
          <span class="run-id">{{ job.label || job.id }}</span>
          <span class="run-status">{{ statusLabel(job.status) }}</span>
          <span class="run-duration">{{ duration(job) }}</span>
        </button>
        <!-- 运行详情:阶段步骤行(对标 Actions job log) -->
        <ol v-if="selectedId === job.id" class="stage-list">
          <li v-for="(event, index) in job.events" :key="index" class="stage-row">
            <FIcon :name="stageKindIcon(event)" :size="12" aria-label="" />
            <span class="stage-name">{{ event.stage }}</span>
            <span class="stage-kind">{{ event.kind.type }}</span>
            <span class="stage-time">{{ eventTime(event) }}</span>
            <span v-if="event.durationMs !== null" class="stage-duration">
              {{ formatDuration(event.durationMs) }}
            </span>
          </li>
          <li v-if="job.error" class="stage-error">{{ job.error }}</li>
          <li v-else-if="job.result" class="stage-result">
            {{ t("studio.assetWorkbench.actionsResources") }}:
            {{ job.result.resourceCount }} · sha256
            {{ String(job.result.sha256 ?? "").slice(0, 12) }}…
          </li>
        </ol>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.actions-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow: auto;
  padding: 10px;
}
.enqueue-box {
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 8px;
}
.enqueue-box summary {
  color: var(--muted-foreground);
  cursor: pointer;
  font-size: 11px;
}
.schema-input {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  height: 96px;
  margin-top: 6px;
  padding: 6px;
  width: 100%;
}
.enqueue-button {
  background: var(--primary, #2563eb);
  border: none;
  border-radius: var(--radius-sm);
  color: #fff;
  cursor: pointer;
  font-size: 12px;
  margin-top: 6px;
  min-height: 26px;
  width: 100%;
}
.enqueue-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
.enqueue-error {
  color: var(--danger, #ef4444);
  font-size: 11px;
  margin: 4px 0 0;
  word-break: break-all;
}
.panel-note {
  color: var(--subtle-foreground);
  font-size: 12px;
  margin: 8px 0;
}
.run-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.run-row {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: grid;
  font: inherit;
  font-size: 12px;
  gap: 8px;
  grid-template-columns: 16px 1fr auto auto;
  min-height: 28px;
  padding: 4px 8px;
  text-align: start;
  width: 100%;
}
.run-row.selected {
  border-color: var(--primary, #2563eb);
}
.run-id {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.run-status,
.run-duration {
  color: var(--muted-foreground);
  font-size: 11px;
}
.stage-list {
  list-style: none;
  margin: 4px 0 0;
  padding: 0 0 0 16px;
}
.stage-row {
  align-items: center;
  display: flex;
  font-size: 11px;
  gap: 6px;
  min-height: 22px;
}
.stage-name {
  color: var(--foreground);
}
.stage-kind {
  color: var(--muted-foreground);
}
.stage-time,
.stage-duration {
  color: var(--muted-foreground);
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  margin-left: auto;
}
.stage-duration {
  margin-left: 4px;
  min-width: 42px;
  text-align: end;
}
.stage-error {
  color: var(--danger, #ef4444);
  font-size: 11px;
  word-break: break-all;
}
.stage-result {
  color: var(--subtle-foreground);
  font-size: 11px;
}
</style>
