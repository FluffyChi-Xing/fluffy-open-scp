<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import FSheet from "@/components/ui/FSheet.vue";
import { isTauri, tauriApi } from "@/api";
import { useModProjectsStore } from "@/stores/modProjects";
import ProjectDashboard from "./components/dashboard/ProjectDashboard.vue";
import ProjectFormSheet from "./components/ProjectFormSheet.vue";
import GroupManagerSheet from "./components/GroupManagerSheet.vue";
import CodeWorkbench from "./components/CodeWorkbench.vue";
import type { ModProjectView } from "@/api/tauri";

/**
 * 开发工作台 = 项目管理面板：统计仪表盘 + 项目列表（分组过滤、CRUD）。
 * 「开发」按钮展开全屏 Code 工作台 sheet（项目文件夹的文件树 + 解析预览）。
 * 原多工具卡片页在 /studio/overview（头部"面板总览"进入）。
 */
const { t } = useI18n();
const store = useModProjectsStore();

const formOpen = ref(false);
const editingProject = ref<ModProjectView | null>(null);
const groupsOpen = ref(false);
const developOpen = ref(false);
const developingProject = ref<ModProjectView | null>(null);

onMounted(() => {
  if (isTauri()) void store.loadAll();
});

function openCreate() {
  editingProject.value = null;
  formOpen.value = true;
}

function openEdit(project: ModProjectView) {
  editingProject.value = project;
  formOpen.value = true;
}

function openDevelop(project: ModProjectView) {
  developingProject.value = project;
  developOpen.value = true;
}

async function submitCreate(
  name: string,
  groupId?: number,
  description?: string,
) {
  await store.createProject(name, groupId, description);
}

async function submitUpdate(
  id: number,
  fields: {
    name: string;
    groupId?: number;
    description?: string;
    status: string;
  },
) {
  await store.updateProject(id, fields);
}

async function submitDelete(project: ModProjectView) {
  if (!window.confirm(t("studio.projects.deleteConfirm"))) return;
  await store.deleteProject(project.id);
}

async function pickDevRoot() {
  const path = await tauriApi.workspace.pickDirectory(
    t("studio.projects.pickDevRootTitle"),
  );
  if (!path) return;
  try {
    await store.setDevRoot(path);
  } catch (cause) {
    store.error =
      cause && typeof cause === "object" && "message" in cause
        ? String(cause.message)
        : String(cause);
  }
}

function formatDateTime(value: number): string {
  if (!value) return "—";
  return new Intl.DateTimeFormat(undefined, {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function statusLabel(status: string): string {
  const key = `studio.dashboard.status.${status}`;
  const label = t(key);
  return label === key ? status : label;
}
</script>

<template>
  <section class="studio-page">
    <header class="page-heading">
      <div>
        <p class="eyebrow">{{ $t("studio.eyebrow") }}</p>
        <FTypography :header="1" spacing="none">{{
          $t("studio.projects.title")
        }}</FTypography>
        <FTypography paragraphy type="secondary">{{
          $t("studio.projects.description")
        }}</FTypography>
      </div>
      <nav class="heading-links">
        <RouterLink class="overview-link" to="/studio/overview">
          <FIcon name="LayoutDashboard" :size="13" aria-label="" />
          {{ $t("studio.projects.overviewLink") }}
        </RouterLink>
      </nav>
    </header>

    <template v-if="isTauri()">
      <div class="root-bar">
        <FIcon name="FolderOpen" :size="15" aria-label="" />
        <span class="root-label">{{ $t("studio.projects.devRootLabel") }}</span>
        <code class="root-path" :class="{ missing: !store.devRoot }">{{
          store.devRoot || $t("studio.projects.devRootMissing")
        }}</code>
        <button class="toolbar-button" type="button" @click="pickDevRoot">
          <FIcon name="FolderOpen" :size="13" aria-label="" />
          {{ $t("studio.projects.pickDevRoot") }}
        </button>
        <span class="spacer"></span>
        <button class="toolbar-button" type="button" @click="groupsOpen = true">
          <FIcon name="Tag" :size="13" aria-label="" />
          {{ $t("studio.projects.groups.manage") }}
        </button>
        <button
          class="toolbar-button primary"
          type="button"
          :disabled="!store.devRoot"
          @click="openCreate"
        >
          <FIcon name="Plus" :size="13" aria-label="" />
          {{ $t("studio.projects.newProject") }}
        </button>
      </div>

      <p v-if="store.error" class="error-banner" role="alert">
        {{ store.error }}
      </p>

      <ProjectDashboard :stats="store.stats" />

      <div v-if="store.devRoot" class="project-pane">
        <div class="pane-head">
          <div
            class="group-tabs"
            role="tablist"
            :aria-label="$t('studio.projects.filterLabel')"
          >
            <button
              class="group-tab"
              role="tab"
              :aria-selected="store.groupFilter === 'all'"
              :class="{ active: store.groupFilter === 'all' }"
              @click="store.groupFilter = 'all'"
            >
              {{ $t("studio.projects.filterAll") }}
              <span class="count">{{ store.projects.length }}</span>
            </button>
            <button
              class="group-tab"
              role="tab"
              :aria-selected="store.groupFilter === 'ungrouped'"
              :class="{ active: store.groupFilter === 'ungrouped' }"
              @click="store.groupFilter = 'ungrouped'"
            >
              {{ $t("studio.projects.filterUngrouped") }}
              <span class="count">{{
                store.projects.filter((project) => !project.groupId).length
              }}</span>
            </button>
            <button
              v-for="group in store.groups"
              :key="group.id"
              class="group-tab"
              role="tab"
              :aria-selected="store.groupFilter === String(group.id)"
              :class="{ active: store.groupFilter === String(group.id) }"
              @click="store.groupFilter = String(group.id)"
            >
              {{ group.name }}
              <span class="count">{{
                store.projects.filter((project) => project.groupId === group.id)
                  .length
              }}</span>
            </button>
          </div>
        </div>

        <div v-if="store.loading" class="pane-empty">
          {{ $t("common.loading") }}
        </div>
        <div v-else-if="!store.filteredProjects.length" class="pane-empty">
          {{ $t("studio.projects.empty") }}
        </div>
        <div
          v-else
          class="table"
          role="table"
          :aria-label="$t('studio.projects.tableLabel')"
        >
          <div class="row row-head" role="row">
            <span role="columnheader">{{ $t("studio.projects.colName") }}</span>
            <span role="columnheader">{{
              $t("studio.projects.colStatus")
            }}</span>
            <span role="columnheader">{{
              $t("studio.projects.colGroup")
            }}</span>
            <span role="columnheader">{{
              $t("studio.projects.colUpdated")
            }}</span>
            <span role="columnheader" class="end">{{
              $t("studio.projects.colActions")
            }}</span>
          </div>
          <div
            v-for="project in store.filteredProjects"
            :key="project.id"
            class="row"
            role="row"
          >
            <span class="name-cell" role="cell">
              <strong>{{ project.name }}</strong>
              <small v-if="project.description">{{
                project.description
              }}</small>
              <small v-if="!project.folderExists" class="missing-flag">
                <FIcon name="CircleAlert" :size="11" aria-label="" />
                {{ $t("studio.projects.folderMissing") }}
              </small>
            </span>
            <span role="cell">
              <span class="status-pill" :class="project.status">{{
                statusLabel(project.status)
              }}</span>
            </span>
            <span class="group-cell" role="cell">{{
              store.groupName(project.groupId) ||
              $t("studio.projects.filterUngrouped")
            }}</span>
            <span class="time-cell" role="cell">{{
              formatDateTime(project.updatedAt)
            }}</span>
            <span class="actions-cell end" role="cell">
              <button
                class="row-button develop"
                type="button"
                :disabled="!project.folderExists"
                :title="$t('studio.projects.openHint')"
                @click="openDevelop(project)"
              >
                <FIcon name="CodeXml" :size="12" aria-label="" />
                {{ $t("studio.projects.open") }}
              </button>
              <button
                class="row-button"
                type="button"
                @click="openEdit(project)"
              >
                <FIcon name="Pencil" :size="12" aria-label="" />
                {{ $t("studio.projects.edit") }}
              </button>
              <button
                class="row-button danger"
                type="button"
                @click="submitDelete(project)"
              >
                <FIcon name="Trash2" :size="12" aria-label="" />
                {{ $t("studio.projects.delete") }}
              </button>
            </span>
          </div>
        </div>
      </div>

      <ProjectFormSheet
        v-model:open="formOpen"
        :groups="store.groups"
        :project="editingProject"
        @create="submitCreate"
        @update="submitUpdate"
      />
      <GroupManagerSheet
        v-model:open="groupsOpen"
        :groups="store.groups"
        :projects="store.projects"
        @create="store.createGroup"
        @rename="store.renameGroup"
        @remove="store.deleteGroup"
      />

      <!-- 开发：全屏 Code 工作台（项目文件夹的文件树 + 解析预览） -->
      <FSheet
        v-if="developingProject"
        v-model:open="developOpen"
        width="100vw"
        :label="$t('studio.code.sheetTitle')"
      >
        <CodeWorkbench
          :project="developingProject"
          @close="developOpen = false"
        />
      </FSheet>
    </template>
    <p v-else class="web-hint">{{ $t("studio.projects.webHint") }}</p>
  </section>
</template>

<style scoped>
.studio-page {
  display: grid;
  gap: 18px;
}
.page-heading {
  align-items: flex-start;
  display: flex;
  justify-content: space-between;
  gap: 16px;
}
.eyebrow {
  color: var(--primary);
  font-size: 11px;
  font-weight: 750;
  letter-spacing: 0.08em;
  margin: 0 0 10px;
}
.heading-links {
  padding-top: 26px;
}
.overview-link {
  align-items: center;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  display: inline-flex;
  font-size: 12px;
  gap: 6px;
  min-height: 30px;
  padding: 0 12px;
  text-decoration: none;
}
.overview-link:hover {
  background: var(--surface-hover);
  color: var(--foreground);
}
.root-bar {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  padding: 10px 14px;
}
.root-label {
  color: var(--muted-foreground);
  font-size: 11.5px;
}
.root-path {
  color: var(--foreground);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.root-path.missing {
  color: var(--warning);
}
.root-bar .spacer {
  flex: 1;
}
.toolbar-button {
  align-items: center;
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12px;
  gap: 6px;
  min-height: 30px;
  padding: 0 12px;
}
.toolbar-button:hover:not(:disabled) {
  background: var(--surface-hover);
}
.toolbar-button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}
.toolbar-button.primary {
  background: var(--primary);
  border-color: var(--primary);
  color: #fff;
}
.toolbar-button.primary:hover:not(:disabled) {
  background: color-mix(in srgb, var(--primary) 85%, #fff);
  border-color: color-mix(in srgb, var(--primary) 85%, #fff);
}
.error-banner {
  background: color-mix(in srgb, var(--danger) 12%, transparent);
  border: 1px solid color-mix(in srgb, var(--danger) 40%, transparent);
  border-radius: var(--radius-md);
  color: var(--danger);
  font-size: 12px;
  margin: 0;
  overflow-wrap: anywhere;
  padding: 10px 14px;
}
.project-pane {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
}
.pane-head {
  border-bottom: 1px solid var(--border);
  display: flex;
  gap: 10px;
  overflow-x: auto;
  padding: 10px 14px 0;
}
.group-tabs {
  display: flex;
  gap: 2px;
}
.group-tab {
  align-items: center;
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 12.5px;
  gap: 6px;
  min-height: 32px;
  padding: 0 10px;
}
.group-tab.active {
  border-bottom-color: var(--primary);
  color: var(--foreground);
  font-weight: 700;
}
.count {
  align-self: center;
  background: var(--surface-elevated);
  border-radius: 999px;
  font-size: 10.5px;
  font-variant-numeric: tabular-nums;
  line-height: 1.4;
  padding: 1px 7px;
}
.pane-empty {
  color: var(--subtle-foreground);
  font-size: 12.5px;
  padding: 32px 12px;
  text-align: center;
}
.table {
  display: grid;
}
.row {
  border-top: 1px solid var(--border);
  display: grid;
  gap: 12px;
  grid-template-columns:
    minmax(160px, 1.6fr) 92px minmax(80px, 0.8fr)
    110px minmax(210px, auto);
  align-items: center;
  padding: 9px 14px;
}
.row:first-child {
  border-top: 0;
}
.row-head {
  border-top: 0;
  color: var(--subtle-foreground);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.name-cell {
  display: grid;
  gap: 2px;
  min-width: 0;
}
.name-cell strong {
  font-size: 12.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.name-cell small {
  color: var(--muted-foreground);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.missing-flag {
  align-items: center;
  color: var(--warning) !important;
  display: inline-flex;
  gap: 4px;
}
.status-pill {
  border-radius: 999px;
  display: inline-block;
  font-size: 11px;
  padding: 2px 9px;
}
.status-pill.active {
  background: color-mix(in srgb, var(--success) 15%, transparent);
  color: var(--success);
}
.status-pill.released {
  background: color-mix(in srgb, var(--primary) 16%, transparent);
  color: var(--primary);
}
.status-pill.archived {
  background: color-mix(in srgb, var(--muted-foreground) 16%, transparent);
  color: var(--muted-foreground);
}
.group-cell,
.time-cell {
  color: var(--muted-foreground);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.actions-cell {
  display: flex;
  gap: 4px;
  justify-content: flex-end;
}
.row-button {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-size: 11.5px;
  gap: 4px;
  min-height: 26px;
  opacity: 0.25;
  padding: 0 8px;
}
.row:hover .row-button,
.row-button:focus-visible {
  opacity: 1;
}
.row-button:hover:not(:disabled) {
  background: var(--surface-hover);
  color: var(--foreground);
}
.row-button:disabled {
  cursor: not-allowed;
  opacity: 0.25;
}
.row-button.develop {
  color: var(--primary);
  opacity: 1;
}
.row-button.develop:hover:not(:disabled) {
  background: color-mix(in srgb, var(--primary) 12%, transparent);
  color: var(--primary);
}
.row-button.danger:hover {
  color: var(--danger);
}
.end {
  text-align: end;
}
.web-hint {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  color: var(--muted-foreground);
  font-size: 12.5px;
  padding: 26px 12px;
  text-align: center;
}
@media (max-width: 900px) {
  .row {
    grid-template-columns: minmax(140px, 1.4fr) 90px minmax(200px, auto);
  }
  .group-cell,
  .time-cell {
    display: none;
  }
}
</style>
