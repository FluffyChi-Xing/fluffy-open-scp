<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useModProjectsStore } from "@/stores/modProjects";
import FSelect from "@/components/ui/FSelect.vue";
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
const selectedLabel = computed(
  () =>
    options.value.find((option) => option.value === selectedRoot.value)?.label ??
    t("studio.assetWorkbench.actionsCardPick"),
);


</script>

<template>
  <section class="actions-card">
    <div class="card-head">
      <div class="card-head-text">
        <h3>{{ $t("studio.assetWorkbench.actionsCardTitle") }}</h3>
        <small>{{ selectedRoot
          ? t("studio.assetWorkbench.actionsCardHintWith", { project: selectedLabel })
          : t("studio.assetWorkbench.actionsCardHint") }}</small>
      </div>
      <label v-if="options.length" class="project-row">
        <span>{{ t("studio.assetWorkbench.actionsCardProject") }}</span>
        <FSelect
          v-model="selectedRoot"
          class="project-select"
          :options="options"
          :aria-label="t('studio.assetWorkbench.actionsCardProject')"
        />
      </label>
    </div>
    <p v-if="!options.length" class="card-note">
      {{ t("studio.assetWorkbench.actionsCardNoProjects") }}
    </p>
    <BuildActionsPanel v-else-if="selectedRoot" :project="selectedRoot" />
  </section>
</template>

<style scoped>
.actions-card {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-top: 16px;
  padding: 16px 18px;
}
.card-head {
  align-items: end;
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  justify-content: space-between;
}
.card-head-text h3 {
  font-size: 15px;
  margin: 0 0 2px;
}
.card-head-text small {
  color: var(--muted-foreground);
  font-size: 12px;
}
.project-row {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  flex-shrink: 0;
  font-size: 12px;
  gap: 8px;
  white-space: nowrap;
}
.project-row > span {
  flex-shrink: 0;
  white-space: nowrap;
}
.project-select {
  min-width: 260px;
}
.card-note {
  color: var(--subtle-foreground);
  font-size: 12px;
  margin: 0;
}
</style>
