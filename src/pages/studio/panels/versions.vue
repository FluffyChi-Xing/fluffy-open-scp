<script setup lang="ts">
/**
 * 编辑版本控制台：按目标文件列出该 overlay 的版本线（资源级前后字节）。
 *
 * 版面参考 git 客户端 / Vercel 部署列表：**左栏文件轨 + 右栏主区**，
 * 主区从上到下是「文件头 → 版本表 → 变更明细」。每个版本是一行定宽网格
 * （版本 / 写入方 / 资源 / 字节 / 时间 / 动作），所有数据落在同一组列上。
 * 版本行必须同时带 `rev-row`（网格）与 `node`（脊线）两个类，缺一个就散架；
 * 列降级按 pane 容器宽度（容器查询），只有双栏并单栏才是视口级决策。
 *
 * 版本脊线是页面的签名元素：每个节点自绘一段竖线，最新版用品牌色实心
 * 圆点 +「最新」徽章，其余为空心——与渲染耗时卡的「实心=当前」语汇一致。
 * 状态章沿用 git 语义配色：新增=绿 / 修改=橙 / 删除=红。
 *
 * 回滚会**立即改写磁盘文件**，因此二次确认；外部漂移需额外勾选「强制」。
 * 删除只清历史记录、不动文件——按钮文案因此是「删除记录」。
 * 页面自身不加左右内边距：应用壳的 content-frame 已提供。
 */
import { computed, onMounted, ref } from "vue";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import { RouterLink } from "vue-router";
import FIcon from "@/components/extensions/FIcon.vue";
import FEmpty from "@/components/extensions/FEmpty.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import { isTauri } from "@/api";
import type { ChangesetSummary } from "@/api/tauri";
import { useVersionConsoleStore } from "@/stores/versionConsole";
import { useToast } from "@/composables/useToast";

const { t } = useI18n();
const toast = useToast();
const consoleStore = useVersionConsoleStore();
const {
  targets,
  activeTarget,
  changesets,
  detail,
  loadingTargets,
  loadingTimeline,
  busy,
  error,
} = storeToRefs(consoleStore);

/** 漂移时需用户显式勾选（会丢掉工具之外的改动）。 */
const force = ref(false);

const latestRevision = computed(() => changesets.value[0]?.revision ?? null);

onMounted(() => {
  void consoleStore.loadTargets();
});

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function formatTime(timestamp: number): string {
  return new Date(timestamp).toLocaleString();
}

/** 行内只留月-日 时:分，完整时间放 title。 */
function formatTimeShort(timestamp: number): string {
  const d = new Date(timestamp);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function formatBytes(value: number): string {
  return value.toLocaleString();
}

const STATUS_KEYS: Record<string, string> = {
  added: "version.statusAdded",
  removed: "version.statusRemoved",
  modified: "version.statusModified",
};

function statusLabel(status: string): string {
  const key = STATUS_KEYS[status];
  return key ? t(key) : status;
}

function tgiLabel(item: { typeId: number; groupId: number; instance: number }): string {
  const hex = (value: number) => value.toString(16).toUpperCase().padStart(4, "0");
  return `${hex(item.typeId)}:${hex(item.groupId)}:${hex(item.instance)}`;
}

async function captureBaseline(): Promise<void> {
  await consoleStore.captureBaseline();
  if (!error.value && !busy.value) toast.info(t("version.captureBaseline"));
}

async function onRollback(entry: ChangesetSummary): Promise<void> {
  if (!window.confirm(t("version.rollbackConfirm", { n: entry.revision }))) return;
  await consoleStore.rollback(entry.id, force.value);
}

async function onDelete(entry: ChangesetSummary): Promise<void> {
  if (!window.confirm(t("version.deleteConfirm", { n: entry.revision }))) return;
  await consoleStore.remove(entry.id);
}
</script>

<template>
  <section class="versions-page">
    <RouterLink class="back-link" to="/studio">
      <FIcon name="ArrowLeft" :size="14" aria-label="" />
      {{ t("version.backToStudio") }}
    </RouterLink>

    <header class="page-header">
      <span class="page-icon"><FIcon name="History" :size="20" aria-label="" /></span>
      <div>
        <FTypography :header="2" spacing="none">{{ t("version.title") }}</FTypography>
        <p class="page-meta">{{ t("version.description") }}</p>
      </div>
    </header>

    <p v-if="!isTauri()" class="notice" role="status">{{ t("version.needTauri") }}</p>
    <p v-else-if="error" class="notice error" role="alert">{{ error }}</p>

    <div class="console">
      <!-- 左栏：已跟踪文件 -->
      <aside class="pane files-pane">
        <header class="pane-head">
          <span class="pane-title">{{ t("version.targets") }}</span>
          <span v-if="targets.length" class="pane-badge">{{ targets.length }}</span>
        </header>
        <p v-if="loadingTargets" class="muted">{{ t("version.loading") }}</p>
        <p v-else-if="!targets.length" class="muted">{{ t("version.noTargets") }}</p>
        <ul v-else class="file-list">
          <li v-for="target in targets" :key="target.targetPath">
            <button
              type="button"
              class="file-row"
              :class="{ active: target.targetPath === activeTarget }"
              :title="target.targetPath"
              @click="consoleStore.openTarget(target.targetPath)"
            >
              <span class="file-icon" aria-hidden="true">
                <FIcon name="Package" :size="13" />
              </span>
              <span class="file-body">
                <span class="file-name">{{ fileName(target.targetPath) }}</span>
                <span class="file-meta">
                  <span class="tabnum">v{{ target.latestRevision }}</span>
                  <span class="sep">·</span>
                  <span class="tabnum">{{ target.changesetCount }}</span>
                </span>
              </span>
            </button>
          </li>
        </ul>
      </aside>

      <!-- 右栏：主区 -->
      <div class="main">
        <header v-if="activeTarget" class="file-bar">
          <div class="file-ident">
            <code class="file-path">{{ activeTarget }}</code>
            <span class="file-sub">
              <span v-if="latestRevision !== null" class="tabnum">
                {{ t("version.revisionLabel", { n: latestRevision }) }}
              </span>
              <span class="sep">·</span>
              <span class="tabnum">{{ changesets.length }}</span>
            </span>
          </div>
          <div class="file-actions">
            <label class="force-row" :title="t('version.forceHint')">
              <input v-model="force" type="checkbox" />
              <span>{{ t("version.rollbackForce") }}</span>
            </label>
            <button type="button" class="ghost-button" :disabled="busy" @click="captureBaseline">
              {{ t("version.captureBaseline") }}
            </button>
          </div>
        </header>

        <!-- 版本表 -->
        <section class="pane">
          <header class="pane-head">
            <span class="pane-title">{{ t("version.revisions") }}</span>
          </header>

          <FEmpty
            v-if="!activeTarget"
            variant="compact"
            icon-name="Package"
            :title="t('version.noTargets')"
          />
          <p v-else-if="loadingTimeline" class="muted">{{ t("version.loading") }}</p>
          <FEmpty
            v-else-if="!changesets.length"
            variant="compact"
            icon-name="History"
            :title="t('version.noRevisions')"
          />
          <template v-else>
            <div class="rev-row row-head" aria-hidden="true">
              <span />
              <span>{{ t("version.colVersion") }}</span>
              <span>{{ t("version.colWriter") }}</span>
              <span class="end">{{ t("version.colResources") }}</span>
              <span class="end">{{ t("version.colBytes") }}</span>
              <span class="end">{{ t("version.colTime") }}</span>
              <span />
            </div>
            <ol class="spine">
              <li
                v-for="(entry, index) in changesets"
                :key="entry.id"
                class="rev-row node"
                :class="{ head: index === 0, active: detail?.changeset.id === entry.id }"
              >
                <span class="marker" aria-hidden="true" />
                <span class="cell-version tabnum">v{{ entry.revision }}</span>
                <span class="cell-writer">
                  <span class="writer-name">{{ entry.writer }}</span>
                  <span v-if="index === 0" class="head-chip">{{ t("version.head") }}</span>
                  <span v-if="entry.restoredFrom" class="branch">
                    <FIcon name="History" :size="11" aria-label="" />
                    {{ t("version.restoredFrom", { n: entry.restoredFrom }) }}
                  </span>
                </span>
                <span class="end cell-resources tabnum">{{ entry.resourceCount }}</span>
                <span class="end cell-bytes tabnum">
                  {{ formatBytes(entry.bytesBefore) }}<span class="arrow">→</span>{{ formatBytes(entry.bytesAfter) }}
                </span>
                <span
                  class="end cell-time tabnum"
                  :title="formatTime(entry.createdAt)"
                >{{ formatTimeShort(entry.createdAt) }}</span>
                <span class="node-actions">
                  <button
                    type="button"
                    class="ghost-button icon-only"
                    :disabled="busy"
                    :aria-label="t('version.detail')"
                    :title="t('version.detail')"
                    @click="consoleStore.loadDetail(entry.id)"
                  >
                    <FIcon name="ListTree" :size="13" aria-label="" />
                  </button>
                  <button
                    type="button"
                    class="ghost-button accent"
                    :disabled="busy"
                    @click="onRollback(entry)"
                  >
                    {{ t("version.rollback") }}
                  </button>
                  <button
                    type="button"
                    class="ghost-button danger"
                    :disabled="busy"
                    @click="onDelete(entry)"
                  >
                    {{ t("version.delete") }}
                  </button>
                </span>
              </li>
            </ol>
          </template>
        </section>

        <!-- 变更明细 -->
        <section class="pane">
          <header class="pane-head">
            <span class="pane-title">
              {{
                detail
                  ? t("version.detailTitle", { n: detail.changeset.revision })
                  : t("version.detail")
              }}
            </span>
            <span v-if="detail" class="pane-badge">{{ detail.items.length }}</span>
          </header>
          <FEmpty
            v-if="!detail"
            variant="compact"
            icon-name="ListTree"
            :title="t('version.detailHint')"
          />
          <template v-else-if="detail.items.length">
            <div class="diff-scroll">
              <div class="diff-row row-head" aria-hidden="true">
                <span>{{ t("version.colStatus") }}</span>
                <span>{{ t("version.colTgi") }}</span>
                <span class="end">{{ t("version.colSize") }}</span>
              </div>
              <ul class="diff-list">
                <li
                  v-for="item in detail.items"
                  :key="`${item.typeId}:${item.groupId}:${item.instance}`"
                  class="diff-row"
                >
                  <span class="status" :class="item.status">{{ statusLabel(item.status) }}</span>
                  <code class="tgi">{{ tgiLabel(item) }}</code>
                  <span class="end tabnum">
                    {{ formatBytes(item.beforeSize) }}<span class="arrow">→</span>{{ formatBytes(item.afterSize) }}
                  </span>
                </li>
              </ul>
            </div>
          </template>
          <p v-else class="muted">{{ t("version.detailEmpty") }}</p>
        </section>
      </div>
    </div>
  </section>
</template>

<style scoped>
.versions-page {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
  padding-bottom: 3rem;
}
.back-link {
  align-items: center;
  color: var(--muted-foreground);
  display: inline-flex;
  font-size: 12px;
  gap: 6px;
  text-decoration: none;
  width: fit-content;
}
.back-link:hover {
  color: var(--foreground);
}
.page-header {
  align-items: center;
  display: flex;
  gap: 1rem;
}
.page-header :deep(h2) {
  margin: 0;
}
.page-icon {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  color: var(--brand);
  display: inline-flex;
  flex-shrink: 0;
  height: 44px;
  justify-content: center;
  width: 44px;
}
.page-meta {
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 3px 0 0;
}
.notice {
  background: var(--surface-hover);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 0;
  padding: 8px 10px;
}
.notice.error {
  color: var(--danger, #d9534f);
}

/* ── 主栅格：左栏文件轨 + 右栏主区，占满容器宽度 ── */
.console {
  align-items: start;
  display: grid;
  gap: 16px;
  grid-template-columns: 288px minmax(0, 1fr);
  width: 100%;
}
.main {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 0;
}
.pane {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  container-type: inline-size;
  min-width: 0;
  padding: 12px 14px 14px;
}
.pane-head {
  align-items: center;
  display: flex;
  gap: 8px;
  justify-content: space-between;
  margin-bottom: 8px;
}
.pane-title {
  color: var(--muted-foreground);
  font-size: 10.5px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.pane-badge {
  background: var(--surface-hover);
  border-radius: 999px;
  color: var(--muted-foreground);
  font-size: 10.5px;
  font-variant-numeric: tabular-nums;
  padding: 1px 7px;
}
.muted {
  color: var(--muted-foreground);
  font-size: 12px;
  margin: 0;
}
.tabnum {
  font-variant-numeric: tabular-nums;
}
.sep {
  opacity: 0.45;
}
.arrow {
  color: var(--muted-foreground);
  margin: 0 3px;
  opacity: 0.7;
}

/* ── 左栏文件轨 ── */
.file-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.file-row {
  align-items: center;
  background: transparent;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--foreground);
  cursor: pointer;
  display: flex;
  gap: 9px;
  padding: 7px 8px;
  text-align: start;
  transition: background-color 120ms ease;
  width: 100%;
}
.file-row:hover {
  background: var(--surface-hover);
}
.file-row.active {
  background: var(--accent);
  box-shadow: inset 2px 0 0 var(--primary);
}
.file-icon {
  color: var(--muted-foreground);
  display: inline-flex;
  flex: none;
  transition: color 120ms ease;
}
.file-row.active .file-icon {
  color: var(--primary);
}
.file-body {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}
.file-name {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.file-meta {
  color: var(--muted-foreground);
  display: flex;
  font-size: 10.5px;
  gap: 5px;
}

/* ── 文件头（当前文件 + 动作） ── */
.file-bar {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  display: flex;
  gap: 16px;
  justify-content: space-between;
  padding: 10px 14px;
}
.file-ident {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}
.file-path {
  color: var(--foreground);
  font-family: ui-monospace, Consolas, monospace;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.file-sub {
  color: var(--muted-foreground);
  display: flex;
  font-size: 11px;
  gap: 5px;
}
.file-actions {
  align-items: center;
  display: flex;
  flex: none;
  gap: 12px;
}
.force-row {
  accent-color: var(--primary);
  align-items: center;
  color: var(--muted-foreground);
  cursor: pointer;
  display: flex;
  font-size: 11px;
  gap: 6px;
  white-space: nowrap;
}

/* ── 版本表 / 变更明细：各自定宽列 ── */
.rev-row {
  align-items: center;
  column-gap: 12px;
  display: grid;
  grid-template-columns:
    18px 56px minmax(120px, 1fr) 72px 148px 108px 236px;
  padding: 0 4px;
}
/* 显式列定位：表头/数据逐列锁定，子元素增减不会引起错位 */
.rev-row > :nth-child(1) { grid-column: 1; grid-row: 1; }
.rev-row > :nth-child(2) { grid-column: 2; grid-row: 1; }
.rev-row > :nth-child(3) { grid-column: 3; grid-row: 1; }
.rev-row > :nth-child(4) { grid-column: 4; grid-row: 1; }
.rev-row > :nth-child(5) { grid-column: 5; grid-row: 1; }
.rev-row > :nth-child(6) { grid-column: 6; grid-row: 1; }
.rev-row > :nth-child(7) { grid-column: 7; grid-row: 1; }
.rev-row > :nth-child(n + 8) { grid-column: 7; grid-row: 1; }
.diff-row {
  align-items: center;
  column-gap: 12px;
  display: grid;
  grid-template-columns: 64px minmax(0, 1fr) 148px;
  height: 32px;
  padding: 0 4px;
}
.row-head {
  border-bottom: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 10.5px;
  height: 26px;
  letter-spacing: 0.04em;
}
.end {
  text-align: end;
  white-space: nowrap;
}
.spine {
  list-style: none;
  margin: 0;
  padding: 0;
}
/* 每个节点自绘一段竖线：从本节点标记底部，接到下一节点标记顶部。
   高度用 min-height：窄容器下动作按钮换行时行随之长高，脊线按 % 自适应。 */
.node {
  height: 44px;
  overflow: hidden;
  position: relative;
}
.node:not(:last-child)::after {
  background: var(--border);
  content: "";
  height: calc(100% - 12px);
  left: 8.5px;
  position: absolute;
  top: calc(50% + 6px);
  width: 1px;
}
.node:hover {
  background: var(--surface-hover);
  border-radius: var(--radius-sm);
}
.node.active {
  background: var(--accent);
  border-radius: var(--radius-sm);
}
.marker {
  background: var(--surface);
  border: 1.5px solid var(--border-strong);
  border-radius: 50%;
  height: 11px;
  left: 3px;
  position: absolute;
  top: calc(50% - 5.5px);
  width: 11px;
}
/* 最新版 = 品牌色实心点，是脊线的锚 */
.node.head .marker {
  background: var(--primary);
  border-color: var(--primary);
}
.node.active .marker {
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--primary) 22%, transparent);
}
.cell-version {
  color: var(--foreground);
  font-family: ui-monospace, Consolas, monospace;
  font-size: 12px;
  font-weight: 700;
}
.cell-writer {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 11.5px;
  gap: 8px;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
}
.head-chip {
  background: var(--accent);
  border-radius: 999px;
  color: var(--accent-foreground);
  flex: none;
  font-size: 10px;
  font-weight: 700;
  padding: 1px 7px;
}
.branch {
  align-items: center;
  display: inline-flex;
  font-size: 10.5px;
  gap: 4px;
  opacity: 0.85;
}
.cell-time {
  color: var(--muted-foreground);
  font-size: 11px;
}
.node-actions {
  display: flex;
  flex-wrap: nowrap;
  gap: 4px;
  justify-content: flex-end;
  opacity: 0;
  translate: 4px 0;
  transition: opacity 140ms ease, translate 140ms ease;
}
.node:hover .node-actions,
.node:focus-within .node-actions {
  opacity: 1;
  translate: 0 0;
}

/* ── 变更明细：限高滚动 + 表头吸附 ── */
.diff-scroll {
  max-height: 360px;
  overflow: auto;
}
.diff-scroll .row-head {
  background: var(--surface);
  position: sticky;
  top: 0;
  z-index: 1;
}
.diff-list {
  display: flex;
  flex-direction: column;
  list-style: none;
  margin: 0;
  padding: 0;
}
.status {
  background: var(--surface-hover);
  border-radius: 999px;
  color: var(--muted-foreground);
  font-size: 10px;
  font-weight: 600;
  justify-self: start;
  padding: 2px 8px;
  white-space: nowrap;
}
/* git 语义配色：A=绿 / M=橙 / D=红 */
.status.added {
  background: color-mix(in srgb, var(--success, #3aa76d) 14%, transparent);
  color: var(--success, #3aa76d);
}
.status.modified {
  background: color-mix(in srgb, var(--warning, #c57c00) 16%, transparent);
  color: var(--warning, #c57c00);
}
.status.removed {
  background: color-mix(in srgb, var(--danger, #d9534f) 14%, transparent);
  color: var(--danger, #d9534f);
}
.tgi {
  color: var(--foreground);
  font-family: ui-monospace, Consolas, monospace;
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ── 控件 ── */
.ghost-button {
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  font-size: 11px;
  gap: 5px;
  height: 24px;
  padding: 0 9px;
  transition: background-color 120ms ease, border-color 120ms ease, color 120ms ease, scale 120ms ease;
  white-space: nowrap;
}
.ghost-button:active:not(:disabled) {
  scale: 0.96;
}
.ghost-button:hover:not(:disabled) {
  border-color: var(--border-strong);
  color: var(--foreground);
}
.ghost-button.accent:hover:not(:disabled) {
  border-color: color-mix(in srgb, var(--primary) 45%, transparent);
  color: var(--primary);
}
.ghost-button.danger:hover:not(:disabled) {
  border-color: var(--danger, #d9534f);
  color: var(--danger, #d9534f);
}
.ghost-button:disabled {
  cursor: default;
  opacity: 0.45;
}
.ghost-button.icon-only {
  padding: 0 6px;
}

/* 列降级按容器宽度而不是视口：版本表在双栏布局里被左栏挤压，
   视口还宽时表可能已经很窄。先收时间列，再收字节列。 */
@container (max-width: 1000px) {
  .rev-row {
    grid-template-columns: 18px 56px minmax(96px, 1fr) 72px 148px 236px;
  }
  .rev-row > .cell-time {
    display: none;
  }
}
@container (max-width: 800px) {
  .rev-row {
    grid-template-columns: 18px 56px minmax(96px, 1fr) 72px 236px;
  }
  .rev-row > .cell-bytes {
    display: none;
  }
  .node-actions {
    opacity: 1;
    translate: 0 0;
  }
}
/* 双栏放不下才并成一栏（视口级布局决策） */
@media (max-width: 1100px) {
  .console {
    grid-template-columns: minmax(0, 1fr);
  }
}
/* 触屏没有 hover，动作按钮常显 */
@media (hover: none) {
  .node-actions {
    opacity: 1;
    translate: 0 0;
  }
}
@media (prefers-reduced-motion: reduce) {
  .node-actions {
    transition: none;
  }
}
</style>
