<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { useI18n } from "vue-i18n";
import { useModProjectsStore } from "@/stores/modProjects";
import { useBuildActivityStore } from "@/stores/buildActivity";
import FIcon from "@/components/extensions/FIcon.vue";
import FSelect from "@/components/ui/FSelect.vue";
import BuildActionsPanel from "./asset/BuildActionsPanel.vue";

/**
 * 项目管理页的 OpenSCP Actions 卡片。
 * 场景编辑器「构建」入队后可关闭；此处查看历史、步骤耗时与日志。
 */
const { t } = useI18n();
const store = useModProjectsStore();
const activity = useBuildActivityStore();
const { lastProjectRoot, lastJobId } = storeToRefs(activity);

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

function pickDefault() {
  if (
    lastProjectRoot.value &&
    options.value.some((option) => option.value === lastProjectRoot.value)
  ) {
    selectedRoot.value = lastProjectRoot.value;
    return;
  }
  if (!selectedRoot.value && options.value.length) {
    selectedRoot.value = options.value[0]?.value ?? "";
  }
}

watch(options, pickDefault, { immediate: true });
watch(lastProjectRoot, pickDefault);
</script>

<template>
  <section class="actions-card">
    <div class="card-head">
      <div class="card-brand">
        <span class="card-mark" aria-hidden="true">
          <FIcon name="Activity" :size="16" aria-label="" />
        </span>
        <div class="card-head-text">
          <h3>{{ $t("studio.assetWorkbench.actionsCardTitle") }}</h3>
          <small>{{
            selectedRoot
              ? t("studio.assetWorkbench.actionsCardHintWith", {
                  project: selectedLabel,
                })
              : t("studio.assetWorkbench.actionsCardHint")
          }}</small>
        </div>
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
    <BuildActionsPanel
      v-else-if="selectedRoot"
      :project="selectedRoot"
      :focus-job-id="selectedRoot === lastProjectRoot ? lastJobId : null"
    />
  </section>
</template>

<style scoped>
.actions-card {
  background:
    linear-gradient(
      180deg,
      color-mix(in srgb, var(--surface-elevated) 70%, transparent),
      var(--surface)
    );
  border: 1px solid var(--border);
  border-radius: 14px;
  box-shadow:
    var(--shadow-sm),
    0 0 0 1px color-mix(in srgb, white 3%, transparent) inset;
  display: flex;
  flex-direction: column;
  gap: 14px;
  margin-top: 16px;
  overflow: hidden;
  padding: 16px 18px 14px;
}
.card-head {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  justify-content: space-between;
}
.card-brand {
  align-items: center;
  display: flex;
  gap: 12px;
  min-width: 0;
}
.card-mark {
  align-items: center;
  background: color-mix(in srgb, var(--primary, #2563eb) 16%, transparent);
  border: 1px solid color-mix(in srgb, var(--primary, #2563eb) 28%, transparent);
  border-radius: 10px;
  color: var(--primary, #60a5fa);
  display: inline-flex;
  flex-shrink: 0;
  height: 34px;
  justify-content: center;
  width: 34px;
}
.card-head-text h3 {
  font-size: 15px;
  font-weight: 600;
  letter-spacing: -0.01em;
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
  min-width: 220px;
}
.card-note {
  color: var(--subtle-foreground);
  font-size: 12px;
  margin: 0;
}
</style>
