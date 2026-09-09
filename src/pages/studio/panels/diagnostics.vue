<script setup lang="ts">
import { computed, onMounted } from "vue";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import { RouterLink } from "vue-router";
import FIcon from "@/components/extensions/FIcon.vue";
import FTypography from "@/components/extensions/FTypography.vue";
import FSkeleton from "@/components/ui/FSkeleton.vue";
import { isTauri } from "@/api";
import { useOverrideScanStore } from "@/stores/overrideScan";

const { t } = useI18n();
const scanStore = useOverrideScanStore();
const {
  gameDir,
  extraRoots,
  scanning,
  error,
  result,
  filter,
  expanded,
  roots,
  conflicts,
} = storeToRefs(scanStore);

const statItems = computed(() => [
  { key: "packages", value: result.value?.stats.packages ?? 0 },
  { key: "entries", value: result.value?.stats.entries ?? 0 },
  { key: "duplicatedTgis", value: result.value?.stats.duplicatedTgis ?? 0 },
  { key: "overriddenEntries", value: result.value?.stats.overriddenEntries ?? 0 },
]);
function formatBytes(value: number) {
  if (value >= 1024 * 1024) return `${(value / 1024 / 1024).toFixed(1)} MB`;
  if (value >= 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${value} B`;
}

onMounted(() => {
  void scanStore.initFromSettings();
});
</script>

<template>
  <section class="diagnostics-page">
    <RouterLink class="back-link" to="/studio">
      <FIcon name="ArrowLeft" :size="14" />
      {{ t("studio.backToStudio") }}
    </RouterLink>

    <header class="page-header">
      <span class="page-icon"><FIcon name="Shield" :size="20" /></span>
      <div>
        <FTypography :header="2" spacing="none">{{
          t("studio.panels.diagnostics.title")
        }}</FTypography>
        <p class="page-meta">{{ t("studio.panels.diagnostics.meta") }}</p>
      </div>
    </header>

    <p v-if="!isTauri()" class="notice" role="status">
      {{ t("studio.diagnostics.mockNotice") }}
    </p>

    <section class="scan-config" :aria-label="t('studio.diagnostics.configLabel')">
      <label class="field">
        <span class="field-label">{{ t("studio.diagnostics.gameDir") }}</span>
        <input
          v-model="gameDir"
          type="text"
          class="field-input mono"
          :placeholder="t('studio.diagnostics.gameDirPlaceholder')"
        />
      </label>
      <label class="field">
        <span class="field-label">{{ t("studio.diagnostics.extraRoots") }}</span>
        <input
          v-model="extraRoots"
          type="text"
          class="field-input mono"
          :placeholder="t('studio.diagnostics.extraRootsPlaceholder')"
        />
      </label>
      <button
        type="button"
        class="run-button"
        :disabled="scanning || !roots.length"
        @click="scanStore.runScan()"
      >
        <FIcon :name="scanning ? 'LoaderCircle' : 'Shield'" :size="15" />
        {{ scanning ? t("studio.diagnostics.scanning") : t("studio.diagnostics.run") }}
      </button>
    </section>

    <p v-if="error" class="error-message" role="alert">{{ error }}</p>

    <div v-if="scanning" class="skeleton-block">
      <FSkeleton v-for="index in 4" :key="index" height="52px" />
    </div>

    <template v-if="result && !scanning">
      <section class="stats-row" :aria-label="t('studio.diagnostics.statsLabel')">
        <article v-for="item in statItems" :key="item.key" class="stat-card">
          <span class="stat-value">{{ item.value.toLocaleString() }}</span>
          <span class="stat-label">{{
            t(`studio.diagnostics.stats.${item.key}`)
          }}</span>
        </article>
      </section>

      <p v-if="result.failed.length" class="notice" role="status">
        {{ t("studio.diagnostics.failedRoots") }}: {{ result.failed.join("; ") }}
      </p>

      <section class="conflicts" :aria-label="t('studio.diagnostics.conflictsLabel')">
        <div class="conflicts-toolbar">
          <h2 class="section-title">
            {{ t("studio.diagnostics.conflictsLabel") }}
            <span class="count-badge">{{ conflicts.length }}</span>
          </h2>
          <input
            v-model="filter"
            type="search"
            class="field-input slim"
            :placeholder="t('studio.diagnostics.filterPlaceholder')"
          />
        </div>

        <p v-if="!conflicts.length" class="empty-state">
          {{ t("studio.diagnostics.noConflicts") }}
        </p>

        <ul v-else class="conflict-list">
          <li v-for="(conflict, index) in conflicts.slice(0, 200)" :key="index" class="conflict-item">
            <button
              type="button"
              class="conflict-row"
              :aria-expanded="expanded.has(index)"
              @click="scanStore.toggleExpanded(index)"
            >
              <FIcon
                :name="expanded.has(index) ? 'ChevronDown' : 'ChevronRight'"
                :size="14"
              />
              <span class="conflict-ext">{{ conflict.ext }}</span>
              <span class="conflict-tgi mono">{{ scanStore.tgiText(conflict) }}</span>
              <span class="conflict-chain-count">
                {{ t("studio.diagnostics.chainCount", conflict.chain.length) }}
              </span>
            </button>
            <ol v-if="expanded.has(index)" class="chain-list">
              <li
                v-for="(item, chainIndex) in conflict.chain"
                :key="chainIndex"
                class="chain-item"
                :data-wins="item.wins"
              >
                <span class="chain-order mono">{{ chainIndex + 1 }}</span>
                <span class="chain-name" :title="item.path">{{ item.name }}</span>
                <span class="chain-size">{{ formatBytes(item.decompressedSize) }}</span>
                <span v-if="item.wins" class="chain-badge">{{
                  t("studio.diagnostics.wins")
                }}</span>
              </li>
            </ol>
          </li>
        </ul>
        <p v-if="conflicts.length > 200" class="more-hint">
          {{ t("studio.diagnostics.moreHint", conflicts.length - 200) }}
        </p>
      </section>
    </template>
  </section>
</template>

<style scoped>
.diagnostics-page {
  padding: 2.25rem 1.5rem 3rem;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}
.back-link {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.75rem;
  color: var(--muted-foreground);
  text-decoration: none;
  width: fit-content;
}
.back-link:hover {
  color: var(--foreground);
}
.page-header {
  display: flex;
  align-items: center;
  gap: 1rem;
}
.page-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  color: var(--brand);
  flex-shrink: 0;
}
.page-header :deep(h2) {
  margin: 0;
}
.page-meta {
  margin: 0.2rem 0 0;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  color: var(--subtle-foreground);
}
.notice,
.error-message {
  margin: 0;
  padding: 0.7rem 1rem;
  border-radius: var(--radius-md);
  font-size: 0.8125rem;
}
.notice {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--muted-foreground);
}
.error-message {
  border: 1px solid color-mix(in oklab, var(--danger) 45%, transparent);
  color: var(--danger);
}
.scan-config {
  display: flex;
  gap: 0.875rem;
  align-items: flex-end;
  flex-wrap: wrap;
  padding: 1.1rem 1.25rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
}
.field {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  flex: 1 1 240px;
}
.field-label {
  font-size: 0.75rem;
  color: var(--muted-foreground);
}
.field-input {
  padding: 0.5rem 0.7rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--background);
  color: var(--foreground);
  font-size: 0.8125rem;
}
.field-input:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}
.field-input.slim {
  flex: 0 1 220px;
  padding: 0.35rem 0.6rem;
}
.mono {
  font-family: var(--font-mono, ui-monospace, monospace);
}
.run-button {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.5rem 1rem;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: var(--primary);
  color: var(--primary-foreground);
  font-size: 0.8125rem;
  cursor: pointer;
}
.run-button:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
.run-button:not(:disabled):hover {
  filter: brightness(1.08);
}
.skeleton-block {
  display: grid;
  gap: 0.5rem;
}
.stats-row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 0.875rem;
}
.stat-card {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  padding: 1rem 1.1rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
}
.stat-value {
  font-size: 1.5rem;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.stat-label {
  font-size: 0.75rem;
  color: var(--muted-foreground);
}
.section-title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin: 0;
  font-size: 0.875rem;
  font-weight: 600;
}
.count-badge {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 0.6875rem;
  padding: 0.1rem 0.5rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--muted-foreground);
}
.conflicts-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
  margin-bottom: 0.75rem;
}
.empty-state {
  margin: 0;
  padding: 1.5rem;
  border: 1px dashed var(--border);
  border-radius: var(--radius-lg);
  text-align: center;
  font-size: 0.8125rem;
  color: var(--subtle-foreground);
}
.conflict-list {
  list-style: none;
  margin: 0;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  overflow: hidden;
}
.conflict-item + .conflict-item {
  border-top: 1px solid var(--border);
}
.conflict-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  width: 100%;
  padding: 0.6rem 0.9rem;
  border: none;
  background: var(--surface);
  color: var(--foreground);
  font-size: 0.8125rem;
  cursor: pointer;
  text-align: left;
}
.conflict-row:hover {
  background: color-mix(in oklab, var(--accent) 6%, var(--surface));
}
.conflict-ext {
  min-width: 5.5rem;
  font-weight: 500;
}
.conflict-tgi {
  color: var(--muted-foreground);
  font-size: 0.75rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.conflict-chain-count {
  margin-left: auto;
  color: var(--subtle-foreground);
  font-size: 0.75rem;
  white-space: nowrap;
}
.chain-list {
  list-style: none;
  margin: 0;
  padding: 0 0.5rem 0.7rem 2.2rem;
  display: grid;
  gap: 0.25rem;
}
.chain-item {
  display: flex;
  align-items: center;
  gap: 0.7rem;
  padding: 0.35rem 0.6rem;
  border-radius: var(--radius-sm);
  font-size: 0.75rem;
  background: color-mix(in oklab, var(--accent) 4%, transparent);
}
.chain-item[data-wins="true"] {
  background: color-mix(in oklab, var(--brand) 12%, transparent);
}
.chain-order {
  color: var(--subtle-foreground);
}
.chain-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.chain-size {
  margin-left: auto;
  color: var(--muted-foreground);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.chain-badge {
  font-size: 0.625rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  padding: 0.1rem 0.45rem;
  border-radius: 999px;
  border: 1px solid color-mix(in oklab, var(--brand) 50%, transparent);
  color: var(--brand);
  white-space: nowrap;
}
.more-hint {
  margin: 0.6rem 0 0;
  font-size: 0.75rem;
  color: var(--subtle-foreground);
}
</style>
