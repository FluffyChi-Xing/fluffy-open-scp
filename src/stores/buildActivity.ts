import { computed, ref, shallowRef } from "vue";
import { defineStore } from "pinia";
import { isTauri } from "@/api";
import { subscribe, type BuildJobStatus, type BuildQueueEventPayload } from "@/api/tauri";

export type BuildNoticeTone = "info" | "success" | "error";

export interface BuildNotice {
  id: string;
  /** i18n key under `notification.*` */
  titleKey: string;
  detail: string;
  tone: BuildNoticeTone;
  createdAtMs: number;
  projectRoot?: string;
  projectLabel?: string;
  jobId?: string;
  status?: BuildJobStatus;
  totalStages?: number;
  completedStages?: string[];
  read: boolean;
}

const MAX_NOTICES = 40;
const STATUS_ORDER: Record<BuildJobStatus, number> = {
  queued: 0,
  running: 1,
  finished: 2,
  failed: 2,
};

/**
 * 构建活动：全局订阅 `build-queue:event`，供消息栏与 Actions 卡片共用。
 * 场景编辑器入队后可关闭；进度仍写入本 store。
 */
export const useBuildActivityStore = defineStore("buildActivity", () => {
  const notices = ref<BuildNotice[]>([]);
  const lastProjectRoot = shallowRef<string | null>(null);
  const lastProjectLabel = shallowRef<string | null>(null);
  const lastJobId = shallowRef<string | null>(null);
  const listening = shallowRef(false);
  let nextId = 1;
  let unlisten: (() => void) | null = null;
  const dismissedJobs = new Set<string>();

  const unreadCount = computed(
    () => notices.value.filter((notice) => !notice.read).length,
  );

  function rememberProject(root: string, label?: string | null, jobId?: string | null) {
    if (!root.trim()) return;
    lastProjectRoot.value = root;
    if (label) lastProjectLabel.value = label;
    if (jobId) lastJobId.value = jobId;
  }

  function pushNotice(input: {
    titleKey: string;
    detail: string;
    tone: BuildNoticeTone;
    projectRoot?: string;
    projectLabel?: string;
  }) {
    const notice: BuildNotice = {
      id: `bn-${nextId++}`,
      titleKey: input.titleKey,
      detail: input.detail,
      tone: input.tone,
      createdAtMs: Date.now(),
      projectRoot: input.projectRoot,
      projectLabel: input.projectLabel,
      read: false,
    };
    notices.value = [notice, ...notices.value].slice(0, MAX_NOTICES);
    if (input.projectRoot) lastProjectRoot.value = input.projectRoot;
    if (input.projectLabel) lastProjectLabel.value = input.projectLabel;
    return notice;
  }

  function markAllRead() {
    notices.value = notices.value.map((notice) =>
      notice.read ? notice : { ...notice, read: true },
    );
  }

  function clear() {
    for (const notice of notices.value) {
      if (notice.jobId) dismissedJobs.add(notice.jobId);
    }
    notices.value = [];
  }

  function handlePayload(payload: BuildQueueEventPayload) {
    if (dismissedJobs.has(payload.jobId)) return;
    const status = payload.status ?? (payload.event ? "running" : undefined);
    if (!status) return;
    const current = notices.value.find((notice) => notice.jobId === payload.jobId);
    if (current?.status && (
      STATUS_ORDER[current.status] === 2 ||
      STATUS_ORDER[current.status] > STATUS_ORDER[status]
    )) return;

    const completedStages = current?.completedStages ?? [];
    const event = payload.event;
    const notice: BuildNotice = {
      id: current?.id ?? `build-${payload.jobId}`,
      titleKey: "notification.packageBuild",
      detail: status === "failed" ? payload.error ?? "" : "",
      tone: status === "failed" ? "error" : status === "finished" ? "success" : "info",
      createdAtMs: current?.createdAtMs ?? Date.now(),
      projectLabel: payload.label || current?.projectLabel,
      jobId: payload.jobId,
      status,
      totalStages: payload.totalStages ?? current?.totalStages,
      completedStages: event?.kind.type === "finished" && !completedStages.includes(event.stage)
        ? [...completedStages, event.stage]
        : completedStages,
      read: current?.read ?? false,
    };
    if (current) {
      Object.assign(current, notice);
    } else {
      notices.value = [notice, ...notices.value].slice(0, MAX_NOTICES);
      lastJobId.value = payload.jobId;
      if (notice.projectLabel) lastProjectLabel.value = notice.projectLabel;
    }
  }

  async function startListening() {
    if (listening.value || !isTauri()) return;
    listening.value = true;
    try {
      unlisten = await subscribe<BuildQueueEventPayload>(
        "build-queue:event",
        handlePayload,
      );
    } catch {
      listening.value = false;
    }
  }

  function stopListening() {
    unlisten?.();
    unlisten = null;
    listening.value = false;
  }

  return {
    notices,
    unreadCount,
    lastProjectRoot,
    lastProjectLabel,
    lastJobId,
    rememberProject,
    pushNotice,
    markAllRead,
    clear,
    startListening,
    stopListening,
  };
});
