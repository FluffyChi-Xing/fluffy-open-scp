<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useModProjectsStore } from "@/stores/modProjects";
import BuildActionsPanel from "./asset/BuildActionsPanel.vue";

/**
 * 项目管理页的 OpenSCP Actions 卡片(P2,2026-11-04 用户指令:构建后台化
 * 之后,Actions 归项目管理页而非资产编辑器——编辑器点了打包即关闭)。
 * 选择项目 → 复用 BuildActionsPanel 查看该项目的构建运行列表/阶段详情。
 */
const { t } = useI18n();
const store = useModProjectsStore();

const options = computed(() =>
  store.projects
    .filter((project) => project.folderPath)
    .map((project) => ({
      label: project.name,
      value: project.folderPath as string,
    })),
);
const selectedRoot = ref("");

function onProjectChange(event: Event) {
  selectedRoot.value = (event.target as HTMLSelectElement).value;
}
</script>

<template>
  <section class="actions-card">
    <div class="card-head">
      <h3>{{ $t("studio.assetWorkbench.actionsCardTitle") }}</h3>
      <small>{{ $t("studio.assetWorkbench.actionsCardHint") }}</small>
    </div>
    <p v-if="!options.length" class="card-note">
      {{ t("studio.assetWorkbench.actionsCardNoProjects") }}
    </p>
    <template v-else>
      <label class="project-row">
        <span>{{ t("studio.assetWorkbench.actionsCardProject") }}</span>
        <select class="project-select" :value="selectedRoot" @change="onProjectChange">
          <option value="">{{ t("studio.assetWorkbench.actionsCardPick") }}</option>
          <option v-for="option in options" :key="option.value" :value="option.value">
            {{ option.label }}
          </option>
        </select>
      </label>
      <BuildActionsPanel v-if="selectedRoot" :project="selectedRoot" />
    </template>
  </section>
</template>

<style scoped>
.actions-card {
  background: var(--surface, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-md, 10px);
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 16px;
  padding: 14px;
}
.card-head h3 {
  font-size: 14px;
  margin: 0;
}
.card-head small {
  color: var(--muted-foreground);
}
.card-note {
  color: var(--subtle-foreground);
  font-size: 12px;
  margin: 4px 0;
}
.project-row {
  align-items: center;
  display: flex;
  font-size: 12px;
  gap: 8px;
}
.project-select {
  background: var(--surface-elevated);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  min-height: 28px;
  min-width: 220px;
  padding: 0 8px;
}
</style>
