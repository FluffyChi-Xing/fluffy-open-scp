import { onMounted, shallowRef } from "vue";
import { isTauri, tauriApi } from "@/api";
import type { RenderTelemetrySummary } from "@/api/tauri";

/**
 * 仪表盘渲染耗时卡片的数据源：按 stage 聚合的近 N 天统计。
 * 数据持久化在 `sc-store`，因此**跨会话保留**。
 */
export function useRenderTelemetry(days = 7) {
  const summary = shallowRef<RenderTelemetrySummary | null>(null);
  const loading = shallowRef(false);
  const error = shallowRef("");

  async function load(): Promise<void> {
    if (!isTauri()) {
      summary.value = null;
      return;
    }
    loading.value = true;
    error.value = "";
    try {
      summary.value = await tauriApi.renderTelemetry.summary(days);
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
    } finally {
      loading.value = false;
    }
  }

  async function clear(): Promise<void> {
    if (!isTauri()) return;
    try {
      await tauriApi.renderTelemetry.clear();
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
      return;
    }
    await load();
  }

  onMounted(() => {
    void load();
  });

  return { summary, loading, error, reload: load, clear };
}
