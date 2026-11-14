<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import { isTauri } from "@/api";
import { useModProjectsStore } from "@/stores/modProjects";
import type { ModProjectView } from "@/api/tauri";
import CodeWorkbench from "../components/CodeWorkbench.vue";
import BuildActionsPanel from "../components/asset/BuildActionsPanel.vue";
import ProjectRepoCodeTab from "../components/project-detail/ProjectRepoCodeTab.vue";
import ProjectRepoSettingsTab from "../components/project-detail/ProjectRepoSettingsTab.vue";
import ProjectAssetCaptureTab from "../components/project-detail/ProjectAssetCaptureTab.vue";

/**
 * 项目详情页（GitHub repo 形态）：
 * Code / Issues(灰) / Develop / Actions / Settings。
 * Develop = 原全屏 Code 工作台；编辑/删除迁入 Settings。
 */
type RepoTab = "code" | "issues" | "develop" | "actions" | "settings" | "capture";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const store = useModProjectsStore();

const projectId = computed(() => Number(route.params.id));
const project = computed(
  () => store.projects.find((entry) => entry.id === projectId.value) ?? null,
);

const tab = ref<RepoTab>("code");

const tabs = computed(() => [
  { id: "code" as const, label: t("studio.repo.tabCode"), icon: "CodeXml", disabled: false },
  {
    id: "issues" as const,
    label: t("studio.repo.tabIssues"),
    icon: "CircleDot",
    disabled: true,
  },
  {
    id: "develop" as const,
    label: t("studio.repo.tabDevelop"),
    icon: "Hammer",
    disabled: !(project.value?.folderExists ?? false),
  },
  {
    id: "actions" as const,
    label: t("studio.repo.tabActions"),
    icon: "Activity",
    disabled: !(project.value?.folderPath ?? false),
  },
  {
    id: "settings" as const,
    label: t("studio.repo.tabSettings"),
    icon: "Settings",
    disabled: false,
  },
  {
    id: "capture" as const,
    label: t("assetCapture.title"),
    icon: "Terminal",
    disabled: !(project.value?.folderExists ?? false),
  },
]);

function statusLabel(status: string): string {
  const key = `studio.dashboard.status.${status}`;
  const label = t(key);
  return label === key ? status : label;
}

function selectTab(next: RepoTab, disabled: boolean) {
  if (disabled) return;
  tab.value = next;
  void router.replace({
    query: { ...route.query, tab: next === "code" ? undefined : next },
  });
}

function syncTabFromQuery() {
  const raw = String(route.query.tab ?? "code");
  const allowed: RepoTab[] = [
    "code",
    "issues",
    "develop",
    "actions",
    "settings",
    "capture",
  ];
  if (!allowed.includes(raw as RepoTab) || raw === "issues") {
    tab.value = "code";
    return;
  }
  if (
    (raw === "develop" || raw === "actions" || raw === "capture") &&
    !(project.value?.folderExists ?? false)
  ) {
    tab.value = "code";
    return;
  }
  tab.value = raw as RepoTab;
}

async function onRemove(target: ModProjectView) {
  await store.deleteProject(target.id);
  await router.push({ name: "studio" });
}

onMounted(async () => {
  if (isTauri() && !store.projects.length) {
    await store.loadAll();
  }
  syncTabFromQuery();
});

watch(
  () => [route.params.id, route.query.tab, project.value?.id] as const,
  () => syncTabFromQuery(),
);
</script>

<template>
  <section class="repo-page">
    <p v-if="!isTauri()" class="web-hint">{{ $t("studio.projects.webHint") }}</p>
    <template v-else-if="!project">
      <p class="web-hint">{{ $t("studio.repo.notFound") }}</p>
      <RouterLink class="back-link" :to="{ name: 'studio' }">
        <FIcon name="ArrowLeft" :size="13" aria-label="" />
        {{ $t("studio.repo.backToList") }}
      </RouterLink>
    </template>
    <template v-else>
      <header class="repo-header">
        <div class="repo-title-row">
          <RouterLink class="back-link" :to="{ name: 'studio' }">
            <FIcon name="ArrowLeft" :size="13" aria-label="" />
            {{ $t("studio.repo.backToList") }}
          </RouterLink>
        </div>
        <div class="repo-name-row">
          <FIcon name="BookOpen" :size="18" aria-label="" />
          <FTypography :header="1" spacing="none">{{ project.name }}</FTypography>
          <span class="status-pill" :class="project.status">{{
            statusLabel(project.status)
          }}</span>
          <span class="private-pill">{{ $t("studio.repo.private") }}</span>
        </div>
        <p v-if="project.description" class="repo-desc">
          {{ project.description }}
        </p>
      </header>

      <nav
        class="repo-tabs"
        role="tablist"
        :aria-label="$t('studio.repo.tabsLabel')"
      >
        <button
          v-for="item in tabs"
          :key="item.id"
          class="repo-tab"
          type="button"
          role="tab"
          :aria-selected="tab === item.id"
          :aria-disabled="item.disabled || undefined"
          :disabled="item.disabled"
          :class="{ active: tab === item.id, disabled: item.disabled }"
          @click="selectTab(item.id, item.disabled)"
        >
          <FIcon :name="item.icon" :size="13" aria-label="" />
          {{ item.label }}
        </button>
      </nav>

      <div class="repo-body">
        <ProjectRepoCodeTab
          v-if="tab === 'code'"
          :project="project"
          :group-name="store.groupName(project.groupId)"
        />

        <div
          v-else-if="tab === 'develop'"
          class="develop-host"
        >
          <CodeWorkbench
            :key="project.id"
            :project="project"
            embedded
            @close="selectTab('code', false)"
          />
        </div>

        <section v-else-if="tab === 'actions'" class="actions-host">
          <BuildActionsPanel
            v-if="project.folderPath"
            :project="project.folderPath"
          />
        </section>

        <ProjectRepoSettingsTab
          v-else-if="tab === 'settings'"
          :project="project"
          :groups="store.groups"
          @remove="onRemove"
        />
        <ProjectAssetCaptureTab
          v-else-if="tab === 'capture'"
          :key="project.id"
          :project="project.relPath"
        />
      </div>
    </template>
  </section>
</template>

<style scoped>
.repo-page {
  /* flex 而非默认 stretch 的 grid：否则 min-height 余量会均分拉高
   * tabs 行，导致下划线/底边远离标签（Actions/Settings 像样式错乱）。 */
  display: flex;
  flex-direction: column;
  gap: 0;
  min-height: calc(100vh - 120px);
}
.web-hint {
  color: var(--muted-foreground);
  font-size: 13px;
}
.back-link {
  align-items: center;
  color: var(--muted-foreground);
  display: inline-flex;
  font-size: 12.5px;
  gap: 6px;
  text-decoration: none;
  width: fit-content;
}
.back-link:hover {
  color: var(--primary);
}
.repo-header {
  display: grid;
  flex: 0 0 auto;
  gap: 10px;
  padding-bottom: 4px;
}
.repo-name-row {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}
.repo-desc {
  color: var(--muted-foreground);
  font-size: 13px;
  margin: 0;
  max-width: 720px;
}
.status-pill,
.private-pill {
  border: 1px solid var(--border);
  border-radius: 999px;
  font-size: 11px;
  padding: 2px 9px;
}
.status-pill.active {
  background: color-mix(in srgb, var(--primary) 18%, transparent);
  border-color: color-mix(in srgb, var(--primary) 40%, transparent);
  color: var(--primary);
}
.status-pill.archived {
  color: var(--muted-foreground);
}
.private-pill {
  color: var(--muted-foreground);
}
.repo-tabs {
  align-items: stretch;
  border-bottom: 1px solid var(--border);
  display: flex;
  flex: 0 0 auto;
  gap: 2px;
  margin-top: 14px;
  overflow-x: auto;
}
.repo-tab {
  align-items: center;
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  flex: 0 0 auto;
  font: inherit;
  font-size: 13px;
  gap: 6px;
  height: 40px;
  padding: 0 12px;
  white-space: nowrap;
}
.repo-tab:hover:not(:disabled) {
  color: var(--foreground);
}
.repo-tab.active {
  border-bottom-color: var(--primary);
  color: var(--foreground);
  font-weight: 600;
}
.repo-tab.disabled,
.repo-tab:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
.repo-body {
  min-height: 0;
  padding-top: 18px;
}
.develop-host {
  display: flex;
  flex-direction: column;
  /* 仅场景编辑器容器：固定 100vh，其它 tab / 页壳不动。 */
  height: 100vh;
  min-height: 0;
  min-width: 0;
}
.develop-host :deep(.code-page) {
  display: flex;
  flex: 1;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.develop-host :deep(.asset-workbench-host),
.develop-host :deep(.maw) {
  flex: 1;
  height: 100%;
  min-height: 0;
}
.actions-host {
  display: block;
  min-width: 0;
}
.actions-host :deep(.actions-panel) {
  gap: 14px;
}
</style>
