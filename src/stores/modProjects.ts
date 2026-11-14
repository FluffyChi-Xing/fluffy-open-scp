import { computed, ref, shallowRef } from "vue";
import { defineStore } from "pinia";
import { isTauri, tauriApi } from "@/api";
import type {
  LegacySyncStatus,
  ModProjectGroup,
  ModProjectStats,
  ModProjectView,
  SetupStatusResponse,
} from "@/api/tauri";

const DEV_ROOT_KEY = "openscp:mod-dev-root";

function messageOf(cause: unknown): string {
  return cause && typeof cause === "object" && "message" in cause
    ? String(cause.message)
    : String(cause);
}

/**
 * 开发工作台（项目管理面板）全局状态：开发目录、项目/分组列表、
 * 统计数据与配置状态（启动缺失检测、引导页共用同一份数据源）。
 * 开发目录自 2026-10-29 起为安装目录下自动创建的 mods/，不再手动指定；
 * 历史手动配置的目录经 legacySync 一次性迁入。
 */
export const useModProjectsStore = defineStore("modProjects", () => {
  const devRoot = ref(localStorage.getItem(DEV_ROOT_KEY) ?? "");
  const setupStatus = shallowRef<SetupStatusResponse | null>(null);
  const legacyStatus = shallowRef<LegacySyncStatus | null>(null);
  const syncing = ref(false);
  const projects = shallowRef<ModProjectView[]>([]);
  const groups = shallowRef<ModProjectGroup[]>([]);
  const stats = shallowRef<ModProjectStats | null>(null);
  const loading = ref(false);
  const error = shallowRef("");

  /** 项目列表按分组过滤："all" | "ungrouped" | 分组 id 字符串。 */
  const groupFilter = ref("all");

  const filteredProjects = computed(() => {
    if (groupFilter.value === "all") return projects.value;
    if (groupFilter.value === "ungrouped") {
      return projects.value.filter((project) => !project.groupId);
    }
    const groupId = Number(groupFilter.value);
    return projects.value.filter((project) => project.groupId === groupId);
  });

  function groupName(groupId?: number): string {
    if (!groupId) return "";
    return groups.value.find((group) => group.id === groupId)?.name ?? "";
  }

  async function loadSetupStatus() {
    if (!isTauri()) return null;
    try {
      const result = await tauriApi.studio.setupStatus();
      setupStatus.value = result;
      if (result.modRoot.path) {
        devRoot.value = result.modRoot.path;
        localStorage.setItem(DEV_ROOT_KEY, devRoot.value);
      }
    } catch {
      setupStatus.value = null;
    }
    await loadLegacyStatus();
    return setupStatus.value;
  }

  /** 历史目录迁移状态：legacyDir 存在且 ≠ 内部 mods/ 时显示同步按钮。 */
  async function loadLegacyStatus() {
    if (!isTauri()) return;
    try {
      legacyStatus.value = await tauriApi.studio.legacyStatus();
    } catch {
      legacyStatus.value = null;
    }
  }

  /** 一次性迁移：旧 mod_root 项目文件夹合并拷入内部 mods/ 后切换配置。 */
  async function syncLegacy(): Promise<number> {
    syncing.value = true;
    error.value = "";
    try {
      const outcome = await tauriApi.studio.syncLegacy();
      legacyStatus.value = outcome.status;
      devRoot.value = outcome.status.internalDir;
      localStorage.setItem(DEV_ROOT_KEY, devRoot.value);
      await loadAll();
      return outcome.copiedCount;
    } catch (cause) {
      error.value = messageOf(cause);
      return 0;
    } finally {
      syncing.value = false;
    }
  }

  async function loadAll() {
    if (!isTauri()) return;
    loading.value = true;
    error.value = "";
    try {
      const [projectList, groupList, statList] = await Promise.all([
        tauriApi.studio.projects.list(),
        tauriApi.studio.groups.list(),
        tauriApi.studio.stats(),
      ]);
      projects.value = projectList;
      groups.value = groupList;
      stats.value = statList;
    } catch (cause) {
      error.value = messageOf(cause);
    } finally {
      loading.value = false;
    }
    // 同步按钮状态随每次进入页面刷新（引导时的首次查询可能失败）
    void loadLegacyStatus();
  }

  async function createProject(
    name: string,
    groupId?: number,
    description?: string,
    author?: string,
    modType?: string,
  ) {
    await tauriApi.studio.projects.create(
      name,
      groupId,
      description,
      author,
      modType,
    );
    await loadAll();
  }

  async function updateProject(
    id: number,
    fields: {
      name: string;
      groupId?: number;
      description?: string;
      status: string;
    },
  ) {
    await tauriApi.studio.projects.update(id, fields);
    await loadAll();
  }

  async function deleteProject(id: number) {
    await tauriApi.studio.projects.remove(id);
    await loadAll();
  }

  async function createGroup(name: string) {
    await tauriApi.studio.groups.create(name);
    await loadAll();
  }

  async function renameGroup(id: number, name: string) {
    await tauriApi.studio.groups.rename(id, name);
    await loadAll();
  }

  async function deleteGroup(id: number) {
    await tauriApi.studio.groups.remove(id);
    if (groupFilter.value === String(id)) groupFilter.value = "all";
    await loadAll();
  }

  return {
    devRoot,
    setupStatus,
    legacyStatus,
    syncing,
    projects,
    groups,
    stats,
    loading,
    error,
    groupFilter,
    filteredProjects,
    groupName,
    loadSetupStatus,
    loadAll,
    loadLegacyStatus,
    syncLegacy,
    createProject,
    updateProject,
    deleteProject,
    createGroup,
    renameGroup,
    deleteGroup,
  };
});
