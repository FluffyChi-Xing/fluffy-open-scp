<script setup lang="ts">
/**
 * 渲染遥测页签 —— 纯只读展示。
 *
 * 刻意不接收任何视口 prop、也不触发视口重建：数据来自
 * (a) 记录器的实时环形缓冲（`renderTelemetry.recent()`，模块级普通数组），
 * (b) 后端按 stage 聚合的近 7 天统计。
 * 因此切换到此页签不会引起任何渲染副作用。
 */
import { onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import { isTauri, tauriApi } from "@/api";
import { renderTelemetry } from "@/lib/renderTelemetry";
import type { RenderTelemetryEntry, RenderTelemetrySummary } from "@/api/tauri";

const { t } = useI18n();
const summary = ref<RenderTelemetrySummary | null>(null);
const loading = ref(false);
const error = ref("");
const recent = ref<RenderTelemetryEntry[]>([]);
let timer: ReturnType<typeof setInterval> | null = null;

const STAGE_LABELS: Record<string, string> = {
  model_load: "package.renderStageModelLoad",
  texture_compose: "package.renderStageTextureCompose",
  lot_render: "package.renderStageLotRender",
  decal_render: "package.renderStageDecalRender",
  scene_rebuild: "package.renderStageSceneRebuild",
};

function stageLabel(stage: string): string {
  const key = STAGE_LABELS[stage];
  return key ? t(key) : stage;
}

function formatMs(value: number): string {
  if (!Number.isFinite(value)) return "—";
  return value >= 100 ? `${value.toFixed(0)} ms` : `${value.toFixed(2)} ms`;
}

/**
 * 实时样本的 metadata 徽标：把「规模 ↔ 耗时」相关的关键字段（phase/规模
 * 计数/缓存命中）拼进行内，供肉眼关联——例如 grouping 触发 + units=200
 * + cacheHit=true 应当毫秒级，而 first_load + materials=8 则承载解码成本。
 */
function metadataBadge(entry: RenderTelemetryEntry): string {
  const meta = entry.metadata;
  if (!meta) return entry.trigger;
  const parts: string[] = [entry.trigger];
  for (const key of [
    "phase",
    "units",
    "decals",
    "materials",
    "meshes",
    "bytes",
    "glbs",
    "groups",
    "tinted",
    "projected",
    "fallback",
    "masked",
    "cacheHit",
    "lod",
    "failed",
  ]) {
    if (meta[key] !== undefined) parts.push(`${key}=${String(meta[key])}`);
  }
  return parts.join(" · ");
}

async function loadSummary(): Promise<void> {
  if (!isTauri()) return;
  loading.value = true;
  error.value = "";
  try {
    summary.value = await tauriApi.renderTelemetry.summary(7);
  } catch (cause) {
    error.value = String(cause);
  } finally {
    loading.value = false;
  }
}

function refreshRecent(): void {
  recent.value = [...renderTelemetry.recent()].reverse();
}

onMounted(() => {
  void loadSummary();
  refreshRecent();
  // 缓冲不是响应式的；页签可见期间低频轮询即可（读一个普通数组，无渲染副作用）。
  timer = setInterval(refreshRecent, 1000);
});

onBeforeUnmount(() => {
  if (timer !== null) clearInterval(timer);
});
</script>

<template>
  <div class="render-telemetry">
    <p class="hint">{{ t("package.renderTelemetryHint") }}</p>

    <section class="block">
      <header class="block-head">
        <span>{{ t("package.renderStages") }}</span>
        <button type="button" class="refresh" :disabled="loading" @click="loadSummary">
          {{ t("package.renderRefresh") }}
        </button>
      </header>
      <p v-if="!isTauri()" class="muted">{{ t("package.renderNeedTauri") }}</p>
      <p v-else-if="error" class="error">{{ error }}</p>
      <p v-else-if="loading && !summary" class="muted">{{ t("package.renderLoading") }}</p>
      <p v-else-if="!summary || !summary.stages.length" class="muted">
        {{ t("package.renderEmpty") }}
      </p>
      <ul v-else class="stage-list">
        <li v-for="stage in summary.stages" :key="stage.stage" class="stage-row">
          <span class="stage-name">{{ stageLabel(stage.stage) }}</span>
          <span class="stage-value">{{ formatMs(stage.avgMs) }}</span>
          <span class="stage-meta">
            p95 {{ formatMs(stage.p95Ms) }} · {{ stage.count }}
          </span>
        </li>
      </ul>
    </section>

    <section class="block">
      <header class="block-head">
        <span>{{ t("package.renderRecent") }}</span>
        <span class="chip">{{ recent.length }}</span>
      </header>
      <p v-if="!recent.length" class="muted">{{ t("package.renderEmpty") }}</p>
      <ul v-else class="recent-list">
        <li v-for="(entry, index) in recent" :key="index" class="recent-row">
          <FIcon name="Activity" :size="12" aria-label="" />
          <span class="stage-name">{{ stageLabel(entry.stage) }}</span>
          <span class="stage-value">{{ formatMs(entry.durationMs) }}</span>
          <span class="stage-meta">{{ metadataBadge(entry) }}</span>
        </li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
.render-telemetry {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 12px;
  overflow-y: auto;
}

.hint {
  margin: 0;
  font-size: 11px;
  line-height: 1.5;
  color: var(--text-secondary);
}

.block {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.block-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 11px;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.refresh {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  font-size: 11px;
  padding: 2px 8px;
  cursor: pointer;
}

.refresh:disabled {
  opacity: 0.5;
  cursor: default;
}

.chip {
  border-radius: 999px;
  background: var(--surface-2);
  padding: 1px 7px;
  font-size: 10px;
}

.stage-list,
.recent-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.stage-row,
.recent-row {
  display: grid;
  grid-template-columns: 1fr auto auto;
  align-items: center;
  gap: 8px;
  padding: 4px 6px;
  border-radius: var(--radius-sm);
  background: var(--surface-2);
  font-size: 11px;
}

.recent-row {
  grid-template-columns: auto 1fr auto auto;
}

.stage-name {
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.stage-value {
  font-variant-numeric: tabular-nums;
  color: var(--text-primary);
}

.stage-meta {
  font-size: 10px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.muted {
  margin: 0;
  font-size: 11px;
  color: var(--text-secondary);
}

.error {
  margin: 0;
  font-size: 11px;
  color: var(--danger, #d9534f);
}
</style>
