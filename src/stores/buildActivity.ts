import { computed, ref, shallowRef } from "vue";
import { defineStore } from "pinia";
import { isTauri } from "@/api";
import { subscribe, type BuildQueueEventPayload } from "@/api/tauri";

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
  read: boolean;
}

const MAX_NOTICES = 40;

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
    jobId?: string;
  }) {
    // 同 job + 同 title 去重，避免 queued/running 重复刷屏。
    if (
      input.jobId &&
      notices.value[0]?.jobId === input.jobId &&
      notices.value[0]?.titleKey === input.titleKey
    ) {
      return notices.value[0];
    }
    const notice: BuildNotice = {
      id: `bn-${nextId++}`,
      titleKey: input.titleKey,
      detail: input.detail,
      tone: input.tone,
      createdAtMs: Date.now(),
      projectRoot: input.projectRoot,
      projectLabel: input.projectLabel,
      jobId: input.jobId,
      read: false,
    };
    notices.value = [notice, ...notices.value].slice(0, MAX_NOTICES);
    if (input.jobId) lastJobId.value = input.jobId;
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
    notices.value = [];
  }

  function handlePayload(payload: BuildQueueEventPayload) {
    // Stage-only ticks update Actions panel live; the message bar only
    // records job lifecycle transitions so it stays scannable.
    if (payload.event && !payload.status) return;

    if (payload.status === "queued") {
      pushNotice({
        titleKey: "notification.buildQueued",
        detail: payload.label || payload.jobId,
        tone: "info",
        jobId: payload.jobId,
        projectLabel: payload.label ?? undefined,
      });
      return;
    }
    if (payload.status === "running") {
      pushNotice({
        titleKey: "notification.buildRunning",
        detail: payload.label || payload.jobId,
        tone: "info",
        jobId: payload.jobId,
        projectLabel: payload.label ?? undefined,
      });
      return;
    }
    if (payload.status === "finished") {
      pushNotice({
        titleKey: "notification.buildFinished",
        detail: payload.label || payload.jobId,
        tone: "success",
        jobId: payload.jobId,
        projectLabel: payload.label ?? undefined,
      });
      return;
    }
    if (payload.status === "failed") {
      pushNotice({
        titleKey: "notification.buildFailed",
        detail: payload.error || payload.label || payload.jobId,
        tone: "error",
        jobId: payload.jobId,
        projectLabel: payload.label ?? undefined,
      });
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
