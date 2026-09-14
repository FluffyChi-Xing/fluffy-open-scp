import { shallowRef } from "vue";
import { defineStore } from "pinia";
import { isTauri, tauriApi } from "@/api";
import type {
  ChangesetDetailResponse,
  ChangesetSummary,
  VersionTargetSummary,
} from "@/api/tauri";
import { useToast } from "@/composables/useToast";

/**
 * 版本控制台状态：已跟踪的 overlay 文件、某文件的版本时间线、单条版本详情。
 *
 * 版本线按**目标文件**分区（与后端一致），因此这里始终围绕
 * `targetPath` 组织数据。所有展示数据都来自后端（SQLite），前端不缓存写入。
 */
export const useVersionConsoleStore = defineStore("versionConsole", () => {
  const toast = useToast();

  const targets = shallowRef<VersionTargetSummary[]>([]);
  const activeTarget = shallowRef<string | null>(null);
  const changesets = shallowRef<ChangesetSummary[]>([]);
  const detail = shallowRef<ChangesetDetailResponse | null>(null);
  const loadingTargets = shallowRef(false);
  const loadingTimeline = shallowRef(false);
  const busy = shallowRef(false);
  const error = shallowRef("");

  async function loadTargets(): Promise<void> {
    if (!isTauri()) return;
    loadingTargets.value = true;
    error.value = "";
    try {
      targets.value = await tauriApi.versions.targets();
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
    } finally {
      loadingTargets.value = false;
    }
  }

  async function openTarget(targetPath: string): Promise<void> {
    activeTarget.value = targetPath;
    detail.value = null;
    await loadTimeline();
  }

  async function loadTimeline(): Promise<void> {
    if (!isTauri() || !activeTarget.value) {
      changesets.value = [];
      return;
    }
    loadingTimeline.value = true;
    error.value = "";
    try {
      changesets.value = await tauriApi.versions.changesets(activeTarget.value);
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
    } finally {
      loadingTimeline.value = false;
    }
  }

  async function loadDetail(changesetId: number): Promise<void> {
    if (!isTauri()) return;
    try {
      detail.value = await tauriApi.versions.detail(changesetId);
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
    }
  }

  /** 把目标文件当前内容登记为基线版本（幂等）。 */
  async function captureBaseline(): Promise<void> {
    if (!isTauri() || !activeTarget.value) return;
    busy.value = true;
    try {
      const result = await tauriApi.versions.captureBaseline(activeTarget.value);
      if (result.created) {
        toast.success(`Baseline v${result.changeset.revision}`);
      }
      await loadTargets();
      await loadTimeline();
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy.value = false;
    }
  }

  /**
   * 回滚到某条版本。回滚本身会追加一条新版本，所以时间线要整体刷新。
   * 目标文件被工具外改动过时后端会返回 `external_drift`，需 `force` 重试。
   */
  async function rollback(changesetId: number, force = false): Promise<boolean> {
    if (!isTauri() || !activeTarget.value) return false;
    busy.value = true;
    error.value = "";
    try {
      const result = await tauriApi.versions.rollback(changesetId, force);
      toast.success(
        `已回滚到 v${result.targetRevision}（新版本 v${result.newChangeset.revision}）`,
      );
      await loadTargets();
      await loadTimeline();
      return true;
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      error.value = message;
      // 漂移需要用户显式确认（会丢掉外部改动）
      if (message.includes("external_drift")) {
        toast.error("目标文件已被外部改动，需勾选「强制」后重试");
      } else {
        toast.error(message);
      }
      return false;
    } finally {
      busy.value = false;
    }
  }

  /** 删除一条版本记录（只删元数据，不动磁盘文件）。 */
  async function remove(changesetId: number): Promise<void> {
    if (!isTauri() || !activeTarget.value) return;
    busy.value = true;
    try {
      await tauriApi.versions.remove(changesetId);
      if (detail.value?.changeset.id === changesetId) detail.value = null;
      await loadTargets();
      await loadTimeline();
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy.value = false;
    }
  }

  return {
    targets,
    activeTarget,
    changesets,
    detail,
    loadingTargets,
    loadingTimeline,
    busy,
    error,
    loadTargets,
    openTarget,
    loadTimeline,
    loadDetail,
    captureBaseline,
    rollback,
    remove,
  };
});
